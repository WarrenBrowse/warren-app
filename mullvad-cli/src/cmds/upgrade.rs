use anyhow::{Context, Result, anyhow, bail};
use futures::StreamExt;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::version::{AppUpgradeEvent, AppVersionInfo};

/// What `warren upgrade` does for a given answer of the daemon.
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    UpToDate,
    /// No published package fits this install: the website is the only way.
    Manual {
        version: String,
    },
    /// The upgrade still has to be downloaded and verified first.
    Download {
        version: String,
    },
    /// A verified package is already waiting.
    Install {
        version: String,
    },
}

fn plan(info: &AppVersionInfo) -> Plan {
    let Some(upgrade) = &info.suggested_upgrade else {
        return Plan::UpToDate;
    };
    let version = upgrade.version.to_string();
    if upgrade.manual_install_only {
        Plan::Manual { version }
    } else if upgrade.verified_installer_path.is_some() {
        Plan::Install { version }
    } else {
        Plan::Download { version }
    }
}

/// Download, verify and install the suggested upgrade, the way the app's
/// "Update" button does.
pub async fn handle() -> Result<()> {
    let mut rpc = MullvadProxyClient::new()
        .await
        .context("Failed to connect to the Warren daemon")?;
    let info = rpc
        .get_version_info()
        .await
        .context("Failed to get version info")?;

    let version = match plan(&info) {
        Plan::UpToDate => {
            println!("Warren {} is up to date", mullvad_version::VERSION);
            return Ok(());
        }
        Plan::Manual { version } => bail!(
            "Warren {version} is available, but no published package fits this install: \
             download it from the website"
        ),
        Plan::Download { version } => {
            download(&mut rpc, &version).await?;
            version
        }
        Plan::Install { version } => version,
    };

    install(&mut rpc, &version).await
}

async fn download(rpc: &mut MullvadProxyClient, version: &str) -> Result<()> {
    let mut events = rpc.app_upgrade_events_listen().await?;
    rpc.app_upgrade()
        .await
        .context("Failed to start the download")?;
    println!("Downloading Warren {version}");
    let mut last_progress = None;
    while let Some(event) = events.next().await {
        match event? {
            AppUpgradeEvent::DownloadProgress(progress) => {
                let step = progress.progress / 10;
                if last_progress != Some(step) {
                    last_progress = Some(step);
                    println!("  {}%", progress.progress);
                }
            }
            AppUpgradeEvent::VerifyingInstaller => println!("Verifying the package"),
            AppUpgradeEvent::VerifiedInstaller => return wait_until_held(rpc).await,
            AppUpgradeEvent::Aborted => bail!("The download was cancelled"),
            AppUpgradeEvent::Error(error) => bail!("The download failed: {error:?}"),
            AppUpgradeEvent::DownloadStarting => {}
        }
    }
    Err(anyhow!("The daemon stopped reporting the download"))
}

/// The downloader announces the verified package a moment before the daemon
/// records it as the one to install, so ask until it does.
async fn wait_until_held(rpc: &mut MullvadProxyClient) -> Result<()> {
    for _ in 0..50 {
        let info = rpc.get_version_info().await?;
        if matches!(plan(&info), Plan::Install { .. }) {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    bail!("The daemon verified the package but did not keep it")
}

/// Longer than an apt waiting out another package manager's lock (600 s).
#[cfg(target_os = "linux")]
const INSTALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

#[cfg(target_os = "linux")]
async fn install(rpc: &mut MullvadProxyClient, version: &str) -> Result<()> {
    use mullvad_update::linux::{UpgradeStatus, parse_status};

    let status_path = rpc
        .app_upgrade_install()
        .await
        .context("The daemon could not start the upgrade")?;
    println!("Installing Warren {version}; the daemon restarts on the way");
    // The daemon is replaced under this command, so the outcome is read from
    // the file the detached upgrade job writes, never asked of the daemon.
    let started = std::time::Instant::now();
    loop {
        if started.elapsed() > INSTALL_TIMEOUT {
            bail!(
                "The upgrade job gave no outcome in {} minutes; see app-upgrade.log in the \
                 daemon's log directory",
                INSTALL_TIMEOUT.as_secs() / 60
            );
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let contents = tokio::fs::read_to_string(&status_path).await.ok();
        match contents.as_deref().and_then(parse_status) {
            Some(UpgradeStatus::Running) => continue,
            Some(UpgradeStatus::Exited(0)) => {
                println!("Warren {version} is installed");
                return Ok(());
            }
            Some(UpgradeStatus::Exited(code)) => bail!(
                "The package manager failed (exit code {code}); its output is in \
                 the daemon's log directory, app-upgrade.log"
            ),
            None => bail!(
                "The upgrade job left no outcome in {}",
                status_path.display()
            ),
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[expect(clippy::unused_async)]
async fn install(_rpc: &mut MullvadProxyClient, version: &str) -> Result<()> {
    bail!("Warren {version} is downloaded and verified: open the app to install it")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mullvad_types::version::SuggestedUpgrade;

    fn info(upgrade: Option<(bool, bool)>) -> AppVersionInfo {
        AppVersionInfo {
            current_version_supported: true,
            suggested_upgrade: upgrade.map(|(verified, manual)| SuggestedUpgrade {
                version: "2100.1".parse().unwrap(),
                changelog: String::new(),
                changelog_translations: Default::default(),
                verified_installer_path: verified.then(|| "/var/cache/warren.deb".into()),
                manual_install_only: manual,
            }),
        }
    }

    #[test]
    fn nothing_to_do_without_a_suggested_upgrade() {
        assert_eq!(plan(&info(None)), Plan::UpToDate);
    }

    #[test]
    fn an_upgrade_is_downloaded_before_it_is_installed() {
        let version = "2100.1".to_owned();
        assert_eq!(
            plan(&info(Some((false, false)))),
            Plan::Download { version }
        );
    }

    #[test]
    fn a_verified_package_is_installed_without_downloading_it_again() {
        // The daemon ignores a download request once it holds a verified
        // package, so asking again would wait for events that never come.
        let version = "2100.1".to_owned();
        assert_eq!(plan(&info(Some((true, false)))), Plan::Install { version });
    }

    #[test]
    fn an_install_no_package_fits_is_sent_to_the_website() {
        let version = "2100.1".to_owned();
        assert_eq!(plan(&info(Some((false, true)))), Plan::Manual { version });
    }
}
