//! The Warren tunnel is a Quinn tunnel keyed by the Ed25519 wallet
//! pubkey, authenticated with the wallet key alone.
//!
//! The account manager tracks the **login state**
//! (logged-in wallet pubkey / logged-out / revoked) and drives the
//! account/subscription flows (login, logout, delete, voucher, expiry)
//! through [`WarrenIdentityService`].
//!
//! **Login-state persistence**: the login state is cached in
//! `device.json` as a pubkey-only [`PrivateDeviceState`]
//! (`LoggedIn(WarrenPubKey)` / `LoggedOut` / `Revoked`). We keep a
//! persistent cache (rather than re-deriving from the mnemonic at boot)
//! so that an explicit logout / revocation survives a restart even when
//! a mnemonic file is still present on disk.

use chrono::{DateTime, Utc};
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
};

use mullvad_api::rest;
#[cfg(target_os = "android")]
use mullvad_types::account::{
    PlayExternalObfuscatedAccountId, PlayPurchase, PlayPurchasePaymentToken,
};
use mullvad_types::{
    account::{AccountNumber, VoucherSubmission},
    device::{DeviceEvent, DeviceEventCause, DeviceState},
    warren_identity::WarrenIdentity,
    warren_pubkey::WarrenPubKey,
};

use std::{future::Future, path::Path, sync::Arc};
use talpid_core::mpsc::Sender;
use talpid_types::ErrorExt;
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

mod account_backend;
mod api;
mod service;
pub(crate) use account_backend::{
    RemoteAccountBackend, WarrenAccountBackend, WarrenRemoteAccountBackend,
};
pub(crate) use service::WarrenIdentityService;

/// Config needed to instantiate the Warren-Remote account backend
/// (= warren-api path). Built by the caller (`Daemon::start`) at boot.
/// `None` falls back to the legacy [`RemoteAccountBackend`] (no
/// mnemonic/signing key available).
///
/// Convention:
/// - `url`: `http(s)://host:port`, without trailing slash.
/// - `signing_key`: Warren identity loaded from the mnemonic
///   (`warren_signer::load_or_create_signing_key`).
#[derive(Clone)]
pub(crate) struct WarrenApiConfig {
    pub url: String,
    // Shared, hot-swappable key handle (cloned from the daemon's
    // `WarrenAuthSigner::shared`). The account-backend client built from
    // it tracks create/restore/logout identity changes without a restart.
    pub signing_key: std::sync::Arc<std::sync::RwLock<ed25519_dalek::SigningKey>>,
}

/// File that stores the account login state.
pub(crate) const DEVICE_CACHE_FILENAME: &str = "device.json";

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("No account is set")]
    NoDevice,
    #[error("Invalid account")]
    InvalidAccount,
    #[error("Account not found")]
    InvalidDevice,
    #[error("Invalid voucher code")]
    InvalidVoucher,
    #[error("The voucher has already been used")]
    UsedVoucher,
    #[error("Failed to read or write account cache")]
    DeviceIoError(#[from] Arc<io::Error>),
    #[error("Failed parse account cache")]
    ParseDeviceCache(#[from] Arc<serde_json::Error>),
    #[error("Unexpected HTTP request error")]
    OtherRestError(#[from] rest::Error),
    #[error("Account changed during operation")]
    AccountChange,
    #[error("The account manager is down")]
    AccountManagerDown,
}

macro_rules! impl_into_arc_err {
    ($ty:ty) => {
        impl From<$ty> for Error {
            fn from(error: $ty) -> Self {
                Error::from(Arc::from(error))
            }
        }
    };
}

impl_into_arc_err!(io::Error);
impl_into_arc_err!(serde_json::Error);

/// Contains the current account login state.
///
/// The `LoggedIn` variant carries the wallet pubkey.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrivateDeviceState {
    LoggedIn(WarrenPubKey),
    LoggedOut,
    Revoked,
}

impl PrivateDeviceState {
    /// Returns whether the account is in the logged-in state.
    pub fn logged_in(&self) -> bool {
        matches!(self, PrivateDeviceState::LoggedIn(_))
    }

    /// Returns whether the state is logged out, as opposed to
    /// logged in or revoked.
    pub fn logged_out(&self) -> bool {
        matches!(self, PrivateDeviceState::LoggedOut)
    }

    /// Returns the logged-in wallet pubkey, if any.
    pub fn pubkey(&self) -> Option<&WarrenPubKey> {
        match self {
            PrivateDeviceState::LoggedIn(pubkey) => Some(pubkey),
            _ => None,
        }
    }

    /// Sets the state to `Revoked`.
    fn revoke(&mut self) {
        *self = PrivateDeviceState::Revoked;
    }

    /// Sets the state to `LoggedOut` and returns the logged-in pubkey, if any.
    fn logout(&mut self) -> Option<WarrenPubKey> {
        match std::mem::replace(self, PrivateDeviceState::LoggedOut) {
            PrivateDeviceState::LoggedIn(pubkey) => Some(pubkey),
            _ => None,
        }
    }
}

impl From<PrivateDeviceState> for DeviceState {
    fn from(state: PrivateDeviceState) -> Self {
        match state {
            PrivateDeviceState::LoggedIn(pubkey) => {
                DeviceState::LoggedIn(WarrenIdentity::new(pubkey))
            }
            PrivateDeviceState::LoggedOut => DeviceState::LoggedOut,
            PrivateDeviceState::Revoked => DeviceState::Revoked,
        }
    }
}

/// Internal helper that parses an `AccountNumber` (= `String`) received
/// from an **external interface** (gRPC client, server API, legacy
/// settings) into a `WarrenPubKey`. If the string is not a valid Warren
/// pubkey, we log a `warn!` and return a zero `WarrenPubKey` (= all
/// requests signed with this pubkey will fail server-side, but the
/// daemon does not crash).
pub(crate) fn account_number_to_warren_pubkey(account_number: &str) -> WarrenPubKey {
    use std::str::FromStr;
    WarrenPubKey::from_str(account_number).unwrap_or_else(|e| {
        log::warn!(
            "account_number is not a valid Warren pubkey ({e}), fallback dummy zero \
             pubkey — relogin required for Warren auth",
        );
        WarrenPubKey::from_bytes(&[0u8; 32])
    })
}

#[derive(Clone)]
pub(crate) enum AccountEvent {
    /// Emitted when the login state changes.
    Device(PrivateDeviceEvent),
    /// Emitted when the account expiry is fetched.
    Expiry(DateTime<Utc>),
}

#[derive(Clone)]
pub(crate) enum PrivateDeviceEvent {
    // (Login carries the wallet pubkey; the others are unit variants.)
    /// Logged in on a new account.
    Login(WarrenPubKey),
    /// The account was logged out due to user (or daemon) action.
    Logout,
    /// The account was revoked because it was not found remotely.
    Revoked,
}

impl From<PrivateDeviceEvent> for DeviceEvent {
    fn from(event: PrivateDeviceEvent) -> DeviceEvent {
        let cause = match event {
            PrivateDeviceEvent::Login(_) => DeviceEventCause::LoggedIn,
            PrivateDeviceEvent::Logout => DeviceEventCause::LoggedOut,
            PrivateDeviceEvent::Revoked => DeviceEventCause::Revoked,
        };
        let new_state = DeviceState::from(event.state());
        DeviceEvent { cause, new_state }
    }
}

impl PrivateDeviceEvent {
    pub fn state(self) -> PrivateDeviceState {
        match self {
            PrivateDeviceEvent::Login(pubkey) => PrivateDeviceState::LoggedIn(pubkey),
            PrivateDeviceEvent::Logout => PrivateDeviceState::LoggedOut,
            PrivateDeviceEvent::Revoked => PrivateDeviceState::Revoked,
        }
    }
}

impl Error {
    pub fn is_network_error(&self) -> bool {
        matches!(self, Error::OtherRestError(error) if error.is_network_error())
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self, Error::OtherRestError(error) if error.is_aborted())
    }
}

type ResponseTx<T> = oneshot::Sender<Result<T, Error>>;

enum AccountManagerCommand {
    Login(AccountNumber, ResponseTx<()>),
    Logout(ResponseTx<()>),
    GetData(ResponseTx<PrivateDeviceState>),
    SubmitVoucher(String, ResponseTx<VoucherSubmission>),
    #[cfg(target_os = "android")]
    InitPlayPurchase(ResponseTx<PlayExternalObfuscatedAccountId>),
    #[cfg(target_os = "android")]
    VerifyPlayPurchase(ResponseTx<()>, PlayPurchase),
    CheckExpiry(ResponseTx<DateTime<Utc>>),
    #[cfg(target_os = "android")]
    Delete(ResponseTx<()>),
    Shutdown(oneshot::Sender<()>),
}

#[derive(Clone)]
pub(crate) struct AccountManagerHandle {
    cmd_tx: mpsc::UnboundedSender<AccountManagerCommand>,
    pub warren_identity_service: WarrenIdentityService,
}

impl AccountManagerHandle {
    pub async fn login(&self, number: AccountNumber) -> Result<(), Error> {
        self.send_command(|tx| AccountManagerCommand::Login(number, tx))
            .await
    }

    pub async fn logout(&self) -> Result<(), Error> {
        self.send_command(AccountManagerCommand::Logout).await
    }

    pub async fn data(&self) -> Result<PrivateDeviceState, Error> {
        self.send_command(AccountManagerCommand::GetData).await
    }

    pub async fn submit_voucher(&self, voucher: String) -> Result<VoucherSubmission, Error> {
        self.send_command(move |tx| AccountManagerCommand::SubmitVoucher(voucher, tx))
            .await
    }

    pub async fn check_expiry(&self) -> Result<DateTime<Utc>, Error> {
        self.send_command(AccountManagerCommand::CheckExpiry).await
    }

    #[cfg(target_os = "android")]
    pub async fn init_play_purchase(&self) -> Result<PlayExternalObfuscatedAccountId, Error> {
        self.send_command(AccountManagerCommand::InitPlayPurchase)
            .await
    }

    #[cfg(target_os = "android")]
    pub async fn verify_play_purchase(&self, play_purchase: PlayPurchase) -> Result<(), Error> {
        self.send_command(move |tx| AccountManagerCommand::VerifyPlayPurchase(tx, play_purchase))
            .await
    }

    #[cfg(target_os = "android")]
    pub async fn delete(&self) -> Result<(), Error> {
        self.send_command(AccountManagerCommand::Delete).await
    }

    pub async fn shutdown(self) {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .unbounded_send(AccountManagerCommand::Shutdown(tx));
        let _ = rx.await;
    }

    async fn send_command<T>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<Result<T, Error>>) -> AccountManagerCommand,
    ) -> Result<T, Error> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .unbounded_send(make_cmd(tx))
            .map_err(|_| Error::AccountManagerDown)?;
        rx.await.map_err(|_| Error::AccountManagerDown)?
    }
}

pub(crate) struct AccountManager {
    cacher: DeviceCacher,
    warren_identity_service: WarrenIdentityService,
    data: PrivateDeviceState,
    listeners: Vec<Box<dyn Sender<AccountEvent> + Send>>,
    expiry_requests: Vec<ResponseTx<DateTime<Utc>>>,
}

impl AccountManager {
    /// Starts the account manager actor and returns a handle to it as well as the
    /// current login state.
    pub async fn spawn(
        rest_handle: rest::MullvadRestHandle,
        settings_dir: &Path,
        listener_tx: impl Sender<AccountEvent> + Send + 'static,
        warren_api_config: Option<WarrenApiConfig>,
    ) -> Result<(AccountManagerHandle, PrivateDeviceState), Error> {
        let (cacher, data) = DeviceCacher::new(settings_dir).await?;
        let number = data.pubkey().map(|pubkey| pubkey.as_str().to_owned());
        let api_availability = rest_handle.availability.clone();
        // 2-branch dispatch of the account backend:
        // 1. `warren_api_config = Some(_)` -> [`WarrenRemoteAccountBackend`]
        //    (talks to warren-api via signed HTTP Ed25519). This is the
        //    standard Warren path: the real subscription expiry is read
        //    from `GET /v1/subscription`.
        // 2. Otherwise (no mnemonic/signing key) -> [`RemoteAccountBackend`]
        //    (legacy Mullvad upstream path).
        let account_backend: std::sync::Arc<dyn WarrenAccountBackend> =
            if let Some(cfg) = warren_api_config {
                let client =
                    warren_api_client::WarrenApiClient::new_shared(cfg.url, Vec::new(), cfg.signing_key);
                std::sync::Arc::new(WarrenRemoteAccountBackend::new(client))
            } else {
                std::sync::Arc::new(RemoteAccountBackend::new(mullvad_api::AccountsProxy::new(
                    rest_handle.clone(),
                )))
            };
        let warren_identity_service = service::spawn_warren_identity_service(
            rest_handle.clone(),
            number,
            api_availability.clone(),
            account_backend,
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded();

        let manager = AccountManager {
            cacher,
            warren_identity_service: warren_identity_service.clone(),
            data: data.clone(),
            listeners: vec![Box::new(listener_tx)],
            expiry_requests: vec![],
        };

        tokio::spawn(manager.run(cmd_rx));
        let handle = AccountManagerHandle {
            cmd_tx,
            warren_identity_service,
        };
        Ok((handle, data))
    }

    async fn run(mut self, mut cmd_rx: mpsc::UnboundedReceiver<AccountManagerCommand>) {
        let mut shutdown_tx = None;
        let mut current_api_call = api::CurrentApiCall::new();

        loop {
            futures::select! {
                api_result = current_api_call => {
                    self.consume_api_result(api_result).await;
                }

                cmd = cmd_rx.next() => {
                    match cmd {
                        Some(AccountManagerCommand::Shutdown(tx)) => {
                            shutdown_tx = Some(tx);
                            break;
                        }
                        Some(AccountManagerCommand::Login(number, tx)) => {
                            // Login adopts the wallet pubkey as the
                            // logged-in identity.
                            let pubkey = account_number_to_warren_pubkey(&number);
                            let _ = tx.send(self.set(PrivateDeviceEvent::Login(pubkey)).await);
                        }
                        Some(AccountManagerCommand::Logout(tx)) => {
                            current_api_call.clear();
                            self.logout(tx).await;
                        }
                        Some(AccountManagerCommand::GetData(tx)) => {
                            let _ = tx.send(Ok(self.data.clone()));
                        }
                        Some(AccountManagerCommand::SubmitVoucher(voucher, tx)) => {
                            self.handle_voucher_submission(tx, voucher, &mut current_api_call);
                        },
                        Some(AccountManagerCommand::CheckExpiry(tx)) => {
                            self.handle_expiry_request(tx, &mut current_api_call);
                        },
                        #[cfg(target_os = "android")]
                        Some(AccountManagerCommand::InitPlayPurchase(tx)) => {
                            self.handle_init_play_purchase(tx, &mut current_api_call);
                        },
                        #[cfg(target_os = "android")]
                        Some(AccountManagerCommand::VerifyPlayPurchase(tx, play_purchase)) => {
                            self.handle_verify_play_purchase(tx, play_purchase, &mut current_api_call);
                        },
                        #[cfg(target_os = "android")]
                        Some(AccountManagerCommand::Delete(tx)) => {
                            current_api_call.clear();
                            self.delete(tx).await;
                        },

                        None => {
                            break;
                        }
                    }
                }
            }
        }
        self.shutdown().await;
        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }
        log::debug!("Account manager has stopped");
    }

    fn handle_voucher_submission(
        &mut self,
        tx: ResponseTx<VoucherSubmission>,
        voucher: String,
        current_api_call: &mut api::CurrentApiCall,
    ) {
        if current_api_call.is_logging_in() {
            let _ = tx.send(Err(Error::AccountChange));
            return;
        }

        let create_submission = move || {
            let pubkey = self.data.pubkey().ok_or(Error::NoDevice)?.clone();
            let warren_identity_service = self.warren_identity_service.clone();
            Ok(async move { warren_identity_service.submit_voucher(pubkey, voucher).await })
        };

        match create_submission() {
            Ok(call) => {
                current_api_call.set_voucher_submission(Box::pin(call), tx);
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    #[cfg(target_os = "android")]
    fn handle_init_play_purchase(
        &mut self,
        tx: ResponseTx<PlayPurchasePaymentToken>,
        current_api_call: &mut api::CurrentApiCall,
    ) {
        if current_api_call.is_logging_in() {
            let _ = tx.send(Err(Error::AccountChange));
            return;
        }

        let init_play_purchase_api_call = move || {
            let pubkey = self.data.pubkey().ok_or(Error::NoDevice)?.clone();
            let warren_identity_service = self.warren_identity_service.clone();
            Ok(async move { warren_identity_service.init_play_purchase(pubkey).await })
        };

        match init_play_purchase_api_call() {
            Ok(call) => {
                current_api_call.set_init_play_purchase(Box::pin(call), tx);
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    fn handle_expiry_request(
        &mut self,
        tx: ResponseTx<DateTime<Utc>>,
        current_api_call: &mut api::CurrentApiCall,
    ) {
        if current_api_call.is_logging_in() {
            let _ = tx.send(Err(Error::AccountChange));
            return;
        }
        if current_api_call.is_checking_expiry() {
            self.expiry_requests.push(tx);
            return;
        }

        match self.expiry_call() {
            Ok(call) => {
                current_api_call.set_expiry_check(Box::pin(call));
                self.expiry_requests.push(tx);
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    #[cfg(target_os = "android")]
    fn handle_verify_play_purchase(
        &mut self,
        tx: ResponseTx<()>,
        play_purchase: PlayPurchase,
        current_api_call: &mut api::CurrentApiCall,
    ) {
        if current_api_call.is_logging_in() {
            let _ = tx.send(Err(Error::AccountChange));
            return;
        }

        let play_purchase_verify_api_call = move || {
            let pubkey = self.data.pubkey().ok_or(Error::NoDevice)?.clone();
            let warren_identity_service = self.warren_identity_service.clone();
            Ok(async move {
                warren_identity_service
                    .verify_play_purchase(pubkey, play_purchase)
                    .await
            })
        };

        match play_purchase_verify_api_call() {
            Ok(call) => {
                current_api_call.set_verify_play_purchase(Box::pin(call), tx);
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    async fn consume_api_result(&mut self, result: api::ApiResult) {
        use api::ApiResult::*;
        match result {
            VoucherSubmission(data_response, tx) => {
                self.consume_voucher_result(data_response, tx).await
            }
            ExpiryCheck(data_response) => self.consume_expiry_result(data_response).await,
            #[cfg(target_os = "android")]
            InitPlayPurchase(data_response, tx) => {
                self.consume_init_play_purchase_result(data_response, tx)
                    .await
            }
            #[cfg(target_os = "android")]
            VerifyPlayPurchase(data_response, tx) => {
                self.consume_verify_play_purchase_result(data_response, tx)
                    .await
            }
        }
    }

    async fn consume_voucher_result(
        &mut self,
        response: Result<VoucherSubmission, Error>,
        tx: ResponseTx<VoucherSubmission>,
    ) {
        match &response {
            Ok(submission) => {
                // Send expiry update event
                let event = AccountEvent::Expiry(submission.new_expiry);
                self.listeners
                    .retain(|listener| listener.send(event.clone()).is_ok());
            }
            Err(Error::InvalidAccount) => {
                self.revoke_device(|| Error::InvalidAccount).await;
            }
            Err(Error::InvalidDevice) => {
                self.revoke_device(|| Error::InvalidDevice).await;
            }
            Err(err) => log::error!("Failed to submit voucher: {}", err),
        }
        let _ = tx.send(response);
    }

    async fn consume_expiry_result(&mut self, response: Result<DateTime<Utc>, Error>) {
        match response {
            Ok(expiry) => {
                if expiry > chrono::Utc::now() {
                    log::debug!("Account has time left");
                } else {
                    log::debug!("Account has no time left");
                }

                // Send expiry update event
                let event = AccountEvent::Expiry(expiry);
                self.listeners
                    .retain(|listener| listener.send(event.clone()).is_ok());

                Self::drain_requests(&mut self.expiry_requests, || Ok(expiry));
            }
            Err(Error::InvalidAccount) => {
                self.revoke_device(|| Error::InvalidAccount).await;
            }
            Err(Error::InvalidDevice) => {
                self.revoke_device(|| Error::InvalidDevice).await;
            }
            Err(err) => {
                log::error!("Failed to check account expiry: {}", err);
                Self::drain_requests(&mut self.expiry_requests, || Err(err.clone()));
            }
        }
    }

    #[cfg(target_os = "android")]
    async fn consume_init_play_purchase_result(
        &mut self,
        response: Result<PlayPurchasePaymentToken, Error>,
        tx: ResponseTx<PlayPurchasePaymentToken>,
    ) {
        match &response {
            Ok(_) => (),
            Err(Error::InvalidAccount) => {
                self.revoke_device(|| Error::InvalidAccount).await;
            }
            Err(Error::InvalidDevice) => {
                self.revoke_device(|| Error::InvalidDevice).await;
            }
            Err(err) => log::error!("Failed to initialize play purchase: {}", err),
        }
        let _ = tx.send(response);
    }

    #[cfg(target_os = "android")]
    async fn consume_verify_play_purchase_result(
        &mut self,
        response: Result<(), Error>,
        tx: ResponseTx<()>,
    ) {
        match &response {
            Ok(_) => (),
            Err(Error::InvalidAccount) => {
                self.revoke_device(|| Error::InvalidAccount).await;
            }
            Err(Error::InvalidDevice) => {
                self.revoke_device(|| Error::InvalidDevice).await;
            }
            Err(err) => log::error!("Failed to verify play purchase: {}", err),
        }
        let _ = tx.send(response);
    }

    fn drain_requests<T>(requests: &mut Vec<ResponseTx<T>>, result: impl Fn() -> Result<T, Error>) {
        for req in requests.drain(0..) {
            let _ = req.send(result());
        }
    }

    async fn revoke_device(&mut self, err_constructor: impl Fn() -> Error) {
        log::debug!("Invalidating the current account");

        if let Err(err) = self.cacher.write(&PrivateDeviceState::Revoked).await {
            log::error!(
                "{}",
                err.display_chain_with_msg("Failed to save account data to disk")
            );
        }
        self.data.revoke();

        Self::drain_requests(&mut self.expiry_requests, || Err(err_constructor()));

        self.listeners.retain(|listener| {
            listener
                .send(AccountEvent::Device(PrivateDeviceEvent::Revoked))
                .is_ok()
        });
    }

    async fn logout(&mut self, tx: ResponseTx<()>) {
        if self.data.logged_out() {
            let _ = tx.send(Ok(()));
            return;
        }
        if let Err(err) = self.cacher.write(&PrivateDeviceState::LoggedOut).await {
            let _ = tx.send(Err(err));
            return;
        }

        self.data.logout();

        self.listeners.retain(|listener| {
            listener
                .send(AccountEvent::Device(PrivateDeviceEvent::Logout))
                .is_ok()
        });

        let _ = tx.send(Ok(()));
    }

    #[cfg(target_os = "android")]
    async fn delete(&mut self, tx: ResponseTx<()>) {
        if self.data.logged_out() {
            let _ = tx.send(Err(Error::AccountChange));
            return;
        }

        let old_pubkey = self.data.pubkey().cloned().ok_or(Error::NoDevice);

        let Ok(old_pubkey) = old_pubkey else {
            let _ = tx.send(old_pubkey.map(|_| ()));
            return;
        };

        let service = self.warren_identity_service.clone();
        match service.delete_account(old_pubkey).await {
            Ok(_) => {
                if let Err(err) = self.cacher.write(&PrivateDeviceState::LoggedOut).await {
                    let _ = tx.send(Err(err));
                    return;
                }
                self.data.logout();
                self.listeners.retain(|listener| {
                    listener
                        .send(AccountEvent::Device(PrivateDeviceEvent::Logout))
                        .is_ok()
                });
                let _ = tx.send(Ok(()));
            }
            Err(err) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    async fn set(&mut self, event: PrivateDeviceEvent) -> Result<(), Error> {
        let device_state = event.clone().state();
        if device_state == self.data {
            return Ok(());
        }

        self.cacher.write(&device_state).await?;
        self.data = device_state;

        let event = AccountEvent::Device(event);
        self.listeners
            .retain(|listener| listener.send(event.clone()).is_ok());

        Ok(())
    }

    fn expiry_call(
        &self,
    ) -> Result<impl Future<Output = Result<chrono::DateTime<Utc>, Error>> + use<>, Error> {
        let pubkey = self.data.pubkey().ok_or(Error::NoDevice)?.clone();
        let warren_identity_service = self.warren_identity_service.clone();
        Ok(async move {
            warren_identity_service
                .get_data_2(pubkey)
                .await
                .map(|data| data.expiry)
        })
    }

    async fn shutdown(self) {
        self.cacher.finalize().await;
    }
}

pub struct DeviceCacher {
    file: io::BufWriter<fs::File>,
    path: std::path::PathBuf,
}

impl DeviceCacher {
    pub async fn new(settings_dir: &Path) -> Result<(DeviceCacher, PrivateDeviceState), Error> {
        let path = settings_dir.join(DEVICE_CACHE_FILENAME);
        let cache_exists = path.is_file();
        let mut should_save = false;

        let mut file = fs::OpenOptions::from(Self::file_options())
            .write(true)
            .read(true)
            .create(true)
            .open(&path)
            .await?;

        let device: PrivateDeviceState = if cache_exists {
            let mut reader = io::BufReader::new(&mut file);
            let mut buffer = String::new();
            reader.read_to_string(&mut buffer).await?;
            if !buffer.is_empty() {
                serde_json::from_str(&buffer).unwrap_or_else(|error| {
                    should_save = true;
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Wiping account config due to an error")
                    );
                    PrivateDeviceState::LoggedOut
                })
            } else {
                should_save = true;
                PrivateDeviceState::LoggedOut
            }
        } else {
            should_save = true;
            PrivateDeviceState::LoggedOut
        };

        let mut store = DeviceCacher {
            file: io::BufWriter::new(file),
            path,
        };

        if should_save {
            store.write(&device).await?;
        }

        Ok((store, device))
    }

    fn file_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // exclusive access
            options.share_mode(0);
        }
        options
    }

    pub async fn write(&mut self, device: &PrivateDeviceState) -> Result<(), Error> {
        let data = serde_json::to_vec_pretty(&device).unwrap();

        self.file.get_mut().set_len(0).await?;
        self.file.seek(io::SeekFrom::Start(0)).await?;
        self.file.write_all(&data).await?;
        self.file.flush().await?;
        self.file.get_mut().sync_data().await?;

        Ok(())
    }

    pub async fn remove(self) -> Result<(), Error> {
        let path = {
            let DeviceCacher { path, file } = self;
            let std_file = file.into_inner().into_std().await;
            let _ = tokio::task::spawn_blocking(move || drop(std_file)).await;
            path
        };
        tokio::fs::remove_file(path).await?;
        Ok(())
    }

    async fn finalize(self) {
        let std_file = self.file.into_inner().into_std().await;
        let _ = tokio::task::spawn_blocking(move || drop(std_file)).await;
    }
}
