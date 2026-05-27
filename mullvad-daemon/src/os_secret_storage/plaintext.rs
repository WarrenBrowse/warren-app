//! Plaintext file-based secret storage.
//!
//! Used as the **default backend on Linux** (where no daemon-friendly
//! OS-native store exists: `secret-service` requires a session D-Bus
//! that the daemon never has, and the kernel keyring does not
//! survive a reboot), and as the **explicit fallback** on macOS /
//! Windows when their respective OS-native backends fail to
//! initialize at startup.
//!
//! The implementation purposefully avoids any custom "obfuscation
//! pretending to be encryption" — that pattern is exactly the
//! Signal Desktop BasicTextEncryption fiasco of 2024 (locally
//! readable key shipped with the binary). When this backend is in
//! use, we surface `is_plaintext() == true` so the boot path can log
//! a loud warning, and a single source of truth governs the file
//! layout.
//!
//! File layout: each `key` becomes a sibling file under
//! `<settings_dir>/secrets/<key>.txt`. We deliberately keep `.txt`
//! to match Mullvad upstream's habit and so that a user inspecting
//! their settings dir sees instantly that the file is sensitive
//! plaintext.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use super::SecretStorage;

/// Sub-directory of `settings_dir` that holds plaintext secret
/// files. Keeping them in a dedicated directory makes filesystem
/// ACLs and exclusion-from-backup rules easier to express.
const SECRETS_SUBDIR: &str = "secrets";

pub struct PlaintextStorage {
    secrets_dir: PathBuf,
}

impl PlaintextStorage {
    pub fn new(settings_dir: &Path) -> Self {
        Self {
            secrets_dir: settings_dir.join(SECRETS_SUBDIR),
        }
    }

    fn key_path(&self, key: &str) -> io::Result<PathBuf> {
        // The `key` namespace is internal to `mullvad-daemon` and
        // every current caller uses hard-coded identifiers like
        // `"warren_mnemonic"`. We still validate at runtime as a
        // defense-in-depth measure — `debug_assert!` would compile
        // out in release builds and the trait being `pub` makes
        // future misuse possible.
        if !super::is_valid_storage_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("secret storage key {key:?} contains forbidden characters"),
            ));
        }
        Ok(self.secrets_dir.join(format!("{key}.txt")))
    }

    fn ensure_dir(&self) -> io::Result<()> {
        if self.secrets_dir.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.secrets_dir)?;
        // Tighten directory perms on Unix-like systems so that a
        // file with relaxed mode created here is still protected by
        // the enclosing directory. No-op on Windows where ACLs are
        // inherited from the parent (settings_dir is already
        // ProgramData-style and root-owned).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&self.secrets_dir)?.permissions();
            perm.set_mode(0o700);
            fs::set_permissions(&self.secrets_dir, perm)?;
        }
        Ok(())
    }
}

impl SecretStorage for PlaintextStorage {
    fn store(&self, key: &str, value: &[u8]) -> io::Result<()> {
        self.ensure_dir()?;
        let final_path = self.key_path(key)?;

        // Atomic write: tempfile in the same directory + rename.
        // Mirrors `warren_signer::set_warren_mnemonic`'s pattern so
        // a daemon crash mid-write never corrupts the existing
        // entry. The tempfile name embeds `pid` + nanos to dodge
        // collisions between concurrent writers.
        let parent = final_path
            .parent()
            .ok_or_else(|| io::Error::other("plaintext storage path has no parent"))?;
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let file_name = final_path
            .file_name()
            .and_then(|os| os.to_str())
            .ok_or_else(|| io::Error::other("plaintext storage key has no file name"))?;
        let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{nanos}"));

        // Best-effort cleanup of stale tempfile.
        let _ = fs::remove_file(&tmp_path);

        {
            use std::io::Write;
            #[cfg(unix)]
            let mut f = {
                use std::os::unix::fs::OpenOptionsExt;
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&tmp_path)?
            };
            #[cfg(not(unix))]
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;

            f.write_all(value)?;
            f.sync_all()?;
        }

        match fs::rename(&tmp_path, &final_path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp_path);
                Err(e)
            }
        }
    }

    fn load(&self, key: &str) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
        let path = self.key_path(key)?;
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        let path = self.key_path(key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn is_plaintext(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        if cfg!(target_os = "linux") {
            "Linux plaintext file (no daemon-friendly OS secret store)"
        } else {
            "plaintext file fallback (OS-native store unavailable)"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated_tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-plaintext-{pid}-{nanos}-{n}"));
        fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn store_then_load_round_trip() {
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        let payload = b"abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";
        storage.store("mnemonic", payload).expect("store ok");

        let loaded = storage.load("mnemonic").expect("load ok").expect("present");
        assert_eq!(loaded.as_slice(), payload);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_key_is_none() {
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        let loaded = storage.load("never_set").expect("io ok");
        assert!(loaded.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_overwrites_existing_value_atomically() {
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        storage.store("k", b"v1").expect("first write");
        storage.store("k", b"v2").expect("overwrite");

        let loaded = storage.load("k").expect("load").expect("present");
        assert_eq!(loaded.as_slice(), b"v2");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_removes_entry_and_is_idempotent() {
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        storage.store("k", b"v").expect("write");
        storage.delete("k").expect("delete first time");
        storage.delete("k").expect("delete is idempotent on missing");

        assert!(storage.load("k").expect("load ok").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_plaintext_returns_true() {
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);
        assert!(storage.is_plaintext());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_path_rejects_path_traversal_in_release_builds() {
        // F-2 regression: ensures the path-traversal guard fires
        // even in release builds (i.e. no `debug_assert!` reliance).
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        for malicious in &[
            "../escape",
            "../../etc/passwd",
            "..",
            ".",
            "",
            "subdir/key",
        ] {
            let err = storage
                .store(malicious, b"x")
                .expect_err(&format!("must reject key {malicious:?}"));
            assert_eq!(
                err.kind(),
                io::ErrorKind::InvalidInput,
                "rejection for {malicious:?} must surface as InvalidInput",
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn stored_file_has_0o600_perms() {
        use std::os::unix::fs::PermissionsExt;
        let dir = isolated_tempdir();
        let storage = PlaintextStorage::new(&dir);

        storage.store("k", b"v").expect("write");
        let path = storage.key_path("k").expect("valid key");
        let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "secret file MUST be 0o600 root-only");

        let _ = fs::remove_dir_all(&dir);
    }
}
