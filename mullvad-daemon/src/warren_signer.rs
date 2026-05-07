//! Warren fork — Phase 2.A.4 V4 : helper qui charge ou génère la
//! mnémonique BIP39 utilisateur depuis `<settings_dir>/warren_mnemonic.txt`,
//! la dérive en [`SigningKey`] Ed25519 via
//! [`warren_identity::derive_node_key`], et la wrap dans un
//! [`mullvad_api::warren_auth::WarrenAuthSigner`] partagé via [`Arc`].
//!
//! **Pourquoi un module dédié** : la fonction est testable en
//! isolation (vs `mullvad_daemon::lib.rs` qui orchestre tout le boot
//! et est non-testable en unit). Permet aussi de wire/unwire la
//! Warren auth via une seule modif de l'unique caller dans `lib.rs`.
//!
//! **Politique d'erreur** : on log et on retourne `None` si la
//! mnémonique est inaccessible / corrompue. Le boot continue en mode
//! Mullvad Bearer historique. La désactivation totale de cette
//! dégradation viendra en Phase 2.D quand toute la chaîne sera 100%
//! Warren (= aucun fallback Bearer possible côté serveur).

use std::path::Path;
use std::sync::Arc;

use mullvad_api::warren_auth::WarrenAuthSigner;

/// Nom du fichier qui stocke la mnémonique BIP39 utilisateur dans
/// `settings_dir`. Convention figée : si on bouge ce nom plus tard,
/// il faudra une migration v15+ qui renomme l'existant.
pub const MNEMONIC_FILENAME: &str = "warren_mnemonic.txt";

/// Charge ou crée la mnémonique BIP39 dans `settings_dir`, la dérive
/// en signing key Ed25519, et retourne un [`WarrenAuthSigner`] partagé.
///
/// Retourne `None` (avec un `log::warn!`) si la mnémonique ne peut pas
/// être chargée ou dérivée, pour permettre au daemon de continuer en
/// mode Mullvad classique pendant la transition Phase 2.
#[must_use]
pub fn load_or_create_signer(settings_dir: &Path) -> Option<Arc<WarrenAuthSigner>> {
    let mnemonic_path = settings_dir.join(MNEMONIC_FILENAME);
    let mnemonic = match warren_identity::load_or_create_mnemonic(&mnemonic_path) {
        Ok(m) => m,
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
    let signing_key = warren_identity::derive_node_key(&seed);
    Some(Arc::new(WarrenAuthSigner::new(signing_key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Utility : un répertoire temporaire isolé pour chaque test.
    /// Pas de `tempfile` ni `uuid` dans les deps daemon, donc on
    /// compose avec `pid + timestamp_nanos + counter` (suffisant
    /// pour éviter les collisions entre tests parallèles
    /// `--test-threads`).
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
        // Phase 2.A.4 V4 — au premier boot du daemon (= settings_dir
        // vide), la fonction doit générer une nouvelle mnémonique
        // BIP39, l'écrire sur disque, et retourner un signer valide.
        let dir = isolated_tempdir();
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "preconditions: pas de mnémonique existante"
        );

        let signer = load_or_create_signer(&dir).expect("must produce a signer on fresh boot");

        // Le fichier doit avoir été créé :
        assert!(
            dir.join(MNEMONIC_FILENAME).exists(),
            "warren_mnemonic.txt doit être créé"
        );
        // Le signer doit produire une pubkey valide (= 64 chars hex) :
        assert_eq!(signer.pubkey_hex().len(), 64);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_signer_is_idempotent_across_calls() {
        // Phase 2.A.4 V4 — au reboot du daemon, la même mnémonique
        // doit produire la même pubkey (= identité utilisateur stable).
        let dir = isolated_tempdir();

        let s1 = load_or_create_signer(&dir).expect("first call");
        let s2 = load_or_create_signer(&dir).expect("second call");
        assert_eq!(
            s1.pubkey_hex(),
            s2.pubkey_hex(),
            "même settings_dir = même pubkey à travers les boots"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_create_signer_returns_none_on_corrupt_mnemonic() {
        // Phase 2.A.4 V4 — si le fichier mnemonic existe mais
        // contient des données corrompues (= pas une mnémonique
        // BIP39 valide), on log et on retourne None plutôt que de
        // crasher le daemon.
        let dir = isolated_tempdir();
        std::fs::write(
            dir.join(MNEMONIC_FILENAME),
            "this is not a valid bip39 mnemonic",
        )
        .expect("write corrupt file");

        let signer = load_or_create_signer(&dir);
        assert!(signer.is_none(), "corruption doit → None, pas panic");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
