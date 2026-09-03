//! VpnService-protected HTTP transport for the v7 token mint.
//!
//! On Android every socket the app opens is subject to the VPN routes the
//! VpnService installed, so a mint request issued while the tunnel is coming
//! up gets routed into the not-yet-passing TUN and black-holes: the
//! connect-time immediate mint tick always lost that race and the first
//! session per process was v6. `VpnService.protect` is the platform fix, and
//! the engine already applies it to the Quinn and multihop sockets it binds
//! itself ([`warrenguard_transport::socket_protect`]); reqwest exposes no
//! pre-connect fd seam, so the mint client cannot. This transport creates its
//! own TCP socket, runs the process-wide protector on the fd BEFORE connect
//! (fail-closed: a refused protect never egresses), then speaks HTTP/1.1 over
//! rustls, mirroring the desktop daemon whose API traffic is likewise
//! excluded from its own tunnel.
//!
//! Trust model and behavior match the SDK's reqwest transport: webpki roots,
//! standard certificate verification even with SNI disabled (the
//! [`HttpRequest::use_sni`] anti-censorship fallback), 5 s connect / 15 s
//! total timeouts, and address-free error strings (no-log discipline). The
//! host name is still resolved by the system resolver, which the protector
//! cannot cover; a resolver black-holed during bring-up surfaces as a
//! connect-class failure and the next refresh tick retries.

use std::net::SocketAddr;
use std::os::raw::c_int;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpSocket;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls;
use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, Method, TransportError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

/// Protects a raw socket fd (routes it onto the physical network). `c_int`
/// rather than `RawFd` so the crate still compiles on non-unix hosts.
type ProtectFn = Arc<dyn Fn(c_int) -> bool + Send + Sync>;

pub(crate) struct ProtectedTransport {
    tls: Arc<rustls::ClientConfig>,
    tls_no_sni: Arc<rustls::ClientConfig>,
    protect: ProtectFn,
}

impl ProtectedTransport {
    /// The production transport: the engine's process-wide protector on
    /// Android (a no-op until the JNI bridge registers it), a pass-through
    /// elsewhere.
    ///
    /// Its only caller is the Android tunnel build, so every other target sees
    /// it as dead. It stays compiled there rather than being gated away: the
    /// pass-through arm is what keeps the host tests exercising the same
    /// constructor the device runs.
    #[cfg_attr(
        not(all(target_os = "android", feature = "tunnel")),
        expect(dead_code, reason = "only the Android tunnel build constructs it")
    )]
    pub(crate) fn new() -> Self {
        #[cfg(all(target_os = "android", feature = "tunnel"))]
        let protect: ProtectFn = Arc::new(warrenguard_transport::socket_protect::protect);
        #[cfg(not(all(target_os = "android", feature = "tunnel")))]
        let protect: ProtectFn = Arc::new(|_| true);
        Self::with_protect(protect)
    }

    pub(crate) fn with_protect(protect: ProtectFn) -> Self {
        let (tls, tls_no_sni) = tls_configs();
        Self {
            tls,
            tls_no_sni,
            protect,
        }
    }

    async fn execute_inner(
        &self,
        request: HttpRequest,
    ) -> Result<(HttpResponse, Option<String>), TransportError> {
        let url = parse_url(&request.url)
            .ok_or_else(|| TransportError::Io("unsupported request url".to_owned()))?;

        // A resolver failure is connect-class: it is exactly what censorship
        // of the primary host looks like, so it must drive the host fallback.
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((url.host.as_str(), url.port))
            .await
            .map_err(|_| TransportError::Connect("resolve failed".to_owned()))?
            .collect();

        // The failure kept is the most specific one across the candidate
        // addresses, named by class only (the report probes read it; no
        // address ever rides along).
        let mut stream = None;
        let mut failure = TransportError::Connect("connection failed".to_owned());
        for addr in addrs {
            let attempt =
                match tokio::time::timeout(CONNECT_TIMEOUT, self.connect_protected(addr)).await {
                    Ok(Ok(s)) => {
                        stream = Some(s);
                        break;
                    }
                    Ok(Err(err)) => err,
                    Err(_) => TransportError::Connect("connect timed out".to_owned()),
                };
            failure = more_specific(failure, attempt);
        }
        let Some(stream) = stream else {
            return Err(failure);
        };

        if url.tls {
            let config = if request.use_sni {
                self.tls.clone()
            } else {
                self.tls_no_sni.clone()
            };
            // Verification is against the requested host either way; only the
            // SNI extension on the wire differs (anti-censorship fallback).
            let server_name = rustls::pki_types::ServerName::try_from(url.host.clone())
                .map_err(|_| TransportError::Io("invalid host name".to_owned()))?;
            let tls_stream = TlsConnector::from(config)
                .connect(server_name, stream)
                .await
                .map_err(|_| TransportError::Connect("tls handshake failed".to_owned()))?;
            h1_roundtrip(tls_stream, &url, request).await
        } else {
            h1_roundtrip(stream, &url, request).await
        }
    }

    /// Binds a socket, protects its fd, then connects: the order the engine's
    /// own tunnel sockets use, and the whole point of this transport.
    async fn connect_protected(
        &self,
        addr: SocketAddr,
    ) -> Result<tokio::net::TcpStream, TransportError> {
        let socket = match addr {
            SocketAddr::V4(_) => TcpSocket::new_v4(),
            SocketAddr::V6(_) => TcpSocket::new_v6(),
        }
        .map_err(|_| TransportError::Io("socket creation failed".to_owned()))?;
        #[cfg(unix)]
        let protected = {
            use std::os::fd::AsRawFd;
            (self.protect)(socket.as_raw_fd())
        };
        #[cfg(not(unix))]
        let protected = true;
        if !protected {
            // Fail closed: an unprotected socket would loop into the TUN (or
            // leak around it); never egress on one.
            return Err(TransportError::Connect("socket protect refused".to_owned()));
        }
        socket
            .connect(addr)
            .await
            .map_err(|err| TransportError::Connect(connect_class(&err).to_owned()))
    }
}

/// Of two candidate addresses' failures, the one that says more about the
/// host. A refusal, a TLS failure or a refused protect proves the path was
/// there; a connect timeout, that a route exists; `network unreachable` only
/// that this address family has no route, the usual dual-stack noise when
/// the AAAA record is tried on a v4-only network. Keeping the last failure
/// let that noise overwrite a refusal in the very fact the probes report.
/// Ties keep the earlier one.
fn more_specific(current: TransportError, next: TransportError) -> TransportError {
    fn rank(err: &TransportError) -> u8 {
        match err {
            TransportError::Connect(msg) => match msg.as_str() {
                "connection refused" | "tls handshake failed" | "socket protect refused" => 0,
                "connect timed out" => 1,
                "network unreachable" => 2,
                _ => 3,
            },
            _ => 3,
        }
    }
    if rank(&next) < rank(&current) {
        next
    } else {
        current
    }
}

/// The class of a failed TCP connect, from the io error kind alone: a fixed
/// phrase per kind, never the address or the OS text.
fn connect_class(err: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match err.kind() {
        ErrorKind::ConnectionRefused => "connection refused",
        ErrorKind::TimedOut => "connect timed out",
        ErrorKind::NetworkUnreachable | ErrorKind::HostUnreachable => "network unreachable",
        _ => "connection failed",
    }
}

impl HttpTransport for ProtectedTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.execute_dated(request)
            .await
            .map(|(response, _)| response)
    }
}

impl ProtectedTransport {
    /// [`HttpTransport::execute`] that also hands back the response's `Date`
    /// header, the trusted clock the forum flows correct a skewed device
    /// against. The SDK's response type carries no headers, so the one
    /// header that matters is read here, where the HTTP/1.1 exchange is
    /// visible.
    pub(crate) async fn execute_dated(
        &self,
        request: HttpRequest,
    ) -> Result<(HttpResponse, Option<String>), TransportError> {
        self.execute_dated_within(request, TOTAL_TIMEOUT).await
    }

    /// [`Self::execute_dated`] under a caller-chosen total deadline. The
    /// default 15 s is the mint's, sized for a request of a few hundred
    /// bytes; it also covers the body upload, so a report carrying
    /// megabytes of logs on a slow uplink died in it as a generic transport
    /// failure after the data was spent. The report path sizes its own bound
    /// to the body it sends.
    pub(crate) async fn execute_dated_within(
        &self,
        request: HttpRequest,
        total: Duration,
    ) -> Result<(HttpResponse, Option<String>), TransportError> {
        tokio::time::timeout(total, self.execute_inner(request))
            .await
            .map_err(|_| TransportError::Io("request timed out".to_owned()))?
    }
}

/// The two TLS configurations (SNI on, SNI off), built once per process.
/// Each one parses the whole webpki root store, and a transport is built per
/// forum request (login preflight, signed POST, report upload, mint tick), so
/// building them per transport spent milliseconds of CPU on every call for a
/// value that never changes.
fn tls_configs() -> (Arc<rustls::ClientConfig>, Arc<rustls::ClientConfig>) {
    static CONFIGS: OnceLock<(Arc<rustls::ClientConfig>, Arc<rustls::ClientConfig>)> =
        OnceLock::new();
    CONFIGS
        .get_or_init(|| (Arc::new(tls_config(true)), Arc::new(tls_config(false))))
        .clone()
}

fn tls_config(sni: bool) -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring provider supports the default TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config.enable_sni = sni;
    config
}

struct ParsedUrl {
    tls: bool,
    host: String,
    port: u16,
    path_and_query: String,
}

/// Minimal `http(s)://host[:port][/path]` splitter; API hosts are always DNS
/// names (never bracketed IPv6 literals), matching the SDK's own fallback
/// host handling.
fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (scheme, rest) = url.split_once("://")?;
    let tls = match scheme {
        "https" => true,
        "http" => false,
        _ => return None,
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            (h, p.parse().ok()?)
        }
        _ => (authority, if tls { 443 } else { 80 }),
    };
    Some(ParsedUrl {
        tls,
        host: host.to_owned(),
        port,
        path_and_query: path.to_owned(),
    })
}

fn to_hyper_method(method: Method) -> hyper::Method {
    match method {
        Method::Get => hyper::Method::GET,
        Method::Post => hyper::Method::POST,
        Method::Delete => hyper::Method::DELETE,
    }
}

async fn h1_roundtrip<S>(
    stream: S,
    url: &ParsedUrl,
    request: HttpRequest,
) -> Result<(HttpResponse, Option<String>), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn io_err<E>(_: E) -> TransportError {
        TransportError::Io("request failed".to_owned())
    }
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(io_err)?;
    // The connection task owns the socket; it ends when the response (and
    // this one-shot sender) is dropped.
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let default_port = if url.tls { 443 } else { 80 };
    let host_header = if url.port == default_port {
        url.host.clone()
    } else {
        format!("{}:{}", url.host, url.port)
    };
    let mut builder = hyper::Request::builder()
        .method(to_hyper_method(request.method))
        .uri(&url.path_and_query)
        .header(hyper::header::HOST, host_header);
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    let req = builder
        .body(Full::new(Bytes::from(request.body)))
        .map_err(io_err)?;

    let response = sender.send_request(req).await.map_err(io_err)?;
    let status = response.status().as_u16();
    let date = response
        .headers()
        .get(hyper::header::DATE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(io_err)?
        .to_bytes()
        .to_vec();
    Ok((HttpResponse { status, body }, date))
}

/// Loopback HTTP/1.1 servers for the host tests of this transport and of the
/// report probes: one canned answer per accepted connection.
#[cfg(test)]
pub(crate) mod loopback {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Reads one HTTP/1.1 request (head + content-length body) then answers
    /// with `response`; returns the captured raw request bytes.
    pub(crate) async fn serve_one(listener: TcpListener, response: String) -> Vec<u8> {
        serve_many(listener, response, 1).await
    }

    /// `serve_one` that reads the request, waits `delay`, then answers: a
    /// server slow enough to run a client deadline out.
    pub(crate) async fn serve_one_after(
        listener: TcpListener,
        delay: std::time::Duration,
        response: String,
    ) -> Vec<u8> {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let captured = read_request(&mut stream).await;
        tokio::time::sleep(delay).await;
        stream.write_all(response.as_bytes()).await.expect("write");
        stream.flush().await.expect("flush");
        captured
    }

    /// `serve_one` for `count` successive connections; returns the last
    /// request captured.
    pub(crate) async fn serve_many(
        listener: TcpListener,
        response: String,
        count: usize,
    ) -> Vec<u8> {
        let mut captured = Vec::new();
        for _ in 0..count {
            captured = serve_accepted(&listener, &response).await;
        }
        captured
    }

    async fn serve_accepted(listener: &TcpListener, response: &str) -> Vec<u8> {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let captured = read_request(&mut stream).await;
        stream.write_all(response.as_bytes()).await.expect("write");
        stream.flush().await.expect("flush");
        captured
    }

    /// One HTTP/1.1 request, head plus its content-length body.
    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut captured = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).await.expect("read");
            captured.extend_from_slice(&buf[..n]);
            let head_end = captured.windows(4).position(|w| w == b"\r\n\r\n");
            if let Some(end) = head_end {
                let head = String::from_utf8_lossy(&captured[..end]).to_ascii_lowercase();
                let body_len: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if captured.len() >= end + 4 + body_len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        captured
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

    use tokio::net::TcpListener;
    use warren_api::transport::{HttpRequest, HttpTransport, Method, TransportError};

    use super::loopback::serve_one;
    use super::{ProtectFn, ProtectedTransport};

    #[test]
    fn the_tls_configs_are_built_once_per_process() {
        // A transport is built per forum request; each build used to parse
        // the whole root store twice.
        let pass: ProtectFn = Arc::new(|_| true);
        let a = ProtectedTransport::with_protect(pass.clone());
        let b = ProtectedTransport::with_protect(pass);

        assert!(
            Arc::ptr_eq(&a.tls, &b.tls),
            "one SNI root store per process"
        );
        assert!(
            Arc::ptr_eq(&a.tls_no_sni, &b.tls_no_sni),
            "one no-SNI root store per process"
        );
    }

    const CANNED_OK: &str = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nhi";

    fn get(url: String) -> HttpRequest {
        HttpRequest {
            method: Method::Get,
            url,
            headers: Vec::new(),
            body: Vec::new(),
            use_sni: true,
        }
    }

    #[tokio::test]
    async fn a_get_round_trips_and_the_protector_saw_the_socket_fd() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(serve_one(listener, CANNED_OK.to_owned()));

        let protected_fd = Arc::new(AtomicI32::new(-1));
        let seen = protected_fd.clone();
        let protect: ProtectFn = Arc::new(move |fd| {
            seen.store(fd, Ordering::SeqCst);
            true
        });
        let transport = ProtectedTransport::with_protect(protect);
        let response = transport
            .execute(get(format!(
                "http://127.0.0.1:{}/v1/tokens/keys",
                addr.port()
            )))
            .await
            .expect("roundtrip");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"hi");
        assert!(
            protected_fd.load(Ordering::SeqCst) >= 0,
            "the protector must have been handed the real socket fd"
        );
        let captured = server.await.expect("server");
        let head = String::from_utf8_lossy(&captured);
        assert!(
            head.starts_with("GET /v1/tokens/keys HTTP/1.1\r\n"),
            "{head}"
        );
    }

    #[tokio::test]
    async fn a_refused_protector_fails_closed_before_any_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        tokio::spawn(async move {
            loop {
                let _ = listener.accept().await;
                count.fetch_add(1, Ordering::SeqCst);
            }
        });

        let protect: ProtectFn = Arc::new(|_| false);
        let transport = ProtectedTransport::with_protect(protect);
        let err = transport
            .execute(get(format!("http://127.0.0.1:{}/x", addr.port())))
            .await
            .expect_err("a refused protect must never egress");
        assert!(matches!(err, TransportError::Connect(_)), "got {err:?}");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            accepted.load(Ordering::SeqCst),
            0,
            "protect runs BEFORE connect: the server must never see a connection"
        );
    }

    #[tokio::test]
    async fn a_post_carries_method_host_headers_and_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(serve_one(listener, CANNED_OK.to_owned()));

        let transport = ProtectedTransport::with_protect(Arc::new(|_| true));
        let request = HttpRequest {
            method: Method::Post,
            url: format!("http://127.0.0.1:{}/v1/tokens/issue", addr.port()),
            headers: vec![("x-warren-probe".to_owned(), "1".to_owned())],
            body: b"{\"epochs\":[]}".to_vec(),
            use_sni: true,
        };
        transport.execute(request).await.expect("roundtrip");

        let captured = String::from_utf8_lossy(&server.await.expect("server")).to_string();
        assert!(
            captured.starts_with("POST /v1/tokens/issue HTTP/1.1\r\n"),
            "{captured}"
        );
        let lower = captured.to_ascii_lowercase();
        assert!(
            lower.contains(&format!("host: 127.0.0.1:{}", addr.port())),
            "{captured}"
        );
        assert!(lower.contains("x-warren-probe: 1"), "{captured}");
        assert!(captured.ends_with("{\"epochs\":[]}"), "{captured}");
    }

    #[tokio::test]
    async fn a_non_2xx_status_is_a_response_not_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(serve_one(
            listener,
            "HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                .to_owned(),
        ));

        let transport = ProtectedTransport::with_protect(Arc::new(|_| true));
        let response = transport
            .execute(get(format!("http://127.0.0.1:{}/x", addr.port())))
            .await
            .expect("a served status is a response, whatever its code");
        assert_eq!(response.status, 503);
        server.await.expect("server");
    }

    #[tokio::test]
    async fn the_caller_deadline_bounds_the_whole_exchange_and_the_default_does_not_apply() {
        // A server that answers 300 ms after reading the request: a 100 ms
        // deadline runs out as "request timed out", a 3 s one gets the answer.
        let delay = std::time::Duration::from_millis(300);
        let transport = ProtectedTransport::with_protect(Arc::new(|_| true));

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(super::loopback::serve_one_after(
            listener,
            delay,
            CANNED_OK.to_owned(),
        ));
        let err = transport
            .execute_dated_within(
                get(format!("http://127.0.0.1:{}/v1/forum/report", addr.port())),
                std::time::Duration::from_millis(100),
            )
            .await
            .expect_err("100 ms must run out on a 300 ms server");
        assert!(
            matches!(&err, TransportError::Io(msg) if msg == "request timed out"),
            "got {err:?}"
        );
        server.abort();

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(super::loopback::serve_one_after(
            listener,
            delay,
            CANNED_OK.to_owned(),
        ));
        let (response, _) = transport
            .execute_dated_within(
                get(format!("http://127.0.0.1:{}/v1/forum/report", addr.port())),
                std::time::Duration::from_secs(3),
            )
            .await
            .expect("3 s covers a 300 ms server");
        assert_eq!(response.status, 200);
        server.await.expect("server");
    }

    #[test]
    fn a_refusal_outranks_the_unreachable_noise_of_the_other_address_family() {
        use super::more_specific;
        let connect = |msg: &str| TransportError::Connect(msg.to_owned());
        let phrase = |err: TransportError| match err {
            TransportError::Connect(msg) => msg,
            other => panic!("expected a Connect error, got {other:?}"),
        };
        // A then AAAA on a v4-only network: refused, then unreachable.
        assert_eq!(
            phrase(more_specific(
                connect("connection refused"),
                connect("network unreachable")
            )),
            "connection refused"
        );
        // AAAA first: the later refusal still wins.
        assert_eq!(
            phrase(more_specific(
                connect("network unreachable"),
                connect("connection refused")
            )),
            "connection refused"
        );
        assert_eq!(
            phrase(more_specific(
                connect("connection failed"),
                connect("connect timed out")
            )),
            "connect timed out"
        );
        assert_eq!(
            phrase(more_specific(
                connect("connect timed out"),
                connect("network unreachable")
            )),
            "connect timed out"
        );
        // The initial placeholder never beats a real attempt.
        assert_eq!(
            phrase(more_specific(
                connect("connection failed"),
                connect("network unreachable")
            )),
            "network unreachable"
        );
        // A tie keeps the first.
        assert_eq!(
            phrase(more_specific(
                connect("connection refused"),
                connect("tls handshake failed")
            )),
            "connection refused"
        );
    }

    #[tokio::test]
    async fn a_refused_connection_is_a_connect_error_without_the_address() {
        // Port 1 on loopback refuses immediately; the classification must
        // drive the SDK host fallback, and the message must not leak the
        // address (no-log discipline), mirroring the reqwest transport.
        let transport = ProtectedTransport::with_protect(Arc::new(|_| true));
        let err = transport
            .execute(get("http://127.0.0.1:1/".to_owned()))
            .await
            .expect_err("nothing listens on port 1");
        match err {
            TransportError::Connect(msg) => {
                assert!(!msg.contains("127.0.0.1"), "must not leak the address");
            }
            other => panic!("expected a Connect error, got {other:?}"),
        }
    }
}
