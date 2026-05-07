use std::net::SocketAddr;
use std::{future::Future, net::IpAddr, pin::Pin, sync::Arc};

use ed25519_dalek::SigningKey;
use talpid_types::net::wireguard::TunnelParameters;
use talpid_warren_iroh::WarrenIrohParameters;
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
use warren_relay_selector::WarrenRelayQuery;

use talpid_types::{ErrorExt, net::IpAvailability, tunnel::ParameterGenerationError};

use crate::device::{AccountManagerHandle, Error as DeviceError, PrivateAccountAndDevice};
use crate::warren_iroh_params::{self, AssembleError};
use crate::warren_relay_selector::DaemonWarrenRelaySelector;

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

    /// Warren mode actif (env var `WARREN_TUNNEL=1`) mais le selector
    /// Warren n'est pas configuré côté generator.
    #[error("Warren tunnel mode requested but no Warren relay selector configured")]
    WarrenSelectorMissing,

    /// Warren mode actif mais la `signing_key` n'a pas pu être chargée
    /// (mnémonique BIP39 absente ou corrompue cf. `warren_signer`).
    #[error("Warren tunnel mode requested but no Warren signing key available")]
    WarrenSigningKeyMissing,

    /// Échec de l'assemblage des `WarrenIrohParameters` (sélection
    /// échouée, ...).
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

    /// Warren fork — Phase 4.F.5 : artefacts pour le path Warren-Iroh
    /// parallèle. `None` côté daemon non-Warren (= path WG seul) ;
    /// `Some` quand `mullvad-daemon::warren_mode::is_enabled() == true`
    /// au boot (cf. `lib.rs` Phase 4.F.5.c).
    ///
    /// `dead_code` allow autorisé : ces champs sont consommés par
    /// `generate_warren_iroh_params` qui sera invoqué depuis le state
    /// machine en Phase 4.F.5.b/c. Cf. règle dans CLAUDE.md sur le
    /// double-allow workaround Phase 1.A.
    #[allow(clippy::allow_attributes)]
    #[allow(dead_code)]
    warren_relay_selector: Option<DaemonWarrenRelaySelector>,
    #[allow(clippy::allow_attributes)]
    #[allow(dead_code)]
    warren_signing_key: Option<SigningKey>,
}

impl ParametersGenerator {
    /// Constructs a tunnel parameters generator avec les artefacts
    /// Warren-Iroh optionnels (Phase 4.F.5).
    ///
    /// Si `warren_relay_selector` ou `warren_signing_key` sont `None`,
    /// `generate_warren_iroh_params` retournera l'erreur typée
    /// correspondante. Le path WireGuard reste utilisable en parallèle
    /// quel que soit l'état des artefacts Warren.
    pub fn new_with_optional_warren(
        account_manager: AccountManagerHandle,
        relay_selector: RelaySelector,
        relay_settings: RelaySettings,
        tunnel_options: TunnelOptions,
        warren_relay_selector: Option<DaemonWarrenRelaySelector>,
        warren_signing_key: Option<SigningKey>,
    ) -> Self {
        Self(Arc::new(Mutex::new(InnerParametersGenerator {
            tunnel_options,
            relay_selector,
            relay_settings,
            account_manager,
            last_generated_relays: None,
            warren_relay_selector,
            warren_signing_key,
        })))
    }

    /// Phase 4.F.5 — assemble un [`WarrenIrohParameters`] pour la
    /// tentative `retry_attempt`, à partir des artefacts Warren stockés.
    ///
    /// API miroir asymétrique de
    /// [`TunnelParametersGenerator::generate`] (qui produit du WG) :
    /// le state machine choisit l'une OU l'autre selon le mode Warren
    /// actif (cf. `warren_mode::is_enabled`).
    ///
    /// # Errors
    ///
    /// - [`Error::WarrenSelectorMissing`] si le selector Warren n'a
    ///   pas été configuré au boot.
    /// - [`Error::WarrenSigningKeyMissing`] si la signing key BIP39
    ///   n'a pas pu être chargée.
    /// - [`Error::WarrenAssemble`] si la sélection elle-même échoue
    ///   (aucun relay matchant).
    pub async fn produce_warren_iroh_params(
        &self,
        retry_attempt: u32,
    ) -> Result<WarrenIrohParameters, Error> {
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
        let params = warren_iroh_params::assemble_for_attempt(
            selector,
            signing_key,
            &WarrenRelayQuery::any(),
            retry_attempt,
        )?;
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

    /// Warren fork — Phase 4.F.5.c.1 : override du default trait method
    /// pour brancher le path Warren-Iroh côté daemon. Délègue à la
    /// méthode publique async existante (Phase 4.F.5.a) qui consomme
    /// les artefacts Warren stockés dans `InnerParametersGenerator`.
    fn generate_warren_iroh_params(
        &mut self,
        retry_attempt: u32,
    ) -> Pin<Box<dyn Future<Output = Result<WarrenIrohParameters, ParameterGenerationError>>>> {
        let this = self.clone();
        Box::pin(async move {
            this.produce_warren_iroh_params(retry_attempt)
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
            // Phase 4.F.5 : les erreurs Warren sont remontées comme
            // `NoMatchingRelay` côté state machine pour le moment ;
            // une variante `WarrenAssemble` dédiée pourra être ajoutée
            // si on veut un message d'erreur UI distinct.
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
