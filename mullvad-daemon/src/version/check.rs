#[cfg(not(target_os = "android"))]
use mullvad_update::version::VersionInfo;
use mullvad_version::Version;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct VersionCache {
    /// Version used for the [VersionCache]. This is needed to ensure that
    /// `current_version_supported` refers to the installed app.
    pub cache_version: Version,
    /// Whether the current (installed) version is supported or an upgrade is required
    pub current_version_supported: bool,
    #[cfg(not(target_os = "android"))]
    /// The latest available versions
    pub version_info: VersionInfo,
    /// When we last checked with platform headers
    pub last_platform_header_check: SystemTime,
    #[cfg(not(target_os = "android"))]
    pub metadata_version: usize,
    /// HTTP ETag associated with this metadata
    pub etag: Option<String>,
}
