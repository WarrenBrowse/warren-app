//! Community-forum wallet login (`POST /v1/forum/login`, warren-core doc 55),
//! the shared, host-tested logic behind both mobile FFI crates.
//!
//! A `warren://forum-login?sid=..&host=..` deep link asks the app to prove wallet
//! ownership to the connect provider. On both Android (`warren-jni`) and iOS
//! (`warren-ios`) the request is signed AND sent inside Rust (like every other
//! signed call), so only the opaque `sid` and the connect `host` cross the FFI
//! boundary and the wallet signature never surfaces to Kotlin / Swift. This
//! crate owns everything that decides the wire bytes, validates the deep-link
//! inputs, and maps the outcome, single-sourced so the two platforms cannot
//! drift; each FFI crate supplies the tokio runtime + reqwest transport that
//! executes the POST, and the `SigningKey` derived from its own secret store.

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

/// True iff `sid` is exactly 32 lowercase hex chars (the forum SSO session id
/// shape; mirrors the desktop `parseForumLoginUrl` guard).
#[must_use]
pub fn is_valid_sid(sid: &str) -> bool {
    sid.len() == 32
        && sid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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

/// A signed `POST /v1/forum/login` request, transport-agnostic so the byte
/// construction (host-tested) and the network execution (per-platform, FFI-only)
/// stay separable.
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

/// Failure building the signed request. Deliberately opaque (one variant): the
/// caller maps it to the generic `Failed` outcome, and no cause with identity
/// material is ever surfaced or logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumRequestError {
    /// The host was not allowlisted, the sid was malformed, or the RNG / clock
    /// was unusable.
    Invalid,
}

/// Build the signed forum-login request for `sid` against `host`, signing with
/// `signing_key` (each platform derives it from its own secret store: Android
/// from the mnemonic, iOS from the wallet seed).
///
/// Validates the host allowlist and the `sid` shape, generates a fresh timestamp
/// and a random 16-byte nonce, and returns the exact bytes to POST.
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
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return Err(ForumRequestError::Invalid);
    }
    let timestamp = unix_secs_now().ok_or(ForumRequestError::Invalid)?;
    let nonce = nonce_16().ok_or(ForumRequestError::Invalid)?;
    let body = format!("{{\"sid\":\"{sid}\"}}");
    let sig = sign_request(
        signing_key,
        "POST",
        "/v1/forum/login",
        body.as_bytes(),
        timestamp,
        nonce,
    );
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
/// results. Kotlin / Swift map [`Self::SubscriptionRequired`] to a "subscription
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

/// The JSON envelope returned across the FFI for `outcome`. A fixed string (no
/// serde) so both platforms decode the same three shapes; never carries any
/// request context.
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
    }

    #[test]
    fn sid_shape_is_enforced() {
        assert!(is_valid_sid(SID));
        assert!(!is_valid_sid("0123456789ABCDEF0123456789abcdef")); // uppercase
        assert!(!is_valid_sid("0123456789abcdef")); // too short
        assert!(!is_valid_sid("0123456789abcdef0123456789abcdeg")); // non-hex
    }

    #[test]
    fn cancel_url_is_built_only_for_a_valid_host_and_sid() {
        assert_eq!(
            build_cancel_url(SID, "connect.warrenbrowse.com").as_deref(),
            Some(
                "https://connect.warrenbrowse.com/v1/session/0123456789abcdef0123456789abcdef/cancel"
            )
        );
        assert_eq!(build_cancel_url(SID, "evil.example.com"), None);
        assert_eq!(build_cancel_url("NOTHEX", "connect.warrenbrowse.com"), None);
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

    #[test]
    fn build_signed_request_produces_a_wire_parity_signature() {
        use sha2::{Digest as _, Sha256};
        use warren_identity::ed25519_dalek::{Signature, Verifier};

        let key = signing_key();
        let req = build_signed_request(&key, SID, "connect.warrenbrowse.com")
            .expect("a valid host + sid must build a request");

        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());

        let header = |name: &str| {
            req.headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("missing header {name}"))
        };
        assert_eq!(header("Content-Type"), "application/json");

        // The signature verifies against the canonical POST message the server
        // reconstructs (proves wire parity, not just header presence). The nonce
        // + timestamp are read back from the headers since they are random.
        let timestamp: u64 = header("X-Warren-Timestamp").parse().unwrap();
        let nonce_hex = header("X-Warren-Nonce");
        let body_hash_hex = hex::encode(Sha256::digest(&req.body));
        let expected = warren_identity::canonical_message(
            "POST",
            "/v1/forum/login",
            timestamp,
            &nonce_hex,
            &body_hash_hex,
        );
        let sig_bytes: [u8; 64] = hex::decode(header("X-Warren-Sig"))
            .unwrap()
            .try_into()
            .unwrap();
        key.verifying_key()
            .verify(expected.as_bytes(), &Signature::from_bytes(&sig_bytes))
            .expect("forum-login signature must verify against the canonical POST message");
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
    fn envelope_json_matches_the_ffi_contract() {
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
