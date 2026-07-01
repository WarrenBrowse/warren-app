//! Loads or generates the user's BIP39 mnemonic, derives it into an
//! Ed25519 [`SigningKey`] via [`warren_identity::derive_node_key`],
//! and wraps it in a shared
//! [`mullvad_api::warren_auth::WarrenAuthSigner`] via [`Arc`].
//!
//! ## Persistence layout (post-secret-storage refactor)
//!
//! The mnemonic is persisted via the [`crate::os_secret_storage`]
//! abstraction:
//!
//! - **macOS**: `/Library/Keychains/System.keychain` entry under
//!   service `"net.warrenvpn.daemon"`, account `"warren_mnemonic"`.
//! - **Windows**: DPAPI ciphertext blob at
//!   `<settings_dir>/secrets/warren_mnemonic.dpapi`.
//! - **Linux** (and macOS / Windows fallback when the OS-native
//!   store is unavailable): plaintext file at
//!   `<settings_dir>/secrets/warren_mnemonic.txt` with mode `0o600`
//!   root-only.
//!
//! ### Legacy compatibility
//!
//! The pre-refactor location was `<settings_dir>/warren_mnemonic.txt`
//! (in-place, no `secrets/` subdir). On boot, if the legacy file is
//! present, we migrate it to the active backend and remove it. See
//! [`migrate_legacy_mnemonic`].
//!
//! Dedicated module so we can wire/unwire Warren auth via a single
//! edit to the sole caller in `lib.rs`, and test the logic in
//! isolation (vs `lib.rs` which orchestrates all of boot and is
//! non-testable in unit).
//!
//! Error policy: we log and return `None` if the mnemonic is
//! inaccessible / corrupted. The boot continues in legacy Bearer
//! mode. This degradation will disappear once the chain is
//! 100% Warren (no Bearer fallback possible on the server side).

use std::io;
use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::SigningKey;
use mullvad_api::warren_auth::WarrenAuthSigner;
use zeroize::Zeroizing;

use crate::os_secret_storage::{SecretStorage, get_storage};

/// Legacy file name for the BIP39 mnemonic, used by versions of
/// Warren prior to the introduction of [`crate::os_secret_storage`].
/// Still referenced because [`migrate_legacy_mnemonic`] looks for it
/// at boot and because [`warren_identity::load_or_create_mnemonic`]
/// is paramaterized with this path during first-launch bootstrap.
pub const MNEMONIC_FILENAME: &str = "warren_mnemonic.txt";

/// Logical key under which the mnemonic is stored in the
/// [`SecretStorage`] backend.
const MNEMONIC_KEY: &str = "warren_mnemonic";

/// Loads or creates the BIP39 mnemonic in `settings_dir`, derives
/// it into an Ed25519 signing key, and returns a shared [`WarrenAuthSigner`].
///
/// Returns `None` (with a `log::warn!`) if the mnemonic cannot be
/// loaded or derived, to allow the daemon to continue in classic
/// Mullvad mode.
#[must_use]
pub fn load_or_create_signer(settings_dir: &Path) -> Option<Arc<WarrenAuthSigner>> {
    let signing_key = load_or_create_signing_key(settings_dir)?;
    Some(Arc::new(WarrenAuthSigner::new(signing_key)))
}

/// Loads or creates the BIP39 mnemonic in `settings_dir` and derives it
/// into the 32-byte pre-HKDF seed (the input to
/// `warren_identity::derive_node_key` / `WarrenIdentity::from_seed`).
///
/// Sibling of [`load_or_create_signing_key`], extracted so a caller that
/// needs the SDK's `warren_identity::WarrenIdentity` (which only builds
/// from this seed, never from an already-derived `SigningKey`) does not
/// have to re-implement the mnemonic-loading logic. Both functions derive
/// from the same on-disk mnemonic and therefore always agree.
///
/// **No-log policy**: NEVER log the returned seed. The caller must
/// consume it then drop it.
#[must_use]
pub fn load_or_create_seed(settings_dir: &Path) -> Option<Zeroizing<[u8; 32]>> {
    let storage = get_storage(settings_dir);
    if storage.is_plaintext() {
        log::warn!(
            "Warren mnemonic is stored in PLAINTEXT via {} (no OS secret store available): \
             back up {} carefully and exclude it from automated/cloud backups",
            storage.backend_name(),
            settings_dir.join("secrets").display()
        );
    }
    let mnemonic = load_or_create_mnemonic_via_storage(&*storage, settings_dir)?;

    match warren_identity::seed_from_mnemonic(&mnemonic) {
        Ok(seed) => Some(seed),
        Err(e) => {
            log::warn!(
                "Warren auth disabled: persisted mnemonic failed BIP39 validation \
                 ({e}) - backend={}",
                storage.backend_name()
            );
            None
        }
    }
}

/// Loads or creates the BIP39 mnemonic in `settings_dir` and derives
/// it into an Ed25519 [`SigningKey`], without the wrapper.
///
/// Sibling of [`load_or_create_signer`]: exposes the raw
/// cryptographic material needed to assemble
/// [`talpid_warren_tunnel::WarrenTunnelParameters`].
///
/// **No-log policy**: NEVER log the returned `SigningKey`
/// (see Warren rule). The caller must consume it then drop it.
#[must_use]
pub fn load_or_create_signing_key(settings_dir: &Path) -> Option<SigningKey> {
    load_or_create_seed(settings_dir).map(|seed| warren_identity::derive_node_key(&seed))
}

/// Migrates a legacy `<settings_dir>/warren_mnemonic.txt` (pre-secret
/// storage layout) into the active [`SecretStorage`] backend.
///
/// Behavior:
/// - Legacy file absent → no-op.
/// - Legacy present but storage already has an entry → assume storage
///   wins (= prior partial migration), delete legacy.
/// - Legacy present, storage empty → write to storage, then delete
///   legacy only after the write succeeded.
fn migrate_legacy_mnemonic(storage: &dyn SecretStorage, settings_dir: &Path) {
    let legacy_path = settings_dir.join(MNEMONIC_FILENAME);
    if !legacy_path.exists() {
        return;
    }

    let legacy_contents = match std::fs::read_to_string(&legacy_path) {
        Ok(s) => Zeroizing::new(s),
        Err(e) => {
            log::warn!(
                "legacy mnemonic file exists at {} but cannot be read ({e}); \
                 leaving alone",
                legacy_path.display()
            );
            return;
        }
    };

    if matches!(storage.load(MNEMONIC_KEY), Ok(Some(_))) {
        log::info!(
            "secret storage already has a mnemonic entry; removing redundant \
             legacy file at {}",
            legacy_path.display()
        );
        if let Err(e) = std::fs::remove_file(&legacy_path) {
            log::warn!("failed to remove redundant legacy mnemonic file: {e}");
        }
        return;
    }

    let trimmed = legacy_contents.trim();
    if let Err(e) = storage.store(MNEMONIC_KEY, trimmed.as_bytes()) {
        log::error!(
            "migration of legacy mnemonic to {} failed: {e} - keeping \
             legacy file as the only persistent copy",
            storage.backend_name()
        );
        return;
    }
    log::info!(
        "migrated legacy mnemonic to {} backend",
        storage.backend_name()
    );
    if let Err(e) = std::fs::remove_file(&legacy_path) {
        log::warn!("secret storage migration succeeded but legacy file removal failed: {e}");
    }
}

/// Loads the mnemonic from the active storage backend, or
/// bootstraps a fresh BIP39 mnemonic via
/// [`warren_identity::load_or_create_mnemonic`] when neither the
/// storage nor the legacy file holds one yet.
///
/// The return is wrapped in `Zeroizing` so the heap buffer is wiped
/// at the call site once the seed has been derived.
fn load_or_create_mnemonic_via_storage(
    storage: &dyn SecretStorage,
    settings_dir: &Path,
) -> Option<Zeroizing<String>> {
    // Migrate first so any subsequent `storage.load` sees the moved
    // entry instead of falling through to bootstrap.
    migrate_legacy_mnemonic(storage, settings_dir);

    match storage.load(MNEMONIC_KEY) {
        Ok(Some(bytes)) => match std::str::from_utf8(&bytes) {
            Ok(s) => Some(Zeroizing::new(s.trim().to_string())),
            Err(e) => {
                log::warn!(
                    "Warren auth disabled: secret storage returned non-UTF8 mnemonic \
                     ({e}) - backend={}",
                    storage.backend_name()
                );
                None
            }
        },
        Ok(None) => bootstrap_fresh_mnemonic(storage, settings_dir),
        Err(e) => {
            log::warn!(
                "Warren auth disabled: secret storage read failed ({e}) - backend={}",
                storage.backend_name()
            );
            None
        }
    }
}

/// First-launch bootstrap: generates a fresh BIP39 mnemonic **in
/// memory only** (never touching the legacy plaintext file path),
/// persists it via the storage backend, and returns it.
///
/// This deliberately bypasses `warren_identity::load_or_create_mnemonic`
/// because that helper unconditionally writes to its `path` argument -
/// which would mean creating a plaintext copy at the legacy path
/// every first launch, even on macOS / Windows where the active
/// backend stores in the System Keychain / DPAPI. Time Machine or
/// Windows Shadow Copy could snapshot that file during the brief
/// window between write and delete, defeating the
/// "excluded-from-backup" guarantee of the OS-native backend.
///
/// We use `bip39::Mnemonic::generate` directly (with the `rand`
/// feature) to produce a fresh 12-word mnemonic. The resulting
/// `Mnemonic` is `Zeroize + ZeroizeOnDrop` thanks to the `zeroize`
/// feature on `bip39`. The `to_string()` allocation is wrapped in
/// `Zeroizing<String>` immediately so the heap buffer is wiped on
/// scope exit. If the storage write fails, we surface the failure
/// and refuse to fall back to a plaintext legacy file - better to
/// disable Warren auth this boot than silently regress the security
/// posture.
fn bootstrap_fresh_mnemonic(
    storage: &dyn SecretStorage,
    _settings_dir: &Path,
) -> Option<Zeroizing<String>> {
    let raw_mnemonic = generate_fresh_mnemonic()
        .inspect_err(|e| log::warn!("Warren auth disabled: BIP39 mnemonic generation failed: {e}"))
        .ok()?;

    if let Err(e) = storage.store(MNEMONIC_KEY, raw_mnemonic.as_bytes()) {
        log::warn!(
            "Warren auth disabled: secret storage write failed at bootstrap \
             ({e}) - backend={}. Refusing to fall back to a plaintext legacy \
             file; the daemon will retry on next boot.",
            storage.backend_name()
        );
        return None;
    }
    Some(raw_mnemonic)
}

/// Restores (= overwrites) the user's BIP39 mnemonic. BIP39
/// validation is performed BEFORE any write (= atomic rejection
/// without disturbing the existing identity).
///
/// **Use case**: identity restore from the GUI (ImportMnemonicView).
/// The GUI caller MUST display a strong confirmation before calling,
/// because the previous identity (and the subscription tied to it)
/// is IRREVERSIBLY replaced. The caller in
/// `mullvad-daemon::on_set_warren_mnemonic` pairs this disk-write
/// with [`reload_signer_from_disk`] + an
/// `account_manager.login(new_pubkey)` so the new identity is
/// activated in the running daemon without requiring a restart.
///
/// # Errors
///
/// - `InvalidData` if `mnemonic` is not a valid BIP39 (checksum,
///   wordlist, count) → existing storage entry untouched.
/// - Other `io::Error` on storage write failures (Keychain, DPAPI,
///   filesystem).
///
/// # No-log policy
///
/// NEVER log `mnemonic`. Only log the fact that a write
/// succeeded/failed (= audit trail) and the backend name.
pub fn set_warren_mnemonic(settings_dir: &Path, mnemonic: &str) -> io::Result<()> {
    // Validate BIP39 before any write so a bad phrase never overwrites a good one.
    warren_identity::seed_from_mnemonic(mnemonic).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid BIP39 mnemonic: {e}"),
        )
    })?;

    let storage = get_storage(settings_dir);
    storage.store(MNEMONIC_KEY, mnemonic.trim().as_bytes())?;

    // Remove any leftover legacy file so we never keep two persistent copies
    // after a successful overwrite.
    let legacy_path = settings_dir.join(MNEMONIC_FILENAME);
    if legacy_path.exists()
        && let Err(e) = std::fs::remove_file(&legacy_path)
        && e.kind() != io::ErrorKind::NotFound
    {
        log::warn!(
            "set_warren_mnemonic stored via {} but failed to remove legacy \
             file at {}: {e}",
            storage.backend_name(),
            legacy_path.display()
        );
    }

    log::info!(
        "set_warren_mnemonic: identity overwritten via {} (content NEVER logged)",
        storage.backend_name()
    );
    Ok(())
}

/// Generates a fresh 12-word BIP39 mnemonic in memory.
///
/// The source `bip39::Mnemonic` holds the entropy and is
/// `ZeroizeOnDrop`; the returned `String` is wrapped in `Zeroizing`
/// so its heap buffer is wiped on drop. NEVER log the result.
fn generate_fresh_mnemonic() -> io::Result<Zeroizing<String>> {
    let mnemonic = bip39::Mnemonic::generate(12)
        .map_err(|e| io::Error::other(format!("BIP39 mnemonic generation failed: {e}")))?;
    Ok(Zeroizing::new(mnemonic.to_string()))
}

/// Removes any leftover legacy plaintext mnemonic file at the
/// pre-secret-storage location, so we never keep a second persistent
/// copy after a vault write/delete. A missing file is a no-op.
fn remove_legacy_mnemonic_file(settings_dir: &Path, storage: &dyn SecretStorage) {
    let legacy_path = settings_dir.join(MNEMONIC_FILENAME);
    if legacy_path.exists()
        && let Err(e) = std::fs::remove_file(&legacy_path)
        && e.kind() != io::ErrorKind::NotFound
    {
        log::warn!(
            "failed to remove legacy mnemonic file at {} (storage backend {}): {e}",
            legacy_path.display(),
            storage.backend_name()
        );
    }
}

/// Mints a brand-new identity: generates a fresh BIP39 mnemonic,
/// persists it via the active secret-storage backend (**overwriting**
/// any existing entry), removes any leftover legacy plaintext file,
/// and returns the new phrase.
///
/// **Use case**: GUI "Create a new account". The caller pairs this
/// with [`reload_signer_from_disk`] + an `account_manager.login(new_pubkey)`
/// to activate the identity without a restart, then shows the returned
/// phrase so the user can back it up before it is ever the only copy.
///
/// Unlike [`bootstrap_fresh_mnemonic`] (first-launch only, guarded by
/// the absence of any stored entry), this is the explicit user-driven
/// rotation path and always overwrites.
///
/// # No-log policy
///
/// NEVER log the returned phrase. Only the backend name is logged.
pub fn generate_and_store_mnemonic(settings_dir: &Path) -> io::Result<Zeroizing<String>> {
    let phrase = generate_fresh_mnemonic()?;
    let storage = get_storage(settings_dir);
    storage.store(MNEMONIC_KEY, phrase.as_bytes())?;
    remove_legacy_mnemonic_file(settings_dir, &*storage);
    log::info!(
        "generate_and_store_mnemonic: new identity minted via {} (content NEVER logged)",
        storage.backend_name()
    );
    Ok(phrase)
}

/// Neutralizes the in-memory signing identity after a true sign-out.
///
/// Replaces the live Ed25519 key in the shared [`WarrenAuthSigner`]
/// with an all-zero sentinel key. This serves two purposes:
/// 1. The previous key material is dropped and zeroized (the
///    `SigningKey` is `ZeroizeOnDrop`), so the just-erased identity no
///    longer lingers in process memory.
/// 2. Any signed request that happens to fire while logged out (e.g. a
///    background relay-list refresh) is signed by the sentinel key, not
///    the user's real (now-erased) identity - the server rejects it
///    rather than the daemon authenticating as the deleted account.
///
/// Pairs with [`clear_warren_mnemonic`] in the GUI logout path.
pub fn clear_signer(signer: &WarrenAuthSigner) {
    signer.replace_signing_key(SigningKey::from_bytes(&[0u8; 32]));
}

/// Erases the persisted BIP39 mnemonic from the active secret-storage
/// backend and deletes any leftover legacy plaintext file. Idempotent:
/// deleting an absent entry succeeds.
///
/// **Use case**: GUI logout = true sign-out. After this call the device
/// no longer holds the identity; the account (and any subscription
/// tied to it) is unrecoverable on this device unless the user backed
/// up the recovery phrase. The caller MUST gate this behind a strong
/// confirmation.
///
/// # No-log policy
///
/// Only the backend name is logged, never any content.
pub fn clear_warren_mnemonic(settings_dir: &Path) -> io::Result<()> {
    let storage = get_storage(settings_dir);
    storage.delete(MNEMONIC_KEY)?;
    remove_legacy_mnemonic_file(settings_dir, &*storage);
    log::info!(
        "clear_warren_mnemonic: identity erased from {} (true sign-out)",
        storage.backend_name()
    );
    Ok(())
}

/// Re-reads the persisted BIP39 mnemonic, derives the Ed25519
/// signing key, and swaps it into the shared [`WarrenAuthSigner`]
/// held by `mullvad-api`'s `RequestFactory`.
///
/// **Use case**: the GUI just called [`set_warren_mnemonic`] and the
/// daemon wants to activate the new identity without restarting the
/// process. After this call, every subsequent API request signed by
/// `signer` uses the freshly derived key.
///
/// Returns the new Ed25519 pubkey bytes on success (= the caller can
/// log the post-swap pubkey for audit and invoke
/// `account_manager.login(...)` against it), or `None` if the
/// mnemonic cannot be loaded or derived (the existing signing key
/// in `signer` is left intact in that case).
///
/// # No-log policy
///
/// NEVER log the mnemonic content nor the signing key. The returned
/// pubkey is public information and may be logged for audit.
#[must_use]
pub fn reload_signer_from_disk(signer: &WarrenAuthSigner, settings_dir: &Path) -> Option<[u8; 32]> {
    let signing_key = load_or_create_signing_key(settings_dir)?;
    let pubkey_bytes = *signing_key.verifying_key().as_bytes();
    signer.replace_signing_key(signing_key);
    Some(pubkey_bytes)
}

/// Reads the user's BIP39 mnemonic from the active storage backend.
///
/// Returns `None` if:
/// - the identity has never been bootstrapped (= empty storage and
///   no legacy file),
/// - the storage backend is inaccessible (= broken Keychain, missing
///   DPAPI, FS error).
///
/// Used by the gRPC `GetWarrenMnemonic` handler to allow the
/// Electron GUI to display the mnemonic in cleartext so the user can
/// back it up.
///
/// # No-log policy
///
/// The returned string is a cryptographic secret. The caller (gRPC
/// handler) must transmit it to the GUI then drop it. NEVER log the
/// content, even in debug.
#[must_use]
pub fn get_warren_mnemonic(settings_dir: &Path) -> Option<Zeroizing<String>> {
    let storage = get_storage(settings_dir);
    // Surface any pending migration so a freshly-installed daemon
    // reading legacy files via this code path also gets cleaned up.
    migrate_legacy_mnemonic(&*storage, settings_dir);

    let bytes = storage.load(MNEMONIC_KEY).ok().flatten()?;
    let s = std::str::from_utf8(&bytes).ok()?;
    Some(Zeroizing::new(s.trim().to_string()))
}

/// Decide whether `device.json` must be re-aligned to the active signer.
///
/// The BIP39-derived signer is the cryptographic root of truth: it signs
/// every API request and builds the tunnel params. `device.json` is only
/// a cache of the logged-in pubkey and can drift from it. Returns
/// `Some(expected)` when the device records a DIFFERENT logged-in pubkey
/// and must be re-logged-in to `expected`; returns `None` when they
/// already match OR when `stored_pubkey` is `None` (logged out / revoked),
/// which is a deliberate state that must be preserved (never auto-login a
/// user who logged out).
#[must_use]
pub fn reconcile_login_target<'a>(
    signer_pubkey_ss58: &'a str,
    stored_pubkey: Option<&str>,
) -> Option<&'a str> {
    match stored_pubkey {
        Some(stored) if stored != signer_pubkey_ss58 => Some(signer_pubkey_ss58),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconcile_noop_when_device_matches_signer() {
        assert_eq!(reconcile_login_target("wbSIGNER", Some("wbSIGNER")), None);
    }

    #[test]
    fn reconcile_targets_signer_when_device_differs() {
        assert_eq!(
            reconcile_login_target("wbSIGNER", Some("wbOTHER")),
            Some("wbSIGNER")
        );
    }

    #[test]
    fn reconcile_skips_logged_out_or_revoked_device() {
        // `device.pubkey()` is None for LoggedOut/Revoked. A deliberate
        // logout/revocation must NEVER be silently re-logged-in.
        assert_eq!(reconcile_login_target("wbSIGNER", None), None);
    }

    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-signer-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn load_or_create_signer_creates_mnemonic_on_first_call() {
        let dir = isolated_tempdir();

        let signer = load_or_create_signer(&dir).expect("must produce a signer on fresh boot");

        // The mnemonic must be readable via `get_warren_mnemonic`
        // after bootstrap (= persisted somewhere by the active
        // storage backend).
        assert!(
            get_warren_mnemonic(&dir).is_some(),
            "bootstrap must leave a retrievable mnemonic"
        );
        // The signer must produce a valid Warren SS58 address (`wb…`):
        let pk = signer.pubkey_ss58();
        assert!(pk.starts_with("wb"), "expected a wb… address, got {pk}");
        assert!(
            (47..=49).contains(&pk.len()),
            "unexpected SS58 length: {pk}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_signer_is_idempotent_across_calls() {
        let dir = isolated_tempdir();

        let s1 = load_or_create_signer(&dir).expect("first call");
        let s2 = load_or_create_signer(&dir).expect("second call");
        assert_eq!(
            s1.pubkey_ss58(),
            s2.pubkey_ss58(),
            "same settings_dir = same pubkey across boots"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_returns_none_when_no_file_exists() {
        let dir = isolated_tempdir();

        let result = get_warren_mnemonic(&dir);
        assert!(
            result.is_none(),
            "absent identity must yield None, not panic or create"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_returns_existing_mnemonic_after_persist() {
        let dir = isolated_tempdir();
        let _ = load_or_create_signer(&dir).expect("bootstrap signer");

        let mnemonic = get_warren_mnemonic(&dir).expect("must return Some after persist");
        let word_count = mnemonic.split_whitespace().count();
        assert!(
            word_count == 12 || word_count == 24,
            "BIP39 mnemonic should be 12 or 24 words, got {word_count}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_yields_deterministic_signing_key() {
        let dir = isolated_tempdir();
        let signing_key = load_or_create_signing_key(&dir).expect("bootstrap key");
        let pubkey_via_signer = hex::encode(signing_key.verifying_key().as_bytes());

        let mnemonic = get_warren_mnemonic(&dir).expect("mnemonic exported");
        let seed = warren_identity::seed_from_mnemonic(&mnemonic).expect("re-derive seed");
        let re_derived = warren_identity::derive_node_key(&seed);
        let pubkey_via_export = hex::encode(re_derived.verifying_key().as_bytes());

        assert_eq!(
            pubkey_via_signer, pubkey_via_export,
            "exported mnemonic MUST re-derive identical pubkey, else backup is broken"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_seed_agrees_with_load_or_create_signing_key() {
        // `load_or_create_signing_key` now delegates to `load_or_create_seed`
        // internally; this pins the observable contract (same on-disk
        // mnemonic -> the seed re-derives the exact same SigningKey via
        // `derive_node_key`) so a future refactor cannot silently desync the
        // two, which would break the SDK-backed `warren_api::WarrenApiClient`
        // callers that only have the seed.
        let dir = isolated_tempdir();

        let signing_key = load_or_create_signing_key(&dir).expect("bootstrap key");
        let seed = load_or_create_seed(&dir).expect("seed must be present after bootstrap");
        let re_derived = warren_identity::derive_node_key(&seed);

        assert_eq!(
            signing_key.verifying_key().to_bytes(),
            re_derived.verifying_key().to_bytes(),
            "load_or_create_seed MUST re-derive to the same key as load_or_create_signing_key"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_and_store_mnemonic_persists_a_valid_phrase() {
        let dir = isolated_tempdir();

        let phrase = generate_and_store_mnemonic(&dir).expect("must mint an identity");
        let word_count = phrase.split_whitespace().count();
        assert_eq!(
            word_count, 12,
            "a fresh Warren identity is a 12-word phrase"
        );

        // The minted phrase must be the one retrievable from the vault.
        let stored = get_warren_mnemonic(&dir).expect("vault holds the new phrase");
        assert_eq!(stored.as_str(), phrase.as_str());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_and_store_mnemonic_rotates_to_a_distinct_identity() {
        // Regression for the "Create a new account is a no-op" bug:
        // two successive creations MUST yield different identities.
        let dir = isolated_tempdir();

        let first = generate_and_store_mnemonic(&dir).expect("first create");
        let first_pk = {
            let seed = warren_identity::seed_from_mnemonic(&first).expect("seed 1");
            hex::encode(
                warren_identity::derive_node_key(&seed)
                    .verifying_key()
                    .as_bytes(),
            )
        };

        let second = generate_and_store_mnemonic(&dir).expect("second create");
        assert_ne!(
            first.as_str(),
            second.as_str(),
            "a new account must mint a fresh phrase, not reuse the old one"
        );
        let second_pk = {
            let seed = warren_identity::seed_from_mnemonic(&second).expect("seed 2");
            hex::encode(
                warren_identity::derive_node_key(&seed)
                    .verifying_key()
                    .as_bytes(),
            )
        };
        assert_ne!(
            first_pk, second_pk,
            "distinct phrases must yield distinct pubkeys"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_warren_mnemonic_erases_identity_and_is_idempotent() {
        let dir = isolated_tempdir();
        let _ = generate_and_store_mnemonic(&dir).expect("mint identity");
        assert!(
            get_warren_mnemonic(&dir).is_some(),
            "identity present before clear"
        );

        clear_warren_mnemonic(&dir).expect("clear must succeed");
        assert!(
            get_warren_mnemonic(&dir).is_none(),
            "vault must be empty after a true sign-out"
        );

        // Idempotent: clearing an already-empty vault is a no-op success.
        clear_warren_mnemonic(&dir).expect("second clear is a no-op success");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_warren_mnemonic_rejects_invalid_bip39() {
        let dir = isolated_tempdir();
        let bogus = "not a valid bip39 mnemonic at all";

        let result = set_warren_mnemonic(&dir, bogus);
        assert!(
            result.is_err(),
            "set_warren_mnemonic must reject non-BIP39 input"
        );
        assert_eq!(
            result.unwrap_err().kind(),
            io::ErrorKind::InvalidData,
            "BIP39 rejection must surface as InvalidData (= grpc InvalidArgument)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_warren_mnemonic_accepts_valid_bip39_and_persists() {
        let dir = isolated_tempdir();
        let valid = "abandon abandon abandon abandon abandon abandon \
                     abandon abandon abandon abandon abandon about";

        set_warren_mnemonic(&dir, valid).expect("valid BIP39 must succeed");
        let read_back = get_warren_mnemonic(&dir).expect("must be readable after set");
        assert_eq!(
            read_back.as_str(),
            valid,
            "round-trip set->get must preserve the mnemonic byte-exact"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_warren_mnemonic_overwrites_existing_identity() {
        let dir = isolated_tempdir();
        let original_signer = load_or_create_signer(&dir).expect("bootstrap original");
        let original_pubkey = original_signer.pubkey_ss58();

        let new_mnemonic = "abandon abandon abandon abandon abandon abandon \
                            abandon abandon abandon abandon abandon about";
        set_warren_mnemonic(&dir, new_mnemonic).expect("set new mnemonic");

        let new_key = load_or_create_signing_key(&dir).expect("new identity");
        let new_pubkey = hex::encode(new_key.verifying_key().as_bytes());

        assert_ne!(
            original_pubkey, new_pubkey,
            "after mnemonic replacement, loading the key from disk MUST return \
             the new pubkey so the G-3 identity-changed detection fires correctly"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mnemonic_derivation_is_consistent_with_load_or_create() {
        let dir = isolated_tempdir();
        let mnemonic = "abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";

        set_warren_mnemonic(&dir, mnemonic).expect("set mnemonic");

        let inline_key = warren_identity::seed_from_mnemonic(mnemonic)
            .map(|seed| warren_identity::derive_node_key(&seed))
            .expect("derivation from known-valid mnemonic must not fail");
        let inline_pubkey = inline_key.verifying_key().as_bytes().to_vec();

        let loaded_key = load_or_create_signing_key(&dir).expect("load key from storage");
        let loaded_pubkey = loaded_key.verifying_key().as_bytes().to_vec();

        assert_eq!(
            inline_pubkey, loaded_pubkey,
            "inline derivation (G-3 identity-changed detection) MUST produce \
             the same pubkey as load_or_create_signing_key (boot path)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_signer_from_disk_swaps_identity_in_place() {
        let dir = isolated_tempdir();

        let signer = load_or_create_signer(&dir).expect("bootstrap signer");
        let original_pubkey = signer.pubkey_ss58();

        let new_mnemonic = "abandon abandon abandon abandon abandon abandon \
                            abandon abandon abandon abandon abandon about";
        set_warren_mnemonic(&dir, new_mnemonic).expect("set new mnemonic");

        let new_pubkey_bytes =
            reload_signer_from_disk(&signer, &dir).expect("reload must succeed after set");
        let new_pubkey_ss58 = mullvad_api::warren_auth::ss58::encode(&new_pubkey_bytes);

        assert_eq!(
            signer.pubkey_ss58(),
            new_pubkey_ss58,
            "the shared signer must now report the post-swap pubkey"
        );
        assert_ne!(
            signer.pubkey_ss58(),
            original_pubkey,
            "the swap must actually change the identity"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_mnemonic_file_is_migrated_on_first_load() {
        // Simulate a pre-secret-storage installation: a plaintext
        // mnemonic file at the legacy path with no storage entry.
        let dir = isolated_tempdir();
        let legacy_path = dir.join(MNEMONIC_FILENAME);
        let mnemonic = "abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";
        std::fs::write(&legacy_path, format!("{mnemonic}\n")).expect("seed legacy");

        // First call triggers migration.
        let signer = load_or_create_signer(&dir).expect("loaded with migrated mnemonic");
        let pubkey = signer.pubkey_ss58();

        // Legacy file should be gone after migration (and we ignore
        // failure on platforms where it cannot be removed for some
        // reason - the data is now in the storage backend either way).
        assert!(
            !legacy_path.exists(),
            "legacy mnemonic file must be removed after successful migration"
        );

        // Subsequent load returns the same identity (= read from
        // storage, not from a re-bootstrap).
        let signer2 = load_or_create_signer(&dir).expect("second load");
        assert_eq!(
            pubkey,
            signer2.pubkey_ss58(),
            "migrated mnemonic must yield a stable identity across loads"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
