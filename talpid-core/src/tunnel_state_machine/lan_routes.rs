//! The networks "Local network sharing" keeps outside the tunnel, published
//! for the tunnel's routes.
//!
//! The firewall lets traffic to those networks leave outside the tunnel; the
//! routes must send it there too, or it goes into the tunnel and never
//! reaches the local network. Both follow the same two values the state
//! machine holds, so they cannot disagree, and a tunnel follows a change
//! without reconnecting.

use ipnetwork::IpNetwork;
use tokio::sync::watch;

/// Publishes the networks to route outside the tunnel: the shared networks
/// while sharing is on, none while it is off.
pub(crate) struct LanRoutes(watch::Sender<Vec<IpNetwork>>);

impl LanRoutes {
    pub(crate) fn new(allow_lan: bool, lan_networks: &[IpNetwork]) -> Self {
        Self(watch::Sender::new(routed(allow_lan, lan_networks)))
    }

    /// Publishes the networks `allow_lan` and `lan_networks` route outside
    /// the tunnel. Followers are woken only when that set changed.
    pub(crate) fn update(&self, allow_lan: bool, lan_networks: &[IpNetwork]) {
        let routed = routed(allow_lan, lan_networks);
        self.0.send_if_modified(|current| {
            if *current == routed {
                return false;
            }
            *current = routed;
            true
        });
    }

    /// Follows the published networks.
    #[cfg_attr(not(any(target_os = "linux", test)), expect(dead_code))]
    pub(crate) fn subscribe(&self) -> watch::Receiver<Vec<IpNetwork>> {
        self.0.subscribe()
    }
}

fn routed(allow_lan: bool, lan_networks: &[IpNetwork]) -> Vec<IpNetwork> {
    if allow_lan {
        lan_networks.to_vec()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets() -> Vec<IpNetwork> {
        vec![
            "192.168.0.0/16".parse().expect("network"),
            "fd00::/8".parse().expect("network"),
        ]
    }

    #[test]
    fn the_shared_networks_are_routed_outside_the_tunnel_while_sharing_is_on() {
        assert_eq!(*LanRoutes::new(true, &nets()).subscribe().borrow(), nets());
    }

    #[test]
    fn no_network_is_routed_outside_the_tunnel_while_sharing_is_off() {
        assert!(
            LanRoutes::new(false, &nets())
                .subscribe()
                .borrow()
                .is_empty()
        );
    }

    #[test]
    fn a_follower_sees_a_change_of_the_setting_or_of_the_list() {
        let routes = LanRoutes::new(false, &nets());
        let mut follower = routes.subscribe();
        follower.mark_unchanged();

        routes.update(true, &nets());
        assert!(follower.has_changed().expect("sender alive"));
        assert_eq!(*follower.borrow_and_update(), nets());

        routes.update(true, &nets()[..1]);
        assert!(follower.has_changed().expect("sender alive"));
        assert_eq!(*follower.borrow_and_update(), nets()[..1].to_vec());
    }

    #[test]
    fn a_list_changed_while_sharing_is_off_wakes_no_follower() {
        let routes = LanRoutes::new(false, &nets());
        let mut follower = routes.subscribe();
        follower.mark_unchanged();

        routes.update(false, &nets()[..1]);

        assert!(!follower.has_changed().expect("sender alive"));
    }
}
