//! The signed multi-hop directory, fetched with the daemon's cadence instead
//! of once per dial.
//!
//! The desktop daemon refetches its signed lists only once they are an hour
//! old (`mullvad-daemon/src/warren_relay_list_updater.rs`, `UPDATE_INTERVAL`)
//! and serves the last good copy in between. The Android connect flow fetched
//! the directory on every dial, so an exit switch paid the round trip for a
//! blob the engine had verified seconds earlier. The engine still verifies the
//! signature and the `expires_at` of every blob it dials with
//! (`tunnel::run_multi_hop_session`), so a cached copy carries no trust of its
//! own; the cache only decides whether a fetch is worth issuing.

use parking_lot::Mutex;
use warren_api::{HttpTransport, WarrenApiClient};

/// The daemon's `UPDATE_INTERVAL`: a copy older than this is refetched.
const STALE_AFTER_SECS: u64 = 60 * 60;

/// A copy is handed out only if it stays valid past the dial it feeds: the
/// engine rejects a directory once `now >= expires_at`, and the dial follows
/// the config build by at most a few seconds.
const EXPIRY_MARGIN_SECS: u64 = 60;

struct Entry {
    raw: String,
    expires_at: u64,
    fetched_at: u64,
}

impl Entry {
    fn valid_at(&self, now_unix: u64) -> bool {
        now_unix.saturating_add(EXPIRY_MARGIN_SECS) < self.expires_at
    }

    /// Younger than the staleness bound. A clock that went backwards reads as
    /// stale, the daemon's own rule (`should_update` forces a refresh on a
    /// negative age), so a skewed device resyncs instead of trusting a copy
    /// whose age it cannot know.
    fn fresh_at(&self, now_unix: u64) -> bool {
        now_unix
            .checked_sub(self.fetched_at)
            .is_some_and(|age| age < STALE_AFTER_SECS)
            && self.valid_at(now_unix)
    }
}

/// Process-wide cache of the last good directory blob.
pub(crate) struct DirectoryCache {
    entry: Mutex<Option<Entry>>,
}

impl DirectoryCache {
    pub(crate) const fn new() -> Self {
        Self {
            entry: Mutex::new(None),
        }
    }

    /// The cached copy while it is younger than the staleness bound and
    /// stays valid past the dial it feeds.
    pub(crate) fn fresh(&self, now_unix: u64) -> Option<String> {
        self.entry
            .lock()
            .as_ref()
            .filter(|e| e.fresh_at(now_unix))
            .map(|e| e.raw.clone())
    }

    /// The cached copy as long as its signed validity holds, whatever its
    /// age: what a failed refetch falls back to, the way the daemon keeps
    /// serving its last good list through an outage.
    pub(crate) fn unexpired(&self, now_unix: u64) -> Option<String> {
        self.entry
            .lock()
            .as_ref()
            .filter(|e| e.valid_at(now_unix))
            .map(|e| e.raw.clone())
    }

    /// Records a fetched directory, keyed on the `expires_at` it carries.
    /// Returns false, and caches nothing, when that field cannot be read: the
    /// engine rejects such a blob at dial time, so remembering it would only
    /// repeat the rejection for an hour.
    pub(crate) fn store(&self, raw: &str, now_unix: u64) -> bool {
        let Some(expires_at) = expires_at_of(raw) else {
            return false;
        };
        *self.entry.lock() = Some(Entry {
            raw: raw.to_owned(),
            expires_at,
            fetched_at: now_unix,
        });
        true
    }

    /// The directory for a dial: the fresh copy, else one fetch, else the
    /// unexpired copy. `None` when nothing usable exists, which the caller
    /// turns into an empty blob (the connect flow fails closed on it).
    pub(crate) async fn fetch_or_cached<T: HttpTransport>(
        &self,
        client: &WarrenApiClient<T>,
        now_unix: u64,
    ) -> Option<String> {
        if let Some(raw) = self.fresh(now_unix) {
            return Some(raw);
        }
        match client.fetch_multihop_directory().await {
            Ok(Some(raw)) => {
                if !self.store(&raw, now_unix) {
                    log::warn!("multihop directory carries no readable expires_at; not cached");
                }
                Some(raw)
            }
            Ok(None) => {
                log::warn!("fetchMultihopDirectory: no directory published (404)");
                self.unexpired(now_unix)
            }
            Err(e) => {
                log::warn!("fetchMultihopDirectory: fetch failed: {e}");
                self.unexpired(now_unix)
            }
        }
    }
}

/// The signed `expires_at` of a directory blob, read without verifying it:
/// verification is the engine's job at dial time, the cache only needs the
/// validity window.
fn expires_at_of(raw: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()?
        .get("expires_at")?
        .as_u64()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use parking_lot::Mutex;
    use warren_api::WarrenApiClient;
    use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
    use warren_identity::WarrenIdentity;

    use super::DirectoryCache;

    const NOW: u64 = 1_700_000_000;

    enum Answer {
        Body(String),
        NotFound,
        Down,
    }

    struct State {
        calls: AtomicUsize,
        answer: Mutex<Answer>,
    }

    /// The HTTP boundary, counted: the cache is only right if the count is.
    #[derive(Clone)]
    struct CountingTransport(Arc<State>);

    impl CountingTransport {
        fn answering(body: String) -> Self {
            Self(Arc::new(State {
                calls: AtomicUsize::new(0),
                answer: Mutex::new(Answer::Body(body)),
            }))
        }

        fn calls(&self) -> usize {
            self.0.calls.load(Ordering::SeqCst)
        }

        fn set(&self, answer: Answer) {
            *self.0.answer.lock() = answer;
        }
    }

    impl HttpTransport for CountingTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            assert!(
                request.url.ends_with("/v1/multihop/directory"),
                "{}",
                request.url
            );
            self.0.calls.fetch_add(1, Ordering::SeqCst);
            match &*self.0.answer.lock() {
                Answer::Body(body) => Ok(HttpResponse {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                }),
                Answer::NotFound => Ok(HttpResponse {
                    status: 404,
                    body: Vec::new(),
                }),
                // An `Io` failure, the class a request dying in its timeout
                // reports; a `Connect` one would make the SDK retry the other
                // hosts and the no-SNI path, which is its own contract.
                Answer::Down => Err(TransportError::Io("request timed out".to_owned())),
            }
        }
    }

    fn directory(expires_at: u64) -> String {
        format!(
            r#"{{"version":2,"nodes":[],"generation":1,"signed_at":{NOW},"expires_at":{expires_at}}}"#
        )
    }

    fn client(transport: &CountingTransport) -> WarrenApiClient<CountingTransport> {
        WarrenApiClient::new(
            "https://api.example.test",
            WarrenIdentity::from_seed(&[0x11; 32]),
            transport.clone(),
        )
    }

    #[tokio::test]
    async fn a_second_dial_within_the_hour_reuses_the_directory() {
        let transport = CountingTransport::answering(directory(NOW + 6 * 3600));
        let client = client(&transport);
        let cache = DirectoryCache::new();

        let first = cache.fetch_or_cached(&client, NOW).await;
        let second = cache.fetch_or_cached(&client, NOW + 3599).await;

        assert_eq!(
            transport.calls(),
            1,
            "one fetch per hour, as the daemon does"
        );
        assert_eq!(first, second);
        assert!(first.is_some());
    }

    #[tokio::test]
    async fn an_hour_old_directory_is_refetched() {
        let transport = CountingTransport::answering(directory(NOW + 6 * 3600));
        let client = client(&transport);
        let cache = DirectoryCache::new();

        cache.fetch_or_cached(&client, NOW).await;
        cache.fetch_or_cached(&client, NOW + 3600).await;

        assert_eq!(
            transport.calls(),
            2,
            "the daemon's staleness bound is one hour"
        );
    }

    #[tokio::test]
    async fn a_directory_about_to_expire_is_refetched() {
        // Fresh by age, but the engine would reject it by the time the dial
        // happens: the signed validity is the key, not the fetch time.
        let transport = CountingTransport::answering(directory(NOW + 30));
        let client = client(&transport);
        let cache = DirectoryCache::new();

        cache.fetch_or_cached(&client, NOW).await;
        cache.fetch_or_cached(&client, NOW + 1).await;

        assert_eq!(transport.calls(), 2);
    }

    #[tokio::test]
    async fn a_failed_refetch_falls_back_to_the_unexpired_copy() {
        let transport = CountingTransport::answering(directory(NOW + 6 * 3600));
        let client = client(&transport);
        let cache = DirectoryCache::new();
        let first = cache.fetch_or_cached(&client, NOW).await;

        transport.set(Answer::Down);
        let later = cache.fetch_or_cached(&client, NOW + 3600).await;

        assert_eq!(transport.calls(), 2, "the refetch was attempted");
        assert_eq!(
            later, first,
            "the last good copy is served through the outage"
        );
    }

    #[tokio::test]
    async fn a_missing_directory_keeps_the_unexpired_copy() {
        let transport = CountingTransport::answering(directory(NOW + 6 * 3600));
        let client = client(&transport);
        let cache = DirectoryCache::new();
        let first = cache.fetch_or_cached(&client, NOW).await;

        transport.set(Answer::NotFound);
        let later = cache.fetch_or_cached(&client, NOW + 3600).await;

        assert_eq!(later, first);
    }

    #[tokio::test]
    async fn a_failed_refetch_of_an_expired_copy_yields_nothing() {
        let transport = CountingTransport::answering(directory(NOW + 3700));
        let client = client(&transport);
        let cache = DirectoryCache::new();
        cache.fetch_or_cached(&client, NOW).await;

        transport.set(Answer::Down);
        let later = cache.fetch_or_cached(&client, NOW + 3700).await;

        assert_eq!(
            later, None,
            "an expired copy would only be rejected at dial time"
        );
    }

    #[tokio::test]
    async fn a_blob_without_expiry_is_handed_out_but_not_cached() {
        let transport = CountingTransport::answering("{}".to_owned());
        let client = client(&transport);
        let cache = DirectoryCache::new();

        let first = cache.fetch_or_cached(&client, NOW).await;
        cache.fetch_or_cached(&client, NOW + 1).await;

        assert_eq!(
            first.as_deref(),
            Some("{}"),
            "the engine decides what to make of it"
        );
        assert_eq!(
            transport.calls(),
            2,
            "nothing without a validity window is remembered"
        );
    }

    #[tokio::test]
    async fn a_clock_that_went_backwards_refetches() {
        let transport = CountingTransport::answering(directory(NOW + 6 * 3600));
        let client = client(&transport);
        let cache = DirectoryCache::new();

        cache.fetch_or_cached(&client, NOW).await;
        cache.fetch_or_cached(&client, NOW - 10).await;

        assert_eq!(
            transport.calls(),
            2,
            "an unknowable age is stale, as for the daemon"
        );
    }
}
