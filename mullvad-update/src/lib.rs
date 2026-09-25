//! Support functions for securely installing or updating Mullvad VPN

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "client")]
pub use client::*;

mod defaults;

pub mod version;

/// Package-manager side of in-app upgrades on Linux
pub mod linux;

/// Parser and serializer for version metadata
pub mod format;

#[cfg(feature = "client")]
pub mod hash;
