//! Tailscale interfaces that coexist with the Warren tunnel on macOS.
//!
//! A tailnet interface carries inner traffic that its own process encrypts and sends out again
//! through sockets the firewall filters (they leave through the Warren tunnel or are blocked).
//! Letting the tailnet ranges pass on that one interface therefore opens nothing beyond it, while
//! blocking them makes every tailnet peer unreachable although Tailscale's underlay works.
//!
//! This module holds the pure decisions: which interfaces qualify, which passes they get and
//! when the set of them changed. Reading the host is confined to [`read_host_interfaces`].

use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};

/// The Tailscale IPv4 range (the CGNAT block, 100.64.0.0/10).
pub const TAILNET_V4: IpNetwork =
    IpNetwork::V4(Ipv4Network::new_checked(Ipv4Addr::new(100, 64, 0, 0), 10).unwrap());

/// The Tailscale IPv6 range (fd7a:115c:a1e0::/48).
pub const TAILNET_V6: IpNetwork = IpNetwork::V6(
    Ipv6Network::new_checked(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 0), 48).unwrap(),
);

/// Only a tunnel device can be a tailnet interface. A physical interface that happens to carry a
/// CGNAT address (some carrier and hotel networks do) must not open its LAN.
const TUNNEL_INTERFACE_PREFIX: &str = "utun";

/// An interface of the host, reduced to what qualification needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInterface {
    pub name: String,
    pub up: bool,
    pub addresses: Vec<IpAddr>,
}

/// Direction of a [`TailnetPass`], seen from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassDirection {
    /// Leaving the host, towards `net`.
    Out,
    /// Entering the host, coming from `net`.
    In,
}

/// One allowance: traffic on `interface` in `direction` with the far end inside `net`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailnetPass {
    pub interface: String,
    pub direction: PassDirection,
    pub net: IpNetwork,
}

fn is_tailnet_address(address: IpAddr) -> bool {
    TAILNET_V4.contains(address) || TAILNET_V6.contains(address)
}

/// The names of the interfaces that coexist with Warren's tunnel: a `utun` interface other than
/// `own_interface`, that is up and holds a tailnet address.
pub fn coexisting_tailnet_interfaces(
    interfaces: &[HostInterface],
    own_interface: Option<&str>,
) -> BTreeSet<String> {
    interfaces
        .iter()
        .filter(|interface| {
            interface.name.starts_with(TUNNEL_INTERFACE_PREFIX)
                && Some(interface.name.as_str()) != own_interface
                && interface.up
                && interface.addresses.iter().copied().any(is_tailnet_address)
        })
        .map(|interface| interface.name.clone())
        .collect()
}

/// The allowances for `interfaces`: both directions of both ranges, on each interface.
pub fn tailnet_passes(interfaces: &BTreeSet<String>) -> Vec<TailnetPass> {
    let mut passes = Vec::with_capacity(interfaces.len() * 4);
    for interface in interfaces {
        for net in [TAILNET_V4, TAILNET_V6] {
            for direction in [PassDirection::Out, PassDirection::In] {
                passes.push(TailnetPass {
                    interface: interface.clone(),
                    direction,
                    net,
                });
            }
        }
    }
    passes
}

/// Remembers the last set of coexisting interfaces, to tell whether the firewall must be
/// applied again.
#[derive(Debug, Default)]
pub struct SetTracker {
    last: BTreeSet<String>,
}

impl SetTracker {
    pub fn new(initial: BTreeSet<String>) -> Self {
        Self { last: initial }
    }

    /// Records `current` and returns whether it differs from the previous set.
    pub fn observe(&mut self, current: BTreeSet<String>) -> bool {
        if self.last == current {
            return false;
        }
        self.last = current;
        true
    }
}

/// Reads the host's interfaces, one entry per interface name.
pub fn read_host_interfaces() -> io::Result<Vec<HostInterface>> {
    use nix::net::if_::InterfaceFlags;
    use std::collections::BTreeMap;

    let mut by_name: BTreeMap<String, HostInterface> = BTreeMap::new();
    for entry in nix::ifaddrs::getifaddrs()? {
        let interface = by_name
            .entry(entry.interface_name.clone())
            .or_insert_with(|| HostInterface {
                name: entry.interface_name.clone(),
                up: entry.flags.contains(InterfaceFlags::IFF_UP),
                addresses: Vec::new(),
            });
        let Some(address) = entry.address else {
            continue;
        };
        if let Some(v4) = address.as_sockaddr_in() {
            interface.addresses.push(IpAddr::V4(v4.ip()));
        } else if let Some(v6) = address.as_sockaddr_in6() {
            interface.addresses.push(IpAddr::V6(v6.ip()));
        }
    }
    Ok(by_name.into_values().collect())
}

/// The coexisting tailnet interfaces of the host right now, none when it cannot be read: a
/// kill switch that cannot see the interfaces stays closed.
pub fn current_coexisting_tailnet_interfaces(own_interface: Option<&str>) -> BTreeSet<String> {
    match read_host_interfaces() {
        Ok(interfaces) => coexisting_tailnet_interfaces(&interfaces, own_interface),
        Err(error) => {
            log::warn!("Failed to read the host interfaces: {error}");
            BTreeSet::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(name: &str, up: bool, addresses: &[&str]) -> HostInterface {
        HostInterface {
            name: name.to_owned(),
            up,
            addresses: addresses.iter().map(|a| a.parse().unwrap()).collect(),
        }
    }

    fn names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn an_up_utun_with_a_tailnet_v4_address_qualifies() {
        let found = coexisting_tailnet_interfaces(
            &[iface("utun13", true, &["100.106.181.5", "fe80::1"])],
            Some("utun14"),
        );
        assert_eq!(found, names(&["utun13"]));
    }

    #[test]
    fn an_up_utun_with_only_a_tailnet_v6_address_qualifies() {
        let found = coexisting_tailnet_interfaces(
            &[iface("utun13", true, &["fd7a:115c:a1e0::1234"])],
            Some("utun14"),
        );
        assert_eq!(found, names(&["utun13"]));
    }

    #[test]
    fn the_warren_tunnel_itself_never_qualifies() {
        let found = coexisting_tailnet_interfaces(
            &[iface("utun14", true, &["100.64.0.9"])],
            Some("utun14"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_down_utun_does_not_qualify() {
        let found = coexisting_tailnet_interfaces(
            &[iface("utun13", false, &["100.106.181.5"])],
            Some("utun14"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_physical_interface_with_a_cgnat_address_does_not_qualify() {
        let found =
            coexisting_tailnet_interfaces(&[iface("en0", true, &["100.72.3.4"])], Some("utun14"));
        assert!(found.is_empty());
    }

    #[test]
    fn a_utun_without_a_tailnet_address_does_not_qualify() {
        let found = coexisting_tailnet_interfaces(
            &[iface(
                "utun3",
                true,
                &["10.8.0.2", "fe80::2", "100.128.0.1"],
            )],
            Some("utun14"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn without_a_warren_tunnel_a_tailnet_utun_still_qualifies() {
        let found =
            coexisting_tailnet_interfaces(&[iface("utun13", true, &["100.106.181.5"])], None);
        assert_eq!(found, names(&["utun13"]));
    }

    #[test]
    fn a_qualifying_interface_gets_both_directions_of_both_ranges_and_nothing_else() {
        let passes = tailnet_passes(&names(&["utun13"]));
        let expected = [
            (PassDirection::Out, "100.64.0.0/10"),
            (PassDirection::In, "100.64.0.0/10"),
            (PassDirection::Out, "fd7a:115c:a1e0::/48"),
            (PassDirection::In, "fd7a:115c:a1e0::/48"),
        ];
        assert_eq!(passes.len(), expected.len());
        for (direction, net) in expected {
            assert!(passes.contains(&TailnetPass {
                interface: "utun13".to_owned(),
                direction,
                net: net.parse().unwrap(),
            }));
        }
    }

    /// Whatever the interfaces are, no pass leaves a tunnel device or names a destination outside
    /// the two tailnet ranges.
    #[test]
    fn no_pass_touches_a_physical_interface_or_a_foreign_range() {
        let interfaces = [
            iface("en0", true, &["100.72.3.4", "192.168.1.5"]),
            iface("bridge100", true, &["fd7a:115c:a1e0::9"]),
            iface("utun13", true, &["100.106.181.5"]),
            iface("utun14", true, &["10.66.0.2"]),
            iface("utun15", true, &["fd7a:115c:a1e0::7"]),
        ];
        let found = coexisting_tailnet_interfaces(&interfaces, Some("utun14"));
        let passes = tailnet_passes(&found);
        assert!(!passes.is_empty());
        for pass in passes {
            assert!(pass.interface.starts_with("utun"), "{pass:?}");
            assert_ne!(pass.interface, "utun14");
            assert!(pass.net == TAILNET_V4 || pass.net == TAILNET_V6, "{pass:?}");
        }
    }

    #[test]
    fn no_interface_yields_no_pass() {
        assert!(tailnet_passes(&BTreeSet::new()).is_empty());
    }

    #[test]
    fn the_tracker_reports_a_change_only_when_the_set_differs() {
        let mut tracker = SetTracker::new(names(&["utun13"]));
        assert!(!tracker.observe(names(&["utun13"])));
        assert!(tracker.observe(names(&["utun13", "utun15"])));
        assert!(!tracker.observe(names(&["utun15", "utun13"])));
        assert!(tracker.observe(names(&[])));
        assert!(!tracker.observe(names(&[])));
    }

    #[test]
    fn the_tracker_notices_an_interface_that_moved_to_another_name() {
        let mut tracker = SetTracker::new(names(&["utun13"]));
        assert!(tracker.observe(names(&["utun16"])));
    }
}
