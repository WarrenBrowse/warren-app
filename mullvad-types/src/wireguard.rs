#![allow(clippy::identity_op)]
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use talpid_types::net::wireguard;

use crate::Intersection;

// `QuantumResistantState`, `DaitaSettings` and `TunnelOptions` feed the
// Quinn tunnel's MTU/DAITA/quantum-resistant settings.

#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug, PartialEq, Eq, Intersection)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum QuantumResistantState {
    #[default]
    On,
    Off,
}

impl QuantumResistantState {
    pub fn enabled(&self) -> bool {
        use QuantumResistantState::*;
        match self {
            On => true,
            Off => false,
        }
    }
}

impl fmt::Display for QuantumResistantState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use QuantumResistantState::*;
        match self {
            On => f.write_str("on"),
            Off => f.write_str("off"),
        }
    }
}

impl FromStr for QuantumResistantState {
    type Err = QuantumResistantStateParseError;

    fn from_str(s: &str) -> Result<QuantumResistantState, Self::Err> {
        use QuantumResistantState::*;
        match s {
            "on" => Ok(On),
            "off" => Ok(Off),
            _ => Err(QuantumResistantStateParseError),
        }
    }
}

/// Returned when `QuantumResistantState::from_str` fails to convert a string into a
/// [`QuantumResistantState`] object.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("Not a valid state")]
pub struct QuantumResistantStateParseError;

#[cfg(daita)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaitaSettings {
    pub enabled: bool,

    #[serde(default = "DaitaSettings::default_use_multihop_if_necessary")]
    /// Whether to use multihop if the selected relay is not DAITA-compatible. Note that this is
    /// the inverse of of "Direct only" in the GUI.
    pub use_multihop_if_necessary: bool,
}

#[cfg(daita)]
impl DaitaSettings {
    /// This setting should be enabled by default.
    const fn default_use_multihop_if_necessary() -> bool {
        true
    }
}

#[cfg(daita)]
impl Default for DaitaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            use_multihop_if_necessary: Self::default_use_multihop_if_necessary(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TunnelOptions {
    /// MTU for the wireguard tunnel
    pub mtu: Option<u16>,
    /// Obtain a PSK using the relay config client.
    pub quantum_resistant: QuantumResistantState,
    /// Configure DAITA
    #[cfg(daita)]
    pub daita: DaitaSettings,
    /// Use userspace WireGuard.
    pub userspace: bool,
}

#[expect(clippy::derivable_impls)]
impl Default for TunnelOptions {
    fn default() -> Self {
        TunnelOptions {
            mtu: None,
            quantum_resistant: QuantumResistantState::default(),
            #[cfg(daita)]
            daita: DaitaSettings::default(),
            userspace: false,
        }
    }
}

impl TunnelOptions {
    pub fn into_talpid_tunnel_options(self) -> wireguard::TunnelOptions {
        wireguard::TunnelOptions {
            mtu: self.mtu,
            quantum_resistant: self.quantum_resistant.enabled(),
            #[cfg(daita)]
            daita: self.daita.enabled,
            userspace: self.userspace,
        }
    }
}
