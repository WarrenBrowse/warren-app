//! App installer

use serde::{Deserialize, Serialize};

use super::architecture::Architecture;

/// App installer
#[derive(Debug, Deserialize, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Installer {
    /// Installer architecture
    pub architecture: Architecture,
    /// Mirrors that host the artifact
    pub urls: Vec<String>,
    /// Size of the installer, in bytes
    pub size: usize,
    /// Hash of the installer, hexadecimal string
    pub sha256: String,
    /// Linux package format the installer is (`deb`, `deb-sysvinit`, `rpm`,
    /// `pacman`), because a Linux release ships one per format for the same
    /// architecture. Absent on macOS and Windows, which ship one installer per
    /// architecture.
    ///
    /// A plain string rather than an enum on purpose: a client that meets a
    /// format published after it was built must skip that installer, and an
    /// unknown enum variant would fail the whole manifest instead. Omitted when
    /// empty so the signed bytes of a macOS or Windows manifest do not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_format: Option<String>,
}
