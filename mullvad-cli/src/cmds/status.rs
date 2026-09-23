use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use futures::StreamExt;
use mullvad_management_interface::{MullvadProxyClient, client::DaemonEvent};
use mullvad_types::{device::DeviceState, states::TunnelState};
use serde::Serialize;
use std::fmt::Debug;

use crate::format;

#[derive(Subcommand, Debug, PartialEq)]
pub enum Status {
    /// Listen for tunnel state changes
    Listen,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Enable verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Enable debug output
    #[arg(long, short = 'd', conflicts_with_all = ["verbose", "json"])]
    debug: bool,

    /// Format output as JSON
    #[arg(long, short = 'j', conflicts_with_all = ["verbose", "debug"])]
    json: bool,
}

impl Status {
    pub async fn listen(
        mut rpc: MullvadProxyClient,
        args: StatusArgs,
        mut previous_tunnel_state: TunnelState,
    ) -> Result<()> {
        let mut event_stream = rpc.events_listen().await?;
        while let Some(event) = event_stream.next().await {
            match event? {
                DaemonEvent::TunnelState(new_state) => {
                    if !print_debug_or_json(&args, "New tunnel state", &new_state)? {
                        format::print_state(&new_state, Some(&previous_tunnel_state), args.verbose);
                        previous_tunnel_state = new_state;
                    }
                }
                DaemonEvent::Settings(settings) => {
                    print_debug_or_json(&args, "New settings", &settings)?;
                }
                DaemonEvent::RelayList(relay_list) => {
                    print_debug_or_json(&args, "New relay list", &relay_list)?;
                }
                DaemonEvent::AppVersionInfo(app_version_info) => {
                    print_debug_or_json(&args, "New app version info", &app_version_info)?;
                }
                DaemonEvent::Device(device) => {
                    print_debug_or_json(&args, "Device event", &device)?;
                }
                DaemonEvent::NewAccessMethod(access_method) => {
                    print_debug_or_json(&args, "New access method", &access_method)?;
                }
                DaemonEvent::LeakDetected(leak) => {
                    #[derive(Debug, Serialize)]
                    struct Leak {
                        interface: String,
                        reachable: Vec<std::net::IpAddr>,
                    }
                    let leak = Leak {
                        interface: leak.interface,
                        reachable: leak.reachable_nodes,
                    };
                    print_debug_or_json(&args, "Leak detected", &leak)?;
                }
            }
        }
        Ok(())
    }
}

pub async fn handle(cmd: Option<Status>, args: StatusArgs) -> Result<()> {
    let mut rpc = MullvadProxyClient::new().await?;
    let state = rpc.get_tunnel_state().await?;
    // The device belongs to the account that owns Warren here: another
    // account still gets the tunnel state, just not the warning about it.
    if let Some(device) = unless_refused(rpc.get_device().await)? {
        print_account_logged_out(&state, &device);
    }

    if !print_debug_or_json(&args, "New tunnel state", &state)? {
        format::print_state(&state, None, args.verbose);
    }

    if cmd == Some(Status::Listen) {
        Status::listen(rpc, args, state).await?;
    }
    Ok(())
}

/// `result`, with a refusal from the daemon turned into `None`: what the
/// daemon keeps for the owner of Warren on this computer.
fn unless_refused<T>(
    result: Result<T, mullvad_management_interface::Error>,
) -> Result<Option<T>, mullvad_management_interface::Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(mullvad_management_interface::Error::Rpc(status))
            if status.code() == mullvad_management_interface::Code::PermissionDenied =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn print_account_logged_out(state: &TunnelState, device: &DeviceState) {
    match state {
        TunnelState::Connecting { .. } | TunnelState::Connected { .. } | TunnelState::Error(_) => {
            match device {
                DeviceState::LoggedOut => {
                    println!("Warning: You are not logged in to an account.")
                }
                DeviceState::Revoked => println!("Warning: This device has been revoked."),
                DeviceState::LoggedIn(_) => (),
            }
        }
        TunnelState::Disconnected { .. } | TunnelState::Disconnecting(_) => (),
    }
}

/// Print the given value as debug or JSON output based on the provided arguments.
///
/// Returns `true` if the value was printed. Returns `false` otherwise, i.e. if
/// both `args.debug` and `args.json` are `false`.
fn print_debug_or_json<T: Debug + Serialize>(
    args: &StatusArgs,
    debug_message: &str,
    t: &T,
) -> Result<bool> {
    if args.debug {
        println!("{debug_message}: {t:#?}");
        Ok(true)
    } else if args.json {
        let json = serde_json::to_string(&t).context("Failed to format output as JSON")?;
        println!("{json}");
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::unless_refused;
    use mullvad_management_interface::{Error, Status};

    #[test]
    fn a_refusal_leaves_nothing_to_show() {
        let refused = Err::<(), _>(Error::from(Status::permission_denied(
            "Warren is set up by another account on this computer",
        )));

        assert!(matches!(unless_refused(refused), Ok(None)));
    }

    #[test]
    fn any_other_failure_is_still_an_error() {
        let down = Err::<(), _>(Error::from(Status::unavailable("daemon is down")));

        assert!(unless_refused(down).is_err());
        assert!(matches!(unless_refused(Ok(7)), Ok(Some(7))));
    }
}
