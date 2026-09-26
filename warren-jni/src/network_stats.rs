//! The public network transparency snapshot (`GET /v1/network/stats`) on
//! Android: one unauthenticated, unsigned GET whose body Kotlin parses and
//! renders.
//!
//! The snapshot is advisory display data, never a trust input, so nothing here
//! verifies a signature and nothing steers a selection on it. Rust only checks
//! that the body is the document it claims to be (a JSON object at a schema
//! version this client understands) and wraps it in the `{"ok":..}` envelope
//! the other display fetches use. The fields themselves are left to Kotlin,
//! which reads them tolerantly: an additive field or a new enum value on the
//! server must not blank the screen of a client that shipped before it.
//!
//! Kotlin owns the cadence (one fetch per window, only while a surface that
//! shows the figures is on screen). There is no conditional GET: the shared
//! API transport reads no response header, so no ETag can be carried across
//! polls, and the snapshot changes at every window anyway.

use serde_json::Value;

/// Route of the snapshot on the API host.
const STATS_PATH: &str = "/v1/network/stats";

/// Refuses a body far beyond what a fleet snapshot weighs, so a captive portal
/// or a broken proxy cannot push megabytes across the bridge into the UI. The
/// shared transport has already read the body by then; this bounds what
/// crosses into Kotlin and what Kotlin parses.
pub(crate) const MAX_BODY_BYTES: usize = 512 * 1024;

/// The schema version this client reads. A rename or a retype bumps it on the
/// server, and a document at another version would be misread field by field,
/// so it is refused rather than half rendered.
const SUPPORTED_VERSION: u64 = 1;

/// What one GET brought back, as the transport saw it.
pub(crate) enum Fetched {
    /// `200`: the body bytes.
    Body(Vec<u8>),
    /// Any other status.
    Status(u16),
    /// The request never got an HTTP answer.
    Transport,
}

/// Why no snapshot crossed the bridge. The tokens are the FFI contract Kotlin
/// switches on; none carries anything the server sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// The request never got an HTTP answer.
    Transport,
    /// `404`: this API does not serve the snapshot (yet). Stable for minutes,
    /// unlike every other failure.
    Unavailable,
    /// Any other non-200 status.
    Status,
    /// Over [`MAX_BODY_BYTES`].
    TooLarge,
    /// Not UTF-8, not JSON, or not a JSON object.
    Malformed,
    /// A JSON object without the supported `version`.
    Version,
}

impl Refusal {
    fn token(self) -> &'static str {
        match self {
            Refusal::Transport => "transport",
            Refusal::Unavailable => "unavailable",
            Refusal::Status => "status",
            Refusal::TooLarge => "too_large",
            Refusal::Malformed => "malformed",
            Refusal::Version => "version",
        }
    }
}

/// The snapshot URL for `api_base`, which may or may not end in a slash.
pub(crate) fn stats_url(api_base: &str) -> String {
    format!("{}{STATS_PATH}", api_base.trim_end_matches('/'))
}

/// The FFI envelope for one fetch: `{"ok":true,"stats":{..}}` carrying the
/// document as served, or `{"ok":false,"reason":".."}` with a fixed class
/// (`transport`, `unavailable`, `status`, `too_large`, `malformed`,
/// `version`). The reason never carries the body or anything the server sent.
pub(crate) fn envelope(fetched: Fetched) -> String {
    match accept(fetched) {
        // The body is spliced in verbatim rather than re-serialized from the
        // parsed value, so a number or a key order the server chose reaches
        // Kotlin exactly as served. It is a JSON object at this point, so the
        // result is a JSON object too.
        Ok(document) => format!(r#"{{"ok":true,"stats":{document}}}"#),
        Err(refusal) => format!(r#"{{"ok":false,"reason":"{}"}}"#, refusal.token()),
    }
}

/// The body as UTF-8 when it is a JSON object at [`SUPPORTED_VERSION`].
fn accept(fetched: Fetched) -> Result<String, Refusal> {
    let body = match fetched {
        Fetched::Body(body) => body,
        Fetched::Status(404) => return Err(Refusal::Unavailable),
        Fetched::Status(_) => return Err(Refusal::Status),
        Fetched::Transport => return Err(Refusal::Transport),
    };
    if body.len() > MAX_BODY_BYTES {
        return Err(Refusal::TooLarge);
    }
    let document = String::from_utf8(body).map_err(|_| Refusal::Malformed)?;
    let value: Value = serde_json::from_str(&document).map_err(|_| Refusal::Malformed)?;
    let object = value.as_object().ok_or(Refusal::Malformed)?;
    if object.get("version").and_then(Value::as_u64) != Some(SUPPORTED_VERSION) {
        return Err(Refusal::Version);
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::{Fetched, MAX_BODY_BYTES, envelope, stats_url};
    use serde_json::{Value, json};

    fn parse(envelope: &str) -> Value {
        serde_json::from_str(envelope).expect("the envelope is always JSON")
    }

    fn body(value: &Value) -> Fetched {
        Fetched::Body(serde_json::to_vec(value).expect("test body serializes"))
    }

    #[test]
    fn stats_url_appends_the_route_to_the_api_base() {
        assert_eq!(
            stats_url("https://api.beta.warrenbrowse.com"),
            "https://api.beta.warrenbrowse.com/v1/network/stats"
        );
    }

    #[test]
    fn stats_url_does_not_double_a_trailing_slash() {
        assert_eq!(
            stats_url("https://api.warrenbrowse.com/"),
            "https://api.warrenbrowse.com/v1/network/stats"
        );
    }

    #[test]
    fn a_version_one_document_is_passed_through_whole() {
        // Unknown fields and unknown enum values ride along untouched: the
        // tolerant reading is Kotlin's, and dropping them here would make an
        // additive server change invisible to a client that could read it.
        let doc = json!({
            "version": 1,
            "generated_at": 1_790_000_000_u64,
            "future_field": {"nested": [1, 2]},
            "exits": [{"exit_id": "ab", "load_level": "overloaded"}],
            "fleet": {"transferred_24h_bytes": 9_000_000_000_000_u64},
        });

        let out = parse(&envelope(body(&doc)));

        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["stats"], doc);
    }

    #[test]
    fn a_document_at_another_version_is_refused() {
        let out = parse(&envelope(body(&json!({"version": 2, "exits": []}))));

        assert_eq!(out, json!({"ok": false, "reason": "version"}));
    }

    #[test]
    fn a_document_without_a_version_is_refused() {
        let out = parse(&envelope(body(&json!({"exits": []}))));

        assert_eq!(out, json!({"ok": false, "reason": "version"}));
    }

    #[test]
    fn a_json_value_that_is_not_an_object_is_malformed() {
        let out = parse(&envelope(body(&json!([{"version": 1}]))));

        assert_eq!(out, json!({"ok": false, "reason": "malformed"}));
    }

    #[test]
    fn a_body_that_is_not_json_is_malformed_and_not_echoed() {
        let out = envelope(Fetched::Body(
            b"<html>503 Service Unavailable</html>".to_vec(),
        ));

        assert_eq!(parse(&out), json!({"ok": false, "reason": "malformed"}));
        assert!(
            !out.contains("Service"),
            "the body must never reach the envelope"
        );
    }

    #[test]
    fn a_404_says_the_api_does_not_serve_the_snapshot_yet() {
        // Kotlin backs off for minutes on this reason and shows "not available
        // right now"; any other status is retried at the next window.
        let out = parse(&envelope(Fetched::Status(404)));

        assert_eq!(out, json!({"ok": false, "reason": "unavailable"}));
    }

    #[test]
    fn another_non_200_status_is_a_status_failure() {
        let out = parse(&envelope(Fetched::Status(503)));

        assert_eq!(out, json!({"ok": false, "reason": "status"}));
    }

    #[test]
    fn a_body_over_the_bound_is_refused_before_parsing() {
        let mut oversize = br#"{"version":1,"pad":""#.to_vec();
        oversize.resize(MAX_BODY_BYTES, b'a');
        oversize.extend_from_slice(br#""}"#);

        let out = parse(&envelope(Fetched::Body(oversize)));

        assert_eq!(out, json!({"ok": false, "reason": "too_large"}));
    }

    #[test]
    fn a_body_at_the_bound_is_accepted() {
        let mut at_bound = br#"{"version":1,"pad":""#.to_vec();
        at_bound.resize(MAX_BODY_BYTES - 2, b'a');
        at_bound.extend_from_slice(br#""}"#);
        assert_eq!(at_bound.len(), MAX_BODY_BYTES);

        let out = parse(&envelope(Fetched::Body(at_bound)));

        assert_eq!(out["ok"], json!(true));
    }

    #[test]
    fn a_body_that_is_not_utf8_is_malformed() {
        let out = parse(&envelope(Fetched::Body(vec![b'{', 0xff, b'}'])));

        assert_eq!(out, json!({"ok": false, "reason": "malformed"}));
    }

    #[test]
    fn a_request_that_got_no_answer_is_a_transport_failure() {
        let out = parse(&envelope(Fetched::Transport));

        assert_eq!(out, json!({"ok": false, "reason": "transport"}));
    }
}
