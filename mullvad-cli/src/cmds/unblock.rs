//! `warren unblock`: restore connectivity from any blocked state.
//!
//! Warren fails closed, so a machine can be left with no traffic in ways the
//! GUI cannot always undo: a tunnel that never comes up, a daemon that cannot
//! stay alive, or firewall state installed by a product environment that is no
//! longer on the machine. This command is the single answer to "the VPN took
//! my internet and I cannot get it back", on every platform.
//!
//! It escalates only as far as needed. Talking to the daemon covers the common
//! case and needs no privileges; the deep sweep needs administrator rights, so
//! it is attempted and, if refused, printed as the exact command to re-run.

use anyhow::Result;
use clap::Args;
use mullvad_management_interface::MullvadProxyClient;
use std::path::PathBuf;
use std::process::Command;

/// The privileged helper that owns the firewall sweep.
const SETUP_BIN: &str = if cfg!(windows) {
    "warren-setup.exe"
} else {
    "warren-setup"
};

#[derive(Args, Debug)]
pub struct Unblock {
    /// Also sweep firewall state left behind by other installs of Warren,
    /// including environments no longer present on this machine. Requires
    /// administrator privileges and stops the system service.
    #[arg(long)]
    deep: bool,
}

/// What `unblock` should do, decided before anything is executed so the
/// decision itself is testable without a daemon or a privileged process.
#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    /// The daemon answers: disarm lockdown and disconnect through it. No
    /// privileges needed, and it cannot fight a live daemon's own firewall
    /// management because the daemon performs it.
    ViaDaemon,
    /// No daemon, or the user asked for the deep sweep: run the privileged
    /// helper, which removes blocking state from every environment.
    ViaSetupBinary,
}

/// Chooses the plan. A reachable daemon is preferred unless the caller
/// explicitly asked to go deeper: the daemon path is reversible, needs no
/// elevation, and leaves the product managing its own firewall.
#[must_use]
pub const fn plan(daemon_reachable: bool, deep: bool) -> Plan {
    if daemon_reachable && !deep {
        Plan::ViaDaemon
    } else {
        Plan::ViaSetupBinary
    }
}

/// The command to re-run with privileges, worded for this platform.
///
/// Printed verbatim for a user who has no internet and is likely reading it
/// over the phone, so it stays a single short line with no placeholders to
/// substitute.
#[must_use]
pub fn elevation_hint(setup_path: &str) -> String {
    if cfg!(windows) {
        format!("Run in an administrator prompt: \"{setup_path}\" reset-firewall")
    } else {
        format!("Run as root: sudo \"{setup_path}\" reset-firewall")
    }
}

/// Absolute path of the privileged helper, which ships next to this binary.
///
/// Resolved from the running executable rather than `PATH`: a machine with
/// two installs must not have one product's CLI drive another's helper, and
/// on a blocked machine `PATH` may be the least trustworthy thing available.
fn setup_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("the CLI has no parent directory"))?;
    Ok(dir.join(SETUP_BIN))
}

impl Unblock {
    pub async fn handle(self) -> Result<()> {
        let client = MullvadProxyClient::new().await;

        match plan(client.is_ok(), self.deep) {
            Plan::ViaDaemon => {
                let mut rpc = client.expect("the plan only picks this path with a live daemon");
                // Order matters: with lockdown still armed, disconnecting
                // keeps the block in place by design.
                rpc.set_lockdown_mode(false).await?;
                rpc.disconnect_tunnel("warren unblock").await?;
                println!("Lockdown mode disabled and tunnel disconnected.");
                println!(
                    "If traffic is still blocked, re-run with --deep to sweep firewall state \
                     left by other installs."
                );
                Ok(())
            }
            Plan::ViaSetupBinary => {
                let setup = setup_binary()?;
                let setup_display = setup.display().to_string();

                if !setup.exists() {
                    anyhow::bail!(
                        "{SETUP_BIN} was not found next to this binary ({setup_display}); \
                         reinstall Warren to recover it"
                    );
                }

                println!("Sweeping firewall state from every Warren install...");
                let status = Command::new(&setup).arg("reset-firewall").status()?;

                if status.success() {
                    println!("Firewall reset. Internet access should be restored.");
                    return Ok(());
                }

                // Almost always a privilege refusal, and the user is offline,
                // so print the exact next step instead of an error code.
                println!("{}", elevation_hint(&setup_display));
                anyhow::bail!("{SETUP_BIN} reset-firewall failed ({status})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Plan, elevation_hint, plan};

    /// The daemon path is the one that needs no privileges, so it must win
    /// whenever it is available. Sending an offline user to an elevated prompt
    /// they may not be able to open is a worse answer than one RPC call.
    #[test]
    fn a_reachable_daemon_is_preferred() {
        assert_eq!(plan(true, false), Plan::ViaDaemon);
    }

    /// The failure this command exists for is a daemon that cannot help:
    /// dead, or alive but blind to the blocking state (installed by another
    /// environment). Both must land on the privileged sweep.
    #[test]
    fn recovery_falls_back_to_the_privileged_sweep() {
        assert_eq!(plan(false, false), Plan::ViaSetupBinary);
        assert_eq!(plan(true, true), Plan::ViaSetupBinary);
        assert_eq!(plan(false, true), Plan::ViaSetupBinary);
    }

    /// The hint is read aloud to someone with no internet, so it must contain
    /// the whole command and nothing to substitute.
    #[test]
    fn the_elevation_hint_is_a_complete_command() {
        let hint = elevation_hint("/opt/Warren VPN/resources/warren-setup");
        assert!(
            hint.contains("reset-firewall"),
            "the hint must name the subcommand: {hint}"
        );
        assert!(
            hint.contains("/opt/Warren VPN/resources/warren-setup"),
            "the hint must carry the full path: {hint}"
        );
        assert!(
            !hint.contains("<") && !hint.contains("{"),
            "the hint must have no placeholder to fill in: {hint}"
        );
    }
}
