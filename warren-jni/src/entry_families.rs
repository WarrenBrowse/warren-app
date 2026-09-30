//! Which address families a circuit's candidate entries can be dialed on,
//! and which this device's network routes.
//!
//! Host-testable, like [`crate::circuit_select`]: the answer is read off an
//! already-verified directory, so it is pure arithmetic over addresses.
//!
//! The Kotlin layer needs it because its retry loop parks when the device's
//! network cannot dial a relay, and "cannot" is a comparison between two sets:
//! the families the network carries, and the families the entries the dial
//! may use publish. It used to assume the second was IPv4 and nothing else
//! (`incidents/2026-09-20-an-ipv6-only-mobile-network-*`), then read it off
//! the whole fleet, which one dual-stack node anywhere satisfied while the
//! pinned entry country published IPv4 only (forum topic 210).

/// An entry hop publishes an IPv4 address.
pub(crate) const FAMILY_V4: i32 = 1;
/// An entry hop publishes an IPv6 address.
pub(crate) const FAMILY_V6: i32 = 2;

/// The families at least one of `endpoints` publishes, as a bitmask of
/// [`FAMILY_V4`] and [`FAMILY_V6`]. `0` means they offer no
/// dialable address at all, which is a fleet-side fault rather than a network
/// one, and reads as "do not park waiting for a network that cannot help".
pub(crate) fn families_of<'a>(
    endpoints: impl IntoIterator<Item = (&'a std::net::SocketAddr, Option<&'a std::net::SocketAddr>)>,
) -> i32 {
    let mut mask = 0;
    for (primary, alt) in endpoints {
        for addr in std::iter::once(primary).chain(alt) {
            mask |= if addr.is_ipv6() { FAMILY_V6 } else { FAMILY_V4 };
        }
    }
    mask
}

/// The families the entries a circuit to the exit `want_exit` may use
/// publish, as a bitmask: the entry the circuit dials is one of these
/// ([`crate::circuit_select::entry_candidates`], the exit itself on a one-hop
/// circuit), so a network that shares no family with this answer can reach
/// none of them. `0` when the exit is not in the directory.
///
/// The whole fleet is the wrong comparison: with one dual-stack node
/// anywhere, an IPv6-only network looked dialable while the entry country the
/// user pinned published IPv4 only, and the precise "no dialable network"
/// cause never fired (forum topic 210).
pub(crate) fn candidate_families(
    directory: &warren_discovery_core::VerifiedMultiHopDirectory,
    want_exit: &[u8; 32],
    two_hop: bool,
    want_entry: Option<&[u8; 32]>,
    want_country: Option<&str>,
) -> i32 {
    let views: Vec<crate::circuit_select::NodeSel<'_>> = directory
        .nodes
        .iter()
        .map(|n| crate::circuit_select::NodeSel {
            exit_ed25519: &n.exit.exit_ed25519_pubkey,
            relay_id: &n.relay.relay_id,
            relay_ed25519: &n.relay.relay_ed25519_pubkey,
            country: &n.country,
        })
        .collect();
    let Some(exit_idx) = views.iter().position(|n| n.exit_ed25519 == want_exit) else {
        return 0;
    };
    let candidates = if two_hop {
        crate::circuit_select::entry_candidates(&views, exit_idx, want_entry, want_country)
    } else {
        vec![exit_idx]
    };
    families_of(candidates.iter().map(|&i| {
        let relay = &directory.nodes[i].relay;
        (&relay.endpoint, relay.endpoint_v6.as_ref())
    }))
}

/// The families this device routes now, as a bitmask, from the kernel's
/// route lookup on a socket carrying the same `VpnService.protect` escape as
/// the dial (so the answer describes the physical network, not the tunnel).
/// `connect(2)` on a datagram socket only looks the route up: no packet
/// leaves. `None` when the escape could not be installed.
#[cfg(any(test, feature = "tunnel"))]
pub(crate) fn measure_network_families() -> Option<i32> {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

    // Documentation addresses: routed by the default route like any public
    // address, and naming nobody.
    let probes = [
        (
            FAMILY_V4,
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
            SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 443)),
        ),
        (
            FAMILY_V6,
            SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)),
            SocketAddr::from((Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 443)),
        ),
    ];
    let mut mask = 0;
    for (family, bind, target) in probes {
        // A host without the family cannot even open a socket of it.
        let Ok(socket) = UdpSocket::bind(bind) else {
            continue;
        };
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if !warrenguard_transport::socket_protect::protect(socket.as_raw_fd()) {
                return None;
            }
        }
        if socket.connect(target).is_ok() {
            mask |= family;
        }
    }
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(raw: &str) -> std::net::SocketAddr {
        raw.parse().expect("static addr parses")
    }

    #[test]
    fn a_v4_only_fleet_reports_v4_only() {
        let v4 = addr("192.0.2.10:443");
        assert_eq!(families_of([(&v4, None)]), FAMILY_V4);
    }

    #[test]
    fn a_dual_stack_node_reports_both() {
        let v4 = addr("192.0.2.10:443");
        let v6 = addr("[2001:db8::10]:443");
        assert_eq!(families_of([(&v4, Some(&v6))]), FAMILY_V4 | FAMILY_V6);
    }

    #[test]
    fn one_dual_stack_node_among_v4_ones_is_enough() {
        // The Kotlin gate asks "can this network reach ANY entry", so one node
        // publishing v6 is what makes an IPv6-only network worth dialing.
        let v4 = addr("192.0.2.10:443");
        let other_v4 = addr("198.51.100.10:443");
        let v6 = addr("[2001:db8::10]:443");
        assert_eq!(
            families_of([(&v4, None), (&other_v4, Some(&v6))]),
            FAMILY_V4 | FAMILY_V6
        );
    }

    #[test]
    fn an_empty_directory_reports_nothing() {
        assert_eq!(families_of([]), 0);
    }

    #[test]
    fn a_real_verified_directory_reports_what_its_nodes_publish() {
        // The production call path, end to end on the host: a signed directory
        // is minted, verified through the same function the JNI binding uses,
        // and its candidate entries' families read off the verified nodes. The minted fleet is
        // v4-only, which is the shape the gate must keep answering for until a
        // node actually binds a listener on the other family.
        use ed25519_dalek::SigningKey;

        let (root, op, server) = (
            SigningKey::from_bytes(&[0x01; 32]),
            SigningKey::from_bytes(&[0x02; 32]),
            SigningKey::from_bytes(&[0x03; 32]),
        );
        let json = warren_discovery_core::test_helpers::mint_directory_json(
            &root,
            &op,
            &server,
            7,
            1000,
            1_000_000_000,
        );
        let server_pin = hex::encode(server.verifying_key().as_bytes());
        let root_pin = hex::encode(root.verifying_key().as_bytes());
        let directory = warren_discovery_core::verify_multihop_directory_any(
            &json,
            &[&server_pin],
            &[&root_pin],
        )
        .expect("the minted directory must verify");
        let exit = directory.nodes[1].exit.exit_ed25519_pubkey;
        assert_eq!(
            candidate_families(&directory, &exit, true, None, None),
            FAMILY_V4
        );
    }

    #[test]
    fn a_v6_primary_reports_v6() {
        // Nothing reads the family from a field NAME: a relay whose primary
        // address is v6 is a v6 entry, whatever the schema calls the slot.
        let v6 = addr("[2001:db8::10]:443");
        assert_eq!(families_of([(&v6, None)]), FAMILY_V6);
    }

    fn node(tag: u8, country: &str, v6: bool) -> warren_discovery_core::NodeEntry {
        let mut node = warren_app_routes::plan::fixture::test_node(tag, country, "City", 10);
        if v6 {
            node.relay.endpoint_v6 = Some(addr(&format!("[2001:db8::{tag}]:443")));
        }
        node
    }

    /// The live beta order of forum topic 210: RO and FR publish IPv4 only.
    fn topic_210_fleet() -> warren_discovery_core::VerifiedMultiHopDirectory {
        warren_app_routes::plan::fixture::directory(vec![
            node(1, "ro", false),
            node(2, "de", true),
            node(3, "fr", false),
            node(4, "fi", true),
        ])
    }

    #[test]
    fn a_pinned_v4_only_entry_country_offers_v4_only_whatever_the_fleet_publishes() {
        let dir = topic_210_fleet();
        // Exit FI, entry pinned to FR.
        let exit = dir.nodes[3].exit.exit_ed25519_pubkey;
        assert_eq!(
            candidate_families(&dir, &exit, true, None, Some("fr")),
            FAMILY_V4
        );
    }

    #[test]
    fn an_unpinned_two_hop_circuit_offers_what_any_distinct_entry_publishes() {
        let dir = topic_210_fleet();
        let exit = dir.nodes[1].exit.exit_ed25519_pubkey;
        assert_eq!(
            candidate_families(&dir, &exit, true, None, None),
            FAMILY_V4 | FAMILY_V6
        );
    }

    #[test]
    fn a_one_hop_circuit_offers_what_its_exit_node_publishes() {
        let dir = topic_210_fleet();
        let ro = dir.nodes[0].exit.exit_ed25519_pubkey;
        assert_eq!(candidate_families(&dir, &ro, false, None, None), FAMILY_V4);
        assert_eq!(candidate_families(&dir, &[0xEE; 32], false, None, None), 0);
    }

    #[test]
    fn the_network_probe_reports_the_family_this_host_routes() {
        // Precondition of every CI runner and dev machine: an IPv4 default
        // route. With no protector registered the probe is a plain route
        // lookup.
        let mask = measure_network_families().expect("no protector is registered on the host");
        assert_ne!(mask & FAMILY_V4, 0, "IPv4 is routed on this host");
        assert_eq!(mask & !(FAMILY_V4 | FAMILY_V6), 0);
    }
}
