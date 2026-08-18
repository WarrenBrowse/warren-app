//! Community-forum wallet login (`POST /v1/forum/login`, doc 55), Android side.
//!
//! The wire bytes, deep-link validation, cancel URL, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by iOS). This module only adds the Android-specific signing entry: it
//! derives the Ed25519 key from the wallet mnemonic and delegates. The network
//! POST that consumes the request is Android-gated in `android_jni`.

pub use warren_forum::{
    ForumLoginOutcome, ForumRequestError, SignedForumRequest, build_cancel_url, envelope,
    is_allowed_connect_host, is_valid_sid, outcome_for_response,
};

/// Build the signed forum-login request for `sid` against `host`, deriving the
/// signing key from the wallet `mnemonic` (the Android secret-store shape; iOS
/// passes a seed-derived key instead). Delegates the wire construction to
/// [`warren_forum::build_signed_request`].
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed, the host is not
/// allowlisted, or the `sid` / RNG / clock is unusable.
pub fn build_signed_request(
    mnemonic: &str,
    sid: &str,
    host: &str,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_request(&key, sid, host)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Official BIP39 all-zero-entropy 12-word vector.
    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const SID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn build_signed_request_from_a_mnemonic_produces_the_forum_login_wire() {
        // Guards the Android mnemonic -> signing-key derivation feeding the
        // shared builder (the shared crate covers the wire parity itself).
        let req = build_signed_request(PHRASE, SID, "connect.warrenbrowse.com")
            .expect("a valid mnemonic + host + sid must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/login");
        assert_eq!(req.body, format!("{{\"sid\":\"{SID}\"}}").into_bytes());
        let names: Vec<&str> = req.headers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"X-Warren-Sig"));
        assert!(names.contains(&"Content-Type"));
    }

    #[test]
    fn build_signed_request_rejects_a_non_allowlisted_host() {
        assert_eq!(
            build_signed_request(PHRASE, SID, "evil.example.com"),
            Err(ForumRequestError::Invalid)
        );
    }
}
