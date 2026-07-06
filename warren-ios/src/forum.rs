//! Community-forum wallet login (`POST /v1/forum/login`, doc 55), iOS side.
//!
//! The wire bytes, deep-link validation, cancel URL, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by Android). This module only adds the iOS-specific signing entry: it
//! takes the seed-derived `WarrenIdentity` and delegates. The network POST that
//! consumes the request is iOS-gated in `warren_forum_ffi`.

pub use warren_forum::{
    ForumLoginOutcome, ForumRequestError, SignedForumRequest, build_cancel_url, envelope,
    is_allowed_connect_host, is_valid_sid, outcome_for_status,
};
use warren_identity::WarrenIdentity;

/// Build the signed forum-login request for `sid` against `host`, signing with
/// the seed-derived `identity`'s key (the iOS secret-store shape; Android passes
/// a mnemonic-derived key instead). Delegates to
/// [`warren_forum::build_signed_request`].
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, or the RNG / clock is unusable.
pub fn build_signed_request(
    identity: &WarrenIdentity,
    sid: &str,
    host: &str,
) -> Result<SignedForumRequest, ForumRequestError> {
    warren_forum::build_signed_request(&identity.signing_key(), sid, host)
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
        let req = build_signed_request(&identity, SID, "connect.warrenbrowse.com")
            .expect("a valid identity + host + sid must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
        let names: Vec<&str> = req.headers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"X-Warren-Sig"));
        assert!(names.contains(&"Content-Type"));
    }

    #[test]
    fn build_signed_request_rejects_a_non_allowlisted_host() {
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        assert_eq!(
            build_signed_request(&identity, SID, "evil.example.com"),
            Err(ForumRequestError::Invalid)
        );
    }
}
