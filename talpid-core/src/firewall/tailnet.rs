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

/// macOS tunnel devices: only one can be a tailnet interface there.
#[cfg(target_os = "macos")]
const TUNNEL_INTERFACE_PREFIX: &str = "utun";

/// An interface of the host, reduced to what qualification needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInterface {
    pub name: String,
    pub up: bool,
    /// A tunnel device (`utun` on macOS, a TUN device on Linux). Only a tunnel device can be a
    /// tailnet interface: a physical interface that happens to carry a CGNAT address (some carrier
    /// and hotel networks do) must not open its LAN.
    pub tunnel: bool,
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

/// Whether an address is one only Tailscale hands out. Every Tailscale node carries one from its
/// ULA prefix, which no other overlay uses, whereas the CGNAT block is shared with NetBird,
/// Cloudflare WARP and some carrier networks.
fn is_tailscale_ula(address: IpAddr) -> bool {
    TAILNET_V6.contains(address)
}

/// The names of the interfaces that coexist with Warren's tunnel: a tunnel device other than
/// `own_interface`, that is up and holds an address of Tailscale's ULA prefix. The IPv4 range is
/// then allowed on it as well, but never qualifies an interface on its own.
pub fn coexisting_tailnet_interfaces(
    interfaces: &[HostInterface],
    own_interface: Option<&str>,
) -> BTreeSet<String> {
    interfaces
        .iter()
        .filter(|interface| {
            interface.tunnel
                && Some(interface.name.as_str()) != own_interface
                && interface.up
                && interface.addresses.iter().copied().any(is_tailscale_ula)
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

#[cfg(target_os = "macos")]
fn is_tunnel_device(name: &str) -> bool {
    name.starts_with(TUNNEL_INTERFACE_PREFIX)
}

/// A TUN device: the kernel exposes `tun_flags` for those alone, whatever they are named.
#[cfg(target_os = "linux")]
fn is_tunnel_device(name: &str) -> bool {
    std::path::Path::new("/sys/class/net")
        .join(name)
        .join("tun_flags")
        .exists()
}

/// The routing table that sends the tailnet ranges to a coexisting tailnet interface on Linux.
///
/// Warren's split sends everything but its own carrier to its tunnel (`ip rule` priorities 49 to
/// 51), ahead of Tailscale's own rules, so tailnet traffic entered Warren's tunnel and died there.
/// A table of Warren's own, looked up at a priority ahead of those, keeps it on the tailnet
/// interface without depending on Tailscale's table number. When the interface goes away the
/// kernel drops its routes with it, the lookup misses, and the traffic falls back into Warren's
/// tunnel.
#[cfg(any(target_os = "linux", test))]
pub const LINUX_TAILNET_TABLE: u32 = 101;
/// The priority of the rules that look [`LINUX_TAILNET_TABLE`] up, ahead of Warren's split.
#[cfg(any(target_os = "linux", test))]
pub const LINUX_TAILNET_RULE_PREF: u32 = 48;

/// The `ip` invocations that make the Linux tailnet routing match `interface`: always the removal
/// of what a previous application left, then, with an interface, its routes and rules.
#[cfg(any(target_os = "linux", test))]
pub fn linux_route_plan(interface: Option<&str>) -> Vec<Vec<String>> {
    let table = LINUX_TAILNET_TABLE.to_string();
    let pref = LINUX_TAILNET_RULE_PREF.to_string();
    let ranges = [("-4", TAILNET_V4), ("-6", TAILNET_V6)];
    let words = |line: &[&str]| line.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    let mut plan = Vec::new();
    for (family, net) in ranges {
        let net = net.to_string();
        plan.push(words(&[
            family, "rule", "del", "to", &net, "lookup", &table, "pref", &pref,
        ]));
        plan.push(words(&[family, "route", "flush", "table", &table]));
    }
    let Some(interface) = interface else {
        return plan;
    };
    for (family, net) in ranges {
        plan.push(words(&[
            family,
            "route",
            "replace",
            &net.to_string(),
            "dev",
            interface,
            "table",
            &table,
        ]));
    }
    for (family, net) in ranges {
        plan.push(words(&[
            family,
            "rule",
            "add",
            "to",
            &net.to_string(),
            "lookup",
            &table,
            "pref",
            &pref,
        ]));
    }
    plan
}

/// Runs [`linux_route_plan`]. A removal of what is not there fails harmlessly and is ignored; an
/// addition that fails is reported, and leaves the tailnet in Warren's tunnel, which is closed.
#[cfg(target_os = "linux")]
pub fn apply_linux_routes(interface: Option<&str>) -> io::Result<()> {
    for args in linux_route_plan(interface) {
        let clearing = args.iter().any(|w| w == "del" || w == "flush");
        let status = std::process::Command::new("ip")
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() && !clearing {
            return Err(io::Error::other(
                "an ip command for the tailnet routes failed",
            ));
        }
    }
    Ok(())
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
                tunnel: is_tunnel_device(&entry.interface_name),
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
            tunnel: name.starts_with("utun") || name.starts_with("tailscale"),
            addresses: addresses.iter().map(|a| a.parse().unwrap()).collect(),
        }
    }

    #[test]
    fn a_linux_tun_holding_a_tailscale_address_qualifies() {
        let found = coexisting_tailnet_interfaces(
            &[
                iface("tailscale0", true, &["100.64.0.2", "fd7a:115c:a1e0::2"]),
                iface("eth0", true, &["192.168.139.40", "fd7a:115c:a1e0::9"]),
            ],
            Some("tun0"),
        );
        assert_eq!(found, names(&["tailscale0"]));
    }

    #[test]
    fn the_linux_route_plan_sends_both_tailnet_ranges_to_the_tailscale_interface() {
        let plan = linux_route_plan(Some("tailscale0"));
        let adds: Vec<String> = plan
            .iter()
            .map(|c| c.join(" "))
            .filter(|c| c.contains(" add ") || c.contains(" replace "))
            .collect();
        assert_eq!(
            adds,
            [
                "-4 route replace 100.64.0.0/10 dev tailscale0 table 101",
                "-6 route replace fd7a:115c:a1e0::/48 dev tailscale0 table 101",
                "-4 rule add to 100.64.0.0/10 lookup 101 pref 48",
                "-6 rule add to fd7a:115c:a1e0::/48 lookup 101 pref 48",
            ]
        );
    }

    #[test]
    fn the_linux_route_plan_always_clears_first_and_adds_nothing_without_an_interface() {
        let clears = linux_route_plan(None);
        assert!(!clears.is_empty());
        assert!(
            clears
                .iter()
                .all(|c| c.contains(&"del".to_owned()) || c.contains(&"flush".to_owned())),
            "{clears:?}"
        );
        let with = linux_route_plan(Some("tailscale0"));
        assert_eq!(
            &with[..clears.len()],
            &clears[..],
            "a re-apply starts from nothing"
        );
    }

    fn names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn an_up_utun_with_a_tailnet_v4_address_qualifies() {
        let found = coexisting_tailnet_interfaces(
            &[iface(
                "utun13",
                true,
                &["100.106.181.5", "fd7a:115c:a1e0::5", "fe80::1"],
            )],
            Some("utun14"),
        );
        assert_eq!(found, names(&["utun13"]));
    }

    /// NetBird holds an address of the CGNAT block on its utun, and Cloudflare WARP one of
    /// 100.96.0.0/12. Neither carries Tailscale's ULA prefix, and opening the tailnet rules on
    /// them would let the peers of those overlays in.
    #[test]
    fn a_cgnat_only_utun_of_another_overlay_does_not_qualify() {
        let found = coexisting_tailnet_interfaces(
            &[
                iface("utun8", true, &["100.64.12.9", "fe80::8"]),
                iface("utun9", true, &["100.96.0.3"]),
            ],
            Some("utun14"),
        );
        assert!(found.is_empty());
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
            &[iface("utun14", true, &["100.64.0.9", "fd7a:115c:a1e0::9"])],
            Some("utun14"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_down_utun_does_not_qualify() {
        let found = coexisting_tailnet_interfaces(
            &[iface(
                "utun13",
                false,
                &["100.106.181.5", "fd7a:115c:a1e0::5"],
            )],
            Some("utun14"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_physical_interface_with_a_cgnat_address_does_not_qualify() {
        let found = coexisting_tailnet_interfaces(
            &[iface("en0", true, &["100.72.3.4", "fd7a:115c:a1e0::4"])],
            Some("utun14"),
        );
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
        let found = coexisting_tailnet_interfaces(
            &[iface(
                "utun13",
                true,
                &["100.106.181.5", "fd7a:115c:a1e0::5"],
            )],
            None,
        );
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
            iface("utun13", true, &["100.106.181.5", "fd7a:115c:a1e0::5"]),
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
