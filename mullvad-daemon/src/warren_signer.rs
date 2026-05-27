//! Loads or generates the user's BIP39 mnemonic from
//! `<settings_dir>/warren_mnemonic.txt`, derives it into an Ed25519
//! [`SigningKey`] via [`warren_identity::derive_node_key`], and wraps it
//! in a shared [`mullvad_api::warren_auth::WarrenAuthSigner`] via
//! [`Arc`].
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

use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::SigningKey;
use mullvad_api::warren_auth::WarrenAuthSigner;
use zeroize::Zeroizing;

/// Name of the file storing the user's BIP39 mnemonic in
/// `settings_dir`. Fixed convention: if we move this name later,
/// it will require a v15+ migration to rename the existing file.
pub const MNEMONIC_FILENAME: &str = "warren_mnemonic.txt";

/// Loads or creates the BIP39 mnemonic in `settings_dir`, derives
/// it into an Ed25519 signing key, and returns a shared [`WarrenAuthSigner`].
///
/// Returns `None` (with a `log::warn!`) if the mnemonic cannot be
/// loaded or derived, to allow the daemon to continue in classic
/// Mullvad mode during the Phase 2 transition.
#[must_use]
pub fn load_or_create_signer(settings_dir: &Path) -> Option<Arc<WarrenAuthSigner>> {
    let signing_key = load_or_create_signing_key(settings_dir)?;
    Some(Arc::new(WarrenAuthSigner::new(signing_key)))
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
    let mnemonic_path = settings_dir.join(MNEMONIC_FILENAME);
    // `warren_identity::load_or_create_mnemonic` returns a raw `String`.
    // Wrap it in `Zeroizing` immediately so the heap buffer is wiped
    // when this scope ends, even if subsequent calls panic or return
    // early. `seed_from_mnemonic` already returns `Zeroizing<[u8; 32]>`
    // for the derived seed.
    let mnemonic = match warren_identity::load_or_create_mnemonic(&mnemonic_path) {
        Ok(m) => Zeroizing::new(m),
        Err(e) => {
            log::warn!(
                "Warren auth disabled: failed to load/create mnemonic at {}: {}",
                mnemonic_path.display(),
                e
            );
            return None;
        }
    };
    let seed = match warren_identity::seed_from_mnemonic(&mnemonic) {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "Warren auth disabled: invalid BIP39 mnemonic at {}: {}",
                mnemonic_path.display(),
                e
            );
            return None;
        }
    };
    Some(warren_identity::derive_node_key(&seed))
}

/// Restores (= overwrites) the user's BIP39 mnemonic in
/// `<settings_dir>/warren_mnemonic.txt`. BIP39 validation is performed
/// BEFORE any write to disk (= atomic rejection without corrupting
/// the existing file).
///
/// **Atomicity**: writes first to a sibling tempfile (mode 0o600
/// on Unix, sync_all before close), then atomic `rename` to the final
/// path (POSIX rename silently replaces the destination).
/// In case of a crash between tempfile and rename, the old mnemonic
/// remains intact.
///
/// **Use case**: identity restore from the GUI (= C.1.d ImportMnemonicView).
/// The GUI caller MUST display a strong confirmation before calling,
/// because the previous identity (and the subscription tied to it)
/// is IRREVERSIBLY replaced. The caller in `mullvad-daemon::on_set_warren_mnemonic`
/// pairs this disk-write with [`reload_signer_from_disk`] + an
/// `account_manager.login(new_pubkey)` so the new identity is
/// activated in the running daemon without requiring a restart.
///
/// # Errors
///
/// - `InvalidData` if `mnemonic` is not a valid BIP39 (checksum,
///   wordlist, count) -> existing file untouched.
/// - Other `io::Error` on tempfile/rename (perms, FS full, etc.).
///
/// # No-log policy
///
/// NEVER log `mnemonic`. Only log the fact that a write
/// succeeded/failed (= audit trail).
pub fn set_warren_mnemonic(settings_dir: &Path, mnemonic: &str) -> std::io::Result<()> {
    use std::io::Write;

    // Step 1 — BIP39 validation BEFORE any write.
    warren_identity::seed_from_mnemonic(mnemonic).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid BIP39 mnemonic: {e}"),
        )
    })?;

    // Step 2 — prepare a unique sibling tempfile.
    let path = settings_dir.join(MNEMONIC_FILENAME);
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{MNEMONIC_FILENAME}.tmp.{pid}.{nanos}"));

    // Best-effort cleanup of a possible leftover from a previous crash.
    let _ = std::fs::remove_file(&tmp_path);

    // Step 3 — atomic write into the tempfile.
    {
        #[cfg(unix)]
        let mut f = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp_path)?
        };
        #[cfg(not(unix))]
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;

        f.write_all(mnemonic.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }

    // Step 4 — atomic rename to the destination (overwrite OK).
    // Contrast with write_mnemonic_file from warren-identity which uses
    // hard_link for fail-on-exists (= load_or_create semantics). Here
    // we WANT to replace.
    match std::fs::rename(&tmp_path, &path) {
        Ok(()) => {
            log::info!("set_warren_mnemonic: identity overwritten (content NEVER logged)");
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

/// Re-reads the persisted BIP39 mnemonic from `settings_dir`, derives
/// the Ed25519 signing key, and swaps it into the shared
/// [`WarrenAuthSigner`] held by `mullvad-api`'s `RequestFactory`.
///
/// **Use case**: the GUI just called [`set_warren_mnemonic`] and the
/// daemon wants to activate the new identity without restarting the
/// process. After this call, every subsequent API request signed by
/// `signer` uses the freshly derived key.
///
/// Returns the new Ed25519 pubkey bytes on success (= the caller can
/// log the post-swap pubkey for audit and invoke
/// `account_manager.login(...)` against it), or `None` if the
/// mnemonic file cannot be loaded or derived (the existing signing
/// key in `signer` is left intact in that case).
///
/// # No-log policy
///
/// NEVER log the mnemonic content nor the signing key. The returned
/// pubkey is public information and may be logged for audit.
#[must_use]
pub fn reload_signer_from_disk(
    signer: &WarrenAuthSigner,
    settings_dir: &Path,
) -> Option<[u8; 32]> {
    let signing_key = load_or_create_signing_key(settings_dir)?;
    let pubkey_bytes = *signing_key.verifying_key().as_bytes();
    signer.replace_signing_key(signing_key);
    Some(pubkey_bytes)
}

/// Reads the **already persisted** user's BIP39 mnemonic in
/// `<settings_dir>/warren_mnemonic.txt`. Read-only: never creates
/// the file (contrast with [`load_or_create_signing_key`]).
///
/// Returns `None` if:
/// - the file does not exist (= identity never bootstrapped),
/// - the file is inaccessible (broken perms, FS error).
///
/// Used by the gRPC `GetWarrenMnemonic` (C.1) handler to
/// allow the Electron GUI to display the mnemonic in cleartext so
/// the user can back it up (= phase 1 criterion #2 "BIP39 mnemonic
/// displayed once and restorable").
///
/// # No-log policy
///
/// The returned string is a cryptographic secret. The caller
/// (gRPC handler) must transmit it to the GUI then drop it.
/// NEVER log the content, even in debug. The fact *that a read
/// occurred* may be logged (= GUI requests audit trail), but
/// never the content.
#[must_use]
pub fn get_warren_mnemonic(settings_dir: &Path) -> Option<Zeroizing<String>> {
    let path = settings_dir.join(MNEMONIC_FILENAME);
    // `read_to_string` allocates a `String` that will be dropped at
    // the end of this expression once we have built the trimmed
    // `Zeroizing<String>`. To avoid the untrimmed copy lingering on
    // the heap, we wrap it in `Zeroizing` immediately on success.
    std::fs::read_to_string(&path)
        .ok()
        .map(Zeroizing::new)
        .map(|raw| Zeroizing::new(raw.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Utility: an isolated temporary directory for each test.
    /// No `tempfile` nor `uuid` in the daemon deps, so we
    /// compose with `pid + timestamp_nanos + counter` (sufficient
    /// to avoid collisions between parallel
    /// `--test-threads` tests).
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
        // On the daemon's first boot (= empty settings_dir),
        // the function must generate a new BIP39 mnemonic,
        // write it to disk, and return a valid signer.
        let dir = isolated_tempdir();
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "preconditions: no existing mnemonic"
        );

        let signer = load_or_create_signer(&dir).expect("must produce a signer on fresh boot");

        // The file must have been created:
        assert!(
            dir.join(MNEMONIC_FILENAME).exists(),
            "warren_mnemonic.txt must be created"
        );
        // The signer must produce a valid pubkey (= 64 hex chars):
        assert_eq!(signer.pubkey_hex().len(), 64);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_signer_is_idempotent_across_calls() {
        // On daemon reboot, the same mnemonic must
        // produce the same pubkey (= stable user identity).
        let dir = isolated_tempdir();

        let s1 = load_or_create_signer(&dir).expect("first call");
        let s2 = load_or_create_signer(&dir).expect("second call");
        assert_eq!(
            s1.pubkey_hex(),
            s2.pubkey_hex(),
            "same settings_dir = same pubkey across boots"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_returns_none_when_no_file_exists() {
        // On first boot, before load_or_create_signer, the file
        // does not exist -> the function must return None without
        // panicking nor creating a file (read-only get).
        let dir = isolated_tempdir();
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "preconditions: no mnemonic"
        );

        let result = get_warren_mnemonic(&dir);
        assert!(
            result.is_none(),
            "absent file must yield None, not panic or create"
        );
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "get_warren_mnemonic must NEVER create the file (read-only)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_returns_existing_mnemonic_after_persist() {
        // After load_or_create_signer (= identity bootstrap), the
        // BIP39 mnemonic must be readable via get_warren_mnemonic
        // and contain 12 or 24 words (= standard BIP39 cardinal).
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
        // Critical cross-function invariant: the mnemonic returned
        // by get_warren_mnemonic, re-derived via warren_identity::
        // {seed_from_mnemonic, derive_node_key}, must produce
        // EXACTLY the same pubkey as load_or_create_signing_key.
        // Otherwise the user who exports their mnemonic for backup
        // ends up with a different identity on restore (= loss
        // of subscription -> phase 1 criterion #2 blocker).
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
    fn set_warren_mnemonic_rejects_invalid_bip39() {
        // A string that is not a valid BIP39 mnemonic
        // (bad checksum, unknown word, etc.) must be REJECTED before
        // writing to disk, otherwise we corrupt the user identity.
        let dir = isolated_tempdir();
        let bogus = "this is not a valid bip39 mnemonic at all";

        let result = set_warren_mnemonic(&dir, bogus);
        assert!(
            result.is_err(),
            "set_warren_mnemonic must reject non-BIP39 input"
        );
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "rejected input must NOT persist (atomicity = no half-write)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_warren_mnemonic_accepts_valid_bip39_and_persists() {
        // A valid BIP39 mnemonic must be written to disk
        // AND be readable via get_warren_mnemonic right after.
        let dir = isolated_tempdir();
        // Fixed 12-word BIP39 mnemonic (= known test vector).
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
        // Restore use case: the identity already exists (= load_or_create_signer
        // has run). set_warren_mnemonic must OVERWRITE this identity with the
        // new one. The GUI caller must display a strong confirmation
        // because this operation is IRREVERSIBLE (= subscription tied to
        // the previous identity = lost).
        let dir = isolated_tempdir();
        let original_signer = load_or_create_signer(&dir).expect("bootstrap original");
        let original_pubkey = original_signer.pubkey_hex();

        let new_mnemonic = "abandon abandon abandon abandon abandon abandon \
                            abandon abandon abandon abandon abandon about";
        set_warren_mnemonic(&dir, new_mnemonic).expect("restore must succeed");

        let new_signer = load_or_create_signer(&dir).expect("re-bootstrap");
        let new_pubkey = new_signer.pubkey_hex();
        assert_ne!(
            original_pubkey, new_pubkey,
            "after restore, pubkey MUST differ (= identity overwritten)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_signer_returns_none_on_corrupt_mnemonic() {
        // If the mnemonic file exists but
        // contains corrupted data (= not a valid BIP39
        // mnemonic), we log and return None rather than
        // crashing the daemon.
        let dir = isolated_tempdir();
        std::fs::write(
            dir.join(MNEMONIC_FILENAME),
            "this is not a valid bip39 mnemonic",
        )
        .expect("write corrupt file");

        let signer = load_or_create_signer(&dir);
        assert!(signer.is_none(), "corruption must -> None, not panic");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // G-3: mnemonic import must invalidate device.json when pubkey changes
    // ------------------------------------------------------------------

    /// Two distinct valid BIP39 mnemonics MUST derive DIFFERENT Ed25519
    /// signing keys. This is the foundational invariant that makes the
    /// G-3 identity-changed detection correct: if it were false, a
    /// mnemonic swap would be silently ignored and device.json would
    /// never be invalidated.
    #[test]
    fn different_mnemonics_produce_different_signing_keys() {
        let mnemonic_a = "abandon abandon abandon abandon abandon abandon \
                          abandon abandon abandon abandon abandon about";
        let mnemonic_b = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

        let seed_a = warren_identity::seed_from_mnemonic(mnemonic_a)
            .expect("mnemonic_a must be a valid BIP39");
        let seed_b = warren_identity::seed_from_mnemonic(mnemonic_b)
            .expect("mnemonic_b must be a valid BIP39");

        let key_a = warren_identity::derive_node_key(&seed_a);
        let key_b = warren_identity::derive_node_key(&seed_b);

        let pubkey_a = key_a.verifying_key().as_bytes().to_vec();
        let pubkey_b = key_b.verifying_key().as_bytes().to_vec();

        assert_ne!(
            pubkey_a, pubkey_b,
            "distinct mnemonics MUST produce distinct Ed25519 signing keys; \
             if equal the G-3 identity-changed detection would be a no-op"
        );
    }

    /// After `set_warren_mnemonic` replaces the identity, loading the
    /// signing key from the updated file MUST yield the pubkey that
    /// corresponds to the NEW mnemonic — not the original one.
    ///
    /// This proves that the on-disk state is consistent with the new
    /// identity before the daemon restarts, which is the post-condition
    /// that the G-3 async logout relies on.
    #[test]
    fn set_warren_mnemonic_then_load_key_gives_new_pubkey() {
        let dir = isolated_tempdir();

        // Bootstrap an original identity.
        let original_key = load_or_create_signing_key(&dir).expect("original identity");
        let original_pubkey = original_key.verifying_key().as_bytes().to_vec();

        // Replace the mnemonic.
        let new_mnemonic = "abandon abandon abandon abandon abandon abandon \
                            abandon abandon abandon abandon abandon about";
        set_warren_mnemonic(&dir, new_mnemonic).expect("set new mnemonic");

        // The new signing key loaded from disk must differ from the original.
        let new_key = load_or_create_signing_key(&dir).expect("new identity");
        let new_pubkey = new_key.verifying_key().as_bytes().to_vec();

        assert_ne!(
            original_pubkey, new_pubkey,
            "after mnemonic replacement, loading the key from disk MUST return \
             the new pubkey so the G-3 identity-changed detection fires correctly"
        );
    }

    /// The identity-changed detection logic used in
    /// `on_set_warren_mnemonic` (G-3 fix) compares the pubkey derived
    /// from the new mnemonic against the pubkey stored in device.json.
    /// This test verifies that the derived pubkey matches what
    /// `load_or_create_signing_key` would return — proving the two
    /// derivation paths are consistent.
    #[test]
    fn mnemonic_derivation_is_consistent_with_load_or_create() {
        let dir = isolated_tempdir();
        let mnemonic = "abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";

        // Write the mnemonic to disk.
        set_warren_mnemonic(&dir, mnemonic).expect("set mnemonic");

        // Derive the key the same way on_set_warren_mnemonic does at runtime.
        let inline_key = warren_identity::seed_from_mnemonic(mnemonic)
            .map(|seed| warren_identity::derive_node_key(&seed))
            .expect("derivation from known-valid mnemonic must not fail");
        let inline_pubkey = inline_key.verifying_key().as_bytes().to_vec();

        // Derive the key via the boot path.
        let loaded_key = load_or_create_signing_key(&dir).expect("load key from disk");
        let loaded_pubkey = loaded_key.verifying_key().as_bytes().to_vec();

        assert_eq!(
            inline_pubkey, loaded_pubkey,
            "inline derivation (G-3 identity-changed detection) MUST produce \
             the same pubkey as load_or_create_signing_key (boot path)"
        );
    }

    /// Hot-swap contract: after `set_warren_mnemonic` + `reload_signer_from_disk`,
    /// the existing `Arc<WarrenAuthSigner>` shared by `mullvad-api` must
    /// sign new requests with the freshly derived key, without being
    /// dropped or recreated. This is what removes the need for a
    /// daemon restart on identity import.
    #[test]
    fn reload_signer_from_disk_swaps_identity_in_place() {
        let dir = isolated_tempdir();

        let signer = load_or_create_signer(&dir).expect("bootstrap signer");
        let original_pubkey = signer.pubkey_hex();

        let new_mnemonic = "abandon abandon abandon abandon abandon abandon \
                            abandon abandon abandon abandon abandon about";
        set_warren_mnemonic(&dir, new_mnemonic).expect("set new mnemonic");

        let new_pubkey_bytes =
            reload_signer_from_disk(&signer, &dir).expect("reload must succeed after set");
        let new_pubkey_hex = hex::encode(new_pubkey_bytes);

        assert_eq!(
            signer.pubkey_hex(),
            new_pubkey_hex,
            "the shared signer must now report the post-swap pubkey"
        );
        assert_ne!(
            signer.pubkey_hex(),
            original_pubkey,
            "the swap must actually change the identity"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
