use std::net::SocketAddr;
use std::{future::Future, net::IpAddr, pin::Pin, sync::Arc};

use ed25519_dalek::SigningKey;
use talpid_types::net::wireguard::TunnelParameters;
use talpid_warren_tunnel::{MultiHopConfig, WarrenTunnelParameters};
use tokio::sync::Mutex;

use mullvad_relay_selector::{GetRelay, RelaySelector, WireguardConfig};
use mullvad_types::{
    endpoint::MullvadEndpoint,
    location::GeoIpLocation,
    relay_constraints::RelaySettings,
    settings::{Settings, TunnelOptions},
};
use talpid_core::tunnel_state_machine::TunnelParametersGenerator;
use talpid_types::net::{obfuscation::Obfuscators, wireguard};

use talpid_types::{ErrorExt, net::IpAvailability, tunnel::ParameterGenerationError};

use crate::device::{AccountManagerHandle, Error as DeviceError, PrivateAccountAndDevice};
use crate::warren_query_from_settings::relay_settings_to_warren_query;
use crate::warren_relay_selector::DaemonWarrenRelaySelector;
use crate::warren_status::WarrenStatusCache;
use crate::warren_tunnel_params::{self, AssembleError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Not logged in on a valid device")]
    NoAuthDetails,

    #[error("Failed to select a matching relay")]
    SelectRelay(#[from] mullvad_relay_selector::Error),

    #[error("Failed to resolve hostname for custom relay")]
    ResolveCustomHostname,

    #[error("Failed to get device data")]
    Device(#[from] DeviceError),

    /// Warren mode active (env var `WARREN_TUNNEL=1`) but the Warren
    /// selector is not configured on the generator side.
    #[error("Warren tunnel mode requested but no Warren relay selector configured")]
    WarrenSelectorMissing,

    /// Warren mode active but the `signing_key` could not be loaded
    /// (BIP39 mnemonic absent or corrupted, see `warren_signer`).
    #[error("Warren tunnel mode requested but no Warren signing key available")]
    WarrenSigningKeyMissing,

    /// Failed to assemble `WarrenTunnelParameters` (selection
    /// failed, ...).
    #[error("Failed to assemble Warren tunnel parameters")]
    WarrenAssemble(#[from] AssembleError),
}

#[derive(Clone)]
pub(crate) struct ParametersGenerator(Arc<Mutex<InnerParametersGenerator>>);

struct InnerParametersGenerator {
    relay_selector: RelaySelector,
    relay_settings: RelaySettings,
    tunnel_options: TunnelOptions,
    account_manager: AccountManagerHandle,

    last_generated_relays: Option<LastSelectedRelays>,

    /// Artifacts for the parallel Warren-Iroh path. `None` on a
    /// non-Warren daemon (= WG path only); `Some` when
    /// `warren_mode::is_enabled()` at boot.
    warren_relay_selector: Option<DaemonWarrenRelaySelector>,
    warren_signing_key: Option<SigningKey>,
    /// Multi-hop config loaded at boot from
    /// `<settings_dir>/warren-multihop.json`. `None` when the user has
    /// not opted into multi-hop (no file or `WARREN_MULTI_HOP` env var
    /// unset). Cloned into every `produce_warren_tunnel_params` call so
    /// the multi-hop dispatcher in `talpid-warren-tunnel` can wire up
    /// the `MultiHopSupervisor`.
    warren_multi_hop: Option<MultiHopConfig>,
    /// Live tunnel status cache shared with the gRPC management
    /// interface. Cloned into the reconnect observer passed to the
    /// multi-hop supervisor so a successful reconnect bumps the
    /// `reconnect_count` field that the Electron UI displays.
    warren_status_cache: WarrenStatusCache,
}

impl ParametersGenerator {
    /// Builds a tunnel parameters generator accepting optional
    /// Warren-Iroh artifacts in addition to the WireGuard path.
    ///
    /// If `warren_relay_selector` or `warren_signing_key` are `None`,
    /// `generate_warren_tunnel_params` will return the corresponding
    /// typed error. The WireGuard path remains usable in parallel
    /// regardless of the state of the Warren artifacts.
    #[expect(
        clippy::too_many_arguments,
        reason = "Constructor for the daemon-side params generator: the eight inputs are all required (4 upstream + 4 Warren). Bundling them into a config struct just to satisfy clippy would obscure the call site at lib.rs."
    )]
    pub fn new_with_optional_warren(
        account_manager: AccountManagerHandle,
        relay_selector: RelaySelector,
        relay_settings: RelaySettings,
        tunnel_options: TunnelOptions,
        warren_relay_selector: Option<DaemonWarrenRelaySelector>,
        warren_signing_key: Option<SigningKey>,
        warren_multi_hop: Option<MultiHopConfig>,
        warren_status_cache: WarrenStatusCache,
    ) -> Self {
        Self(Arc::new(Mutex::new(InnerParametersGenerator {
            tunnel_options,
            relay_selector,
            relay_settings,
            account_manager,
            last_generated_relays: None,
            warren_relay_selector,
            warren_signing_key,
            warren_multi_hop,
            warren_status_cache,
        })))
    }

    /// Assembles a [`WarrenTunnelParameters`] for the
    /// `retry_attempt` attempt, from the stored Warren artifacts.
    ///
    /// Asymmetric mirror API of
    /// [`TunnelParametersGenerator::generate`] (which produces WG):
    /// the state machine picks one OR the other depending on
    /// `warren_mode::is_enabled`.
    ///
    /// # Errors
    ///
    /// - [`Error::WarrenSelectorMissing`] if the Warren selector was
    ///   not configured at boot.
    /// - [`Error::WarrenSigningKeyMissing`] if the BIP39 signing key
    ///   could not be loaded.
    /// - [`Error::WarrenAssemble`] if the selection itself fails
    ///   (no matching relay).
    pub async fn produce_warren_tunnel_params(
        &self,
        retry_attempt: u32,
    ) -> Result<WarrenTunnelParameters, Error> {
        let inner = self.0.lock().await;
        let selector = inner
            .warren_relay_selector
            .as_ref()
            .ok_or(Error::WarrenSelectorMissing)?;
        let signing_key = inner
            .warren_signing_key
            .as_ref()
            .ok_or(Error::WarrenSigningKeyMissing)?
            .clone();
        let query = relay_settings_to_warren_query(&inner.relay_settings);
        let multi_hop = inner.warren_multi_hop.clone();
        let mut params = warren_tunnel_params::assemble_for_attempt(
            selector,
            signing_key,
            &query,
            retry_attempt,
            multi_hop,
        )?;
        // Wire the multi-hop reconnect observer that bumps the
        // daemon-side WarrenStatusCache so the Electron UI
        // `reconnect_count` row advances on every successful reconnect.
        // The closure is harmless on the single-hop path: it is forwarded
        // through `params.on_reconnect` but never invoked because
        // `start_single_hop` ignores the field (no auto-reconnect
        // supervisor in /v1 single-hop).
        let cache = inner.warren_status_cache.clone();
        params.on_reconnect = Some(Arc::new(move || cache.record_reconnect()));
        Ok(params)
    }

    /// Sets the tunnel options to use when generating new tunnel parameters.
    pub async fn set_tunnel_options(&self, tunnel_options: &TunnelOptions) {
        self.0.lock().await.tunnel_options = tunnel_options.clone();
    }

    /// Updates generator state from full settings and keeps relay-selector config in sync.
    pub async fn set_settings(&self, settings: Settings) {
        let mut inner = self.0.lock().await;
        inner.relay_settings = settings.relay_settings.clone();
        inner.relay_selector.set_config(&settings);
    }

    pub async fn last_relay_was_overridden(&self) -> bool {
        let inner = self.0.lock().await;
        let Some(relays) = inner.last_generated_relays.as_ref() else {
            return false;
        };
        relays.server_override
    }

    /// Gets the location associated with the last generated tunnel parameters.
    pub async fn get_last_location(&self) -> Option<GeoIpLocation> {
        let inner = self.0.lock().await;

        let relays = inner.last_generated_relays.as_ref()?;

        let (entry, exit, obfuscator) = match &relays.config {
            WireguardConfig::Singlehop { exit } => {
                (None, exit, relays.has_obfuscator.then_some(exit))
            }
            WireguardConfig::Multihop { exit, entry } => {
                (Some(entry), exit, relays.has_obfuscator.then_some(entry))
            }
        };
        let location = exit.location.clone();

        Some(GeoIpLocation {
            ipv4: None,
            ipv6: None,
            country: location.country,
            city: Some(location.city),
            latitude: location.latitude,
            longitude: location.longitude,
            mullvad_exit_ip: true,
            hostname: Some(exit.hostname.clone()),
            entry_hostname: entry.map(|relay| relay.hostname.clone()),
            obfuscator_hostname: obfuscator.map(|relay| relay.hostname.clone()),
        })
    }
}

impl InnerParametersGenerator {
    async fn generate(
        &mut self,
        retry_attempt: u32,
        ip_availability: IpAvailability,
    ) -> Result<TunnelParameters, Error> {
        // Custom tunnel endpoints bypass relay selection entirely.
        if let RelaySettings::CustomTunnelEndpoint(ref endpoint) = self.relay_settings {
            self.last_generated_relays = None;
            return endpoint
                .to_tunnel_parameters(self.tunnel_options.clone())
                .map_err(|e| {
                    log::error!("Failed to resolve hostname for custom tunnel config: {}", e);
                    Error::ResolveCustomHostname
                });
        }

        let data = self.device().await?;
        let selected_relay = self
            .relay_selector
            .get_relay(retry_attempt as usize, ip_availability)?;

        let GetRelay {
            endpoint,
            obfuscator,
            inner,
        } = selected_relay;

        let server_override = {
            let first_relay = match &inner {
                WireguardConfig::Singlehop { exit } => exit,
                WireguardConfig::Multihop { exit: _, entry } => entry,
            };
            match endpoint.peer.endpoint {
                SocketAddr::V4(_) => first_relay.overridden_ipv4,
                SocketAddr::V6(_) => first_relay.overridden_ipv6,
            }
        };

        self.last_generated_relays = Some(LastSelectedRelays {
            config: inner,
            has_obfuscator: obfuscator.is_some(),
            server_override,
        });

        Ok(self.create_wireguard_tunnel_parameters(endpoint, data, obfuscator))
    }

    fn create_wireguard_tunnel_parameters(
        &self,
        endpoint: MullvadEndpoint,
        data: PrivateAccountAndDevice,
        obfuscator_config: Option<Obfuscators>,
    ) -> TunnelParameters {
        let tunnel_ipv4 = data.device.wg_data.addresses.ipv4_address.ip();
        let tunnel_ipv6 = data.device.wg_data.addresses.ipv6_address.ip();
        let tunnel = wireguard::TunnelConfig {
            private_key: data.device.wg_data.private_key,
            addresses: vec![IpAddr::from(tunnel_ipv4), IpAddr::from(tunnel_ipv6)],
        };

        wireguard::TunnelParameters {
            connection: wireguard::ConnectionConfig {
                tunnel,
                peer: endpoint.peer,
                exit_peer: endpoint.exit_peer,
                ipv4_gateway: endpoint.ipv4_gateway,
                ipv6_gateway: Some(endpoint.ipv6_gateway),
                #[cfg(target_os = "linux")]
                fwmark: Some(mullvad_types::TUNNEL_FWMARK),
            },
            options: self
                .tunnel_options
                .wireguard
                .clone()
                .into_talpid_tunnel_options(),
            generic_options: self.tunnel_options.generic.clone(),
            obfuscation: obfuscator_config,
        }
    }

    async fn device(&self) -> Result<PrivateAccountAndDevice, Error> {
        let device_state = self.account_manager.data().await?;
        device_state.into_device().ok_or(Error::NoAuthDetails)
    }
}

impl TunnelParametersGenerator for ParametersGenerator {
    fn generate(
        &mut self,
        retry_attempt: u32,
        ip_availability: IpAvailability,
    ) -> Pin<Box<dyn Future<Output = Result<TunnelParameters, ParameterGenerationError>>>> {
        let generator = self.0.clone();
        Box::pin(async move {
            let mut inner = generator.lock().await;
            inner
                .generate(retry_attempt, ip_availability)
                .await
                .inspect_err(|error| {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to generate tunnel parameters")
                    );
                })
                .map_err(ParameterGenerationError::from)
        })
    }

    /// Override of the default trait method to wire the Warren-Iroh
    /// path on the daemon side. Delegates to
    /// [`Self::produce_warren_tunnel_params`] which consumes the
    /// Warren artifacts stored in `InnerParametersGenerator`.
    fn generate_warren_tunnel_params(
        &mut self,
        retry_attempt: u32,
    ) -> Pin<Box<dyn Future<Output = Result<WarrenTunnelParameters, ParameterGenerationError>>>>
    {
        let this = self.clone();
        Box::pin(async move {
            this.produce_warren_tunnel_params(retry_attempt)
                .await
                .inspect_err(|error| {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to generate Warren tunnel parameters")
                    );
                })
                .map_err(ParameterGenerationError::from)
        })
    }
}

impl From<Error> for ParameterGenerationError {
    fn from(error: Error) -> Self {
        match error {
            Error::SelectRelay(mullvad_relay_selector::Error::NoBridge) => {
                ParameterGenerationError::NoMatchingBridgeRelay
            }
            Error::ResolveCustomHostname => {
                ParameterGenerationError::CustomTunnelHostResolutionError
            }
            Error::SelectRelay(mullvad_relay_selector::Error::IpVersionUnavailable { family }) => {
                ParameterGenerationError::IpVersionUnavailable { family }
            }
            Error::SelectRelay(mullvad_relay_selector::Error::NoRelayEntry(_)) => {
                ParameterGenerationError::NoMatchingRelayEntry
            }
            Error::SelectRelay(mullvad_relay_selector::Error::NoRelayExit(_)) => {
                ParameterGenerationError::NoMatchingRelayExit
            }
            Error::NoAuthDetails | Error::SelectRelay(_) | Error::Device(_) => {
                ParameterGenerationError::NoMatchingRelay
            }
            // Warren errors mapped to `NoMatchingRelay` on the state
            // machine side. A dedicated variant can be added if
            // the UI needs to distinguish these cases.
            Error::WarrenSelectorMissing
            | Error::WarrenSigningKeyMissing
            | Error::WarrenAssemble(_) => ParameterGenerationError::NoMatchingRelay,
        }
    }
}

/// Contains all relays that were selected last time when tunnel parameters were generated.
///
/// Represents all relays generated for a WireGuard tunnel.
/// The traffic flow can look like this:
///     client -> obfuscator -> entry -> exit -> internet
/// But for most users, it will look like this:
///     client -> entry -> internet
struct LastSelectedRelays {
    config: WireguardConfig,
    has_obfuscator: bool,
    server_override: bool,
}
