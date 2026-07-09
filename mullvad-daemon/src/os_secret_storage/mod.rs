//! OS-native secret storage abstraction.
//!
//! Wraps the platform-specific secret stores (System Keychain on
//! macOS, DPAPI machine scope on Windows) behind a unified trait so
//! that `warren_signer` can persist the BIP39 mnemonic without
//! shipping it as plaintext in `<settings_dir>`. On platforms where
//! no daemon-friendly OS-native store exists (= Linux, where
//! `secret-service` requires a session D-Bus the daemon does not
//! have), we explicitly fall back to a plaintext file with a loud
//! warning, instead of pretending to encrypt with a custom scheme -
//! the latter is exactly the Signal Desktop "BasicTextEncryption"
//! fiasco we want to avoid.
//!
//! ## Threat model improvements vs raw `0o600 root:root` plaintext
//!
//! - macOS: System Keychain is excluded from Time Machine backups
//!   by default, protecting against backup leak. Its file is
//!   encrypted at rest with a key derived from `/var/db/SystemKey`,
//!   which is also `0o600 root:root` - defending against offline
//!   disk theft on unencrypted volumes.
//! - Windows: DPAPI with `CRYPTPROTECT_LOCAL_MACHINE` ties the
//!   ciphertext to the machine's master key, surviving user logoff
//!   while still requiring local SYSTEM privileges to decrypt.
//!   Windows Backup excludes DPAPI master keys by default.
//! - Linux: no improvement (fallback is plaintext file, same as
//!   upstream Mullvad's `account.json`). Documented to the user via
//!   the `LinuxStorage::IS_PLAINTEXT` flag and a startup warning.
//!
//! ## What this trait is NOT
//!
//! It is **not** a full key-management system. The plaintext path
//! ([`SecretStorage::load`] return value) is exposed only inside the
//! daemon's secure boundary (root-owned process) and is always
//! wrapped in `zeroize::Zeroizing` by the caller. We do not attempt
//! to defend against an attacker who already has root on the machine
//! - that battle is lost upstream of this module.

use std::io;

use zeroize::Zeroizing;

/// Whitelists keys that can be safely embedded in a filesystem path
/// by the plaintext / DPAPI backends. Shared single source of truth so
/// macOS / Linux / Windows agree on what constitutes a valid key.
///
/// Accepts ASCII alphanumerics, `_`, and `-`. Rejects path
/// separators, traversal components (`.` / `..`), and everything
/// else by construction. This is stricter than strictly necessary
/// (`bip39_words.json` would not be a valid key, for example) but
/// matches our internal usage (`"warren_mnemonic"`) and provides
/// strong runtime guarantees against accidental path injection.
pub(crate) fn is_valid_storage_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

mod plaintext;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

/// Persistent OS-native secret storage. Implementations are
/// per-platform; the daemon picks one at startup via
/// [`get_storage`] and uses it for the lifetime of the process.
///
/// The byte slice arguments and return values are wrapped in
/// `Zeroizing` so callers cannot accidentally let secret material
/// linger on the heap after use.
pub trait SecretStorage: Send + Sync {
    /// Persist `value` under the logical `key`. Overwrites any
    /// existing entry under the same key atomically (the trait
    /// contract is "last write wins" - implementations MUST NOT
    /// surface a "conflict" error when overwriting).
    fn store(&self, key: &str, value: &[u8]) -> io::Result<()>;

    /// Read the value stored under `key`. Returns `Ok(None)` when
    /// the key does not exist (= first-launch on this machine);
    /// returns `Err` only on transient I/O or OS-store failures.
    fn load(&self, key: &str) -> io::Result<Option<Zeroizing<Vec<u8>>>>;

    /// Remove the value stored under `key`. A no-op if the key
    /// does not exist (idempotent).
    fn delete(&self, key: &str) -> io::Result<()>;

    /// Returns `true` when this backend stores secrets **in
    /// plaintext** on disk (= Linux fallback). The boot path uses
    /// this to surface a loud warning so the user knows their
    /// settings directory contains sensitive material in clear.
    fn is_plaintext(&self) -> bool;

    /// Short human-readable name of the backend, e.g. `"macOS
    /// System Keychain"`, `"Windows DPAPI (LocalMachine)"`,
    /// `"Linux plaintext file"`. Used in startup logs and the
    /// management interface for diagnostics.
    fn backend_name(&self) -> &'static str;
}

/// Returns the platform-appropriate secret storage backend, falling
/// back to the plaintext file implementation when the OS-native
/// backend cannot be constructed.
///
/// `settings_dir` is the daemon settings directory passed in from
/// `DaemonConfig`. It is needed by the plaintext fallback (Linux,
/// degraded macOS/Windows) and as the location of the DPAPI
/// ciphertext blob on Windows. The macOS Keychain implementation
/// ignores it (Keychain is its own file under `/Library/Keychains`).
///
/// ## Performance note
///
/// On macOS this constructs a fresh `MacOsKeychainStorage` and runs
/// the open-time write+delete probe each time. The cost is ~1ms per
/// call and is only paid on `set_warren_mnemonic` / `get_warren_mnemonic`
/// (user-triggered operations, rare) and at `WarrenIdentityManager::load`
/// (once per boot). Caching the backend on the `Daemon` struct is a
/// future optimization; it requires threading `&dyn SecretStorage`
/// through several public functions in `warren_signer` and is deferred
/// until that surface stabilizes.
/// Env var that, when set to any non-empty value, forces the
/// plaintext file backend regardless of OS-native availability.
///
/// **Why it exists**: on macOS, writing to `/Library/Keychains/System.keychain`
/// from an unsigned root binary triggers an Authorization Services
/// prompt ("warren-daemon souhaite utiliser le trousseau Système")
/// at **every** daemon launch, because the binary's code signature
/// hash changes on each `cargo build`. A stable Apple Developer ID
/// signature is required for the macOS Authorization database to
/// remember the grant - that is set up only on signed release
/// builds. In development this is unworkable, so we offer an opt-out
/// that keeps the dev loop friction-free.
///
/// **Set it in `scripts/dev/warren-dev.sh`** (or your shell profile)
/// when developing locally. On signed release builds, leave it unset
/// so the System Keychain backend is used as designed.
const PLAINTEXT_OVERRIDE_ENV: &str = "WARREN_USE_PLAINTEXT_STORAGE";

#[must_use]
pub fn get_storage(settings_dir: &std::path::Path) -> Box<dyn SecretStorage> {
    if std::env::var_os(PLAINTEXT_OVERRIDE_ENV).is_some_and(|v| !v.is_empty()) {
        // `get_storage` is called multiple times during a single
        // daemon boot (once per call site in `warren_signer`).
        // Logging the override notice
        // every time would drown the boot log in three or four
        // identical lines. `std::sync::Once` guarantees at most one
        // INFO line per process lifetime; subsequent calls take the
        // override silently.
        static ANNOUNCE: std::sync::Once = std::sync::Once::new();
        ANNOUNCE.call_once(|| {
            log::info!(
                "{PLAINTEXT_OVERRIDE_ENV} is set: using plaintext storage instead of \
                 the OS-native secret store. This is intended for development builds \
                 that lack a stable code signature - DO NOT use in production."
            );
        });
        return Box::new(plaintext::PlaintextStorage::new(settings_dir));
    }

    #[cfg(target_os = "macos")]
    {
        match macos::MacOsKeychainStorage::open() {
            Ok(s) => return Box::new(s),
            Err(e) => {
                log::warn!(
                    "macOS System Keychain unavailable ({e}); falling back to plaintext \
                     storage in settings_dir - backups of settings_dir will contain \
                     the BIP39 mnemonic in clear"
                );
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match windows::WindowsDpapiStorage::new(settings_dir) {
            Ok(s) => return Box::new(s),
            Err(e) => {
                log::warn!(
                    "Windows DPAPI unavailable ({e}); falling back to plaintext \
                     storage in settings_dir - backups of settings_dir will contain \
                     the BIP39 mnemonic in clear"
                );
            }
        }
    }

    Box::new(plaintext::PlaintextStorage::new(settings_dir))
}
