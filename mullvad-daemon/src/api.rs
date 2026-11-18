use std::net::SocketAddr;

use futures::{StreamExt, channel::mpsc};
use mullvad_api::AddressCache;
use mullvad_api::FileAddressCacheBacking;
use mullvad_api::{
    access_mode::AccessMethodResolver,
    availability::ApiAvailability,
    proxy::{ApiConnectionMode, ProxyConfig},
};
use mullvad_management_interface::async_trait;
use mullvad_types::access_method::{AccessMethod, BuiltInAccessMethod};
use talpid_types::net::AllowedEndpoint;
use talpid_types::net::Endpoint;
use talpid_types::net::TransportProtocol;
use talpid_types::net::{AllowedClients, Connectivity};

pub struct DaemonAccessMethodResolver {
    address_cache: AddressCache<FileAddressCacheBacking>,
}

impl DaemonAccessMethodResolver {
    pub fn new(address_cache: AddressCache<FileAddressCacheBacking>) -> Self {
        Self { address_cache }
    }
}

#[async_trait]
impl AccessMethodResolver for DaemonAccessMethodResolver {
    async fn resolve_access_method_setting(
        &mut self,
        access_method: &AccessMethod,
    ) -> Option<(AllowedEndpoint, ApiConnectionMode)> {
        let connection_mode = {
            match access_method {
                AccessMethod::BuiltIn(BuiltInAccessMethod::Direct) => ApiConnectionMode::Direct,
                AccessMethod::Custom(config) => {
                    ApiConnectionMode::Proxied(ProxyConfig::from(config.clone()))
                }
            }
        };
        let endpoint =
            resolve_allowed_endpoint(&connection_mode, self.address_cache.get_address().await);
        Some((endpoint, connection_mode))
    }

    async fn default_connection_mode(&self) -> AllowedEndpoint {
        log::trace!("Defaulting to direct API connection");
        resolve_allowed_endpoint(
            &ApiConnectionMode::Direct,
            self.address_cache.get_address().await,
        )
    }
}

pub fn resolve_allowed_endpoint(
    connection_mode: &ApiConnectionMode,
    fallback: SocketAddr,
) -> AllowedEndpoint {
    let endpoint = match connection_mode.get_endpoint() {
        Some(endpoint) => endpoint,
        None => Endpoint::from_socket_address(fallback, TransportProtocol::Tcp),
    };
    let clients = allowed_clients(connection_mode);
    AllowedEndpoint { endpoint, clients }
}

#[cfg(unix)]
pub fn allowed_clients(connection_mode: &ApiConnectionMode) -> AllowedClients {
    match connection_mode {
        ApiConnectionMode::Proxied(ProxyConfig::Socks5Local(_)) => AllowedClients::All,
        ApiConnectionMode::Direct | ApiConnectionMode::Proxied(_) => AllowedClients::Root,
    }
}

#[cfg(windows)]
pub fn allowed_clients(connection_mode: &ApiConnectionMode) -> AllowedClients {
    match connection_mode {
        ApiConnectionMode::Proxied(ProxyConfig::Socks5Local(_)) => AllowedClients::all(),
        ApiConnectionMode::Direct | ApiConnectionMode::Proxied(_) => {
            let daemon_exe = std::env::current_exe().expect("failed to obtain executable path");
            vec![
                daemon_exe
                    .parent()
                    .expect("missing executable parent directory")
                    .join("warren-problem-report.exe"),
                daemon_exe,
            ]
            .into()
        }
    }
}

/// Forwards the received values from `offline_state_rx` to the [`ApiAvailability`].
///
/// `online_edge_tx` is bumped on every offline→online transition so
/// background fetchers (the Warren multi-hop directory updater and exit
/// list updater) can force an immediate refresh the moment connectivity
/// returns, instead of waiting out their coarse periodic timers. This is
/// the sleep/wake recovery path: after a wake the network is briefly
/// unreachable and the periodic timers (whose monotonic clock does not
/// advance during sleep on macOS) would otherwise leave the daemon on a
/// stale directory for many minutes.
pub(crate) fn forward_offline_state(
    api_availability: ApiAvailability,
    mut offline_state_rx: mpsc::UnboundedReceiver<Connectivity>,
    online_edge_tx: tokio::sync::watch::Sender<u64>,
    warren_status_cache: crate::warren_status::WarrenStatusCache,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Some(initial) = offline_state_rx.next().await else {
            log::debug!("The offline monitor closed before reporting a state");
            return;
        };
        let mut was_offline = initial.is_offline();
        log::info!(
            "Initial offline state - {state}",
            state = if was_offline { "offline" } else { "online" },
        );
        api_availability.set_offline(was_offline);
        warren_status_cache.set_host_offline(was_offline);

        while let Some(state) = offline_state_rx.next().await {
            log::info!("Detecting changes to offline state - {state:?}");
            let is_offline = state.is_offline();
            // Offline→online edge: signal the Warren background updaters.
            if was_offline && !is_offline {
                online_edge_tx.send_modify(|n| *n = n.wrapping_add(1));
            }
            was_offline = is_offline;
            api_availability.set_offline(is_offline);
            // Push the verdict to the UI on the edge itself: the tunnel
            // state machine holds Connected through its migration grace,
            // so this flag is the user's only immediate signal.
            warren_status_cache.set_host_offline(is_offline);
        }
    })
}

#[cfg(test)]
mod offline_state_tests {
    use super::*;

    /// The offline monitor can go away before it reports anything (a failed start, a
    /// shutdown): the forwarder then ends quietly instead of panicking in a detached task.
    #[tokio::test]
    async fn a_monitor_that_closes_before_its_first_state_ends_the_forwarder() {
        let (tx, rx) = mpsc::unbounded();
        drop(tx);
        let (online_edge_tx, _online_edge_rx) = tokio::sync::watch::channel(0);

        let forwarder = forward_offline_state(
            ApiAvailability::default(),
            rx,
            online_edge_tx,
            crate::warren_status::WarrenStatusCache::default(),
        );

        assert!(forwarder.await.is_ok(), "the forwarder panicked");
    }
}
