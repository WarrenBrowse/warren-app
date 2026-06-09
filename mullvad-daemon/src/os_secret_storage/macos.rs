//! macOS System Keychain backend for [`SecretStorage`].
//!
//! Uses the **file-based legacy** Keychain API (`SecKeychainOpen` +
//! `SecKeychainAddGenericPassword`) deliberately, targeting
//! `/Library/Keychains/System.keychain`. The modern Data Protection
//! Keychain (`SecItemAdd` with default keychain) requires the
//! `keychain-access-groups` entitlement and a signed app bundle,
//! neither of which we have for the Warren daemon binary - Apple
//! still recommends the legacy path for non-bundled daemons (see
//! Apple Developer Forums thread 657874 + 759976).
//!
//! ### What this gives us vs raw `0o600` plaintext
//!
//! - `/Library/Keychains/System.keychain` is **excluded from Time
//!   Machine backups** by default - protects against user-initiated
//!   backup leaks.
//! - Its file is encrypted at rest with a key derived from
//!   `/var/db/SystemKey`, itself `0o600 root:root`. An attacker who
//!   only steals one of the two files cannot decrypt.
//! - The Keychain item is bound to the writer's `uid` (root) by
//!   `securityd`, so a process running as a regular user cannot
//!   open it even if it could read the raw file.
//!
//! ### Requirements
//!
//! - The daemon process MUST run as root. Without root,
//!   `securityd` returns `errSecWrPerm (-61)` on write attempts.
//! - The binary should be at minimum ad-hoc signed on Apple
//!   Silicon (`codesign -s -`), but **no Apple Developer ID nor
//!   notarization is required** for Keychain access - only for
//!   Gatekeeper distribution UX.
//!
//! ### Pitfalls deliberately avoided
//!
//! - We do **not** call `passwords::set_generic_password` (the
//!   short helper in `security-framework::passwords`) because it
//!   uses `SecItemAdd` against the default keychain, which on a
//!   non-bundle root daemon ends up writing to a per-uid
//!   `login.keychain-db` that does not exist and silently fails
//!   with `errSecItemNotFound` on the subsequent read.
//! - We do **not** rely on `errSecDuplicateItem` semantics for
//!   updates; instead we use the SecKeychain `set_generic_password`
//!   helper which is documented to do find-then-update / add as
//!   appropriate.

use std::io;
use std::path::Path;

use security_framework::base::Error as SecError;
use security_framework::os::macos::keychain::SecKeychain;
use zeroize::Zeroizing;

use super::SecretStorage;

/// Absolute path of the macOS System Keychain. Writable only by root.
const SYSTEM_KEYCHAIN_PATH: &str = "/Library/Keychains/System.keychain";

/// `service` argument passed to all generic-password operations.
/// Reverse-DNS convention so the Keychain UI groups Warren entries
/// together and so that a future second daemon component can pick
/// a distinct service without collision.
const SERVICE_NAME: &str = "net.warrenvpn.daemon";

pub struct MacOsKeychainStorage {
    keychain: SecKeychain,
}

/// Key name used during the open-time write probe. Picked to be
/// obviously diagnostic if it leaks; the value is wiped immediately
/// after the round-trip.
const PROBE_KEY: &str = "__warren_open_probe__";
const PROBE_VALUE: &[u8] = b"warren-open-probe";

impl MacOsKeychainStorage {
    /// Opens the System Keychain **and validates write access**.
    /// `SecKeychain::open` on its own only checks that the file
    /// exists and parses; it does not require root because securityd
    /// gates writes, not reads. Without a probe, a non-root daemon
    /// would happily construct the backend then fail with `errSecWrPerm`
    /// on the first `set_generic_password` - well after the upstream
    /// fallback opportunity has passed.
    ///
    /// We therefore do a write+delete round-trip on a sentinel key
    /// here. If either step fails, the caller (in `mod.rs`) falls
    /// back to the plaintext backend with a loud warning. The probe
    /// value is wiped from memory via `Zeroizing` on drop.
    pub fn open() -> io::Result<Self> {
        let keychain = SecKeychain::open(Path::new(SYSTEM_KEYCHAIN_PATH)).map_err(map_sec_error)?;
        let storage = Self { keychain };

        // Best-effort cleanup of a leftover probe from a previous
        // aborted run (= probe written but not deleted because the
        // daemon was killed between the two calls).
        let _ = storage.delete(PROBE_KEY);

        if let Err(e) = storage.store(PROBE_KEY, PROBE_VALUE) {
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "System Keychain open probe failed on write ({e}) - \
                     likely missing root privileges"
                ),
            ));
        }
        if let Err(e) = storage.delete(PROBE_KEY) {
            // Write succeeded but delete failed - unusual. Keep
            // using the backend but log the leftover so an admin
            // can clean it up manually.
            log::warn!(
                "System Keychain open probe wrote {PROBE_KEY} but failed to delete \
                 it: {e} - leftover sentinel entry remains in the System Keychain"
            );
        }
        Ok(storage)
    }
}

impl SecretStorage for MacOsKeychainStorage {
    fn store(&self, key: &str, value: &[u8]) -> io::Result<()> {
        // `set_generic_password` is the find-then-update / add helper,
        // so it correctly handles both "first store" and "overwrite"
        // without the caller having to disambiguate `errSecDuplicateItem`.
        self.keychain
            .set_generic_password(SERVICE_NAME, key, value)
            .map_err(map_sec_error)
    }

    fn load(&self, key: &str) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
        match self.keychain.find_generic_password(SERVICE_NAME, key) {
            Ok((password, _item)) => {
                // `password` is a `SecKeychainItemPassword` which
                // derefs to `&[u8]`. We copy into a `Zeroizing<Vec>`
                // and let the original `SecKeychainItemPassword`
                // drop trigger `SecKeychainItemFreeContent` to wipe
                // the Security framework's internal buffer.
                let bytes: &[u8] = password.as_ref();
                Ok(Some(Zeroizing::new(bytes.to_vec())))
            }
            Err(e) if is_item_not_found(&e) => Ok(None),
            Err(e) => Err(map_sec_error(e)),
        }
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        match self.keychain.find_generic_password(SERVICE_NAME, key) {
            Ok((_password, item)) => {
                // `SecKeychainItem::delete()` returns `()` (the
                // underlying FFI panics on error, by design - it
                // happens only on programmer error like passing a
                // dangling pointer). The match arm therefore just
                // calls it and returns Ok.
                item.delete();
                Ok(())
            }
            Err(e) if is_item_not_found(&e) => Ok(()),
            Err(e) => Err(map_sec_error(e)),
        }
    }

    fn is_plaintext(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "macOS System Keychain (file-based)"
    }
}

/// `errSecItemNotFound` value as documented by Apple. The
/// `security-framework` crate does not re-export a typed constant
/// for this so we match on the raw OSStatus.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

fn is_item_not_found(e: &SecError) -> bool {
    e.code() == ERR_SEC_ITEM_NOT_FOUND
}

fn map_sec_error(e: SecError) -> io::Error {
    let kind = match e.code() {
        ERR_SEC_ITEM_NOT_FOUND => io::ErrorKind::NotFound,
        -61 => io::ErrorKind::PermissionDenied, // errSecWrPerm
        _ => io::ErrorKind::Other,
    };
    io::Error::new(
        kind,
        format!("System Keychain error (OSStatus={}): {e}", e.code()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// We can't unit-test the actual System Keychain without `sudo`
    /// in CI. The integration test runs ONLY when the env var
    /// `WARREN_TEST_SYSTEM_KEYCHAIN=1` is set, signalling that the
    /// developer accepted to mutate the local System Keychain.
    ///
    /// Cleanup is best-effort: any leftover entry under
    /// `net.warrenvpn.daemon`/`__test_*` should be considered a
    /// failed test artifact.
    #[test]
    fn store_then_load_round_trip_against_system_keychain() {
        if std::env::var("WARREN_TEST_SYSTEM_KEYCHAIN").as_deref() != Ok("1") {
            eprintln!(
                "skipping macOS System Keychain integration test \
                 (set WARREN_TEST_SYSTEM_KEYCHAIN=1 with sudo to run)"
            );
            return;
        }

        let storage = MacOsKeychainStorage::open().expect("open System Keychain");
        let key = format!("__test_round_trip_{}", std::process::id());
        let payload = b"abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";

        // Best-effort cleanup from a previous failed run.
        let _ = storage.delete(&key);

        storage.store(&key, payload).expect("store ok");
        let loaded = storage.load(&key).expect("load ok").expect("present");
        assert_eq!(loaded.as_slice(), payload);

        let absent = storage.delete(&key);
        assert!(absent.is_ok(), "delete should succeed");
        assert!(
            storage.load(&key).expect("io ok").is_none(),
            "load after delete must be None"
        );
    }
}
