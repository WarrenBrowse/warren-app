//! TLS for the daemon's HTTPS clients.
//!
//! reqwest's own rustls config keeps session tickets and resumes with them, which lets a server
//! link a client's connections across time and across the addresses it came from (in clear before
//! the tunnel, then through each exit). The engine's clients and the SDK's marked transport
//! already refuse resumption; this config gives the daemon's reqwest clients the same.
//!
//! A preconfigured config replaces every TLS option of the reqwest builder, `tls_sni` included,
//! so SNI is chosen here and nowhere else.

use std::sync::Arc;

/// Hands `builder` the daemon's TLS config, with or without SNI.
pub(crate) fn configure(builder: reqwest::ClientBuilder, sni: bool) -> reqwest::ClientBuilder {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    builder.use_preconfigured_tls(client_config(roots, sni))
}

/// The ring provider and the protocol versions reqwest's rustls backend uses, HTTP/1.1 over
/// ALPN since the daemon's reqwest has no HTTP/2, and no resumption.
fn client_config(roots: rustls::RootCertStore, sni: bool) -> rustls::ClientConfig {
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("the ring provider supports the default protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.resumption = rustls::client::Resumption::disabled();
    config.enable_sni = sni;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    use rustls::{HandshakeKind, ServerConfig};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const CA: &[u8] = include_bytes!("testdata/tls-test-ca.cert.pem");
    const CERT: &[u8] = include_bytes!("testdata/tls-localhost.cert.pem");
    const KEY: &[u8] = include_bytes!("testdata/tls-localhost.key.pem");

    /// What the server saw of one TLS handshake.
    #[derive(Debug, PartialEq)]
    struct Seen {
        sni: Option<String>,
        kind: Option<HandshakeKind>,
    }

    /// A TLS server that issues session tickets and would resume on them, so a client that
    /// resumes shows up as a resumed handshake.
    async fn server() -> (u16, tokio::sync::mpsc::UnboundedReceiver<Seen>) {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from_pem_slice(CERT).unwrap()],
                PrivateKeyDer::from_pem_slice(KEY).unwrap(),
            )
            .unwrap();
        config.ticketer = rustls::crypto::ring::Ticketer::new().unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let (tcp, _) = listener.accept().await.unwrap();
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    continue;
                };
                let (_, connection) = tls.get_ref();
                let _ = tx.send(Seen {
                    sni: connection.server_name().map(str::to_owned),
                    kind: connection.handshake_kind(),
                });
                let mut request = [0u8; 1024];
                let _ = tls.read(&mut request).await;
                let _ = tls
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
                let _ = tls.shutdown().await;
            }
        });
        (port, rx)
    }

    async fn two_connections(sni: bool) -> Vec<Seen> {
        let (port, mut seen) = server().await;
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from_pem_slice(CA).unwrap())
            .unwrap();
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(client_config(roots, sni))
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();

        let url = format!("https://localhost:{port}/");
        let mut handshakes = Vec::new();
        for _ in 0..2 {
            client.get(&url).send().await.unwrap();
            handshakes.push(seen.recv().await.unwrap());
        }
        handshakes
    }

    #[tokio::test]
    async fn a_second_connection_does_not_resume_the_first_session() {
        let handshakes = two_connections(true).await;

        assert_eq!(handshakes[1].kind, Some(HandshakeKind::Full));
    }

    #[tokio::test]
    async fn sni_stays_off_when_the_caller_turns_it_off() {
        let handshakes = two_connections(false).await;

        assert!(handshakes.iter().all(|seen| seen.sni.is_none()));
    }
}
