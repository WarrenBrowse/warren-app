//! Problem-report collection on Android, the substrate of the in-app bug
//! report: the same collector, redaction and `System information:` header the
//! desktop ships (`mullvad-problem-report`), fed with what only Kotlin can
//! read (platform state) and what only Rust writes (the engine log file).
//!
//! The pure parts (the metadata and redaction inputs crossing the JNI) are
//! host-tested here; the collection itself is Android-gated because the
//! collector's Android arm is.

use std::collections::BTreeMap;

/// Longest metadata key accepted from Kotlin. Keys are ours, so a long one is
/// a bug rather than an attack; the cap only keeps the header readable.
const MAX_KEY_CHARS: usize = 64;
/// Longest metadata value kept, per key: a value is one fact, never a log.
const MAX_VALUE_CHARS: usize = 512;
/// Most redaction strings accepted: the wallet address plus a handful.
const MAX_REDACT_STRINGS: usize = 16;

/// Parses the metadata Kotlin hands over as one JSON object of string values.
/// Keys are lower-cased, and a value is clamped so a runaway platform read
/// cannot bloat the header. Non-string values are rendered with their JSON
/// form, so a boolean or a number needs no special casing on the Kotlin side.
///
/// # Errors
///
/// Returns `Err` when `json` is not a JSON object.
pub fn parse_metadata(json: &str) -> Result<BTreeMap<String, String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "metadata is not JSON".to_owned())?;
    let object = value
        .as_object()
        .ok_or_else(|| "metadata is not a JSON object".to_owned())?;
    let mut out = BTreeMap::new();
    for (key, value) in object {
        let key: String = key
            .trim()
            .to_ascii_lowercase()
            .chars()
            .take(MAX_KEY_CHARS)
            .collect();
        if key.is_empty() {
            continue;
        }
        let rendered = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => "null".to_owned(),
            other => other.to_string(),
        };
        let clamped: String = rendered
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(MAX_VALUE_CHARS)
            .collect();
        out.insert(key, clamped);
    }
    Ok(out)
}

/// Parses the redaction strings Kotlin hands over as a JSON array of strings
/// (the wallet address, at least). Empty entries are dropped, the list is
/// capped, and an unparseable input redacts nothing extra rather than failing
/// the report: the collector's own IP/MAC/UUID rules still apply.
#[must_use]
pub fn parse_redact_strings(json: &str) -> Vec<String> {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .take(MAX_REDACT_STRINGS)
                .collect()
        })
        .unwrap_or_default()
}

/// The outcome envelope of a collection, for the JNI caller.
#[must_use]
pub fn collect_envelope(result: &Result<u64, String>) -> String {
    match result {
        Ok(bytes) => format!(r#"{{"ok":true,"bytes":{bytes}}}"#),
        Err(reason) => {
            // The reason is one of our fixed phrases plus an io error kind,
            // never a path or a value from the report.
            let safe: String = reason
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == ':')
                .take(120)
                .collect();
            format!(r#"{{"ok":false,"error":"{safe}"}}"#)
        }
    }
}

/// Collects the redacted report into `output_path`. The metadata block gets
/// every key Kotlin supplied (platform, ROM, clock, deep-link routing, tunnel
/// state, probes); the log blocks are the Rust engine log directory, the
/// Kotlin app log directory and a `logcat -d` dump, each clamped and redacted
/// by the shared collector. Returns the size of the written report.
///
/// # Errors
///
/// A fixed phrase naming the failing step plus the io error kind; never a
/// path or a value from the report.
#[cfg(target_os = "android")]
pub fn collect(
    metadata: BTreeMap<String, String>,
    redact: Vec<String>,
    rust_log_dir: &std::path::Path,
    app_log_dir: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<u64, String> {
    use mullvad_problem_report::ProblemReportCollector;

    // The platform facts Kotlin read are handed to the metadata hook the
    // collector already consults on Android.
    talpid_platform_metadata::set_extra_metadata(metadata.into_iter().collect());
    let collector = ProblemReportCollector {
        extra_logs: Vec::new(),
        redact_custom_strings: redact,
        android_log_dir: rust_log_dir.to_path_buf(),
        extra_logs_dir: app_log_dir.to_path_buf(),
        unverified_purchases: 0,
        pending_purchases: 0,
    };
    collector
        .write_to_path(output_path)
        .map_err(|e| format!("collect failed: {}", error_kind(&e)))?;
    std::fs::metadata(output_path)
        .map(|m| m.len())
        .map_err(|e| format!("report unreadable: {:?}", e.kind()))
}

/// A coarse name for a collector failure: the variant, never its path. On
/// Android the collector has exactly one failure, the write of the report
/// (every log read is folded into the report as an error block instead).
#[cfg(target_os = "android")]
fn error_kind(err: &mullvad_problem_report::Error) -> &'static str {
    let mullvad_problem_report::Error::WriteReportError { .. } = err;
    "write"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_keys_are_lowercased_values_clamped_and_control_chars_dropped() {
        let parsed = parse_metadata(
            r#"{"Time-Auto":"1","clock-offset":12,"probe":{"class":"ok"},"note":"a\nb","":"x"}"#,
        )
        .expect("an object parses");
        assert_eq!(parsed["time-auto"], "1");
        assert_eq!(parsed["clock-offset"], "12");
        assert_eq!(parsed["probe"], r#"{"class":"ok"}"#);
        assert_eq!(
            parsed["note"], "a b",
            "a newline cannot forge a header line"
        );
        assert!(!parsed.contains_key(""));
        let long = format!(r#"{{"k":"{}"}}"#, "v".repeat(2_000));
        assert_eq!(
            parse_metadata(&long).expect("parses")["k"].chars().count(),
            MAX_VALUE_CHARS
        );
    }

    #[test]
    fn metadata_must_be_an_object() {
        assert!(parse_metadata("[1]").is_err());
        assert!(parse_metadata("nope").is_err());
    }

    #[test]
    fn redact_strings_are_trimmed_deduplicated_of_empties_and_capped() {
        assert_eq!(
            parse_redact_strings(r#"[" wb7kgy8FF4rx ","","x"]"#),
            vec!["wb7kgy8FF4rx".to_owned(), "x".to_owned()]
        );
        assert!(parse_redact_strings("not json").is_empty());
        assert!(parse_redact_strings(r#"{"a":1}"#).is_empty());
        let many = format!("[{}]", vec!["\"s\""; 40].join(","));
        assert_eq!(parse_redact_strings(&many).len(), MAX_REDACT_STRINGS);
    }

    #[test]
    fn collect_envelope_carries_the_size_or_a_sanitized_reason() {
        assert_eq!(collect_envelope(&Ok(1234)), r#"{"ok":true,"bytes":1234}"#);
        assert_eq!(
            collect_envelope(&Err("collect failed: write \"/data/x\"".into())),
            r#"{"ok":false,"error":"collect failed: write datax"}"#,
            "a path never survives into the envelope"
        );
    }
}
