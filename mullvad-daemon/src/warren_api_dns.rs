//! DNS for the daemon's own Warren fetchers, so they survive the kill switch
//! they exist to lift.
//!
//! In the blocking error state the firewall drops DNS host-wide and permits
//! exactly one thing: the API endpoint, by IP, built in [`crate::api`] from
//! `mullvad-api`'s address cache. A fetcher that asks the system resolver for
//! the API host therefore dies at the moment it is the way out: the multi-hop
//! directory refresh, the v7 token top-up and the version check all reported
//! "Temporary failure in name resolution" during a measured blackout.
//!
//! This resolver answers the API host from that same address cache, so the
//! address the firewall permits is the address the fetch dials, and hands
//! every other name to the system resolver untouched. TLS is unchanged: the
//! request still carries the hostname, so SNI and certificate verification
//! bind to the API host exactly as before.
//!
//! The cache holds the address this daemon resolved at start, before the
//! firewall was armed, or the one a previous daemon persisted when this start
//! resolved nothing (`mullvad_api::AddressCache::resolved_or_persisted`),
//! which is what keeps a daemon restarted behind its own kill switch able to
//! reach the API at all. When it holds nothing usable, the system resolver
//! stays the only option and is used as the fallback.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use mullvad_api::{AddressCache, FileAddressCacheBacking};

/// Snapshot of the daemon's last known good API address, read on demand so a
/// later cache update is picked up without rebuilding any HTTP client.
pub(crate) type ApiAddressSource =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<SocketAddr>> + Send>> + Send + Sync>;

/// Where one resolution request is answered from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// The daemon's cached API address, which the firewall permits by IP.
    Cached(SocketAddr),
    /// The system resolver: any host that is not the API, or an API host the
    /// daemon has no usable address for yet.
    SystemDns,
}

/// Resolution policy. Pure, so the whole decision is testable without a
/// resolver, a cache or a network.
fn route(name: &str, api_host: &str, cached: Option<SocketAddr>) -> Route {
    if !name.eq_ignore_ascii_case(api_host) {
        return Route::SystemDns;
    }
    match cached {
        Some(addr) if mullvad_api::is_dialable(addr) => Route::Cached(addr),
        _ => Route::SystemDns,
    }
}

struct Inner {
    api_host: String,
    address: ApiAddressSource,
    announced_cached: AtomicBool,
    announced_fallback: AtomicBool,
}

impl Inner {
    /// Logs the path taken the first time each one is taken, so the choice is
    /// visible in a daemon log without one line per request.
    fn announce(&self, route: Route) {
        match route {
            Route::Cached(_) => {
                if !self.announced_cached.swap(true, Ordering::Relaxed) {
                    log::info!(
                        "Warren API DNS: answering {host} from the daemon address cache, so a \
                         fetch still reaches the API while the firewall blocks DNS",
                        host = self.api_host,
                    );
                }
            }
            Route::SystemDns => {
                if !self.announced_fallback.swap(true, Ordering::Relaxed) {
                    log::warn!(
                        "Warren API DNS: no cached address for {host}, falling back to the \
                         system resolver; a fetch during the blocking state will fail",
                        host = self.api_host,
                    );
                }
            }
        }
    }
}

/// A [`reqwest::dns::Resolve`] that answers the API host from the daemon's
/// address cache and delegates everything else to the system resolver.
#[derive(Clone)]
pub(crate) struct ApiHostResolver {
    inner: Arc<Inner>,
}

impl ApiHostResolver {
    /// `api_host` is the host the address cache was built for
    /// (`ApiEndpoint::host`), so an operator who overrides the API URL to a
    /// different host keeps ordinary DNS for it.
    pub(crate) fn new(api_host: String, address: ApiAddressSource) -> Self {
        Self {
            inner: Arc::new(Inner {
                api_host,
                address,
                announced_cached: AtomicBool::new(false),
                announced_fallback: AtomicBool::new(false),
            }),
        }
    }

    /// The production source: the daemon's own `mullvad-api` address cache,
    /// the very cache [`crate::api::resolve_allowed_endpoint`] builds the
    /// firewall's permitted endpoint from.
    pub(crate) fn from_address_cache(
        api_host: String,
        cache: AddressCache<FileAddressCacheBacking>,
    ) -> Self {
        Self::new(
            api_host,
            Arc::new(move || {
                let cache = cache.clone();
                Box::pin(async move { Some(cache.get_address().await) })
            }),
        )
    }
}

impl reqwest::dns::Resolve for ApiHostResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let inner = Arc::clone(&self.inner);
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let cached = if host.eq_ignore_ascii_case(&inner.api_host) {
                (inner.address)().await
            } else {
                None
            };
            match route(&host, &inner.api_host, cached) {
                Route::Cached(addr) => {
                    inner.announce(Route::Cached(addr));
                    Ok(Box::new(std::iter::once(addr)) as reqwest::dns::Addrs)
                }
                Route::SystemDns => {
                    if host.eq_ignore_ascii_case(&inner.api_host) {
                        inner.announce(Route::SystemDns);
                    }
                    system_lookup(host).await
                }
            }
        })
    }
}

/// The system resolver, port left at 0 the way reqwest's own default resolver
/// leaves it (the URL's port is applied afterwards).
async fn system_lookup(
    host: String,
) -> Result<reqwest::dns::Addrs, Box<dyn std::error::Error + Send + Sync>> {
    let addrs = tokio::net::lookup_host((host.as_str(), 0))
        .await?
        .collect::<Vec<_>>();
    Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
}

/// The process-wide resolver, installed once by `Daemon::start`. A global
/// because the fetchers that need it are reached from deep, synchronous call
/// sites (the tunnel parameter generator mints a token provider per attempt)
/// where threading a handle down would carry it through half the daemon.
static INSTALLED: OnceLock<ApiHostResolver> = OnceLock::new();

/// Installs the process-wide resolver. Later calls are ignored: the daemon
/// starts one API endpoint per process.
pub(crate) fn install(resolver: ApiHostResolver) {
    let _ = INSTALLED.set(resolver);
}

/// The installed resolver, or `None` when nothing installed one (unit tests,
/// tools, a build path that never starts a daemon), where the plain system
/// resolver is the correct behaviour.
pub(crate) fn resolver() -> Option<Arc<dyn reqwest::dns::Resolve>> {
    INSTALLED
        .get()
        .map(|r| Arc::new(r.clone()) as Arc<dyn reqwest::dns::Resolve>)
}

/// Points `builder` at the installed API resolver. A no-op when none is
/// installed, so every caller keeps working outside a running daemon.
pub(crate) fn with_api_resolver(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    match resolver() {
        Some(resolver) => builder.dns_resolver2(resolver),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const API_HOST: &str = "api.beta.warrenbrowse.test";
    const CACHED: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 7)),
        443,
    );

    fn fixed_source(addr: Option<SocketAddr>) -> ApiAddressSource {
        Arc::new(move || Box::pin(async move { addr }))
    }

    #[test]
    fn api_host_is_answered_from_the_cache() {
        assert_eq!(
            route(API_HOST, API_HOST, Some(CACHED)),
            Route::Cached(CACHED)
        );
    }

    #[test]
    fn api_host_match_ignores_case() {
        assert_eq!(
            route("API.Beta.WarrenBrowse.Test", API_HOST, Some(CACHED)),
            Route::Cached(CACHED),
            "DNS names are case insensitive; a mixed-case host must still hit the cache"
        );
    }

    #[test]
    fn any_other_host_goes_to_system_dns() {
        // The guard that keeps an overridden API URL, and every unrelated
        // host, on ordinary DNS.
        assert_eq!(
            route("example.test", API_HOST, Some(CACHED)),
            Route::SystemDns
        );
    }

    #[test]
    fn an_unresolved_cache_falls_back_to_system_dns() {
        // `API_IP_DEFAULT` is the unspecified sentinel: the daemon never
        // resolved the API, so there is nothing to dial and DNS is the only
        // remaining option.
        let sentinel = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 443);
        assert_eq!(route(API_HOST, API_HOST, Some(sentinel)), Route::SystemDns);
        assert_eq!(route(API_HOST, API_HOST, None), Route::SystemDns);
    }

    #[test]
    fn a_zero_port_is_not_dialable() {
        let no_port = SocketAddr::new(CACHED.ip(), 0);
        assert_eq!(route(API_HOST, API_HOST, Some(no_port)), Route::SystemDns);
    }

    #[tokio::test]
    async fn resolve_returns_the_cached_address_for_the_api_host() {
        use reqwest::dns::Resolve;

        let resolver = ApiHostResolver::new(API_HOST.to_owned(), fixed_source(Some(CACHED)));
        let addrs: Vec<SocketAddr> = resolver
            .resolve(reqwest::dns::Name::from_str(API_HOST).unwrap())
            .await
            .expect("resolution must succeed from the cache alone")
            .collect();
        assert_eq!(addrs, vec![CACHED]);
    }

    /// The wiring test: a client built through [`ApiHostResolver`] must reach
    /// a server whose host name no resolver on earth can answer. `.test` is
    /// reserved by RFC 6761 and never resolves, so a response here can only
    /// have come through the cached address.
    #[tokio::test]
    async fn a_client_reaches_the_cached_address_for_an_unresolvable_api_host() {
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("GET", "/v1/probe").with_status(200).create();
        let addr: SocketAddr = server.host_with_port().parse().expect("mockito addr");

        let resolver = ApiHostResolver::new(API_HOST.to_owned(), fixed_source(Some(addr)));
        let client = reqwest::Client::builder()
            .dns_resolver2(Arc::new(resolver) as Arc<dyn reqwest::dns::Resolve>)
            .build()
            .unwrap();

        let status = client
            .get(format!("http://{API_HOST}:{}/v1/probe", addr.port()))
            .send()
            .await
            .expect("the cached address must be dialed without any DNS query")
            .status();

        assert_eq!(status, 200);
        mock.assert();
    }

    /// The honest fallback: with nothing cached, the same unresolvable host
    /// must go to the system resolver and fail there, never silently reuse
    /// some other address.
    #[tokio::test]
    async fn a_client_without_a_cached_address_falls_back_to_system_dns() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/probe")
            .with_status(200)
            .expect(0)
            .create();
        let addr: SocketAddr = server.host_with_port().parse().expect("mockito addr");

        let resolver = ApiHostResolver::new(API_HOST.to_owned(), fixed_source(None));
        let client = reqwest::Client::builder()
            .dns_resolver2(Arc::new(resolver) as Arc<dyn reqwest::dns::Resolve>)
            .build()
            .unwrap();

        let result = client
            .get(format!("http://{API_HOST}:{}/v1/probe", addr.port()))
            .send()
            .await;

        assert!(
            result.is_err(),
            "an unresolvable host with no cached address must fail in the system resolver"
        );
        mock.assert();
    }

    /// The host guard, end to end: a cached API address must never be used
    /// for a different host.
    #[tokio::test]
    async fn a_client_never_uses_the_cached_address_for_another_host() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/probe")
            .with_status(200)
            .expect(0)
            .create();
        let addr: SocketAddr = server.host_with_port().parse().expect("mockito addr");

        let resolver = ApiHostResolver::new(API_HOST.to_owned(), fixed_source(Some(addr)));
        let client = reqwest::Client::builder()
            .dns_resolver2(Arc::new(resolver) as Arc<dyn reqwest::dns::Resolve>)
            .build()
            .unwrap();

        let result = client
            .get(format!("http://unrelated.invalid:{}/v1/probe", addr.port()))
            .send()
            .await;

        assert!(
            result.is_err(),
            "the cached API address must not answer for an unrelated host"
        );
        mock.assert();
    }

    #[test]
    fn no_resolver_is_installed_outside_a_running_daemon() {
        // `with_api_resolver` must stay a no-op for every caller that runs
        // without a daemon, so the fetchers keep working in tests and tools.
        assert!(resolver().is_none());
    }
}
