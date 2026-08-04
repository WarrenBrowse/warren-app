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

use std::sync::Arc;
use std::time::Duration;

use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, Method, TransportError};

/// Connect budget, mirroring the SDK transport.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Total request budget, mirroring the SDK transport.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

struct Clients {
    with_sni: reqwest::Client,
    without_sni: reqwest::Client,
}

/// Shared, cheap to clone: every clone reuses the same connection pool, so a
/// per-call `WarrenApiClient` rebuild costs no extra TLS handshake.
#[derive(Clone)]
pub(crate) struct WarrenApiTransport {
    clients: Arc<Clients>,
}

impl WarrenApiTransport {
    /// The daemon transport, resolving the API host through the installed
    /// resolver when a daemon installed one.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::with_resolver(crate::warren_api_dns::resolver())
    }

    fn with_resolver(resolver: Option<Arc<dyn reqwest::dns::Resolve>>) -> Self {
        let build = |sni: bool| {
            let mut builder = reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(TOTAL_TIMEOUT)
                .tls_sni(sni);
            if let Some(resolver) = resolver.clone() {
                builder = builder.dns_resolver2(resolver);
            }
            builder
                .build()
                .expect("reqwest client build failed: invalid TLS backend configuration")
        };
        Self {
            clients: Arc::new(Clients {
                with_sni: build(true),
                without_sni: build(false),
            }),
        }
    }

    /// The client honouring `use_sni`. The SDK's fallback sequence retries its
    /// last attempt with SNI off to defeat SNI-based blocking, so the two
    /// clients must stay distinct.
    fn client_for(&self, use_sni: bool) -> &reqwest::Client {
        if use_sni {
            &self.clients.with_sni
        } else {
            &self.clients.without_sni
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
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = self
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
        let body = resp
            .bytes()
            .await
            .map_err(|e| to_transport_error(&e))?
            .to_vec();
        Ok(HttpResponse { status, body })
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

    /// The SNI toggle must select a genuinely different client: collapsing the
    /// two would silently disable the SDK's no-SNI anti-censorship retry.
    /// Whether SNI leaves the wire is the SDK's own test, which needs TLS.
    #[test]
    fn the_sni_toggle_selects_a_distinct_client() {
        let transport = WarrenApiTransport::with_resolver(None);
        assert!(!std::ptr::eq(
            transport.client_for(true),
            transport.client_for(false)
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
