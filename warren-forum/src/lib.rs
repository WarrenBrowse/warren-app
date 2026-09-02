//! Community-forum wallet login (`POST /v1/forum/login`, warren-core doc 55)
//! and in-app bug report (`POST /v1/forum/report`), the shared, host-tested
//! logic behind both mobile FFI crates.
//!
//! A `warren://forum-login?sid=..&host=..` deep link asks the app to prove wallet
//! ownership to the connect provider. On both Android (`warren-jni`) and iOS
//! (`warren-ios`) the request is signed AND sent inside Rust (like every other
//! signed call), so only the opaque `sid` and the connect `host` cross the FFI
//! boundary and the wallet signature never surfaces to Kotlin / Swift. This
//! crate owns everything that decides the wire bytes, validates the deep-link
//! inputs, and maps the outcome, single-sourced so the two platforms cannot
//! drift; each FFI crate supplies the tokio runtime + transport that executes
//! the POST, and the `SigningKey` derived from its own secret store.

#![forbid(unsafe_code)]

use warren_identity::ed25519_dalek::SigningKey;
use warren_identity::signing::sign_request;

/// The single connect host accepted from a forum-login deep link. A hard
/// allowlist: a hostile link must not be able to point the wallet-signed request
/// at an attacker-controlled server (mirrors the desktop `ALLOWED_CONNECT_HOSTS`).
const ALLOWED_CONNECT_HOST: &str = "connect.warrenbrowse.com";

/// True iff `host` is the allowlisted connect host.
#[must_use]
pub fn is_allowed_connect_host(host: &str) -> bool {
    host == ALLOWED_CONNECT_HOST
}

/// The allowlisted connect host, for the flows that carry no deep link (the
/// in-app report, the sign-in code typed by hand).
#[must_use]
pub fn connect_host() -> &'static str {
    ALLOWED_CONNECT_HOST
}

/// True iff `sid` is exactly 32 lowercase hex chars (the forum SSO session id
/// shape; mirrors the desktop `parseForumLoginUrl` guard).
#[must_use]
pub fn is_valid_sid(sid: &str) -> bool {
    sid.len() == 32
        && sid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A sign-in code as a person types it: the 32 hex characters of the session
/// id, in any case, with any spaces or dashes a display may have grouped them
/// with. Returns the canonical sid, or `None` for anything else.
#[must_use]
pub fn normalize_sign_in_code(typed: &str) -> Option<String> {
    let cleaned: String = typed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    is_valid_sid(&cleaned).then_some(cleaned)
}

/// Build the best-effort cancel URL `POST /v1/session/<sid>/cancel`, or `None`
/// if `host`/`sid` are invalid. Tells the connect provider the user declined so
/// the waiting browser page unblocks instead of polling to timeout (mirrors the
/// desktop `cancelForumLogin`). Unsigned: it carries no wallet material.
#[must_use]
pub fn build_cancel_url(sid: &str, host: &str) -> Option<String> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return None;
    }
    Some(format!("https://{host}/v1/session/{sid}/cancel"))
}

/// The unsigned status URL `GET /v1/session/<sid>/status` the browser polls.
/// The app reads it once before signing: a `Date` header from the connect
/// host is a trusted clock to correct its own against, and a session already
/// gone is told apart from a refused signature before a signature is spent.
#[must_use]
pub fn build_status_url(sid: &str, host: &str) -> Option<String> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return None;
    }
    Some(format!("https://{host}/v1/session/{sid}/status"))
}

/// A signed request, transport-agnostic so the byte construction (host-tested)
/// and the network execution (per-platform, FFI-only) stay separable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedForumRequest {
    /// Absolute request URL.
    pub url: String,
    /// Header name/value pairs: `Content-Type` plus the four `X-Warren-*` auth
    /// headers, ready to attach verbatim.
    pub headers: Vec<(String, String)>,
    /// The canonical request body bytes: what was signed is what is sent.
    pub body: Vec<u8>,
}

/// Failure building a signed request. Deliberately opaque (one variant): the
/// caller maps it to the generic `Failed` outcome, and no cause with identity
/// material is ever surfaced or logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumRequestError {
    /// The host was not allowlisted, the sid was malformed, the report was
    /// not a JSON object, or the RNG / clock was unusable.
    Invalid,
}

/// Build the signed forum-login request for `sid` against `host`, signing with
/// `signing_key` (each platform derives it from its own secret store: Android
/// from the mnemonic, iOS from the wallet seed), stamped with the device clock.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, or the OS RNG / clock is unavailable.
pub fn build_signed_request(
    signing_key: &SigningKey,
    sid: &str,
    host: &str,
) -> Result<SignedForumRequest, ForumRequestError> {
    let timestamp = unix_secs_now().ok_or(ForumRequestError::Invalid)?;
    build_signed_request_at(signing_key, sid, host, timestamp)
}

/// [`build_signed_request`] with an explicit timestamp, for a caller that has
/// corrected the device clock against the server's (see
/// [`clock_offset_secs`]): the provider refuses a signature more than a minute
/// off its own clock, and a device that has never synchronised time is
/// otherwise refused on every attempt whatever the user does.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, or the OS RNG is unavailable.
pub fn build_signed_request_at(
    signing_key: &SigningKey,
    sid: &str,
    host: &str,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return Err(ForumRequestError::Invalid);
    }
    let body = format!("{{\"sid\":\"{sid}\"}}");
    signed_post(
        signing_key,
        host,
        "/v1/forum/login",
        body.into_bytes(),
        timestamp,
    )
}

/// Build the signed in-app report request. `report_json` is the report's
/// fields as one JSON object (the FFI caller assembles it from the form: the
/// field names are the connect contract's); `log_gz` is the gzipped redacted
/// problem report, attached as the `log_gz_b64` field when present. The body
/// is serialised exactly once here, so the signed bytes are the sent bytes.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if `report_json` is not a JSON object, if it
/// already carries a `log_gz_b64` field, or if the OS RNG is unavailable.
pub fn build_signed_report_request(
    signing_key: &SigningKey,
    report_json: &str,
    log_gz: Option<&[u8]>,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let mut report: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(report_json).map_err(|_| ForumRequestError::Invalid)?;
    if report.contains_key("log_gz_b64") {
        return Err(ForumRequestError::Invalid);
    }
    if let Some(gz) = log_gz {
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, gz);
        report.insert("log_gz_b64".to_owned(), serde_json::Value::String(b64));
    }
    let body = serde_json::to_vec(&serde_json::Value::Object(report))
        .map_err(|_| ForumRequestError::Invalid)?;
    signed_post(
        signing_key,
        ALLOWED_CONNECT_HOST,
        "/v1/forum/report",
        body,
        timestamp,
    )
}

fn signed_post(
    signing_key: &SigningKey,
    host: &str,
    path: &str,
    body: Vec<u8>,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let nonce = nonce_16().ok_or(ForumRequestError::Invalid)?;
    let sig = sign_request(signing_key, "POST", path, &body, timestamp, nonce);
    let mut headers: Vec<(String, String)> = sig
        .headers()
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
    Ok(SignedForumRequest {
        url: format!("https://{host}{path}"),
        headers,
        body,
    })
}

/// The device clock's offset from the server's, in seconds, from the `Date`
/// header of a TLS-authenticated response of the connect host: positive when
/// the device is behind. Add it to the device time to stamp a request the
/// server's 60 s window accepts. `None` when the header does not parse.
#[must_use]
pub fn clock_offset_secs(date_header: &str, device_now: u64) -> Option<i64> {
    let server = httpdate::parse_http_date(date_header.trim()).ok()?;
    let server_secs = server.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(i64::try_from(server_secs).ok()? - i64::try_from(device_now).ok()?)
}

/// The device clock now, shifted by `offset_secs` (the value of
/// [`clock_offset_secs`], or zero when none was measured), or `None` if the
/// clock is before the epoch or the shift overflows.
#[must_use]
pub fn timestamp_with_offset(offset_secs: i64) -> Option<u64> {
    let now = i64::try_from(unix_secs_now()?).ok()?;
    u64::try_from(now.checked_add(offset_secs)?).ok()
}

/// The forum identity an approved login carries back: the pairwise handle
/// (keyed derivation, so the client can learn it nowhere else) and the digest
/// slot the badge indexes. Mirrors the desktop `ForumIdentity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumIdentity {
    /// Proquint handle, `lusab-babad-dovok`.
    pub handle: String,
    /// Position in the broadcast activity digest; absent when the allocator
    /// had no room.
    pub notify_slot: Option<u32>,
}

/// True iff `handle` has the proquint shape the derivation produces (three
/// lowercase five-letter groups). Anything else is not ours and is dropped.
#[must_use]
pub fn is_forum_handle(handle: &str) -> bool {
    let groups: Vec<&str> = handle.split('-').collect();
    groups.len() == 3
        && groups
            .iter()
            .all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_lowercase()))
}

/// The identity out of an approved body, `None` when the body carries no
/// usable handle (an older provider): the login is still approved, the
/// identity is simply unknown, which is what the desktop does.
#[must_use]
pub fn parse_login_identity(body: &[u8]) -> Option<ForumIdentity> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let handle = value.get("handle")?.as_str()?;
    if !is_forum_handle(handle) {
        return None;
    }
    let notify_slot = value
        .get("notify_slot")
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    Some(ForumIdentity {
        handle: handle.to_owned(),
        notify_slot,
    })
}

/// Why an attempt failed before or below the provider's verdict. Coarse by
/// design: it names a class for the log and the report, never a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailReason {
    /// The FFI runtime was not initialised.
    Runtime,
    /// The signed request could not be built (input or RNG).
    Build,
    /// The request never got an HTTP answer (DNS, connect, TLS, timeout, or a
    /// tunnel that swallowed it).
    Transport,
    /// The provider answered with a status the outcome table does not name.
    Http(u16),
}

impl FailReason {
    /// The stable token the FFI envelope carries, e.g. `transport`, `http-502`.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            FailReason::Runtime => "runtime".to_owned(),
            FailReason::Build => "build".to_owned(),
            FailReason::Transport => "transport".to_owned(),
            FailReason::Http(status) => format!("http-{status}"),
        }
    }
}

/// Coarse outcome of a forum-login attempt. Kotlin / Swift map
/// [`Self::SubscriptionRequired`] and [`Self::ClockSkew`] to their own messages
/// and everything else to a generic failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForumLoginOutcome {
    /// The provider accepted the signature (the browser completes the login),
    /// with the identity it handed back when the body carried one.
    Approved(Option<ForumIdentity>),
    /// The wallet has never subscribed to Warren; forum access is refused (403).
    SubscriptionRequired,
    /// The device clock is outside the provider's accepted window, so the
    /// signature was refused (401 + connect's `clock_skew` token). The one
    /// failure the user can repair themselves.
    ClockSkew,
    /// The session is gone: expired, cancelled or already consumed (404).
    Expired,
    /// Any other failure, with its class.
    Failed(FailReason),
}

/// Connect's machine-readable 401 body for a clock outside the window. Frozen
/// wire detail: warren-connect's sso_flow test asserts the exact response bytes
/// `{"error":"clock_skew"}`, which is what this substring rides on.
const CLOCK_SKEW_BODY_TOKEN: &[u8] = br#""error":"clock_skew""#;

fn has_clock_skew_token(body: &[u8]) -> bool {
    body.windows(CLOCK_SKEW_BODY_TOKEN.len())
        .any(|w| w == CLOCK_SKEW_BODY_TOKEN)
}

/// Map an HTTP response to the outcome: 2xx approved (with the identity the
/// body carries), 403 subscription-required, 401 carrying connect's
/// `clock_skew` token a clock skew, 404 a dead session, anything else failed
/// with its status.
#[must_use]
pub fn outcome_for_response(status: u16, body: &[u8]) -> ForumLoginOutcome {
    match status {
        200..=299 => ForumLoginOutcome::Approved(parse_login_identity(body)),
        403 => ForumLoginOutcome::SubscriptionRequired,
        401 if has_clock_skew_token(body) => ForumLoginOutcome::ClockSkew,
        404 => ForumLoginOutcome::Expired,
        other => ForumLoginOutcome::Failed(FailReason::Http(other)),
    }
}

/// The JSON envelope returned across the FFI for `outcome`. Hand-built (no
/// derive) so both platforms decode the same shapes; the `ok`/`error` pair is
/// the frozen contract, `handle`, `notify_slot` and `reason` are additive.
/// Never carries any request context.
#[must_use]
pub fn envelope(outcome: &ForumLoginOutcome) -> String {
    match outcome {
        ForumLoginOutcome::Approved(None) => r#"{"ok":true}"#.to_owned(),
        ForumLoginOutcome::Approved(Some(identity)) => match identity.notify_slot {
            Some(slot) => format!(
                r#"{{"ok":true,"handle":"{}","notify_slot":{slot}}}"#,
                identity.handle
            ),
            None => format!(r#"{{"ok":true,"handle":"{}"}}"#, identity.handle),
        },
        ForumLoginOutcome::SubscriptionRequired => {
            r#"{"ok":false,"error":"subscription-required"}"#.to_owned()
        }
        ForumLoginOutcome::ClockSkew => r#"{"ok":false,"error":"clock-skew"}"#.to_owned(),
        ForumLoginOutcome::Expired => r#"{"ok":false,"error":"expired"}"#.to_owned(),
        ForumLoginOutcome::Failed(reason) => {
            format!(
                r#"{{"ok":false,"error":"error","reason":"{}"}}"#,
                reason.token()
            )
        }
    }
}

/// What the provider made of an in-app report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportOutcome {
    /// The topic exists; `logs` says whether the staff delivery completed
    /// (`attached`), partially failed after the topic was created
    /// (`partial`), or was not requested (`none`).
    Created {
        /// The forum topic id.
        topic_id: u64,
        /// Public topic URL.
        topic_url: String,
        /// The forum identity, when the body carried one.
        identity: Option<ForumIdentity>,
        /// `attached`, `partial` or `none`.
        logs: String,
    },
    /// Never paid and not staff (403): the guest help form is the channel.
    SubscriptionRequired,
    /// Device clock outside the window (401 + token).
    ClockSkew,
    /// Over the per-wallet or global budget (429).
    RateLimited,
    /// The gzipped report is over a size cap (413).
    TooLarge,
    /// A field is outside its caps (422): fix the form.
    Invalid,
    /// The provider failed on its own side (5xx): nothing the reporter can do.
    ServerError,
    /// Any other failure, with its class.
    Failed(FailReason),
}

/// Map the provider's answer to the report outcome.
#[must_use]
pub fn report_outcome_for_response(status: u16, body: &[u8]) -> ReportOutcome {
    match status {
        200..=299 => {
            let value: Option<serde_json::Value> = serde_json::from_slice(body).ok();
            let topic_id = value
                .as_ref()
                .and_then(|v| v.get("topic_id"))
                .and_then(serde_json::Value::as_u64);
            match topic_id {
                Some(topic_id) => {
                    let topic_url = value
                        .as_ref()
                        .and_then(|v| v.get("topic_url"))
                        .and_then(serde_json::Value::as_str)
                        .map_or_else(
                            || format!("https://forum.warrenbrowse.com/t/{topic_id}"),
                            str::to_owned,
                        );
                    let logs = value
                        .as_ref()
                        .and_then(|v| v.get("logs"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("none")
                        .to_owned();
                    ReportOutcome::Created {
                        topic_id,
                        topic_url,
                        identity: parse_login_identity(body),
                        logs,
                    }
                }
                None => ReportOutcome::Failed(FailReason::Http(status)),
            }
        }
        403 => ReportOutcome::SubscriptionRequired,
        401 if has_clock_skew_token(body) => ReportOutcome::ClockSkew,
        429 => ReportOutcome::RateLimited,
        413 => ReportOutcome::TooLarge,
        422 => ReportOutcome::Invalid,
        500..=599 => ReportOutcome::ServerError,
        other => ReportOutcome::Failed(FailReason::Http(other)),
    }
}

/// The FFI envelope for a report outcome. `ok` plus `error` is the contract;
/// on success `topic_id`, `topic_url`, `logs` and the identity ride along.
#[must_use]
pub fn report_envelope(outcome: &ReportOutcome) -> String {
    match outcome {
        ReportOutcome::Created {
            topic_id,
            topic_url,
            identity,
            logs,
        } => {
            let mut map = serde_json::Map::new();
            map.insert("ok".into(), serde_json::Value::Bool(true));
            map.insert("topic_id".into(), serde_json::Value::from(*topic_id));
            map.insert(
                "topic_url".into(),
                serde_json::Value::String(topic_url.clone()),
            );
            map.insert("logs".into(), serde_json::Value::String(logs.clone()));
            if let Some(identity) = identity {
                map.insert(
                    "handle".into(),
                    serde_json::Value::String(identity.handle.clone()),
                );
                if let Some(slot) = identity.notify_slot {
                    map.insert("notify_slot".into(), serde_json::Value::from(slot));
                }
            }
            serde_json::Value::Object(map).to_string()
        }
        ReportOutcome::SubscriptionRequired => {
            r#"{"ok":false,"error":"subscription-required"}"#.to_owned()
        }
        ReportOutcome::ClockSkew => r#"{"ok":false,"error":"clock-skew"}"#.to_owned(),
        ReportOutcome::RateLimited => r#"{"ok":false,"error":"rate-limited"}"#.to_owned(),
        ReportOutcome::TooLarge => r#"{"ok":false,"error":"too-large"}"#.to_owned(),
        ReportOutcome::Invalid => r#"{"ok":false,"error":"invalid"}"#.to_owned(),
        ReportOutcome::ServerError => r#"{"ok":false,"error":"server-error"}"#.to_owned(),
        ReportOutcome::Failed(reason) => {
            format!(
                r#"{{"ok":false,"error":"error","reason":"{}"}}"#,
                reason.token()
            )
        }
    }
}

/// 16 bytes of OS entropy for the request nonce. `None` only if the OS RNG is
/// unavailable.
fn nonce_16() -> Option<[u8; 16]> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).ok()?;
    Some(buf)
}

/// Current Unix time in seconds, or `None` if the clock is before the epoch.
fn unix_secs_now() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "0123456789abcdef0123456789abcdef";

    fn signing_key() -> SigningKey {
        // A fixed non-zero secret: the request must build and sign every time.
        SigningKey::from_bytes(&[0x11u8; 32])
    }

    #[test]
    fn only_the_connect_host_is_allowlisted() {
        assert!(is_allowed_connect_host("connect.warrenbrowse.com"));
        assert!(!is_allowed_connect_host("evil.example.com"));
        assert!(!is_allowed_connect_host(
            "connect.warrenbrowse.com.evil.com"
        ));
        assert!(!is_allowed_connect_host("api.warrenbrowse.com"));
        assert!(!is_allowed_connect_host(""));
        assert_eq!(connect_host(), "connect.warrenbrowse.com");
    }

    #[test]
    fn sid_shape_is_enforced() {
        assert!(is_valid_sid(SID));
        assert!(!is_valid_sid("0123456789ABCDEF0123456789abcdef")); // uppercase
        assert!(!is_valid_sid("0123456789abcdef")); // too short
        assert!(!is_valid_sid("0123456789abcdef0123456789abcdeg")); // non-hex
    }

    #[test]
    fn a_typed_sign_in_code_is_normalized_to_the_sid() {
        // The approval page shows the id in one run; a person may type it in
        // groups, in capitals, or paste it with a stray space.
        assert_eq!(normalize_sign_in_code(SID).as_deref(), Some(SID));
        assert_eq!(
            normalize_sign_in_code(" 0123 4567-89AB cdef\n0123456789abcdef ").as_deref(),
            Some(SID)
        );
        assert_eq!(normalize_sign_in_code("0123456789abcdef"), None);
        assert_eq!(
            normalize_sign_in_code("not a code at all, thirty-two chars!"),
            None
        );
    }

    #[test]
    fn cancel_and_status_urls_are_built_only_for_a_valid_host_and_sid() {
        assert_eq!(
            build_cancel_url(SID, "connect.warrenbrowse.com").as_deref(),
            Some(
                "https://connect.warrenbrowse.com/v1/session/0123456789abcdef0123456789abcdef/cancel"
            )
        );
        assert_eq!(build_cancel_url(SID, "evil.example.com"), None);
        assert_eq!(build_cancel_url("NOTHEX", "connect.warrenbrowse.com"), None);
        assert_eq!(
            build_status_url(SID, "connect.warrenbrowse.com").as_deref(),
            Some(
                "https://connect.warrenbrowse.com/v1/session/0123456789abcdef0123456789abcdef/status"
            )
        );
        assert_eq!(build_status_url(SID, "evil.example.com"), None);
    }

    #[test]
    fn build_signed_request_refuses_a_non_allowlisted_host_or_bad_sid() {
        assert_eq!(
            build_signed_request(&signing_key(), SID, "evil.example.com"),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_request(
                &signing_key(),
                "0123456789ABCDEF0123456789abcdef",
                "connect.warrenbrowse.com"
            ),
            Err(ForumRequestError::Invalid)
        );
    }

    fn header(req: &SignedForumRequest, name: &str) -> String {
        req.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("missing header {name}"))
    }

    fn assert_wire_parity(req: &SignedForumRequest, path: &str) {
        use sha2::{Digest as _, Sha256};
        use warren_identity::ed25519_dalek::{Signature, Verifier};

        assert_eq!(header(req, "Content-Type"), "application/json");
        // The signature verifies against the canonical POST message the server
        // reconstructs (proves wire parity, not just header presence). The nonce
        // + timestamp are read back from the headers since they are random.
        let timestamp: u64 = header(req, "X-Warren-Timestamp").parse().unwrap();
        let nonce_hex = header(req, "X-Warren-Nonce");
        let body_hash_hex = hex::encode(Sha256::digest(&req.body));
        let expected =
            warren_identity::canonical_message("POST", path, timestamp, &nonce_hex, &body_hash_hex);
        let sig_bytes: [u8; 64] = hex::decode(header(req, "X-Warren-Sig"))
            .unwrap()
            .try_into()
            .unwrap();
        signing_key()
            .verifying_key()
            .verify(expected.as_bytes(), &Signature::from_bytes(&sig_bytes))
            .expect("signature must verify against the canonical POST message");
    }

    #[test]
    fn build_signed_request_produces_a_wire_parity_signature() {
        let req = build_signed_request(&signing_key(), SID, "connect.warrenbrowse.com")
            .expect("a valid host + sid must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
        assert_wire_parity(&req, "/v1/forum/login");
    }

    #[test]
    fn an_explicit_timestamp_is_the_one_signed() {
        // A device whose clock is off signs with the server-corrected time;
        // the header must carry exactly that value or the correction is moot.
        let req = build_signed_request_at(
            &signing_key(),
            SID,
            "connect.warrenbrowse.com",
            1_800_000_000,
        )
        .expect("builds");
        assert_eq!(header(&req, "X-Warren-Timestamp"), "1800000000");
        assert_wire_parity(&req, "/v1/forum/login");
    }

    #[test]
    fn clock_offset_is_read_from_an_http_date() {
        // Wed, 02 Sep 2026 17:50:28 GMT is 1788371428; a device 90 s behind
        // reads +90, one 3 s ahead reads -3, and garbage reads nothing.
        assert_eq!(
            clock_offset_secs("Wed, 02 Sep 2026 17:50:28 GMT", 1_788_371_428 - 90),
            Some(90)
        );
        assert_eq!(
            clock_offset_secs(" Wed, 02 Sep 2026 17:50:28 GMT ", 1_788_371_428 + 3),
            Some(-3)
        );
        assert_eq!(clock_offset_secs("yesterday", 1), None);
        assert!(timestamp_with_offset(0).is_some());
        assert!(timestamp_with_offset(i64::MIN).is_none());
    }

    #[test]
    fn report_request_signs_the_body_it_sends_with_the_log_attached() {
        let req = build_signed_report_request(
            &signing_key(),
            r#"{"platform":"android","area":"other","frequency":"always","what_happened":"The sign-in button does nothing at all, twice."}"#,
            Some(b"\x1f\x8b\x08\x00fake"),
            1_800_000_000,
        )
        .expect("a JSON object report builds");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/report");
        let body: serde_json::Value = serde_json::from_slice(&req.body).expect("json body");
        assert_eq!(body["platform"], "android");
        assert_eq!(
            body["what_happened"],
            "The sign-in button does nothing at all, twice."
        );
        assert_eq!(body["log_gz_b64"], "H4sIAGZha2U=");
        assert_wire_parity(&req, "/v1/forum/report");

        let without = build_signed_report_request(
            &signing_key(),
            r#"{"platform":"android"}"#,
            None,
            1_800_000_000,
        )
        .expect("builds");
        let body: serde_json::Value = serde_json::from_slice(&without.body).expect("json body");
        assert!(body.get("log_gz_b64").is_none());
    }

    #[test]
    fn report_request_refuses_a_non_object_or_a_smuggled_log_field() {
        assert_eq!(
            build_signed_report_request(&signing_key(), "[1,2]", None, 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_report_request(&signing_key(), "not json", None, 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_report_request(&signing_key(), r#"{"log_gz_b64":"AAAA"}"#, Some(b"x"), 1),
            Err(ForumRequestError::Invalid),
            "the log rides in exactly one field, the one this crate fills"
        );
    }

    #[test]
    fn status_maps_to_the_desktop_outcomes() {
        assert_eq!(
            outcome_for_response(200, b""),
            ForumLoginOutcome::Approved(None)
        );
        assert_eq!(
            outcome_for_response(204, b""),
            ForumLoginOutcome::Approved(None)
        );
        assert_eq!(
            outcome_for_response(403, b""),
            ForumLoginOutcome::SubscriptionRequired
        );
        assert_eq!(
            outcome_for_response(401, b""),
            ForumLoginOutcome::Failed(FailReason::Http(401))
        );
        assert_eq!(outcome_for_response(404, b""), ForumLoginOutcome::Expired);
        assert_eq!(
            outcome_for_response(500, b""),
            ForumLoginOutcome::Failed(FailReason::Http(500))
        );
    }

    #[test]
    fn an_approved_body_yields_the_identity_the_desktop_reads() {
        // Same rules as the desktop `parseForumIdentityResponse`: the proquint
        // shape decides, the slot is optional, and a body without a usable
        // handle still approves the login.
        assert_eq!(
            outcome_for_response(
                200,
                br#"{"status":"approved","handle":"lusab-babad-dovok","notify_slot":42}"#
            ),
            ForumLoginOutcome::Approved(Some(ForumIdentity {
                handle: "lusab-babad-dovok".into(),
                notify_slot: Some(42),
            }))
        );
        assert_eq!(
            outcome_for_response(
                200,
                br#"{"status":"approved","handle":"lusab-babad-dovok"}"#
            ),
            ForumLoginOutcome::Approved(Some(ForumIdentity {
                handle: "lusab-babad-dovok".into(),
                notify_slot: None,
            }))
        );
        assert_eq!(
            outcome_for_response(200, br#"{"status":"approved","handle":"Admin Bob"}"#),
            ForumLoginOutcome::Approved(None)
        );
        assert!(is_forum_handle("lusab-babad-dovok"));
        assert!(!is_forum_handle("lusab-babad"));
        assert!(!is_forum_handle("LUSAB-babad-dovok"));
    }

    #[test]
    fn a_401_carrying_the_clock_token_is_a_clock_skew() {
        // The one 401 the user can repair themselves: connect names it with a
        // frozen JSON token (its sso_flow test pins the exact bytes) so the app
        // can say "fix your clock" instead of "try again in a moment", which
        // was the dead end every 2026-08-18 reporter hit.
        assert_eq!(
            outcome_for_response(401, br#"{"error":"clock_skew"}"#),
            ForumLoginOutcome::ClockSkew
        );
        // The token decides, not the 401: any other body stays generic.
        assert_eq!(
            outcome_for_response(401, b"timestamp outside accepted window"),
            ForumLoginOutcome::Failed(FailReason::Http(401))
        );
        // And the 401 decides too: the token on another status means nothing.
        assert_eq!(
            outcome_for_response(500, br#"{"error":"clock_skew"}"#),
            ForumLoginOutcome::Failed(FailReason::Http(500))
        );
        assert_eq!(
            outcome_for_response(200, br#"{"error":"clock_skew"}"#),
            ForumLoginOutcome::Approved(None)
        );
    }

    #[test]
    fn envelope_json_matches_the_ffi_contract() {
        assert_eq!(
            envelope(&ForumLoginOutcome::Approved(None)),
            r#"{"ok":true}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Approved(Some(ForumIdentity {
                handle: "lusab-babad-dovok".into(),
                notify_slot: Some(42),
            }))),
            r#"{"ok":true,"handle":"lusab-babad-dovok","notify_slot":42}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Approved(Some(ForumIdentity {
                handle: "lusab-babad-dovok".into(),
                notify_slot: None,
            }))),
            r#"{"ok":true,"handle":"lusab-babad-dovok"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::SubscriptionRequired),
            r#"{"ok":false,"error":"subscription-required"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::ClockSkew),
            r#"{"ok":false,"error":"clock-skew"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Expired),
            r#"{"ok":false,"error":"expired"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Failed(FailReason::Transport)),
            r#"{"ok":false,"error":"error","reason":"transport"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Failed(FailReason::Http(502))),
            r#"{"ok":false,"error":"error","reason":"http-502"}"#
        );
        assert_eq!(
            envelope(&ForumLoginOutcome::Failed(FailReason::Runtime)),
            r#"{"ok":false,"error":"error","reason":"runtime"}"#
        );
    }

    #[test]
    fn report_outcomes_follow_the_connect_status_table() {
        let created = report_outcome_for_response(
            201,
            br#"{"status":"created","topic_id":142,"topic_url":"https://forum.warrenbrowse.com/t/142","handle":"lusab-babad-dovok","notify_slot":7,"logs":"attached"}"#,
        );
        assert_eq!(
            created,
            ReportOutcome::Created {
                topic_id: 142,
                topic_url: "https://forum.warrenbrowse.com/t/142".into(),
                identity: Some(ForumIdentity {
                    handle: "lusab-babad-dovok".into(),
                    notify_slot: Some(7),
                }),
                logs: "attached".into(),
            }
        );
        assert_eq!(
            report_envelope(&created),
            r#"{"handle":"lusab-babad-dovok","logs":"attached","notify_slot":7,"ok":true,"topic_id":142,"topic_url":"https://forum.warrenbrowse.com/t/142"}"#
        );
        assert_eq!(
            report_outcome_for_response(201, b"{}"),
            ReportOutcome::Failed(FailReason::Http(201)),
            "a success without a topic id is not a created topic"
        );
        assert_eq!(
            report_outcome_for_response(403, b""),
            ReportOutcome::SubscriptionRequired
        );
        assert_eq!(
            report_outcome_for_response(401, br#"{"error":"clock_skew"}"#),
            ReportOutcome::ClockSkew
        );
        assert_eq!(
            report_outcome_for_response(401, b""),
            ReportOutcome::Failed(FailReason::Http(401))
        );
        assert_eq!(
            report_outcome_for_response(429, b""),
            ReportOutcome::RateLimited
        );
        assert_eq!(
            report_outcome_for_response(413, b""),
            ReportOutcome::TooLarge
        );
        assert_eq!(
            report_outcome_for_response(422, br#"{"error":"invalid_report"}"#),
            ReportOutcome::Invalid
        );
        assert_eq!(
            report_outcome_for_response(502, b""),
            ReportOutcome::ServerError
        );
        assert_eq!(
            report_envelope(&ReportOutcome::RateLimited),
            r#"{"ok":false,"error":"rate-limited"}"#
        );
        assert_eq!(
            report_envelope(&ReportOutcome::Failed(FailReason::Transport)),
            r#"{"ok":false,"error":"error","reason":"transport"}"#
        );
    }
}
