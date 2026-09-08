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
    MAX_LOG_GZ_BYTES, ReportOutcome, SessionPreflight, SignedForumRequest, build_cancel_url,
    build_status_url, classify_status_preflight, clock_offset_secs, connect_host, envelope,
    is_allowed_connect_host, is_valid_sid, normalize_sign_in_code, notifications_envelope,
    notifications_outcome_for_response, outcome_for_response, report_envelope,
    report_outcome_for_response, seen_envelope, seen_outcome_for_response, timestamp_with_offset,
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

/// The path the attach-logs upload (`POST /v1/forum/attach-logs`, the
/// forum's "attach your logs" page) is signed for and posted to.
pub const FORUM_ATTACH_PATH: &str = "/v1/forum/attach-logs";

/// The canonical body of the attach-logs upload: the session id, the topic
/// (0 for a pre-topic session, where the report is still being composed and
/// the forum binds the logs after creation) and the gzipped redacted report
/// as `log_gz_b64`. Serialised by the shared report-body builder, so the
/// size cap and the base64 alphabet are the ones every other upload uses.
/// The provider reads the object with serde; its own tests send this key
/// order.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] for a malformed `sid` or an empty log
/// (the provider refuses both, so nothing is signed for them);
/// [`ForumRequestError::LogTooLarge`] over [`MAX_LOG_GZ_BYTES`].
pub fn attach_body(sid: &str, topic_id: u64, log_gz: &[u8]) -> Result<Vec<u8>, ForumRequestError> {
    if !is_valid_sid(sid) || log_gz.is_empty() {
        return Err(ForumRequestError::Invalid);
    }
    warren_forum::report_body(
        &format!(r#"{{"sid":"{sid}","topic_id":{topic_id}}}"#),
        Some(log_gz),
    )
}

/// Build the signed attach-logs upload for `sid` and `topic_id` against
/// `host` from the wallet `mnemonic`, stamped with the corrected `timestamp`.
/// The mirror of the desktop daemon's `sign_forum_attach_logs`: the body is
/// serialised once here, so the signed bytes are the sent bytes.
///
/// # Errors
///
/// [`ForumRequestError::Invalid`] if the host is not allowlisted, the `sid`
/// is malformed, the log is empty, the mnemonic is malformed or the RNG is
/// unusable; [`ForumRequestError::LogTooLarge`] over [`MAX_LOG_GZ_BYTES`].
pub fn build_signed_attach_request(
    mnemonic: &str,
    sid: &str,
    host: &str,
    topic_id: u64,
    log_gz: &[u8],
    timestamp: u64,
) -> Result<SignedForumRequest, ForumRequestError> {
    // The allowlist gate comes first, as in the shared crate: a hostile link
    // must never reach a signature, whatever else it carries.
    if !is_allowed_connect_host(host) {
        return Err(ForumRequestError::Invalid);
    }
    let body = attach_body(sid, topic_id, log_gz)?;
    let key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|_| ForumRequestError::Invalid)?;
    let nonce = nonce_16().ok_or(ForumRequestError::Invalid)?;
    let sig = warren_identity::signing::sign_request(
        &key,
        "POST",
        FORUM_ATTACH_PATH,
        &body,
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
        url: format!("https://{host}{FORUM_ATTACH_PATH}"),
        headers,
        body,
    })
}

/// Sixteen bytes of OS entropy for the request nonce, through the `rand`
/// the mnemonic generator already links.
fn nonce_16() -> Option<[u8; 16]> {
    use bip39::rand::RngCore as _;
    let mut buf = [0u8; 16];
    bip39::rand::rngs::OsRng.try_fill_bytes(&mut buf).ok()?;
    Some(buf)
}

/// The unsigned status URL `GET /v1/attach/<sid>/status` the forum page
/// polls. Read once before signing, for the same two reasons as the login's:
/// the answer's `Date` is a trusted clock, and a session already gone is
/// told apart from a refused signature before one is spent.
#[must_use]
pub fn build_attach_status_url(sid: &str, host: &str) -> Option<String> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return None;
    }
    Some(format!("https://{host}/v1/attach/{sid}/status"))
}

/// The best-effort cancel URL `POST /v1/attach/<sid>/cancel`, so the waiting
/// forum page shows "cancelled" instead of polling to its timeout. Unsigned.
#[must_use]
pub fn build_attach_cancel_url(sid: &str, host: &str) -> Option<String> {
    if !is_allowed_connect_host(host) || !is_valid_sid(sid) {
        return None;
    }
    Some(format!("https://{host}/v1/attach/{sid}/cancel"))
}

/// What the provider made of an attach-logs upload: the desktop
/// `ForumAttachResult` table, plus the clock-skew refusal the mobile login
/// already tells apart (its own message names the fix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumAttachOutcome {
    /// Attached to the topic, or parked for a pre-topic session.
    Attached,
    /// The wallet is not the author of the topic (403).
    NotAuthor,
    /// The session is gone: expired, cancelled, consumed, or bound to another
    /// topic than the one sent (404).
    Expired,
    /// The provider refused the payload size (413).
    TooLarge,
    /// The signature was refused for a device clock outside the window.
    ClockSkew,
    /// The provider failed on its own side (5xx); nothing to fix here.
    ServerError,
    /// Any other failure, with its class.
    Failed(FailReason),
}

/// Connect's machine-readable 401 body for a clock outside the window, the
/// same frozen token the login outcome reads.
const CLOCK_SKEW_BODY_TOKEN: &[u8] = br#""error":"clock_skew""#;

fn body_carries(body: &[u8], token: &[u8]) -> bool {
    body.windows(token.len()).any(|w| w == token)
}

/// Map the provider's answer to the outcome.
#[must_use]
pub fn attach_outcome_for_response(status: u16, body: &[u8]) -> ForumAttachOutcome {
    match status {
        200..=299 => ForumAttachOutcome::Attached,
        403 => ForumAttachOutcome::NotAuthor,
        404 => ForumAttachOutcome::Expired,
        413 => ForumAttachOutcome::TooLarge,
        401 if body_carries(body, CLOCK_SKEW_BODY_TOKEN) => ForumAttachOutcome::ClockSkew,
        500..=599 => ForumAttachOutcome::ServerError,
        other => ForumAttachOutcome::Failed(FailReason::Http(other)),
    }
}

/// The JSON envelope returned across the JNI for an attach outcome, in the
/// shape of [`envelope`] so the Kotlin decoder is the login's. Never carries
/// any request context.
#[must_use]
pub fn attach_envelope(outcome: &ForumAttachOutcome) -> String {
    match outcome {
        ForumAttachOutcome::Attached => r#"{"ok":true}"#.to_owned(),
        ForumAttachOutcome::NotAuthor => r#"{"ok":false,"error":"not-author"}"#.to_owned(),
        ForumAttachOutcome::Expired => r#"{"ok":false,"error":"expired"}"#.to_owned(),
        ForumAttachOutcome::TooLarge => r#"{"ok":false,"error":"too-large"}"#.to_owned(),
        ForumAttachOutcome::ClockSkew => r#"{"ok":false,"error":"clock-skew"}"#.to_owned(),
        ForumAttachOutcome::ServerError => r#"{"ok":false,"error":"server-error"}"#.to_owned(),
        ForumAttachOutcome::Failed(reason) => format!(
            r#"{{"ok":false,"error":"error","reason":"{}"}}"#,
            reason.token()
        ),
    }
}

/// What a session id typed by hand stands for. The forum's attach page
/// prints its session id in the same shape as the sign-in page's code, and a
/// reader whose "Open the app" button did nothing types whichever they see
/// into the one screen that takes a code; the app places it by reading the
/// two unsigned status endpoints before any prompt is raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeKind {
    /// A pending sign-in session: the login consent applies.
    Login,
    /// A pending attach-logs session: the attach consent applies.
    Attach,
    /// Neither answers, or the attach session is already received, done or
    /// cancelled: the code is spent, whatever it was.
    Gone,
    /// The reads did not settle it (a transport failure, an unexpected
    /// status): the caller falls back to the login flow, which preflights
    /// again before signing.
    Unknown,
}

/// Connect's status word for an attach session the app has not answered yet.
const ATTACH_PENDING_TOKEN: &[u8] = br#""status":"pending""#;

/// Places a typed code from the login status read (`GET
/// /v1/session/<sid>/status`) and, when that one answered 404, the attach
/// status read (`GET /v1/attach/<sid>/status`) with its body. `None` for a
/// read that got no HTTP answer.
#[must_use]
pub fn classify_code_probe(login_status: Option<u16>, attach: Option<(u16, &[u8])>) -> CodeKind {
    match login_status {
        Some(200..=299) => CodeKind::Login,
        Some(404) => match attach {
            Some((200..=299, body)) if body_carries(body, ATTACH_PENDING_TOKEN) => CodeKind::Attach,
            Some((200..=299, _)) | Some((404, _)) => CodeKind::Gone,
            _ => CodeKind::Unknown,
        },
        _ => CodeKind::Unknown,
    }
}

/// The JSON envelope of a code probe, `{"kind":"login"|"attach"|"gone"|"unknown"}`.
#[must_use]
pub fn code_probe_envelope(kind: CodeKind) -> String {
    let word = match kind {
        CodeKind::Login => "login",
        CodeKind::Attach => "attach",
        CodeKind::Gone => "gone",
        CodeKind::Unknown => "unknown",
    };
    format!(r#"{{"kind":"{word}"}}"#)
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

#[cfg(test)]
mod attach_tests {
    use super::*;

    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const SID: &str = "0123456789abcdef0123456789abcdef";
    const HOST: &str = "connect.warrenbrowse.com";

    #[test]
    fn the_attach_body_pins_the_wire_contract() {
        // The three fields warren-connect's `AttachLogsBody` reads, in the
        // sorted order its own integration tests send (serde_json), with the
        // standard base64 alphabet and padding.
        assert_eq!(
            attach_body(SID, 42, b"hello").expect("builds"),
            format!(r#"{{"log_gz_b64":"aGVsbG8=","sid":"{SID}","topic_id":42}}"#).into_bytes()
        );
        assert_eq!(
            attach_body(SID, 0, b"hello").expect("pre-topic builds"),
            format!(r#"{{"log_gz_b64":"aGVsbG8=","sid":"{SID}","topic_id":0}}"#).into_bytes()
        );
    }

    #[test]
    fn the_attach_body_refuses_an_empty_log_and_one_over_the_broker_cap() {
        assert_eq!(attach_body(SID, 42, b""), Err(ForumRequestError::Invalid));
        let at_cap = vec![0u8; MAX_LOG_GZ_BYTES];
        assert!(
            attach_body(SID, 42, &at_cap).is_ok(),
            "the cap itself fits the field"
        );
        let over = vec![0u8; MAX_LOG_GZ_BYTES + 1];
        assert_eq!(
            attach_body(SID, 42, &over),
            Err(ForumRequestError::LogTooLarge)
        );
    }

    #[test]
    fn build_signed_attach_request_targets_the_attach_route_at_the_given_stamp() {
        let req = build_signed_attach_request(PHRASE, SID, HOST, 42, b"gz", 1_800_000_000)
            .expect("a valid mnemonic + link must build a request");
        assert_eq!(
            req.url,
            "https://connect.warrenbrowse.com/v1/forum/attach-logs"
        );
        assert_eq!(req.body, attach_body(SID, 42, b"gz").expect("body"));
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

    #[test]
    fn the_attach_session_urls_follow_the_allowlist() {
        assert_eq!(
            build_attach_status_url(SID, HOST).as_deref(),
            Some(
                "https://connect.warrenbrowse.com/v1/attach/0123456789abcdef0123456789abcdef/status"
            )
        );
        assert_eq!(
            build_attach_cancel_url(SID, HOST).as_deref(),
            Some(
                "https://connect.warrenbrowse.com/v1/attach/0123456789abcdef0123456789abcdef/cancel"
            )
        );
        assert_eq!(build_attach_status_url(SID, "evil.example.com"), None);
        assert_eq!(build_attach_cancel_url("NOTHEX", HOST), None);
    }

    #[test]
    fn the_provider_answers_map_like_the_desktop_attach_result() {
        // The table `desktop/.../main/forum-attach.ts` applies, plus the
        // clock-skew refusal the mobile login already tells apart.
        assert_eq!(
            attach_outcome_for_response(200, b"{\"status\":\"attached\"}"),
            ForumAttachOutcome::Attached
        );
        assert_eq!(
            attach_outcome_for_response(200, b"{\"status\":\"received\"}"),
            ForumAttachOutcome::Attached
        );
        assert_eq!(
            attach_outcome_for_response(403, b"{\"error\":\"not_author\"}"),
            ForumAttachOutcome::NotAuthor
        );
        assert_eq!(
            attach_outcome_for_response(404, b""),
            ForumAttachOutcome::Expired
        );
        assert_eq!(
            attach_outcome_for_response(413, b""),
            ForumAttachOutcome::TooLarge
        );
        assert_eq!(
            attach_outcome_for_response(401, b"{\"error\":\"clock_skew\"}"),
            ForumAttachOutcome::ClockSkew
        );
        assert_eq!(
            attach_outcome_for_response(401, b"{\"error\":\"unauthorized\"}"),
            ForumAttachOutcome::Failed(FailReason::Http(401))
        );
        assert_eq!(
            attach_outcome_for_response(500, b""),
            ForumAttachOutcome::ServerError
        );
        assert_eq!(
            attach_outcome_for_response(502, b""),
            ForumAttachOutcome::ServerError
        );
        assert_eq!(
            attach_outcome_for_response(429, b""),
            ForumAttachOutcome::Failed(FailReason::Http(429))
        );
    }

    #[test]
    fn the_attach_envelope_carries_the_class_and_never_a_value() {
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::Attached),
            r#"{"ok":true}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::NotAuthor),
            r#"{"ok":false,"error":"not-author"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::Expired),
            r#"{"ok":false,"error":"expired"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::TooLarge),
            r#"{"ok":false,"error":"too-large"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::ClockSkew),
            r#"{"ok":false,"error":"clock-skew"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::ServerError),
            r#"{"ok":false,"error":"server-error"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::Failed(FailReason::Transport)),
            r#"{"ok":false,"error":"error","reason":"transport"}"#
        );
        assert_eq!(
            attach_envelope(&ForumAttachOutcome::Failed(FailReason::Http(418))),
            r#"{"ok":false,"error":"error","reason":"http-418"}"#
        );
    }

    #[test]
    fn a_typed_code_is_placed_by_the_two_status_reads() {
        // The login status answers first; only a dead login session is asked
        // about as an attach session, and only a pending one is offered as
        // one: a received or cancelled attach session is as spent as a dead
        // login.
        assert_eq!(classify_code_probe(Some(200), None), CodeKind::Login);
        assert_eq!(
            classify_code_probe(Some(404), Some((200, br#"{"status":"pending"}"#))),
            CodeKind::Attach
        );
        assert_eq!(
            classify_code_probe(Some(404), Some((200, br#"{"status":"received"}"#))),
            CodeKind::Gone
        );
        assert_eq!(
            classify_code_probe(
                Some(404),
                Some((200, br#"{"status":"cancelled","reason":"user_cancelled"}"#))
            ),
            CodeKind::Gone
        );
        assert_eq!(
            classify_code_probe(Some(404), Some((404, b""))),
            CodeKind::Gone
        );
        assert_eq!(classify_code_probe(Some(404), None), CodeKind::Unknown);
        assert_eq!(classify_code_probe(Some(503), None), CodeKind::Unknown);
        assert_eq!(classify_code_probe(None, None), CodeKind::Unknown);
    }

    #[test]
    fn the_code_probe_envelope_names_the_kind() {
        assert_eq!(code_probe_envelope(CodeKind::Login), r#"{"kind":"login"}"#);
        assert_eq!(
            code_probe_envelope(CodeKind::Attach),
            r#"{"kind":"attach"}"#
        );
        assert_eq!(code_probe_envelope(CodeKind::Gone), r#"{"kind":"gone"}"#);
        assert_eq!(
            code_probe_envelope(CodeKind::Unknown),
            r#"{"kind":"unknown"}"#
        );
    }
}
