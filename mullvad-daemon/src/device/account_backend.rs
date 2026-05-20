//! Abstract trait for the account-level operations that used to go
//! through `AccountsProxy` to `api.mullvad.net`.
//!
//! Lets us dispatch at boot between:
//! - [`RemoteAccountBackend`]: thin wrap over the legacy Mullvad
//!   `AccountsProxy`. Behavior strictly identical in non-`local`.
//! - [`LocalAccountBackend`]: stateless POC that serves data
//!   consistent with the mnemonic loaded at boot, **without touching
//!   the network**. Replaces the env-var bypass `WARREN_LOCAL_ACCOUNT=1`
//!   with a real pluggable backend.
//!
//! MVP scope (3 methods): `create_account`, `get_data`,
//! `delete_account`. The other methods (`submit_voucher`,
//! `get_www_auth_token`, `init_play_purchase`, `verify_play_purchase`,
//! Android `delete_account`) stay in
//! [`super::service::WarrenIdentityService`] directly on the `AccountsProxy`
//! for this phase; to migrate in C.1+ if needed.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use mullvad_api::AccountsProxy;
use mullvad_api::rest;
use mullvad_types::account::{AccountData, AccountNumber};
use mullvad_types::warren_pubkey::WarrenPubKey;

/// Type alias for the futures returned by the trait. `Pin<Box<dyn …>>`
/// is required by object-safety (`Arc<dyn WarrenAccountBackend>`).
/// `'static` is required by `retry_future` compatibility (which
/// requires the futures returned by the factory to be `'static`) —
/// each trait impl must clone its deps before `Box::pin(async move {…})`.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Abstract backend for the MVP critical account-level operations.
///
/// All methods return `Result<_, rest::Error>` to
/// preserve ABI compatibility with `retry_future` and with the existing
/// error map (`map_rest_error` on the
/// [`super::service`] side). In local mode, `rest::Error` is produced
/// only for degraded cases (disk corruption for instance) —
/// the nominal path is always `Ok`.
pub trait WarrenAccountBackend: Send + Sync {
    /// Creates a new account. Returns the produced `AccountNumber`.
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>>;

    /// Fetches the account data (= mainly the expiry).
    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>>;

    /// Removes the account (= erases the local identity in POC mode).
    ///
    /// Outside Android, `WarrenIdentityService` does not expose
    /// `delete_account` (see `service.rs:#[cfg(target_os = "android")]`),
    /// so this trait method is compiled but not invoked
    /// on non-Android targets. Kept to allow the future
    /// migration of a desktop `delete_account` flow if needed, and
    /// used by tests on all targets.
    #[cfg_attr(
        not(any(test, target_os = "android")),
        expect(
            dead_code,
            reason = "only called on Android and in cross-platform tests"
        )
    )]
    fn delete_account(&self, account: AccountNumber) -> BoxFut<Result<(), rest::Error>>;
}

/// Thin wrap of the legacy Mullvad `AccountsProxy`. Delegates each
/// trait method to the corresponding `AccountsProxy`. Behavior
/// strictly identical to the pre-Warren-fork Mullvad path.
#[derive(Clone)]
pub struct RemoteAccountBackend {
    proxy: AccountsProxy,
}

impl RemoteAccountBackend {
    #[must_use]
    pub fn new(proxy: AccountsProxy) -> Self {
        Self { proxy }
    }
}

impl WarrenAccountBackend for RemoteAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.create_account().await })
    }

    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.get_data(account).await })
    }

    fn delete_account(&self, account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        #[cfg(target_os = "android")]
        {
            let proxy = self.proxy.clone();
            Box::pin(async move { proxy.delete_account(account).await })
        }
        #[cfg(not(target_os = "android"))]
        {
            // `AccountsProxy::delete_account` does not exist outside Android
            // on the Mullvad upstream side; we return an explicit error.
            let _ = account;
            Box::pin(async move { Err(rest::Error::Aborted) })
        }
    }
}

/// POC backend that serves data consistent with the Warren mnemonic
/// loaded at boot, without any network call. Idempotent and
/// deterministic (modulo `Utc::now()` for `get_data.expiry`).
///
/// The source of truth for the identity is the `pubkey: WarrenPubKey`
/// derived from `warren_signer::load_or_create_signing_key` — `create_account`
/// returns this pubkey hex as the `AccountNumber` to stay consistent
/// with the `device.json` produced by
/// [`crate::warren_device_bootstrap::ensure_local_device`].
///
/// `delete_account` removes `device.json` and `warren_mnemonic.txt`
/// to reproduce the classic Mullvad "logged out" semantics in local
/// mode: the user will have to re-bootstrap to start over.
#[derive(Clone)]
pub struct LocalAccountBackend {
    pubkey: WarrenPubKey,
    /// Used exclusively by `delete_account` (see the trait method's
    /// doc: not invoked outside Android on the caller side, but
    /// the field is necessary for the cross-platform test and for
    /// a future desktop migration).
    settings_dir: Arc<PathBuf>,
}

impl LocalAccountBackend {
    /// Builds a local backend from the current Warren pubkey and
    /// the `settings_dir` to use for `delete_account`.
    #[must_use]
    pub fn new(pubkey: WarrenPubKey, settings_dir: PathBuf) -> Self {
        Self {
            pubkey,
            settings_dir: Arc::new(settings_dir),
        }
    }

    /// Expiry returned by `get_data` in local mode: `Utc::now() +
    /// 100 years`. Consistent with `handle_account_data_result` on
    /// the caller side which interprets `expiry >= now` ->
    /// `resume_background()` (Wireguard BG key rotation enabled as expected).
    fn far_future_expiry() -> chrono::DateTime<Utc> {
        // 36500 days ~ 100 years. Well beyond any reasonable
        // usage duration, < `chrono::DateTime::MAX` which
        // panics beyond year 262143.
        Utc::now() + chrono::Duration::days(36500)
    }
}

impl WarrenAccountBackend for LocalAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        // POC identity = pubkey hex (64 chars). Idempotent by
        // construction: the pubkey does not change for a given
        // settings_dir (same mnemonic).
        let number = self.pubkey.as_str().to_owned();
        Box::pin(async move { Ok(number) })
    }

    fn get_data(&self, _account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        // The Mullvad `AccountId` is an opaque `String`; we return
        // the pubkey hex for consistency with `create_account`. The expiry
        // pushes `handle_account_data_result` to `resume_background()`.
        let id = self.pubkey.as_str().to_owned();
        let data = AccountData {
            id,
            expiry: Self::far_future_expiry(),
        };
        Box::pin(async move { Ok(data) })
    }

    fn delete_account(&self, _account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let settings_dir = self.settings_dir.clone();
        Box::pin(async move {
            // Removes device.json (= "logged out" state for the
            // DeviceCacher on next boot) and the BIP39 mnemonic
            // (= identity). Idempotent: an already-absent file
            // does not produce an error.
            let device_path = settings_dir.join(super::DEVICE_CACHE_FILENAME);
            let mnemonic_path = settings_dir.join(crate::warren_signer::MNEMONIC_FILENAME);
            for path in [device_path, mnemonic_path] {
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        log::error!(
                            "LocalAccountBackend::delete_account failed to remove {}: {}",
                            path.display(),
                            e
                        );
                        return Err(rest::Error::Aborted);
                    }
                }
            }
            Ok(())
        })
    }
}

/// Warren-Remote backend — Phase G.3 — implements
/// [`WarrenAccountBackend`] via the signed HTTP `warren-api-client`
/// client that talks to the warren-api server (= alternative to the
/// `RemoteAccountBackend` path that talks to `api.mullvad.net`).
///
/// Enabled in `warren_mode = true && warren_local_account = false` mode
/// (= 3rd branch of the dispatch in `device/mod.rs`, see Phase G.4).
///
/// Mapping semantics:
/// - `create_account()`: returns the `WarrenApiClient` pubkey hex
///   (= Warren signer identity at daemon boot). No server call —
///   real account creation on the warren-api side goes through the
///   voucher flow (`POST /v1/register` non-auth) outside this trait.
/// - `get_data(account)`: signed `GET /v1/subscription` ->
///   [`AccountData`] with `id = account` and `expiry` reconstructed from
///   `expires_at` (unix seconds -> `chrono::DateTime<Utc>`).
/// - `delete_account(account)`: signed `DELETE /v1/account`.
#[derive(Clone)]
pub struct WarrenRemoteAccountBackend {
    client: Arc<warren_api_client::WarrenApiClient>,
}

impl WarrenRemoteAccountBackend {
    /// Builds a backend from a `WarrenApiClient` configured at
    /// boot. The client carries the Ed25519 `SigningKey` (= Warren
    /// identity) and the `warren-api` URL.
    #[must_use]
    pub fn new(client: warren_api_client::WarrenApiClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl WarrenAccountBackend for WarrenRemoteAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        // No server call: the Warren identity is fixed by the
        // mnemonic loaded at boot. The actual subscription creation on
        // the warren-api side goes through the voucher flow (`POST /v1/register`
        // non-auth) outside this trait.
        let pubkey_hex = self.client.pubkey_hex();
        Box::pin(async move { Ok(pubkey_hex) })
    }

    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client.get_subscription().await.map_err(map_client_error)?;
            let expiry = expiry_from_unix_secs(resp.expires_at)?;
            Ok(AccountData {
                id: account,
                expiry,
            })
        })
    }

    fn delete_account(&self, _account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move { client.delete_account().await.map_err(map_client_error) })
    }
}

/// Reconstructs `expiry: DateTime<Utc>` from `expires_at: u64` (unix
/// seconds). The warren-api server provides the expiry in seconds
/// (JSON-consistent), Mullvad uses `chrono::DateTime`. Only possible
/// error: `expires_at` overflows `i64` (= year > 9 billion) ->
/// returns `Aborted` rather than panic.
fn expiry_from_unix_secs(secs: u64) -> Result<chrono::DateTime<Utc>, rest::Error> {
    let secs_i64 = i64::try_from(secs).map_err(|_| rest::Error::Aborted)?;
    chrono::DateTime::from_timestamp(secs_i64, 0).ok_or(rest::Error::Aborted)
}

/// Maps a [`warren_api_client::ClientError`] to a Mullvad
/// [`rest::Error`] to preserve the contract of the traits
/// [`WarrenAccountBackend`] / [`super::device_backend::WarrenDeviceBackend`].
///
/// Convention: a non-2xx HTTP status -> `ApiError(StatusCode, msg)`
/// (mappable on the caller side via `map_rest_error`). Everything
/// else (transport down, serde, clock) -> `Aborted` — consistent with
/// the Mullvad pattern for infrastructure failures.
pub(super) fn map_client_error(err: warren_api_client::ClientError) -> rest::Error {
    use warren_api_client::ClientError;
    match err {
        ClientError::ServerStatus { status, body } => {
            let code = rest::StatusCode::from_u16(status)
                .unwrap_or(rest::StatusCode::INTERNAL_SERVER_ERROR);
            let msg = if body.is_empty() {
                format!("warren-api {status}")
            } else {
                format!("warren-api {status}: {body}")
            };
            rest::Error::ApiError(code, msg)
        }
        // Transport / serde / clock -> infra down.
        _ => rest::Error::Aborted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn fixed_pubkey() -> WarrenPubKey {
        WarrenPubKey::from_str(&"a".repeat(64)).expect("valid hex 64ch")
    }

    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-account-backend-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[tokio::test]
    async fn local_get_data_returns_far_future_expiry() {
        // Critical regression: if the returned expiry < now, the caller
        // `handle_account_data_result` (see service.rs) triggers
        // `pause_background()` -> Wireguard BG key rotation
        // stops silently. This is exactly the behavior
        // we want to avoid in local POC mode.
        let backend = LocalAccountBackend::new(fixed_pubkey(), isolated_tempdir());
        let data = backend
            .get_data("ignored".to_owned())
            .await
            .expect("local get_data must never fail in nominal case");

        let lower_bound = Utc::now() + chrono::Duration::days(50 * 365);
        assert!(
            data.expiry > lower_bound,
            "expiry {} must be > now + 50 years to activate resume_background",
            data.expiry
        );
    }

    #[tokio::test]
    async fn local_create_account_returns_pubkey_hex_deterministic() {
        // Critical regression: if `create_account` returned a
        // random or call-varying String, the
        // `device.json` bootstrapped from the mnemonic would become
        // orphaned from the created account -> the user could no
        // longer reload their session after reboot.
        let backend = LocalAccountBackend::new(fixed_pubkey(), isolated_tempdir());
        let n1 = backend
            .create_account()
            .await
            .expect("create_account must never fail locally");
        let n2 = backend
            .create_account()
            .await
            .expect("create_account must never fail locally");

        assert_eq!(
            n1, n2,
            "create_account must be idempotent (= deterministic)"
        );
        assert_eq!(
            n1,
            fixed_pubkey().as_str(),
            "AccountNumber MUST be the pubkey hex (= consistency with device.json bootstrap)"
        );
    }

    #[tokio::test]
    async fn local_delete_account_removes_device_json_and_mnemonic() {
        // Critical regression: if delete_account does not remove the
        // identity artifacts, the user stays "logged in" via
        // device.json after an account delete = serious UX +
        // security bug (impossible to "really" log out).
        let dir = isolated_tempdir();
        let device_path = dir.join(super::super::DEVICE_CACHE_FILENAME);
        let mnemonic_path = dir.join(crate::warren_signer::MNEMONIC_FILENAME);
        std::fs::write(&device_path, "{}").expect("write device.json");
        std::fs::write(&mnemonic_path, "test mnemonic").expect("write mnemonic");

        let backend = LocalAccountBackend::new(fixed_pubkey(), dir.clone());
        backend
            .delete_account("ignored".to_owned())
            .await
            .expect("delete_account must succeed");

        assert!(
            !device_path.exists(),
            "device.json must be removed after delete_account"
        );
        assert!(
            !mnemonic_path.exists(),
            "warren_mnemonic.txt must be removed after delete_account"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn local_delete_account_is_idempotent_on_missing_files() {
        // Edge case: if the user calls delete_account twice, or
        // the bootstrap has already been cleaned up manually, we
        // must not raise an error (which would be interpreted
        // as an API failure and propagated to the UI).
        let dir = isolated_tempdir();
        let backend = LocalAccountBackend::new(fixed_pubkey(), dir.clone());

        backend
            .delete_account("ignored".to_owned())
            .await
            .expect("delete_account with absent files must return Ok");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ===================================================================
    // WarrenRemoteAccountBackend — Phase G.3 tests E2E.
    //
    // Strategy: spawn warren-api in-process (axum::serve loopback),
    // build an Ed25519-signed `WarrenApiClient`, instantiate the backend,
    // exercise each trait method. Checks the wire mapping warren-api
    // <-> `mullvad_types::AccountData` + the `ClientError` <->
    // `rest::Error::ApiError` mapping.
    // ===================================================================

    use ed25519_dalek::SigningKey;
    use std::sync::Arc as TestArc;
    use warren_api_client::WarrenApiClient;

    /// Spawns warren-api in-process and returns (URL, AppState).
    /// The `AppState` lets tests inspect / pre-populate the
    /// server stores (= shortcut equivalent to the signed admin
    /// endpoints coming in M5).
    async fn spawn_warren_api() -> (String, TestArc<warren_api::AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().expect("local addr");
        let state = warren_api::AppState::in_memory();
        let app = warren_api::build_router(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        (format!("http://{addr}"), state)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_create_returns_signing_pubkey() {
        // Critical regression: `create_account` must return the
        // pubkey hex of the signer identity (= consistency with
        // `device.json` on the `warren_device_bootstrap` side). No
        // server call — purely local.
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[60u8; 32]);
        let expected_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let acc = backend.create_account().await.expect("create OK");
        assert_eq!(acc, expected_pubkey_hex);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_reads_subscription_expiry() {
        // Nominal case: sub present on warren-api side -> backend.get_data()
        // returns AccountData with expiry reconstructed from expires_at.
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[61u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        // Pre-populate server-side (= equivalent to a prior /v1/register).
        state.subscriptions.insert(&pubkey_hex, 1_700_000_000);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);
        let data = backend
            .get_data(pubkey_hex.clone())
            .await
            .expect("get_data OK");

        assert_eq!(data.id, pubkey_hex, "id == account passed as arg");
        assert_eq!(
            data.expiry.timestamp(),
            1_700_000_000_i64,
            "expiry must reflect server expires_at"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_returns_apierror_404_when_no_sub() {
        // Critical regression: if the ClientError -> rest::Error mapping
        // loses the 404 status, the caller (`handle_account_data_result`)
        // interprets a generic error instead of "non-existent account"
        // -> degraded UX + inconsistent device.json state.
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[62u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .get_data(pubkey_hex)
            .await
            .expect_err("must fail with 404 mapping");
        match err {
            rest::Error::ApiError(code, _) => {
                assert_eq!(code.as_u16(), 404, "404 must transit intact");
            }
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_removes_subscription() {
        // Nominal case: delete_account removes the sub on the server side.
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[63u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        state.subscriptions.insert(&pubkey_hex, 9_999_999_999);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);
        backend
            .delete_account(pubkey_hex.clone())
            .await
            .expect("delete OK");

        assert!(
            state.subscriptions.get_expiry(&pubkey_hex).is_none(),
            "sub must have disappeared server-side after delete_account"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_returns_apierror_404_when_no_sub() {
        // Regression: if we try to delete a non-existent sub, the
        // backend must propagate 404 -> caller can decide to ignore it
        // or log it cleanly (vs generic error).
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[64u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .delete_account(pubkey_hex)
            .await
            .expect_err("must fail 404");
        match err {
            rest::Error::ApiError(code, _) => {
                assert_eq!(code.as_u16(), 404);
            }
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }
}
