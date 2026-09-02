//! Live network probes taken while a problem report is collected for a send.
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
//! The probes deliberately cross the tunnel boundary: the protected socket
//! egresses from the device's own address, the default client from the exit
//! while a tunnel is up. Two rules keep that from becoming a linkage. They
//! run only for a send, where a protected POST to the same host follows
//! anyway and the user has asked for it ("View the logs" records
//! [`not_run`]); and the default leg on the connect host is asked only after
//! the protected one has failed, never alongside it, so the broker never sees
//! the two addresses in the same instant.
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

/// The class of a transport failure, from the fixed phrases the transports
/// emit: the one table the probes, the login log and the report log all read,
/// so a phrase can not drift out of one of them. The protected transport
/// names the connect failure it saw (refused, timed out, unreachable,
/// resolver, TLS, protect); the SDK's client only knows connect versus
/// after-connect, so its failures class coarser (`connect-failed`,
/// `read-timeout`, `io-failed`).
pub(crate) fn class_of(err: &TransportError) -> &'static str {
    match err {
        TransportError::Connect(msg) => match msg.as_str() {
            "connection refused" => "connect-refused",
            "connect timed out" => "connect-timeout",
            "network unreachable" => "connect-unreachable",
            "resolve failed" => "dns-failed",
            "tls handshake failed" => "tls-failed",
            "socket protect refused" => "protect-refused",
            _ => "connect-failed",
        },
        TransportError::Io(msg) => match msg.as_str() {
            "probe timed out" | "request timed out" => "read-timeout",
            _ => "io-failed",
        },
        _ => "failed",
    }
}

/// The class of one probe: `ok-<ms>ms` for a 2xx, `http-<n>` for any other
/// served status, else the failure class.
fn class(probed: &Probed) -> String {
    match &probed.0 {
        Ok((response, _)) if (200..300).contains(&response.status) => {
            format!("ok-{}ms", probed.1.as_millis())
        }
        Ok((response, _)) => format!("http-{}", response.status),
        Err(err) => class_of(err).to_owned(),
    }
}

/// One probe over the default client, which may be missing when the SDK
/// client could not be built (a broken TLS stack): then the leg reads
/// `unavailable` rather than failing the whole collection.
async fn via_default<D: DatedGet>(
    default: Option<&D>,
    url: String,
    bound: Duration,
) -> Option<Probed> {
    match default {
        Some(client) => Some(timed(client.get_dated(url), bound).await),
        None => None,
    }
}

fn class_or_unavailable(probed: Option<&Probed>) -> String {
    probed.map_or_else(|| "unavailable".to_owned(), class)
}

async fn dns_probe(host: &str, bound: Duration) -> String {
    let started = Instant::now();
    match tokio::time::timeout(bound, tokio::net::lookup_host((host, 443u16))).await {
        Ok(Ok(addrs)) => format!("ok-{}-{}ms", addrs.count(), started.elapsed().as_millis()),
        Ok(Err(_)) => "dns-failed".to_owned(),
        Err(_) => "dns-timeout".to_owned(),
    }
}

/// The probe keys of a report collected without a send ("View the logs"):
/// every key a run writes, valued `not-run`, so the header keeps one shape
/// and a reader knows the legs were not asked rather than lost. Nothing
/// leaves the device for a report the user only reads.
pub(crate) fn not_run() -> BTreeMap<String, String> {
    let mut facts = BTreeMap::new();
    for key in [
        "probe-connect-protected",
        "probe-connect-default",
        "probe-api",
        "probe-dns-connect",
    ] {
        facts.insert(key.to_owned(), "not-run".to_owned());
    }
    facts.insert("clock-offset".to_owned(), "unknown".to_owned());
    facts.insert("clock-offset-source".to_owned(), "none".to_owned());
    facts
}

/// Runs the probes and returns their facts, keyed for the report's metadata
/// block:
///
/// - `probe-connect-protected`: the connect host's health route through the
///   protected socket;
/// - `probe-connect-default`: the same route through the default client, asked
///   only after the protected leg failed (`skipped` otherwise: a protected
///   answer leaves it nothing to explain, and asking both at once would hand
///   the broker the device address and the exit address as a pair);
/// - `probe-dns-connect`: the system resolver on the connect host;
/// - `probe-api`: the API's public network descriptor through the default
///   client (a different host, so it runs alongside the protected leg);
/// - `clock-offset`, `clock-offset-source`: server minus device seconds from
///   the first answering dated probe, and which leg supplied it.
///
/// `default` is `None` when the SDK client could not be built; its legs then
/// read `unavailable`.
#[cfg(target_os = "android")]
pub(crate) async fn run<P: DatedGet, D: DatedGet>(
    targets: &ProbeTargets,
    protected: &P,
    default: Option<&D>,
    device_now: u64,
) -> BTreeMap<String, String> {
    run_with_bound(targets, protected, default, device_now, PROBE_TIMEOUT).await
}

async fn run_with_bound<P: DatedGet, D: DatedGet>(
    targets: &ProbeTargets,
    protected: &P,
    default: Option<&D>,
    device_now: u64,
    bound: Duration,
) -> BTreeMap<String, String> {
    let health = format!("{}/healthz", targets.connect_base);
    let network = format!("{}/v1/network", targets.api_base);
    let (protected_leg, api, dns) = tokio::join!(
        timed(protected.get_dated(health.clone()), bound),
        via_default(default, network, bound),
        dns_probe(&targets.connect_host, bound),
    );
    let default_leg = match &protected_leg.0 {
        Ok(_) => None,
        Err(_) => via_default(default, health, bound).await,
    };

    let mut facts = BTreeMap::new();
    facts.insert("probe-connect-protected".to_owned(), class(&protected_leg));
    facts.insert(
        "probe-connect-default".to_owned(),
        match (&protected_leg.0, default_leg.as_ref()) {
            (Ok(_), _) => "skipped".to_owned(),
            (Err(_), leg) => class_or_unavailable(leg),
        },
    );
    facts.insert("probe-api".to_owned(), class_or_unavailable(api.as_ref()));
    facts.insert("probe-dns-connect".to_owned(), dns);

    let dated = [
        ("protected", Some(&protected_leg)),
        ("default", default_leg.as_ref()),
    ]
    .into_iter()
    .find_map(|(source, probed)| match probed.map(|p| &p.0) {
        Some(Ok((_, Some(date)))) => {
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
    use warren_api::transport::{HttpResponse, TransportError};

    use super::{DatedGet, ProbeTargets, class_of, not_run, run_with_bound};
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

    /// The default leg as the device has it: the SDK's reqwest client, which
    /// hands back no `Date` header and only the three coarse phrases its
    /// `to_transport_error` can produce (`connection failed` for anything at
    /// connect time, DNS included; `request timed out`; `request failed`).
    /// Speaking through the loopback transport, but classing like reqwest,
    /// so the test tells the two legs apart the way the report does.
    struct SdkLike(ProtectedTransport);

    impl DatedGet for SdkLike {
        async fn get_dated(
            &self,
            url: String,
        ) -> Result<(HttpResponse, Option<String>), TransportError> {
            match self.0.get_dated(url).await {
                Ok((response, _)) => Ok((response, None)),
                Err(TransportError::Connect(_)) => {
                    Err(TransportError::Connect("connection failed".to_owned()))
                }
                Err(TransportError::Io(msg)) if msg == "request timed out" => {
                    Err(TransportError::Io(msg))
                }
                Err(_) => Err(TransportError::Io("request failed".to_owned())),
            }
        }
    }

    fn sdk_like() -> SdkLike {
        SdkLike(passthrough())
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
            Some(&sdk_like()),
            DEVICE_NOW,
            bound,
        )
        .await
    }

    #[test]
    fn the_transport_phrases_class_the_way_the_report_reads_them() {
        // The protected transport's own phrases, one class each.
        for (phrase, class) in [
            ("connection refused", "connect-refused"),
            ("connect timed out", "connect-timeout"),
            ("network unreachable", "connect-unreachable"),
            ("resolve failed", "dns-failed"),
            ("tls handshake failed", "tls-failed"),
            ("socket protect refused", "protect-refused"),
        ] {
            assert_eq!(class_of(&TransportError::Connect(phrase.to_owned())), class);
        }
        // The SDK client's phrases: coarse, but each its own class.
        assert_eq!(
            class_of(&TransportError::Connect("connection failed".to_owned())),
            "connect-failed"
        );
        assert_eq!(
            class_of(&TransportError::Io("request timed out".to_owned())),
            "read-timeout"
        );
        assert_eq!(
            class_of(&TransportError::Io("request failed".to_owned())),
            "io-failed"
        );
    }

    #[test]
    fn a_report_collected_without_a_send_names_every_probe_key_as_not_run() {
        let facts = not_run();
        for key in [
            "probe-connect-protected",
            "probe-connect-default",
            "probe-api",
            "probe-dns-connect",
        ] {
            assert_eq!(facts[key], "not-run", "{key}");
        }
        assert_eq!(facts["clock-offset"], "unknown");
        assert_eq!(facts["clock-offset-source"], "none");
    }

    #[tokio::test]
    async fn a_dated_answer_yields_ok_and_the_clock_offset_from_the_protected_leg() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_one(connect, dated_ok()));
        let api_server = tokio::spawn(serve_one(api, UNDATED_OK.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        for key in ["probe-connect-protected", "probe-api"] {
            let value = &facts[key];
            assert!(
                value.starts_with("ok-") && value.ends_with("ms"),
                "{key} = {value}"
            );
        }
        assert_eq!(facts["probe-connect-default"], "skipped");
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
    async fn the_connect_host_is_asked_once_while_the_protected_leg_answers() {
        // The broker must never see the protected and the default leg as a
        // pair: while the protected one answers, the default one is not sent.
        // The connect server waits for TWO connections and must still be
        // waiting for the second once the probes are done.
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_many(connect, dated_ok(), 2));
        let api_server = tokio::spawn(serve_one(api, UNDATED_OK.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        assert!(facts["probe-connect-protected"].starts_with("ok-"));
        assert_eq!(facts["probe-connect-default"], "skipped");
        let second = tokio::time::timeout(Duration::from_millis(200), connect_server).await;
        assert!(
            second.is_err(),
            "a second connection reached the connect host: the default leg was sent"
        );
        api_server.await.expect("api server");
    }

    #[tokio::test]
    async fn a_refused_connection_is_classed_by_each_leg_in_its_own_words() {
        let port = refused_port().await;

        let facts = probe(port, port, Duration::from_secs(3)).await;

        assert_eq!(facts["probe-connect-protected"], "connect-refused");
        // The protected leg failed, so the default one was asked, and the SDK
        // client can only say that the connect failed.
        assert_eq!(facts["probe-connect-default"], "connect-failed");
        assert_eq!(facts["probe-api"], "connect-failed");
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
    async fn a_served_500_is_classed_by_its_status_and_needs_no_default_leg() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_one(connect, SERVER_ERROR.to_owned()));
        let api_server = tokio::spawn(serve_one(api, SERVER_ERROR.to_owned()));

        let facts = probe(connect_port, api_port, Duration::from_secs(3)).await;

        assert_eq!(facts["probe-connect-protected"], "http-500");
        assert_eq!(facts["probe-connect-default"], "skipped");
        assert_eq!(facts["probe-api"], "http-500");
        connect_server.await.expect("connect server");
        api_server.await.expect("api server");
    }

    #[tokio::test]
    async fn an_unresolvable_host_is_dns_failed_on_the_protected_leg_and_connect_failed_on_the_sdks()
     {
        let facts = run_with_bound(
            &ProbeTargets {
                connect_host: "connect.invalid.".to_owned(),
                connect_base: "http://connect.invalid.:1".to_owned(),
                api_base: "http://api.invalid.:1".to_owned(),
            },
            &passthrough(),
            Some(&sdk_like()),
            DEVICE_NOW,
            Duration::from_secs(5),
        )
        .await;

        assert_eq!(facts["probe-dns-connect"], "dns-failed");
        assert_eq!(facts["probe-connect-protected"], "dns-failed");
        // reqwest folds a resolver failure into its connect failure.
        assert_eq!(facts["probe-connect-default"], "connect-failed");
        assert_eq!(facts["probe-api"], "connect-failed");
    }

    #[tokio::test]
    async fn without_a_default_client_its_legs_read_unavailable() {
        let port = refused_port().await;

        let facts = run_with_bound(
            &targets(port, port),
            &passthrough(),
            None::<&SdkLike>,
            DEVICE_NOW,
            Duration::from_secs(3),
        )
        .await;

        assert_eq!(facts["probe-connect-protected"], "connect-refused");
        assert_eq!(facts["probe-connect-default"], "unavailable");
        assert_eq!(facts["probe-api"], "unavailable");
    }

    #[tokio::test]
    async fn no_fact_ever_carries_an_address_or_a_port() {
        let (connect, connect_port) = listener().await;
        let (api, api_port) = listener().await;
        let connect_server = tokio::spawn(serve_one(connect, dated_ok()));
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
