//! Windows DPAPI backend for [`SecretStorage`].
//!
//! Uses `CryptProtectData` / `CryptUnprotectData` with the
//! `CRYPTPROTECT_LOCAL_MACHINE` flag. The Warren daemon runs as
//! `LocalSystem` via the Service Control Manager, so the
//! machine-scope DPAPI master key (held in `LSASS`'s memory and
//! derived from `\SYSTEM` registry hive at boot) is the right
//! authentication context: the ciphertext can be read back by any
//! SYSTEM-context code on the same machine after a reboot, but is
//! cryptographically unreadable when the disk is moved offline.
//!
//! ### What this gives us vs raw plaintext on Windows
//!
//! - Ciphertext bound to the machine: an unencrypted volume cloned
//!   to another machine will not decrypt without also extracting
//!   the source machine's DPAPI master key (offline attack
//!   requires `\SYSTEM` registry hive + boot key).
//! - Windows Backup excludes DPAPI master keys by default - a
//!   user-level backup that grabs the ciphertext blob cannot
//!   decrypt it on a restore to a different machine.
//!
//! ### What it does NOT protect against
//!
//! - A SYSTEM-context process on the same machine. DPAPI machine
//!   scope is by design accessible to any admin/SYSTEM caller.
//!   We are not pretending otherwise - this layer is purely
//!   defense-in-depth against offline disk theft and naive
//!   backup leak.
//!
//! ### File layout
//!
//! The ciphertext blobs live under
//! `<settings_dir>/secrets/<key>.dpapi` (note the `.dpapi`
//! extension to make their nature obvious to an admin inspecting
//! the directory). Atomic writes via tempfile + rename, same
//! pattern as `plaintext` and `warren_signer::set_warren_mnemonic`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Foundation::LocalFree;
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CryptProtectData, CryptUnprotectData,
};
use zeroize::Zeroizing;

use super::SecretStorage;

/// Sub-directory of `settings_dir` that holds DPAPI-encrypted
/// blobs. The plaintext fallback uses the same sub-directory with
/// a different extension (`.txt`), so we can detect inconsistent
/// state at boot (= both a `.dpapi` and a `.txt` for the same key).
const SECRETS_SUBDIR: &str = "secrets";

/// File extension for DPAPI ciphertext blobs.
const DPAPI_EXTENSION: &str = "dpapi";

/// Optional `entropy` passed to `CryptProtectData`. Tied to the
/// fixed string `"net.warrenvpn.daemon.identity"` - a second factor
/// that prevents another SYSTEM-context process from trivially
/// decrypting our blob unless they also know this constant. It is
/// **not** a secret (the binary is distributed), but it makes
/// cross-app leakage strictly harder than `CryptProtectData(blob,
/// NULL, ...)` against a generic DPAPI snoop.
const ENTROPY: &[u8] = b"net.warrenvpn.daemon.identity";

pub struct WindowsDpapiStorage {
    secrets_dir: PathBuf,
}

impl WindowsDpapiStorage {
    /// Probes that DPAPI is available by encrypting and decrypting
    /// a tiny test buffer. If this fails (= broken LSASS, missing
    /// machine key on a fresh sysprep image, etc.), the caller is
    /// expected to fall back to the plaintext backend rather than
    /// hard-failing the daemon.
    pub fn new(settings_dir: &Path) -> io::Result<Self> {
        // Probe round-trip. We do not persist anything to disk
        // here; this is purely a "can I call DPAPI at all" check.
        let probe = b"warren-dpapi-probe";
        let ciphertext = dpapi_protect(probe)?;
        let plaintext = dpapi_unprotect(&ciphertext)?;
        if plaintext.as_slice() != probe {
            return Err(io::Error::other(
                "DPAPI round-trip probe returned a different value than the input \
                 - refusing to use a broken DPAPI subsystem",
            ));
        }
        Ok(Self {
            secrets_dir: settings_dir.join(SECRETS_SUBDIR),
        })
    }

    fn key_path(&self, key: &str) -> io::Result<PathBuf> {
        // Runtime path-traversal guard shared with `plaintext.rs`.
        if !super::is_valid_storage_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("secret storage key {key:?} contains forbidden characters"),
            ));
        }
        Ok(self.secrets_dir.join(format!("{key}.{DPAPI_EXTENSION}")))
    }

    fn ensure_dir(&self) -> io::Result<()> {
        if !self.secrets_dir.exists() {
            fs::create_dir_all(&self.secrets_dir)?;
        }
        Ok(())
    }
}

impl SecretStorage for WindowsDpapiStorage {
    fn store(&self, key: &str, value: &[u8]) -> io::Result<()> {
        self.ensure_dir()?;
        let final_path = self.key_path(key)?;

        let ciphertext = dpapi_protect(value)?;

        // Atomic write: tempfile + rename, same pattern as
        // `plaintext` storage. We do not zeroize the plaintext
        // `value` here - the caller owns its lifecycle and is
        // expected to wrap secrets in `Zeroizing` upstream.
        let parent = final_path
            .parent()
            .ok_or_else(|| io::Error::other("dpapi storage path has no parent"))?;
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let file_name = final_path
            .file_name()
            .and_then(|os| os.to_str())
            .ok_or_else(|| io::Error::other("dpapi storage key has no file name"))?;
        let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{nanos}"));

        let _ = fs::remove_file(&tmp_path);

        {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            f.write_all(&ciphertext)?;
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
        let ciphertext = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let plaintext = dpapi_unprotect(&ciphertext)?;
        Ok(Some(plaintext))
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
        false
    }

    fn backend_name(&self) -> &'static str {
        "Windows DPAPI (CRYPTPROTECT_LOCAL_MACHINE)"
    }
}

/// Wraps `CryptProtectData` for byte buffers. Returns the ciphertext
/// owned by Rust (the `CRYPT_INTEGER_BLOB` returned by the API
/// allocates via `LocalAlloc`; we copy into a `Vec<u8>` and free
/// it before returning to keep the unsafe FFI surface tight).
fn dpapi_protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: all input pointers are valid for the duration of the
    // call (Rust borrow checker keeps `plaintext` and `ENTROPY` alive
    // until the call returns). The output `CRYPT_INTEGER_BLOB`
    // members are populated by `CryptProtectData` on success and we
    // immediately copy into a `Vec<u8>` + `LocalFree` the original
    // buffer before returning.
    unsafe {
        CryptProtectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut output,
        )
    }
    .map_err(|e| io::Error::other(format!("CryptProtectData failed: {e}")))?;

    // SAFETY: on Ok, `output.pbData` points to a `LocalAlloc`'d
    // buffer of `output.cbData` bytes that the caller MUST free
    // with `LocalFree`.
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
    }

    Ok(bytes)
}

/// Wraps `CryptUnprotectData`. The returned plaintext is wrapped
/// in `Zeroizing` so the heap buffer is wiped when dropped.
fn dpapi_unprotect(ciphertext: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: ciphertext.len() as u32,
        pbData: ciphertext.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: same contract as `dpapi_protect`.
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut output,
        )
    }
    .map_err(|e| io::Error::other(format!("CryptUnprotectData failed: {e}")))?;

    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // Wipe the LocalAlloc buffer ourselves before freeing, since
    // it transiently held the plaintext secret. `LocalFree` itself
    // does not zeroize.
    unsafe {
        std::ptr::write_bytes(output.pbData, 0u8, output.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
    }

    Ok(Zeroizing::new(bytes))
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
        let dir = std::env::temp_dir().join(format!("warren-dpapi-{pid}-{nanos}-{n}"));
        fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn store_then_load_round_trip() {
        let dir = isolated_tempdir();
        let storage = WindowsDpapiStorage::new(&dir).expect("DPAPI available in CI");

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
        let storage = WindowsDpapiStorage::new(&dir).expect("DPAPI available");
        let loaded = storage.load("never_set").expect("io ok");
        assert!(loaded.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ciphertext_on_disk_is_not_plaintext() {
        let dir = isolated_tempdir();
        let storage = WindowsDpapiStorage::new(&dir).expect("DPAPI available");
        let payload = b"this is a secret string";
        storage.store("k", payload).expect("store ok");

        let raw = fs::read(storage.key_path("k").expect("valid key")).expect("read raw blob");
        assert!(
            !raw.windows(payload.len()).any(|w| w == payload),
            "DPAPI ciphertext on disk MUST NOT contain the plaintext payload"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn entropy_is_required_for_unprotect() {
        // Encrypt with our entropy, then try to decrypt with a
        // different entropy by bypassing the helper. Confirms our
        // entropy actually participates in the binding.
        let dir = isolated_tempdir();
        let storage = WindowsDpapiStorage::new(&dir).expect("DPAPI available");
        let payload = b"sensitive";
        storage.store("k", payload).expect("store ok");

        let ciphertext = fs::read(storage.key_path("k")).expect("read blob");

        // Decrypt with wrong entropy: should error.
        let wrong_entropy = b"not-warren-entropy";
        let input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: wrong_entropy.len() as u32,
            pbData: wrong_entropy.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let result = unsafe {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut output,
            )
        };
        assert!(
            result.is_err(),
            "CryptUnprotectData with wrong entropy MUST fail"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
