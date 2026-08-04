//! Persistent anti-rollback high-water mark for the multi-hop directory.
//!
//! iOS has no long-lived daemon to hold the highest trusted directory
//! `generation` in memory (the way `mullvad-daemon` does), so each tunnel
//! connect is effectively a cold start. To keep the rollback gate
//! meaningful across connects, the FFI persists the high-water mark in a
//! small file inside the App Group container (path supplied by Swift).
//!
//! The value is a single decimal `u64`. Reads default to `0` (gate
//! disabled) on any error, so a missing/corrupt file degrades to
//! "first connect" rather than bricking the tunnel. Writes are atomic
//! (temp file + rename) so a crash mid-write cannot leave a torn value.
//!
//! Threat model: the file is NOT authenticated. Its integrity rests on the
//! App Group container sandbox (unreachable by other apps on a non-jailbroken
//! device). The gate is defense-in-depth on top of the signature + root-cert
//! verification, which always runs regardless of this file; a tampered file
//! can at worst revert the gate to an earlier-but-still-signature-verified
//! directory, never accept an unsigned or unrooted one.

use std::path::Path;

/// Read the persisted high-water generation. Returns `0` (gate disabled)
/// when the file is absent, unreadable, or does not parse, so a fresh
/// install or a corrupt file falls back to "trust the first directory".
pub fn read_high_water(path: &Path) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

/// Persist `generation` as the new high-water mark, but only when it is
/// strictly higher than what is already stored (monotonic; never lowers
/// the mark). Best-effort: a write failure is logged and ignored, since
/// the gate still held for this connect via the in-memory value.
pub fn raise_high_water(path: &Path, generation: u64) {
    if generation <= read_high_water(path) {
        return;
    }
    // Atomic replace: write to a sibling temp file then rename over the
    // target so a reader never observes a partially-written value. The temp
    // name carries the pid so two overlapping extension processes (an old
    // and a new NetworkExtension during a handover) never share one temp
    // path and corrupt each other's write.
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, generation.to_string()) {
        log_persist_failure("write temp file", &e);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        log_persist_failure("commit high-water mark", &e);
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Log a best-effort persistence failure. Only emits under the `tunnel`
/// feature (production iOS), where `tracing` is linked; the host `cargo
/// test` build compiles this module without that dependency, so it no-ops.
#[cfg(feature = "tunnel")]
fn log_persist_failure(stage: &str, e: &std::io::Error) {
    tracing::warn!(error = %e, stage, "failed to persist multi-hop generation high-water mark");
}
#[cfg(not(feature = "tunnel"))]
fn log_persist_failure(_stage: &str, _e: &std::io::Error) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_missing_file_is_zero() {
        let path = std::env::temp_dir().join("warren-mh-gen-missing-xyz.txt");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_high_water(&path), 0);
    }

    #[test]
    fn raise_then_read_round_trips_and_is_monotonic() {
        let path = std::env::temp_dir().join("warren-mh-gen-roundtrip.txt");
        let _ = std::fs::remove_file(&path);

        raise_high_water(&path, 7);
        assert_eq!(read_high_water(&path), 7);

        // A lower generation must not lower the mark.
        raise_high_water(&path, 3);
        assert_eq!(read_high_water(&path), 7);

        // A higher generation raises it.
        raise_high_water(&path, 12);
        assert_eq!(read_high_water(&path), 12);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_reads_as_zero() {
        let path = std::env::temp_dir().join("warren-mh-gen-corrupt.txt");
        std::fs::write(&path, "not-a-number").unwrap();
        assert_eq!(read_high_water(&path), 0);
        let _ = std::fs::remove_file(&path);
    }
}
