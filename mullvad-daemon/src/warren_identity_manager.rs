//! Single source of truth for the daemon's Warren identity.
//!
//! The daemon has two live views of the identity: the derived Ed25519
//! `SigningKey` inside the shared [`WarrenAuthSigner`] (REST signing,
//! tunnel RPK identity, forum SSO) and the pre-HKDF BIP39 seed handle
//! backing the SDK `WarrenApiClient` (account backend: purchases,
//! subscription balance, incident reports). The SDK's `WarrenIdentity`
//! only builds from the pre-HKDF seed, never from an already-derived
//! `SigningKey` (re-running HKDF on key bytes would mint a different
//! identity), so the two views cannot share one storage cell and must
//! be swapped together.
//!
//! Two invariants, both load-bearing for billing (a violation credits
//! paid vouchers to a wallet the user does not own):
//! - Both views are always rewritten by the single internal
//!   [`swap_to`](Self::swap_to) from ONE seed value; the daemon flows
//!   have no other mutation surface.
//! - The manager never mints an identity. Until the user explicitly
//!   creates or imports one in onboarding, both views hold the fixed
//!   placeholder sentinel, which the server refuses to authenticate or
//!   register.

use std::io;
use std::path::Path;
use std::sync::Arc;

use mullvad_api::warren_auth::WarrenAuthSigner;
use zeroize::Zeroizing;

use crate::warren_sdk_client::{SharedWarrenSeed, sentinel_seed};
use crate::warren_signer;

/// Owns the daemon-wide Warren identity and every hot-swap of it.
pub struct WarrenIdentityManager {
    signer: Arc<WarrenAuthSigner>,
    seed: SharedWarrenSeed,
}

impl WarrenIdentityManager {
    /// Boot constructor: loads the persisted BIP39 mnemonic in
    /// `settings_dir`, if any, and builds both identity views from the
    /// same seed value, in one read. When no identity has been created
    /// yet (or the storage backend fails, logged by `load_seed`), both
    /// views hold the placeholder sentinel until onboarding mints or
    /// imports a real one.
    #[must_use]
    pub fn load(settings_dir: &Path) -> Self {
        let seed = warren_signer::load_seed(settings_dir).unwrap_or_else(sentinel_seed);
        let signing_key = warren_identity::derive_node_key(&seed);
        Self {
            signer: Arc::new(WarrenAuthSigner::new(signing_key)),
            seed: Arc::new(std::sync::RwLock::new(seed)),
        }
    }

    /// The shared signer consumed by the REST factory, the tunnel and
    /// the forum SSO. Hot-swaps through this manager propagate to
    /// every clone.
    #[must_use]
    pub fn signer(&self) -> Arc<WarrenAuthSigner> {
        self.signer.clone()
    }

    /// The shared seed handle consumed by the SDK-backed clients
    /// (account backend, incident reporters). Hot-swaps through this
    /// manager propagate to every clone.
    #[must_use]
    pub fn seed_handle(&self) -> SharedWarrenSeed {
        self.seed.clone()
    }

    /// SS58 address of the identity both views currently hold.
    #[must_use]
    pub fn pubkey_ss58(&self) -> String {
        self.signer.pubkey_ss58()
    }

    /// The one and only swap: rewrites BOTH views from a single seed
    /// value and returns the new public key. Private so no caller can
    /// ever update one view without the other.
    fn swap_to(&self, seed: Zeroizing<[u8; 32]>) -> [u8; 32] {
        let signing_key = warren_identity::derive_node_key(&seed);
        let pubkey = signing_key.verifying_key().to_bytes();
        *self.seed.write().expect("warren seed RwLock poisoned") = seed;
        self.signer.replace_signing_key(signing_key);
        pubkey
    }

    /// Re-reads the persisted mnemonic (just written by
    /// `warren_signer::set_warren_mnemonic`) and swaps both views to
    /// it. Returns the new public key, or `None` when the mnemonic
    /// cannot be loaded or derived, in which case NEITHER view was
    /// touched (no partial swap).
    #[must_use]
    pub fn reload_from_disk(&self, settings_dir: &Path) -> Option<[u8; 32]> {
        let seed = warren_signer::load_seed(settings_dir)?;
        Some(self.swap_to(seed))
    }

    /// Mints a brand-new identity (create-account flow): generates and
    /// persists a fresh mnemonic, then swaps both views to it.
    ///
    /// # Errors
    /// Propagates mnemonic generation/persistence failures; both views
    /// keep the previous identity in that case.
    pub fn mint_new_identity(&self, settings_dir: &Path) -> io::Result<[u8; 32]> {
        let phrase = warren_signer::generate_and_store_mnemonic(settings_dir)?;
        let seed = warren_identity::seed_from_mnemonic(&phrase)
            .map_err(|e| io::Error::other(format!("freshly generated mnemonic invalid: {e}")))?;
        Ok(self.swap_to(seed))
    }

    /// Neutralizes the identity after a true sign-out: both views swap
    /// to the fixed, publicly-known sentinel seed. Any request that
    /// fires afterwards is signed by the sentinel, which the server
    /// must never honor (it holds no subscription and warren-api
    /// rejects it outright), instead of the just-erased account.
    pub fn clear(&self) {
        let _ = self.swap_to(sentinel_seed());
    }

    /// True once the user has created or imported an identity; false
    /// while both views still hold the placeholder sentinel. Gates
    /// every flow that must not run as the sentinel (boot reconcile,
    /// forum SSO signing).
    #[must_use]
    pub fn has_user_identity(&self) -> bool {
        *self.seed.read().expect("warren seed RwLock poisoned") != sentinel_seed()
    }

    /// True when both views derive from the same seed. The manager
    /// keeps this invariant by construction; exposed so boot and tests
    /// can assert it cheaply.
    #[must_use]
    pub fn is_coherent(&self) -> bool {
        let derived = {
            let seed = self.seed.read().expect("warren seed RwLock poisoned");
            warren_identity::derive_node_key(&seed)
        };
        mullvad_api::warren_auth::ss58::encode(&derived.verifying_key().to_bytes())
            == self.signer.pubkey_ss58()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warren_identity::WarrenIdentity;

    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-idmgr-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    fn sdk_address(manager: &WarrenIdentityManager) -> String {
        let seed = manager.seed_handle();
        let guard = seed.read().expect("seed lock");
        WarrenIdentity::from_seed(&guard).address()
    }

    #[test]
    fn load_on_fresh_storage_is_the_placeholder_and_mints_nothing() {
        // The exact moment the user first sees their recovery phrase in
        // onboarding must also be the moment the identity comes into
        // existence: an identity minted at boot is a wallet the user
        // cannot back up, and money paid onto it is lost.
        let dir = isolated_tempdir();

        let manager = WarrenIdentityManager::load(&dir);

        let sentinel_address = WarrenIdentity::from_seed(&sentinel_seed()).address();
        assert_eq!(
            manager.pubkey_ss58(),
            sentinel_address,
            "an un-onboarded daemon must run as the placeholder sentinel"
        );
        assert!(!manager.has_user_identity());
        assert!(
            warren_signer::get_warren_mnemonic(&dir).is_none(),
            "loading must not persist any identity"
        );
        assert!(manager.is_coherent());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_builds_both_views_from_the_persisted_seed() {
        let dir = isolated_tempdir();
        let _ = warren_signer::generate_and_store_mnemonic(&dir).expect("mint identity");

        let manager = WarrenIdentityManager::load(&dir);

        assert_eq!(
            manager.pubkey_ss58(),
            sdk_address(&manager),
            "the signer and the SDK seed must describe the same identity at boot"
        );
        assert!(manager.has_user_identity());
        assert!(manager.is_coherent());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mint_new_identity_keeps_signer_and_sdk_seed_in_lockstep() {
        // If a mint swapped the signer without the SDK seed, purchases
        // would be registered (and the balance displayed) under a wallet
        // the user does not own.
        let dir = isolated_tempdir();
        let manager = WarrenIdentityManager::load(&dir);
        let placeholder_address = manager.pubkey_ss58();

        let new_pubkey = manager.mint_new_identity(&dir).expect("mint");

        let new_address = mullvad_api::warren_auth::ss58::encode(&new_pubkey);
        assert_eq!(
            manager.pubkey_ss58(),
            new_address,
            "the signer must be on the freshly minted identity"
        );
        assert_eq!(
            sdk_address(&manager),
            new_address,
            "the SDK seed must follow the mint: a purchase right after \
             create-account must register under the displayed wallet"
        );
        assert_ne!(
            new_address, placeholder_address,
            "mint must leave the placeholder"
        );
        assert!(manager.has_user_identity());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_from_disk_swaps_both_views_to_the_imported_identity() {
        let dir = isolated_tempdir();
        let manager = WarrenIdentityManager::load(&dir);
        let mnemonic = "abandon abandon abandon abandon abandon abandon \
                        abandon abandon abandon abandon abandon about";
        warren_signer::set_warren_mnemonic(&dir, mnemonic).expect("persist import");

        let new_pubkey = manager
            .reload_from_disk(&dir)
            .expect("reload after a successful import");

        let expected = WarrenIdentity::from_mnemonic(mnemonic)
            .expect("valid phrase")
            .address();
        assert_eq!(
            mullvad_api::warren_auth::ss58::encode(&new_pubkey),
            expected
        );
        assert_eq!(manager.pubkey_ss58(), expected);
        assert_eq!(
            sdk_address(&manager),
            expected,
            "the SDK seed must follow a restore"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_swaps_both_views_to_the_sentinel() {
        let dir = isolated_tempdir();
        let manager = WarrenIdentityManager::load(&dir);
        let user_address = mullvad_api::warren_auth::ss58::encode(
            &manager.mint_new_identity(&dir).expect("mint identity"),
        );

        manager.clear();

        let sentinel_address = WarrenIdentity::from_seed(&sentinel_seed()).address();
        assert_ne!(
            manager.pubkey_ss58(),
            user_address,
            "logout must stop signing as the user's identity"
        );
        assert_eq!(manager.pubkey_ss58(), sentinel_address);
        assert_eq!(
            sdk_address(&manager),
            sentinel_address,
            "the SDK seed must be neutralized together with the signer"
        );
        assert!(!manager.has_user_identity());
        assert!(manager.is_coherent());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
