//! The daemon's HTTP transport for the signed warren-api clients.
//!
//! The SDK ships `warren_api::reqwest_transport::ReqwestTransport`, which
//! builds its `reqwest::Client` internally and so cannot be told how to
//! resolve. The daemon needs exactly that: its API host must resolve from the
//! address cache the firewall's allowed endpoint is built from, or the v7
//! token top-up and every account call die the moment the blocking state
//! drops system DNS (see [`crate::warren_api_dns`]).
//!
//! Behaviour otherwise matches the SDK transport it replaces: the same 5 s
//! connect / 15 s total budget, the same two clients so the SDK's
//! anti-censorship fallback can retry without SNI, and the same address-free
//! error mapping (the `reqwest` `Display` carries the URL and the resolved IP,
//! so it is never propagated).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use mullvad_types::states::TunnelState;
use talpid_types::tunnel::TunnelStateTransition;
use tokio::sync::watch;

use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, Method, TransportError};

/// Connect budget, mirroring the SDK transport.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Total request budget, mirroring the SDK transport.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

struct Clients {
    with_sni: reqwest::Client,
    without_sni: reqwest::Client,
}

/// Counts the times the tunnel's routes appeared or went away.
#[derive(Clone)]
pub(crate) struct RouteChanges(Arc<watch::Sender<u64>>);

impl RouteChanges {
    pub(crate) fn new() -> Self {
        Self(Arc::new(watch::Sender::new(0)))
    }

    /// Records that the routes changed.
    pub(crate) fn changed(&self) {
        self.0.send_modify(|count| *count = count.wrapping_add(1));
    }

    fn current(&self) -> u64 {
        *self.0.borrow()
    }
}

/// Whether going from `previous` to `next` may move the host's routes: the
/// tunnel takes the default route while it connects and gives it back when a
/// disconnect ends or a connection attempt fails. Only a connected tunnel
/// reporting a new MTU or leg count, and the start of a disconnect (the
/// routes stay until it ends), move nothing.
pub(crate) fn routes_move(previous: &TunnelState, next: &TunnelStateTransition) -> bool {
    match next {
        TunnelStateTransition::Connected(_) => !previous.is_connected(),
        TunnelStateTransition::Disconnecting(_) => false,
        _ => true,
    }
}

/// The daemon's route changes, recorded by its tunnel state handling.
pub(crate) fn route_changes() -> &'static RouteChanges {
    static ROUTES: std::sync::OnceLock<RouteChanges> = std::sync::OnceLock::new();
    ROUTES.get_or_init(RouteChanges::new)
}

/// Shared, cheap to clone: every clone reuses the same connection pool, so a
/// per-call `WarrenApiClient` rebuild costs no extra TLS handshake.
#[derive(Clone)]
pub(crate) struct WarrenApiTransport {
    resolver: Option<Arc<dyn reqwest::dns::Resolve>>,
    routes: RouteChanges,
    clients: Arc<Mutex<(u64, Arc<Clients>)>>,
}

impl WarrenApiTransport {
    /// The daemon transport, resolving the API host through the installed
    /// resolver when a daemon installed one.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::with_routes(crate::warren_api_dns::resolver(), route_changes().clone())
    }

    #[cfg(test)]
    fn with_resolver(resolver: Option<Arc<dyn reqwest::dns::Resolve>>) -> Self {
        Self::with_routes(resolver, RouteChanges::new())
    }

    fn with_routes(resolver: Option<Arc<dyn reqwest::dns::Resolve>>, routes: RouteChanges) -> Self {
        let epoch = routes.current();
        let clients = Arc::new(Mutex::new((
            epoch,
            Arc::new(build_clients(resolver.as_ref())),
        )));
        Self {
            resolver,
            routes,
            clients,
        }
    }

    /// The connection pool of the current routes: a connection kept alive
    /// from before a route change sits on a path that no longer exists.
    fn clients(&self) -> Arc<Clients> {
        let epoch = self.routes.current();
        let mut guard = self.clients.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.0 != epoch {
            *guard = (epoch, Arc::new(build_clients(self.resolver.as_ref())));
        }
        Arc::clone(&guard.1)
    }
}

/// How many times a request is sent again because the routes moved while it
/// failed to connect: once for the tunnel taking the route, once more for a
/// reconnect landing right after.
const RESENDS_ON_ROUTE_CHANGE: usize = 2;

/// Runs `send` and, when an attempt failed to connect while the routes moved,
/// runs it again on the routes of now. Only a failure to connect is sent
/// again: the request then never reached the API, so its signature (the SDK
/// signs before the transport sees the request) is not replayed, which the
/// API would refuse. The SDK's own host fallback re-sends the same signed
/// request on connect failures for the same reason.
async fn send_following_routes<F, Fut>(
    routes: &RouteChanges,
    mut send: F,
) -> Result<HttpResponse, TransportError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<HttpResponse, TransportError>>,
{
    let mut resends = 0;
    loop {
        let before = routes.current();
        let result = send().await;
        match result {
            Err(error)
                if error.is_connect()
                    && routes.current() != before
                    && resends < RESENDS_ON_ROUTE_CHANGE =>
            {
                resends += 1;
                log::debug!(
                    "Warren API: a request could not connect while the routes moved; \
                     sending it again"
                );
            }
            other => return other,
        }
    }
}

fn build_clients(resolver: Option<&Arc<dyn reqwest::dns::Resolve>>) -> Clients {
    {
        let build = |sni: bool| {
            let mut builder = reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(TOTAL_TIMEOUT);
            if let Some(resolver) = resolver.cloned() {
                builder = builder.dns_resolver2(resolver);
            }
            crate::warren_tls::configure(builder, sni)
                .build()
                .expect("reqwest client build failed: invalid TLS backend configuration")
        };
        Clients {
            with_sni: build(true),
            without_sni: build(false),
        }
    }
}

impl Clients {
    /// The client honouring `use_sni`. The SDK's fallback sequence retries its
    /// last attempt with SNI off to defeat SNI-based blocking, so the two
    /// clients must stay distinct.
    fn client_for(&self, use_sni: bool) -> &reqwest::Client {
        if use_sni {
            &self.with_sni
        } else {
            &self.without_sni
        }
    }
}

fn to_reqwest_method(method: Method) -> reqwest::Method {
    match method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Delete => reqwest::Method::DELETE,
    }
}

/// Classifies a `reqwest` failure. Connect-establishment failures drive the
/// SDK's host fallback; everything else is terminal for that attempt. The
/// `reqwest` error is deliberately not propagated: its `Display` carries the
/// URL and the resolved IP.
fn to_transport_error(e: &reqwest::Error) -> TransportError {
    if e.is_connect() {
        TransportError::Connect("connection failed".to_owned())
    } else if e.is_timeout() {
        TransportError::Io("request timed out".to_owned())
    } else {
        TransportError::Io("request failed".to_owned())
    }
}

impl HttpTransport for WarrenApiTransport {
    /// A request whose connection could not be established while the tunnel
    /// took or gave back the route is sent again on the routes of now
    /// ([`send_following_routes`]). Its packets kept the source address of the
    /// path they started on and went unanswered: the first token refresh of a
    /// fresh daemon, issued as its first tunnel was connecting, died that way
    /// (`all API hosts are unreachable`), since the SDK's only fallback, the
    /// same host without SNI, is refused by the API.
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        send_following_routes(&self.routes, || {
            let clients = self.clients();
            let request = request.clone();
            async move { send(&clients, request).await }
        })
        .await
    }
}

async fn send(clients: &Clients, request: HttpRequest) -> Result<HttpResponse, TransportError> {
    {
        let mut builder = clients
            .client_for(request.use_sni)
            .request(to_reqwest_method(request.method), &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if !request.body.is_empty() {
            builder = builder.body(request.body);
        }
        let resp = builder.send().await.map_err(|e| to_transport_error(&e))?;
        let status = resp.status().as_u16();
        // The SDK reads the server's clock off a refusal's `Date` and signs
        // again at it; without the header a drifted clock is refused forever.
        let date = resp
            .headers()
            .get(reqwest::header::DATE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = resp
            .bytes()
            .await
            .map_err(|e| to_transport_error(&e))?
            .to_vec();
        let response = HttpResponse::new(status, body);
        Ok(match date {
            Some(date) => response.with_date(date),
            None => response,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warren_api_dns::{ApiAddressSource, ApiHostResolver};
    use std::net::SocketAddr;

    const API_HOST: &str = "api.beta.warrenbrowse.test";

    fn fixed_source(addr: SocketAddr) -> ApiAddressSource {
        Arc::new(move || Box::pin(async move { Some(addr) }))
    }

    fn resolver_for(addr: SocketAddr) -> Option<Arc<dyn reqwest::dns::Resolve>> {
        Some(Arc::new(ApiHostResolver::new(
            API_HOST.to_owned(),
            fixed_source(addr),
        )) as Arc<dyn reqwest::dns::Resolve>)
    }

    /// The point of the whole module: a signed API call must reach the cached
    /// address without a DNS query. `.test` is reserved by RFC 6761 and never
    /// resolves, so a 200 here can only have come through the resolver.
    #[tokio::test]
    async fn a_request_reaches_the_cached_address_without_dns() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/tokens")
            .match_header("x-warren-address", "wb-test")
            .match_body("payload")
            .with_status(201)
            .with_body("minted")
            .create();
        let addr: SocketAddr = server.host_with_port().parse().expect("mockito addr");

        let transport = WarrenApiTransport::with_resolver(resolver_for(addr));
        let response = transport
            .execute(HttpRequest {
                method: Method::Post,
                url: format!("http://{API_HOST}:{}/v1/tokens", addr.port()),
                headers: vec![("x-warren-address".to_owned(), "wb-test".to_owned())],
                body: b"payload".to_vec(),
                use_sni: true,
            })
            .await
            .expect("the cached address must be dialed without any DNS query");

        assert_eq!(response.status, 201);
        assert_eq!(response.body, b"minted");
        mock.assert();
    }

    /// The SDK learns the server's clock from a refusal's `Date` header and
    /// signs again at it: a transport that dropped the header would leave a
    /// device whose clock drifted refused on every signed call (forum topic
    /// 219, a Windows clock 91 s fast).
    #[tokio::test]
    async fn the_answers_date_header_reaches_the_sdk() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/v1/subscription")
            .with_status(401)
            .with_header("date", "Tue, 14 Nov 2023 22:13:20 GMT")
            .with_body(r#"{"error":"clock_skew"}"#)
            .create();
        let addr: SocketAddr = server.host_with_port().parse().expect("mockito addr");

        let transport = WarrenApiTransport::with_resolver(resolver_for(addr));
        let response = transport
            .execute(HttpRequest {
                method: Method::Get,
                url: format!("http://{API_HOST}:{}/v1/subscription", addr.port()),
                headers: vec![],
                body: vec![],
                use_sni: true,
            })
            .await
            .expect("answered");

        assert_eq!(
            response.date.as_deref(),
            Some("Tue, 14 Nov 2023 22:13:20 GMT")
        );
    }

    /// The SNI toggle must select a genuinely different client: collapsing the
    /// two would silently disable the SDK's no-SNI anti-censorship retry.
    /// Whether SNI leaves the wire is the SDK's own test, which needs TLS.
    #[test]
    fn the_sni_toggle_selects_a_distinct_client() {
        let transport = WarrenApiTransport::with_resolver(None);
        assert!(!std::ptr::eq(
            transport.clients().client_for(true),
            transport.clients().client_for(false)
        ));
    }

    fn answered() -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(200, b"ok".to_vec()))
    }

    /// Runs `send_following_routes` over attempts that end with `outcomes`,
    /// moving the routes during the first attempt when `routes_move`, and
    /// says how many attempts it made and what it returned.
    async fn attempts(
        outcomes: Vec<Result<HttpResponse, TransportError>>,
        routes_move: bool,
    ) -> (usize, Result<HttpResponse, TransportError>) {
        let routes = RouteChanges::new();
        let made = std::sync::atomic::AtomicUsize::new(0);
        let outcomes = Mutex::new(outcomes.into_iter());
        let result = send_following_routes(&routes, || {
            if made.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 && routes_move {
                routes.changed();
            }
            let outcome = outcomes
                .lock()
                .unwrap()
                .next()
                .expect("an outcome for every attempt");
            async move { outcome }
        })
        .await;
        (made.load(std::sync::atomic::Ordering::SeqCst), result)
    }

    fn connect_failed() -> Result<HttpResponse, TransportError> {
        Err(TransportError::Connect("connection failed".to_owned()))
    }

    /// The case of the first token refresh of a fresh daemon: its connection,
    /// opened on the physical interface as the tunnel connected, never got
    /// through once the tunnel took the route. It never reached the API, so
    /// sending it again, same signature included, replays nothing.
    #[tokio::test]
    async fn a_request_that_could_not_connect_while_the_routes_moved_is_sent_again() {
        let (made, result) = attempts(vec![connect_failed(), answered()], true).await;

        assert_eq!(made, 2);
        assert_eq!(result.expect("answered").body, b"ok");
    }

    #[tokio::test]
    async fn a_request_that_could_not_connect_on_steady_routes_is_left_to_the_sdk() {
        let (made, result) = attempts(vec![connect_failed()], false).await;

        assert_eq!(made, 1);
        assert!(result.expect_err("no second attempt").is_connect());
    }

    /// A request that connected may have reached the API, which refuses a
    /// signature it has already seen: sending it again would turn a success
    /// into a replay refusal.
    #[tokio::test]
    async fn a_request_that_connected_is_never_sent_again() {
        let lost = Err(TransportError::Io("request timed out".to_owned()));

        let (made, result) = attempts(vec![lost], true).await;

        assert_eq!(made, 1);
        assert!(!result.expect_err("the first outcome").is_connect());
    }

    /// Connections kept alive from before a route change are not reused
    /// after it.
    #[tokio::test]
    async fn a_route_change_retires_the_pooled_connections() {
        let routes = RouteChanges::new();
        let transport = WarrenApiTransport::with_routes(None, routes.clone());
        let pool_before = transport.clients();

        routes.changed();

        assert!(!Arc::ptr_eq(&pool_before, &transport.clients()));
    }

    fn endpoint() -> talpid_types::net::TunnelEndpoint {
        talpid_types::net::TunnelEndpoint {
            endpoint: talpid_types::net::Endpoint::new(
                std::net::Ipv4Addr::LOCALHOST,
                443,
                talpid_types::net::TransportProtocol::Udp,
            ),
            quantum_resistant: false,
            obfuscation: None,
            entry_endpoint: None,
            tunnel_interface: None,
            #[cfg(daita)]
            daita: false,
            effective_mtu: None,
            legs_bonded: 0,
            legs_not_delivering: 0,
            tunnel_type: talpid_types::net::TunnelType::Warren,
        }
    }

    fn connected() -> TunnelState {
        TunnelState::Connected {
            endpoint: endpoint(),
            location: None,
            feature_indicators: Default::default(),
        }
    }

    fn connecting() -> TunnelState {
        TunnelState::Connecting {
            endpoint: endpoint(),
            location: None,
            feature_indicators: Default::default(),
        }
    }

    #[test]
    fn the_routes_move_on_every_transition_but_a_refreshed_connection_and_a_disconnect_start() {
        use talpid_types::tunnel::ActionAfterDisconnect;
        let disconnecting = TunnelState::Disconnecting(ActionAfterDisconnect::Reconnect);

        assert!(routes_move(
            &connecting(),
            &TunnelStateTransition::Connected(endpoint())
        ));
        assert!(routes_move(
            &disconnecting,
            &TunnelStateTransition::Connecting(endpoint())
        ));
        assert!(
            routes_move(
                &connecting(),
                &TunnelStateTransition::Error(talpid_types::tunnel::ErrorState::new(
                    talpid_types::tunnel::ErrorStateCause::IsOffline,
                    None,
                ))
            ),
            "a tunnel that failed while connecting may have taken the route already"
        );
        assert!(!routes_move(
            &connected(),
            &TunnelStateTransition::Connected(endpoint())
        ));
        assert!(!routes_move(
            &connected(),
            &TunnelStateTransition::Disconnecting(ActionAfterDisconnect::Nothing)
        ));
    }

    /// No-log discipline: a connect failure must not carry the URL, the host
    /// or the resolved IP into the error the daemon logs.
    #[tokio::test]
    async fn a_connect_failure_is_reported_without_any_address() {
        // Port 1 on the loopback refuses immediately, so this is a connect
        // failure and not a timeout.
        let transport = WarrenApiTransport::with_resolver(None);
        let error = transport
            .execute(HttpRequest {
                method: Method::Get,
                url: "http://127.0.0.1:1/v1/subscription".to_owned(),
                headers: vec![],
                body: vec![],
                use_sni: true,
            })
            .await
            .expect_err("a refused connection must fail");

        assert!(error.is_connect(), "must drive the SDK host fallback");
        let rendered = error.to_string();
        assert!(!rendered.contains("127.0.0.1"), "leaked an address");
        assert!(!rendered.contains("/v1/subscription"), "leaked the URL");
    }
}
