//! Abstract trait for the account-level operations that used to go
//! through `AccountsProxy` to `api.mullvad.net`.
//!
//! Lets us dispatch at boot between:
//! - [`WarrenRemoteAccountBackend`]: signed HTTP backend talking to
//!   warren-api (the standard Warren path - real subscription expiry).
//! - [`RemoteAccountBackend`]: thin wrap over the legacy Mullvad
//!   `AccountsProxy`, used only when no Warren signing key is available.
//!
//! Scope (3 methods): `get_data`, `delete_account`,
//! `submit_voucher`. The remaining methods
//! (`get_www_auth_token`, `init_play_purchase`, `verify_play_purchase`)
//! stay in [`super::service::WarrenIdentityService`] directly on
//! the `AccountsProxy` for this phase.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use mullvad_api::AccountsProxy;
use mullvad_api::rest;
use mullvad_types::account::{AccountData, AccountNumber, VoucherSubmission};

use crate::warren_sdk_client::SharedWarrenApiClient;

/// Type alias for the futures returned by the trait. `Pin<Box<dyn …>>`
/// is required by object-safety (`Arc<dyn WarrenAccountBackend>`).
/// `'static` is required by `retry_future` compatibility (which
/// requires the futures returned by the factory to be `'static`) -
/// each trait impl must clone its deps before `Box::pin(async move {…})`.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Abstract backend for the MVP critical account-level operations.
///
/// All methods return `Result<_, rest::Error>` to
/// preserve ABI compatibility with `retry_future` and with the existing
/// error map (`map_rest_error` on the
/// [`super::service`] side). In local mode, `rest::Error` is produced
/// only for degraded cases (disk corruption for instance) -
/// the nominal path is always `Ok`.
pub trait WarrenAccountBackend: Send + Sync {
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

/// Warren-Remote backend - implements
/// [`WarrenAccountBackend`] via the signed HTTP client
/// ([`SharedWarrenApiClient`], wrapping the SDK's `warren_api::WarrenApiClient`)
/// that talks to the warren-api server (= alternative to the
/// `RemoteAccountBackend` path that talks to `api.mullvad.net`).
///
/// The standard Warren account backend (= the warren-remote branch of
/// the dispatch in `device/mod.rs`).
///
/// Mapping semantics:
/// - `get_data(account)`: signed `GET /v1/subscription` ->
///   [`AccountData`] with `id = account` and `expiry` reconstructed from
///   `expires_at` (unix seconds -> `chrono::DateTime<Utc>`).
/// - `delete_account(account)`: signed `DELETE /v1/account`.
#[derive(Clone)]
pub struct WarrenRemoteAccountBackend {
    client: Arc<SharedWarrenApiClient>,
    /// Secrets pulled from `GET /v1/checkout/{wpid}/voucher` whose
    /// `POST /v1/register` has not succeeded yet, keyed by wpid. The
    /// pull consumes the server-side single-use mapping, so a register
    /// failure (API briefly down) would otherwise burn a PAID voucher:
    /// the GUI keeps polling the same wpid, and this cache lets every
    /// subsequent poll retry the register with the already-pulled
    /// secret instead of 404-ing forever. In-memory only: a daemon
    /// crash inside that window still loses the secret (accepted
    /// residual, doc 35 section 7).
    pulled_unregistered: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

impl WarrenRemoteAccountBackend {
    /// Builds a backend from a [`SharedWarrenApiClient`] configured at
    /// boot. The client carries the shared, hot-swappable Warren identity
    /// seed and the `warren-api` URL.
    #[must_use]
    pub fn new(client: SharedWarrenApiClient) -> Self {
        Self {
            client: Arc::new(client),
            pulled_unregistered: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
}

impl WarrenAccountBackend for WarrenRemoteAccountBackend {
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
        let pubkey_ss58 = client.address();
        let pulled_unregistered = self.pulled_unregistered.clone();
        Box::pin(async move {
            // App-initiated purchase (doc 35): the GUI polls this same
            // entry point with the 32-hex wpid it generated before
            // opening the checkout site. A wpid can never collide with
            // a voucher (16 Crockford-32 chars after normalization),
            // so the shape fully determines the path: pull the queued
            // secret from the API first, then redeem it as usual.
            let wpid = as_wpid(&voucher);
            let voucher = match &wpid {
                Some(wpid) => {
                    // A previous poll may have pulled the secret and
                    // then failed the register: the server-side
                    // mapping is consumed, the cache is the only copy.
                    let cached = pulled_unregistered
                        .lock()
                        .expect("not poisoned")
                        .get(wpid)
                        .cloned();
                    match cached {
                        Some(secret) => secret,
                        None => match client
                            .pull_pending_voucher(wpid)
                            .await
                            .map_err(map_client_error)?
                        {
                            Some(secret) => {
                                pulled_unregistered
                                    .lock()
                                    .expect("not poisoned")
                                    .insert(wpid.clone(), secret.clone());
                                secret
                            }
                            // Webhook not landed yet (or id expired).
                            // Surface the legacy INVALID_VOUCHER code so
                            // the GUI poll treats it as "not ready, try
                            // again".
                            None => {
                                return Err(rest::Error::ApiError(
                                    rest::StatusCode::NOT_FOUND,
                                    mullvad_api::INVALID_VOUCHER.to_owned(),
                                ));
                            }
                        },
                    }
                }
                None => voucher,
            };

            let req = warren_api::RegisterAccountRequest {
                pubkey_ss58: warren_api::PubkeySs58::try_from(pubkey_ss58.as_str())
                    .map_err(|_| rest::Error::Aborted)?,
                voucher_secret: voucher,
                referral_code: None,
            };
            // The pull above consumed the single-use mapping: a
            // transient register failure here would burn the paid
            // voucher. Retry transport-level failures a few times
            // before giving up (server-side 4xx are final); on a
            // pulled wpid the cache above lets the NEXT GUI poll keep
            // retrying the register even after these retries fail.
            let mut resp = None;
            let mut last_err = None;
            for attempt in 0u32..3 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                match client.register_with_voucher(&req).await {
                    Ok(r) => {
                        resp = Some(r);
                        break;
                    }
                    Err(e @ warren_api::ClientError::ServerStatus { .. }) => {
                        // Final server-side verdict. AlreadyRedeemed on a
                        // cached secret means a previous attempt DID land
                        // server-side: drop the cache entry so the poll
                        // stops replaying it.
                        if let Some(wpid) = &wpid {
                            pulled_unregistered
                                .lock()
                                .expect("not poisoned")
                                .remove(wpid);
                        }
                        return Err(map_voucher_register_error(e));
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            let resp = match resp {
                Some(r) => r,
                None => {
                    // Transport still down: keep the cached secret (if
                    // any) so the next poll retries the register.
                    return Err(map_voucher_register_error(
                        last_err.expect("loop ran at least once without success"),
                    ));
                }
            };
            if let Some(wpid) = &wpid {
                pulled_unregistered
                    .lock()
                    .expect("not poisoned")
                    .remove(wpid);
            }
            let new_expiry = expiry_from_unix_secs(resp.expires_at)?;
            let now_secs = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
            let time_added = resp.expires_at.saturating_sub(now_secs);
            Ok(VoucherSubmission {
                new_expiry,
                time_added,
            })
        })
    }
}

/// Detect the warren purchase id shape: exactly 32 ASCII hex chars
/// (after trimming), normalized to lowercase. Mirrors
/// `warren_api::providers::normalize_wpid`. Anything else is treated
/// as a regular voucher secret.
fn as_wpid(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.len() == 32 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

/// Maps a warren-api error specifically for the voucher
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
/// body in the error log for diagnostics - and the user gets the
/// generic toast rather than a silent failure.
fn map_voucher_register_error(err: warren_api::ClientError) -> rest::Error {
    use warren_api::ClientError;
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
            // specifically should not normally happen here - it
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

/// Maps a [`warren_api::ClientError`] to a Mullvad
/// [`rest::Error`] to preserve the contract of the
/// [`WarrenAccountBackend`] trait.
///
/// Convention: a non-2xx HTTP status -> `ApiError(StatusCode, msg)`
/// (mappable on the caller side via `map_rest_error`). Everything
/// else (transport down, serde, clock) -> `Aborted` - consistent with
/// the Mullvad pattern for infrastructure failures.
pub(super) fn map_client_error(err: warren_api::ClientError) -> rest::Error {
    use warren_api::ClientError;
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
    use crate::warren_sdk_client::SharedWarrenApiClient;

    // ===================================================================
    // WarrenRemoteAccountBackend - E2E tests.
    //
    // Strategy: spawn the REAL warren-core warren-api server in-process
    // (axum::serve loopback, dev-dependency only, never shipped), build a
    // `SharedWarrenApiClient` (wrapping the SDK's `warren_api::WarrenApiClient`),
    // instantiate the backend, exercise each trait method. Checks the wire
    // mapping warren-api <-> `mullvad_types::AccountData` + the
    // `ClientError` <-> `rest::Error::ApiError` mapping against the real
    // DTO surface, proving wire-compat of the SDK client.
    // ===================================================================

    use std::sync::{Arc as TestArc, RwLock};
    use warren_identity::WarrenIdentity;
    use zeroize::Zeroizing;

    /// Spawns warren-api in-process and returns (URL, AppState).
    /// The `AppState` lets tests inspect / pre-populate the
    /// server stores (= shortcut equivalent to the signed admin
    /// endpoints).
    async fn spawn_warren_api() -> (String, TestArc<warren_api_server::AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().expect("local addr");
        let state = warren_api_server::AppState::in_memory();
        let app = warren_api_server::build_router(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        (format!("http://{addr}"), state)
    }

    /// Builds a `SharedWarrenApiClient` around a fixed seed (no hot-swap
    /// needed in these tests: the identity never changes mid-test).
    fn client_with_seed(api_url: String, seed: [u8; 32]) -> SharedWarrenApiClient {
        SharedWarrenApiClient::new(api_url, TestArc::new(RwLock::new(Zeroizing::new(seed))))
    }

    fn address_for_seed(seed: [u8; 32]) -> String {
        WarrenIdentity::from_seed(&seed).address()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_reads_subscription_expiry() {
        // Nominal case: sub present on warren-api side -> backend.get_data()
        // returns AccountData with expiry reconstructed from expires_at.
        let (api_url, state) = spawn_warren_api().await;
        let seed = [61u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        // Pre-populate server-side (= equivalent to a prior /v1/register).
        state.subscriptions.insert(&pubkey_ss58, 1_700_000_000);

        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);
        let data = backend
            .get_data(pubkey_ss58.clone())
            .await
            .expect("get_data OK");

        assert_eq!(data.id, pubkey_ss58, "id == account passed as arg");
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
        let seed = [62u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .get_data(pubkey_ss58)
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
        let seed = [63u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        state.subscriptions.insert(&pubkey_ss58, 9_999_999_999);

        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);
        backend
            .delete_account(pubkey_ss58.clone())
            .await
            .expect("delete OK");

        assert!(
            state.subscriptions.get_expiry(&pubkey_ss58).is_none(),
            "sub must have disappeared server-side after delete_account"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_returns_apierror_404_when_no_sub() {
        // Regression: if we try to delete a non-existent sub, the
        // backend must propagate 404 -> caller can decide to ignore it
        // or log it cleanly (vs generic error).
        let (api_url, _state) = spawn_warren_api().await;
        let seed = [64u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .delete_account(pubkey_ss58)
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
    // App-initiated purchase path (doc 35): submit_voucher with a wpid
    // pulls the queued secret from GET /v1/checkout/{wpid}/voucher
    // before redeeming it through POST /v1/register.
    // ===================================================================

    #[test]
    fn as_wpid_accepts_only_32_hex_chars() {
        assert_eq!(
            super::as_wpid("0123456789ABCDEF0123456789abcdef").as_deref(),
            Some("0123456789abcdef0123456789abcdef"),
            "32 hex chars (any case, trimmed) are a wpid, lowercased"
        );
        assert_eq!(
            super::as_wpid("  0123456789abcdef0123456789abcdef\n").as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        // Voucher display and raw forms must NOT be mistaken for wpids.
        assert!(super::as_wpid("ABCD-EFGH-JKMN-PQRS").is_none());
        assert!(super::as_wpid("ABCDEFGHJKMNPQRS").is_none());
        assert!(super::as_wpid("").is_none());
        assert!(super::as_wpid("0123456789abcdef0123456789abcdeg").is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_submit_voucher_with_wpid_pulls_and_redeems() {
        let (api_url, state) = spawn_warren_api().await;
        let seed = [65u8; 32];
        let pubkey_ss58 = address_for_seed(seed);

        // Simulate the webhook outcome: a minted voucher whose secret
        // is queued under the app-chosen wpid.
        let (secret, hash) = warren_api_server::generate_voucher_secret();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(state.vouchers.create(
            hash,
            30 * 86_400,
            warren_api_server::PaymentMethod::Card,
            now
        ));
        let wpid = "00112233445566778899aabbccddeeff";
        state.pending_vouchers.put(wpid, &secret, now + 600);

        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);
        let submission = backend
            .submit_voucher(pubkey_ss58.clone(), wpid.to_owned())
            .await
            .expect("wpid pull + redeem must succeed");
        assert!(
            submission.time_added >= 30 * 86_400 - 60,
            "credited time must reflect the voucher duration, got {}",
            submission.time_added
        );
        assert!(
            state.subscriptions.get_expiry(&pubkey_ss58).is_some(),
            "subscription must exist after the wpid flow"
        );

        // The mapping is single-use: a second poll on the same wpid
        // reports INVALID_VOUCHER (the GUI stops polling on success,
        // so this only happens on a replay).
        let err = backend
            .submit_voucher(pubkey_ss58, wpid.to_owned())
            .await
            .expect_err("second pull must fail");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 404);
                assert_eq!(msg, mullvad_api::INVALID_VOUCHER);
            }
            other => panic!("expected ApiError(404, INVALID_VOUCHER), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_wpid_cache_drops_on_final_server_verdict() {
        // The pulled-secret cache must not replay a secret the server
        // has definitively rejected: a ServerStatus outcome clears the
        // entry, and the next poll goes back to the pull path (404 ->
        // INVALID_VOUCHER), not an infinite replay of the dead secret.
        let (api_url, state) = spawn_warren_api().await;
        let seed = [67u8; 32];
        let pubkey_ss58 = address_for_seed(seed);

        // Pending entry WITHOUT a matching voucher row: the pull
        // succeeds (secret cached) but the register 400s (final).
        let (secret, _hash) = warren_api_server::generate_voucher_secret();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let wpid = "aaaabbbbccccdddd0000111122223333";
        state.pending_vouchers.put(wpid, &secret, now + 600);

        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .submit_voucher(pubkey_ss58.clone(), wpid.to_owned())
            .await
            .expect_err("register of an unknown voucher must fail");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 400);
                assert_eq!(msg, mullvad_api::INVALID_VOUCHER);
            }
            other => panic!("expected ApiError(400, INVALID_VOUCHER), got {other:?}"),
        }
        assert!(
            backend
                .pulled_unregistered
                .lock()
                .expect("not poisoned")
                .is_empty(),
            "final server verdict must clear the cached secret"
        );

        // Next poll: pull path again, mapping already consumed -> 404.
        let err = backend
            .submit_voucher(pubkey_ss58, wpid.to_owned())
            .await
            .expect_err("consumed mapping must 404");
        match err {
            rest::Error::ApiError(code, _) => assert_eq!(code.as_u16(), 404),
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_submit_voucher_with_unready_wpid_maps_invalid_voucher() {
        // Polling case: the webhook has not landed yet, so the pull
        // 404s. The GUI maps INVALID_VOUCHER to "keep polling".
        let (api_url, _state) = spawn_warren_api().await;
        let seed = [66u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        let client = client_with_seed(api_url, seed);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .submit_voucher(pubkey_ss58, "ffeeddccbbaa99887766554433221100".to_owned())
            .await
            .expect_err("unready wpid must fail");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 404);
                assert_eq!(msg, mullvad_api::INVALID_VOUCHER);
            }
            other => panic!("expected ApiError(404, INVALID_VOUCHER), got {other:?}"),
        }
    }

    // ===================================================================
    // Voucher error mapping - locks down the substring matching against
    // the human-readable bodies emitted by
    // `warren_api::handlers::subscription::register`. Should the upstream
    // strings be edited, these tests fail loudly and prompt a sync of
    // the substrings in `map_voucher_register_error`.
    // ===================================================================

    fn server_status(status: u16, body: &str) -> warren_api::ClientError {
        warren_api::ClientError::ServerStatus {
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
        // that should be diagnosable in the daemon log - fall through
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
        // Any non-`ServerStatus` warren-api error (transport, serde,
        // system clock pre-epoch, …) is infra down - must map to
        // `Aborted` to align with the convention of [`map_client_error`].
        // `BadClock` is a convenient no-arg variant for unit testing the
        // catch-all arm.
        let err = super::map_voucher_register_error(warren_api::ClientError::BadClock);
        match err {
            rest::Error::Aborted => {}
            other => panic!("expected Aborted, got {other:?}"),
        }
    }
}
