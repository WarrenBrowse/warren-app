//! Resolve valid proxy configurations via DoH.
//!
use crate::config;
use core::fmt;
use hickory_resolver::{
    TokioResolver,
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    net::{NetError, proto::rr::RData, runtime::TokioRuntimeProvider},
};
use rustls::ClientConfig;
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::time::error::Elapsed;

/// The port to connect to the DoH resolvers over.
const RESOLVER_PORT: u16 = 443;
const DEFAULT_TIMEOUT: Duration = std::time::Duration::from_secs(10);

pub struct Nameserver {
    pub name: String,
    pub addr: Vec<IpAddr>,
}

#[derive(Debug)]
pub enum Error {
    ResolutionError(NetError),
    Timeout(Elapsed),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ResolutionError(err) => err.fmt(f),
            Error::Timeout(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ResolutionError(err) => err.source(),
            Self::Timeout(err) => err.source(),
        }
    }
}

/// Returns the public DoH resolvers used to bootstrap the Encrypted DNS
/// proxy. Warren keeps only Quad9: Cloudflare (1.1.1.1 / 1.0.0.1) was dropped
/// so the daemon never contacts Cloudflare-operated resolvers.
pub fn default_resolvers() -> Vec<Nameserver> {
    vec![Nameserver {
        name: "dns.quad9.net".to_owned(),
        addr: vec![
            "9.9.9.9".parse().unwrap(),
            "149.112.112.112".parse().unwrap(),
        ],
    }]
}

/// Calls [resolve_configs] with a given `domain` using known DoH resolvers provided by [default_resolvers]
pub async fn resolve_default_config(domain: &str) -> Result<Vec<config::ProxyConfig>, Error> {
    resolve_configs(&default_resolvers(), domain).await
}

/// Looks up the `domain` towards the given `resolvers`, and try to deserialize all the returned
/// AAAA records into [`ProxyConfig`](config::ProxyConfig)s.
pub async fn resolve_configs(
    resolvers: &[Nameserver],
    domain: &str,
) -> Result<Vec<config::ProxyConfig>, Error> {
    let mut name_servers = Vec::new();
    for resolver in resolvers.iter() {
        let server_name: Arc<str> = Arc::from(resolver.name.as_str());
        for addr in resolver.addr.iter().copied() {
            let mut ns_config = NameServerConfig::https(addr, server_name.clone(), None);
            ns_config.connections.iter_mut().for_each(|conn| {
                conn.port = RESOLVER_PORT;
            });
            name_servers.push(ns_config);
        }
    }
    let nameservers = ResolverConfig::from_parts(None, vec![], name_servers);

    let mut resolver_config: ResolverOpts = Default::default();
    resolver_config.timeout = Duration::from_secs(5);
    resolve_config_with_resolverconfig(nameservers, resolver_config, domain, DEFAULT_TIMEOUT).await
}

pub async fn resolve_config_with_resolverconfig(
    resolver_config: ResolverConfig,
    options: ResolverOpts,
    domain: &str,
    timeout: Duration,
) -> Result<Vec<config::ProxyConfig>, Error> {
    let resolver =
        TokioResolver::builder_with_config(resolver_config, TokioRuntimeProvider::default())
            .with_options(options)
            .with_tls_config(client_config_tls12())
            .build()
            .map_err(Error::ResolutionError)?;

    let lookup = tokio::time::timeout(timeout, resolver.ipv6_lookup(domain))
        .await
        .map_err(Error::Timeout)?
        .map_err(Error::ResolutionError)?;

    let addrs = lookup
        .answers()
        .iter()
        .filter_map(|record| match &record.data {
            RData::AAAA(aaaa_record) => Some(aaaa_record.0),
            _ => None,
        });

    let mut proxy_configs = Vec::new();
    for addr in addrs {
        match config::ProxyConfig::try_from(addr) {
            Ok(proxy_config) => {
                log::trace!("IPv6 {addr} parsed into proxy config: {proxy_config:?}");
                proxy_configs.push(proxy_config);
            }
            Err(config::Error::XorV1Unsupported) => continue, // ignore deprecated configs
            Err(e) => log::error!("IPv6 {addr} fails to parse to a proxy config: {e}"),
        }
    }

    Ok(proxy_configs)
}

fn client_config_tls12() -> ClientConfig {
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

// Live-network smoke test: resolves Warren's Encrypted DNS proxy configs over
// Quad9 DoH. Ignored by default — it requires network access and the
// `dns.warrenbrowse.com` AAAA records to be published.
#[cfg(test)]
#[tokio::test]
#[ignore = "hits live network (Quad9 DoH) and depends on published AAAA records"]
async fn test_resolution() {
    let nameservers = vec![Nameserver {
        name: "dns.quad9.net".to_owned(),
        addr: vec![
            "9.9.9.9".parse().unwrap(),
            "149.112.112.112".parse().unwrap(),
        ],
    }];

    let _ = resolve_configs(&nameservers, "dns.warrenbrowse.com")
        .await
        .unwrap();
}

#[cfg(test)]
#[test]
fn default_resolvers_dont_panic() {
    let _ = default_resolvers();
}
