//! Persistent trust-on-first-use (TOFU) store for exit Ed25519 pubkeys.
//!
//! Mirrors the desktop daemon's `warren_pinned_exit_pubkeys` setting
//! (`mullvad-types::settings::WarrenPinnedExitPubkeys` +
//! `mullvad-daemon::tunnel::warren_pin_verify`). iOS has no long-lived
//! daemon, so the pin table lives in a JSON file inside the App Group
//! container (path supplied by Swift) and is read on each connect.
//!
//! Why this matters on the multi-hop path: the exit's `exit_id` is a stable
//! 16-byte routing tag covered by the directory signature, but the exit's
//! `exit_ed25519_pubkey` (its QUIC TLS RPK identity) is NOT signature-covered
//! in `/v1`. TOFU pins the observed RPK against the signed `exit_id` so a
//! later key swap under the same `exit_id` (a compromised exit presenting a
//! different RPK) is detected and the connection fails closed until the user
//! explicitly trusts the new key.
//!
//! Threat model: the file is NOT authenticated. Its integrity rests on the
//! App Group sandbox (unreachable by other apps on a non-jailbroken device).
//! A tampered/cleared file degrades to "trust on next first-seen", never to
//! accepting a key the user already rejected silently; the connection still
//! fails closed on a genuine mismatch against whatever is stored.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The persisted pin table. Keyed by lowercase-hex `exit_id` (16-byte
/// routing tag on the multi-hop path), mirroring the desktop setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedExitPubkeys {
    /// `exit_id_hex` -> the trusted pin. `BTreeMap` for deterministic
    /// serialization (stable file diffs / reproducible tests).
    #[serde(default)]
    pub entries: BTreeMap<String, PinnedExitPubkey>,
}

/// One trusted exit pin plus forensic context for the mismatch UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedExitPubkey {
    /// Hex-encoded 32-byte Ed25519 verifying key trusted for this exit.
    pub pubkey_hex: String,
    /// Unix seconds when this pin was first established.
    pub first_seen_unix: u64,
    /// Unix seconds of the most recent match (bumped on every reconnect
    /// where the observed pubkey matches `pubkey_hex`).
    pub last_seen_unix: u64,
    /// ISO 3166 alpha-2 country code captured at pin time (forensic
    /// context for the mismatch report). Empty when unknown.
    #[serde(default)]
    pub country_code: String,
}

/// Outcome of a single pin check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinOutcome {
    /// No pin existed for this `exit_id`: one was just inserted (TOFU).
    FirstSeen,
    /// The observed pubkey matches the stored pin.
    Match,
    /// The observed pubkey differs from the stored pin. Carries the
    /// previously trusted pubkey for the mismatch report. The caller MUST
    /// fail the connection closed and ask the user to trust or reject.
    Mismatch { pinned: String },
}

/// Pure pin-check against a mutable table, ported verbatim from the desktop
/// `warren_pin_verify`. On `FirstSeen` it inserts; on `Match` it bumps
/// `last_seen_unix`; on `Mismatch` it leaves the existing pin untouched (the
/// user decides via `trust`). The caller persists the table afterwards.
pub fn pin_verify(
    table: &mut PinnedExitPubkeys,
    exit_id_hex: &str,
    observed_pubkey_hex: &str,
    country_code: &str,
    now_unix: u64,
) -> PinOutcome {
    match table.entries.get_mut(exit_id_hex) {
        None => {
            table.entries.insert(
                exit_id_hex.to_owned(),
                PinnedExitPubkey {
                    pubkey_hex: observed_pubkey_hex.to_owned(),
                    first_seen_unix: now_unix,
                    last_seen_unix: now_unix,
                    country_code: country_code.to_owned(),
                },
            );
            PinOutcome::FirstSeen
        }
        Some(existing) if existing.pubkey_hex == observed_pubkey_hex => {
            existing.last_seen_unix = now_unix;
            PinOutcome::Match
        }
        Some(existing) => PinOutcome::Mismatch {
            pinned: existing.pubkey_hex.clone(),
        },
    }
}

/// Load the pin table from `path`. Returns an empty table when the file is
/// absent, unreadable, or does not parse, so a fresh install or a corrupt
/// file degrades to "trust on first-seen" rather than bricking the tunnel.
pub fn load(path: &Path) -> PinnedExitPubkeys {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist `table` to `path` atomically (temp file + rename), so a reader
/// never observes a partially-written table and a crash mid-write cannot
/// tear the file. Best-effort: a failure is logged and ignored.
pub fn store(path: &Path, table: &PinnedExitPubkeys) {
    let Ok(json) = serde_json::to_string(table) else {
        return;
    };
    // The temp name carries the pid so two overlapping extension processes
    // (an old and a new NetworkExtension during a handover) never share one
    // temp path and corrupt each other's write.
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, json) {
        log_persist_failure("write temp file", &e);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        log_persist_failure("commit pin table", &e);
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Trust a (possibly new) pubkey for `exit_id`, overwriting any existing
/// pin. Called when the user accepts a mismatch ("Trust new key"). Resets
/// `first_seen_unix` to now since this is a fresh trust decision. Persists.
pub fn trust(path: &Path, exit_id_hex: &str, pubkey_hex: &str, country_code: &str, now_unix: u64) {
    let mut table = load(path);
    table.entries.insert(
        exit_id_hex.to_owned(),
        PinnedExitPubkey {
            pubkey_hex: pubkey_hex.to_owned(),
            first_seen_unix: now_unix,
            last_seen_unix: now_unix,
            country_code: country_code.to_owned(),
        },
    );
    store(path, &table);
}

/// Clear all pins. Returns the number of entries dropped (for the Settings
/// "Reset pinned exit keys" confirmation). Persists the now-empty table.
pub fn reset(path: &Path) -> usize {
    let table = load(path);
    let count = table.entries.len();
    if count > 0 {
        store(path, &PinnedExitPubkeys::default());
    }
    count
}

/// Log a best-effort persistence failure. Only emits under the `tunnel`
/// feature (production iOS) where `tracing` is linked; the host `cargo test`
/// build compiles this module without that dependency, so it no-ops.
#[cfg(feature = "tunnel")]
fn log_persist_failure(stage: &str, e: &std::io::Error) {
    tracing::warn!(error = %e, stage, "failed to persist exit pubkey pin table");
}
#[cfg(not(feature = "tunnel"))]
fn log_persist_failure(_stage: &str, _e: &std::io::Error) {}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_A: &str = "11111111111111111111111111111111";
    const KEY_1: &str = "aaaa";
    const KEY_2: &str = "bbbb";

    #[test]
    fn first_seen_then_match_then_mismatch() {
        let mut t = PinnedExitPubkeys::default();

        assert_eq!(
            pin_verify(&mut t, ID_A, KEY_1, "FR", 100),
            PinOutcome::FirstSeen
        );
        // The pin was recorded.
        assert_eq!(t.entries.get(ID_A).unwrap().pubkey_hex, KEY_1);
        assert_eq!(t.entries.get(ID_A).unwrap().first_seen_unix, 100);

        // Same key later => Match, last_seen bumped, first_seen untouched.
        assert_eq!(
            pin_verify(&mut t, ID_A, KEY_1, "FR", 200),
            PinOutcome::Match
        );
        assert_eq!(t.entries.get(ID_A).unwrap().first_seen_unix, 100);
        assert_eq!(t.entries.get(ID_A).unwrap().last_seen_unix, 200);

        // A different key under the same exit_id => Mismatch, pin untouched.
        assert_eq!(
            pin_verify(&mut t, ID_A, KEY_2, "FR", 300),
            PinOutcome::Mismatch {
                pinned: KEY_1.to_owned()
            }
        );
        assert_eq!(t.entries.get(ID_A).unwrap().pubkey_hex, KEY_1);
    }

    #[test]
    fn load_missing_is_empty_and_store_round_trips() {
        let path = std::env::temp_dir().join("warren-pin-roundtrip-xyz.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load(&path), PinnedExitPubkeys::default());

        let mut t = PinnedExitPubkeys::default();
        pin_verify(&mut t, ID_A, KEY_1, "DE", 42);
        store(&path, &t);
        assert_eq!(load(&path), t);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_loads_as_empty() {
        let path = std::env::temp_dir().join("warren-pin-corrupt-xyz.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), PinnedExitPubkeys::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn trust_overwrites_and_reset_clears() {
        let path = std::env::temp_dir().join("warren-pin-trust-xyz.json");
        let _ = std::fs::remove_file(&path);

        let mut t = PinnedExitPubkeys::default();
        pin_verify(&mut t, ID_A, KEY_1, "FR", 10);
        store(&path, &t);

        // Trust a new key for the same exit: overwrites the pin.
        trust(&path, ID_A, KEY_2, "FR", 20);
        assert_eq!(load(&path).entries.get(ID_A).unwrap().pubkey_hex, KEY_2);

        // After trusting, the new key matches.
        let mut t2 = load(&path);
        assert_eq!(
            pin_verify(&mut t2, ID_A, KEY_2, "FR", 30),
            PinOutcome::Match
        );

        // Reset drops everything and reports the count.
        assert_eq!(reset(&path), 1);
        assert!(load(&path).entries.is_empty());
        // Reset on an empty table reports zero.
        assert_eq!(reset(&path), 0);

        let _ = std::fs::remove_file(&path);
    }
}
