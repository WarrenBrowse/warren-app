//! Pure multi-hop circuit selection for the Android tunnel.
//!
//! Host-testable, like [`crate::natpmp_follow`]: the datapath that actually
//! dials the chosen node lives in the Android-gated [`crate::tunnel`] module,
//! but the decision of WHICH directory node is the entry and which is the exit
//! is pure index arithmetic over the already-verified directory, unit-tested
//! on the host.
//!
//! Single-hop (the 1-hop circuit) collapses the entry onto the exit node so
//! the dispatch frame carries `exit_id == the dialed node` and that node's
//! unified `:443` dispatcher terminates locally (no outbound dial). Mirrors
//! iOS `select_one_hop` and the desktop daemon `select_one_hop_circuit`
//! (relay index == exit index). Two-hop keeps a DISTINCT entry and FAILS
//! CLOSED rather than silently collapsing, so an opted-in 2-hop request is
//! never downgraded to a 1-hop circuit (a privacy downgrade).

/// The per-node fields circuit selection reads, decoupled from the signed
/// `warren_discovery_core::NodeEntry` so the selection is testable without
/// constructing (and signing) a whole directory. Byte-only: selection never
/// inspects signatures, because it runs over an already-verified directory.
#[derive(Clone, Copy)]
pub(crate) struct NodeSel<'a> {
    /// The node's exit identity (Ed25519), matched against the requested exit.
    pub exit_ed25519: &'a [u8; 32],
    /// The node's relay routing tag, used to test entry/exit distinctness.
    pub relay_id: &'a [u8; 16],
    /// The node's relay identity (Ed25519), matched against an entry hint.
    pub relay_ed25519: &'a [u8; 32],
    /// ISO 3166-1 alpha-2 country, matched case-insensitively against the
    /// entry-country hint.
    pub country: &'a str,
}

/// Why circuit selection failed. Both variants are fail-closed: the caller
/// stores `Disconnected` and never serves a different topology than requested.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CircuitSelectError {
    /// The requested exit pubkey is not present in the verified directory.
    ExitNotInDirectory,
    /// A 2-hop circuit was requested but no node distinct from the exit
    /// exists, so no distinct entry relay can front it. Collapsing onto the
    /// exit here would silently downgrade an opted-in 2-hop request to a
    /// 1-hop circuit (a privacy downgrade), so selection fails closed instead.
    NoDistinctEntry,
    /// Entries exist, and this host's network routes none of them: every
    /// candidate publishes only address families the network does not carry
    /// (a v4-only entry on an IPv6-only mobile network, forum topic 210). The
    /// candidates are never widened past the pinned entry country to escape
    /// it; the user is told the network is the cause instead.
    NoReachableEntry,
}

/// The entries a two-hop circuit to `nodes[exit_idx]` may use, as indices
/// into `nodes`, in order of preference: the pubkey hint first when it names
/// a node distinct from the exit, then the nodes of the entry country, or
/// every distinct node when the country is unset or has no node that can
/// front this exit.
///
/// A country that does have such nodes is a hard bound: nodes outside it are
/// not candidates, so a network that reaches none of them ends in
/// [`CircuitSelectError::NoReachableEntry`] rather than in a silent move to a
/// country the user did not choose.
pub(crate) fn entry_candidates(
    nodes: &[NodeSel<'_>],
    exit_idx: usize,
    want_entry: Option<&[u8; 32]>,
    want_country: Option<&str>,
) -> Vec<usize> {
    let exit_relay_id = nodes[exit_idx].relay_id;
    let distinct = |i: &usize| nodes[*i].relay_id != exit_relay_id;
    // Trim/empty-filter the country hint here so an empty string means "any"
    // rather than matching a blank country field.
    let want_country = want_country.map(str::trim).filter(|s| !s.is_empty());
    let hinted = want_entry
        .and_then(|w| (0..nodes.len()).find(|i| distinct(i) && nodes[*i].relay_ed25519 == w));
    let in_country: Vec<usize> = want_country
        .map(|c| {
            (0..nodes.len())
                .filter(|i| distinct(i) && nodes[*i].country.eq_ignore_ascii_case(c))
                .collect()
        })
        .unwrap_or_default();
    let rest: Vec<usize> = if in_country.is_empty() {
        (0..nodes.len()).filter(distinct).collect()
    } else {
        in_country
    };
    let mut out: Vec<usize> = hinted.into_iter().collect();
    out.extend(rest.into_iter().filter(|i| Some(*i) != hinted));
    out
}

/// Resolve `(entry_idx, exit_idx)` into `nodes` for the requested circuit.
///
/// `two_hop == false` is the single-hop 1-hop circuit: entry and exit collapse
/// onto the SAME node, so the returned indices are equal. `two_hop == true`
/// picks a DISTINCT entry among [`entry_candidates`] and fails closed when no
/// distinct node exists. The precedence and distinctness rules match the
/// shipping desktop / iOS 2-hop selection so all three clients build the same
/// circuit shape.
///
/// `reachable` receives the candidate entries in order of preference and
/// returns those this host's network can route, in the same order (production
/// passes [`probe_reachable`]). The first one is the entry: on a network that
/// routes every family the order, and so the circuit, is unchanged.
pub(crate) fn select_circuit_indices(
    nodes: &[NodeSel<'_>],
    want_exit: &[u8; 32],
    two_hop: bool,
    want_entry: Option<&[u8; 32]>,
    want_country: Option<&str>,
    reachable: impl FnOnce(&[usize]) -> Vec<usize>,
) -> Result<(usize, usize), CircuitSelectError> {
    let exit_idx = nodes
        .iter()
        .position(|n| n.exit_ed25519 == want_exit)
        .ok_or(CircuitSelectError::ExitNotInDirectory)?;

    let candidates = if two_hop {
        entry_candidates(nodes, exit_idx, want_entry, want_country)
    } else {
        // 1-hop circuit: the exit node is also the entry relay, so the setup
        // frame's exit_id names the dialed node and it terminates locally.
        vec![exit_idx]
    };
    if candidates.is_empty() {
        return Err(CircuitSelectError::NoDistinctEntry);
    }
    let entry_idx = reachable(&candidates)
        .first()
        .copied()
        .ok_or(CircuitSelectError::NoReachableEntry)?;
    Ok((entry_idx, exit_idx))
}

/// The entries among `candidates` (indices into `relays`) this host can dial
/// now, in the caller's order, from the engine's own kernel probe
/// ([`warrenguard_transport::reachable_entries`]): the same route lookup and
/// the same `VpnService.protect` escape as the dial socket, so what it
/// measures is the physical network rather than the tunnel being replaced.
///
/// `bind_addr` is the address the supervisor dials from. A probe that cannot
/// answer (the protect hook refused the socket) keeps the candidate, so the
/// filter only ever removes an entry the kernel said it cannot route.
#[cfg(any(test, feature = "tunnel"))]
pub(crate) fn probe_reachable<'a>(
    relays: impl Fn(usize) -> &'a warrenguard_multihop::RelayDescriptorSigned,
    candidates: &[usize],
    bind_addr: std::net::SocketAddr,
) -> Vec<usize> {
    warrenguard_transport::reachable_entries(candidates.iter().map(|&i| relays(i)), bind_addr, None)
        .map(|kept| kept.into_iter().map(|k| candidates[k]).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{CircuitSelectError, NodeSel, entry_candidates, select_circuit_indices};

    /// Owns the byte buffers a [`NodeSel`] borrows, so a test can build a
    /// directory of nodes without constructing (and signing) real descriptors.
    struct OwnedNode {
        exit_ed: [u8; 32],
        relay_id: [u8; 16],
        relay_ed: [u8; 32],
        country: String,
    }

    impl OwnedNode {
        fn view(&self) -> NodeSel<'_> {
            NodeSel {
                exit_ed25519: &self.exit_ed,
                relay_id: &self.relay_id,
                relay_ed25519: &self.relay_ed,
                country: &self.country,
            }
        }
    }

    /// A node whose exit id, relay id and relay pubkey are all derived from
    /// `tag`, so the exit is addressed by `[tag; 32]` and the relay-entry hint
    /// by `[tag + 1; 32]`.
    fn node(tag: u8, country: &str) -> OwnedNode {
        OwnedNode {
            exit_ed: [tag; 32],
            relay_id: [tag; 16],
            relay_ed: [tag.wrapping_add(1); 32],
            country: country.to_owned(),
        }
    }

    fn views(nodes: &[OwnedNode]) -> Vec<NodeSel<'_>> {
        nodes.iter().map(OwnedNode::view).collect()
    }

    /// A network that routes every candidate: selection is then exactly the
    /// preference order.
    fn all_reachable(candidates: &[usize]) -> Vec<usize> {
        candidates.to_vec()
    }

    /// A network that cannot route the nodes at `unroutable` (a v4-only
    /// entry seen from an IPv6-only network), keeping the caller's order.
    fn routes_all_but(unroutable: &'static [usize]) -> impl FnOnce(&[usize]) -> Vec<usize> {
        move |candidates| {
            candidates
                .iter()
                .copied()
                .filter(|i| !unroutable.contains(i))
                .collect()
        }
    }

    #[test]
    fn single_hop_collapses_entry_onto_the_exit_node() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        // Request the second node as exit, single-hop.
        let got = select_circuit_indices(&v, &[2; 32], false, None, None, all_reachable)
            .expect("single-hop must select the exit as its own entry");
        // entry_idx == exit_idx: the 1-hop circuit rides one node.
        assert_eq!(got, (1, 1));
    }

    #[test]
    fn single_hop_succeeds_with_a_single_node_directory() {
        // A lone node cannot form a 2-hop circuit, but single-hop rides it.
        let nodes = [node(7, "NL")];
        let v = views(&nodes);
        let got = select_circuit_indices(&v, &[7; 32], false, None, None, all_reachable)
            .expect("single-hop needs no distinct entry");
        assert_eq!(got, (0, 0));
    }

    #[test]
    fn two_hop_picks_the_first_distinct_entry_by_default() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit is node index 1; the first distinct node is index 0.
        let got = select_circuit_indices(&v, &[2; 32], true, None, None, all_reachable)
            .expect("a distinct entry exists");
        assert_eq!(got, (0, 1));
    }

    #[test]
    fn two_hop_honours_a_distinct_entry_pubkey_hint() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit node index 0; hint the entry pubkey of node index 2 ([4; 32]).
        let got = select_circuit_indices(&v, &[1; 32], true, Some(&[4; 32]), None, all_reachable)
            .expect("the hinted entry is distinct from the exit");
        assert_eq!(got, (2, 0));
    }

    #[test]
    fn two_hop_falls_back_to_country_when_the_hint_is_absent() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit node index 0; prefer an entry in SG (node index 2).
        let got = select_circuit_indices(&v, &[1; 32], true, None, Some("sg"), all_reachable)
            .expect("an SG entry exists and is distinct");
        assert_eq!(got, (2, 0));
    }

    #[test]
    fn two_hop_ignores_a_hint_that_points_at_the_exit_node() {
        // Two nodes; the entry hint names the EXIT node's own relay pubkey.
        // It must be ignored (a relay must differ from the exit) and selection
        // falls through to the first distinct node, never collapsing.
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        // Exit is node index 0 ([1; 32]); its relay pubkey is [2; 32].
        let got = select_circuit_indices(&v, &[1; 32], true, Some(&[2; 32]), None, all_reachable)
            .expect("a distinct entry exists despite the self-pointing hint");
        assert_eq!(got, (1, 0));
    }

    #[test]
    fn two_hop_fails_closed_when_no_distinct_entry_exists() {
        // A lone node: a 2-hop request cannot be satisfied and must NOT
        // silently collapse to a 1-hop circuit.
        let nodes = [node(9, "NL")];
        let v = views(&nodes);
        let err = select_circuit_indices(&v, &[9; 32], true, None, None, all_reachable)
            .expect_err("a lone node cannot serve a 2-hop circuit");
        assert_eq!(err, CircuitSelectError::NoDistinctEntry);
    }

    #[test]
    fn an_unknown_exit_fails_closed_in_both_modes() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        assert_eq!(
            select_circuit_indices(&v, &[42; 32], true, None, None, all_reachable),
            Err(CircuitSelectError::ExitNotInDirectory)
        );
        assert_eq!(
            select_circuit_indices(&v, &[42; 32], false, None, None, all_reachable),
            Err(CircuitSelectError::ExitNotInDirectory)
        );
    }

    #[test]
    fn two_hop_skips_an_entry_this_network_cannot_route() {
        // Forum topic 210: the live directory lists a v4-only node first, and
        // an IPv6-only phone took it as the entry for every exit.
        let nodes = [node(1, "RO"), node(2, "DE"), node(3, "SG")];
        let v = views(&nodes);
        let got = select_circuit_indices(&v, &[3; 32], true, None, None, routes_all_but(&[0]))
            .expect("DE is routable and distinct from the exit");
        assert_eq!(got, (1, 2));
    }

    #[test]
    fn a_one_hop_exit_this_network_cannot_route_is_the_typed_error() {
        let nodes = [node(1, "RO"), node(2, "DE")];
        let v = views(&nodes);
        assert_eq!(
            select_circuit_indices(&v, &[1; 32], false, None, None, routes_all_but(&[0])),
            Err(CircuitSelectError::NoReachableEntry)
        );
    }

    #[test]
    fn a_pinned_entry_country_the_network_cannot_route_is_never_widened() {
        // FR is pinned and v4-only; DE would be routable, but moving the user
        // to a country they did not choose is not ours to decide.
        let nodes = [node(1, "FR"), node(2, "DE"), node(3, "SG")];
        let v = views(&nodes);
        assert_eq!(
            select_circuit_indices(&v, &[3; 32], true, None, Some("fr"), routes_all_but(&[0])),
            Err(CircuitSelectError::NoReachableEntry)
        );
    }

    #[test]
    fn a_country_with_no_node_to_front_the_exit_leaves_every_entry_open() {
        // The pinned country holds only the exit itself: the pin cannot be
        // honoured by any entry, so it constrains nothing (the shipping rule).
        let nodes = [node(1, "FR"), node(2, "DE"), node(3, "SG")];
        let v = views(&nodes);
        assert_eq!(entry_candidates(&v, 0, None, Some("FR")), vec![1, 2]);
    }

    #[test]
    fn candidates_put_the_hint_first_then_the_pinned_country() {
        let nodes = [node(1, "FR"), node(2, "DE"), node(3, "FR"), node(4, "SG")];
        let v = views(&nodes);
        // Exit is node 3 (SG); the hint names node 1's relay ([3; 32]).
        assert_eq!(
            entry_candidates(&v, 3, Some(&[3; 32]), Some("fr")),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn the_engine_probe_keeps_only_what_this_host_routes_in_caller_order() {
        // A pinned IPv4 bind reaches no IPv6 address, which stands in for an
        // IPv4-only network on any test host without touching its routes.
        let relay = |endpoint: &str| warrenguard_multihop::RelayDescriptorSigned {
            relay_id: [0x11; 16],
            relay_ed25519_pubkey: [0x22; 32],
            endpoint: endpoint.parse().expect("static addr parses"),
            endpoint_v6: None,
            cover_domain: None,
            tcp_fallback: false,
            signature: [0x33; 64],
        };
        let relays = [
            relay("127.0.0.1:9"),
            relay("[2001:db8::1]:443"),
            relay("127.0.0.2:9"),
        ];
        let kept = super::probe_reachable(
            |i| &relays[i],
            &[2, 1, 0],
            "127.0.0.1:0".parse().expect("static addr parses"),
        );
        assert_eq!(kept, vec![2, 0]);
    }
}
