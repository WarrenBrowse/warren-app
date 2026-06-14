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

/// Spawn the background poller that fetches signed version metadata from the
/// Warren update channel and feeds [`VersionCache`] updates to the version
/// router.
///
/// Detection (is there a newer version? is the running one still supported?)
/// runs on every desktop platform. The in-app download/install step is gated
/// separately by the `in_app_upgrade` cfg (Windows + macOS); on Linux the
/// metadata carries no installers and the GUI points the user at the download
/// page instead.
///
/// `refresh_rx` is woken by the router whenever a frontend asks for a fresh
/// check; the poller otherwise re-checks on a fixed interval.
#[cfg(not(target_os = "android"))]
pub(super) fn spawn_version_updater(
    rollout_threshold_seed: Option<u32>,
    new_version_tx: futures::channel::mpsc::UnboundedSender<VersionCache>,
    refresh_rx: futures::channel::mpsc::UnboundedReceiver<()>,
) {
    tokio::spawn(version_updater_loop(
        rollout_threshold_seed,
        new_version_tx,
        refresh_rx,
    ));
}

#[cfg(not(target_os = "android"))]
async fn version_updater_loop(
    rollout_threshold_seed: Option<u32>,
    new_version_tx: futures::channel::mpsc::UnboundedSender<VersionCache>,
    mut refresh_rx: futures::channel::mpsc::UnboundedReceiver<()>,
) {
    use futures::StreamExt;
    use mullvad_update::api::MetaRepositoryPlatform;

    /// Poll interval while everything is healthy.
    const UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);
    /// Shorter retry interval after a failed check (network or host issues).
    const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

    let Some(platform) = MetaRepositoryPlatform::current() else {
        log::warn!("App update checks disabled: no update metadata for this OS");
        return;
    };
    let Some(architecture) = native_update_architecture() else {
        log::warn!("App update checks disabled: could not detect the CPU architecture");
        return;
    };

    // Anti-rollback floor: a fetched manifest must carry a `metadata_version`
    // at least this high. It is ratcheted up after each accepted response so a
    // replayed older manifest is rejected for the rest of this daemon run.
    let mut lowest_metadata_version = mullvad_update::version::MIN_VERIFY_METADATA_VERSION;

    loop {
        let interval = match check_once(
            platform,
            architecture,
            rollout_threshold_seed,
            lowest_metadata_version,
        )
        .await
        {
            Ok(cache) => {
                lowest_metadata_version = lowest_metadata_version.max(cache.metadata_version);
                // The receiver lives as long as the router; a send error means
                // the daemon is shutting down, so stop polling.
                if new_version_tx.unbounded_send(cache).is_err() {
                    break;
                }
                UPDATE_INTERVAL
            }
            Err(error) => {
                log::error!("Failed to check for app updates: {error:#}");
                RETRY_INTERVAL
            }
        };

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            refresh = refresh_rx.next() => {
                // `None` means the router dropped its refresh sender: shut down.
                if refresh.is_none() {
                    break;
                }
            }
        }
    }
}

/// Perform a single signed-metadata fetch and turn it into a [`VersionCache`].
///
/// Signature verification and anti-rollback are enforced inside
/// `get_versions_for_platform` against the trusted Warren signing keys.
#[cfg(not(target_os = "android"))]
async fn check_once(
    platform: mullvad_update::api::MetaRepositoryPlatform,
    architecture: mullvad_update::format::Architecture,
    rollout_threshold_seed: Option<u32>,
    lowest_metadata_version: usize,
) -> anyhow::Result<VersionCache> {
    use anyhow::Context;
    use mullvad_update::api::HttpVersionInfoProvider;
    use mullvad_update::version::VersionParameters;
    use mullvad_update::version::rollout::Rollout;

    let current_version: Version = mullvad_version::VERSION
        .parse()
        .map_err(|err| anyhow::anyhow!("Failed to parse the running app version: {err}"))?;

    let response =
        HttpVersionInfoProvider::get_versions_for_platform(platform, lowest_metadata_version)
            .await
            .context("Failed to fetch signed version metadata")?;

    // Forced-update gate. The resulting bool flows to the GUI as
    // `AppVersionInfo.supported` and drives the blocking update screen.
    let current_version_supported =
        current_version_is_supported(&current_version, &response.signed);

    let metadata_version = response.signed.metadata_version;

    let rollout = match rollout_threshold_seed {
        Some(seed) => Rollout::threshold(seed, current_version.clone()),
        // No persisted rollout seed yet: only surface fully rolled-out
        // releases so a partially-rolled-out version is never suggested to a
        // client that has not been assigned a stable position in the rollout.
        None => Rollout::complete(),
    };

    let params = VersionParameters {
        architecture,
        rollout,
        // Linux ships no in-app installers, so accept installer-less releases
        // there; Windows and macOS require a matching installer.
        allow_empty: !cfg!(in_app_upgrade),
        lowest_metadata_version,
    };

    let version_info = VersionInfo::try_from_response(&params, response.signed)
        .context("Failed to parse version metadata for this platform")?;

    Ok(VersionCache {
        cache_version: current_version,
        current_version_supported,
        version_info,
        last_platform_header_check: SystemTime::now(),
        metadata_version,
        etag: None,
    })
}

/// Decide whether the running app version is still supported.
///
/// If the manifest declares a `minimum_supported_version`, the app is supported
/// iff it is at least that version (this is the forced-update lever: raise the
/// minimum to hard-block older clients). Otherwise fall back to the upstream
/// rule, "supported iff the running version is listed in `releases`". Dev builds
/// are always considered supported so local testing is never blocked.
#[cfg(not(target_os = "android"))]
fn current_version_is_supported(
    current_version: &Version,
    response: &mullvad_update::format::response::Response,
) -> bool {
    mullvad_update::version::is_current_version_supported(current_version, response)
}

/// Map the detected native CPU architecture to the metadata architecture enum.
#[cfg(not(target_os = "android"))]
fn native_update_architecture() -> Option<mullvad_update::format::Architecture> {
    use mullvad_update::format::Architecture;

    match talpid_platform_metadata::get_native_arch() {
        Ok(Some(talpid_platform_metadata::Architecture::X86)) => Some(Architecture::X86),
        Ok(Some(talpid_platform_metadata::Architecture::Arm64)) => Some(Architecture::Arm64),
        Ok(None) => None,
        Err(error) => {
            log::warn!("Failed to detect the native CPU architecture: {error}");
            None
        }
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::{Version, current_version_is_supported};
    use mullvad_update::format::release::Release;
    use mullvad_update::format::response::Response;
    use mullvad_update::version::rollout::Rollout;

    fn version(raw: &str) -> Version {
        raw.parse().expect("valid version")
    }

    fn response(minimum: Option<&str>, listed: &[&str]) -> Response {
        Response {
            metadata_version: 1,
            metadata_expiry: chrono::DateTime::UNIX_EPOCH,
            minimum_supported_version: minimum.map(version),
            releases: listed
                .iter()
                .map(|raw| Release {
                    version: version(raw),
                    changelog: String::new(),
                    installers: vec![],
                    rollout: Rollout::complete(),
                })
                .collect(),
        }
    }

    /// The forced-update lever: with a `minimum_supported_version`, clients
    /// below it are blocked and clients at or above it are supported, whether
    /// or not their exact version is listed.
    #[test]
    fn minimum_version_blocks_older_clients() {
        let response = response(Some("1.2.0"), &["1.0.0", "1.2.0"]);
        assert!(!current_version_is_supported(&version("1.0.0"), &response));
        assert!(current_version_is_supported(&version("1.2.0"), &response));
        assert!(current_version_is_supported(&version("1.5.0"), &response));
    }

    /// Without a declared minimum, support falls back to "is the running
    /// version present in the release list".
    #[test]
    fn without_minimum_falls_back_to_listed_versions() {
        let response = response(None, &["1.2.0"]);
        assert!(current_version_is_supported(&version("1.2.0"), &response));
        assert!(!current_version_is_supported(&version("1.0.0"), &response));
    }

    /// Dev builds are never blocked, even below the minimum and unlisted, so
    /// local development and testing are never locked out.
    #[test]
    fn dev_builds_are_always_supported() {
        let response = response(Some("9999.0.0"), &[]);
        assert!(current_version_is_supported(
            &version("1.0.0-dev-abcdef"),
            &response,
        ));
    }
}
