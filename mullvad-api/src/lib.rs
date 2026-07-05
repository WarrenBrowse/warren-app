#![allow(rustdoc::private_intra_doc_links)]
use async_trait::async_trait;
#[cfg(target_os = "android")]
use futures::channel::mpsc;
use hyper::body::Incoming;
use mullvad_types::account::{AccountData, AccountNumber, VoucherSubmission};
#[cfg(target_os = "android")]
use mullvad_types::account::{PlayExternalObfuscatedAccountId, PlayPurchase};
use proxy::{ApiConnectionMode, ConnectionModeProvider};
use std::{collections::BTreeMap, future::Future, io, net::SocketAddr, path::Path, sync::Arc};
use talpid_types::ErrorExt;

pub mod availability;
use availability::ApiAvailability;
pub mod rest;
#[cfg(not(target_os = "ios"))]
pub mod version;

mod abortable_stream;
pub mod access_mode;
mod https_client;
pub mod proxy;
mod tls_stream;
#[cfg(target_os = "android")]
pub use crate::https_client::SocketBypassRequest;

mod access;
mod address_cache;
mod device;
mod relay_list;
/// Auth wallet: Ed25519 signature on HTTP API requests.
/// See `warren-core/docs/06-auth-wallet.md`.
pub mod warren_auth;

pub use address_cache::Error as AddressCacheError;
pub use address_cache::{AddressCache, AddressCacheBacking, FileAddressCacheBacking};

pub use device::DevicesProxy;
pub use hyper::StatusCode;
pub use relay_list::{CachedRelayList, ETag, RelayListProxy};

/// Error code returned by the Mullvad API if the voucher has alreaby been used.
pub const VOUCHER_USED: &str = "VOUCHER_USED";

/// Error code returned by the Mullvad API if the voucher code is invalid.
pub const INVALID_VOUCHER: &str = "INVALID_VOUCHER";

/// Warren: the voucher is past its deadline or was revoked by the admin.
pub const VOUCHER_EXPIRED: &str = "VOUCHER_EXPIRED";

/// Warren: the app-initiated purchase (wpid) has no queued voucher yet;
/// the GUI keeps polling.
pub const VOUCHER_NOT_READY: &str = "VOUCHER_NOT_READY";

/// Error code returned by the Mullvad API if the account number is invalid.
pub const INVALID_ACCOUNT: &str = "INVALID_ACCOUNT";

/// Error code returned by the Mullvad API if the device does not exist.
pub const DEVICE_NOT_FOUND: &str = "DEVICE_NOT_FOUND";

/// Error code returned by the Mullvad API if the access token is invalid.
pub const INVALID_ACCESS_TOKEN: &str = "INVALID_ACCESS_TOKEN";

pub const MAX_DEVICES_REACHED: &str = "MAX_DEVICES_REACHED";
pub const PUBKEY_IN_USE: &str = "PUBKEY_IN_USE";

pub const API_IP_CACHE_FILENAME: &str = "api-ip-address.txt";

const ACCOUNTS_URL_PREFIX: &str = "accounts/v1";
const APP_URL_PREFIX: &str = "app/v1";

#[cfg(target_os = "ios")]
const APPLE_PAYMENT_URL_PREFIX: &str = "payments/apple/v2";

#[cfg(target_os = "android")]
const GOOGLE_PAYMENTS_URL_PREFIX: &str = "payments/google-play/v1";

use mullvad_api_constants::*;

/// A hostname and socketaddr to reach the Mullvad REST API over.
#[derive(Debug, Clone)]
pub struct ApiEndpoint {
    /// An overriden API hostname. Initialized with the value of the environment
    /// variable `MULLVAD_API_HOST` if it has been set.
    ///
    /// Use the associated function [`Self::host`] to read this value with a
    /// default fallback if `MULLVAD_API_HOST` was not set.
    pub host: Option<String>,
    /// An overriden API address. Initialized with the value of the environment
    /// variable `MULLVAD_API_ADDR` if it has been set.
    ///
    /// Use the associated function [`Self::address`] to read this value with
    /// a default fallback if `MULLVAD_API_ADDR` was not set.
    ///
    /// # Note
    ///
    /// If [`Self::address`] is populated with `Some(SocketAddr)`, it should
    /// always be respected when establishing API connections.
    pub address: Option<SocketAddr>,
    #[cfg(any(feature = "api-override", test))]
    pub disable_tls: bool,
    #[cfg(feature = "api-override")]
    /// Whether bridges/proxies can be used to access the API or not. This is
    /// useful primarily for testing purposes.
    ///
    /// * If `force_direct` is `true`, bridges and proxies will not be used to reach the API.
    /// * If `force_direct` is `false`, bridges and proxies can be used to reach the API.
    ///
    /// # Note
    ///
    /// By default, `force_direct` will be `true` if the `api-override` feature
    /// is enabled and overrides are in use. This is supposedly less error prone, as
    /// common targets such as Devmole might be unreachable from behind a bridge server.
    ///
    /// To disable `force_direct`, set the environment variable
    /// `MULLVAD_API_FORCE_DIRECT=0` before starting the daemon.
    pub force_direct: bool,
}

impl ApiEndpoint {
    /// Returns the endpoint to connect to the API over.
    ///
    /// # Panics
    ///
    /// Panics if `MULLVAD_API_ADDR`, `MULLVAD_API_HOST` or
    /// `MULLVAD_API_DISABLE_TLS` has invalid contents.
    #[cfg(feature = "api-override")]
    pub fn from_env_vars() -> ApiEndpoint {
        let host_var = Self::read_var(env::API_HOST_VAR);
        let address_var = Self::read_var(env::API_ADDR_VAR);
        let disable_tls_var = Self::read_var(env::DISABLE_TLS_VAR);
        let force_direct = Self::read_var(env::API_FORCE_DIRECT_VAR);

        let mut api = ApiEndpoint {
            host: None,
            address: None,
            disable_tls: false,
            force_direct: force_direct
                .map(|force_direct| force_direct != "0")
                .unwrap_or_else(|| host_var.is_some() || address_var.is_some()),
        };

        match (host_var, address_var) {
            (None, None) => {}
            (Some(host), None) => {
                use std::net::ToSocketAddrs;
                log::debug!(
                    "{api_addr} not found. Resolving API IP address from {api_host}={host}",
                    api_addr = env::API_ADDR_VAR,
                    api_host = env::API_HOST_VAR
                );
                api.address = format!("{host}:{API_PORT_DEFAULT}")
                    .to_socket_addrs()
                    .unwrap_or_else(|_| {
                        panic!(
                            "Unable to resolve API IP address from host {host}:{API_PORT_DEFAULT}"
                        )
                    })
                    .next();
                api.host = Some(host);
            }
            (host, Some(address)) => {
                let addr = address.parse().unwrap_or_else(|_| {
                    panic!(
                        "{api_addr}={address} is not a valid socketaddr",
                        api_addr = env::API_ADDR_VAR,
                    )
                });
                api.address = Some(addr);
                api.host = host;
            }
        }

        if api.host.is_none() && api.address.is_none() {
            if disable_tls_var.is_some() {
                log::warn!(
                    "{disable_tls} is ignored since {api_host} and {api_addr} are not set",
                    disable_tls = env::DISABLE_TLS_VAR,
                    api_host = env::API_HOST_VAR,
                    api_addr = env::API_ADDR_VAR,
                );
            }
        } else {
            api.disable_tls = disable_tls_var
                .as_ref()
                .map(|disable_tls| disable_tls != "0")
                .unwrap_or(api.disable_tls);

            log::debug!(
                "Overriding API. Using {host} at {scheme}{addr} (force direct={direct})",
                host = api.host(),
                addr = api.address(),
                scheme = if api.disable_tls {
                    "http://"
                } else {
                    "https://"
                },
                direct = api.force_direct,
            );
        }
        api
    }

    #[cfg(feature = "api-override")]
    pub fn should_disable_address_cache(&self) -> bool {
        self.host.is_some() || self.address.is_some()
    }

    /// Returns the endpoint to connect to the API over.
    ///
    /// # Panics
    ///
    /// Panics if `MULLVAD_API_ADDR`, `MULLVAD_API_HOST` or
    /// `MULLVAD_API_DISABLE_TLS` has invalid contents.
    #[cfg(not(feature = "api-override"))]
    pub fn from_env_vars() -> ApiEndpoint {
        let env_vars = [
            env::API_HOST_VAR,
            env::API_ADDR_VAR,
            env::DISABLE_TLS_VAR,
            env::API_FORCE_DIRECT_VAR,
        ];

        if env_vars.map(Self::read_var).iter().any(Option::is_some) {
            log::warn!(
                "These variables are ignored in production builds: {env_vars_pretty}",
                env_vars_pretty = env_vars.join(", ")
            );
        }

        ApiEndpoint {
            host: None,
            address: None,
            #[cfg(test)]
            disable_tls: false,
        }
    }

    /// Returns a new API endpoint with the given host and socket address.
    pub fn new(
        host: String,
        address: SocketAddr,
        #[cfg(any(feature = "api-override", test))] disable_tls: bool,
    ) -> Self {
        Self {
            host: Some(host),
            address: Some(address),
            #[cfg(any(feature = "api-override", test))]
            disable_tls,
            #[cfg(feature = "api-override")]
            force_direct: false,
        }
    }

    pub fn set_addr(&mut self, address: SocketAddr) {
        self.address = Some(address);
    }

    /// Read the [`Self::host`] value, falling back to
    /// [`API_HOST_DEFAULT`] as default value if it does not exist.
    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or(API_HOST_DEFAULT)
    }

    /// Read the [`Self::address`] value. Resolution order:
    /// 1. an explicit override (`MULLVAD_API_ADDR`) if set;
    /// 2. the pinned [`API_PINNED_IP`] if configured (no DNS query - the
    ///    bootstrap-privacy path, `None` by default);
    /// 3. otherwise resolve [`API_HOST_DEFAULT`] via system DNS, falling back
    ///    to the [`API_IP_DEFAULT`] sentinel if that fails.
    pub fn address(&self) -> SocketAddr {
        // Explicit override (MULLVAD_API_ADDR), then a pinned IP - both skip
        // DNS. `API_PINNED_IP` is `None` by default, so this normally falls
        // through to system DNS (unchanged behaviour).
        if let Some(addr) = Self::pinned_or_explicit(self.address, API_PINNED_IP) {
            return addr;
        }
        use std::net::ToSocketAddrs;
        (API_HOST_DEFAULT, API_PORT_DEFAULT)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .unwrap_or(SocketAddr::new(API_IP_DEFAULT, API_PORT_DEFAULT))
    }

    /// Pure resolution-order helper (testable): an explicit override wins,
    /// then the pinned IP; `None` means the caller must resolve via DNS.
    fn pinned_or_explicit(
        explicit: Option<SocketAddr>,
        pinned: Option<std::net::IpAddr>,
    ) -> Option<SocketAddr> {
        explicit.or_else(|| pinned.map(|ip| SocketAddr::new(ip, API_PORT_DEFAULT)))
    }

    /// Try to read the value of an environment variable. Returns `None` if the
    /// environment variable has not been set.
    ///
    /// # Panics
    ///
    /// Panics if the environment variable was found, but it did not contain
    /// valid unicode data.
    fn read_var(key: &'static str) -> Option<String> {
        use std::env;
        match env::var(key) {
            Ok(v) => Some(v),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => panic!("{key} does not contain valid UTF-8"),
        }
    }
}

#[async_trait]
pub trait DnsResolver: 'static + Send + Sync {
    async fn resolve(&self, host: String) -> io::Result<Vec<SocketAddr>>;
}

/// DNS resolver that relies on `ToSocketAddrs` (`getaddrinfo`).
pub struct DefaultDnsResolver;

#[async_trait]
impl DnsResolver for DefaultDnsResolver {
    async fn resolve(&self, host: String) -> io::Result<Vec<SocketAddr>> {
        use std::net::ToSocketAddrs;
        // Spawn a blocking thread, since `to_socket_addrs` relies on `libc::getaddrinfo`, which
        // blocks and either has no timeout or a very long one.
        let addrs = tokio::task::spawn_blocking(move || (host, 0).to_socket_addrs())
            .await
            .expect("DNS task panicked")?;
        Ok(addrs.collect())
    }
}

/// DNS resolver that always returns no results
pub struct NullDnsResolver;

#[async_trait]
impl DnsResolver for NullDnsResolver {
    async fn resolve(&self, _host: String) -> io::Result<Vec<SocketAddr>> {
        Ok(vec![])
    }
}

/// Startup reachability-probe budget for a pinned API IP before falling back
/// to DNS. Short: a live pinned IP answers a TCP SYN in well under this; the
/// budget only bites on the rare dead-pin path.
const PINNED_IP_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// True if a TCP connection to `addr` completes within `dur`.
async fn tcp_reachable(addr: SocketAddr, dur: std::time::Duration) -> bool {
    matches!(
        tokio::time::timeout(dur, tokio::net::TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}

/// Bootstrap-privacy resilience for a pinned API IP (`API_PINNED_IP`).
///
/// When a pin is in effect, the cache was seeded from the pinned IP (or a
/// stale on-disk cache). Probe that seed; if it is unreachable at startup,
/// re-seed from DNS so a dead/rotated pin cannot brick the app. The privacy
/// benefit (no DNS query) is given up only on this failure path. No-op when no
/// pin is in effect (`API_PINNED_IP == None` or an explicit address override),
/// so default behaviour is unchanged.
async fn seed_pinned_or_dns_fallback<T: address_cache::AddressCacheBacking>(
    endpoint: &ApiEndpoint,
    cache: &address_cache::AddressCache<T>,
) {
    if endpoint.address.is_some() || API_PINNED_IP.is_none() {
        return;
    }
    let seeded = cache.get_address().await;
    if tcp_reachable(seeded, PINNED_IP_PROBE_TIMEOUT).await {
        return;
    }
    match DefaultDnsResolver.resolve(endpoint.host().to_owned()).await {
        Ok(addrs) if !addrs.is_empty() => {
            let a = addrs[0];
            let port = if a.port() == 0 {
                API_PORT_DEFAULT
            } else {
                a.port()
            };
            let dns_addr = SocketAddr::new(a.ip(), port);
            log::warn!(
                "pinned API IP {seeded} unreachable at startup; falling back to DNS-resolved {dns_addr}"
            );
            if let Err(e) = cache.set_address(dns_addr).await {
                log::error!("failed to apply DNS fallback API address: {e}");
            }
        }
        _ => log::warn!(
            "pinned API IP {seeded} unreachable and DNS fallback failed; keeping pinned address"
        ),
    }
}

/// A type that helps with the creation of API connections.
pub struct Runtime<B = FileAddressCacheBacking>
where
    B: AddressCacheBacking,
{
    handle: tokio::runtime::Handle,
    address_cache: Arc<AddressCache<B>>,
    api_availability: availability::ApiAvailability,
    endpoint: ApiEndpoint,
    #[cfg(target_os = "android")]
    socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to construct a rest client")]
    RestError(#[from] rest::Error),

    #[error("Failed to load address cache")]
    AddressCacheError(#[from] address_cache::Error),

    #[error("API availability check failed")]
    ApiCheckError(#[from] availability::Error),

    #[error("DNS resolution error")]
    ResolutionFailed(#[from] std::io::Error),
}

impl Runtime {
    /// Will create a new Runtime without a cache with the provided API endpoint.
    pub fn new(
        handle: tokio::runtime::Handle,
        endpoint: &ApiEndpoint,
        #[cfg(target_os = "android")] socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
    ) -> Self {
        let address_cache = Arc::new(AddressCache::new(endpoint, None));
        Runtime {
            handle,
            address_cache,
            api_availability: ApiAvailability::default(),
            endpoint: endpoint.clone(),
            #[cfg(target_os = "android")]
            socket_bypass_tx,
        }
    }

    /// Create a new `Runtime` using the specified directories.
    /// Try to use the cache directory first, and fall back on the bundled address otherwise.
    /// Will try to construct an API endpoint from the environment.
    pub async fn with_cache(
        endpoint: &ApiEndpoint,
        cache_dir: &Path,
        write_changes: bool,
        #[cfg(target_os = "android")] socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
    ) -> Result<Self, Error> {
        let handle = tokio::runtime::Handle::current();

        #[cfg(feature = "api-override")]
        if endpoint.should_disable_address_cache() {
            return Ok(Self::new(
                handle,
                endpoint,
                #[cfg(target_os = "android")]
                socket_bypass_tx,
            ));
        }

        let cache_file = cache_dir.join(API_IP_CACHE_FILENAME);
        let write_file = if write_changes {
            Some(cache_file.clone().into_boxed_path())
        } else {
            None
        };

        let address_cache = match AddressCache::from_file(
            &cache_file,
            write_file.clone(),
            endpoint.host().to_owned(),
        )
        .await
        {
            Ok(cache) => cache,
            Err(error) => {
                if cache_file.exists() {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg(
                            "Failed to load cached API addresses. Falling back on bundled address"
                        )
                    );
                }
                AddressCache::new(endpoint, write_file)
            }
        };
        let address_cache = Arc::new(address_cache);
        // Bootstrap-privacy resilience: a dead/rotated pinned IP must not brick
        // startup. No-op unless an API IP is pinned.
        seed_pinned_or_dns_fallback(endpoint, &address_cache).await;
        let api_availability = ApiAvailability::default();

        Ok(Runtime {
            handle,
            address_cache,
            api_availability,
            endpoint: endpoint.clone(),
            #[cfg(target_os = "android")]
            socket_bypass_tx,
        })
    }

    /// Returns a new request service handle
    pub fn rest_handle(&self, dns_resolver: impl DnsResolver) -> rest::RequestServiceHandle {
        self.new_request_service(
            ApiConnectionMode::Direct.into_provider(),
            Arc::new(dns_resolver),
            #[cfg(target_os = "android")]
            None,
            #[cfg(any(feature = "api-override", test))]
            false,
        )
    }
}

impl<B: AddressCacheBacking> Runtime<B> {
    pub async fn with_cache_backing(
        handle: tokio::runtime::Handle,
        endpoint: &ApiEndpoint,
        backing: Arc<B>,
        #[cfg(target_os = "android")] socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
    ) -> Runtime<B>
    where
        B: AddressCacheBacking,
    {
        let address_cache = Arc::new(
            AddressCache::from_backing_or(endpoint.host().to_owned(), backing, endpoint).await,
        );
        Runtime {
            handle,
            address_cache,
            api_availability: ApiAvailability::default(),
            endpoint: endpoint.clone(),
            #[cfg(target_os = "android")]
            socket_bypass_tx,
        }
    }

    pub fn address_cache(&self) -> &AddressCache<B> {
        &self.address_cache
    }

    /// Returns a request factory initialized to create requests for the master API Assumes an API
    /// endpoint that is constructed from env vars, or uses default values.
    pub fn mullvad_rest_handle<T: ConnectionModeProvider + 'static>(
        &self,
        connection_mode_provider: T,
    ) -> rest::MullvadRestHandle {
        self.mullvad_rest_handle_with_warren_signer(connection_mode_provider, None)
    }

    /// Variant of [`Self::mullvad_rest_handle`] that attaches a
    /// [`rest::WarrenAuthSigner`] on the `RequestFactory` when
    /// `warren_signer` is `Some`. If `None`, behavior is identical
    /// to [`Self::mullvad_rest_handle`] (= legacy Mullvad Bearer
    /// mode).
    ///
    /// The caller (= mullvad-daemon) holds the `Arc<WarrenAuthSigner>`
    /// derived from the BIP39 mnemonic stored locally (see crate
    /// `warren-identity` on the warren-core side).
    pub fn mullvad_rest_handle_with_warren_signer<T: ConnectionModeProvider + 'static>(
        &self,
        connection_mode_provider: T,
        warren_signer: Option<Arc<warren_auth::WarrenAuthSigner>>,
    ) -> rest::MullvadRestHandle {
        let service = self.new_request_service(
            connection_mode_provider,
            Arc::clone(&self.address_cache),
            #[cfg(target_os = "android")]
            self.socket_bypass_tx.clone(),
            #[cfg(any(feature = "api-override", test))]
            self.endpoint.disable_tls,
        );
        let hostname = self.endpoint.host().to_owned();
        let token_store = access::AccessTokenStore::new(service.clone(), hostname.clone());
        // Invariant: the production factory MUST carry an
        // AccessTokenStore so downstream `Request::account(...)` does
        // not return `Error::NoAccessTokenStore`. Catches a future
        // refactor that would inadvertently pass `None` here. The
        // Warren-Remote dispatch in
        // `mullvad-daemon/src/device/mod.rs` additionally bypasses
        // this factory entirely (it routes through `WarrenApiClient`),
        // but legacy Mullvad fallback paths still rely on this
        // invariant.
        let mut factory = rest::RequestFactory::new(hostname, Some(token_store));
        debug_assert!(
            factory.has_access_token_store(),
            "invariant violated: MullvadRestHandle factory must carry a token store"
        );
        if let Some(signer) = warren_signer {
            factory = factory.with_warren_signer(signer);
        }

        rest::MullvadRestHandle::new(service, factory, self.availability_handle())
    }

    /// Creates a new request service and returns a handle to it.
    fn new_request_service<T: ConnectionModeProvider + 'static>(
        &self,
        connection_mode_provider: T,
        dns_resolver: Arc<impl DnsResolver>,
        #[cfg(target_os = "android")] socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
        #[cfg(any(feature = "api-override", test))] disable_tls: bool,
    ) -> rest::RequestServiceHandle {
        rest::RequestService::spawn(
            self.api_availability.clone(),
            connection_mode_provider,
            dns_resolver,
            #[cfg(target_os = "android")]
            socket_bypass_tx,
            #[cfg(any(feature = "api-override", test))]
            disable_tls,
        )
    }

    pub fn handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }

    pub fn availability_handle(&self) -> ApiAvailability {
        self.api_availability.clone()
    }
}

#[derive(Clone)]
pub struct AccountsProxy {
    handle: rest::MullvadRestHandle,
}

impl AccountsProxy {
    pub fn new(handle: rest::MullvadRestHandle) -> Self {
        Self { handle }
    }

    pub fn get_data(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<AccountData, rest::Error>> + use<> {
        let request = self.get_data_response(account);

        async move { request.await?.deserialize().await }
    }

    pub fn get_data_response(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            // `get_or_signed` injects the 4 `X-Warren-*` headers when
            // a signer is configured (see
            // `mullvad_rest_handle_with_warren_signer` on the daemon side).
            // Otherwise, plain request identical to the legacy path.
            let request = factory
                .get_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/accounts/me"))?
                .expected_status(&[StatusCode::OK])
                .account(account)?;
            service.request(request).await
        }
    }

    pub fn create_account(
        &self,
    ) -> impl Future<Output = Result<AccountNumber, rest::Error>> + use<> {
        #[derive(serde::Deserialize)]
        struct AccountCreationResponse {
            number: AccountNumber,
        }

        let request = self.create_account_response();

        async move {
            let account: AccountCreationResponse = request.await?.deserialize().await?;
            Ok(account.number)
        }
    }

    pub fn create_account_response(
        &self,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/accounts"))?
                .expected_status(&[StatusCode::CREATED]);
            service.request(request).await
        }
    }

    pub fn submit_voucher(
        &self,
        account: AccountNumber,
        voucher_code: String,
    ) -> impl Future<Output = Result<VoucherSubmission, rest::Error>> + use<> {
        #[derive(serde::Serialize)]
        struct VoucherSubmission {
            voucher_code: String,
        }

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();
        let submission = VoucherSubmission { voucher_code };

        async move {
            let request = factory
                .post_json_or_signed(&format!("{APP_URL_PREFIX}/submit-voucher"), &submission)?
                .account(account)?
                .expected_status(&[StatusCode::OK]);
            service.request(request).await?.deserialize().await
        }
    }

    pub fn delete_account(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<(), rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .delete_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/accounts/me"))?
                .account(account.clone())?
                .header("Mullvad-Account-Number", &account)?
                .expected_status(&[StatusCode::NO_CONTENT]);

            let _ = service.request(request).await?;
            Ok(())
        }
    }

    #[cfg(target_os = "ios")]
    pub async fn init_storekit_payment(
        &self,
        account: AccountNumber,
    ) -> Result<rest::Response<Incoming>, rest::Error> {
        let request = self
            .handle
            .factory
            .post(&format!("{APPLE_PAYMENT_URL_PREFIX}/init"))?
            .expected_status(&[StatusCode::OK])
            .account(account)?;
        self.handle.service.request(request).await
    }

    #[cfg(target_os = "ios")]
    pub async fn check_storekit_payment(
        &self,
        body: Vec<u8>,
    ) -> Result<rest::Response<Incoming>, rest::Error> {
        let request = self
            .handle
            .factory
            .post_json_bytes(&format!("{APPLE_PAYMENT_URL_PREFIX}/check"), body)?
            .expected_status(&[StatusCode::OK]);
        self.handle.service.request(request).await
    }

    #[cfg(target_os = "android")]
    pub fn init_play_purchase(
        &mut self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<PlayExternalObfuscatedAccountId, rest::Error>> + use<> {
        #[derive(serde::Deserialize)]
        struct PlayPurchaseInitResponse {
            obfuscated_id: String,
        }

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post_json(&format!("{GOOGLE_PAYMENTS_URL_PREFIX}/init"), &())?
                .account(account)?
                .expected_status(&[StatusCode::OK]);
            let response = service.request(request).await?;

            let PlayPurchaseInitResponse { obfuscated_id } = response.deserialize().await?;

            Ok(obfuscated_id)
        }
    }

    #[cfg(target_os = "android")]
    pub fn verify_play_purchase(
        &mut self,
        account: AccountNumber,
        play_purchase: PlayPurchase,
    ) -> impl Future<Output = Result<(), rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post_json(
                    &format!("{GOOGLE_PAYMENTS_URL_PREFIX}/acknowledge"),
                    &play_purchase,
                )?
                .account(account)?
                .expected_status(&[StatusCode::NO_CONTENT]);

            let response = service.request(request).await;
            match response {
                Err(e) => {
                    log::error!("verify_play_purchase failed: #{:?}", e);
                    Err(e)
                }
                Ok(_) => Ok(()),
            }
        }
    }

    pub fn get_www_auth_token(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<String, rest::Error>> + use<> {
        #[derive(serde::Deserialize)]
        struct AuthTokenResponse {
            auth_token: String,
        }

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post(&format!("{APP_URL_PREFIX}/www-auth-token"))?
                .account(account)?
                .expected_status(&[StatusCode::OK]);
            let response = service.request(request).await?;
            let response: AuthTokenResponse = response.deserialize().await?;
            Ok(response.auth_token)
        }
    }
}

pub struct ProblemReportProxy {
    handle: rest::MullvadRestHandle,
}

impl ProblemReportProxy {
    pub fn new(handle: rest::MullvadRestHandle) -> Self {
        Self { handle }
    }

    pub fn problem_report(
        &self,
        email: &str,
        message: &str,
        log: &str,
        metadata: &BTreeMap<String, String>,
    ) -> impl Future<Output = Result<(), rest::Error>> + use<> {
        #[derive(serde::Serialize)]
        struct ProblemReport {
            address: String,
            message: String,
            log: String,
            metadata: BTreeMap<String, String>,
        }

        let report = ProblemReport {
            address: email.to_owned(),
            message: message.to_owned(),
            log: log.to_owned(),
            metadata: metadata.clone(),
        };

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post_json(&format!("{APP_URL_PREFIX}/problem-report"), &report)?
                .expected_status(&[StatusCode::NO_CONTENT]);
            service.request(request).await?;
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct ApiProxy {
    handle: rest::MullvadRestHandle,
}

impl ApiProxy {
    pub fn new(handle: rest::MullvadRestHandle) -> Self {
        Self { handle }
    }

    pub async fn get_api_addrs(&self) -> Result<Vec<SocketAddr>, rest::Error> {
        self.get_api_addrs_response().await?.deserialize().await
    }

    pub async fn get_api_addrs_response(&self) -> Result<rest::Response<Incoming>, rest::Error> {
        let request = self
            .handle
            .factory
            .get(&format!("{APP_URL_PREFIX}/api-addrs"))?
            .expected_status(&[StatusCode::OK]);

        self.handle.service.request(request).await
    }

    /// Check the availablility of `{APP_URL_PREFIX}/api-addrs`.
    pub async fn api_addrs_available(&self) -> Result<bool, rest::Error> {
        let request = self
            .handle
            .factory
            .head(&format!("{APP_URL_PREFIX}/api-addrs"))?
            .expected_status(&[StatusCode::OK]);

        let response = self.handle.service.request(request).await?;
        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod bootstrap_privacy_tests {
    use super::ApiEndpoint;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn pinned_or_explicit_resolution_order() {
        let pinned_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
        let explicit = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)), 8443);

        // No override, no pin → None (caller resolves via DNS = shipped default).
        assert_eq!(ApiEndpoint::pinned_or_explicit(None, None), None);

        // Pinned only → <ip>:443.
        assert_eq!(
            ApiEndpoint::pinned_or_explicit(None, Some(pinned_ip)),
            Some(SocketAddr::new(pinned_ip, 443))
        );

        // Explicit override always wins over the pin.
        assert_eq!(
            ApiEndpoint::pinned_or_explicit(Some(explicit), Some(pinned_ip)),
            Some(explicit)
        );
        assert_eq!(
            ApiEndpoint::pinned_or_explicit(Some(explicit), None),
            Some(explicit)
        );
    }

    #[tokio::test]
    async fn tcp_reachable_distinguishes_open_and_closed() {
        use std::time::Duration;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        // An open listener is reachable.
        assert!(super::tcp_reachable(addr, Duration::from_secs(2)).await);
        // Once closed, the port refuses (RST) → unreachable, drives the
        // DNS-fallback path for a dead pinned IP.
        drop(listener);
        assert!(!super::tcp_reachable(addr, Duration::from_millis(800)).await);
    }
}
