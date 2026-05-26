#[cfg(target_os = "android")]
use crate::android::InetNetwork;
use crate::net::{IpVersion, TunnelEndpoint};
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(target_os = "android")]
use std::net::IpAddr;

/// Event emitted from the states in `talpid_core::tunnel_state_machine` when the tunnel state
/// machine enters a new state.
#[derive(Clone, Debug)]
pub enum TunnelStateTransition {
    /// No connection is established and network is unsecured.
    #[cfg(not(target_os = "android"))]
    Disconnected {
        /// Whether internet access is blocked due to lockdown mode
        locked_down: bool,
    },
    #[cfg(target_os = "android")]
    /// No connection is established and network is unsecured.
    Disconnected {},
    /// Network is secured but tunnel is still connecting.
    Connecting(TunnelEndpoint),
    /// Tunnel is connected.
    Connected(TunnelEndpoint),
    /// Disconnecting tunnel.
    Disconnecting(ActionAfterDisconnect),
    /// Tunnel is disconnected but usually secured by blocking all connections.
    Error(ErrorState),
}

/// Action that will be taken after disconnection is complete.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAfterDisconnect {
    Nothing,
    Block,
    Reconnect,
}

/// Represents the tunnel state machine entering an error state during a [`TunnelStateTransition`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ErrorState {
    /// Reason why the tunnel state machine ended up in the error state
    cause: ErrorStateCause,
    /// Indicates whether the daemon is currently blocking all traffic. This _should_ always
    /// succeed - in the case it does not, the user should be notified that no traffic is being
    /// blocked.
    /// An error value means there was a serious error and the intended security properties are not
    /// being upheld.
    block_failure: Option<FirewallPolicyError>,
}

impl ErrorState {
    pub fn new(cause: ErrorStateCause, block_failure: Option<FirewallPolicyError>) -> Self {
        Self {
            cause,
            block_failure,
        }
    }

    pub fn is_blocking(&self) -> bool {
        self.block_failure.is_none()
    }

    pub fn cause(&self) -> &ErrorStateCause {
        &self.cause
    }

    pub fn block_failure(&self) -> Option<&FirewallPolicyError> {
        self.block_failure.as_ref()
    }
}

/// Reason for the tunnel state machine entering an [`ErrorState`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "reason", content = "details")]
pub enum ErrorStateCause {
    /// Authentication with remote server failed.
    AuthFailed(Option<String>),
    /// Failed to configure IPv6 because it's disabled in the platform.
    Ipv6Unavailable,
    /// Failed to set firewall policy.
    SetFirewallPolicyError(FirewallPolicyError),
    /// Failed to set system DNS server.
    SetDnsError,
    /// Android has rejected one or more DNS server addresses.
    #[cfg(target_os = "android")]
    InvalidDnsServers(Vec<IpAddr>),
    /// Android has rejected due to invalid IPV6 config.
    #[cfg(target_os = "android")]
    InvalidIPv6Config {
        addresses: Vec<IpAddr>,
        routes: Vec<InetNetwork>,
        dns_servers: Vec<IpAddr>,
    },
    /// Failed to create tunnel device.
    #[cfg(target_os = "windows")]
    CreateTunnelDevice { os_error: Option<i32> },
    /// Failed to start connection to remote server.
    StartTunnelError,
    /// Tunnel parameter generation failure
    TunnelParameterError(ParameterGenerationError),
    /// This device is offline, no tunnels can be established.
    IsOffline,
    #[cfg(target_os = "android")]
    NotPrepared,
    #[cfg(target_os = "android")]
    OtherAlwaysOnApp { app_name: String },
    #[cfg(target_os = "android")]
    OtherLegacyAlwaysOnVpn,
    /// Error reported by split tunnel module.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "android"))]
    SplitTunnelError,
    /// Missing permissions required by macOS split tunneling.
    #[cfg(target_os = "macos")]
    NeedFullDiskPermissions,
    /// Warren TOFU pubkey mismatch: the exit's Ed25519 pubkey differed from the
    /// pinned one. The user must explicitly trust the new key or reset all pins
    /// via the management interface before another connect attempt.
    WarrenPubkeyMismatch {
        exit_id_hex: String,
        pinned: String,
        observed: String,
    },
}

impl ErrorStateCause {
    #[cfg(target_os = "macos")]
    pub fn prevents_filtering_resolver(&self) -> bool {
        matches!(self, Self::SetDnsError)
    }
}

/// Errors that can occur when generating tunnel parameters.
#[derive(thiserror::Error, Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGenerationError {
    /// Failure to select a matching entry tunnel relay
    #[error("Failure to select a matching entry tunnel relay")]
    NoMatchingRelayEntry,
    /// Failure to select a matching exit tunnel relay
    #[error("Failure to select a matching exit tunnel relay")]
    NoMatchingRelayExit,
    /// Failure to select a matching tunnel relay, but we do not know if it is an entry or an exit
    #[error("Failure to select a matching tunnel relay")]
    NoMatchingRelay,
    /// Failure to select a matching bridge relay
    #[error("Failure to select a matching bridge relay")]
    NoMatchingBridgeRelay,
    /// Failure to resolve the hostname of a custom tunnel configuration
    #[error("Can't resolve hostname for custom tunnel host")]
    CustomTunnelHostResolutionError,
    /// User has selected an IP version that is not available on the network
    #[error("The requested IP version ({family}) is not available")]
    IpVersionUnavailable { family: IpVersion },
    /// Warren TOFU pubkey mismatch: the exit's observed Ed25519 pubkey diverges
    /// from the pinned one. The user must explicitly trust the new key or reset
    /// all pins via the gRPC management interface before reconnecting.
    #[error(
        "Warren exit pubkey mismatch (exit_id={exit_id_hex}, pinned={pinned}, observed={observed})"
    )]
    WarrenPubkeyMismatch {
        exit_id_hex: String,
        pinned: String,
        observed: String,
    },
}

/// Application that prevents setting the firewall policy.
#[cfg(windows)]
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct BlockingApplication {
    pub name: String,
    pub pid: u32,
}

/// Errors that can occur when setting the firewall policy.
#[derive(thiserror::Error, Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "reason", content = "details")]
pub enum FirewallPolicyError {
    /// General firewall failure
    #[error("Failed to set firewall policy")]
    Generic,
    /// An application prevented the firewall policy from being set
    #[cfg(windows)]
    #[error("An application prevented the firewall policy from being set")]
    Locked(Option<BlockingApplication>),
}

impl fmt::Display for ErrorStateCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::ErrorStateCause::*;
        let description = match self {
            AuthFailed(reason) => {
                return write!(
                    f,
                    "Authentication with remote server failed: {}",
                    match reason {
                        Some(reason) => reason.as_str(),
                        None => "No reason provided",
                    }
                );
            }
            Ipv6Unavailable => "Failed to configure IPv6 because it's disabled in the platform",
            SetFirewallPolicyError(err) => {
                return match err {
                    #[cfg(windows)]
                    FirewallPolicyError::Locked(Some(value)) => {
                        write!(f, "{}: {} (pid {})", err, value.name, value.pid)
                    }
                    _ => write!(f, "{err}"),
                };
            }
            SetDnsError => "Failed to set system DNS server",
            #[cfg(target_os = "android")]
            InvalidDnsServers(addresses) => {
                return write!(
                    f,
                    "Invalid DNS server addresses used in tunnel configuration: {}",
                    addresses
                        .iter()
                        .map(IpAddr::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            #[cfg(target_os = "android")]
            InvalidIPv6Config {
                addresses,
                routes,
                dns_servers,
            } => {
                return write!(
                    f,
                    "Invalid ipv6 tunnel configuration. addresses: {} routes: {} dns_servers: {}",
                    addresses
                        .iter()
                        .map(IpAddr::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    routes
                        .iter()
                        .map(InetNetwork::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    dns_servers
                        .iter()
                        .map(IpAddr::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            StartTunnelError => "Failed to start connection to remote server",
            #[cfg(target_os = "windows")]
            CreateTunnelDevice {
                os_error: Some(error),
            } => return write!(f, "Failed to create tunnel device: {error}"),
            #[cfg(target_os = "windows")]
            CreateTunnelDevice { os_error: None } => {
                return write!(f, "Failed to create tunnel device");
            }
            TunnelParameterError(err) => {
                return write!(f, "Failure to generate tunnel parameters: {err}");
            }
            IsOffline => "This device is offline, no tunnels can be established",
            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "android"))]
            SplitTunnelError => "The split tunneling module reported an error",
            #[cfg(target_os = "macos")]
            NeedFullDiskPermissions => "Need full disk access to enable split tunneling",
            #[cfg(target_os = "android")]
            NotPrepared => "This device is not prepared",
            #[cfg(target_os = "android")]
            OtherAlwaysOnApp { app_name: _ } => "Another app is set as always on",
            #[cfg(target_os = "android")]
            OtherLegacyAlwaysOnVpn => "Another legacy vpn profile is set as always on",
            WarrenPubkeyMismatch {
                exit_id_hex,
                pinned,
                observed,
            } => {
                return write!(
                    f,
                    "Warren exit pubkey mismatch: exit {exit_id_hex} \
                     (pinned={pinned}, observed={observed}). \
                     Trust the new key or reset pinned keys to reconnect."
                );
            }
        };

        write!(f, "{description}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M-8 regression: `WarrenPubkeyPinMismatch` must produce
    /// `ParameterGenerationError::WarrenPubkeyMismatch`, not `NoMatchingRelay`.
    /// This is tested via the `ParameterGenerationError` enum directly (the
    /// daemon-side `From<tunnel::Error>` conversion is tested in the daemon crate).
    #[test]
    fn parameter_generation_error_warren_pubkey_mismatch_variant_exists() {
        let err = ParameterGenerationError::WarrenPubkeyMismatch {
            exit_id_hex: "deadbeef".to_string(),
            pinned: "aa".to_string(),
            observed: "bb".to_string(),
        };
        // Ensure the Display includes enough context for the user.
        let msg = format!("{err}");
        assert!(
            msg.contains("deadbeef"),
            "display should include exit_id_hex: {msg:?}"
        );
        assert!(
            msg.contains("aa"),
            "display should include pinned key: {msg:?}"
        );
        assert!(
            msg.contains("bb"),
            "display should include observed key: {msg:?}"
        );
    }

    /// M-8: `ErrorStateCause::WarrenPubkeyMismatch` must be distinct from
    /// `TunnelParameterError(NoMatchingRelay)`.
    #[test]
    fn error_state_cause_warren_pubkey_mismatch_is_distinct_from_no_matching_relay() {
        let mismatch = ErrorStateCause::WarrenPubkeyMismatch {
            exit_id_hex: "aaaa".to_string(),
            pinned: "p1".to_string(),
            observed: "p2".to_string(),
        };
        let no_relay = ErrorStateCause::TunnelParameterError(
            ParameterGenerationError::NoMatchingRelay,
        );
        // They must not be equal (the discriminants must differ).
        // PartialEq is not derived; compare via Display instead.
        let mismatch_msg = format!("{mismatch}");
        let no_relay_msg = format!("{no_relay}");
        assert_ne!(
            mismatch_msg, no_relay_msg,
            "WarrenPubkeyMismatch and NoMatchingRelay must produce different messages"
        );
        assert!(
            mismatch_msg.contains("pubkey"),
            "mismatch message should mention 'pubkey': {mismatch_msg:?}"
        );
    }
}
