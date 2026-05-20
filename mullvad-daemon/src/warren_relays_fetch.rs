//! Bootstrap fetcher for `<cache_dir>/warren-relays.json`.
//!
//! At daemon boot in Warren mode, we attempt a `GET {api_url}/v1/exits`
//! (public endpoint, see `warren-api/src/handlers.rs` § `list_exits`),
//! and write the raw response to the cache. The server Ed25519
//! signature verification (v2 format) is then done by
//! `DaemonWarrenRelaySelector::load_from_cache_dir`.
//!
//! Best-effort: if the fetch fails (network down, DNS, TLS, 5xx, invalid
//! JSON), we log a warn and leave the previous cache in place. The
//! state machine will return `NoRelayMatch` if no valid cache
//! exists, expected behavior = the user is not yet
//! connectable to the Warren network.

use std::path::Path;
use std::time::Duration;

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const RELAYS_FILENAME: &str = "warren-relays.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("server returned non-success status {0}")]
    Status(u16),

    #[error("response body is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to write {0}: {1}")]
    Io(String, #[source] std::io::Error),
}

/// Fetches `/v1/exits` from `api_url` and writes the response to
/// `<cache_dir>/warren-relays.json`. Returns the number of bytes written
/// on success.
///
/// The body is syntactically verified (`serde_json::Value`) before
/// writing so as not to corrupt an existing valid cache with a
/// non-JSON response (Caddy error page, HTML redirect, etc.). The
/// cryptographic verification (server Ed25519 signature) remains the
/// responsibility of the downstream loader.
pub async fn fetch_and_cache_relays(api_url: &str, cache_dir: &Path) -> Result<usize, Error> {
    let url = format!("{}/v1/exits", api_url.trim_end_matches('/'));
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Status(status.as_u16()));
    }
    let body = resp.text().await?;
    let _: serde_json::Value = serde_json::from_str(&body)?;
    let path = cache_dir.join(RELAYS_FILENAME);
    std::fs::write(&path, &body).map_err(|e| Error::Io(path.display().to_string(), e))?;
    Ok(body.len())
}
