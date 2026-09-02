//! Live network probes taken while a problem report is collected.
//!
//! The report header already says what the device IS (ROM, restrictions,
//! tunnel state); these keys say what the network DOES at that moment, which
//! is the question a failed forum sign-in leaves open: the forum POST rides
//! the VpnService-protected transport, but the host name is still resolved by
//! the system resolver, and a TUN between states swallows the plain path. One
//! probe per leg tells the three apart from the report alone: the connect
//! host through the protected socket, the same host through the default
//! (unprotected) client, the resolver, and the API host.
//!
//! Every value is a class or a duration, never an address, so the block needs
//! no redaction. The `Date` header of the first answering probe doubles as
//! the clock offset the login and report paths correct the signature with.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use warren_api::transport::{HttpRequest, HttpResponse, Method, TransportError};

/// Bound on one probe. The protected transport's own connect timeout (5 s)
/// fires first, so a slower failure here is a read that never came back.
#[cfg(target_os = "android")]
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);

/// Where the probes go. Full origins, so the tests point them at loopback
/// listeners over plain HTTP.
pub(crate) struct ProbeTargets {
    /// The connect host name, resolved on its own for the resolver probe.
    pub connect_host: String,
    /// `scheme://host[:port]` of the connect broker.
    pub connect_base: String,
    /// `scheme://host[:port]` of the Warren API.
    pub api_base: String,
}

impl ProbeTargets {
    /// The production targets: the allowlisted connect host and the API of
    /// this build's product environment.
    #[cfg(target_os = "android")]
    pub(crate) fn production() -> Self {
        let host = crate::forum::connect_host();
        Self {
            connect_host: host.to_owned(),
            connect_base: format!("https://{host}"),
            api_base: crate::product::PRODUCT_API_URL.to_owned(),
        }
    }
}

/// A GET that may hand back the response's `Date` header. The SDK's response
/// type carries no headers, so only the transport that speaks HTTP/1.1
/// itself can supply one; the others answer `None` and still classify.
pub(crate) trait DatedGet {
    async fn get_dated(
        &self,
        url: String,
    ) -> Result<(HttpResponse, Option<String>), TransportError>;
}

fn get(url: String) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url,
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    }
}

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
impl DatedGet for crate::protected_transport::ProtectedTransport {
    async fn get_dated(
        &self,
        url: String,
    ) -> Result<(HttpResponse, Option<String>), TransportError> {
        self.execute_dated(get(url)).await
    }
}

/// The SDK's reqwest client, the default (unprotected) leg on the device. Its
/// transport feature is only pulled in by the Android build.
#[cfg(target_os = "android")]
impl DatedGet for warren_api::reqwest_transport::ReqwestTransport {
    async fn get_dated(
        &self,
        url: String,
    ) -> Result<(HttpResponse, Option<String>), TransportError> {
        use warren_api::transport::HttpTransport;
        self.execute(get(url))
            .await
            .map(|response| (response, None))
    }
}

type Probed = (
    Result<(HttpResponse, Option<String>), TransportError>,
    Duration,
);

async fn timed(
    fut: impl Future<Output = Result<(HttpResponse, Option<String>), TransportError>>,
    bound: Duration,
) -> Probed {
    let started = Instant::now();
    let result = match tokio::time::timeout(bound, fut).await {
        Ok(result) => result,
        Err(_) => Err(TransportError::Io("probe timed out".to_owned())),
    };
    (result, started.elapsed())
}

/// The class of one probe: `ok-<ms>ms` for a 2xx, `http-<n>` for any other
/// served status, else the failure leg. The protected transport names the
/// connect failure it saw (refused, timed out, unreachable, resolver, TLS);
/// the SDK's client only knows connect versus after-connect, so its failures
/// class coarser.
fn class(probed: &Probed) -> String {
    match &probed.0 {
        Ok((response, _)) if (200..300).contains(&response.status) => {
            format!("ok-{}ms", probed.1.as_millis())
        }
        Ok((response, _)) => format!("http-{}", response.status),
        Err(TransportError::Connect(msg)) => match msg.as_str() {
            "connection refused" => "connect-refused",
            "connect timed out" => "connect-timeout",
            "network unreachable" => "connect-unreachable",
            "resolve failed" => "dns-failed",
            "tls handshake failed" => "tls-failed",
            "socket protect refused" => "protect-refused",
            _ => "connect-failed",
        }
        .to_owned(),
        Err(TransportError::Io(msg)) => match msg.as_str() {
            "probe timed out" | "request timed out" => "read-timeout",
            _ => "io-failed",
        }
        .to_owned(),
        Err(_) => "failed".to_owned(),
    }
}

async fn dns_probe(host: &str, bound: Duration) -> String {
    let started = Instant::now();
    match tokio::time::timeout(bound, tokio::net::lookup_host((host, 443u16))).await {
        Ok(Ok(addrs)) => format!("ok-{}-{}ms", addrs.count(), started.elapsed().as_millis()),
        Ok(Err(_)) => "dns-failed".to_owned(),
        Err(_) => "dns-timeout".to_owned(),
    }
}

/// Runs the four probes concurrently and returns their facts, keyed for the
/// report's metadata block:
///
/// - `probe-connect-protected`, `probe-connect-default`: the connect host's
///   health route through each transport;
/// - `probe-dns-connect`: the system resolver on the connect host;
/// - `probe-api`: the API's public network descriptor through the default
///   client;
/// - `clock-offset`, `clock-offset-source`: server minus device seconds from
///   the first answering dated probe, and which leg supplied it.
#[cfg(target_os = "android")]
pub(crate) async fn run(
    targets: &ProbeTargets,
    protected: &impl DatedGet,
    default: &impl DatedGet,
    device_now: u64,
) -> BTreeMap<String, String> {
    run_with_bound(targets, protected, default, device_now, PROBE_TIMEOUT).await
}

async fn run_with_bound(
    targets: &ProbeTargets,
    protected: &impl DatedGet,
    default: &impl DatedGet,
    device_now: u64,
    bound: Duration,
) -> BTreeMap<String, String> {
    let health = format!("{}/healthz", targets.connect_base);
    let network = format!("{}/v1/network", targets.api_base);
    let (via_protected, via_default, api, dns) = tokio::join!(
        timed(protected.get_dated(health.clone()), bound),
        timed(default.get_dated(health), bound),
        timed(default.get_dated(network), bound),
        dns_probe(&targets.connect_host, bound),
    );

    let mut facts = BTreeMap::new();
    facts.insert("probe-connect-protected".to_owned(), class(&via_protected));
    facts.insert("probe-connect-default".to_owned(), class(&via_default));
    facts.insert("probe-api".to_owned(), class(&api));
    facts.insert("probe-dns-connect".to_owned(), dns);

    let dated = [("protected", &via_protected), ("default", &via_default)]
        .into_iter()
        .find_map(|(source, probed)| match &probed.0 {
            Ok((_, Some(date))) => {
                crate::forum::clock_offset_secs(date, device_now).map(|offset| (source, offset))
            }
            _ => None,
        });
    match dated {
        Some((source, offset)) => {
            facts.insert("clock-offset".to_owned(), format!("{offset}s"));
            facts.insert("clock-offset-source".to_owned(), source.to_owned());
        }
        None => {
            facts.insert("clock-offset".to_owned(), "unknown".to_owned());
            facts.insert("clock-offset-source".to_owned(), "none".to_owned());
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::net::TcpListener;

    use super::{ProbeTargets, run_with_bound};
    use crate::protected_transport::ProtectedTransport;
    use crate::protected_transport::loopback::{serve_many, serve_one};

    const DEVICE_NOW: u64 = 1_788_371_428;
    /// Two minutes ahead of `DEVICE_NOW`, the class of skew behind the
    /// 2026-08-18 sign-in failures.
    const SERVER_DATE: &str = "Wed, 02 Sep 2026 17:52:28 GMT";
    const UNDATED_OK: &str = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok";
    const SERVER_ERROR: &str =
        "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

    fn dated_ok() -> String {
        format!(
            "HTTP/1.1 200 OK\r\ndate: {SERVER_DATE}\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
        )
    }

    fn passthrough() -> ProtectedTransport {
        ProtectedTransport::with_protect(Arc::new(|_| true))
    }

    fn targets(connect_port: u16, api_port: u16) -> ProbeTargets {
        ProbeTargets {
            connect_host: "127.0.0.1".to_owned(),
            connect_base: format!("http://127.0.0.1:{connect_port}"),
            api_base: format!("http://127.0.0.1:{api_port}"),
        }
    }

    async fn listener() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        (listener, port)
    }

    /// A port nothing listens on: bound, read, released.
    async fn refused_port() -> u16 {
        let (listener, port) = listener().await;
        drop(listener);
        port
    }

    async fn probe(
        connect_port: u16,
        api_port: u16,
        bound: Duration,
    ) -> std::collections::BTreeMap<String, String> {
        run_with_bound(
            &targets(connect_port, api_port),
            &passthrough(),
            &passthrough(),
            DEVICE_NOW,
            bound,
        )
        .await
    }

    #[tokio::test]
    async fn a_dated_answer_yields_ok_and_the_clock_offset_from_the_protected_leg() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        // The health route is asked twice (protected and default leg); both
        // answers are dated, so the source key is what proves the precedence.
        let connect_server = tokio::spawn(serve_many(connect, dated_ok(), 2));
        let api_server = tokio::spawn(serve_one(api, UNDATED_OK.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        for key in [
            "probe-connect-protected",
            "probe-connect-default",
            "probe-api",
        ] {
            let value = &facts[key];
            assert!(
                value.starts_with("ok-") && value.ends_with("ms"),
                "{key} = {value}"
            );
        }
        assert!(
            facts["probe-dns-connect"].starts_with("ok-1-"),
            "{}",
            facts["probe-dns-connect"]
        );
        assert_eq!(
            crate::forum::clock_offset_secs(SERVER_DATE, DEVICE_NOW),
            Some(120),
            "the fixture is the server two minutes ahead of the device"
        );
        assert_eq!(facts["clock-offset"], "120s");
        assert_eq!(facts["clock-offset-source"], "protected");
        connect_server.await.expect("connect server");
        api_server.await.expect("api server");
    }

    #[tokio::test]
    async fn a_refused_connection_is_classed_connect_refused() {
        let port = refused_port().await;

        let facts = probe(port, port, Duration::from_secs(3)).await;

        assert_eq!(facts["probe-connect-protected"], "connect-refused");
        assert_eq!(facts["probe-connect-default"], "connect-refused");
        assert_eq!(facts["probe-api"], "connect-refused");
        assert_eq!(facts["clock-offset"], "unknown");
        assert_eq!(facts["clock-offset-source"], "none");
    }

    #[tokio::test]
    async fn a_listener_that_never_answers_is_classed_read_timeout() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        // Accept and hold every connection open without ever writing.
        let hold = |listener: TcpListener| async move {
            let mut held = Vec::new();
            loop {
                let (stream, _) = listener.accept().await.expect("accept");
                held.push(stream);
            }
        };
        let connect_server = tokio::spawn(hold(connect));
        let api_server = tokio::spawn(hold(api));

        let facts = probe(connect_port, api_port, Duration::from_millis(400)).await;

        assert_eq!(facts["probe-connect-protected"], "read-timeout");
        assert_eq!(facts["probe-connect-default"], "read-timeout");
        assert_eq!(facts["probe-api"], "read-timeout");
        connect_server.abort();
        api_server.abort();
    }

    #[tokio::test]
    async fn a_served_500_is_classed_by_its_status() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_many(connect, SERVER_ERROR.to_owned(), 2));
        let api_server = tokio::spawn(serve_one(api, SERVER_ERROR.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        assert_eq!(facts["probe-connect-protected"], "http-500");
        assert_eq!(facts["probe-connect-default"], "http-500");
        assert_eq!(facts["probe-api"], "http-500");
        connect_server.await.expect("connect server");
        api_server.await.expect("api server");
    }

    #[tokio::test]
    async fn an_unresolvable_host_is_classed_dns_failed_on_every_leg() {
        let facts = run_with_bound(
            &ProbeTargets {
                connect_host: "connect.invalid.".to_owned(),
                connect_base: "http://connect.invalid.:1".to_owned(),
                api_base: "http://api.invalid.:1".to_owned(),
            },
            &passthrough(),
            &passthrough(),
            DEVICE_NOW,
            Duration::from_secs(5),
        )
        .await;

        assert_eq!(facts["probe-dns-connect"], "dns-failed");
        assert_eq!(facts["probe-connect-protected"], "dns-failed");
        assert_eq!(facts["probe-connect-default"], "dns-failed");
        assert_eq!(facts["probe-api"], "dns-failed");
    }

    #[tokio::test]
    async fn no_fact_ever_carries_an_address_or_a_port() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_many(connect, dated_ok(), 2));
        let api_server = tokio::spawn(serve_one(api, UNDATED_OK.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        for (key, value) in &facts {
            assert!(
                !value.contains("127.0.0.1")
                    && !value.contains(&connect_port.to_string())
                    && !value.contains(&api_port.to_string()),
                "{key} = {value} leaks the target"
            );
        }
        connect_server.await.expect("connect server");
        api_server.await.expect("api server");
    }
}
