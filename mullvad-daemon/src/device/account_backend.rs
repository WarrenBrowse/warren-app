//! Trait abstrait Warren fork — Phase C.1 — pour les opérations
//! account-level qui partaient en `AccountsProxy` vers `api.mullvad.net`.
//!
//! Permet d'aiguiller au boot entre :
//! - [`RemoteAccountBackend`] : thin wrap sur l'`AccountsProxy` Mullvad
//!   historique. Comportement strictement identique en non-`local`.
//! - [`LocalAccountBackend`] : POC stateless qui sert des données
//!   cohérentes avec la mnémonique chargée au boot, **sans toucher
//!   au réseau**. Remplace l'env-var-bypass `WARREN_LOCAL_ACCOUNT=1`
//!   de la Phase B.3 par un vrai backend pluggable.
//!
//! Périmètre MVP (3 méthodes) : `create_account`, `get_data`,
//! `delete_account`. Les autres méthodes (`submit_voucher`,
//! `get_www_auth_token`, `init_play_purchase`, `verify_play_purchase`,
//! `delete_account` Android) restent dans
//! [`super::service::WarrenIdentityService`] direct sur l'`AccountsProxy`
//! pour cette phase ; à migrer en C.1+ si nécessaire.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use mullvad_api::AccountsProxy;
use mullvad_api::rest;
use mullvad_types::account::{AccountData, AccountNumber};
use mullvad_types::warren_pubkey::WarrenPubKey;

/// Type alias pour les futures retournées par le trait. `Pin<Box<dyn …>>`
/// est imposé par l'object-safety (`Arc<dyn WarrenAccountBackend>`).
/// `'static` est imposé par compat `retry_future` (qui exige que les
/// futures retournées par la factory soient `'static`) — chaque impl
/// du trait doit cloner ses deps avant `Box::pin(async move {…})`.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Backend abstrait pour les opérations account-level critiques MVP.
///
/// Toutes les méthodes retournent un `Result<_, rest::Error>` pour
/// préserver la compatibilité ABI avec `retry_future` et avec la map
/// d'erreur existante (`map_rest_error` côté
/// [`super::service`]). En mode local, les `rest::Error` sont produits
/// uniquement pour les cas dégradés (corruption disque par exemple) —
/// le path nominal est toujours `Ok`.
pub trait WarrenAccountBackend: Send + Sync {
    /// Crée un nouveau compte. Retourne l'`AccountNumber` produit.
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>>;

    /// Récupère les données du compte (= principalement l'expiry).
    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>>;

    /// Supprime le compte (= efface l'identité locale en mode POC).
    ///
    /// Hors Android, le `WarrenIdentityService` n'expose pas
    /// `delete_account` (cf. `service.rs:#[cfg(target_os = "android")]`),
    /// donc cette méthode du trait est compilée mais non invoquée
    /// côté cible non-Android. Conservée pour permettre la migration
    /// future d'un flow `delete_account` desktop si nécessaire, et
    /// utilisée par les tests sur toutes les cibles.
    #[cfg_attr(
        not(any(test, target_os = "android")),
        expect(
            dead_code,
            reason = "appelée uniquement côté Android et dans les tests cross-platform"
        )
    )]
    fn delete_account(&self, account: AccountNumber) -> BoxFut<Result<(), rest::Error>>;
}

/// Wrap fin de l'`AccountsProxy` Mullvad historique. Délègue chaque
/// méthode du trait à l'`AccountsProxy` correspondant. Comportement
/// strictement identique au path Mullvad pré-Warren-fork.
#[derive(Clone)]
pub struct RemoteAccountBackend {
    proxy: AccountsProxy,
}

impl RemoteAccountBackend {
    #[must_use]
    pub fn new(proxy: AccountsProxy) -> Self {
        Self { proxy }
    }
}

impl WarrenAccountBackend for RemoteAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.create_account().await })
    }

    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.get_data(account).await })
    }

    fn delete_account(&self, account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        #[cfg(target_os = "android")]
        {
            let proxy = self.proxy.clone();
            Box::pin(async move { proxy.delete_account(account).await })
        }
        #[cfg(not(target_os = "android"))]
        {
            // L'`AccountsProxy::delete_account` n'existe pas hors Android
            // côté Mullvad upstream ; on retourne une erreur explicite.
            let _ = account;
            Box::pin(async move { Err(rest::Error::Aborted) })
        }
    }
}

/// Backend POC qui sert des données cohérentes avec la mnémonique
/// Warren chargée au boot, sans aucun appel réseau. Idempotent et
/// déterministe (modulo `Utc::now()` pour `get_data.expiry`).
///
/// La source de vérité pour l'identité est la `pubkey: WarrenPubKey`
/// dérivée de `warren_signer::load_or_create_signing_key` — `create_account`
/// renvoie cette pubkey hex comme `AccountNumber` pour rester cohérent
/// avec le `device.json` produit par
/// [`crate::warren_device_bootstrap::ensure_local_device`].
///
/// `delete_account` supprime le `device.json` et le `warren_mnemonic.txt`
/// pour reproduire la sémantique "logged out" Mullvad classique en mode
/// local : le user devra re-bootstrap pour recommencer.
#[derive(Clone)]
pub struct LocalAccountBackend {
    pubkey: WarrenPubKey,
    /// Utilisé exclusivement par `delete_account` (cf. doc de la
    /// méthode trait : non-invoquée hors Android côté caller, mais
    /// le field est nécessaire pour le test cross-platform et pour
    /// une future migration desktop).
    settings_dir: Arc<PathBuf>,
}

impl LocalAccountBackend {
    /// Construit un backend local depuis la pubkey Warren courante et
    /// le `settings_dir` à utiliser pour `delete_account`.
    #[must_use]
    pub fn new(pubkey: WarrenPubKey, settings_dir: PathBuf) -> Self {
        Self {
            pubkey,
            settings_dir: Arc::new(settings_dir),
        }
    }

    /// Expiry retournée par `get_data` en mode local : `Utc::now() +
    /// 100 ans`. Cohérent avec `handle_account_data_result` côté caller
    /// qui interprète `expiry >= now` → `resume_background()` (rotation
    /// BG des clés Wireguard activée comme attendu).
    fn far_future_expiry() -> chrono::DateTime<Utc> {
        // 36500 jours ≈ 100 ans. Bien au-delà de toute durée
        // raisonnable d'utilisation, < `chrono::DateTime::MAX` qui
        // panique au-delà de l'an 262143.
        Utc::now() + chrono::Duration::days(36500)
    }
}

impl WarrenAccountBackend for LocalAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        // Identité POC = pubkey hex (64 chars). Idempotent par
        // construction : la pubkey ne change pas pour un settings_dir
        // donné (même mnémonique).
        let number = self.pubkey.as_str().to_owned();
        Box::pin(async move { Ok(number) })
    }

    fn get_data(&self, _account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        // L'`AccountId` Mullvad est un `String` opaque ; on retourne
        // la pubkey hex pour cohérence avec `create_account`. L'expiry
        // pousse `handle_account_data_result` à `resume_background()`.
        let id = self.pubkey.as_str().to_owned();
        let data = AccountData {
            id,
            expiry: Self::far_future_expiry(),
        };
        Box::pin(async move { Ok(data) })
    }

    fn delete_account(&self, _account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let settings_dir = self.settings_dir.clone();
        Box::pin(async move {
            // Supprime le device.json (= état "logged out" pour le
            // DeviceCacher au prochain boot) et la mnémonique BIP39
            // (= identité). Idempotent : un fichier déjà absent ne
            // produit pas d'erreur.
            let device_path = settings_dir.join(super::DEVICE_CACHE_FILENAME);
            let mnemonic_path = settings_dir.join(crate::warren_signer::MNEMONIC_FILENAME);
            for path in [device_path, mnemonic_path] {
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        log::error!(
                            "LocalAccountBackend::delete_account failed to remove {}: {}",
                            path.display(),
                            e
                        );
                        return Err(rest::Error::Aborted);
                    }
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn fixed_pubkey() -> WarrenPubKey {
        WarrenPubKey::from_str(&"a".repeat(64)).expect("valid hex 64ch")
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
        let dir = std::env::temp_dir().join(format!("warren-account-backend-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[tokio::test]
    async fn local_get_data_returns_far_future_expiry() {
        // Régression critique : si l'expiry retournée < now, le caller
        // `handle_account_data_result` (cf. service.rs) déclenche
        // `pause_background()` → la rotation BG des clés Wireguard
        // s'arrête silencieusement. C'est exactement le comportement
        // qu'on veut éviter en mode local POC.
        let backend = LocalAccountBackend::new(fixed_pubkey(), isolated_tempdir());
        let data = backend
            .get_data("ignored".to_owned())
            .await
            .expect("local get_data ne doit jamais fail en cas nominal");

        let lower_bound = Utc::now() + chrono::Duration::days(50 * 365);
        assert!(
            data.expiry > lower_bound,
            "expiry {} doit être > now + 50 ans pour activer resume_background",
            data.expiry
        );
    }

    #[tokio::test]
    async fn local_create_account_returns_pubkey_hex_deterministic() {
        // Régression critique : si `create_account` retournait un
        // String random ou différent d'un appel à l'autre, le
        // `device.json` bootstrappé via la mnémonique deviendrait
        // orphelin du compte créé → l'utilisateur ne pourrait plus
        // recharger sa session après reboot.
        let backend = LocalAccountBackend::new(fixed_pubkey(), isolated_tempdir());
        let n1 = backend
            .create_account()
            .await
            .expect("create_account ne doit jamais fail localement");
        let n2 = backend
            .create_account()
            .await
            .expect("create_account ne doit jamais fail localement");

        assert_eq!(
            n1, n2,
            "create_account doit être idempotent (= déterministe)"
        );
        assert_eq!(
            n1,
            fixed_pubkey().as_str(),
            "AccountNumber DOIT être la pubkey hex (= cohérence avec device.json bootstrap)"
        );
    }

    #[tokio::test]
    async fn local_delete_account_removes_device_json_and_mnemonic() {
        // Régression critique : si delete_account ne supprime pas les
        // artefacts identitaires, le user reste "logged in" via
        // device.json après un account delete = bug grave UX +
        // sécurité (impossible de "vraiment" se déconnecter).
        let dir = isolated_tempdir();
        let device_path = dir.join(super::super::DEVICE_CACHE_FILENAME);
        let mnemonic_path = dir.join(crate::warren_signer::MNEMONIC_FILENAME);
        std::fs::write(&device_path, "{}").expect("write device.json");
        std::fs::write(&mnemonic_path, "test mnemonic").expect("write mnemonic");

        let backend = LocalAccountBackend::new(fixed_pubkey(), dir.clone());
        backend
            .delete_account("ignored".to_owned())
            .await
            .expect("delete_account doit succeed");

        assert!(
            !device_path.exists(),
            "device.json doit être supprimé après delete_account"
        );
        assert!(
            !mnemonic_path.exists(),
            "warren_mnemonic.txt doit être supprimé après delete_account"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn local_delete_account_is_idempotent_on_missing_files() {
        // Edge case : si le user appelle delete_account 2 fois, ou
        // que le bootstrap a déjà été nettoyé manuellement, on ne
        // doit pas remonter une erreur (qui serait interprétée
        // comme un échec API et propagée à l'UI).
        let dir = isolated_tempdir();
        let backend = LocalAccountBackend::new(fixed_pubkey(), dir.clone());

        backend
            .delete_account("ignored".to_owned())
            .await
            .expect("delete_account avec fichiers absents doit retourner Ok");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
