use crate::{BIN_NAME, format};
use anyhow::{Result, anyhow, bail};
use futures::{Stream, StreamExt};
use mullvad_management_interface::{MullvadProxyClient, client::DaemonEvent};
use mullvad_types::{device::DeviceState, states::TunnelState};

pub async fn connect(wait: bool) -> Result<()> {
    let mut rpc = MullvadProxyClient::new().await?;

    let listener = if wait {
        Some(rpc.events_listen().await?)
    } else {
        None
    };

    if rpc.connect_tunnel().await.map_err(connect_refusal)?
        && let Some(receiver) = listener
    {
        wait_for_tunnel_state(receiver, |state| match state {
            TunnelState::Connected { .. } => Ok(true),
            TunnelState::Error(_) => Err(anyhow!("Failed to connect")),
            _ => Ok(false),
        })
        .await?;
    }

    Ok(())
}

pub async fn disconnect(wait: bool) -> Result<()> {
    let mut rpc = MullvadProxyClient::new().await?;

    let listener = if wait {
        Some(rpc.events_listen().await?)
    } else {
        None
    };

    if rpc
        .disconnect_tunnel(&format!("{BIN_NAME} disconnect"))
        .await?
        && let Some(receiver) = listener
    {
        wait_for_tunnel_state(receiver, |state| Ok(state.is_disconnected())).await?;
    }

    Ok(())
}

pub async fn reconnect(wait: bool) -> Result<()> {
    let mut rpc = MullvadProxyClient::new().await?;

    let device_state = rpc.get_device().await?;
    print_account_loggedout(&device_state);

    let listener = if wait {
        Some(rpc.events_listen().await?)
    } else {
        None
    };

    let reconnecting = rpc.reconnect_tunnel().await?;
    if !reconnecting {
        bail!("Not reconnecting due to being in disconnected state")
    }
    if let Some(receiver) = listener {
        wait_for_tunnel_state(receiver, |state| match state {
            TunnelState::Connected { .. } => Ok(true),
            TunnelState::Error(_) => Err(anyhow!("Failed to reconnect")),
            _ => Ok(false),
        })
        .await?;
    }

    Ok(())
}

async fn wait_for_tunnel_state(
    mut event_stream: impl Stream<
        Item = std::result::Result<DaemonEvent, mullvad_management_interface::Error>,
    > + Unpin,
    matches_event: impl Fn(&TunnelState) -> Result<bool>,
) -> Result<()> {
    while let Some(state) = event_stream.next().await {
        if let DaemonEvent::TunnelState(new_state) = state? {
            format::print_state(&new_state, None, false);
            if matches_event(&new_state)? {
                return Ok(());
            }
        }
    }
    Err(anyhow!("Failed to wait for expected tunnel state"))
}

/// A connect the daemon refused on the account's side (logged out, device
/// revoked, access revoked), told with what fixes it when something does. Any
/// other failure passes through unchanged.
fn connect_refusal(error: mullvad_management_interface::Error) -> anyhow::Error {
    use mullvad_management_interface::Error;
    match error {
        Error::NotLoggedIn => anyhow!(
            "Not connecting: no account is logged in on this device. Log in with \
             `{BIN_NAME} account login` first."
        ),
        Error::DeviceRevoked => anyhow!(
            "Not connecting: this device has been revoked. Log in again with \
             `{BIN_NAME} account login`."
        ),
        Error::AccessRevoked(reason) => anyhow!(
            "Not connecting: {reason}. `{BIN_NAME} account standing` shows the warnings \
             behind it."
        ),
        other => other.into(),
    }
}

/// Warns about a revoked device, which the daemon holds in the blocked state
/// while the user wants a tunnel.
///
/// A connect needs no warning here: the daemon refuses it on a logged-out or
/// revoked device and says why.
fn print_account_loggedout(state: &DeviceState) {
    match state {
        DeviceState::Revoked => println!("Warning: This device has been revoked"),
        DeviceState::LoggedOut | DeviceState::LoggedIn(_) => return,
    };

    println!(
        "Warren is blocking all network traffic until you perform one of the following actions:

1. Log in to a Warren account with available time/credits.
2. Disconnect from Warren VPN. This can either be done from the CLI or the Warren App.

For more information, try 'warren account -h' or 'warren disconnect -h'"
    );
}

#[cfg(test)]
mod tests {
    use super::connect_refusal;
    use mullvad_management_interface::{Error, Status};

    #[test]
    fn a_logged_out_connect_names_the_login_command() {
        let message = connect_refusal(Error::NotLoggedIn).to_string();

        assert!(message.contains("no account is logged in"), "{message}");
        assert!(message.contains("account login"), "{message}");
    }

    #[test]
    fn a_connect_on_a_revoked_device_names_the_login_command() {
        let message = connect_refusal(Error::DeviceRevoked).to_string();

        assert!(message.contains("revoked"), "{message}");
        assert!(message.contains("account login"), "{message}");
    }

    #[test]
    fn a_connect_refused_for_a_revoked_access_gives_the_daemons_reason() {
        let reason = "access to this Warren account is revoked until 2028-01-01 00:00 UTC";

        let message = connect_refusal(Error::AccessRevoked(reason.to_owned())).to_string();

        assert!(message.starts_with("Not connecting: access"), "{message}");
        assert!(message.contains("2028-01-01"), "{message}");
        assert!(message.contains("account standing"), "{message}");
    }

    #[test]
    fn any_other_connect_failure_passes_through() {
        let error = connect_refusal(Error::from(Status::unavailable("daemon is down")));

        assert!(
            matches!(error.downcast_ref::<Error>(), Some(Error::Rpc(_))),
            "{error:?}"
        );
    }
}
