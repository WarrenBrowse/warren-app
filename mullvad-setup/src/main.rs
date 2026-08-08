use clap::Parser;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_version::Version;
use std::{path::PathBuf, process, str::FromStr, sync::LazyLock};
use talpid_core::firewall::{self, Firewall};
use talpid_types::ErrorExt;
use tracing_subscriber::EnvFilter;

mod deadman;
#[cfg(target_os = "windows")]
mod driver_setup;
#[cfg(target_os = "windows")]
mod service;

static APP_VERSION: LazyLock<Version> =
    LazyLock::new(|| Version::from_str(mullvad_version::VERSION).unwrap());

#[repr(i32)]
enum ExitStatus {
    Ok = 0,
    Error = 1,
    VersionNotOlder = 2,
    DaemonNotRunning = 3,
}

impl From<Error> for ExitStatus {
    fn from(error: Error) -> ExitStatus {
        match error {
            Error::RpcConnectionError(_) => ExitStatus::DaemonNotRunning,
            _ => ExitStatus::Error,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to connect to RPC client")]
    RpcConnectionError(#[source] mullvad_management_interface::Error),

    #[error("RPC call failed")]
    DaemonRpcError(#[source] mullvad_management_interface::Error),

    #[error("This command cannot be run if the daemon is active")]
    DaemonIsRunning,

    #[error("Firewall error")]
    FirewallError(#[source] firewall::Error),

    #[error("Failed to restore DNS state")]
    DnsRecoveryError(#[source] talpid_dns::Error),

    #[error("Failed to obtain settings directory path")]
    SettingsPathError(#[source] mullvad_paths::Error),

    #[error("Failed to obtain cache directory path")]
    CachePathError(#[source] mullvad_paths::Error),

    #[error("Failed to read the device cache")]
    ReadDeviceCacheError(#[source] mullvad_daemon::device::Error),

    #[error("Failed to write the device cache")]
    WriteDeviceCacheError(#[source] mullvad_daemon::device::Error),

    #[error("Cannot parse the version string")]
    ParseVersionStringError,

    #[cfg(target_os = "windows")]
    #[error("Failed to start system service")]
    StartService(#[source] windows_service::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to query system service")]
    QueryServiceStatus(#[source] windows_service::Error),

    #[cfg(target_os = "windows")]
    #[error("Starting system service timed out")]
    StartServiceTimeout,

    #[cfg(target_os = "windows")]
    #[error("Failed to open service")]
    OpenService(#[source] windows_service::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to open service control manager")]
    OpenServiceControlManager(#[source] windows_service::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to open split tunnel device")]
    OpenDevice(#[source] std::io::Error),

    #[cfg(target_os = "windows")]
    #[error("IoControl operation failed")]
    IoControl(#[source] std::io::Error),

    #[cfg(target_os = "windows")]
    #[error("Split tunnel driver is in unexpected state: {0}")]
    UnexpectedDriverState(u64),

    #[cfg(target_os = "windows")]
    #[error("Failed to enumerate or uninstall devices")]
    DeviceEnumeration(#[source] std::io::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to control service")]
    ServiceControl(#[source] windows_service::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to load driver DLL")]
    LoadLibrary(#[source] std::io::Error),

    #[cfg(target_os = "windows")]
    #[error("Failed to delete driver")]
    DeleteDriver,
    #[error("Failed to manage the detached update guard")]
    Deadman(#[source] deadman::Error),
}

#[derive(Debug, Parser)]
#[command(author, version = mullvad_version::VERSION, about, long_about = None)]
#[command(propagate_version = true)]
#[command(
    arg_required_else_help = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
enum Cli {
    /// Move a running daemon into a blocking state and save its target state
    PrepareRestart,
    /// Remove any firewall rules introduced by the daemon
    ResetFirewall,
    /// Remove the current device from the active account
    RemoveDevice,
    /// Checks whether the given version is older than the current version
    IsOlderVersion {
        /// Version string to compare the current version
        #[arg(required = true)]
        old_version: String,
    },
    /// Stage the detached update guard and arm its OS timer.
    ///
    /// Run by every installer BEFORE it arms the lockdown and kills the daemon,
    /// so a machine whose daemon never comes back regains its internet on its
    /// own instead of waiting for a human with a terminal.
    ArmDeadman,
    /// Remove the detached update guard. Run by every installer on success.
    DisarmDeadman,
    /// The guard's own timer action. Resets the firewall only when no daemon is
    /// managing the machine, then removes the guard either way.
    DeadmanFire,
    /// Start the Mullvad daemon service
    #[cfg(target_os = "windows")]
    StartService,
    /// Manage Mullvad-installed Windows drivers
    #[cfg(target_os = "windows")]
    #[command(subcommand)]
    Driver(DriverCommand),
}

#[cfg(target_os = "windows")]
#[derive(Debug, clap::Subcommand)]
enum DriverCommand {
    /// Remove a Mullvad-installed Windows driver
    #[command(subcommand)]
    Remove(DriverRemoveCommand),
}

#[cfg(target_os = "windows")]
#[derive(Debug, clap::Subcommand)]
enum DriverRemoveCommand {
    /// Reset split tunnel driver, uninstall the ST device, stop and delete the service
    SplitTunnel,
    /// Remove the Wintun driver (loads wintun.dll from the same directory)
    Wintun,
    /// Uninstall an abandoned Wintun network adapter with the legacy GUID
    WintunAbandonedDevice,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let result = match Cli::parse() {
        Cli::PrepareRestart => prepare_restart().await,
        Cli::ArmDeadman => arm_deadman(),
        Cli::DisarmDeadman => disarm_deadman(),
        Cli::DeadmanFire => deadman_fire().await,
        Cli::ResetFirewall => reset_firewall().await,
        Cli::RemoveDevice => remove_device().await,
        Cli::IsOlderVersion { old_version } => {
            match is_older_version(&old_version) {
                // Returning exit status
                Ok(status) => process::exit(status as i32),
                Err(error) => Err(error),
            }
        }
        #[cfg(target_os = "windows")]
        Cli::StartService => service::start().await,
        #[cfg(target_os = "windows")]
        Cli::Driver(DriverCommand::Remove(cmd)) => match cmd {
            DriverRemoveCommand::SplitTunnel => driver_setup::remove_split_tunnel(),
            DriverRemoveCommand::Wintun => driver_setup::remove_wintun(),
            DriverRemoveCommand::WintunAbandonedDevice => {
                driver_setup::remove_wintun_abandoned_device()
            }
        },
    };

    if let Err(e) = result {
        eprintln!("{}", e.display_chain());
        process::exit(ExitStatus::from(e) as i32);
    }
}

fn is_older_version(old_version: &str) -> Result<ExitStatus, Error> {
    let parsed_version =
        Version::from_str(old_version).map_err(|_| Error::ParseVersionStringError)?;

    Ok(if *APP_VERSION > parsed_version {
        ExitStatus::Ok
    } else {
        ExitStatus::VersionNotOlder
    })
}

/// Stages a copy of this binary outside any install directory and registers a
/// one-shot OS timer that runs [`Cli::DeadmanFire`] on it.
fn arm_deadman() -> Result<(), Error> {
    deadman::arm().map_err(Error::Deadman)
}

/// Removes the guard and its timer. Succeeds when nothing was armed, so an
/// installer can call it unconditionally.
fn disarm_deadman() -> Result<(), Error> {
    deadman::disarm().map_err(Error::Deadman)
}

/// The timer fired. A daemon answering means the update finished and owns the
/// firewall; nobody answering means the machine is sealed with no owner.
async fn deadman_fire() -> Result<(), Error> {
    let daemon_answers = MullvadProxyClient::new().await.is_ok();
    let verdict = deadman::deadman_verdict(daemon_answers);
    eprintln!("Update guard fired: {verdict:?}");
    let outcome = match verdict {
        deadman::DeadmanVerdict::DaemonAlive => Ok(()),
        deadman::DeadmanVerdict::ResetFirewall => reset_firewall().await,
    };
    // The guard removes itself on EVERY path, including a failed reset: leaving
    // an armed timer behind would re-fire against a machine somebody has since
    // taken over.
    let _ = deadman::disarm();
    outcome
}

async fn prepare_restart() -> Result<(), Error> {
    let mut rpc = MullvadProxyClient::new()
        .await
        .map_err(Error::RpcConnectionError)?;
    rpc.prepare_restart().await.map_err(Error::DaemonRpcError)?;
    Ok(())
}

async fn reset_firewall() -> Result<(), Error> {
    // Ensure that the daemon isn't running
    if MullvadProxyClient::new().await.is_ok() {
        return Err(Error::DaemonIsRunning);
    }

    let firewall_result = Firewall::new(
        #[cfg(target_os = "linux")]
        mullvad_types::TUNNEL_FWMARK,
        #[cfg(target_os = "linux")]
        None,
        // TODO split-tunneling?
        #[cfg(target_os = "linux")]
        None,
    )
    .map_err(Error::FirewallError)
    // Sweep every product environment, not just this build's. This command is
    // the last resort for a machine that is blocked with no working product,
    // and the block it has to lift may have been installed by an environment
    // that is no longer present: an install that moved channel, or one
    // predating the current firewall identity scheme. A scoped reset silently
    // leaves that machine offline, with every product-side indicator green.
    .and_then(|mut firewall| {
        firewall
            .reset_policy_all_generations()
            .map_err(Error::FirewallError)
    });

    // Repair DNS too, and even if the firewall reset failed: this command is
    // the rescue for a machine whose daemon cannot come back up, and an
    // unblocked firewall with resolution still aimed at a dead in-tunnel
    // resolver would leave the user just as offline.
    let dns_result = talpid_dns::recover_after_crash().map_err(Error::DnsRecoveryError);

    firewall_result.and(dns_result)
}

async fn remove_device() -> Result<(), Error> {
    // Clears the local login-state cache if a login state is present.
    let (_cache_path, settings_path) = get_paths()?;
    let (cacher, state) = mullvad_daemon::device::DeviceCacher::new(&settings_path)
        .await
        .map_err(Error::ReadDeviceCacheError)?;
    if state.pubkey().is_some() {
        cacher
            .remove()
            .await
            .map_err(Error::WriteDeviceCacheError)?;
    }

    Ok(())
}

fn get_paths() -> Result<(PathBuf, PathBuf), Error> {
    let cache_path = mullvad_paths::cache_dir().map_err(Error::CachePathError)?;
    let settings_path = mullvad_paths::settings_dir().map_err(Error::SettingsPathError)?;
    Ok((cache_path, settings_path))
}
