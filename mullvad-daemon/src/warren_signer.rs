//! Charge ou génère la mnémonique BIP39 utilisateur depuis
//! `<settings_dir>/warren_mnemonic.txt`, la dérive en [`SigningKey`]
//! Ed25519 via [`warren_identity::derive_node_key`], et la wrap dans
//! un [`mullvad_api::warren_auth::WarrenAuthSigner`] partagé via
//! [`Arc`].
//!
//! Module dédié pour permettre de wirer/unwirer la Warren auth via
//! une seule modif de l'unique caller dans `lib.rs`, et tester la
//! logique en isolation (vs `lib.rs` qui orchestre tout le boot et
//! est non-testable en unit).
//!
//! Politique d'erreur : on log et on retourne `None` si la mnémonique
//! est inaccessible / corrompue. Le boot continue en mode Bearer
//! historique. Cette dégradation disparaîtra quand la chaîne sera
//! 100 % Warren (aucun fallback Bearer possible côté serveur).

use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::SigningKey;
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
    let signing_key = load_or_create_signing_key(settings_dir)?;
    Some(Arc::new(WarrenAuthSigner::new(signing_key)))
}

/// Charge ou crée la mnémonique BIP39 dans `settings_dir` et la dérive
/// en [`SigningKey`] Ed25519, sans wrapper.
///
/// Sœur de [`load_or_create_signer`] : expose le matériel
/// cryptographique brut nécessaire pour assembler des
/// [`talpid_warren_iroh::WarrenIrohParameters`].
///
/// **Politique no-log** : ne JAMAIS logger la `SigningKey` retournée
/// (cf. règle Warren). Le caller doit la consommer puis la dropper.
#[must_use]
pub fn load_or_create_signing_key(settings_dir: &Path) -> Option<SigningKey> {
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
    Some(warren_identity::derive_node_key(&seed))
}

/// Lit la mnémonique BIP39 utilisateur **déjà persistée** dans
/// `<settings_dir>/warren_mnemonic.txt`. Read-only : ne crée jamais
/// le fichier (contraste avec [`load_or_create_signing_key`]).
///
/// Retourne `None` si :
/// - le fichier n'existe pas (= identité jamais bootstrappée),
/// - le fichier est inaccessible (perms cassées, FS error).
///
/// Utilisé par le handler gRPC `GetWarrenMnemonic` (C.1) pour
/// permettre au GUI Electron d'afficher la mnémonique en clair afin
/// que l'utilisateur la sauvegarde (= critère phase 1 #2 "Mnemonic
/// BIP39 affiché 1 fois et restaurable").
///
/// # Politique no-log
///
/// La string retournée est un secret cryptographique. Le caller
/// (handler gRPC) doit la transmettre au GUI puis la dropper. Ne
/// JAMAIS logger le contenu, même en debug. Le fait *qu'une lecture
/// a eu lieu* peut être loggé (= audit trail GUI requests), mais
/// jamais le contenu.
#[must_use]
pub fn get_warren_mnemonic(settings_dir: &Path) -> Option<String> {
    let path = settings_dir.join(MNEMONIC_FILENAME);
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
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
        // Au premier boot du daemon (= settings_dir
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
        // Au reboot du daemon, la même mnémonique
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
    fn get_warren_mnemonic_returns_none_when_no_file_exists() {
        // Au premier boot, avant load_or_create_signer, le fichier
        // n'existe pas → la fonction doit retourner None sans paniquer
        // ni créer de fichier (read-only get).
        let dir = isolated_tempdir();
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "préconditions : pas de mnémonique"
        );

        let result = get_warren_mnemonic(&dir);
        assert!(
            result.is_none(),
            "absent file must yield None, not panic or create"
        );
        assert!(
            !dir.join(MNEMONIC_FILENAME).exists(),
            "get_warren_mnemonic ne doit JAMAIS créer le fichier (read-only)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_warren_mnemonic_returns_existing_mnemonic_after_persist() {
        // Après load_or_create_signer (= bootstrap identité), la
        // mnémonique BIP39 doit être lisible via get_warren_mnemonic
        // et contenir 12 ou 24 mots (= cardinal BIP39 standard).
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
        // Invariant cross-fonction critique : la mnémonique retournée
        // par get_warren_mnemonic, re-dérivée via warren_identity::
        // {seed_from_mnemonic, derive_node_key}, doit produire
        // EXACTEMENT la même pubkey que load_or_create_signing_key.
        // Sinon le user qui exporte sa mnémonique pour backup se
        // retrouve avec une identité différente au restore (= perte
        // de subscription → blocker phase 1 critère #2).
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
    fn load_or_create_signer_returns_none_on_corrupt_mnemonic() {
        // Si le fichier mnemonic existe mais
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
