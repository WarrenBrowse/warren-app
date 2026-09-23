//! iOS-side multi-hop directory verification + circuit selection.
//!
//! The Warren fleet is multi-hop only: production exits run the unified
//! `:443` dispatcher (server side `warrenguard-multihop-server`) and
//! accept only the multi-hop setup. So every iOS connection rides
//! the multi-hop wire protocol, either as a 2-hop circuit (entry != exit,
//! country diverse) or a 1-hop circuit collapsed onto a single trusted
//! node (classic-VPN privacy, same wire).
//!
//! Transport split: Swift fetches `GET {api}/v2/multihop/directory` (frozen
//! `/v1` on an older backend) over
//! URLSession (native TLS, no reqwest on the iOS Rust target) and hands
//! the raw JSON to the FFI. This module performs the SECURITY half in
//! Rust, mirroring `mullvad-daemon::warren_multi_hop_directory`: it
//! verifies the signed directory against the pinned offline root
//! (`verify_multihop_directory_any`) and selects a circuit. The trust
//! anchor is the baked root pubkey; warren-api cannot forge a node
//! without the offline operational key the root vouches for.
//!
//! Anti-rollback: `verify_directory` takes a `min_generation` and rejects
//! a directory whose `generation` is below the highest already trusted (a
//! compromised server replaying an older, validly-signed set). The caller
//! persists the high-water mark across connects (the FFI keeps it in an App
//! Group file, since iOS has no long-lived daemon to hold it in memory).
//!
//! Defense-in-depth: the directory envelope is signature-checked against the
//! baked API **server** pin
//! (`crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX`, the same
//! value the daemon pins), on top of the root-anchored operational
//! certificate.
//!
//! Periodic refresh: Swift re-fetches every 30 min and calls
//! [`verify_generation`]; when the trusted generation advances (a fleet
//! change) it re-applies the fresh directory and reconnects onto a freshly
//! selected circuit, mirroring the daemon's timer-driven updater.

use ed25519_dalek::VerifyingKey;
use warren_discovery_core::{
    DEFAULT_RTT_TTL_SECS, DirectoryError, ExitCandidate, PathAwareParams, RttCache,
    VerifiedMultiHopDirectory, node_rtt_from, pick_exit, select_circuit_path_aware, valid_circuits,
    verify_multihop_directory_any,
};
use warrenguard_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};

/// Failure to verify + select a circuit from a fetched directory.
#[derive(Debug, thiserror::Error)]
pub enum SelectError {
    /// The signed directory failed the trust-chain verification.
    #[error("directory verification failed")]
    Verify(#[from] DirectoryError),
    /// The signed directory is past its `expires_at` (replay defense).
    #[error("directory is expired")]
    Expired,
    /// The signed directory's `generation` is below the high-water mark
    /// (rollback defense): a validly-signed but stale set being replayed.
    #[error("directory generation {got} is below the trusted high-water mark {min}")]
    Rollback { got: u64, min: u64 },
}

/// Build-time-baked **root** pubkey pin (64-char hex), identical to the
/// daemon's (`mullvad-daemon::warren_multi_hop_directory`). This is the
/// production multi-hop trust anchor: the offline root key whose public
/// half is compiled in so the directory's operational certificate is
/// verified without any runtime configuration.
const WARREN_MULTIHOP_ROOT_PUBKEY_BAKED: &str =
    "33cd9279ad06d1ee884235e763b876fa70598094944bdcfb82375bd9aaa67b08";

/// A selected circuit: the descriptors `MultiHopClient::connect` /
/// `SupervisorConfig` need. For a 1-hop circuit `relay` and `exit` are
/// the two roles of the SAME directory node.
pub struct SelectedCircuit {
    pub relay: RelayDescriptorSigned,
    pub exit: ExitDescriptorSigned,
    pub operational_pubkey: VerifyingKey,
    /// The verified directory's monotonic content version. The caller
    /// raises its persisted high-water mark to this after a successful
    /// selection (anti-rollback).
    pub generation: u64,
}

/// Selects a circuit from a verified directory honoring the optional
/// country hints. `two_hop` picks a 2-hop circuit (entry != exit, different
/// countries); otherwise a 1-hop circuit (one node as both relay and exit).
/// Returns `None` when no node/pair satisfies the rules.
///
/// `entry_rtt` is the client-measured entry-RTT store fed by the
/// supervisor's `on_path_rtt` observer; an empty store keeps the legacy
/// weight ordering bit-identical.
///
/// `avoid` lists the nodes (by exit id) left out in both roles: those that
/// refused a dial or announced a drain.
pub fn select_circuit(
    dir: &VerifiedMultiHopDirectory,
    two_hop: bool,
    entry_country: &str,
    exit_country: &str,
    entry_rtt: &RttCache,
    now_unix: u64,
    avoid: &[[u8; 16]],
) -> Option<SelectedCircuit> {
    if two_hop {
        select_two_hop(
            dir,
            entry_country,
            exit_country,
            entry_rtt,
            now_unix,
            avoid,
            None,
        )
    } else {
        select_one_hop(dir, exit_country, avoid)
    }
}

/// Verify the signed directory and return its trusted `generation` without
/// selecting a circuit. Used by the periodic refresh to decide whether the
/// fleet changed (a higher generation) and a re-selection is warranted.
///
/// Like [`verify_directory`], this does NOT raise the persisted high-water
/// mark: that happens only on a successful connect, so a verified-but-unused
/// directory (e.g. an inflated-generation forgery under a server-key
/// compromise) cannot poison the mark via a periodic check.
///
/// # Errors
/// Same as [`verify_directory`] (verification, expiry, rollback).
pub fn verify_generation(
    json: &str,
    now_unix: u64,
    min_generation: u64,
) -> Result<u64, SelectError> {
    Ok(verify_directory(json, now_unix, min_generation)?.generation)
}

/// Verifies the signed directory JSON: envelope signature + server pin +
/// root-anchored operational certificate, then expiry and the anti-rollback
/// gate. The `generation` field is trusted only here, strictly AFTER the
/// signature verification, so a forged generation cannot clear the gate.
///
/// `min_generation` is the caller's persisted anti-rollback high-water
/// mark: a verified directory whose `generation` is strictly below it is
/// rejected. Pass `0` to disable the gate (first connect / no stored mark).
///
/// # Errors
/// [`DirectoryError`] when the JSON does not parse or the trust chain
/// (envelope signature, operational certificate under the baked root,
/// per-node attestation) does not verify; [`SelectError::Expired`] when the
/// signed directory is past its `expires_at`; [`SelectError::Rollback`]
/// when its `generation` is below `min_generation`.
pub fn verify_directory(
    json: &str,
    now_unix: u64,
    min_generation: u64,
) -> Result<VerifiedMultiHopDirectory, SelectError> {
    // Root pin is the anchor (operational cert is verified against the
    // offline root). The server pin is defense-in-depth: the envelope
    // signature is additionally checked against the baked API server key,
    // so a compromised server cannot present a validly-rooted directory
    // signed by a different envelope key.
    let dir = verify_multihop_directory_any(
        json,
        &[crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX],
        &[WARREN_MULTIHOP_ROOT_PUBKEY_BAKED],
    )?;
    if dir.is_expired(now_unix) {
        return Err(SelectError::Expired);
    }
    if dir.generation < min_generation {
        return Err(SelectError::Rollback {
            got: dir.generation,
            min: min_generation,
        });
    }
    Ok(dir)
}

fn country_matches(filter: &str, country: &str) -> bool {
    filter.is_empty() || filter.eq_ignore_ascii_case(country)
}

fn circuit_from(
    dir: &VerifiedMultiHopDirectory,
    entry_idx: usize,
    exit_idx: usize,
) -> SelectedCircuit {
    SelectedCircuit {
        relay: dir.nodes[entry_idx].relay.clone(),
        exit: dir.nodes[exit_idx].exit.clone(),
        operational_pubkey: dir.operational_pubkey,
        generation: dir.generation,
    }
}

/// `prefer_exit` keeps that exit when some pair can still reach it, so moving
/// off a refusing entry does not change the user's egress.
fn select_two_hop(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    entry_rtt: &RttCache,
    now_unix: u64,
    avoid: &[[u8; 16]],
    prefer_exit: Option<[u8; 16]>,
) -> Option<SelectedCircuit> {
    // The diversity rule is the shared neutral one, and the pick is the shared
    // path-aware selector fed by the client-measured entry RTTs (None on empty
    // pairs). No advisory on iOS yet: Swift owns HTTP and does not fetch
    // `/v1/path-quality`, and an absent advisory is neutral. With an empty
    // store this is bit-identical to the legacy `pick_circuit_by_weight`.
    let mut pairs = valid_circuits(dir, entry_country, exit_country, avoid);
    if let Some(exit) = prefer_exit {
        let keeping: Vec<(usize, usize)> = pairs
            .iter()
            .copied()
            .filter(|&(_, x)| *dir.nodes[x].exit.exit_id.as_bytes() == exit)
            .collect();
        if !keeping.is_empty() {
            pairs = keeping;
        }
    }
    let (entry_idx, exit_idx) = select_circuit_path_aware(
        dir,
        &pairs,
        None,
        node_rtt_from(entry_rtt, now_unix, DEFAULT_RTT_TTL_SECS),
        now_unix,
        None,
        &PathAwareParams::default(),
    )?;
    Some(circuit_from(dir, entry_idx, exit_idx))
}

/// Selects a 1-hop circuit: one node serves as both entry relay and exit
/// (reached on the node's unified `:443` dispatcher). Honors the
/// `exit_country` hint; the pick among the matching nodes is the shared
/// `warren_discovery_core::pick_exit` (highest weight, ties broken by the
/// smallest `exit_id`), the same call the daemon's `select_one_hop_circuit`
/// makes, so a directory resolves to one node on every platform.
fn select_one_hop(
    dir: &VerifiedMultiHopDirectory,
    exit_country: &str,
    avoid: &[[u8; 16]],
) -> Option<SelectedCircuit> {
    let candidates: Vec<usize> = dir
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| country_matches(exit_country, &n.country))
        .filter(|(_, n)| !avoid.contains(n.exit.exit_id.as_bytes()))
        .map(|(i, _)| i)
        .collect();
    let ranked: Vec<ExitCandidate> = candidates
        .iter()
        .map(|&i| ExitCandidate::from(&dir.nodes[i]))
        .collect();
    let idx = candidates[pick_exit(&ranked)?];
    Some(circuit_from(dir, idx, idx))
}

/// The circuit an iOS session dials, and the nodes it moved off.
///
/// A refused dial is taken as the refusal of the node the connection
/// terminates at (the entry relay, which on a one-hop circuit is the exit), so
/// a hostile relay can make the session avoid that relay and nothing else. A
/// drain is announced in band by the exit itself.
pub struct CircuitRetarget {
    dir: VerifiedMultiHopDirectory,
    two_hop: bool,
    entry_country: String,
    exit_country: String,
    relay_id: [u8; 16],
    exit_id: [u8; 16],
    /// Nodes left out of the selection, by exit id, with the unix second
    /// each was recorded; offered again after `avoid_ttl_secs`, once a drain
    /// can be over.
    avoided: Vec<([u8; 16], u64)>,
    avoid_ttl_secs: u64,
}

impl CircuitRetarget {
    /// Start from the circuit the session dialed first. `avoid_ttl_secs` is
    /// the engine's drained-exit avoid window.
    #[must_use]
    pub fn new(
        dir: VerifiedMultiHopDirectory,
        two_hop: bool,
        entry_country: &str,
        exit_country: &str,
        current: &SelectedCircuit,
        avoid_ttl_secs: u64,
    ) -> Self {
        Self {
            dir,
            two_hop,
            entry_country: entry_country.to_owned(),
            exit_country: exit_country.to_owned(),
            relay_id: current.relay.relay_id,
            exit_id: *current.exit.exit_id.as_bytes(),
            avoided: Vec::new(),
            avoid_ttl_secs,
        }
    }

    /// The circuit to move to after the node carrying `refused_relay_id`
    /// refused a dial, or `None` to stay on the supervisor's backoff.
    /// `admits` is the key-pin check a candidate exit must pass.
    pub fn on_refusal(
        &mut self,
        refused_relay_id: &[u8; 16],
        entry_rtt: &RttCache,
        now_unix: u64,
        admits: impl Fn(&SelectedCircuit) -> bool,
    ) -> Option<SelectedCircuit> {
        // A dial already in flight to a node this session left can still come
        // back refused; it says nothing about the circuit now dialed.
        if *refused_relay_id != self.relay_id {
            return None;
        }
        let node = self
            .dir
            .nodes
            .iter()
            .find(|n| n.relay.relay_id == *refused_relay_id)
            .map(|n| *n.exit.exit_id.as_bytes())?;
        self.avoid(node, now_unix);
        let keep_exit = (node != self.exit_id).then_some(self.exit_id);
        self.move_to(entry_rtt, now_unix, keep_exit, admits)
    }

    /// The circuit to move to after the exit announced a drain, or `None`
    /// when no other exit can serve the session.
    pub fn on_drain(
        &mut self,
        entry_rtt: &RttCache,
        now_unix: u64,
        admits: impl Fn(&SelectedCircuit) -> bool,
    ) -> Option<SelectedCircuit> {
        self.avoid(self.exit_id, now_unix);
        self.move_to(entry_rtt, now_unix, None, admits)
    }

    fn avoid(&mut self, node_exit_id: [u8; 16], now_unix: u64) {
        let ttl = self.avoid_ttl_secs;
        self.avoided
            .retain(|&(id, at)| id != node_exit_id && now_unix.saturating_sub(at) < ttl);
        self.avoided.push((node_exit_id, now_unix));
    }

    /// Select with the avoided nodes left out, and adopt the result only when
    /// it is a different circuit the key pins admit.
    fn move_to(
        &mut self,
        entry_rtt: &RttCache,
        now_unix: u64,
        keep_exit: Option<[u8; 16]>,
        admits: impl Fn(&SelectedCircuit) -> bool,
    ) -> Option<SelectedCircuit> {
        let avoid: Vec<[u8; 16]> = self.avoided.iter().map(|&(id, _)| id).collect();
        let candidate = if self.two_hop {
            select_two_hop(
                &self.dir,
                &self.entry_country,
                &self.exit_country,
                entry_rtt,
                now_unix,
                &avoid,
                keep_exit,
            )
        } else {
            select_one_hop(&self.dir, &self.exit_country, &avoid)
        }?;
        let ids = (candidate.relay.relay_id, *candidate.exit.exit_id.as_bytes());
        if ids == (self.relay_id, self.exit_id) || !admits(&candidate) {
            return None;
        }
        (self.relay_id, self.exit_id) = ids;
        Some(candidate)
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_discovery_core::{NodeEntry, pick_circuit_by_weight};
    use warrenguard_multihop::{
        ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
        relay_descriptor_signing_payload, sign_node_attestation,
    };

    use super::*;

    const NOW: u64 = 1_000_000;

    fn op_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    fn node(op: &SigningKey, tag: u8, country: &str, asn: u32, weight: u64) -> NodeEntry {
        let endpoint: SocketAddr = format!("198.51.100.{tag}:443").parse().unwrap();
        let relay_id = [tag; 16];
        let relay_ed = [tag.wrapping_add(1); 32];
        let relay_sig = op
            .sign(&relay_descriptor_signing_payload(&relay_id, &relay_ed))
            .to_bytes();
        let exit_id = ExitId::from_bytes([tag; 16]);
        let exit_x = [tag.wrapping_add(2); 32];
        let exit_sig = op
            .sign(&exit_descriptor_signing_payload(exit_id, &exit_x))
            .to_bytes();
        NodeEntry {
            relay: RelayDescriptorSigned {
                relay_id,
                relay_ed25519_pubkey: relay_ed,
                endpoint,
                endpoint_v6: None,
                signature: relay_sig,
                cover_domain: None,
                tcp_fallback: false,
            },
            exit: ExitDescriptorSigned {
                exit_id,
                exit_ed25519_pubkey: relay_ed,
                exit_x25519_multihop_pubkey: exit_x,
                exit_mlkem768_pubkey: None,
                endpoint: Some(endpoint),
                signature: exit_sig,
                dns_disabled: false,
                cover_domain: None,
            },
            country: country.to_owned(),
            city: "City".to_owned(),
            asn,
            weight,
            attestation_hex: hex::encode(sign_node_attestation(
                op, &relay_id, &relay_ed, asn, country,
            )),
            edge_cert_sha256: None,
        }
    }

    fn dir(nodes: Vec<NodeEntry>) -> VerifiedMultiHopDirectory {
        VerifiedMultiHopDirectory {
            operational_pubkey: op_key().verifying_key(),
            nodes,
            generation: 1,
            signed_at: 0,
            expires_at: u64::MAX,
            dropped: 0,
        }
    }

    /// The shared crate's `exit_pick.json` vector through THIS platform's
    /// 1-hop pick, the way the daemon replays it through its own: the shared
    /// rule and what iOS actually dials cannot drift apart unseen.
    #[test]
    fn exit_vectors_replay_through_the_one_hop_selection() {
        let op = op_key();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../warren-contract/warren-discovery/tests/fixtures/exit_pick.json"
        ))
        .expect("exit_pick.json must parse");
        let cases = fixture["exit"].as_array().expect("exit section");
        assert!(cases.len() >= 8, "the exit section must keep its cases");
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            // The tag numbers the node (relay id included) while the exit id
            // comes from the vector, so a full tie on duplicate exit ids is
            // still told apart by the relay id of the pick.
            let nodes: Vec<NodeEntry> = case["candidates"]
                .as_array()
                .expect("candidates")
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let tag = u8::try_from(i + 1).expect("small fixtures");
                    let mut n = node(&op, tag, "de", 0, c["weight"].as_u64().expect("weight"));
                    let id: [u8; 16] = hex::decode(c["exit_id"].as_str().expect("id"))
                        .expect("fixture ids are hex")
                        .try_into()
                        .expect("fixture ids are 16 bytes");
                    n.exit.exit_id = ExitId::from_bytes(id);
                    n
                })
                .collect();
            let d = dir(nodes);
            let picked = select_one_hop(&d, "", &[]).map(|c| c.relay.relay_id);
            let expected = case["expected"]
                .as_u64()
                .map(|i| d.nodes[usize::try_from(i).expect("index")].relay.relay_id);
            assert_eq!(
                picked, expected,
                "exit vector `{name}` diverged from the iOS 1-hop pick"
            );
        }
    }

    #[test]
    fn client_measured_entry_rtt_biases_the_two_hop_pick() {
        let op = op_key();
        // Equal weights: the legacy order tie-breaks on ids and picks the
        // DE entry. The store keys by the entry's Ed25519 pubkey (node tag
        // N mints [N+1; 32]).
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 100),
            node(&op, 3, "nl", 3, 100),
        ]);
        let baseline =
            select_two_hop(&d, "", "nl", &RttCache::new(), NOW, &[], None).expect("circuit");
        assert_eq!(
            baseline.relay.relay_id, [1; 16],
            "precondition: id tie-break"
        );
        let mut store = RttCache::new();
        store.record([2; 32], 200, NOW);
        store.record([3; 32], 15, NOW);
        let biased = select_two_hop(&d, "", "nl", &store, NOW, &[], None).expect("circuit");
        assert_eq!(
            biased.relay.relay_id, [2; 16],
            "the measured near entry must outrank the id tie-break"
        );
    }

    #[test]
    fn an_empty_rtt_store_keeps_the_two_hop_pick_bit_identical() {
        let op = op_key();
        for weights in [[100, 100, 100], [1, 500, 20], [7, 7, 900]] {
            let d = dir(vec![
                node(&op, 1, "de", 1, weights[0]),
                node(&op, 2, "fr", 2, weights[1]),
                node(&op, 3, "nl", 3, weights[2]),
            ]);
            let pairs = valid_circuits(&d, "", "nl", &[]);
            let legacy = pick_circuit_by_weight(&d, &pairs).map(|(e, x)| {
                (
                    d.nodes[e].relay.relay_id,
                    *d.nodes[x].exit.exit_id.as_bytes(),
                )
            });
            let got = select_two_hop(&d, "", "nl", &RttCache::new(), NOW, &[], None)
                .map(|c| (c.relay.relay_id, *c.exit.exit_id.as_bytes()));
            assert_eq!(got, legacy, "weights {weights:?}");
        }
    }

    #[test]
    fn selected_circuit_carries_the_directory_trust_context() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 1, 100), node(&op, 3, "nl", 3, 100)]);
        let c = select_two_hop(&d, "", "nl", &RttCache::new(), NOW, &[], None).expect("circuit");
        assert_eq!(c.generation, 1);
        assert_eq!(c.operational_pubkey, op.verifying_key());
    }

    /// The engine's drained-exit avoid window.
    const TTL: u64 = 300;

    /// A 2-hop circuit and the retarget that holds it.
    fn two_hop_session(
        d: &VerifiedMultiHopDirectory,
        entry_country: &str,
        exit_country: &str,
    ) -> (SelectedCircuit, CircuitRetarget) {
        let first = select_circuit(
            d,
            true,
            entry_country,
            exit_country,
            &RttCache::new(),
            NOW,
            &[],
        )
        .expect("a first circuit");
        let retarget =
            CircuitRetarget::new(d.clone(), true, entry_country, exit_country, &first, TTL);
        (first, retarget)
    }

    fn ids(c: &SelectedCircuit) -> ([u8; 16], [u8; 16]) {
        (c.relay.relay_id, *c.exit.exit_id.as_bytes())
    }

    #[test]
    fn a_refusing_entry_is_replaced_and_the_exit_kept() {
        let op = op_key();
        // The heavy FR entry shares NL3's AS, so it can only front NL5: once
        // DE refuses, the best pair overall is FR -> NL5, and keeping the
        // user's egress takes the light SE entry in front of NL3.
        let d = dir(vec![
            node(&op, 1, "de", 1, 1_000),
            node(&op, 2, "fr", 3, 500),
            node(&op, 3, "nl", 3, 100),
            node(&op, 4, "se", 4, 10),
            node(&op, 5, "nl", 5, 50),
        ]);
        let (first, mut retarget) = two_hop_session(&d, "", "nl");
        assert_eq!(
            ids(&first),
            ([1; 16], [3; 16]),
            "precondition: DE fronts NL3"
        );

        let moved = retarget
            .on_refusal(&[1; 16], &RttCache::new(), NOW, |_| true)
            .expect("another entry can front the same exit");

        assert_eq!(ids(&moved), ([4; 16], [3; 16]), "the egress stays");
    }

    #[test]
    fn a_refusing_one_hop_node_moves_the_session_to_another_node() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 1, 100), node(&op, 2, "fr", 2, 50)]);
        let first = select_circuit(&d, false, "", "", &RttCache::new(), NOW, &[]).expect("circuit");
        assert_eq!(ids(&first), ([1; 16], [1; 16]));
        let mut retarget = CircuitRetarget::new(d.clone(), false, "", "", &first, TTL);

        let moved = retarget
            .on_refusal(&[1; 16], &RttCache::new(), NOW, |_| true)
            .expect("another node can serve");

        assert_eq!(ids(&moved), ([2; 16], [2; 16]));
    }

    #[test]
    fn a_refusal_by_a_node_the_session_left_is_ignored() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 50),
            node(&op, 3, "se", 3, 40),
        ]);
        let first = select_circuit(&d, false, "", "", &RttCache::new(), NOW, &[]).expect("circuit");
        let mut retarget = CircuitRetarget::new(d.clone(), false, "", "", &first, TTL);
        assert!(
            retarget
                .on_refusal(&[1; 16], &RttCache::new(), NOW, |_| true)
                .is_some()
        );

        // A dial that was in flight to the node the session left comes back
        // refused just before that node's avoid window ends.
        assert!(
            retarget
                .on_refusal(&[1; 16], &RttCache::new(), NOW + TTL - 1, |_| true)
                .is_none()
        );

        // It said nothing about the circuit dialed now, so it must not have
        // renewed that node's avoid window: once the window of the refusal
        // that counted is over, the node can take the session back.
        let back = retarget
            .on_refusal(&[2; 16], &RttCache::new(), NOW + TTL, |_| true)
            .expect("the first node is available again");
        assert_eq!(ids(&back), ([1; 16], [1; 16]));
    }

    #[test]
    fn a_node_that_refused_is_offered_again_once_its_drain_can_be_over() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 1, 100), node(&op, 2, "fr", 2, 50)]);
        let first = select_circuit(&d, false, "", "", &RttCache::new(), NOW, &[]).expect("circuit");
        let mut retarget = CircuitRetarget::new(d.clone(), false, "", "", &first, TTL);
        assert!(
            retarget
                .on_refusal(&[1; 16], &RttCache::new(), NOW, |_| true)
                .is_some()
        );

        assert!(
            retarget
                .on_refusal(&[2; 16], &RttCache::new(), NOW + 10, |_| true)
                .is_none(),
            "the first node is still avoided"
        );
        let back = retarget
            .on_refusal(&[2; 16], &RttCache::new(), NOW + TTL, |_| true)
            .expect("the first node's avoid window has passed");
        assert_eq!(ids(&back), ([1; 16], [1; 16]));
    }

    #[test]
    fn a_drain_moves_the_session_to_another_exit_of_the_pinned_country() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 3, "nl", 3, 100),
            node(&op, 5, "nl", 5, 90),
        ]);
        let (first, mut retarget) = two_hop_session(&d, "", "nl");
        assert_eq!(*first.exit.exit_id.as_bytes(), [3; 16]);

        let moved = retarget
            .on_drain(&RttCache::new(), NOW, |_| true)
            .expect("the other NL exit can serve");

        assert_eq!(*moved.exit.exit_id.as_bytes(), [5; 16]);
    }

    #[test]
    fn a_drain_with_no_other_exit_in_the_pinned_country_finds_nothing() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 1, 100), node(&op, 3, "nl", 3, 100)]);
        let (_, mut retarget) = two_hop_session(&d, "", "nl");

        assert!(retarget.on_drain(&RttCache::new(), NOW, |_| true).is_none());
    }

    #[test]
    fn a_candidate_the_key_pins_refuse_is_never_dialed() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 3, "nl", 3, 100),
            node(&op, 5, "nl", 5, 90),
        ]);
        let (_, mut retarget) = two_hop_session(&d, "", "nl");

        assert!(
            retarget
                .on_drain(&RttCache::new(), NOW, |_| false)
                .is_none()
        );
        // Refused by the pins, the candidate was not adopted: the session is
        // still on its first exit, which another drain can move off.
        let moved = retarget
            .on_drain(&RttCache::new(), NOW + 1, |_| true)
            .expect("the NL alternative, now admitted");
        assert_eq!(*moved.exit.exit_id.as_bytes(), [5; 16]);
    }

    #[test]
    fn verify_entry_points_reject_unparseable_json() {
        assert!(matches!(
            verify_directory("not json", NOW, 0),
            Err(SelectError::Verify(_))
        ));
        assert!(matches!(
            verify_generation("not json", NOW, 0),
            Err(SelectError::Verify(_))
        ));
    }
}
