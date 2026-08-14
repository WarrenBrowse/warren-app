//! Observation of a dual-homed host: two or more active non-tunnel interfaces
//! holding an IPv4 address on the default gateway's subnet.
//!
//! # Why (a shape that loses half the downlink and says nothing)
//!
//! When a host reaches the same LAN through two interfaces, the return path of
//! the carrier's UDP flow is chosen by the LAN, not by us: replies can come back
//! on the interface the socket is not bound to and be dropped, so the tunnel
//! keeps sending and the downlink loses a large fraction of its datagrams with
//! every layer above reporting a healthy connection. The carrier discovery reads
//! only THE default route, so it cannot see the second interface at all.
//!
//! This module is an INDICATOR, never a guard: it takes no action and changes no
//! configuration. It exists so the one line a problem report needs is in the log
//! at connect time, instead of being reconstructed from datagram counters days
//! later.
//!
//! Interface NAMES are the only datum logged. A name separates "two physical
//! NICs on one LAN" from "another VPN owns the default route"; the addresses
//! that would complete the picture are identity material and stay out.

use std::net::Ipv4Addr;

/// Name prefixes of devices that are not a physical uplink.
///
/// The tunnel's own `utun` is the one that matters (its address is handed out by
/// the exit and could land on any subnet, this host's LAN included), the rest are
/// listed so a host running other tunnels or a container bridge does not read as
/// dual-homed. Matching is on the prefix because every one of these families is
/// numbered (`utun0`, `ipsec1`, `bridge100`).
const NON_UPLINK_PREFIXES: &[&str] = &[
    "utun", "tun", "tap", "ipsec", "ppp", "gif", "stf", "wg", "lo", "awdl", "llw", "bridge",
];

fn is_uplink(name: &str) -> bool {
    !NON_UPLINK_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, netmask: Ipv4Addr) -> bool {
    let mask = u32::from(netmask);
    u32::from(a) & mask == u32::from(b) & mask
}

/// Names of the distinct uplink interfaces holding an IPv4 address on the same
/// subnet as `gateway`, sorted and deduplicated.
///
/// Pure over the enumeration (`(interface name, address, netmask)`) so the
/// classification is testable without an operating system. Two or more names
/// mean the host is dual-homed onto the gateway's LAN.
///
/// A `/32` netmask is skipped: it describes a host route with no subnet to
/// share, so every address would trivially "match" the gateway only when equal
/// to it, and a point-to-point link would read as a second home.
#[must_use]
pub(crate) fn uplinks_on_gateway_subnet(
    addrs: &[(String, Ipv4Addr, Ipv4Addr)],
    gateway: Ipv4Addr,
) -> Vec<String> {
    let mut names: Vec<String> = addrs
        .iter()
        .filter(|(name, _, _)| is_uplink(name))
        .filter(|(_, _, netmask)| !netmask.is_unspecified() && *netmask != Ipv4Addr::BROADCAST)
        .filter(|(_, ip, netmask)| same_subnet(*ip, gateway, *netmask))
        .map(|(name, _, _)| name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Every `(interface, IPv4, netmask)` the host currently carries on an interface
/// that is both up and running.
///
/// Interfaces that are configured but not running (an unplugged Ethernet
/// adapter keeps its address on macOS) are excluded here rather than in
/// [`uplinks_on_gateway_subnet`]: a link that carries nothing cannot steal a
/// reply.
#[cfg(target_os = "macos")]
#[must_use]
pub(crate) fn enumerate_running_ipv4_uplinks() -> Vec<(String, Ipv4Addr, Ipv4Addr)> {
    use nix::net::if_::InterfaceFlags;

    let Ok(addrs) = nix::ifaddrs::getifaddrs() else {
        return Vec::new();
    };
    let running = InterfaceFlags::IFF_UP | InterfaceFlags::IFF_RUNNING;
    addrs
        .filter(|ifa| ifa.flags.contains(running))
        .filter_map(|ifa| {
            let ip = ifa.address.as_ref()?.as_sockaddr_in()?.ip();
            let netmask = ifa.netmask.as_ref()?.as_sockaddr_in()?.ip();
            Some((ifa.interface_name.clone(), ip, netmask))
        })
        .collect()
}

/// The dual-homing observation for the host's current default gateway: the
/// uplink names sharing its subnet, or an empty vector when there is nothing to
/// report (single-homed, or an IPv6 default route this check does not cover).
#[cfg(target_os = "macos")]
#[must_use]
pub(crate) fn observe_dual_homing(gateway: std::net::IpAddr) -> Vec<String> {
    let std::net::IpAddr::V4(gateway) = gateway else {
        return Vec::new();
    };
    let names = uplinks_on_gateway_subnet(&enumerate_running_ipv4_uplinks(), gateway);
    if names.len() < 2 { Vec::new() } else { names }
}

/// Emit the connect-time warning when the host is dual-homed onto the gateway's
/// LAN. Called once per connect, right after the default route is resolved.
#[cfg(target_os = "macos")]
pub(crate) fn warn_if_dual_homed(gateway: std::net::IpAddr) {
    let names = observe_dual_homing(gateway);
    if names.is_empty() {
        return;
    }
    log::warn!(
        "Warren: this host reaches the default gateway's subnet through {} active interfaces \
         ({}). The LAN, not this client, picks which one carries the carrier's replies, so part \
         of the downlink can be dropped while the tunnel still reports Connected. Leaving one \
         interface on that subnet is the fix; nothing here changes the configuration.",
        names.len(),
        names.join(", ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(name: &str, ip: &str, netmask: &str) -> (String, Ipv4Addr, Ipv4Addr) {
        (
            name.to_owned(),
            ip.parse().unwrap(),
            netmask.parse().unwrap(),
        )
    }

    fn gateway() -> Ipv4Addr {
        "192.168.1.1".parse().unwrap()
    }

    #[test]
    fn a_single_uplink_on_the_gateway_subnet_is_not_dual_homed() {
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("lo0", "127.0.0.1", "255.0.0.0"),
        ];
        assert_eq!(uplinks_on_gateway_subnet(&addrs, gateway()), vec!["en0"]);
    }

    #[test]
    fn two_uplinks_on_the_gateway_subnet_are_both_reported() {
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("en5", "192.168.1.77", "255.255.255.0"),
        ];
        assert_eq!(
            uplinks_on_gateway_subnet(&addrs, gateway()),
            vec!["en0", "en5"]
        );
    }

    #[test]
    fn an_uplink_on_another_subnet_is_ignored() {
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("en5", "10.0.0.4", "255.255.255.0"),
        ];
        assert_eq!(uplinks_on_gateway_subnet(&addrs, gateway()), vec!["en0"]);
    }

    #[test]
    fn tunnel_interfaces_never_count_as_a_second_home() {
        // The exit hands out the tunnel address, so it can legitimately land on
        // this host's LAN subnet. Counting it would report every connected
        // session as dual-homed.
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("utun4", "192.168.1.9", "255.255.255.0"),
            addr("ipsec0", "192.168.1.10", "255.255.255.0"),
        ];
        assert_eq!(uplinks_on_gateway_subnet(&addrs, gateway()), vec!["en0"]);
    }

    #[test]
    fn an_interface_with_several_addresses_is_reported_once() {
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("en0", "192.168.1.43", "255.255.255.0"),
        ];
        assert_eq!(uplinks_on_gateway_subnet(&addrs, gateway()), vec!["en0"]);
    }

    #[test]
    fn a_host_route_netmask_is_not_a_shared_subnet() {
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("en5", "192.168.1.1", "255.255.255.255"),
        ];
        assert_eq!(uplinks_on_gateway_subnet(&addrs, gateway()), vec!["en0"]);
    }

    #[test]
    fn a_wider_netmask_on_the_second_uplink_still_shares_the_subnet() {
        // The incident shape: a second adapter handed a /16 by another DHCP
        // server still receives the LAN's broadcast domain.
        let addrs = vec![
            addr("en0", "192.168.1.42", "255.255.255.0"),
            addr("en5", "192.168.9.7", "255.255.0.0"),
        ];
        assert_eq!(
            uplinks_on_gateway_subnet(&addrs, gateway()),
            vec!["en0", "en5"]
        );
    }
}
