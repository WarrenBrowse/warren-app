//! This module implements fetching of information about app versions

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::fs;
#[cfg(test)]
use vec1::Vec1;

use crate::defaults;
use crate::format::response::SignedResponse;
use crate::version::{VersionInfo, VersionParameters};

use super::version_provider::VersionInfoProvider;

/// Available platforms in the default metadata repository
#[derive(Debug, Clone, Copy)]
pub enum MetaRepositoryPlatform {
    Windows,
    Linux,
    Macos,
    Android,
    Ios,
}

impl MetaRepositoryPlatform {
    /// Return the current platform
    pub fn current() -> Option<Self> {
        cfg_select! {
            target_os = "windows" => { Some(Self::Windows) }
            target_os = "linux"   => { Some(Self::Linux) }
            target_os = "macos"   => { Some(Self::Macos) }
            target_os = "android" => { Some(Self::Android) }
            target_os = "ios"     => { Some(Self::Ios) }
            _ => { None }
        }
    }

    /// Return complete URL used for the metadata. Resolves the
    /// releases endpoint through [`defaults::releases_url`] so the
    /// `WARREN_UPDATE_URL` env var override (and the Warren default
    /// when the var is unset) take precedence over the upstream
    /// Mullvad constant.
    pub fn url(&self) -> String {
        format!("{}/{}", defaults::releases_url(), self.filename())
    }

    fn filename(&self) -> &str {
        match self {
            MetaRepositoryPlatform::Windows => "windows.json",
            MetaRepositoryPlatform::Linux => "linux.json",
            MetaRepositoryPlatform::Macos => "macos.json",
            MetaRepositoryPlatform::Android => "android.json",
            MetaRepositoryPlatform::Ios => "ios.json",
        }
    }
}

/// Obtain version data using a GET request
pub struct HttpVersionInfoProvider {
    /// Endpoint for GET request
    url: String,
    /// Optional host to resolve (to the IP) without DNS
    resolve: Option<(&'static str, IpAddr)>,
    /// Accepted root certificate. Defaults are used unless specified
    pinned_certificate: Option<reqwest::Certificate>,
    /// Optional DNS override for the whole fetch. The daemon passes one that
    /// answers the API host from its address cache, so an update check still
    /// resolves while the blocking state drops every DNS query on the host
    /// (the metadata is served from the API host, which the firewall permits
    /// by IP).
    dns_resolver: Option<Arc<dyn reqwest::dns::Resolve>>,
    /// If set, the response metadata will be serialized and written to this path
    dump_to_path: Option<PathBuf>,
}

impl VersionInfoProvider for HttpVersionInfoProvider {
    async fn get_version_info(&self, params: &VersionParameters) -> anyhow::Result<VersionInfo> {
        let response = self.get_versions(params.lowest_metadata_version).await?;
        VersionInfo::try_from_response(params, response.signed)
    }

    fn set_metadata_dump_path(&mut self, path: PathBuf) {
        self.dump_to_path = Some(path);
    }
}

impl From<MetaRepositoryPlatform> for HttpVersionInfoProvider {
    /// Construct an [HttpVersionInfoProvider] for the given platform using reasonable defaults.
    ///
    /// By default, `pinned_certificate` will be set to the LE root certificate.
    fn from(platform: MetaRepositoryPlatform) -> Self {
        HttpVersionInfoProvider {
            url: platform.url(),
            // Unused by Warren: `API_PINNED_IP` is `None`, so there is no
            // fixed IP to resolve the update host to. A caller that needs to
            // skip DNS supplies a resolver instead ([`Self::with_dns_resolver`]).
            resolve: None,
            pinned_certificate: Some(defaults::PINNED_CERTIFICATE.clone()),
            dns_resolver: None,
            dump_to_path: None,
        }
    }
}

impl HttpVersionInfoProvider {
    /// Maximum size of the GET response, in bytes
    const SIZE_LIMIT: usize = 1024 * 1024;

    /// Bound on establishing the connection (DNS + TCP + TLS).
    const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    /// Bound on the whole request. The version router defers in-app
    /// downloads on this fetch and the poller runs it single-flight, so a
    /// stalled connection with no deadline wedges the update flow and every
    /// pending version request until the daemon restarts. The manifest is
    /// tens of kilobytes; a minute is generous on any usable link.
    const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

    /// Retrieve version metadata for the given platform using reasonable defaults.
    ///
    /// By default, `pinned_certificate` will be set to the LE root certificate, and
    /// `verifying_keys` will be set to the keys in `trusted-metadata-signing-keys`.
    pub async fn get_versions_for_platform(
        platform: MetaRepositoryPlatform,
        lowest_metadata_version: usize,
    ) -> anyhow::Result<SignedResponse> {
        HttpVersionInfoProvider::from(platform)
            .get_versions(lowest_metadata_version)
            .await
    }

    /// Resolves the metadata host through `dns_resolver` instead of the
    /// system resolver. Certificate validation is untouched: the request still
    /// carries the host name, so the pinned root and the host name check apply
    /// exactly as before.
    #[must_use]
    pub fn with_dns_resolver(
        mut self,
        dns_resolver: Option<Arc<dyn reqwest::dns::Resolve>>,
    ) -> Self {
        self.dns_resolver = dns_resolver;
        self
    }

    /// Download and verify signed data with sane defaults
    ///
    /// By default, `pinned_certificate` will be set to the LE root certificate, and
    /// and the keys in `trusted-metadata-signing-keys` will be used for verification.
    pub async fn get_versions(
        &self,
        lowest_metadata_version: usize,
    ) -> anyhow::Result<SignedResponse> {
        self.get_versions_inner(|raw_json| {
            SignedResponse::deserialize_and_verify(raw_json, lowest_metadata_version)
        })
        .await
    }

    /// Download and verify signed data with the given keys
    #[cfg(test)]
    async fn get_versions_with_keys(
        &self,
        lowest_metadata_version: usize,
        verifying_keys: &Vec1<crate::format::key::VerifyingKey>,
    ) -> anyhow::Result<SignedResponse> {
        self.get_versions_inner(|raw_json| {
            SignedResponse::deserialize_and_verify_at_time(
                verifying_keys,
                raw_json,
                chrono::DateTime::UNIX_EPOCH,
                lowest_metadata_version,
            )
        })
        .await
    }

    async fn get_versions_inner(
        &self,
        deserialize_fn: impl FnOnce(&[u8]) -> anyhow::Result<SignedResponse>,
    ) -> anyhow::Result<SignedResponse> {
        let raw_json = Self::get(
            &self.url,
            self.pinned_certificate.clone(),
            self.resolve,
            self.dns_resolver.clone(),
        )
        .await?;
        let signed_response = deserialize_fn(&raw_json)?;
        if let Some(path) = &self.dump_to_path {
            fs::write(path, raw_json)
                .await
                .context("Failed to save cache")?;
        }
        Ok(signed_response)
    }

    /// Retrieve the `latest.json` file.
    ///
    /// - `pinned_certificate` will be set to the LE root certificate.
    /// - DNS will be used to look up the URL.
    /// - The JSON response is not signed.
    ///
    /// Resolves the metadata endpoint through [`defaults::metadata_url`]
    /// so the `WARREN_METADATA_URL` env var override (and the Warren
    /// default when the var is unset) take precedence over the upstream
    /// Mullvad constant.
    pub async fn get_latest_versions_file() -> anyhow::Result<String> {
        Self::get(
            &format!("{}/latest.json", defaults::metadata_url()),
            Some(defaults::PINNED_CERTIFICATE.clone()),
            None,
            None,
        )
        .await
        .and_then(|raw_json: Vec<u8>| Ok(String::from_utf8(raw_json)?))
        .context("Failed to get latest.json file")
    }

    /// Perform a simple GET request, with a size limit, and return it as bytes
    ///
    /// # Arguments
    /// `url` - URL to fetch
    /// `pinned_certificate` - Optional pinned certificate for TLS verification
    /// `resolve` - Optional host to resolve (to the IP) without DNS
    /// `dns_resolver` - Optional resolver for every other name in the request
    async fn get(
        url: &str,
        pinned_certificate: Option<reqwest::Certificate>,
        resolve: Option<(&'static str, IpAddr)>,
        dns_resolver: Option<Arc<dyn reqwest::dns::Resolve>>,
    ) -> anyhow::Result<Vec<u8>> {
        let mut req_builder = reqwest::Client::builder()
            .connect_timeout(Self::CONNECT_TIMEOUT)
            .timeout(Self::REQUEST_TIMEOUT);
        req_builder = req_builder.min_tls_version(reqwest::tls::Version::TLS_1_3);

        if let Some(pinned_certificate) = pinned_certificate {
            req_builder = req_builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(pinned_certificate);
        }

        // Resolve name without DNS
        if let Some((host, addr)) = resolve {
            req_builder = req_builder.resolve(host, (addr, 0).into());
        }

        // Resolve every name through the caller's resolver (the daemon's
        // address cache) instead of the system one.
        if let Some(dns_resolver) = dns_resolver {
            req_builder = req_builder.dns_resolver2(dns_resolver);
        }

        // Initiate GET request
        let mut req = req_builder
            .build()?
            .get(url)
            .send()
            .await
            .context("Failed to fetch version")?;

        // Fail if content length exceeds limit
        let content_len_limit = Self::SIZE_LIMIT.try_into().expect("Invalid size limit");
        if req.content_length() > Some(content_len_limit) {
            anyhow::bail!("Version info exceeded limit: {} bytes", Self::SIZE_LIMIT);
        }

        if let Err(err) = req.error_for_status_ref() {
            return Err(err).context("GET request failed");
        }

        let mut read_n = 0;
        let mut data = vec![];

        while let Some(chunk) = req.chunk().await.context("Failed to retrieve chunk")? {
            read_n += chunk.len();

            // Fail if content length exceeds limit
            if read_n > Self::SIZE_LIMIT {
                anyhow::bail!("Version info exceeded limit: {} bytes", Self::SIZE_LIMIT);
            }

            data.extend_from_slice(&chunk);
        }

        Ok(data)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use async_tempfile::TempDir;
    use insta::assert_yaml_snapshot;
    use vec1::vec1;

    // These tests rely on `insta` for snapshot testing. If they fail due to snapshot assertions,
    // then most likely the snapshots need to be updated. The most convenient way to review
    // changes to, and update, snapshots is by running `cargo insta review`.

    /// Stands in for the daemon's address-cache resolver: answers exactly one
    /// host and refuses everything else.
    struct FixedResolver {
        host: &'static str,
        addr: std::net::SocketAddr,
    }

    impl reqwest::dns::Resolve for FixedResolver {
        fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
            let answer = name
                .as_str()
                .eq_ignore_ascii_case(self.host)
                .then_some(self.addr);
            Box::pin(async move {
                match answer {
                    Some(addr) => Ok(Box::new(std::iter::once(addr)) as reqwest::dns::Addrs),
                    None => Err(Box::<dyn std::error::Error + Send + Sync>::from(
                        "host not in the cache",
                    )),
                }
            })
        }
    }

    /// The update check must reach the metadata host through a supplied
    /// resolver, with no DNS query at all: that is what keeps it alive while
    /// the kill switch blocks DNS. `.test` is reserved by RFC 6761 and never
    /// resolves, so a verified response can only have come from the resolver.
    #[tokio::test]
    async fn version_metadata_is_fetched_through_the_supplied_resolver() -> anyhow::Result<()> {
        let valid_key =
            crate::format::key::VerifyingKey::from_hex(include_str!("../../test-pubkey"))
                .expect("valid key");
        let verifying_keys = vec1![valid_key];

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/version")
            .with_body(include_bytes!("../../test-version-response.json"))
            .create();

        let host = "api.beta.warrenbrowse.test";
        let url = format!("http://{host}:{}/version", server.socket_address().port());

        let info_provider = HttpVersionInfoProvider {
            url,
            pinned_certificate: None,
            resolve: None,
            dns_resolver: None,
            dump_to_path: None,
        }
        .with_dns_resolver(Some(Arc::new(FixedResolver {
            host,
            addr: server.socket_address(),
        })));

        info_provider
            .get_versions_with_keys(0, &verifying_keys)
            .await
            .context("the resolver must reach the metadata host without any DNS query")?;

        mock.assert();
        Ok(())
    }

    /// A metadata fetch against a host that accepts the connection and then
    /// never answers must fail on its own. The version router defers in-app
    /// downloads on this fetch and the background poller runs it
    /// single-flight, so a fetch with no deadline wedges the whole update
    /// flow forever (the GUI sits on "Starting download..." at 0%).
    #[tokio::test(start_paused = true)]
    async fn a_stalled_metadata_fetch_fails_instead_of_hanging() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        // Hold every accepted connection open without ever writing a byte.
        let server = tokio::spawn(async move {
            let mut connections = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.push(stream);
            }
        });

        let url = format!("http://{addr}/windows.json");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(600),
            HttpVersionInfoProvider::get(&url, None, None, None),
        )
        .await
        .expect("the fetch must give up on its own, not hang past its deadline");
        result.expect_err("a server that never answers cannot yield metadata");

        server.abort();
    }

    /// Test HTTP version info provider
    ///
    /// We're not testing the correctness of [version] here, only the HTTP client
    #[tokio::test]
    async fn test_http_version_provider() -> anyhow::Result<()> {
        let valid_key =
            crate::format::key::VerifyingKey::from_hex(include_str!("../../test-pubkey"))
                .expect("valid key");
        let verifying_keys = vec1![valid_key];

        // Start HTTP server
        let mut server = mockito::Server::new_async().await;
        let _mock: mockito::Mock = server
            .mock("GET", "/version")
            // Respond with some version response payload
            .with_body(include_bytes!("../../test-version-response.json"))
            .create();

        // Resolve some host to our mockito server
        let host = "fakeurl.biz";
        let url = format!("http://{host}:{}/version", server.socket_address().port());
        let resolve = (host, server.socket_address().ip());

        let temp_dump_dir = TempDir::new().await.unwrap();
        let temp_dump = temp_dump_dir.join("metadata.json");

        // Construct query and provider
        let info_provider = HttpVersionInfoProvider {
            url,
            pinned_certificate: None,
            resolve: Some(resolve),
            dns_resolver: None,
            dump_to_path: Some(temp_dump.clone()),
        };

        let info = info_provider
            .get_versions_with_keys(0, &verifying_keys)
            .await
            .context("Expected valid version info")?;

        // Expect: Our query should yield some version response
        assert_yaml_snapshot!(info);

        // Expect: Dumped data should exist and look the same
        let cached_data = fs::read(temp_dump).await.expect("expected dumped info");
        let cached_info = SignedResponse::deserialize_and_verify_at_time(
            &verifying_keys,
            &cached_data,
            chrono::DateTime::UNIX_EPOCH,
            0,
        )
        .unwrap();
        assert_eq!(cached_info, info);

        Ok(())
    }
}
