//! Community-forum wallet login (`POST /v1/forum/login`, doc 55) and the
//! forum page's attach-logs upload (`POST /v1/forum/attach-logs`), iOS side.
//!
//! The wire bytes, deep-link validation, session URLs, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by Android). This module only adds the iOS-specific signing entries:
//! they take the seed-derived `WarrenIdentity` and delegate. The network
//! requests that consume them are iOS-gated in `warren_forum_ffi`.

pub use warren_forum::{ForumRequestError, SignedForumRequest};
// Consumed only by the iOS-gated network module (`warren_forum_ffi`), so the
// host test build would flag these re-exports as unused. The host/sid
// validators are not re-exported: the shared builders already validate
// internally and no iOS caller consumes them directly.
#[cfg(target_os = "ios")]
pub use warren_forum::{
    CodeKind, FailReason, ForumAttachOutcome, ForumLoginOutcome, MAX_LOG_GZ_BYTES,
    SessionPreflight, attach_envelope, attach_outcome_for_response, build_attach_cancel_url,
    build_attach_status_url, build_cancel_url, build_status_url, classify_code_probe,
    classify_status_preflight, code_probe_envelope, envelope, outcome_for_response,
    timestamp_with_offset, upload_deadline,
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

/// Build the signed attach-logs upload for `sid` and `topic_id` against
/// `host`, signing with the seed-derived `identity`'s key and stamped with
/// `timestamp`, the value the attach preflight corrected against the connect
/// host's `Date` header. Delegates to
/// [`warren_forum::build_signed_attach_request`].
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid` is
/// malformed, the log is empty or the RNG is unusable;
/// [`ForumRequestError::LogTooLarge`] over [`warren_forum::MAX_LOG_GZ_BYTES`].
pub fn build_signed_attach_request(
    identity: &WarrenIdentity,
    sid: &str,
    host: &str,
    topic_id: u64,
    log_gz: &[u8],
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    warren_forum::build_signed_attach_request(
        &identity.signing_key(),
        sid,
        host,
        topic_id,
        log_gz,
        timestamp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn build_signed_attach_request_from_a_seed_identity_produces_the_attach_wire() {
        // Guards the iOS seed -> identity -> signing-key path feeding the
        // shared attach builder (the shared crate covers the wire parity).
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        let req = build_signed_attach_request(
            &identity,
            SID,
            "connect.warrenbrowse.com",
            42,
            b"gz",
            1_800_000_000,
        )
        .expect("a valid identity + link must build a request");
        assert_eq!(
            req.url,
            "https://connect.warrenbrowse.com/v1/forum/attach-logs"
        );
        assert_eq!(
            req.body,
            warren_forum::attach_body(SID, 42, b"gz").expect("body")
        );
        let stamped = req
            .headers
            .iter()
            .find(|(n, _)| n == "X-Warren-Timestamp")
            .map(|(_, v)| v.as_str());
        assert_eq!(stamped, Some("1800000000"));
    }

    #[test]
    fn build_signed_attach_request_refuses_a_non_allowlisted_host_and_an_oversized_log() {
        let identity = WarrenIdentity::from_seed(&[0x11u8; 32]);
        assert_eq!(
            build_signed_attach_request(&identity, SID, "evil.example.com", 42, b"gz", 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_attach_request(
                &identity,
                SID,
                "connect.warrenbrowse.com",
                42,
                &vec![0u8; warren_forum::MAX_LOG_GZ_BYTES + 1],
                1
            ),
            Err(ForumRequestError::LogTooLarge)
        );
    }

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
