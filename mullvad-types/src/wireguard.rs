#![allow(clippy::identity_op)]
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

// `QuantumResistantState`, `DaitaSettings` and `TunnelOptions` feed the
// Warren tunnel's DAITA and quantum-resistant settings.

#[derive(Serialize, Deserialize, Default, Copy, Clone, Debug, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaitaSettings {
    pub enabled: bool,

    #[serde(default = "DaitaSettings::default_use_multihop_if_necessary")]
    /// Whether to use multihop if the selected relay is not DAITA-compatible. Note that this is
    /// the inverse of of "Direct only" in the GUI.
    pub use_multihop_if_necessary: bool,
}

impl DaitaSettings {
    /// This setting should be enabled by default.
    const fn default_use_multihop_if_necessary() -> bool {
        true
    }
}

impl Default for DaitaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            use_multihop_if_necessary: Self::default_use_multihop_if_necessary(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TunnelOptions {
    /// Require the post-quantum seal on the tunnel session.
    pub quantum_resistant: QuantumResistantState,
    /// Configure DAITA
    pub daita: DaitaSettings,
}
