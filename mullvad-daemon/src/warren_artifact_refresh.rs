//! Shared policy pieces of the periodic signed-artifact refreshers: the
//! relay-list updater and the multi-hop directory updater are the two
//! instances. The pieces that drifted apart in the field (the post-wake
//! transport-failure fast retry, the ETag conditional GET, the atomic cache
//! write) live here once, parameterized per artifact; the two loops keep
//! only their artifact-specific verify/accept plumbing and cadences.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Per-request network timeout, shared by both artifact fetchers.
pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// Current unix epoch seconds (saturates to 0 on a pre-epoch clock).
pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Splits a pinned-pubkey config value (single key, or comma-separated set
/// for key rotation) into the slice the `_any` verifiers take.
/// Empty/`None` yields empty (TOFU).
pub(crate) fn split_pins(pin: Option<&str>) -> Vec<&str> {
    pin.map(|p| {
        p.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// Post-wake fast retry after a TRANSPORT fetch failure (the stranding
/// class: a coarse periodic tick whose tokio timer does not even advance
/// while the host sleeps would leave a woken client stale for up to a full
/// interval). Only transport failures arm it; verify/status failures and
/// successes clear it. Delays double from `min` up to `max`.
pub(crate) struct TransportRetryBackoff {
    min: Duration,
    max: Duration,
    current: Option<Duration>,
}

impl TransportRetryBackoff {
    pub(crate) fn new(min: Duration, max: Duration) -> Self {
        Self {
            min,
            max,
            current: None,
        }
    }

    /// Arms (or doubles) the fast retry; returns the next delay.
    pub(crate) fn on_transport_failure(&mut self) -> Duration {
        let next = self.current.map_or(self.min, |d| (d * 2).min(self.max));
        self.current = Some(next);
        next
    }

    /// Disarms the fast retry (success, or a non-transport failure a fast
    /// retry would not help).
    pub(crate) fn clear(&mut self) {
        self.current = None;
    }

    /// The armed delay, if any.
    pub(crate) fn delay(&self) -> Option<Duration> {
        self.current
    }
}

/// Outcome of one conditional GET.
pub(crate) enum FetchResponse {
    /// `304`: the cached artifact is current.
    NotModified,
    /// `200`: a fresh body; `etag` is the response validator, falling back
    /// to the request's own (a server that stops sending one must not
    /// erase the last known validator).
    Body { body: String, etag: Option<String> },
    /// Any other status; the caller maps it to its artifact semantics
    /// (e.g. `404` = not published yet vs keep-last-good).
    Status(u16),
}

/// One ETag conditional GET: sends `If-None-Match` when a validator is
/// known, so an unchanged artifact costs one header round trip instead of a
/// full body download + re-verify.
///
/// # Errors
///
/// Propagates transport failures (`reqwest`) untouched; the caller's retry
/// policy classifies them.
pub(crate) async fn conditional_get(
    http: &reqwest::Client,
    url: &str,
    etag: Option<String>,
) -> Result<FetchResponse, reqwest::Error> {
    let mut req = http.get(url);
    if let Some(tag) = &etag {
        req = req.header(reqwest::header::IF_NONE_MATCH, tag);
    }
    let resp = req.send().await?;
    let status = resp.status().as_u16();
    if status == 304 {
        return Ok(FetchResponse::NotModified);
    }
    if status != 200 {
        return Ok(FetchResponse::Status(status));
    }
    let new_etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or(etag);
    let body = resp.text().await?;
    Ok(FetchResponse::Body {
        body,
        etag: new_etag,
    })
}

/// Atomically replaces the on-disk artifact cache: write to a temp sibling,
/// then rename, so a crash mid-write can never leave a truncated cache that
/// would fail verification at the next boot.
pub(crate) fn write_cache_atomic(cache_path: &Path, raw: &str) -> std::io::Result<()> {
    let tmp = cache_path.with_extension("json.tmp");
    std::fs::write(&tmp, raw)?;
    std::fs::rename(&tmp, cache_path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_retry_doubles_from_min_to_cap_and_clears() {
        let mut b = TransportRetryBackoff::new(Duration::from_secs(15), Duration::from_secs(300));
        assert_eq!(b.delay(), None, "disarmed until a transport failure");
        assert_eq!(b.on_transport_failure(), Duration::from_secs(15));
        assert_eq!(b.on_transport_failure(), Duration::from_secs(30));
        assert_eq!(b.on_transport_failure(), Duration::from_secs(60));
        assert_eq!(b.on_transport_failure(), Duration::from_secs(120));
        assert_eq!(b.on_transport_failure(), Duration::from_secs(240));
        assert_eq!(
            b.on_transport_failure(),
            Duration::from_secs(300),
            "capped at max"
        );
        assert_eq!(b.on_transport_failure(), Duration::from_secs(300));
        assert_eq!(b.delay(), Some(Duration::from_secs(300)));
        b.clear();
        assert_eq!(b.delay(), None);
        assert_eq!(
            b.on_transport_failure(),
            Duration::from_secs(15),
            "re-arms from min after a clear"
        );
    }

    #[test]
    fn split_pins_handles_rotation_sets_and_blanks() {
        assert!(split_pins(None).is_empty());
        assert!(split_pins(Some("  ,, ")).is_empty());
        assert_eq!(split_pins(Some("aa")), vec!["aa"]);
        assert_eq!(split_pins(Some("aa, bb ,")), vec!["aa", "bb"]);
    }

    fn isolated_tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "warren-artifact-refresh-test-{}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn write_cache_atomic_writes_and_leaves_no_temp() {
        let dir = isolated_tempdir();
        let path = dir.join("artifact.json");
        write_cache_atomic(&path, "hello-signed-artifact").expect("write ok");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "hello-signed-artifact"
        );
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temp file may remain");
    }

    #[test]
    fn write_cache_atomic_overwrites_previous_good_cache() {
        let dir = isolated_tempdir();
        let path = dir.join("artifact.json");
        write_cache_atomic(&path, "first").expect("write ok");
        write_cache_atomic(&path, "second").expect("overwrite ok");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
    }
}
