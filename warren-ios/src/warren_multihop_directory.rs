//! iOS-side multi-hop directory verification + circuit selection.
//!
//! The Warren fleet is multi-hop only: production exits run the unified
//! `:443` dispatcher (server side `warrenguard-multihop-server`) and
//! accept only the multi-hop setup. So every iOS connection rides
//! the multi-hop wire protocol, either as a 2-hop circuit (entry != exit,
//! country diverse) or a 1-hop circuit collapsed onto a single trusted
//! node (classic-VPN privacy, same wire).
//!
//! Transport split: Swift fetches `GET {api}/v1/multihop/directory` over
//! URLSession (native TLS, no reqwest on the iOS Rust target) and hands
//! the raw JSON to the FFI. This module performs the SECURITY half in
//! Rust, mirroring `mullvad-daemon::warren_multi_hop_directory`: it
//! verifies the signed directory against the pinned offline root
//! (`verify_multihop_directory_any`) and selects a circuit. The trust
//! anchor is the baked root pubkey; warren-api cannot forge a node
//! without the offline operational key the root vouches for.
//!
//! Anti-rollback: `verify_and_select` takes a `min_generation` and rejects
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
    DEFAULT_RTT_TTL_SECS, DirectoryError, PathAwareParams, RttCache, VerifiedMultiHopDirectory,
    node_rtt_from, select_circuit_path_aware, valid_circuits, verify_multihop_directory_any,
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

/// Verifies the signed directory JSON against the baked root pin and
/// selects a circuit honoring the optional country hints. `two_hop`
/// picks a 2-hop circuit (entry != exit, different countries); otherwise
/// a 1-hop circuit (one node as both relay and exit). Returns `None` when
/// no node/pair satisfies the rules.
///
/// `min_generation` is the caller's persisted anti-rollback high-water
/// mark: a verified directory whose `generation` is strictly below it is
/// rejected. Pass `0` to disable the gate (first connect / no stored mark).
///
/// `entry_rtt` is the client-measured entry-RTT store fed by the
/// supervisor's `on_path_rtt` observer; an empty store keeps the legacy
/// weight ordering bit-identical.
///
/// # Errors
/// [`DirectoryError`] when the JSON does not parse or the trust chain
/// (envelope signature, operational certificate under the baked root,
/// per-node attestation) does not verify; [`SelectError::Expired`] when the
/// signed directory is past its `expires_at`; [`SelectError::Rollback`]
/// when its `generation` is below `min_generation`.
pub fn verify_and_select(
    json: &str,
    two_hop: bool,
    entry_country: &str,
    exit_country: &str,
    entry_rtt: &RttCache,
    now_unix: u64,
    min_generation: u64,
) -> Result<Option<SelectedCircuit>, SelectError> {
    let dir = verify_fresh(json, now_unix, min_generation)?;
    Ok(if two_hop {
        select_two_hop(&dir, entry_country, exit_country, entry_rtt, now_unix)
    } else {
        select_one_hop(&dir, exit_country)
    })
}

/// Verify the signed directory and return its trusted `generation` without
/// selecting a circuit. Used by the periodic refresh to decide whether the
/// fleet changed (a higher generation) and a re-selection is warranted.
///
/// Like [`verify_and_select`], this does NOT raise the persisted high-water
/// mark: that happens only on a successful connect, so a verified-but-unused
/// directory (e.g. an inflated-generation forgery under a server-key
/// compromise) cannot poison the mark via a periodic check.
///
/// # Errors
/// Same as [`verify_and_select`] (verification, expiry, rollback).
pub fn verify_generation(
    json: &str,
    now_unix: u64,
    min_generation: u64,
) -> Result<u64, SelectError> {
    Ok(verify_fresh(json, now_unix, min_generation)?.generation)
}

/// Shared verification prefix: envelope signature + server pin + root-anchored
/// operational certificate, then expiry and the anti-rollback gate. The
/// `generation` field is trusted only here, strictly AFTER the signature
/// verification, so a forged generation cannot clear the gate.
fn verify_fresh(
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

fn select_two_hop(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    entry_rtt: &RttCache,
    now_unix: u64,
) -> Option<SelectedCircuit> {
    // iOS has no drain avoid-set (no long-lived daemon holding one), so pass an
    // empty exclusion; the diversity rule is the shared neutral one, and the
    // pick is the shared path-aware selector fed by the client-measured entry
    // RTTs (None on empty pairs). No advisory on iOS yet: Swift owns HTTP and
    // does not fetch `/v1/path-quality`, and an absent advisory is neutral.
    // With an empty store this is bit-identical to the legacy
    // `pick_circuit_by_weight`.
    let pairs = valid_circuits(dir, entry_country, exit_country, &[]);
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
/// `exit_country` hint; deterministic highest-weight pick, ties broken by
/// smallest `exit_id`. Mirrors
/// `mullvad-daemon::warren_multi_hop_directory::select_one_hop_circuit`.
fn select_one_hop(dir: &VerifiedMultiHopDirectory, exit_country: &str) -> Option<SelectedCircuit> {
    let mut candidates: Vec<usize> = dir
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| country_matches(exit_country, &n.country))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|&a, &b| {
        dir.nodes[b].weight.cmp(&dir.nodes[a].weight).then_with(|| {
            dir.nodes[a]
                .exit
                .exit_id
                .as_bytes()
                .cmp(dir.nodes[b].exit.exit_id.as_bytes())
        })
    });
    let idx = candidates[0];
    Some(circuit_from(dir, idx, idx))
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
        let baseline = select_two_hop(&d, "", "nl", &RttCache::new(), NOW).expect("circuit");
        assert_eq!(
            baseline.relay.relay_id, [1; 16],
            "precondition: id tie-break"
        );
        let mut store = RttCache::new();
        store.record([2; 32], 200, NOW);
        store.record([3; 32], 15, NOW);
        let biased = select_two_hop(&d, "", "nl", &store, NOW).expect("circuit");
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
            let got = select_two_hop(&d, "", "nl", &RttCache::new(), NOW)
                .map(|c| (c.relay.relay_id, *c.exit.exit_id.as_bytes()));
            assert_eq!(got, legacy, "weights {weights:?}");
        }
    }

    #[test]
    fn selected_circuit_carries_the_directory_trust_context() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 1, 100), node(&op, 3, "nl", 3, 100)]);
        let c = select_two_hop(&d, "", "nl", &RttCache::new(), NOW).expect("circuit");
        assert_eq!(c.generation, 1);
        assert_eq!(c.operational_pubkey, op.verifying_key());
    }

    #[test]
    fn verify_entry_points_reject_unparseable_json() {
        assert!(matches!(
            verify_and_select("not json", true, "", "nl", &RttCache::new(), NOW, 0),
            Err(SelectError::Verify(_))
        ));
        assert!(matches!(
            verify_generation("not json", NOW, 0),
            Err(SelectError::Verify(_))
        ));
    }
}
