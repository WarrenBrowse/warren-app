//! Community-forum wallet login (`POST /v1/forum/login`, doc 55), iOS side.
//!
//! The wire bytes, deep-link validation, cancel URL, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by Android). This module only adds the iOS-specific signing entry: it
//! takes the seed-derived `WarrenIdentity` and delegates. The network POST that
//! consumes the request is iOS-gated in `warren_forum_ffi`.

pub use warren_forum::{ForumRequestError, SignedForumRequest};
// Consumed only by the iOS-gated network module (`warren_forum_ffi`), so the
// host test build would flag these re-exports as unused. The host/sid
// validators are not re-exported: the shared builders already validate
// internally and no iOS caller consumes them directly.
#[cfg(target_os = "ios")]
pub use warren_forum::{
    FailReason, ForumLoginOutcome, SessionPreflight, build_cancel_url, build_status_url,
    classify_status_preflight, envelope, outcome_for_response, timestamp_with_offset,
};
use warren_identity::WarrenIdentity;

/// Build the signed forum-login request for `sid` against `host`, signing with
/// the seed-derived `identity`'s key (the iOS secret-store shape; Android passes
/// a mnemonic-derived key instead) and stamped with `timestamp`: the value the
/// login preflight corrected against the connect host's `Date` header
/// ([`warren_forum::timestamp_with_offset`]), so a device whose clock sits
/// outside the broker's 60 s window still signs a request it accepts.
/// Delegates to [`warren_forum::build_signed_request_at`].
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, or the RNG is unusable.
pub fn build_signed_request_at(
    identity: &WarrenIdentity,
    sid: &str,
    host: &str,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    warren_forum::build_signed_request_at(&identity.signing_key(), sid, host, timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn build_signed_request_from_a_seed_identity_produces_the_forum_login_wire() {
        // Guards the iOS seed -> identity -> signing-key path feeding the shared
        // builder (the shared crate covers the wire parity itself).
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        let req =
            build_signed_request_at(&identity, SID, "connect.warrenbrowse.com", 1_800_000_000)
                .expect("a valid identity + host + sid must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
        let names: Vec<&str> = req.headers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"X-Warren-Sig"));
        assert!(names.contains(&"Content-Type"));
    }

    #[test]
    fn build_signed_request_at_stamps_the_corrected_time_not_the_device_clock() {
        // The preflight hands the FFI a server-corrected timestamp; the header
        // the broker checks against its 60 s window must carry that value.
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        let req =
            build_signed_request_at(&identity, SID, "connect.warrenbrowse.com", 1_800_000_000)
                .expect("a valid identity + host + sid + timestamp must build a request");
        let stamped = req
            .headers
            .iter()
            .find(|(n, _)| n == "X-Warren-Timestamp")
            .map(|(_, v)| v.as_str());
        assert_eq!(stamped, Some("1800000000"));
    }

    #[test]
    fn build_signed_request_rejects_a_non_allowlisted_host() {
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        assert_eq!(
            build_signed_request_at(&identity, SID, "evil.example.com", 1_800_000_000),
            Err(ForumRequestError::Invalid)
        );
    }
}
