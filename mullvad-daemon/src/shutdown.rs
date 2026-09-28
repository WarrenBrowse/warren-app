use ctrlc;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Unable to attach ctrl-c handler")]
pub struct Error(#[from] ctrlc::Error);

pub fn set_shutdown_signal_handler(f: impl Fn() + 'static + Send) -> Result<(), Error> {
    ctrlc::set_handler(f)?;
    Ok(())
}

/// Returns true if the init system reported that the machine is not shutting down or entering
/// maintenance. When neither init system can answer, the return value is `false` and it is assumed
/// that the machine is shutting down, which keeps the firewall blocking.
///
/// systemd is asked first. On a distribution that does not run it (MX Linux and Devuan sysvinit
/// editions ship no systemd at all) the SysV runlevel answers instead: sysvinit switches to
/// runlevel 0 or 6 before running the `K*` stop scripts, so the runlevel separates a machine
/// shutdown from an administrator stopping the service. Without that fallback every
/// `service warren-daemon stop` on such a host would read as a shutdown and arm the kill switch,
/// leaving the machine offline with no daemon left to unblock it.
#[cfg(target_os = "linux")]
pub fn is_shutdown_user_initiated() -> bool {
    use talpid_types::ErrorExt;
    match talpid_dbus::systemd::is_host_running() {
        Ok(running) => running,
        Err(err) => {
            log::debug!(
                "{}",
                err.display_chain_with_msg(
                    "systemd could not be asked whether the host is running"
                )
            );
            match sysv_runlevel_is_running() {
                Some(running) => running,
                None => {
                    log::error!(
                        "Failed to determine if host is shutting down, assuming it is shutting down"
                    );
                    false
                }
            }
        }
    }
}

/// Reads the current SysV runlevel and reports whether the machine is staying up.
///
/// Returns `None` when the runlevel cannot be established, so the caller keeps its fail-closed
/// default rather than guessing.
#[cfg(target_os = "linux")]
fn sysv_runlevel_is_running() -> Option<bool> {
    let output = std::process::Command::new("runlevel").output().ok()?;
    if !output.status.success() {
        return None;
    }
    runlevel_output_is_running(&String::from_utf8_lossy(&output.stdout))
}

/// Parses the output of `runlevel` ("<previous> <current>") into "the machine is staying up".
///
/// Runlevels 0 (halt) and 6 (reboot) mean the machine is going down; the unknown runlevel and any
/// unparsable output yield `None`. Compiled in test builds on every platform so the parsing stays
/// covered wherever the suite runs.
#[cfg(any(target_os = "linux", test))]
fn runlevel_output_is_running(output: &str) -> Option<bool> {
    let current = output.split_whitespace().nth(1)?;
    match current {
        "0" | "6" => Some(false),
        "1" | "2" | "3" | "4" | "5" | "S" | "s" => Some(true),
        _ => None,
    }
}

/// On `SIGUSR1`, stop the daemon the way an app update does: the target state is saved and the
/// firewall keeps blocking, so the daemon that starts next resumes where this one stopped. The
/// systemd unit sends it for `systemctl restart` (`RestartKillSignal`), which would otherwise
/// arrive as a SIGTERM from a running host, read as a user stop, and open the firewall for the
/// length of the restart.
#[cfg(target_os = "linux")]
pub fn install_restart_signal_handler(commands: crate::DaemonCommandSender) -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigusr1 = signal(SignalKind::user_defined1())?;
    tokio::spawn(async move {
        if sigusr1.recv().await.is_some() {
            log::warn!("SIGUSR1 caught, stopping for a restart with the firewall kept blocking");
            let _ = commands.send(crate::DaemonCommand::PrepareRestart(true));
        }
    });
    Ok(())
}

/// Currently returns false all of the time to ensure that no leaks occur during shutdown.
// FIXME: implement shutdown detection - the current implementation will always block network
// traffic when the daemon is shut down.
#[cfg(target_os = "macos")]
pub fn is_shutdown_user_initiated() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::runlevel_output_is_running;

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn sigusr1_stops_the_daemon_as_a_restart_that_keeps_blocking() {
        use crate::{DaemonCommand, DaemonCommandChannel, InternalDaemonEvent};
        use futures::StreamExt;

        let mut channel = DaemonCommandChannel::new();
        super::install_restart_signal_handler(channel.sender()).unwrap();

        nix::sys::signal::raise(nix::sys::signal::Signal::SIGUSR1).unwrap();

        let event =
            tokio::time::timeout(std::time::Duration::from_secs(5), channel.receiver.next())
                .await
                .expect("no command after SIGUSR1");
        assert!(matches!(
            event,
            Some(InternalDaemonEvent::Command(DaemonCommand::PrepareRestart(
                true
            )))
        ));
    }

    #[test]
    fn halt_and_reboot_runlevels_are_a_machine_shutdown() {
        assert_eq!(runlevel_output_is_running("5 0\n"), Some(false));
        assert_eq!(runlevel_output_is_running("3 6\n"), Some(false));
    }

    #[test]
    fn multi_user_runlevels_mean_the_machine_stays_up() {
        assert_eq!(runlevel_output_is_running("N 5\n"), Some(true));
        assert_eq!(runlevel_output_is_running("N 2\n"), Some(true));
    }

    #[test]
    fn the_early_boot_runlevel_means_the_machine_stays_up() {
        assert_eq!(runlevel_output_is_running("N S\n"), Some(true));
    }

    #[test]
    fn an_unreadable_runlevel_yields_no_answer() {
        assert_eq!(runlevel_output_is_running("unknown\n"), None);
        assert_eq!(runlevel_output_is_running(""), None);
        assert_eq!(runlevel_output_is_running("N ?\n"), None);
    }
}
