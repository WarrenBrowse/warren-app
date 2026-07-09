//! Abstract trait for the account-level operations, so the daemon drives
//! them against the Warren backend instead of `AccountsProxy`/`api.mullvad.net`.
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

/// Refuses to act when the account the daemon designates differs from the
/// identity the SDK client signs with. Under a signer/device desync,
/// proceeding would credit a purchase to, serve the balance of, or delete
/// a wallet the user does not own; the conflict must surface instead.
/// No-log: only short prefixes of the two addresses are logged.
fn check_account_matches_identity(account: &str, identity: &str) -> Result<(), rest::Error> {
    if account == identity {
        return Ok(());
    }
    fn redact(s: &str) -> &str {
        s.get(..8).unwrap_or(s)
    }
    log::error!(
        "warren account backend: requested account {}... but the signing identity is {}...; \
         refusing (identity desync)",
        redact(account),
        redact(identity)
    );
    Err(rest::Error::ApiError(
        rest::StatusCode::CONFLICT,
        mullvad_api::WARREN_IDENTITY_DESYNC.to_owned(),
    ))
}

impl WarrenAccountBackend for WarrenRemoteAccountBackend {
    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            check_account_matches_identity(&account, &client.address())?;
            let resp = client.get_subscription().await.map_err(map_client_error)?;
            let expiry = expiry_from_unix_secs(resp.expires_at)?;
            Ok(AccountData {
                id: account,
                expiry,
            })
        })
    }

    fn delete_account(&self, account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            check_account_matches_identity(&account, &client.address())?;
            client.delete_account().await.map_err(map_client_error)
        })
    }

    fn submit_voucher(
        &self,
        account: AccountNumber,
        voucher: String,
    ) -> BoxFut<Result<VoucherSubmission, rest::Error>> {
        let client = self.client.clone();
        let pubkey_ss58 = client.address();
        let pulled_unregistered = self.pulled_unregistered.clone();
        Box::pin(async move {
            // Checked BEFORE the wpid pull below: the pull consumes the
            // single-use server-side mapping, so running it under a
            // desynced identity would burn the PAID voucher into the
            // wrong wallet with no recovery.
            check_account_matches_identity(&account, &pubkey_ss58)?;
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
                            // Webhook not landed yet (or id expired):
                            // the GUI keeps polling on this signal.
                            None => {
                                return Err(rest::Error::ApiError(
                                    rest::StatusCode::NOT_FOUND,
                                    mullvad_api::VOUCHER_NOT_READY.to_owned(),
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
            // Pre-register expiry, so a 409 after a lost response can be
            // told apart from a genuinely spent voucher. `None` = fetch
            // failed, recovery disabled; `Some(0)` = no subscription yet.
            let baseline_expiry = match client.get_subscription().await {
                Ok(r) => Some(r.expires_at),
                Err(warren_api::ClientError::ServerStatus { status: 404, .. }) => Some(0),
                Err(_) => None,
            };

            // The pull above consumed the single-use mapping: a
            // transient register failure here would burn the paid
            // voucher. Retry transport failures and 5xx a few times
            // (4xx are final); on a pulled wpid the cache above lets
            // the NEXT GUI poll keep retrying the register even after
            // these retries fail.
            let mut resp = None;
            let mut last_err = None;
            let mut retried_after_failure = false;
            for attempt in 0u32..3 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    retried_after_failure = true;
                }
                match client.register_with_voucher(&req).await {
                    Ok(r) => {
                        resp = Some(r);
                        break;
                    }
                    Err(e @ warren_api::ClientError::ServerStatus { status, .. })
                        if status < 500 =>
                    {
                        if let Some(wpid) = &wpid {
                            pulled_unregistered
                                .lock()
                                .expect("not poisoned")
                                .remove(wpid);
                        }
                        let mapped = map_voucher_register_error(e);
                        // An earlier attempt may have landed with its
                        // response lost: a replay then 409s although the
                        // account WAS credited. Verify against the
                        // baseline and report the success it is.
                        if retried_after_failure
                            && matches!(&mapped, rest::Error::ApiError(_, code)
                                        if code == mullvad_api::VOUCHER_USED)
                            && let Some(baseline) = baseline_expiry
                            && let Ok(sub) = client.get_subscription().await
                            && sub.expires_at > baseline
                        {
                            let now = chrono::Utc::now().timestamp().max(0) as u64;
                            return Ok(VoucherSubmission {
                                new_expiry: expiry_from_unix_secs(sub.expires_at)?,
                                time_added: sub.expires_at.saturating_sub(baseline.max(now)),
                            });
                        }
                        return Err(mapped);
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
            // The voucher's OWN granted duration, straight from the
            // server. Deriving it from `expires_at - now` would report
            // the account's total remaining time and over-report for a
            // user who still had a balance ("1 year added" when the
            // voucher only granted a month on top of 11 existing).
            let time_added = resp.added_secs;
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
            {
                Some(mullvad_api::INVALID_VOUCHER)
            } else if body.contains("voucher was cancelled") || body.contains("voucher expired") {
                // The user typed a real code that is past its deadline or
                // revoked; "invalid" would send them into retype loops.
                Some(mullvad_api::VOUCHER_EXPIRED)
            } else if body.contains("voucher already redeemed")
                || body.contains("voucher redemption limit reached")
            {
                // Single-use already redeemed, or a multi-account voucher
                // at its quota: in both cases the code is spent and can
                // no longer be redeemed ("already used").
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

    // WarrenRemoteAccountBackend adapter tests: the app-side DTO <->
    // mullvad_types / ClientError <-> rest::Error mapping and the stateful
    // voucher orchestration. Only the HTTP peer is mocked; that the client's
    // routes/methods/headers match a real server is proven in warren-core
    // (`warren-api/tests/sdk_client_wire_compat.rs`), so this repo keeps no
    // warren-core dependency.

    use std::sync::{Arc as TestArc, RwLock};
    use warren_identity::WarrenIdentity;
    use zeroize::Zeroizing;

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs()
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
        // Nominal case: a 200 subscription body -> backend.get_data()
        // returns AccountData with expiry reconstructed from expires_at.
        let mut server = mockito::Server::new_async().await;
        let sub = server
            .mock("GET", "/v1/subscription")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"expires_at":1700000000}"#)
            .create_async()
            .await;
        let seed = [61u8; 32];
        let pubkey_ss58 = address_for_seed(seed);

        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));
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
        sub.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_returns_apierror_404_when_no_sub() {
        // Critical regression: if the ClientError -> rest::Error mapping
        // loses the 404 status, the caller (`handle_account_data_result`)
        // interprets a generic error instead of "non-existent account"
        // -> degraded UX + inconsistent device.json state.
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/v1/subscription")
            .with_status(404)
            .create_async()
            .await;
        let seed = [62u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .get_data(address_for_seed(seed))
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
    async fn warren_remote_account_delete_issues_signed_delete() {
        // Nominal case: delete_account issues the signed DELETE /v1/account
        // and reports success on the server's 204. The mock assertion is
        // what proves the request went out (the server-side effect itself
        // is covered by warren-core's own tests).
        let mut server = mockito::Server::new_async().await;
        let del = server
            .mock("DELETE", "/v1/account")
            .with_status(204)
            .create_async()
            .await;
        let seed = [63u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        backend
            .delete_account(address_for_seed(seed))
            .await
            .expect("delete OK");

        del.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_returns_apierror_404_when_no_sub() {
        // Regression: if we try to delete a non-existent sub, the
        // backend must propagate 404 -> caller can decide to ignore it
        // or log it cleanly (vs generic error).
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("DELETE", "/v1/account")
            .with_status(404)
            .create_async()
            .await;
        let seed = [64u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .delete_account(address_for_seed(seed))
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
        // The pull mapping is single-use: `expect(1)` on the 200 mock
        // hands the second poll over to the 404 mock, reproducing the
        // server-side consumption without a real server.
        let mut server = mockito::Server::new_async().await;
        let wpid = "00112233445566778899aabbccddeeff";
        let checkout = format!("/v1/checkout/{wpid}/voucher");
        // Returning user: the account already had ~11 months, so the new
        // expiry is ~1 year out while THIS voucher granted only 30 days.
        // The two numbers must not be conflated (that was the bug).
        let voucher_added = 30 * 86_400;
        let expires_at = now_secs() + 365 * 86_400;
        let pull_ok = server
            .mock("GET", checkout.as_str())
            .with_status(200)
            .with_body(r#"{"voucher_secret":"XXXX-YYYY-ZZZZ-WWWW"}"#)
            .expect(1)
            .create_async()
            .await;
        let pull_gone = server
            .mock("GET", checkout.as_str())
            .with_status(404)
            .create_async()
            .await;
        let register = server
            .mock("POST", "/v1/register")
            .with_status(201)
            .with_body(format!(
                r#"{{"expires_at":{expires_at},"added_secs":{voucher_added}}}"#
            ))
            .create_async()
            .await;
        let seed = [65u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let submission = backend
            .submit_voucher(pubkey_ss58.clone(), wpid.to_owned())
            .await
            .expect("wpid pull + redeem must succeed");
        assert_eq!(
            submission.time_added, voucher_added,
            "time_added must be the voucher's granted duration, NOT the \
             account's total remaining time (expires_at - now)"
        );
        pull_ok.assert_async().await;
        register.assert_async().await;

        // The mapping is single-use: a replay poll on the same wpid
        // reports not-ready (the GUI stops polling on success).
        let err = backend
            .submit_voucher(pubkey_ss58, wpid.to_owned())
            .await
            .expect_err("second pull must fail");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 404);
                assert_eq!(msg, mullvad_api::VOUCHER_NOT_READY);
            }
            other => panic!("expected ApiError(404, VOUCHER_NOT_READY), got {other:?}"),
        }
        pull_gone.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_wpid_cache_drops_on_final_server_verdict() {
        // The pulled-secret cache must not replay a secret the server
        // has definitively rejected: a ServerStatus outcome clears the
        // entry, and the next poll goes back to the pull path (404 ->
        // INVALID_VOUCHER), not an infinite replay of the dead secret.
        let mut server = mockito::Server::new_async().await;
        let wpid = "aaaabbbbccccdddd0000111122223333";
        let checkout = format!("/v1/checkout/{wpid}/voucher");
        // Pull succeeds (secret cached), then register 400s (final).
        let _pull_ok = server
            .mock("GET", checkout.as_str())
            .with_status(200)
            .with_body(r#"{"voucher_secret":"XXXX-YYYY-ZZZZ-WWWW"}"#)
            .expect(1)
            .create_async()
            .await;
        let _pull_gone = server
            .mock("GET", checkout.as_str())
            .with_status(404)
            .create_async()
            .await;
        let _register = server
            .mock("POST", "/v1/register")
            .with_status(400)
            .with_body(r#"{"error":"voucher unknown or invalid"}"#)
            .create_async()
            .await;
        let seed = [67u8; 32];
        let pubkey_ss58 = address_for_seed(seed);
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

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
    async fn warren_remote_submit_voucher_with_unready_wpid_maps_not_ready() {
        // The webhook has not landed yet, so the pull 404s. The GUI
        // keeps polling on the dedicated not-ready signal and the
        // daemon logs it at debug, not error.
        let mut server = mockito::Server::new_async().await;
        let wpid = "ffeeddccbbaa99887766554433221100";
        let _pull = server
            .mock("GET", format!("/v1/checkout/{wpid}/voucher").as_str())
            .with_status(404)
            .create_async()
            .await;
        let seed = [66u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .submit_voucher(address_for_seed(seed), wpid.to_owned())
            .await
            .expect_err("unready wpid must fail");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 404);
                assert_eq!(msg, mullvad_api::VOUCHER_NOT_READY);
            }
            other => panic!("expected ApiError(404, VOUCHER_NOT_READY), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn register_retries_5xx_then_succeeds() {
        let mut server = mockito::Server::new_async().await;
        let _sub = server
            .mock("GET", "/v1/subscription")
            .with_status(404)
            .create_async()
            .await;
        let _r503 = server
            .mock("POST", "/v1/register")
            .with_status(503)
            .expect(1)
            .create_async()
            .await;
        let expires_at = now_secs() + 30 * 86_400;
        let r_ok = server
            .mock("POST", "/v1/register")
            .with_status(201)
            .with_body(format!(
                r#"{{"expires_at":{expires_at},"added_secs":2592000}}"#
            ))
            .create_async()
            .await;
        let seed = [70u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let submission = backend
            .submit_voucher(address_for_seed(seed), "AAAA-BBBB-CCCC-DDDD".to_owned())
            .await
            .expect("a transient 5xx must be retried, not surfaced as final");
        assert_eq!(submission.time_added, 2_592_000);
        r_ok.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn conflict_after_lost_response_reports_success_when_expiry_advanced() {
        // Attempt 1 landed but its response was lost; the replay 409s
        // although the account WAS credited.
        let mut server = mockito::Server::new_async().await;
        let expires_at = now_secs() + 30 * 86_400;
        let _sub_baseline = server
            .mock("GET", "/v1/subscription")
            .with_status(404)
            .expect(1)
            .create_async()
            .await;
        let _sub_after = server
            .mock("GET", "/v1/subscription")
            .with_status(200)
            .with_body(format!(r#"{{"expires_at":{expires_at}}}"#))
            .create_async()
            .await;
        let _r503 = server
            .mock("POST", "/v1/register")
            .with_status(503)
            .expect(1)
            .create_async()
            .await;
        let _r409 = server
            .mock("POST", "/v1/register")
            .with_status(409)
            .with_body(r#"{"error":"voucher already redeemed"}"#)
            .create_async()
            .await;
        let seed = [71u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let submission = backend
            .submit_voucher(address_for_seed(seed), "AAAA-BBBB-CCCC-DDDD".to_owned())
            .await
            .expect("409 after a lost response with advanced expiry is a success");
        assert_eq!(submission.new_expiry.timestamp() as u64, expires_at);
        let now = now_secs();
        assert!(
            submission.time_added >= expires_at - now - 5
                && submission.time_added <= expires_at - now + 5,
            "time_added must approximate the credited duration, got {}",
            submission.time_added
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn conflict_without_prior_retry_propagates_voucher_used() {
        let mut server = mockito::Server::new_async().await;
        let _sub = server
            .mock("GET", "/v1/subscription")
            .with_status(404)
            .create_async()
            .await;
        let _r409 = server
            .mock("POST", "/v1/register")
            .with_status(409)
            .with_body(r#"{"error":"voucher already redeemed"}"#)
            .create_async()
            .await;
        let seed = [72u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .submit_voucher(address_for_seed(seed), "AAAA-BBBB-CCCC-DDDD".to_owned())
            .await
            .expect_err("a first-attempt 409 is a genuine already-used");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409);
                assert_eq!(msg, mullvad_api::VOUCHER_USED);
            }
            other => panic!("expected ApiError(409, VOUCHER_USED), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn conflict_after_retry_without_expiry_advance_propagates_voucher_used() {
        let mut server = mockito::Server::new_async().await;
        let expires_at = now_secs() + 10 * 86_400;
        let _sub = server
            .mock("GET", "/v1/subscription")
            .with_status(200)
            .with_body(format!(r#"{{"expires_at":{expires_at}}}"#))
            .create_async()
            .await;
        let _r503 = server
            .mock("POST", "/v1/register")
            .with_status(503)
            .expect(1)
            .create_async()
            .await;
        let _r409 = server
            .mock("POST", "/v1/register")
            .with_status(409)
            .with_body(r#"{"error":"voucher already redeemed"}"#)
            .create_async()
            .await;
        let seed = [73u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .submit_voucher(address_for_seed(seed), "AAAA-BBBB-CCCC-DDDD".to_owned())
            .await
            .expect_err("unchanged expiry means the voucher was spent elsewhere");
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409);
                assert_eq!(msg, mullvad_api::VOUCHER_USED);
            }
            other => panic!("expected ApiError(409, VOUCHER_USED), got {other:?}"),
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
    fn map_voucher_register_error_cancelled_410_maps_voucher_expired() {
        // A revoked voucher is "no longer valid", not "wrong code": the
        // user typed it correctly and must not be sent into retype loops.
        let err = super::map_voucher_register_error(server_status(
            410,
            r#"{"error":"voucher was cancelled by the admin"}"#,
        ));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 410);
                assert_eq!(msg, mullvad_api::VOUCHER_EXPIRED);
            }
            other => panic!("expected ApiError(410, VOUCHER_EXPIRED), got {other:?}"),
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
    fn map_voucher_register_error_exhausted_409_maps_voucher_used() {
        // A multi-account voucher whose redemption quota is reached can
        // no longer be redeemed: the user must see "already used", not a
        // generic "an error occurred".
        let err = super::map_voucher_register_error(server_status(
            409,
            r#"{"error":"voucher redemption limit reached"}"#,
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
    fn map_voucher_register_error_expired_410_maps_voucher_expired() {
        let err =
            super::map_voucher_register_error(server_status(410, r#"{"error":"voucher expired"}"#));
        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 410);
                assert_eq!(msg, mullvad_api::VOUCHER_EXPIRED);
            }
            other => panic!("expected ApiError(410, VOUCHER_EXPIRED), got {other:?}"),
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_get_data_refuses_a_mismatched_account() {
        // Defense in depth for the identity-desync class of bug: serving
        // the signing identity's balance labeled with a DIFFERENT
        // requested account silently masks a signer/device divergence
        // (the GUI shows another wallet's subscription as the user's).
        // The backend must refuse loudly and send NO request.
        let mut server = mockito::Server::new_async().await;
        let sub = server
            .mock("GET", "/v1/subscription")
            .with_status(200)
            .with_body(r#"{"expires_at":1700000000}"#)
            .expect(0)
            .create_async()
            .await;
        let seed = [73u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .get_data(address_for_seed([74u8; 32]))
            .await
            .expect_err("a mismatched account must be refused");

        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409, "desync surfaces as a conflict");
                assert_eq!(msg, mullvad_api::WARREN_IDENTITY_DESYNC);
            }
            other => panic!("expected ApiError(409, WARREN_IDENTITY_DESYNC), got {other:?}"),
        }
        sub.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_submit_voucher_refuses_a_mismatched_account_before_pulling() {
        // If the identity check ran AFTER the wpid pull, the single-use
        // server-side mapping would be consumed and the PAID voucher
        // registered under the signing identity instead of the account
        // the user sees. No HTTP traffic may happen for a mismatched
        // account.
        let mut server = mockito::Server::new_async().await;
        let pull = server
            .mock(
                "GET",
                mockito::Matcher::Regex("^/v1/checkout/.*".to_owned()),
            )
            .expect(0)
            .create_async()
            .await;
        let register = server
            .mock("POST", "/v1/register")
            .expect(0)
            .create_async()
            .await;
        let seed = [75u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));
        let wpid = "aaaabbbbccccddddeeeeffff00001111".to_owned();

        let err = backend
            .submit_voucher(address_for_seed([76u8; 32]), wpid)
            .await
            .expect_err("a mismatched account must be refused");

        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409, "desync surfaces as a conflict");
                assert_eq!(msg, mullvad_api::WARREN_IDENTITY_DESYNC);
            }
            other => panic!("expected ApiError(409, WARREN_IDENTITY_DESYNC), got {other:?}"),
        }
        pull.assert_async().await;
        register.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_delete_account_refuses_a_mismatched_account() {
        // Deleting under a desync would erase the signing identity's
        // account server-side while the user believes they are deleting
        // the displayed one: destructive, so the guard applies here too.
        let mut server = mockito::Server::new_async().await;
        let del = server
            .mock("DELETE", "/v1/account")
            .expect(0)
            .create_async()
            .await;
        let seed = [77u8; 32];
        let backend = WarrenRemoteAccountBackend::new(client_with_seed(server.url(), seed));

        let err = backend
            .delete_account(address_for_seed([78u8; 32]))
            .await
            .expect_err("a mismatched account must be refused");

        match err {
            rest::Error::ApiError(code, msg) => {
                assert_eq!(code.as_u16(), 409, "desync surfaces as a conflict");
                assert_eq!(msg, mullvad_api::WARREN_IDENTITY_DESYNC);
            }
            other => panic!("expected ApiError(409, WARREN_IDENTITY_DESYNC), got {other:?}"),
        }
        del.assert_async().await;
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
