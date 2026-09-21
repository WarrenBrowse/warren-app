//! Which address families the fleet's entry hops can be dialed on.
//!
//! Host-testable, like [`crate::circuit_select`]: the answer is read off an
//! already-verified directory, so it is pure arithmetic over addresses.
//!
//! The Kotlin layer needs it because its retry loop parks when the device's
//! network cannot dial a relay, and "cannot" is a comparison between two sets:
//! the families the network carries, and the families the fleet publishes. It
//! used to assume the second was IPv4 and nothing else, which was true of every
//! deployment until a node bound a v6 listener, and which cost an IPv6-only
//! mobile network every connection it tried
//! (`incidents/2026-09-20-an-ipv6-only-mobile-network-*`).

/// An entry hop publishes an IPv4 address.
pub(crate) const FAMILY_V4: i32 = 1;
/// An entry hop publishes an IPv6 address.
pub(crate) const FAMILY_V6: i32 = 2;

/// The families at least one entry hop of `directory` publishes, as a bitmask
/// of [`FAMILY_V4`] and [`FAMILY_V6`]. `0` means the directory offers no
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

/// [`families_of`] over a verified directory.
pub(crate) fn directory_families(
    directory: &warren_discovery_core::VerifiedMultiHopDirectory,
) -> i32 {
    families_of(
        directory
            .nodes
            .iter()
            .map(|n| (&n.relay.endpoint, n.relay.endpoint_v6.as_ref())),
    )
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
        // and its families read off the verified nodes. The minted fleet is
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
        assert_eq!(directory_families(&directory), FAMILY_V4);
    }

    #[test]
    fn a_v6_primary_reports_v6() {
        // Nothing reads the family from a field NAME: a relay whose primary
        // address is v6 is a v6 entry, whatever the schema calls the slot.
        let v6 = addr("[2001:db8::10]:443");
        assert_eq!(families_of([(&v6, None)]), FAMILY_V6);
    }
}
