//! Community-forum wallet login (`POST /v1/forum/login`, warren-core doc 55),
//! the host-tested half. iOS counterpart of Android's `warren-jni::forum`.
//!
//! A `warren://forum-login?sid=..&host=..` deep link asks the app to prove wallet
//! ownership to the connect provider. As on Android, the request is signed AND
//! sent inside Rust (like every other signed call), so only the opaque `sid` and
//! the connect `host` cross the FFI boundary and the wallet signature never
//! surfaces to Swift. Everything that decides the wire bytes, validates the
//! deep-link inputs, and maps the outcome lives here so it is unit-testable
//! off-device; the iOS-only `warren_forum_ffi` layer supplies the tokio runtime
//! and the reqwest transport that actually executes the POST.

use warren_identity::WarrenIdentity;

/// The single connect host accepted from a forum-login deep link. A hard
/// allowlist: a hostile link must not be able to point the wallet-signed request
/// at an attacker-controlled server (mirrors the desktop `ALLOWED_CONNECT_HOSTS`
/// and Android's `ALLOWED_CONNECT_HOST`).
const ALLOWED_CONNECT_HOST: &str = "connect.warrenbrowse.com";

/// True iff `host` is the allowlisted connect host.
#[must_use]
pub fn is_allowed_connect_host(host: &str) -> bool {
    host == ALLOWED_CONNECT_HOST
}

/// True iff `sid` is exactly 32 lowercase hex chars (the forum SSO session id
/// shape; mirrors the desktop `parseForumLoginUrl` guard and Android's
/// `is_valid_sid`).
#[must_use]
pub fn is_valid_sid(sid: &str) -> bool {
    sid.len() == 32
        && sid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A signed `POST /v1/forum/login` request, transport-agnostic so the byte
/// construction (host-tested) and the network execution (iOS-only) stay
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

/// Build the signed forum-login request for `sid` against `host`, signing with
/// `identity` (derived from the wallet seed at the FFI boundary).
///
/// Validates the host allowlist and the `sid` shape, generates a fresh timestamp
/// and a random 16-byte nonce, and returns the exact bytes to POST. All failure
/// paths collapse to [`ForumRequestError::Invalid`], which carries no identity
/// material.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, or the OS RNG / clock is unavailable.
pub fn build_signed_request(
    identity: &WarrenIdentity,
    sid: &str,
    host: &str,
) -> Result<SignedForumRequest, ForumRequestError> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return Err(ForumRequestError::Invalid);
    }
    let timestamp = unix_secs_now().ok_or(ForumRequestError::Invalid)?;
    let nonce = nonce_16().ok_or(ForumRequestError::Invalid)?;
    let body = format!("{{\"sid\":\"{sid}\"}}");
    let sig = identity.sign_request("POST", "/v1/forum/login", body.as_bytes(), timestamp, nonce);
    let mut headers: Vec<(String, String)> = sig
        .headers()
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
    Ok(SignedForumRequest {
        url: format!("https://{host}/v1/forum/login"),
        headers,
        body: body.into_bytes(),
    })
}

/// Coarse outcome of a forum-login attempt, matching the desktop's three
/// results. Swift maps [`Self::SubscriptionRequired`] to a "subscription
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
/// `approveForumLogin` (and Android `outcome_for_status`) use.
#[must_use]
pub fn outcome_for_status(status: u16) -> ForumLoginOutcome {
    match status {
        200..=299 => ForumLoginOutcome::Approved,
        403 => ForumLoginOutcome::SubscriptionRequired,
        _ => ForumLoginOutcome::Failed,
    }
}

/// The JSON envelope returned to Swift for `outcome`. A fixed string so it never
/// carries any request context; matches the Android `WarrenJni.forumLogin`
/// contract byte-for-byte.
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
    /// The host was not allowlisted, the sid was malformed, or the RNG / clock
    /// was unusable.
    Invalid,
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

    fn identity() -> WarrenIdentity {
        // A fixed non-zero seed: the request must build and sign deterministically.
        WarrenIdentity::from_seed(&[0x11u8; 32])
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
    }

    #[test]
    fn sid_shape_is_enforced() {
        assert!(is_valid_sid(SID));
        assert!(!is_valid_sid("0123456789ABCDEF0123456789abcdef")); // uppercase
        assert!(!is_valid_sid("0123456789abcdef")); // too short
        assert!(!is_valid_sid("0123456789abcdef0123456789abcdeg")); // non-hex
    }

    #[test]
    fn build_signed_request_produces_the_forum_login_wire() {
        let req = build_signed_request(&identity(), SID, "connect.warrenbrowse.com")
            .expect("a valid host + sid must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
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
        assert_eq!(
            build_signed_request(&identity(), SID, "evil.example.com"),
            Err(ForumRequestError::Invalid)
        );
    }

    #[test]
    fn build_signed_request_refuses_a_malformed_sid() {
        assert_eq!(
            build_signed_request(
                &identity(),
                "0123456789ABCDEF0123456789abcdef",
                "connect.warrenbrowse.com"
            ),
            Err(ForumRequestError::Invalid)
        );
    }

    #[test]
    fn nonce_is_sixteen_random_bytes() {
        let a = nonce_16().expect("OS RNG available");
        let b = nonce_16().expect("OS RNG available");
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
    fn envelope_json_matches_the_swift_contract() {
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
