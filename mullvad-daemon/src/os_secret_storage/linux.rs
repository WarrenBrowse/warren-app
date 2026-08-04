//! Linux secret storage sealed to the machine, via `systemd-creds`.
//!
//! `secret-service` needs a session D-Bus the root daemon never has, and the
//! kernel keyring does not survive a reboot, so the daemon used to fall back to
//! a plaintext file. `systemd-creds` closes that gap without inventing a custom
//! scheme: it is systemd's own credential encryption (AES-256-GCM), sealing to
//! the TPM2 when the machine has one, and otherwise to the root-only host key
//! `/var/lib/systemd/credential.secret`. That is the same guarantee Windows gets
//! from DPAPI machine scope and macOS from the System Keychain, so the three
//! desktop platforms now agree: a copy of the settings directory (a backup, a
//! snapshot, a stolen disk on a TPM machine) does not yield the mnemonic.
//!
//! `--name` is bound into the AEAD's associated data, so a blob sealed for one
//! key cannot be replayed under another.
//!
//! Legacy installs carry a plaintext `<key>.txt` from the previous backend.
//! `load` migrates it in place on first read (seal, then remove the plaintext
//! and any tempfile left by an interrupted write), so an upgrade both keeps the
//! wallet working and stops leaving the phrase in clear.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zeroize::Zeroizing;

use super::SecretStorage;
use super::plaintext::PlaintextStorage;

const SECRETS_SUBDIR: &str = "secrets";
const SYSTEMD_CREDS: &str = "systemd-creds";

pub struct SystemdCredsStorage {
    secrets_dir: PathBuf,
    /// Owns the atomic write, the 0o600 mode and the tempfile sweep. We hand it
    /// ciphertext, never the secret, and reuse it rather than duplicating the
    /// crash-safe write dance.
    blobs: PlaintextStorage,
}

impl SystemdCredsStorage {
    /// Fails when `systemd-creds` is absent or cannot seal on this host (no
    /// TPM2 *and* no host key, a container without `/var/lib/systemd`, a
    /// non-systemd distribution). The caller then falls back to the previous
    /// backend, so an exotic host degrades instead of losing its wallet.
    pub fn new(settings_dir: &Path) -> io::Result<Self> {
        let storage = Self {
            secrets_dir: settings_dir.join(SECRETS_SUBDIR),
            blobs: PlaintextStorage::new(settings_dir),
        };
        // Probe with a real round trip: `systemd-creds` being on PATH proves
        // nothing about whether this host can actually seal (the host key is
        // created lazily and needs root).
        let probe = storage.seal("warren_probe", b"probe")?;
        let opened = storage.unseal("warren_probe", &probe)?;
        if opened.as_slice() != b"probe" {
            return Err(io::Error::other(
                "systemd-creds round trip returned unexpected plaintext",
            ));
        }
        Ok(storage)
    }

    fn run(&self, args: &[&str], stdin_bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        let mut child = Command::new(SYSTEMD_CREDS)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("systemd-creds stdin unavailable"))?
            .write_all(stdin_bytes)?;

        let out = child.wait_with_output()?;
        if !out.status.success() {
            // stderr can name the host key path and the TPM state; it never
            // echoes the credential itself, and it is what makes a sealing
            // failure diagnosable. Keep it, it carries no secret.
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(io::Error::other(format!(
                "systemd-creds {} failed: {}",
                args.first().copied().unwrap_or("?"),
                err.trim()
            )));
        }
        Ok(Zeroizing::new(out.stdout))
    }

    fn seal(&self, key: &str, value: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        self.run(&["encrypt", &format!("--name={key}"), "-", "-"], value)
    }

    fn unseal(&self, key: &str, blob: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        self.run(&["decrypt", &format!("--name={key}"), "-", "-"], blob)
    }

    /// Path of the legacy plaintext file this backend supersedes.
    fn legacy_path(&self, key: &str) -> io::Result<PathBuf> {
        if !super::is_valid_storage_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("secret storage key {key:?} contains forbidden characters"),
            ));
        }
        Ok(self.secrets_dir.join(format!("{key}.txt")))
    }

    /// Seals a legacy plaintext entry and removes the clear copy. Returns the
    /// secret so the caller's `load` can serve the very first read that
    /// triggered the migration.
    fn migrate_legacy(&self, key: &str) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
        let legacy = self.legacy_path(key)?;
        let clear = match fs::read(&legacy) {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };

        self.store(key, &clear)?;
        // Only now that the sealed copy is durable do we drop the clear one:
        // a crash in between leaves both, never neither.
        let _ = fs::remove_file(&legacy);
        self.blobs.delete(key)?;

        log::info!("migrated the Linux secret store from a plaintext file to systemd-creds");
        Ok(Some(clear))
    }

    fn blob_key(key: &str) -> String {
        format!("{key}_cred")
    }
}

impl SecretStorage for SystemdCredsStorage {
    fn store(&self, key: &str, value: &[u8]) -> io::Result<()> {
        let sealed = self.seal(key, value)?;
        self.blobs.store(&Self::blob_key(key), &sealed)
    }

    fn load(&self, key: &str) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
        match self.blobs.load(&Self::blob_key(key))? {
            Some(blob) => Ok(Some(self.unseal(key, &blob)?)),
            None => self.migrate_legacy(key),
        }
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        // Sweep the legacy plaintext too: a sign-out that leaves the old clear
        // file behind would defeat the whole point of this backend.
        if let Ok(legacy) = self.legacy_path(key) {
            let _ = fs::remove_file(legacy);
        }
        let _ = self.blobs.delete(key);
        self.blobs.delete(&Self::blob_key(key))
    }

    fn is_plaintext(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "Linux systemd-creds (TPM2 when present, else the root-only host key)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn systemd_creds_available() -> bool {
        Command::new(SYSTEMD_CREDS)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn isolated_tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-creds-{pid}-{n}"));
        fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn store_seals_the_secret_and_never_writes_it_in_clear() {
        if !systemd_creds_available() {
            return;
        }
        let dir = isolated_tempdir();
        let Ok(storage) = SystemdCredsStorage::new(&dir) else {
            return; // host cannot seal (no TPM2 and no host key); nothing to assert
        };

        let phrase = b"abandon abandon abandon abandon abandon abandon \
                       abandon abandon abandon abandon abandon about";
        storage.store("warren_mnemonic", phrase).expect("store");

        let on_disk = fs::read(dir.join(SECRETS_SUBDIR).join("warren_mnemonic_cred.txt"))
            .expect("sealed blob written");
        assert!(
            !on_disk
                .windows(phrase.len())
                .any(|w| w == phrase.as_slice()),
            "the sealed blob MUST NOT contain the phrase in clear"
        );

        let loaded = storage
            .load("warren_mnemonic")
            .expect("load")
            .expect("present");
        assert_eq!(loaded.as_slice(), phrase.as_slice());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_migrates_a_legacy_plaintext_entry_and_removes_the_clear_copy() {
        if !systemd_creds_available() {
            return;
        }
        let dir = isolated_tempdir();
        let Ok(storage) = SystemdCredsStorage::new(&dir) else {
            return;
        };

        // An install from the previous backend: the phrase sits in clear.
        let legacy = PlaintextStorage::new(&dir);
        let phrase = b"legacy twelve word phrase";
        legacy
            .store("warren_mnemonic", phrase)
            .expect("seed legacy");
        let legacy_path = dir.join(SECRETS_SUBDIR).join("warren_mnemonic.txt");
        assert!(legacy_path.exists());

        let loaded = storage
            .load("warren_mnemonic")
            .expect("load")
            .expect("present");

        assert_eq!(
            loaded.as_slice(),
            phrase.as_slice(),
            "the upgrade must not lose the wallet"
        );
        assert!(
            !legacy_path.exists(),
            "the plaintext copy MUST be gone once the sealed one is durable"
        );
        assert_eq!(
            storage
                .load("warren_mnemonic")
                .expect("second load")
                .expect("still present")
                .as_slice(),
            phrase.as_slice(),
            "the migrated entry must survive the migration"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_removes_both_the_sealed_blob_and_any_legacy_plaintext() {
        if !systemd_creds_available() {
            return;
        }
        let dir = isolated_tempdir();
        let Ok(storage) = SystemdCredsStorage::new(&dir) else {
            return;
        };

        storage.store("warren_mnemonic", b"phrase").expect("store");
        PlaintextStorage::new(&dir)
            .store("warren_mnemonic", b"phrase")
            .expect("seed a stale legacy copy");

        storage.delete("warren_mnemonic").expect("delete");

        assert!(storage.load("warren_mnemonic").expect("load").is_none());
        assert!(
            !dir.join(SECRETS_SUBDIR)
                .join("warren_mnemonic.txt")
                .exists()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_blob_cannot_be_replayed_under_another_key() {
        if !systemd_creds_available() {
            return;
        }
        let dir = isolated_tempdir();
        let Ok(storage) = SystemdCredsStorage::new(&dir) else {
            return;
        };

        let sealed = storage.seal("warren_mnemonic", b"phrase").expect("seal");
        // `--name` is bound into the AAD, so opening it as another credential
        // must fail rather than hand back the phrase.
        assert!(
            storage.unseal("other_key", &sealed).is_err(),
            "a blob sealed under one name MUST NOT open under another"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backend_does_not_report_itself_as_plaintext() {
        if !systemd_creds_available() {
            return;
        }
        let dir = isolated_tempdir();
        if let Ok(storage) = SystemdCredsStorage::new(&dir) {
            assert!(!storage.is_plaintext());
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
