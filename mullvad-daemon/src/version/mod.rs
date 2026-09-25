use std::io;

pub mod check;
pub mod downloader;
#[cfg(target_os = "linux")]
pub mod linux_upgrade;
pub mod router;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to open app version cache file for reading")]
    ReadVersionCache(#[source] io::Error),

    #[error("Failed to open app version cache file for writing")]
    WriteVersionCache(#[source] io::Error),

    #[error("Failure in serialization of the version info")]
    Serialize(#[source] serde_json::Error),

    #[error("Failure in deserialization of the version info")]
    Deserialize(#[source] serde_json::Error),

    #[error("Failed to check the latest app version")]
    Download(#[source] mullvad_api::rest::Error),

    #[error("API availability check failed")]
    ApiCheck(#[source] mullvad_api::availability::Error),

    #[error("Response is missing a valid stable version")]
    MissingStable,

    #[error("Clearing version check cache due to old version")]
    OutdatedVersion,

    #[error("Version updater is down")]
    VersionUpdaterDown,

    #[error("App version check failed and no cached version info exists")]
    VersionCheckFailed,

    #[error("Version router is down")]
    VersionRouterClosed,

    #[error("Version cache update was aborted")]
    UpdateAborted,

    #[error("No downloaded and verified upgrade to install")]
    NoVerifiedInstaller,

    #[error("The daemon installs upgrades on Linux only; this platform's app runs its installer")]
    InstallUnsupported,

    #[cfg(target_os = "linux")]
    #[error("Failed to start the upgrade")]
    Install(#[source] linux_upgrade::Error),
}

/// Contains the date of the git commit this was built from
pub const COMMIT_DATE: &str = include_str!(concat!(env!("OUT_DIR"), "/git-commit-date.txt"));

pub fn is_beta_version() -> bool {
    mullvad_version::VERSION.contains("beta")
}

pub fn is_dev_version() -> bool {
    mullvad_version::VERSION.contains("dev")
}

pub fn log_version(bin_name: &str) {
    log::info!(
        "Starting {} - {} {} (product env: {})",
        bin_name,
        mullvad_version::VERSION,
        COMMIT_DATE,
        warren_product_env::ENV_NAME,
    )
}
