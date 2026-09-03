//! Community-forum wallet login (`POST /v1/forum/login`, doc 55) and in-app
//! bug report (`POST /v1/forum/report`), Android side.
//!
//! The wire bytes, deep-link validation, cancel URL, and outcome mapping are
//! single-sourced in the shared [`warren_forum`] crate (host-tested there and
//! reused by iOS). This module only adds the Android-specific signing entry: it
//! derives the Ed25519 key from the wallet mnemonic and delegates. The network
//! POST that consumes the request is Android-gated in `android_jni`.

use std::time::Duration;

pub use warren_forum::{
    FailReason, ForumIdentity, ForumLoginOutcome, ForumNotificationsOutcome, ForumRequestError,
    ReportOutcome, SignedForumRequest, build_cancel_url, build_status_url, clock_offset_secs,
    connect_host, envelope, is_allowed_connect_host, is_valid_sid, normalize_sign_in_code,
    notifications_envelope, notifications_outcome_for_response, outcome_for_response,
    report_envelope, report_outcome_for_response, seen_envelope, seen_outcome_for_response,
    timestamp_with_offset,
};

/// The total deadline of a report upload, from the body it sends: 20 s for
/// the exchange itself plus 10 s per MiB of body. The forum transport's
/// default 15 s is the token mint's, sized for a few hundred bytes, and it
/// covered the upload too, so a report with a few MiB of logs on a slow
/// mobile uplink (the network a report is filed from) died in it as a generic
/// transport failure after the data was spent. 10 s per MiB is a floor of
/// about 0.8 Mbit/s; a no-log report keeps roughly the mint's own bound, a
/// report at the log cap gets three minutes.
#[must_use]
pub fn upload_deadline(body_len: usize) -> Duration {
    const MIB: usize = 1024 * 1024;
    let mib = u64::try_from(body_len.div_ceil(MIB)).unwrap_or(u64::MAX);
    Duration::from_secs(20u64.saturating_add(10u64.saturating_mul(mib)))
}

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
    fn the_upload_deadline_grows_with_the_body() {
        // A bodyless request keeps close to the mint's 15 s; every MiB, or
        // part of one, buys 10 s; the 16 MB body of a report at the log cap
        // gets three minutes rather than the 15 s that could never carry it.
        assert_eq!(upload_deadline(0), Duration::from_secs(20));
        assert_eq!(upload_deadline(1), Duration::from_secs(30));
        assert_eq!(upload_deadline(1024 * 1024), Duration::from_secs(30));
        assert_eq!(upload_deadline(1024 * 1024 + 1), Duration::from_secs(40));
        assert_eq!(upload_deadline(16 * 1024 * 1024), Duration::from_secs(180));
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
