//! Abstracts over an active VPN tunnel.
//!
//! Warren fork : le backend interne est une enum [`TunnelBackend`]
//! qui dispatche entre le `WireguardMonitor` upstream et le
//! `WarrenTunnelMonitor`. Le path WG est strictement préservé — aucun
//! changement de comportement pour les déploiements qui n'activent
//! pas le backend Warren via `WARREN_TUNNEL=1`.
use std::path;

use talpid_tunnel::TunnelArgs;
#[cfg(target_os = "android")]
use talpid_tunnel::tun_provider;
use talpid_types::net::{wireguard as wireguard_types, wireguard::TunnelParameters};
use talpid_types::tunnel::ErrorStateCause;
use talpid_warren_tunnel::{WarrenTunnelMonitor, WarrenTunnelParameters};
use talpid_wireguard::WireguardMonitor;

const WIREGUARD_LOG_FILENAME: &str = "wireguard.log";

/// Results from operations in the tunnel module.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in the [`TunnelMonitor`].
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Tunnel can't have IPv6 enabled because the system has disabled IPv6 support.
    #[error("Can't enable IPv6 on tunnel interface because IPv6 is disabled")]
    EnableIpv6,

    /// Running on an operating system which is not supported yet.
    #[error("Tunnel type not supported on this operating system")]
    UnsupportedPlatform,

    /// Failed to rotate tunnel log file
    #[error("Failed to rotate tunnel log file")]
    RotateLogError(#[from] crate::logging::RotateLogError),

    /// There was an error listening for events from the Wireguard tunnel
    #[error("Failed while listening for events from the Wireguard tunnel")]
    TunnelMonitoring(#[from] talpid_wireguard::Error),

    /// There was an error listening for events from the Warren-Iroh tunnel.
    #[error("Failed while listening for events from the Warren-Iroh tunnel")]
    WarrenTunnelMonitoring(#[from] talpid_warren_tunnel::Error),
}

impl From<Error> for ErrorStateCause {
    fn from(error: Error) -> ErrorStateCause {
        match error {
            Error::EnableIpv6 => ErrorStateCause::Ipv6Unavailable,

            #[cfg(target_os = "android")]
            Error::TunnelMonitoring(talpid_wireguard::Error::TunnelError(
                talpid_wireguard::TunnelError::SetupTunnelDevice(
                    tun_provider::Error::OtherLegacyAlwaysOnVpn,
                ),
            )) => ErrorStateCause::OtherLegacyAlwaysOnVpn,

            #[cfg(target_os = "android")]
            Error::TunnelMonitoring(talpid_wireguard::Error::TunnelError(
                talpid_wireguard::TunnelError::SetupTunnelDevice(
                    tun_provider::Error::OtherAlwaysOnApp { app_name },
                ),
            )) => ErrorStateCause::OtherAlwaysOnApp { app_name },

            #[cfg(target_os = "android")]
            Error::TunnelMonitoring(talpid_wireguard::Error::TunnelError(
                talpid_wireguard::TunnelError::SetupTunnelDevice(tun_provider::Error::NotPrepared),
            )) => ErrorStateCause::NotPrepared,

            #[cfg(target_os = "android")]
            Error::TunnelMonitoring(talpid_wireguard::Error::TunnelError(
                talpid_wireguard::TunnelError::SetupTunnelDevice(
                    tun_provider::Error::InvalidDnsServers(addresses),
                ),
            )) => ErrorStateCause::InvalidDnsServers(addresses),

            #[cfg(target_os = "android")]
            Error::TunnelMonitoring(talpid_wireguard::Error::TunnelError(
                talpid_wireguard::TunnelError::SetupTunnelDevice(
                    tun_provider::Error::InvalidIpv6Config {
                        addresses,
                        routes,
                        dns_servers,
                    },
                ),
            )) => ErrorStateCause::InvalidIPv6Config {
                addresses,
                routes,
                dns_servers,
            },
            #[cfg(target_os = "windows")]
            error => match error.get_tunnel_device_error() {
                Some(error) => ErrorStateCause::CreateTunnelDevice {
                    os_error: error.raw_os_error(),
                },
                None => ErrorStateCause::StartTunnelError,
            },
            #[cfg(not(target_os = "windows"))]
            _ => ErrorStateCause::StartTunnelError,
        }
    }
}

impl Error {
    /// Return whether retrying the operation that caused this error is likely to succeed.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Error::TunnelMonitoring(error) => error.is_recoverable(),
            Error::WarrenTunnelMonitoring(error) => error.is_recoverable(),
            _ => false,
        }
    }

    /// Get the inner tunnel device error, if there is one
    #[cfg(target_os = "windows")]
    pub fn get_tunnel_device_error(&self) -> Option<&std::io::Error> {
        match self {
            Error::TunnelMonitoring(error) => error.get_tunnel_device_error(),
            // Warren-Iroh n'expose pas (encore) un `tunnel_device_error`
            // équivalent ; Phase 1.B raffinera si nécessaire.
            _ => None,
        }
    }
}

/// Backend de tunnel sous-jacent — Warren fork.
///
/// Enum dispatch statique entre les implémentations supportées. L'ajout
/// d'un nouveau backend nécessite une variante ici + une factory dans
/// [`TunnelMonitor`]. Approche choisie vs. `Box<dyn Tunnel>` : 2 backends
/// connus (WG upstream + Warren-Iroh), API riches conservées
/// (`is_recoverable`, `get_tunnel_device_error`) sans downcast.
enum TunnelBackend {
    /// Backend historique WireGuard (path upstream Mullvad inchangé).
    Wireguard(WireguardMonitor),
    /// Backend Warren-Iroh, construit via [`TunnelMonitor::start_warren_tunnel`]
    /// quand `warren_mode` est actif côté state machine.
    WarrenTunnel(WarrenTunnelMonitor),
}

/// Abstraction for monitoring a VPN tunnel.
pub struct TunnelMonitor {
    backend: TunnelBackend,
}

impl TunnelMonitor {
    /// Creates a new `TunnelMonitor` that connects to the given remote and notifies `on_event`
    /// on tunnel state changes.
    pub fn start(
        tunnel_parameters: &TunnelParameters,
        log_dir: &Option<path::PathBuf>,
        args: TunnelArgs<'_>,
    ) -> Result<Self> {
        Self::ensure_ipv6_can_be_used_if_enabled(tunnel_parameters)?;
        let log_file = Self::prepare_tunnel_log_file(log_dir.as_ref())?;

        Self::start_wireguard_tunnel(tunnel_parameters, log_file, args)
    }

    /// Démarre un tunnel via le backend Warren-Iroh.
    ///
    /// Factory séparée de [`Self::start`] parce que les paramètres Iroh
    /// divergent du `TunnelParameters` WireGuard (champs distincts, pas
    /// d'obfuscation, pas d'options WG). Invoquée par
    /// `connecting_state::start_tunnel_warren` quand `warren_mode` est
    /// actif.
    ///
    /// # Errors
    ///
    /// [`Error::WarrenTunnelMonitoring`] si le backend Iroh échoue à
    /// initialiser.
    pub fn start_warren_tunnel(
        params: &WarrenTunnelParameters,
        log_dir: &Option<path::PathBuf>,
        args: TunnelArgs<'_>,
    ) -> Result<Self> {
        let log_file = Self::prepare_tunnel_log_file(log_dir.as_ref())?;
        let monitor = WarrenTunnelMonitor::start(params, args, log_file.as_deref())?;
        Ok(TunnelMonitor {
            backend: TunnelBackend::WarrenTunnel(monitor),
        })
    }

    fn start_wireguard_tunnel(
        params: &wireguard_types::TunnelParameters,
        log: Option<path::PathBuf>,
        args: TunnelArgs<'_>,
    ) -> Result<Self> {
        let monitor = talpid_wireguard::WireguardMonitor::start(params, args, log.as_deref())?;
        Ok(TunnelMonitor {
            backend: TunnelBackend::Wireguard(monitor),
        })
    }

    fn ensure_ipv6_can_be_used_if_enabled(tunnel_parameters: &TunnelParameters) -> Result<()> {
        let options = &tunnel_parameters.generic_options;
        if options.enable_ipv6 && !is_ipv6_enabled_in_os() {
            Err(Error::EnableIpv6)
        } else {
            Ok(())
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn prepare_tunnel_log_file(log_dir: Option<&path::PathBuf>) -> Result<Option<path::PathBuf>> {
        Ok(log_dir.map(|dir| dir.join(WIREGUARD_LOG_FILENAME)))
    }

    #[cfg(target_os = "windows")]
    fn prepare_tunnel_log_file(log_dir: Option<&path::PathBuf>) -> Result<Option<path::PathBuf>> {
        if let Some(log_dir) = log_dir {
            let filename = WIREGUARD_LOG_FILENAME;
            let tunnel_log = log_dir.join(filename);
            crate::logging::rotate_log(&tunnel_log)?;
            Ok(Some(tunnel_log))
        } else {
            Ok(None)
        }
    }

    /// Consumes the monitor and blocks until the tunnel exits or there is an error.
    pub fn wait(self) -> Result<()> {
        match self.backend {
            TunnelBackend::Wireguard(monitor) => monitor.wait().map_err(Error::from),
            TunnelBackend::WarrenTunnel(monitor) => monitor.wait().map_err(Error::from),
        }
    }
}

#[cfg(target_os = "windows")]
fn is_ipv6_enabled_in_os() -> bool {
    use winreg::{RegKey, enums::*};

    const IPV6_DISABLED_ON_TUNNELS_MASK: u32 = 0x01;

    // Check registry if IPv6 is disabled on tunnel interfaces, as documented in
    // https://support.microsoft.com/en-us/help/929852/guidance-for-configuring-ipv6-in-windows-for-advanced-users
    let globally_enabled = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters")
        .and_then(|ipv6_config| ipv6_config.get_value("DisabledComponents"))
        .map(|ipv6_disabled_bits: u32| (ipv6_disabled_bits & IPV6_DISABLED_ON_TUNNELS_MASK) == 0)
        .unwrap_or(true);

    if globally_enabled {
        true
    } else {
        log::debug!("IPv6 disabled in all tunnel interfaces");
        false
    }
}

#[cfg(not(target_os = "windows"))]
fn is_ipv6_enabled_in_os() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/sys/net/ipv6/conf/all/disable_ipv6")
            .map(|disable_ipv6| disable_ipv6.trim() == "0")
            .unwrap_or(false)
    }
    #[cfg(any(target_os = "macos", target_os = "android"))]
    {
        true
    }
}
