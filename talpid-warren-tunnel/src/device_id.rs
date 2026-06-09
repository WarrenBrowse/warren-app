//! Stable per-install device identifier for the Warren v2 device cap.
//!
//! The exit keys every live session by `(account_pubkey, device_id)` and
//! refuses a handshake once an account holds
//! [`warren_config::MAX_DEVICES_PER_ACCOUNT`] live leases
//! ([`warren_config::SESSION_LEASE_TTL_SECS`] TTL).
//!
//! If the client announced a **fresh random** `device_id` on every
//! connection attempt (as `ClientTunnel::with_signing_key` does by
//! default), the tunnel state machine's reconnect loop would mint a new
//! lease per attempt. A few failed attempts then exhaust the cap and the
//! account is locked out of connecting for a full TTL - an *undesired*
//! block that has nothing to do with the user's intent. See the incident
//! where repeated reconnects produced `handshake failed: device limit
//! reached for this account`.
//!
//! The fix: derive **one** identifier that is stable across reconnects,
//! retries and daemon restarts, and persist it next to the daemon
//! settings so it is the same for the whole install (and distinct per
//! machine, which keeps the cap meaningful across real devices).

use std::{path::Path, sync::OnceLock};

use warren_protocol::DEVICE_ID_LEN;

static DEVICE_ID: OnceLock<[u8; DEVICE_ID_LEN]> = OnceLock::new();

/// Returns the process-wide stable device id, loading or creating the
/// persisted value on first use.
///
/// Resolution order:
/// 1. persisted `<settings_dir>/warren-device-id` (created on first run);
/// 2. if the settings dir cannot be resolved, a per-process random id (still stable for this
///    process via the `OnceLock`, so the reconnect loop never leaks leases within a run).
#[must_use]
pub fn device_id() -> [u8; DEVICE_ID_LEN] {
    *DEVICE_ID.get_or_init(|| match mullvad_paths::settings_dir() {
        Ok(dir) => load_or_create(&dir.join("warren-device-id")),
        Err(error) => {
            log::warn!(
                "Warren: settings dir unavailable ({error}); using an ephemeral \
                 per-process device id (stable for this run only)"
            );
            rand::random()
        }
    })
}

/// Loads the 16-byte device id from `path`, or creates + persists a fresh
/// random one when the file is absent or malformed.
///
/// Never returns an error: on any I/O failure it falls back to a random
/// id so a connection is always possible (a non-persisted id is strictly
/// better than refusing to connect).
fn load_or_create(path: &Path) -> [u8; DEVICE_ID_LEN] {
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(id) = <[u8; DEVICE_ID_LEN]>::try_from(bytes.as_slice()) {
            return id;
        }
        log::warn!(
            "Warren: device-id file at {} is malformed ({} bytes); regenerating",
            path.display(),
            bytes.len()
        );
    }

    let id: [u8; DEVICE_ID_LEN] = rand::random();
    match std::fs::write(path, id) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Err(error) =
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                {
                    log::warn!("Warren: could not chmod 0600 the device-id file: {error}");
                }
            }
        }
        Err(error) => log::warn!(
            "Warren: could not persist device id to {} ({error}); \
             using an ephemeral id this run",
            path.display()
        ),
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp_dir(tag: &str) -> std::path::PathBuf {
        // Avoid extra dev-deps: derive a unique dir from pid + a counter.
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "warren-devid-{}-{}-{tag}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn load_or_create_persists_and_is_stable_for_a_path() {
        let dir = unique_tmp_dir("stable");
        let path = dir.join("warren-device-id");

        let first = load_or_create(&path);
        assert!(path.exists(), "device id must be persisted to disk");
        let second = load_or_create(&path);
        assert_eq!(
            first, second,
            "a persisted device id must be reused across calls (no per-call randomness) - \
             this is what stops the reconnect loop from exhausting the device cap"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn distinct_paths_get_distinct_ids() {
        // Different installs (different settings dirs) must not collide,
        // otherwise the per-account device cap could not distinguish two
        // real machines.
        let dir = unique_tmp_dir("distinct");
        let a = load_or_create(&dir.join("id-a"));
        let b = load_or_create(&dir.join("id-b"));
        assert_ne!(a, b, "independent device-id files must differ");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_file_is_regenerated_to_correct_length() {
        let dir = unique_tmp_dir("malformed");
        let path = dir.join("warren-device-id");
        std::fs::write(&path, b"too-short").expect("seed malformed file");

        let id = load_or_create(&path);
        // A wrong-length file is replaced by a correct 16-byte id, and
        // the persisted bytes round-trip on the next read.
        assert_eq!(load_or_create(&path), id, "regenerated id must persist");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
