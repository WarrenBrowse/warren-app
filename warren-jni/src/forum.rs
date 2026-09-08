//! Community-forum wallet login (`POST /v1/forum/login`, doc 55) and in-app
//! bug report (`POST /v1/forum/report`), Android side.
//!
//! The wire bytes, deep-link validation, cancel URL, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by iOS). This module only adds the Android-specific signing entry: it
//! derives the Ed25519 key from the wallet mnemonic and delegates. The network
//! POST that consumes the request is Android-gated in `android_jni`.

pub use warren_forum::{
    AttachTopic, CodeKind, CodePlacement, FORUM_ATTACH_PATH, FailReason, ForumAttachOutcome,
    ForumIdentity, ForumLoginOutcome, ForumNotificationsOutcome, ForumRequestError,
    MAX_FORUM_TOPIC_ID, MAX_LOG_GZ_BYTES, PRE_TOPIC_ID, ReportOutcome, SessionPreflight,
    SignedForumRequest, attach_body, attach_envelope, attach_outcome_for_response,
    build_attach_cancel_url, build_attach_meta_url, build_attach_status_url, build_cancel_url,
    build_status_url, classify_code_probe, classify_status_preflight, clock_offset_secs,
    code_placement_envelope, code_probe_envelope, connect_host, envelope, is_allowed_connect_host,
    is_valid_sid, normalize_sign_in_code, notifications_envelope,
    notifications_outcome_for_response, outcome_for_response, parse_attach_meta_topic,
    parse_topic_id, place_code, refuse_before_transport, report_envelope,
    report_outcome_for_response, seen_envelope, seen_outcome_for_response, timestamp_with_offset,
    upload_deadline,
};

/// Build the signed forum-login request for `sid` against `host`, deriving the
/// signing key from the wallet `mnemonic` (the Android secret-store shape; iOS
/// passes a seed-derived key instead), stamped with the device clock.
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

/// [`build_signed_request`] with an explicit timestamp: the device clock
/// corrected by the offset measured against the connect host.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed, the host is not
/// allowlisted, or the `sid` / RNG is unusable.
pub fn build_signed_request_at(
    mnemonic: &str,
    sid: &str,
    host: &str,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_request_at(&key, sid, host, timestamp)
}

/// Build the signed in-app report request from the wallet `mnemonic`, the
/// report fields (one JSON object, see
/// [`warren_forum::build_signed_report_request`]) and the optional gzipped
/// redacted problem report.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed, the report is
/// not a JSON object, or the RNG is unusable; [`ForumRequestError::LogTooLarge`]
/// if `log_gz` is over `warren_forum::MAX_LOG_GZ_BYTES`.
pub fn build_signed_report_request(
    mnemonic: &str,
    report_json: &str,
    log_gz: Option<&[u8]>,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_report_request(&key, report_json, log_gz, timestamp)
}

/// Build the signed panel read (`POST /v1/forum/notifications`) from the
/// wallet `mnemonic`, stamped with the corrected `timestamp`.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed or the RNG is
/// unusable.
pub fn build_signed_notifications_request(
    mnemonic: &str,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_notifications_request(&key, timestamp)
}

/// Build the signed mark-seen (`POST /v1/forum/notifications/seen`) from the
/// wallet `mnemonic`, stamped with the corrected `timestamp`.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed or the RNG is
/// unusable.
pub fn build_signed_notifications_seen_request(
    mnemonic: &str,
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_notifications_seen_request(&key, timestamp)
}

/// Build the signed attach-logs upload for `sid` and `topic_id` against
/// `host` from the wallet `mnemonic`, stamped with the corrected `timestamp`.
/// The Android shape of [`warren_forum::build_signed_attach_request`]: the
/// key is derived here, everything that decides the wire is the shared crate's.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the mnemonic is malformed, the host is
/// not allowlisted, the `sid` is malformed, the log is empty or the RNG is
/// unusable; [`ForumRequestError::LogTooLarge`] over [`MAX_LOG_GZ_BYTES`].
pub fn build_signed_attach_request(
    mnemonic: &str,
    sid: &str,
    host: &str,
    topic_id: u64,
    log_gz: &[u8],
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    warren_forum::build_signed_attach_request(&key, sid, host, topic_id, log_gz, timestamp)
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

    #[test]
    fn a_corrected_timestamp_is_the_one_stamped() {
        let req = build_signed_request_at(PHRASE, SID, "connect.warrenbrowse.com", 1_800_000_000)
            .expect("builds");
        let stamp = req
            .headers
            .iter()
            .find(|(n, _)| n == "X-Warren-Timestamp")
            .map(|(_, v)| v.as_str());
        assert_eq!(stamp, Some("1800000000"));
    }

    #[test]
    fn the_panel_read_and_the_mark_seen_from_a_mnemonic_target_their_own_routes() {
        let read = build_signed_notifications_request(PHRASE, 1_800_000_000).expect("builds");
        assert_eq!(
            read.url,
            "https://connect.warrenbrowse.com/v1/forum/notifications"
        );
        assert_eq!(read.body, b"{}");
        let seen = build_signed_notifications_seen_request(PHRASE, 1_800_000_000).expect("builds");
        assert_eq!(
            seen.url,
            "https://connect.warrenbrowse.com/v1/forum/notifications/seen"
        );
        assert_eq!(seen.body, b"{}");
        assert_eq!(
            build_signed_notifications_request("not a mnemonic", 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_notifications_seen_request("not a mnemonic", 1),
            Err(ForumRequestError::Invalid)
        );
    }

    #[test]
    fn build_signed_report_request_from_a_mnemonic_targets_the_report_route() {
        let req = build_signed_report_request(
            PHRASE,
            r#"{"platform":"android","area":"other","frequency":"once","what_happened":"Long enough to pass the caps."}"#,
            Some(b"gz"),
            1_800_000_000,
        )
        .expect("a valid mnemonic + report must build a request");
        assert_eq!(req.url, "https://connect.warrenbrowse.com/v1/forum/report");
        assert!(req.body.windows(12).any(|w| w == b"\"log_gz_b64\""));
        assert_eq!(
            build_signed_report_request("not a mnemonic", "{}", None, 1),
            Err(ForumRequestError::Invalid)
        );
    }
}

#[cfg(test)]
mod attach_tests {
    use super::*;

    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const SID: &str = "0123456789abcdef0123456789abcdef";
    const HOST: &str = "connect.warrenbrowse.com";

    #[test]
    fn build_signed_attach_request_from_a_mnemonic_targets_the_attach_route_at_the_given_stamp() {
        // Guards the Android mnemonic -> signing-key derivation feeding the
        // shared builder (the shared crate covers the wire parity itself).
        let req = build_signed_attach_request(PHRASE, SID, HOST, 42, b"gz", 1_800_000_000)
            .expect("a valid mnemonic + link must build a request");
        assert_eq!(
            req.url,
            "https://connect.warrenbrowse.com/v1/forum/attach-logs"
        );
        assert_eq!(
            req.body,
            format!(r#"{{"log_gz_b64":"Z3o=","sid":"{SID}","topic_id":42}}"#).into_bytes()
        );
        let header = |name: &str| {
            req.headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(header("X-Warren-Timestamp"), Some("1800000000"));
        assert!(header("X-Warren-Sig").is_some());
        assert!(header("X-Warren-PubKey").is_some());
        assert!(header("X-Warren-Nonce").is_some());
        assert_eq!(header("Content-Type"), Some("application/json"));
    }

    #[test]
    fn build_signed_attach_request_refuses_what_the_login_builder_refuses() {
        assert_eq!(
            build_signed_attach_request(PHRASE, SID, "evil.example.com", 42, b"gz", 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_attach_request(PHRASE, "NOTHEX", HOST, 42, b"gz", 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_attach_request("not a mnemonic", SID, HOST, 42, b"gz", 1),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            build_signed_attach_request(PHRASE, SID, HOST, 42, &vec![0u8; MAX_LOG_GZ_BYTES + 1], 1),
            Err(ForumRequestError::LogTooLarge)
        );
    }
}
