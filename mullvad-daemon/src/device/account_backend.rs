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
//! Scope (4 methods): `create_account`, `get_data`,
//! `delete_account`, `submit_voucher`. The remaining methods
//! (`get_www_auth_token`, `init_play_purchase`, `verify_play_purchase`)
//! stay in [`super::service::WarrenIdentityService`] directly on
//! the `AccountsProxy` for this phase.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use mullvad_api::AccountsProxy;
use mullvad_api::rest;
use mullvad_types::account::{AccountData, AccountNumber, VoucherSubmission};
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

    /// Redeems a voucher to create or extend a subscription.
    /// In warren remote mode: calls `POST /v1/register` (unsigned).
    /// In local mode: returns a far-future expiry (no network).
    /// In legacy Mullvad mode: delegates to `AccountsProxy::submit_voucher`.
    fn submit_voucher(
        &self,
        account: AccountNumber,
        voucher: String,
    ) -> BoxFut<Result<VoucherSubmission, rest::Error>>;
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
            let _ = account;
            Box::pin(async move { Err(rest::Error::Aborted) })
        }
    }

    fn submit_voucher(
        &self,
        account: AccountNumber,
        voucher: String,
    ) -> BoxFut<Result<VoucherSubmission, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.submit_voucher(account, voucher).await })
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
    /// warren-api base URL used **only** by `submit_voucher` to
    /// perform the real, unsigned `POST /v1/register` redemption.
    /// Local mode bypasses the *subscription expiry check* (it serves
    /// a far-future stub from `get_data`), but voucher redemption is a
    /// genuine backend operation: it binds the daemon's pubkey to a
    /// subscription so the exit accepts the handshake. Faking it here
    /// (the historical behaviour) made the GUI "add voucher" field
    /// silently enroll nothing, so any code "succeeded" forever while
    /// the tunnel stayed blocked. Resolved from `WARREN_API_URL` when
    /// set, else the production default.
    warren_api_url: String,
}

/// Resolves the warren-api URL for local-mode voucher redemption:
/// `WARREN_API_URL` (non-empty) wins, otherwise the shared production
/// default ([`crate::warren_remote_config::DEFAULT_WARREN_API_URL`]).
/// Mirrors the env override honoured elsewhere for the warren-api URL
/// so a developer pointing at a local backend redeems against it too.
fn default_voucher_api_url() -> String {
    std::env::var("WARREN_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::warren_remote_config::DEFAULT_WARREN_API_URL.to_owned())
}

impl LocalAccountBackend {
    /// Builds a local backend from the current Warren pubkey and
    /// the `settings_dir` to use for `delete_account`. The voucher
    /// redemption URL is resolved from `WARREN_API_URL` or the
    /// production default.
    #[must_use]
    pub fn new(pubkey: WarrenPubKey, settings_dir: PathBuf) -> Self {
        Self::new_with_api_url(pubkey, settings_dir, default_voucher_api_url())
    }

    /// Same as [`new`](Self::new) but with an explicit warren-api URL.
    /// Used by tests to point voucher redemption at a controlled
    /// endpoint.
    #[must_use]
    pub fn new_with_api_url(
        pubkey: WarrenPubKey,
        settings_dir: PathBuf,
        warren_api_url: String,
    ) -> Self {
        Self {
            pubkey,
            settings_dir: Arc::new(settings_dir),
            warren_api_url,
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

    fn submit_voucher(
        &self,
        _account: AccountNumber,
        voucher: String,
    ) -> BoxFut<Result<VoucherSubmission, rest::Error>> {
        // Voucher redemption is a REAL backend operation, even in
        // local mode: `POST /v1/register` (unsigned) binds this
        // daemon's pubkey to a subscription so the exit accepts the
        // handshake. The previous stub fabricated a far-future success
        // for any input and contacted nothing — the GUI's "add
        // voucher" field then reported success for every code while
        // the pubkey stayed unenrolled and the tunnel blocked. We now
        // mirror `WarrenRemoteAccountBackend::submit_voucher` and fail
        // closed on any backend error so an invalid / used voucher
        // surfaces honestly instead of a lie.
        let url = self.warren_api_url.clone();
        let pubkey = self.pubkey.clone();
        Box::pin(async move {
            // The register endpoint is unsigned and carries the pubkey
            // explicitly in the body, so a no-key client is correct
            // here (the daemon's real signing identity is unrelated to
            // this call).
            let client = warren_api_client::WarrenApiClient::new_unsigned(url);
            let req = warren_api_client::RegisterAccountRequest {
                pubkey_hex: pubkey.as_str().parse().map_err(|_| rest::Error::Aborted)?,
                voucher_secret: voucher,
                referral_code: None,
            };
            let resp = client
                .register_with_voucher(&req)
                .await
                .map_err(map_voucher_register_error)?;
            let new_expiry = expiry_from_unix_secs(resp.expires_at)?;
            let now_secs = Utc::now().timestamp() as u64;
            let time_added = resp.expires_at.saturating_sub(now_secs);
            Ok(VoucherSubmission {
                new_expiry,
                time_added,
            })
        })
    }

    fn delete_account(&self, _account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let settings_dir = self.settings_dir.clone();
        Box::pin(async move {
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

    fn submit_voucher(
        &self,
        _account: AccountNumber,
        voucher: String,
    ) -> BoxFut<Result<VoucherSubmission, rest::Error>> {
        let client = self.client.clone();
        let pubkey_hex = client.pubkey_hex();
        Box::pin(async move {
            let req = warren_api_client::RegisterAccountRequest {
                pubkey_hex: pubkey_hex
                    .parse()
                    .map_err(|_| rest::Error::Aborted)?,
                voucher_secret: voucher,
                referral_code: None,
            };
            let resp = client
                .register_with_voucher(&req)
                .await
                .map_err(map_voucher_register_error)?;
            let new_expiry = expiry_from_unix_secs(resp.expires_at)?;
            let now_secs = Utc::now().timestamp() as u64;
            let time_added = resp.expires_at.saturating_sub(now_secs);
            Ok(VoucherSubmission {
                new_expiry,
                time_added,
            })
        })
    }
}

/// Maps a warren-api-client error specifically for the voucher
/// redemption path (`POST /v1/register`).
///
/// The downstream Mullvad legacy pipeline (`device::service::map_rest_error`
/// → `device::Error::{InvalidVoucher,UsedVoucher}` → `Code::{NotFound,
/// ResourceExhausted}` → frontend `'invalid'` / `'already_used'`) only
/// recognises a handful of well-known error-code strings carried in the
/// `rest::Error::ApiError(_, code)` second field (constants
/// `mullvad_api::INVALID_VOUCHER`, `mullvad_api::VOUCHER_USED`, …).
///
/// The Warren backend instead returns structured `{"error": "<human
/// message>"}` JSON bodies (see `warren_api::handlers::subscription::
/// register`). The generic [`map_client_error`] flattens those into
/// opaque `"warren-api <status>: <body>"` strings, which match none of
/// the legacy codes and fall through to `OtherRestError` →
/// `Code::InvalidArgument` → the frontend's generic "An error occurred"
/// toast. Translating each known Warren body into the matching legacy
/// code keeps the existing downstream layers (and the i18n strings
/// already shipped) intact.
///
/// Substring matching (not strict equality) is intentional: the human
/// messages in `warren-api` are part of an internal contract but are
/// not versioned, so a future edit (`"voucher unknown or invalid" →
/// "Voucher is invalid."`) should still route correctly. Should a
/// rewrite ever break the substring, the fallback preserves the raw
/// body in the error log for diagnostics — and the user gets the
/// generic toast rather than a silent failure.
fn map_voucher_register_error(err: warren_api_client::ClientError) -> rest::Error {
    use warren_api_client::ClientError;
    match err {
        ClientError::ServerStatus { status, ref body } => {
            let code = rest::StatusCode::from_u16(status)
                .unwrap_or(rest::StatusCode::INTERNAL_SERVER_ERROR);
            // Map to the legacy Mullvad code constants so
            // `device::service::map_rest_error` picks the right
            // `Error::InvalidVoucher` / `Error::UsedVoucher` variant.
            let mullvad_code: Option<&'static str> = if body.contains("voucher unknown or invalid")
                || body.contains("voucher was cancelled")
            {
                // Cancelled vouchers are surfaced as invalid: the
                // user cannot meaningfully distinguish "wrong code"
                // from "code revoked by the admin" and the recovery
                // action (contact the admin for a fresh voucher)
                // is the same.
                Some(mullvad_api::INVALID_VOUCHER)
            } else if body.contains("voucher already redeemed") {
                Some(mullvad_api::VOUCHER_USED)
            } else {
                None
            };
            if let Some(code_str) = mullvad_code {
                return rest::Error::ApiError(code, code_str.to_owned());
            }
            // "pubkey already registered" and any other 4xx fall
            // through to the generic path: the user sees a generic
            // error, and the operator gets the raw body in the
            // daemon log to triage. "pubkey already registered"
            // specifically should not normally happen here — it
            // would mean `get_account_data` returned 404 for a
            // pubkey that the server side still has on file, which
            // is a server-state inconsistency worth investigating.
            let msg = if body.is_empty() {
                format!("warren-api {status}")
            } else {
                format!("warren-api {status}: {body}")
            };
            rest::Error::ApiError(code, msg)
        }
        // Transport / serde / clock -> infra down. Same convention
        // as the generic `map_client_error`.
        _ => rest::Error::Aborted,
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
    async fn local_submit_voucher_fails_closed_instead_of_fabricating_success() {
        // Regression: the local backend used to IGNORE the voucher and
        // return a fabricated ~100-year success for ANY input,
        // contacting no backend and enrolling nothing — so the GUI
        // "add voucher" field reported success for every code (even the
        // same one repeatedly) while the pubkey stayed unenrolled and
        // the exit kept refusing the handshake, blocking internet.
        // Voucher redemption must hit warren-api for real; when the
        // backend is unreachable the call MUST fail, never fabricate a
        // far-future subscription.
        let backend = LocalAccountBackend::new_with_api_url(
            fixed_pubkey(),
            isolated_tempdir(),
            // Refused port: an immediate connection error, so the unit
            // test carries no real network dependency.
            "http://127.0.0.1:1".to_owned(),
        );
        let result = backend
            .submit_voucher("ignored".to_owned(), "ANY-CODE-1234".to_owned())
            .await;
        assert!(
            result.is_err(),
            "local submit_voucher must fail closed when warren-api is unreachable, \
             not fabricate a success (the historical lying-stub regression)"
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

    // ===================================================================
    // Voucher error mapping — locks down the substring matching against
    // the human-readable bodies emitted by
    // `warren_api::handlers::subscription::register`. Should the upstream
    // strings be edited, these tests fail loudly and prompt a sync of
    // the substrings in `map_voucher_register_error`.
    // ===================================================================

    fn server_status(status: u16, body: &str) -> warren_api_client::ClientError {
        warren_api_client::ClientError::ServerStatus {
            status,
            body: body.to_owned(),
        }
    }

    #[test]
    fn map_voucher_register_error_unknown_voucher_400_maps_invalid_voucher() {
        let err = super::map_voucher_register_error(server_status(
            400,
            r#"{"error":"voucher unknown or invalid"}"#,
        ));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 400);
                assert_eq!(msg, mullvad_api::INVALID_VOUCHER);
            }
            other => panic!("expected ApiError(400, INVALID_VOUCHER), got {other:?}"),
        }
    }

    #[test]
    fn map_voucher_register_error_cancelled_410_maps_invalid_voucher() {
        // Cancelled vouchers are surfaced as invalid for the user
        // (admin-side action, no recovery distinct from "wrong code").
        let err = super::map_voucher_register_error(server_status(
            410,
            r#"{"error":"voucher was cancelled by the admin"}"#,
        ));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 410);
                assert_eq!(msg, mullvad_api::INVALID_VOUCHER);
            }
            other => panic!("expected ApiError(410, INVALID_VOUCHER), got {other:?}"),
        }
    }

    #[test]
    fn map_voucher_register_error_already_redeemed_409_maps_voucher_used() {
        let err = super::map_voucher_register_error(server_status(
            409,
            r#"{"error":"voucher already redeemed"}"#,
        ));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409);
                assert_eq!(msg, mullvad_api::VOUCHER_USED);
            }
            other => panic!("expected ApiError(409, VOUCHER_USED), got {other:?}"),
        }
    }

    #[test]
    fn map_voucher_register_error_pubkey_already_registered_falls_through() {
        // "pubkey already registered" is a server-state inconsistency
        // that should be diagnosable in the daemon log — fall through
        // to the opaque body to preserve the raw context.
        let err = super::map_voucher_register_error(server_status(
            409,
            r#"{"error":"pubkey already registered"}"#,
        ));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409);
                assert!(
                    msg.contains("pubkey already registered"),
                    "raw body must be preserved for diagnostics, got: {msg}"
                );
                // Critically, NOT mapped to a Mullvad legacy code:
                // the user gets the generic toast and the operator
                // gets the precise body in the log.
                assert_ne!(msg, mullvad_api::INVALID_VOUCHER);
                assert_ne!(msg, mullvad_api::VOUCHER_USED);
            }
            other => panic!("expected ApiError(409, raw body), got {other:?}"),
        }
    }

    #[test]
    fn map_voucher_register_error_bad_clock_maps_aborted() {
        // Any non-`ServerStatus` warren-api-client error (transport,
        // serde, system clock pre-epoch, …) is infra down — must
        // map to `Aborted` to align with the convention of
        // [`map_client_error`]. `BadClock` is a convenient no-arg
        // variant for unit testing the catch-all arm.
        let err = super::map_voucher_register_error(warren_api_client::ClientError::BadClock);
        match err {
            rest::Error::Aborted => {}
            other => panic!("expected Aborted, got {other:?}"),
        }
    }
}
