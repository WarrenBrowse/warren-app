//! Community-forum wallet login (`POST /v1/forum/login`, warren-core doc 55),
//! the host-tested half.
//!
//! A `warren://forum-login?sid=..&host=..` deep link asks the app to prove wallet
//! ownership to the connect provider. On Android there is no daemon: the request
//! is signed AND sent inside Rust (like every other signed call), so only the
//! opaque `sid` and the connect `host` cross the JNI boundary and the wallet
//! signature never surfaces to Kotlin. Everything that decides the wire bytes,
//! validates the deep-link inputs, and maps the outcome lives here so it is
//! unit-testable off-device; the Android-only `android_jni` layer supplies the
//! tokio runtime and the reqwest transport that actually executes the POST.

use crate::wallet::{ForumLoginSigned, WalletError, sign_forum_login};

/// The single connect host accepted from a forum-login deep link. A hard
/// allowlist: a hostile link must not be able to point the wallet-signed request
/// at an attacker-controlled server (mirrors the desktop `ALLOWED_CONNECT_HOSTS`).
const ALLOWED_CONNECT_HOST: &str = "connect.warrenbrowse.com";

/// True iff `host` is the allowlisted connect host.
#[must_use]
pub fn is_allowed_connect_host(host: &str) -> bool {
    host == ALLOWED_CONNECT_HOST
}

/// A signed `POST /v1/forum/login` request, transport-agnostic so the byte
/// construction (host-tested) and the network execution (Android-only) stay
/// separable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedForumRequest {
    /// Absolute request URL (`https://<host>/v1/forum/login`).
    pub url: String,
    /// Header name/value pairs: `Content-Type` plus the four `X-Warren-*` auth
    /// headers, ready to attach verbatim.
    pub headers: Vec<(String, String)>,
    /// The canonical request body bytes (`{"sid":"<sid>"}`).
    pub body: Vec<u8>,
}

/// Build the signed forum-login request for `sid` against `host`.
///
/// Validates the host allowlist and (via [`sign_forum_login`]) the `sid` shape,
/// generates a fresh timestamp and a random 16-byte nonce, and returns the exact
/// bytes to POST. All failure paths collapse to [`ForumRequestError::Invalid`],
/// which carries no identity material.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` /
/// mnemonic is malformed, or the OS RNG / clock is unavailable.
pub fn build_signed_request(
    mnemonic: &str,
    sid: &str,
    host: &str,
) -> Result<SignedForumRequest, ForumRequestError> {
    if !is_allowed_connect_host(host) {
        return Err(ForumRequestError::Invalid);
    }
    let timestamp = unix_secs_now().ok_or(ForumRequestError::Invalid)?;
    let nonce_hex = nonce_hex_16().ok_or(ForumRequestError::Invalid)?;
    let ForumLoginSigned { headers, body } =
        sign_forum_login(mnemonic, sid, timestamp, &nonce_hex)?;
    let mut header_pairs: Vec<(String, String)> = headers.into_iter().collect();
    header_pairs.push(("Content-Type".to_owned(), "application/json".to_owned()));
    Ok(SignedForumRequest {
        url: format!("https://{host}/v1/forum/login"),
        headers: header_pairs,
        body: body.into_bytes(),
    })
}

/// Coarse outcome of a forum-login attempt, matching the desktop's three
/// results. Kotlin maps [`Self::SubscriptionRequired`] to a "subscription
/// required" message and everything else to a generic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumLoginOutcome {
    /// The provider accepted the signature (the browser completes the login).
    Approved,
    /// The wallet has never subscribed to Warren; forum access is refused (403).
    SubscriptionRequired,
    /// Any other failure (bad request, provider error, transport error).
    Failed,
}

/// Map an HTTP status to the outcome: 2xx approved, 403 subscription-required,
/// anything else failed. Byte-for-byte the contract the desktop
/// `approveForumLogin` uses.
#[must_use]
pub fn outcome_for_status(status: u16) -> ForumLoginOutcome {
    match status {
        200..=299 => ForumLoginOutcome::Approved,
        403 => ForumLoginOutcome::SubscriptionRequired,
        _ => ForumLoginOutcome::Failed,
    }
}

/// The JSON envelope returned to Kotlin for `outcome`. A fixed string (no serde)
/// so it is available on the host build too; never carries any request context.
#[must_use]
pub fn envelope(outcome: ForumLoginOutcome) -> &'static str {
    match outcome {
        ForumLoginOutcome::Approved => r#"{"ok":true}"#,
        ForumLoginOutcome::SubscriptionRequired => {
            r#"{"ok":false,"error":"subscription-required"}"#
        }
        ForumLoginOutcome::Failed => r#"{"ok":false,"error":"error"}"#,
    }
}

/// Failure building the signed request. Deliberately opaque (one variant): the
/// caller maps it to the generic `Failed` outcome, and no cause with identity
/// material is ever surfaced or logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumRequestError {
    /// The host was not allowlisted, or the sid / mnemonic / RNG / clock was
    /// unusable.
    Invalid,
}

impl From<WalletError> for ForumRequestError {
    fn from(_: WalletError) -> Self {
        // Collapse the wallet error (which may mention sid/mnemonic state) to an
        // opaque marker so no identity material leaks into a log or an envelope.
        ForumRequestError::Invalid
    }
}

/// 16 bytes of OS entropy, lowercase hex (32 chars), for the request nonce.
/// `None` only if the OS RNG is unavailable.
fn nonce_hex_16() -> Option<String> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).ok()?;
    Some(hex::encode(buf))
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

    // Official BIP39 all-zero-entropy 12-word vector (as used in wallet tests).
    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const SID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn only_the_connect_host_is_allowlisted() {
        assert!(is_allowed_connect_host("connect.warrenbrowse.com"));
        // A hostile or look-alike host must be refused before anything is signed.
        assert!(!is_allowed_connect_host("evil.example.com"));
        assert!(!is_allowed_connect_host(
            "connect.warrenbrowse.com.evil.com"
        ));
        assert!(!is_allowed_connect_host("api.warrenbrowse.com"));
        assert!(!is_allowed_connect_host(""));
    }

    #[test]
    fn build_signed_request_produces_the_forum_login_wire() {
        let req = build_signed_request(PHRASE, SID, "connect.warrenbrowse.com")
            .expect("a valid host + sid + mnemonic must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        // The body is the exact canonical challenge the signature covers.
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
        // The four auth headers plus Content-Type must all be attached.
        let names: Vec<&str> = req.headers.iter().map(|(n, _)| n.as_str()).collect();
        for expected in [
            "X-Warren-PubKey",
            "X-Warren-Sig",
            "X-Warren-Timestamp",
            "X-Warren-Nonce",
            "Content-Type",
        ] {
            assert!(names.contains(&expected), "missing header {expected}");
        }
        let content_type = req
            .headers
            .iter()
            .find(|(n, _)| n == "Content-Type")
            .map(|(_, v)| v.as_str());
        assert_eq!(content_type, Some("application/json"));
    }

    #[test]
    fn build_signed_request_refuses_a_non_allowlisted_host() {
        // A hostile deep-link host must never be signed against.
        assert_eq!(
            build_signed_request(PHRASE, SID, "evil.example.com"),
            Err(ForumRequestError::Invalid)
        );
    }

    #[test]
    fn build_signed_request_refuses_a_malformed_sid() {
        // Uppercase (wrong shape) sid is rejected by the underlying signer.
        assert_eq!(
            build_signed_request(
                PHRASE,
                "0123456789ABCDEF0123456789abcdef",
                "connect.warrenbrowse.com"
            ),
            Err(ForumRequestError::Invalid)
        );
    }

    #[test]
    fn nonce_is_sixteen_random_bytes_lowercase_hex() {
        let a = nonce_hex_16().expect("OS RNG available");
        let b = nonce_hex_16().expect("OS RNG available");
        assert_eq!(a.len(), 32, "16 bytes = 32 hex chars");
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        );
        assert_ne!(a, b, "two nonces must not collide");
    }

    #[test]
    fn status_maps_to_the_desktop_outcomes() {
        assert_eq!(outcome_for_status(200), ForumLoginOutcome::Approved);
        assert_eq!(outcome_for_status(204), ForumLoginOutcome::Approved);
        assert_eq!(
            outcome_for_status(403),
            ForumLoginOutcome::SubscriptionRequired
        );
        assert_eq!(outcome_for_status(401), ForumLoginOutcome::Failed);
        assert_eq!(outcome_for_status(500), ForumLoginOutcome::Failed);
    }

    #[test]
    fn envelope_json_matches_the_kotlin_contract() {
        assert_eq!(envelope(ForumLoginOutcome::Approved), r#"{"ok":true}"#);
        assert_eq!(
            envelope(ForumLoginOutcome::SubscriptionRequired),
            r#"{"ok":false,"error":"subscription-required"}"#
        );
        assert_eq!(
            envelope(ForumLoginOutcome::Failed),
            r#"{"ok":false,"error":"error"}"#
        );
    }
}
