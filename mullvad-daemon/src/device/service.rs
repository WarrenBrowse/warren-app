use std::{future::Future, sync::Arc, time::Duration};

use chrono::Utc;
use futures::future::{AbortHandle, abortable};
#[cfg(target_os = "android")]
use mullvad_types::account::{PlayPurchase, PlayPurchasePaymentToken};
use mullvad_types::{
    account::{AccountData, AccountNumber, VoucherSubmission},
    device::{Device, DeviceId},
    warren_pubkey::WarrenPubKey,
    wireguard::WireguardData,
};
use talpid_types::net::wireguard::PrivateKey;

use super::{
    Error, PrivateAccountAndDevice, PrivateDevice, WarrenAccountBackend, WarrenDeviceBackend,
};
use mullvad_api::{
    AccountsProxy,
    availability::ApiAvailability,
    rest::{self, MullvadRestHandle},
};
use talpid_future::retry::{ConstantInterval, ExponentialBackoff, Jittered, retry_future};
/// Retry strategy used for user-initiated actions that require immediate feedback
const RETRY_ACTION_STRATEGY: ConstantInterval = ConstantInterval::new(Duration::ZERO, Some(3));
/// Retry strategy used for background tasks
const RETRY_BACKOFF_STRATEGY: Jittered<ExponentialBackoff> = Jittered::jitter(
    ExponentialBackoff::new(Duration::from_secs(4), 5).max_delay(Some(Duration::from_hours(24))),
);

#[derive(Clone)]
pub struct DeviceService {
    api_availability: ApiAvailability,
    /// Backend abstrait pour les 5 opérations device-level. Le
    /// caller (`AccountManager::spawn`) injecte `RemoteDeviceBackend`
    /// ou `LocalDeviceBackend` selon `local_account_mode`.
    backend: Arc<dyn WarrenDeviceBackend>,
}

impl DeviceService {
    /// Construit un `DeviceService` à partir d'un backend déjà résolu.
    /// Utilisé par `AccountManager::spawn` qui aiguille `Remote` vs
    /// `Local` selon `local_account_mode`.
    pub fn new(api_availability: ApiAvailability, backend: Arc<dyn WarrenDeviceBackend>) -> Self {
        Self {
            backend,
            api_availability,
        }
    }

    /// Generate a new device for a given account number
    pub fn generate_for_account(
        &self,
        account_number: AccountNumber,
    ) -> impl Future<Output = Result<PrivateAccountAndDevice, Error>> + Send + use<> {
        let private_key = PrivateKey::new_from_random();
        let pubkey = private_key.public_key();

        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let number_copy = account_number.clone();
        async move {
            let factory = move || {
                let number = number_copy.clone();
                let pubkey = pubkey.clone();
                let backend = backend.clone();
                async move { backend.create(number, pubkey).await }
            };
            let (device, addresses) = retry_future(
                factory,
                move |result| should_retry(result, &api_handle),
                RETRY_ACTION_STRATEGY,
            )
            .await
            .map_err(map_rest_error)?;

            Ok(PrivateAccountAndDevice {
                pubkey: super::account_number_to_warren_pubkey(&account_number),
                device: PrivateDevice::try_from_device(
                    device,
                    WireguardData {
                        private_key,
                        addresses,
                        created: Utc::now(),
                    },
                )?,
            })
        }
    }

    pub async fn generate_for_account_with_backoff(
        &self,
        account_number: AccountNumber,
    ) -> Result<PrivateAccountAndDevice, Error> {
        let private_key = PrivateKey::new_from_random();
        let pubkey = private_key.public_key();

        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let number_copy = account_number.clone();
        let factory = move || {
            let number = number_copy.clone();
            let pubkey = pubkey.clone();
            let backend = backend.clone();
            let api_handle = api_handle.clone();
            async move { api_handle.when_online(backend.create(number, pubkey)).await }
        };
        let (device, addresses) =
            retry_future(factory, should_retry_backoff, RETRY_BACKOFF_STRATEGY)
                .await
                .map_err(map_rest_error)?;

        Ok(PrivateAccountAndDevice {
            pubkey: super::account_number_to_warren_pubkey(&account_number),
            device: PrivateDevice::try_from_device(
                device,
                WireguardData {
                    private_key,
                    addresses,
                    created: Utc::now(),
                },
            )?,
        })
    }

    pub async fn remove_device(
        &self,
        account_number: AccountNumber,
        device_id: DeviceId,
    ) -> Result<Vec<Device>, Error> {
        self.remove_device_inner(account_number.clone(), device_id)
            .await?;
        self.list_devices(account_number).await
    }

    async fn remove_device_inner(
        &self,
        number: AccountNumber,
        device: DeviceId,
    ) -> Result<(), Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        retry_future(
            move || {
                let backend = backend.clone();
                let number = number.clone();
                let device = device.clone();
                async move { backend.remove(number, device).await }
            },
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)?;
        Ok(())
    }

    pub async fn remove_device_with_backoff(
        &self,
        number: AccountNumber,
        device: DeviceId,
    ) -> Result<(), Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();

        retry_future(
            // NOTE: Not honoring "paused" state, because the account may have no time on it.
            move || {
                let backend = backend.clone();
                let number = number.clone();
                let device = device.clone();
                let api_handle = api_handle.clone();
                async move { api_handle.when_online(backend.remove(number, device)).await }
            },
            should_retry_backoff,
            // Not setting a maximum interval
            RETRY_BACKOFF_STRATEGY.clone().max_delay(None),
        )
        .await
        .map_err(map_rest_error)?;

        Ok(())
    }

    pub async fn rotate_key(
        &self,
        number: AccountNumber,
        device: DeviceId,
    ) -> Result<WireguardData, Error> {
        let private_key = PrivateKey::new_from_random();

        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let pubkey = private_key.public_key();
        let factory = move || {
            let number = number.clone();
            let device = device.clone();
            let pubkey = pubkey.clone();
            let backend = backend.clone();
            async move { backend.replace_wg_key(number, device, pubkey).await }
        };
        let addresses = retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)?;

        Ok(WireguardData {
            private_key,
            addresses,
            created: Utc::now(),
        })
    }

    pub async fn rotate_key_with_backoff(
        &self,
        number: AccountNumber,
        device: DeviceId,
    ) -> Result<WireguardData, Error> {
        let private_key = PrivateKey::new_from_random();

        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let pubkey = private_key.public_key();

        let rotate_retry_strategy = std::iter::repeat(Duration::from_hours(24));

        let addresses = retry_future(
            move || {
                let backend = backend.clone();
                let number = number.clone();
                let device = device.clone();
                let pubkey = pubkey.clone();
                let api_handle = api_handle.clone();
                async move {
                    api_handle
                        .when_bg_resumes(backend.replace_wg_key(number, device, pubkey))
                        .await
                }
            },
            should_retry_backoff,
            rotate_retry_strategy,
        )
        .await
        .map_err(map_rest_error)?;

        Ok(WireguardData {
            private_key,
            addresses,
            created: Utc::now(),
        })
    }

    pub async fn list_devices(&self, number: AccountNumber) -> Result<Vec<Device>, Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let factory = move || {
            let number = number.clone();
            let backend = backend.clone();
            async move { backend.list(number).await }
        };
        retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)
    }

    pub async fn list_devices_with_backoff(
        &self,
        number: AccountNumber,
    ) -> Result<Vec<Device>, Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();

        let factory = move || {
            let number = number.clone();
            let backend = backend.clone();
            let api_handle = api_handle.clone();
            async move { api_handle.when_online(backend.list(number)).await }
        };
        retry_future(factory, should_retry_backoff, RETRY_BACKOFF_STRATEGY)
            .await
            .map_err(map_rest_error)
    }

    pub async fn get(&self, number: AccountNumber, device: DeviceId) -> Result<Device, Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let number = number.clone();
        let device = device.clone();
        let factory = move || {
            let number = number.clone();
            let device = device.clone();
            let backend = backend.clone();
            async move { backend.get(number, device).await }
        };
        retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)
    }
}

#[derive(Clone)]
pub struct WarrenIdentityService {
    api_availability: ApiAvailability,
    initial_check_abort_handle: AbortHandle,
    /// `AccountsProxy` Mullvad pour les méthodes non-MVP (voucher,
    /// www_auth_token, init/verify_play_purchase Android). Garde le
    /// path historique tant que ces flows ne sont pas migrés.
    proxy: AccountsProxy,
    /// Backend abstrait pour les 3 méthodes MVP critiques
    /// (`create_account`, `get_data`, `delete_account`). Le caller
    /// (`spawn_warren_identity_service`) injecte `RemoteAccountBackend`
    /// ou `LocalAccountBackend` selon `local_account_mode`.
    backend: Arc<dyn WarrenAccountBackend>,
}

impl WarrenIdentityService {
    pub fn create_account(
        &self,
    ) -> impl Future<Output = Result<AccountNumber, rest::Error>> + use<> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        retry_future(
            move || backend.create_account(),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
    }

    pub fn get_www_auth_token(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<String, rest::Error>> + use<> {
        let proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        retry_future(
            move || proxy.get_www_auth_token(account.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
    }

    /// Prend une `WarrenPubKey` au lieu d'un `AccountNumber` brut.
    /// Route via [`WarrenAccountBackend`] pour permettre le mode local
    /// POC.
    pub async fn get_data(&self, pubkey: WarrenPubKey) -> Result<AccountData, rest::Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let result = retry_future(
            move || backend.get_data(account_number.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if handle_account_data_result(&result, &self.api_availability) {
            self.initial_check_abort_handle.abort();
        }
        result
    }

    pub async fn get_data_2(&self, pubkey: WarrenPubKey) -> Result<AccountData, Error> {
        self.get_data(pubkey).await.map_err(map_rest_error)
    }

    pub async fn submit_voucher(
        &self,
        pubkey: WarrenPubKey,
        voucher: String,
    ) -> Result<VoucherSubmission, Error> {
        let proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let result = retry_future(
            move || proxy.submit_voucher(account_number.clone(), voucher.clone()),
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn init_play_purchase(
        &self,
        pubkey: WarrenPubKey,
    ) -> Result<PlayPurchasePaymentToken, Error> {
        let mut proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let factory = move || {
            let account_number = account_number.clone();

            proxy.init_play_purchase(account_number)
        };
        let result = retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn verify_play_purchase(
        &self,
        pubkey: WarrenPubKey,
        play_purchase: PlayPurchase,
    ) -> Result<(), Error> {
        let mut proxy = self.proxy.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();
        let factory = move || {
            let account_number = account_number.clone();
            let play_purchase = play_purchase.clone();

            proxy.verify_play_purchase(account_number, play_purchase)
        };
        let result = retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await;
        if result.is_ok() {
            self.initial_check_abort_handle.abort();
            self.api_availability.resume_background();
        }
        result.map_err(map_rest_error)
    }

    #[cfg(target_os = "android")]
    pub async fn delete_account(&self, pubkey: WarrenPubKey) -> Result<(), Error> {
        let backend = self.backend.clone();
        let api_handle = self.api_availability.clone();
        let account_number: AccountNumber = pubkey.as_str().to_owned();

        let factory = move || {
            let account_number = account_number.clone();
            backend.delete_account(account_number)
        };
        retry_future(
            factory,
            move |result| should_retry(result, &api_handle),
            RETRY_ACTION_STRATEGY,
        )
        .await
        .map_err(map_rest_error)?;

        Ok(())
    }
}

pub fn spawn_warren_identity_service(
    api_handle: MullvadRestHandle,
    number: Option<AccountNumber>,
    api_availability: ApiAvailability,
    backend: Arc<dyn WarrenAccountBackend>,
) -> WarrenIdentityService {
    let accounts_proxy = AccountsProxy::new(api_handle);
    api_availability.pause_background();

    let api_availability_copy = api_availability.clone();
    let accounts_proxy_copy = accounts_proxy.clone();
    let backend_copy = backend.clone();

    let (future, initial_check_abort_handle) = abortable(async move {
        let Some(number) = number else {
            // Fresh install pré-login : pas d'`AccountNumber` connu,
            // rien à demander au backend. On laisse l'`api_availability`
            // en paused (déjà fait juste avant l'`abortable`).
            api_availability.pause_background();
            return;
        };

        let future_generator = move || {
            let backend = backend.clone();
            let number = number.clone();
            let api_availability = api_availability.clone();
            async move {
                let expiry_fut = api_availability.when_online(backend.get_data(number)).await;
                handle_account_data_result(&expiry_fut, &api_availability)
            }
        };
        let should_retry = move |state_was_updated: &bool| -> bool { !*state_was_updated };
        retry_future(future_generator, should_retry, RETRY_BACKOFF_STRATEGY).await;
    });
    tokio::spawn(future);

    WarrenIdentityService {
        api_availability: api_availability_copy,
        initial_check_abort_handle,
        proxy: accounts_proxy_copy,
        backend: backend_copy,
    }
}

fn handle_account_data_result(
    result: &Result<AccountData, rest::Error>,
    api_availability: &ApiAvailability,
) -> bool {
    match result {
        Ok(_data) if _data.expiry >= chrono::Utc::now() => {
            api_availability.resume_background();
            true
        }
        Ok(_data) => {
            api_availability.pause_background();
            true
        }
        Err(mullvad_api::rest::Error::ApiError(_status, code)) => {
            if code == mullvad_api::INVALID_ACCOUNT {
                api_availability.pause_background();
                return true;
            }
            false
        }
        Err(_) => false,
    }
}

fn should_retry<T>(result: &Result<T, rest::Error>, api_handle: &ApiAvailability) -> bool {
    match result {
        Err(error) if error.is_network_error() => !api_handle.is_offline(),
        _ => false,
    }
}

fn should_retry_backoff<T>(result: &Result<T, rest::Error>) -> bool {
    match result {
        Ok(_) => false,
        Err(error) => {
            if let rest::Error::ApiError(status, code) = error {
                *status != rest::StatusCode::NOT_FOUND
                    && code != mullvad_api::DEVICE_NOT_FOUND
                    && code != mullvad_api::INVALID_ACCOUNT
                    && code != mullvad_api::MAX_DEVICES_REACHED
                    && code != mullvad_api::PUBKEY_IN_USE
            } else {
                true
            }
        }
    }
}

fn map_rest_error(error: rest::Error) -> Error {
    match error {
        rest::Error::ApiError(_status, ref code) => match code.as_str() {
            // TODO: Implement invalid payment
            mullvad_api::DEVICE_NOT_FOUND | mullvad_api::INVALID_ACCESS_TOKEN => {
                Error::InvalidDevice
            }
            mullvad_api::INVALID_ACCOUNT => Error::InvalidAccount,
            mullvad_api::MAX_DEVICES_REACHED => Error::MaxDevicesReached,
            mullvad_api::INVALID_VOUCHER => Error::InvalidVoucher,
            mullvad_api::VOUCHER_USED => Error::UsedVoucher,
            _ => Error::OtherRestError(error),
        },
        error => Error::OtherRestError(error),
    }
}
