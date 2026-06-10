//! iOS-side multi-hop directory verification + circuit selection.
//!
//! The Warren fleet is multi-hop only: production exits run the unified
//! `:443` dispatcher and no longer serve the legacy single-hop
//! `Setup`/`SetupAck` handshake (see `warren-exit::multihop`). So every
//! iOS connection rides the multi-hop wire protocol, either as a 2-hop
//! circuit (entry != exit, country diverse) or a 1-hop circuit collapsed
//! onto a single trusted node (classic-VPN privacy, same wire).
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
//! Deliberately out of scope for this first cut (follow-ups, documented
//! so they are not mistaken for done): periodic 30-min directory refresh
//! + anti-rollback generation high-water mark (the daemon updater does
//! this; here Swift fetches once per connect), and the server-key pin
//! (empty here; the root chain is the anchor, the envelope is still
//! signature-checked for internal consistency).

use ed25519_dalek::VerifyingKey;
use warren_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};
use warren_relay_selector::{
    DirectoryError, VerifiedMultiHopDirectory, verify_multihop_directory_any,
};

/// Failure to verify + select a circuit from a fetched directory.
#[derive(Debug, thiserror::Error)]
pub enum SelectError {
    /// The signed directory failed the trust-chain verification.
    #[error("directory verification failed")]
    Verify(#[from] DirectoryError),
    /// The signed directory is past its `expires_at` (replay defense).
    #[error("directory is expired")]
    Expired,
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
}

/// Verifies the signed directory JSON against the baked root pin and
/// selects a circuit honoring the optional country hints. `two_hop`
/// picks a 2-hop circuit (entry != exit, different countries); otherwise
/// a 1-hop circuit (one node as both relay and exit). Returns `None` when
/// no node/pair satisfies the rules.
///
/// # Errors
/// [`DirectoryError`] when the JSON does not parse or the trust chain
/// (envelope signature, operational certificate under the baked root,
/// per-node attestation) does not verify, or when the signed directory
/// is past its `expires_at`.
pub fn verify_and_select(
    json: &str,
    two_hop: bool,
    entry_country: &str,
    exit_country: &str,
    now_unix: u64,
) -> Result<Option<SelectedCircuit>, SelectError> {
    // Root pin is the anchor (non-empty => operational cert is verified
    // against the offline root). Server pins empty: the envelope signature
    // is still checked for internal consistency, and the root chain stops
    // node forgery regardless.
    let dir = verify_multihop_directory_any(json, &[], &[WARREN_MULTIHOP_ROOT_PUBKEY_BAKED])?;
    if dir.is_expired(now_unix) {
        return Err(SelectError::Expired);
    }
    Ok(if two_hop {
        select_two_hop(&dir, entry_country, exit_country)
    } else {
        select_one_hop(&dir, exit_country)
    })
}

/// `true` if the directory spans >= 2 distinct (non-zero) ASNs, in which
/// case a 2-hop circuit must place entry and exit on different ASNs.
fn as_diversity_required(dir: &VerifiedMultiHopDirectory) -> bool {
    let mut seen = std::collections::HashSet::new();
    for n in &dir.nodes {
        if n.asn != 0 {
            seen.insert(n.asn);
        }
    }
    seen.len() >= 2
}

fn country_matches(filter: &str, country: &str) -> bool {
    filter.is_empty() || filter.eq_ignore_ascii_case(country)
}

/// Every `(entry_idx, exit_idx)` pair that forms a valid 2-hop circuit
/// under the country hints + security rules: distinct nodes, different
/// countries (mandatory), and different ASNs when the fleet supports it.
/// Mirrors `mullvad-daemon::warren_multi_hop_directory::valid_circuits`.
fn valid_circuits(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
) -> Vec<(usize, usize)> {
    let as_req = as_diversity_required(dir);
    let mut pairs = Vec::new();
    for (i, e) in dir.nodes.iter().enumerate() {
        if !country_matches(entry_country, &e.country) {
            continue;
        }
        for (j, x) in dir.nodes.iter().enumerate() {
            if i == j {
                continue;
            }
            if !country_matches(exit_country, &x.country) {
                continue;
            }
            if e.relay.relay_id == x.relay.relay_id {
                continue;
            }
            if e.country.eq_ignore_ascii_case(&x.country) {
                continue;
            }
            if as_req && (e.asn == 0 || x.asn == 0 || e.asn == x.asn) {
                continue;
            }
            pairs.push((i, j));
        }
    }
    pairs
}

/// Deterministic `(entry_idx, exit_idx)` pick: highest combined
/// `entry.weight * exit.weight`, ties broken by `(relay_id, exit_id)`.
/// Deterministic on purpose (a per-call RNG churned the daemon's tunnel
/// into a reconnect loop); `pairs` must be non-empty.
fn weighted_pick_pair(dir: &VerifiedMultiHopDirectory, pairs: &[(usize, usize)]) -> (usize, usize) {
    let weight = |&(i, j): &(usize, usize)| {
        dir.nodes[i]
            .weight
            .max(1)
            .saturating_mul(dir.nodes[j].weight.max(1))
    };
    let mut ranked: Vec<(usize, usize)> = pairs.to_vec();
    ranked.sort_by(|a, b| {
        weight(b)
            .cmp(&weight(a))
            .then_with(|| dir.nodes[a.0].relay.relay_id.cmp(&dir.nodes[b.0].relay.relay_id))
            .then_with(|| {
                dir.nodes[a.1]
                    .exit
                    .exit_id
                    .as_bytes()
                    .cmp(dir.nodes[b.1].exit.exit_id.as_bytes())
            })
    });
    ranked[0]
}

fn circuit_from(dir: &VerifiedMultiHopDirectory, entry_idx: usize, exit_idx: usize) -> SelectedCircuit {
    SelectedCircuit {
        relay: dir.nodes[entry_idx].relay.clone(),
        exit: dir.nodes[exit_idx].exit.clone(),
        operational_pubkey: dir.operational_pubkey,
    }
}

fn select_two_hop(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
) -> Option<SelectedCircuit> {
    let pairs = valid_circuits(dir, entry_country, exit_country);
    if pairs.is_empty() {
        return None;
    }
    let (entry_idx, exit_idx) = weighted_pick_pair(dir, &pairs);
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
