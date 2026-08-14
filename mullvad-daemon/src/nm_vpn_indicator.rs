//! Making the desktop's own network indicator say a VPN is up.
//!
//! GNOME and KDE both read one thing: whether NetworkManager holds an
//! active connection whose type is `vpn`. Warren builds its tunnel itself,
//! so NetworkManager knows nothing about it and the desktop shows a plain
//! wired or wireless connection while the user is tunnelled.
//!
//! This watches tunnel state transitions and, while connected, asks
//! NetworkManager to hold a VPN connection standing for the live tunnel.
//! It is deliberately an observer: it sits outside the tunnel state machine
//! and can never fail a connection. Everything it does is best effort, and
//! a machine with no NetworkManager simply gets nothing.

use std::net::IpAddr;

use talpid_types::tunnel::TunnelStateTransition;

// Only the Linux implementation consumes these two, but they are plain
// platform-independent logic and their tests are worth running everywhere: a
// macOS dev breaking the multi-hop peer choice should find out locally, not on
// the Linux runner. So they stay compiled on every platform rather than being
// gated behind the same cfg as their only caller.
//
// Suppressing the resulting `dead_code` needs the cfg to name the exact build
// where the lint fires, because `expect` is itself linted when nothing fires:
// off Linux the function has no caller in the library, but the tests below do
// call it, and the struct is never reported at all (the function that builds it
// is the dead item rustc names). So only the function carries a suppression,
// and only outside the test build.

/// The tunnel as NetworkManager needs it described.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelDescription {
    /// Interface the engine created.
    pub interface: String,
    /// Address of the host the tunnel talks to. NetworkManager rejects a
    /// VPN configuration that carries no external gateway, so this is not
    /// optional. It stays on the machine: it is the peer the user's own
    /// routing table already names.
    pub gateway: IpAddr,
}

/// What the indicator should show for a given transition.
///
/// Only the connected state warrants a VPN connection. Connecting does not:
/// the tunnel carries nothing yet, and a desktop claiming VPN protection
/// before traffic flows through it would be saying something untrue.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "the NetworkManager indicator is Linux only")
)]
pub fn desired_indicator(transition: &TunnelStateTransition) -> Option<TunnelDescription> {
    let TunnelStateTransition::Connected(endpoint) = transition else {
        return None;
    };
    let interface = endpoint.tunnel_interface.clone()?;
    // On multi-hop the tunnel is established with the entry hop, and that
    // is the peer the machine actually sends packets to. Naming the exit
    // here would describe a host no local route points at.
    let peer = endpoint
        .entry_endpoint
        .as_ref()
        .unwrap_or(&endpoint.endpoint);
    Some(TunnelDescription {
        interface,
        gateway: peer.address.ip(),
    })
}

pub use imp::NmVpnIndicator;

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::mpsc;
    use std::thread;

    use talpid_dbus::network_manager::{NetworkManager, VpnIndicatorConnection};
    use talpid_types::tunnel::TunnelStateTransition;

    use super::{TunnelDescription, desired_indicator};

    /// Name the connection carries in the desktop's network menu.
    fn connection_id() -> String {
        warren_product_env::DISPLAY_NAME.to_owned()
    }

    enum Command {
        Publish(TunnelDescription),
        Withdraw,
    }

    /// Publishes the running tunnel to NetworkManager, off the daemon's
    /// task threads.
    ///
    /// The work is handed to a plain thread because the NetworkManager
    /// client is blocking, and because a slow or wedged NetworkManager must
    /// never hold up a tunnel state transition.
    pub struct NmVpnIndicator {
        commands: Option<mpsc::Sender<Command>>,
        last: Option<TunnelDescription>,
    }

    impl NmVpnIndicator {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel();
            let spawned = thread::Builder::new()
                .name("nm-vpn-indicator".to_owned())
                .spawn(move || run(&rx));
            let commands = match spawned {
                Ok(_handle) => Some(tx),
                Err(error) => {
                    log::warn!("Cannot run the VPN indicator: {error}");
                    None
                }
            };
            NmVpnIndicator {
                commands,
                last: None,
            }
        }

        pub fn on_tunnel_state_transition(&mut self, transition: &TunnelStateTransition) {
            let desired = desired_indicator(transition);
            if desired == self.last {
                return;
            }
            self.last = desired.clone();
            let Some(commands) = &self.commands else {
                return;
            };
            let command = match desired {
                Some(description) => Command::Publish(description),
                None => Command::Withdraw,
            };
            if commands.send(command).is_err() {
                log::debug!("The VPN indicator worker is gone");
            }
        }
    }

    /// Owns the published connection for as long as the daemon runs, and
    /// takes it down when the channel closes, so a daemon shutdown does not
    /// leave the desktop claiming a VPN.
    fn run(commands: &mpsc::Receiver<Command>) {
        let mut published: Option<VpnIndicatorConnection> = None;
        while let Ok(command) = commands.recv() {
            match command {
                Command::Publish(description) => {
                    withdraw(&mut published);
                    published = publish(&description);
                }
                Command::Withdraw => withdraw(&mut published),
            }
        }
        // The daemon is going away. Take the connection down rather than
        // leave the desktop claiming a VPN that has no daemon behind it.
        withdraw(&mut published);
    }

    fn publish(description: &TunnelDescription) -> Option<VpnIndicatorConnection> {
        let manager = match NetworkManager::new() {
            Ok(manager) => manager,
            Err(error) => {
                log::debug!("No NetworkManager to show the VPN in: {error}");
                return None;
            }
        };
        if let Err(error) = manager.ensure_network_manager_exists() {
            log::debug!("No NetworkManager to show the VPN in: {error}");
            return None;
        }
        match manager.publish_vpn_indicator(
            warren_product_env::NM_VPN_SERVICE,
            &connection_id(),
            &description.interface,
            description.gateway,
        ) {
            Ok(connection) => {
                log::debug!("Desktop now shows a VPN on {}", description.interface);
                Some(connection)
            }
            Err(error) => {
                log::debug!("Could not show the VPN in the desktop indicator: {error}");
                None
            }
        }
    }

    fn withdraw(published: &mut Option<VpnIndicatorConnection>) {
        let Some(connection) = published.take() else {
            return;
        };
        let Ok(manager) = NetworkManager::new() else {
            return;
        };
        if let Err(error) = manager.withdraw_vpn_indicator(connection) {
            // The plugin's own watchdog retracts the connection when the
            // interface goes, so this is a delay, never a stuck indicator.
            log::debug!("Could not take the VPN indicator down: {error}");
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use talpid_types::tunnel::TunnelStateTransition;

    /// Other platforms show a VPN through their own mechanism (Android's
    /// `VpnService`, iOS's packet tunnel provider) or not at all.
    pub struct NmVpnIndicator;

    impl NmVpnIndicator {
        pub fn new() -> Self {
            NmVpnIndicator
        }

        pub fn on_tunnel_state_transition(&mut self, _transition: &TunnelStateTransition) {}
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use talpid_types::net::{Endpoint, TransportProtocol, TunnelEndpoint, TunnelType};
    use talpid_types::tunnel::{ActionAfterDisconnect, TunnelStateTransition};

    use super::*;

    fn endpoint(address: &str) -> Endpoint {
        Endpoint {
            address: address.parse::<SocketAddr>().unwrap(),
            protocol: TransportProtocol::Udp,
        }
    }

    fn tunnel(interface: Option<&str>, entry: Option<&str>) -> TunnelEndpoint {
        TunnelEndpoint {
            endpoint: endpoint("198.51.100.9:443"),
            quantum_resistant: false,
            obfuscation: None,
            entry_endpoint: entry.map(endpoint),
            tunnel_interface: interface.map(str::to_owned),
            #[cfg(daita)]
            daita: false,
            effective_mtu: None,
            legs_bonded: 0,
            legs_downlink_stalled: 0,
            tunnel_type: TunnelType::Warren,
        }
    }

    #[test]
    fn shows_a_vpn_while_connected() {
        let transition = TunnelStateTransition::Connected(tunnel(Some("warren0"), None));

        let shown = desired_indicator(&transition).unwrap();

        assert_eq!(shown.interface, "warren0");
        assert_eq!(shown.gateway, "198.51.100.9".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn names_the_entry_hop_on_a_multi_hop_tunnel() {
        // The exit is not a host this machine sends packets to, so naming
        // it would describe a peer no local route points at.
        let transition =
            TunnelStateTransition::Connected(tunnel(Some("warren0"), Some("203.0.113.4:443")));

        let shown = desired_indicator(&transition).unwrap();

        assert_eq!(shown.gateway, "203.0.113.4".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn shows_nothing_while_still_connecting() {
        let transition = TunnelStateTransition::Connecting(tunnel(Some("warren0"), None));

        assert_eq!(desired_indicator(&transition), None);
    }

    #[test]
    fn shows_nothing_once_disconnecting() {
        let transition = TunnelStateTransition::Disconnecting(ActionAfterDisconnect::Nothing);

        assert_eq!(desired_indicator(&transition), None);
    }

    #[test]
    fn shows_nothing_when_the_tunnel_has_no_interface_to_point_at() {
        let transition = TunnelStateTransition::Connected(tunnel(None, None));

        assert_eq!(desired_indicator(&transition), None);
    }
}
