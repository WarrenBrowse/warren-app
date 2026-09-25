//! On-demand fetcher of the public `GET /v1/network/stats` transparency
//! snapshot (warren-core doc 106).
//!
//! Nothing here runs in the background: a periodic request is itself a
//! fingerprint of a running client, so the API is asked only when a frontend
//! asks the daemon, and every caller is served from one cached body until the
//! window that snapshot describes has closed. The body is passed through
//! verbatim after a sanity check and the frontends parse it, so the daemon
//! stays independent of the contract crate's revision of the schema.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use tokio::time::Instant;
use warren_discovery_core::WarrenRelayList;

/// Per-request timeout, as for every other public Warren fetch.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
/// The only schema this daemon passes through.
const SUPPORTED_VERSION: u64 = 1;
/// Refuses a body far beyond what a fleet snapshot can weigh, so a captive
/// portal or a broken proxy cannot push megabytes into both UIs.
const MAX_BODY_BYTES: usize = 512 * 1024;
/// The range the server accepts for its window (doc 106 knobs). A snapshot
/// claiming another window is cached as if it said the nearest bound.
const MIN_WINDOW_SECS: u64 = 30;
const MAX_WINDOW_SECS: u64 = 3600;
/// Floor on the interval between two fetches whatever the clocks say: a local
/// clock ahead of the server's would otherwise expire every snapshot on
/// arrival and turn each frontend poll into a request.
const MIN_REFETCH: Duration = Duration::from_secs(15);
/// How long an API without the endpoint is believed before asking again.
const UNSUPPORTED_TTL: Duration = Duration::from_secs(10 * 60);

/// A snapshot that passed the sanity check, with the two fields the cache
/// needs read out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsSnapshot {
    json: String,
    generated_at: u64,
    window_secs: u64,
}

impl StatsSnapshot {
    /// The body exactly as the API served it.
    #[must_use]
    pub fn json(&self) -> &str {
        &self.json
    }
}

/// What the API answered, once classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsOutcome {
    Snapshot(StatsSnapshot),
    /// The API predates the endpoint (404): a stable answer, not a failure.
    Unsupported,
}

/// Why no snapshot could be served. Transient: the caller asks again later.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StatsError {
    #[error("the Warren API base is not resolved yet")]
    NotConfigured,
    #[error("the network stats request failed")]
    Transport(#[source] reqwest::Error),
    #[error("the network stats endpoint answered HTTP {0}")]
    Status(u16),
    #[error("the network stats body was rejected: {0}")]
    InvalidBody(&'static str),
}

/// One exit of the signed relay list, named both ways: the `exit_id` the
/// snapshot carries and the hostname the frontends key their relays on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitHostname {
    pub exit_id: String,
    pub hostname: String,
}

/// `{api_base}/v1/network/stats`, tolerant of a trailing slash on the base.
fn endpoint_url(api_base: &str) -> String {
    format!("{}/v1/network/stats", api_base.trim_end_matches('/'))
}

/// Classify an HTTP answer. `Err` is transient and never cached.
fn classify(status: u16, body: &[u8]) -> Result<StatsOutcome, StatsError> {
    match status {
        200 => sanity_check(body).map(StatsOutcome::Snapshot),
        404 => Ok(StatsOutcome::Unsupported),
        other => Err(StatsError::Status(other)),
    }
}

/// Accepts a body only when it is a JSON object of the supported version that
/// says when its window closed and how long it lasts.
fn sanity_check(body: &[u8]) -> Result<StatsSnapshot, StatsError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(StatsError::InvalidBody("larger than any fleet snapshot"));
    }
    let json = std::str::from_utf8(body).map_err(|_| StatsError::InvalidBody("not UTF-8"))?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| StatsError::InvalidBody("not JSON"))?;
    let object = value
        .as_object()
        .ok_or(StatsError::InvalidBody("not a JSON object"))?;
    if object.get("version").and_then(serde_json::Value::as_u64) != Some(SUPPORTED_VERSION) {
        return Err(StatsError::InvalidBody("unsupported schema version"));
    }
    let generated_at = object
        .get("generated_at")
        .and_then(serde_json::Value::as_u64)
        .ok_or(StatsError::InvalidBody("no generated_at"))?;
    let window_secs = object
        .get("window_secs")
        .and_then(serde_json::Value::as_u64)
        .ok_or(StatsError::InvalidBody("no window_secs"))?;
    Ok(StatsSnapshot {
        json: json.to_owned(),
        generated_at,
        window_secs,
    })
}

/// The join table the frontends need, from the signed relay list.
fn exit_hostnames_of(list: &WarrenRelayList) -> Vec<ExitHostname> {
    list.relays()
        .iter()
        .map(|relay| ExitHostname {
            exit_id: relay.exit_id().to_hex(),
            hostname: crate::warren_relay_list_view::relay_hostname(relay),
        })
        .collect()
}

struct CachedOutcome {
    outcome: StatsOutcome,
    fetched_at: Instant,
}

impl CachedOutcome {
    fn is_fresh(&self, now: Instant, now_unix: u64) -> bool {
        let age = now.saturating_duration_since(self.fetched_at);
        match &self.outcome {
            StatsOutcome::Unsupported => age < UNSUPPORTED_TTL,
            StatsOutcome::Snapshot(snapshot) => {
                let window = snapshot.window_secs.clamp(MIN_WINDOW_SECS, MAX_WINDOW_SECS);
                // The snapshot cannot outlive one window from the moment it
                // was fetched, which bounds a local clock that runs behind.
                let window_open = age < Duration::from_secs(window)
                    && now_unix < snapshot.generated_at.saturating_add(window);
                age < MIN_REFETCH || window_open
            }
        }
    }
}

/// Shared handle: the gRPC service reads through it, the daemon feeds it the
/// API base once resolved and every verified relay list.
#[derive(Clone, Default)]
pub struct WarrenNetworkStats {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    api_base: OnceLock<String>,
    client: OnceLock<reqwest::Client>,
    // Held across the fetch, so callers racing on an expired entry share one
    // request instead of each sending their own.
    cache: tokio::sync::Mutex<Option<CachedOutcome>>,
    exit_hostnames: RwLock<Vec<ExitHostname>>,
}

impl WarrenNetworkStats {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the API base the snapshot is fetched from. The first call wins:
    /// the base is resolved once at boot, like every other Warren fetcher's.
    pub fn set_api_base(&self, api_base: String) {
        let _ = self.inner.api_base.set(api_base);
    }

    /// Replaces the `exit_id` to hostname join with the one of `list`.
    pub fn set_relay_list(&self, list: &WarrenRelayList) {
        *self
            .inner
            .exit_hostnames
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = exit_hostnames_of(list);
    }

    #[must_use]
    pub fn exit_hostnames(&self) -> Vec<ExitHostname> {
        self.inner
            .exit_hostnames
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The current snapshot, from the cache while its window is open,
    /// otherwise fetched now.
    ///
    /// # Errors
    ///
    /// Every [`StatsError`] is transient: no API base yet, a transport
    /// failure, an unexpected status, or a body that failed the sanity check.
    pub async fn get(&self) -> Result<StatsOutcome, StatsError> {
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        self.get_with(Instant::now(), now_unix, || self.fetch())
            .await
    }

    async fn get_with<F, Fut>(
        &self,
        now: Instant,
        now_unix: u64,
        fetch: F,
    ) -> Result<StatsOutcome, StatsError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<StatsOutcome, StatsError>>,
    {
        let mut cache = self.inner.cache.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.is_fresh(now, now_unix))
        {
            return Ok(cached.outcome.clone());
        }
        let outcome = fetch().await?;
        *cache = Some(CachedOutcome {
            outcome: outcome.clone(),
            fetched_at: now,
        });
        Ok(outcome)
    }

    async fn fetch(&self) -> Result<StatsOutcome, StatsError> {
        let api_base = self.inner.api_base.get().ok_or(StatsError::NotConfigured)?;
        let client = match self.inner.client.get() {
            Some(client) => client,
            None => {
                // Resolved from the daemon's address cache like every other
                // Warren fetcher, so a blocking state that drops system DNS
                // does not blank the view.
                let client = crate::warren_api_dns::with_api_resolver(
                    reqwest::Client::builder().timeout(FETCH_TIMEOUT),
                )
                .build()
                .map_err(StatsError::Transport)?;
                self.inner.client.get_or_init(|| client)
            }
        };
        let response = client
            .get(endpoint_url(api_base))
            .send()
            .await
            .map_err(StatsError::Transport)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BODY_BYTES as u64)
        {
            return Err(StatsError::InvalidBody("larger than any fleet snapshot"));
        }
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(StatsError::Transport)?;
        classify(status, &body)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ed25519_dalek::SigningKey;
    use warren_discovery_core::warren_types::{ExitId, WarrenPubkey};
    use warren_discovery_core::{Addr, Ingress, Listener, Location, WarrenRelay};

    use super::*;

    const FIXTURE: &str = r#"{"version":1,"environment":"beta","generated_at":1790000000,
        "window_secs":60,"exit_users_rounding":5,"exit_live_threshold":20,
        "users":{"accounts_total":1,"subscribers_active":1,"connected":0},
        "fleet":{"exits_online":0,"exits_total":0,"download_bps":0,"upload_bps":0,
        "capacity_bps":0,"load_percent":0,"transferred_24h_bytes":0,
        "peak_connected_24h":0,"peak_throughput_24h_bps":0},"exits":[],"history":[]}"#;

    fn snapshot(generated_at: u64, window_secs: u64) -> StatsOutcome {
        StatsOutcome::Snapshot(StatsSnapshot {
            json: "{}".to_owned(),
            generated_at,
            window_secs,
        })
    }

    #[test]
    fn endpoint_url_joins_with_and_without_trailing_slash() {
        assert_eq!(
            endpoint_url("https://api.beta.warrenbrowse.com/"),
            "https://api.beta.warrenbrowse.com/v1/network/stats"
        );
        assert_eq!(
            endpoint_url("https://api.beta.warrenbrowse.com"),
            "https://api.beta.warrenbrowse.com/v1/network/stats"
        );
    }

    #[test]
    fn classify_passes_a_valid_snapshot_through_verbatim() {
        let outcome = classify(200, FIXTURE.as_bytes()).expect("the fixture is valid");
        assert_eq!(
            outcome,
            StatsOutcome::Snapshot(StatsSnapshot {
                json: FIXTURE.to_owned(),
                generated_at: 1_790_000_000,
                window_secs: 60,
            })
        );
    }

    #[test]
    fn classify_maps_404_to_unsupported() {
        assert_eq!(
            classify(404, b"not found").expect("404 is a stable answer"),
            StatsOutcome::Unsupported
        );
    }

    #[test]
    fn classify_rejects_other_statuses_as_transient() {
        assert!(matches!(
            classify(503, b"busy"),
            Err(StatsError::Status(503))
        ));
    }

    #[test]
    fn sanity_check_rejects_a_body_that_is_not_json() {
        assert!(matches!(
            sanity_check(b"<html>captive portal</html>"),
            Err(StatsError::InvalidBody(_))
        ));
    }

    #[test]
    fn sanity_check_rejects_json_that_is_not_an_object() {
        assert!(matches!(
            sanity_check(b"[1,2,3]"),
            Err(StatsError::InvalidBody(_))
        ));
    }

    #[test]
    fn sanity_check_rejects_another_schema_version() {
        let body = FIXTURE.replace(r#""version":1"#, r#""version":2"#);
        assert!(matches!(
            sanity_check(body.as_bytes()),
            Err(StatsError::InvalidBody(_))
        ));
    }

    #[test]
    fn sanity_check_rejects_a_snapshot_without_its_window() {
        let body = FIXTURE.replace(r#""window_secs":60,"#, "");
        assert!(matches!(
            sanity_check(body.as_bytes()),
            Err(StatsError::InvalidBody(_))
        ));
    }

    #[test]
    fn sanity_check_rejects_an_oversized_body() {
        let body = format!(
            r#"{{"version":1,"generated_at":1,"window_secs":60,"pad":"{}"}}"#,
            "x".repeat(MAX_BODY_BYTES)
        );
        assert!(matches!(
            sanity_check(body.as_bytes()),
            Err(StatsError::InvalidBody(_))
        ));
    }

    #[test]
    fn a_snapshot_is_fresh_until_its_window_closes() {
        let fetched_at = Instant::now();
        let cached = CachedOutcome {
            outcome: snapshot(1_000, 60),
            fetched_at,
        };
        let later = fetched_at + Duration::from_secs(30);
        assert!(cached.is_fresh(later, 1_059));
        assert!(!cached.is_fresh(later, 1_060));
    }

    #[test]
    fn a_snapshot_is_kept_for_the_minimum_refetch_interval_whatever_the_clock() {
        // Local clock far ahead of the server's: without the floor every
        // frontend poll would become a request.
        let fetched_at = Instant::now();
        let cached = CachedOutcome {
            outcome: snapshot(1_000, 60),
            fetched_at,
        };
        assert!(cached.is_fresh(fetched_at + Duration::from_secs(5), 99_999));
        assert!(!cached.is_fresh(fetched_at + MIN_REFETCH, 99_999));
    }

    #[test]
    fn a_snapshot_expires_after_one_window_even_with_a_clock_behind() {
        // Local clock far behind: `generated_at + window` would never pass.
        let fetched_at = Instant::now();
        let cached = CachedOutcome {
            outcome: snapshot(1_000, 60),
            fetched_at,
        };
        assert!(!cached.is_fresh(fetched_at + Duration::from_secs(60), 0));
    }

    #[test]
    fn the_window_used_for_caching_is_clamped_to_the_server_range() {
        let fetched_at = Instant::now();
        let cached = CachedOutcome {
            outcome: snapshot(1_000, 1_000_000),
            fetched_at,
        };
        let after_max = fetched_at + Duration::from_secs(MAX_WINDOW_SECS);
        assert!(!cached.is_fresh(after_max, 1_000));
    }

    #[test]
    fn an_unsupported_answer_is_believed_for_its_ttl() {
        let fetched_at = Instant::now();
        let cached = CachedOutcome {
            outcome: StatsOutcome::Unsupported,
            fetched_at,
        };
        assert!(cached.is_fresh(fetched_at + UNSUPPORTED_TTL - Duration::from_secs(1), 0));
        assert!(!cached.is_fresh(fetched_at + UNSUPPORTED_TTL, 0));
    }

    fn counting_fetch(
        calls: &AtomicUsize,
        outcome: Result<StatsOutcome, StatsError>,
    ) -> impl FnOnce() -> std::future::Ready<Result<StatsOutcome, StatsError>> + '_ {
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(outcome)
        }
    }

    #[tokio::test]
    async fn get_serves_the_cached_snapshot_while_its_window_is_open() {
        let stats = WarrenNetworkStats::new();
        let calls = AtomicUsize::new(0);
        let t0 = Instant::now();

        let first = stats
            .get_with(t0, 1_000, counting_fetch(&calls, Ok(snapshot(1_000, 60))))
            .await
            .expect("fetched");
        let second = stats
            .get_with(
                t0 + Duration::from_secs(30),
                1_030,
                counting_fetch(&calls, Ok(snapshot(1_060, 60))),
            )
            .await
            .expect("cached");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn get_fetches_again_once_the_window_has_closed() {
        let stats = WarrenNetworkStats::new();
        let calls = AtomicUsize::new(0);
        let t0 = Instant::now();

        stats
            .get_with(t0, 1_000, counting_fetch(&calls, Ok(snapshot(1_000, 60))))
            .await
            .expect("fetched");
        let second = stats
            .get_with(
                t0 + Duration::from_secs(60),
                1_060,
                counting_fetch(&calls, Ok(snapshot(1_060, 60))),
            )
            .await
            .expect("refetched");

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(second, snapshot(1_060, 60));
    }

    #[tokio::test]
    async fn get_does_not_cache_a_failure() {
        let stats = WarrenNetworkStats::new();
        let calls = AtomicUsize::new(0);
        let t0 = Instant::now();

        let failed = stats
            .get_with(
                t0,
                1_000,
                counting_fetch(&calls, Err(StatsError::Status(502))),
            )
            .await;
        let retried = stats
            .get_with(
                t0 + Duration::from_secs(1),
                1_001,
                counting_fetch(&calls, Ok(snapshot(1_000, 60))),
            )
            .await;

        assert!(failed.is_err());
        assert_eq!(retried.expect("second attempt"), snapshot(1_000, 60));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    fn relay(seed: u8) -> WarrenRelay {
        let key = SigningKey::from_bytes(&[seed; 32]);
        WarrenRelay::from_public(
            WarrenPubkey::from_bytes(key.verifying_key().to_bytes()),
            ExitId::from_bytes([seed; 16]),
            Location::new("FR", "Paris"),
            100,
            true,
            vec![Ingress::new(
                Addr::new("10.0.0.1".parse().expect("literal address"), None),
                vec![Listener::new(443, "quic", "h3")],
            )],
            true,
            false,
        )
    }

    #[test]
    fn the_join_names_each_exit_by_its_id_and_its_relay_list_hostname() {
        let list = WarrenRelayList::new(vec![relay(0xab), relay(0xcd)]);
        let view = crate::warren_relay_list_view::to_mullvad_relay_list(&list);
        let view_hostnames: Vec<String> = view.countries[0].cities[0]
            .relays
            .iter()
            .map(|relay| relay.hostname.clone())
            .collect();

        let join = exit_hostnames_of(&list);

        assert_eq!(join.len(), 2);
        assert_eq!(join[0].exit_id, "ab".repeat(16));
        assert_eq!(join[1].exit_id, "cd".repeat(16));
        assert!(view_hostnames.contains(&join[0].hostname));
        assert!(view_hostnames.contains(&join[1].hostname));
        assert_ne!(join[0].hostname, join[1].hostname);
    }

    #[test]
    fn set_relay_list_replaces_the_join() {
        let stats = WarrenNetworkStats::new();
        stats.set_relay_list(&WarrenRelayList::new(vec![relay(1), relay(2)]));
        stats.set_relay_list(&WarrenRelayList::new(vec![relay(3)]));

        let join = stats.exit_hostnames();

        assert_eq!(join.len(), 1);
        assert_eq!(join[0].exit_id, "03".repeat(16));
    }
}
