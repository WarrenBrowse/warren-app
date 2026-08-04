//! Background fetcher of the public `GET /v1/network` environment
//! descriptor (`warren_contract::dto::NetworkInfoResponse`).
//!
//! The endpoint is unauthenticated display data: environment label,
//! degraded flag, default bandwidth cap and whether payment flows are
//! exposed. The loop fetches once at daemon startup and then
//! periodically, records the result in the shared
//! [`WarrenStatusCache`] (which pushes it to both UIs over the
//! existing `WarrenStatusUpdates` stream) and backs off exponentially
//! on transient failures. An API that predates the endpoint (404)
//! is not an error: the cache simply keeps `None` and the loop stays
//! on the slow refresh cadence, so old servers cost nothing.

use std::time::Duration;

use warren_contract::dto::NetworkInfoResponse;

use crate::warren_status::{NetworkInfoSnapshot, WarrenStatusCache};

/// Cadence of the steady-state refresh. The descriptor changes rarely
/// (an ops-side flip of the beta cap), so a slow poll is enough.
const REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// First retry delay after a transient failure.
const INITIAL_RETRY: Duration = Duration::from_secs(15);
/// Upper bound of the failure backoff.
const MAX_RETRY: Duration = Duration::from_secs(10 * 60);
/// Per-request timeout.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// Outcome of one fetch attempt, separated from the transport so the
/// classification is unit-testable without a network.
#[derive(Debug, PartialEq, Eq)]
enum FetchOutcome {
    /// The API served a descriptor.
    Info(NetworkInfoSnapshot),
    /// The API predates the endpoint (404): not an error, no info.
    Unsupported,
}

/// `{api_base}/v1/network`, tolerant of a trailing slash on the base.
fn endpoint_url(api_base: &str) -> String {
    format!("{}/v1/network", api_base.trim_end_matches('/'))
}

fn snapshot_from(resp: NetworkInfoResponse) -> NetworkInfoSnapshot {
    NetworkInfoSnapshot {
        environment: resp.environment,
        degraded: resp.degraded,
        default_rate_bps: resp.default_rate_bps,
        payments_enabled: resp.payments_enabled,
    }
}

/// Classify an HTTP response into an outcome. `Err` means transient
/// (retry with backoff); `Ok` feeds the cache.
fn classify(status: u16, body: &[u8]) -> Result<FetchOutcome, String> {
    match status {
        200 => serde_json::from_slice::<NetworkInfoResponse>(body)
            .map(|resp| FetchOutcome::Info(snapshot_from(resp)))
            .map_err(|e| format!("invalid /v1/network body: {e}")),
        404 => Ok(FetchOutcome::Unsupported),
        other => Err(format!("/v1/network returned HTTP {other}")),
    }
}

/// Spawn the refresh loop against the daemon's resolved API base URL.
pub(crate) fn spawn(api_base: String, cache: WarrenStatusCache) {
    tokio::spawn(run(api_base, cache));
}

async fn run(api_base: String, cache: WarrenStatusCache) {
    // Resolved from the daemon's address cache like every other Warren
    // fetcher, so a blocking state does not silently freeze the descriptor.
    let Ok(client) =
        crate::warren_api_dns::with_api_resolver(reqwest::Client::builder().timeout(FETCH_TIMEOUT))
            .build()
    else {
        log::warn!("network-info fetcher disabled: reqwest client build failed");
        return;
    };
    let url = endpoint_url(&api_base);
    let mut retry = INITIAL_RETRY;
    loop {
        let outcome = match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.bytes().await {
                    Ok(body) => classify(status, &body),
                    Err(e) => Err(format!("/v1/network body read failed: {e}")),
                }
            }
            Err(e) => Err(format!("/v1/network request failed: {e}")),
        };
        let delay = match outcome {
            Ok(FetchOutcome::Info(snapshot)) => {
                cache.set_network_info(Some(snapshot));
                retry = INITIAL_RETRY;
                REFRESH_INTERVAL
            }
            Ok(FetchOutcome::Unsupported) => {
                // Old server: leave whatever the cache holds (normally
                // None) untouched and re-check on the slow cadence, in
                // case the backend gains the endpoint under us.
                retry = INITIAL_RETRY;
                REFRESH_INTERVAL
            }
            Err(e) => {
                log::debug!("network-info fetch failed (will retry): {e}");
                let delay = retry;
                retry = (retry * 2).min(MAX_RETRY);
                delay
            }
        };
        tokio::time::sleep(delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_url_joins_with_and_without_trailing_slash() {
        assert_eq!(
            endpoint_url("https://api.beta.warrenbrowse.com"),
            "https://api.beta.warrenbrowse.com/v1/network"
        );
        assert_eq!(
            endpoint_url("https://api.beta.warrenbrowse.com/"),
            "https://api.beta.warrenbrowse.com/v1/network"
        );
    }

    #[test]
    fn classify_parses_a_degraded_beta_descriptor() {
        let body = br#"{"environment":"beta","degraded":true,"default_rate_bps":20000000,"payments_enabled":false}"#;
        let outcome = classify(200, body).expect("valid body must classify");
        assert_eq!(
            outcome,
            FetchOutcome::Info(NetworkInfoSnapshot {
                environment: "beta".to_owned(),
                degraded: true,
                default_rate_bps: Some(20_000_000),
                payments_enabled: false,
            })
        );
    }

    #[test]
    fn classify_accepts_a_descriptor_without_rate_cap() {
        let body = br#"{"environment":"production","degraded":false,"payments_enabled":true}"#;
        let outcome = classify(200, body).expect("absent rate cap is valid");
        assert_eq!(
            outcome,
            FetchOutcome::Info(NetworkInfoSnapshot {
                environment: "production".to_owned(),
                degraded: false,
                default_rate_bps: None,
                payments_enabled: true,
            })
        );
    }

    #[test]
    fn classify_maps_404_to_unsupported_not_error() {
        // Servers predating the endpoint must not put the loop on the
        // failure backoff: 404 is a stable, expected answer.
        assert_eq!(classify(404, b"not found"), Ok(FetchOutcome::Unsupported));
    }

    #[test]
    fn classify_rejects_server_errors_as_transient() {
        assert!(classify(500, b"boom").is_err());
        assert!(classify(429, b"slow down").is_err());
    }

    #[test]
    fn classify_rejects_invalid_json_as_transient() {
        assert!(classify(200, b"<html>captive portal</html>").is_err());
    }
}
