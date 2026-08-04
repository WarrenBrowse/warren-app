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
