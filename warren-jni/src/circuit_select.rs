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
}

/// Resolve `(entry_idx, exit_idx)` into `nodes` for the requested circuit.
///
/// `two_hop == false` is the single-hop 1-hop circuit: entry and exit collapse
/// onto the SAME node, so the returned indices are equal. `two_hop == true`
/// picks a DISTINCT entry by precedence (explicit pubkey hint, then entry
/// country, then the first distinct node) and fails closed when no distinct
/// node exists. The precedence and distinctness rules match the shipping
/// desktop / iOS 2-hop selection so all three clients build the same circuit
/// shape.
pub(crate) fn select_circuit_indices(
    nodes: &[NodeSel<'_>],
    want_exit: &[u8; 32],
    two_hop: bool,
    want_entry: Option<&[u8; 32]>,
    want_country: Option<&str>,
) -> Result<(usize, usize), CircuitSelectError> {
    let exit_idx = nodes
        .iter()
        .position(|n| n.exit_ed25519 == want_exit)
        .ok_or(CircuitSelectError::ExitNotInDirectory)?;

    if !two_hop {
        // 1-hop circuit: the exit node is also the entry relay, so the setup
        // frame's exit_id names the dialed node and it terminates locally.
        return Ok((exit_idx, exit_idx));
    }

    let exit_relay_id = nodes[exit_idx].relay_id;
    // Trim/empty-filter the country hint here so an empty string means "any"
    // rather than matching a blank country field.
    let want_country = want_country.map(str::trim).filter(|s| !s.is_empty());
    let entry_idx = want_entry
        .and_then(|w| {
            nodes
                .iter()
                .position(|n| n.relay_id != exit_relay_id && n.relay_ed25519 == w)
        })
        .or_else(|| {
            want_country.and_then(|c| {
                nodes
                    .iter()
                    .position(|n| n.relay_id != exit_relay_id && n.country.eq_ignore_ascii_case(c))
            })
        })
        .or_else(|| nodes.iter().position(|n| n.relay_id != exit_relay_id))
        .ok_or(CircuitSelectError::NoDistinctEntry)?;
    Ok((entry_idx, exit_idx))
}

#[cfg(test)]
mod tests {
    use super::{CircuitSelectError, NodeSel, select_circuit_indices};

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

    #[test]
    fn single_hop_collapses_entry_onto_the_exit_node() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        // Request the second node as exit, single-hop.
        let got = select_circuit_indices(&v, &[2; 32], false, None, None)
            .expect("single-hop must select the exit as its own entry");
        // entry_idx == exit_idx: the 1-hop circuit rides one node.
        assert_eq!(got, (1, 1));
    }

    #[test]
    fn single_hop_succeeds_with_a_single_node_directory() {
        // A lone node cannot form a 2-hop circuit, but single-hop rides it.
        let nodes = [node(7, "NL")];
        let v = views(&nodes);
        let got = select_circuit_indices(&v, &[7; 32], false, None, None)
            .expect("single-hop needs no distinct entry");
        assert_eq!(got, (0, 0));
    }

    #[test]
    fn two_hop_picks_the_first_distinct_entry_by_default() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit is node index 1; the first distinct node is index 0.
        let got = select_circuit_indices(&v, &[2; 32], true, None, None)
            .expect("a distinct entry exists");
        assert_eq!(got, (0, 1));
    }

    #[test]
    fn two_hop_honours_a_distinct_entry_pubkey_hint() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit node index 0; hint the entry pubkey of node index 2 ([4; 32]).
        let got = select_circuit_indices(&v, &[1; 32], true, Some(&[4; 32]), None)
            .expect("the hinted entry is distinct from the exit");
        assert_eq!(got, (2, 0));
    }

    #[test]
    fn two_hop_falls_back_to_country_when_the_hint_is_absent() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SG")];
        let v = views(&nodes);
        // Exit node index 0; prefer an entry in SG (node index 2).
        let got = select_circuit_indices(&v, &[1; 32], true, None, Some("sg"))
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
        let got = select_circuit_indices(&v, &[1; 32], true, Some(&[2; 32]), None)
            .expect("a distinct entry exists despite the self-pointing hint");
        assert_eq!(got, (1, 0));
    }

    #[test]
    fn two_hop_fails_closed_when_no_distinct_entry_exists() {
        // A lone node: a 2-hop request cannot be satisfied and must NOT
        // silently collapse to a 1-hop circuit.
        let nodes = [node(9, "NL")];
        let v = views(&nodes);
        let err = select_circuit_indices(&v, &[9; 32], true, None, None)
            .expect_err("a lone node cannot serve a 2-hop circuit");
        assert_eq!(err, CircuitSelectError::NoDistinctEntry);
    }

    #[test]
    fn an_unknown_exit_fails_closed_in_both_modes() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        assert_eq!(
            select_circuit_indices(&v, &[42; 32], true, None, None),
            Err(CircuitSelectError::ExitNotInDirectory)
        );
        assert_eq!(
            select_circuit_indices(&v, &[42; 32], false, None, None),
            Err(CircuitSelectError::ExitNotInDirectory)
        );
    }
}
