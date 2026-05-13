//! Trait abstrait pour les opérations account-level qui partaient en
//! `AccountsProxy` vers `api.mullvad.net`.
//!
//! Permet d'aiguiller au boot entre :
//! - [`RemoteAccountBackend`] : thin wrap sur l'`AccountsProxy` Mullvad
//!   historique. Comportement strictement identique en non-`local`.
//! - [`LocalAccountBackend`] : POC stateless qui sert des données
//!   cohérentes avec la mnémonique chargée au boot, **sans toucher
//!   au réseau**. Remplace l'env-var-bypass `WARREN_LOCAL_ACCOUNT=1`
//!   par un vrai backend pluggable.
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

/// Backend Warren-Remote — Phase G.3 — implémente
/// [`WarrenAccountBackend`] via le HTTP client signé `warren-api-client`
/// qui parle au serveur warren-api (= alternative au path
/// `RemoteAccountBackend` qui parle à `api.mullvad.net`).
///
/// Activé en mode `warren_mode = true && warren_local_account = false`
/// (= 3e branche du dispatch dans `device/mod.rs`, cf. Phase G.4).
///
/// Sémantique mapping :
/// - `create_account()` : retourne la pubkey hex du `WarrenApiClient`
///   (= identité Warren signataire au boot du daemon). Pas d'appel
///   serveur — la création de compte côté warren-api passe par le flow
///   voucher (`POST /v1/register` non-auth) hors de ce trait.
/// - `get_data(account)` : `GET /v1/subscription` signé →
///   [`AccountData`] avec `id = account` et `expiry` reconstitué depuis
///   `expires_at` (unix seconds → `chrono::DateTime<Utc>`).
/// - `delete_account(account)` : `DELETE /v1/account` signé.
#[derive(Clone)]
pub struct WarrenRemoteAccountBackend {
    client: Arc<warren_api_client::WarrenApiClient>,
}

impl WarrenRemoteAccountBackend {
    /// Construit un backend depuis un `WarrenApiClient` configuré au
    /// boot. Le client porte la `SigningKey` Ed25519 (= identité
    /// Warren) et l'URL `warren-api`.
    #[must_use]
    pub fn new(client: warren_api_client::WarrenApiClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl WarrenAccountBackend for WarrenRemoteAccountBackend {
    fn create_account(&self) -> BoxFut<Result<AccountNumber, rest::Error>> {
        // Pas d'appel serveur : l'identité Warren est figée par la
        // mnémonique chargée au boot. La création réelle de la sub
        // côté warren-api se fait via le flow voucher (`POST /v1/register`
        // non-auth) hors de ce trait.
        let pubkey_hex = self.client.pubkey_hex();
        Box::pin(async move { Ok(pubkey_hex) })
    }

    fn get_data(&self, account: AccountNumber) -> BoxFut<Result<AccountData, rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client.get_subscription().await.map_err(map_client_error)?;
            let expiry = expiry_from_unix_secs(resp.expires_at)?;
            Ok(AccountData {
                id: account,
                expiry,
            })
        })
    }

    fn delete_account(&self, _account: AccountNumber) -> BoxFut<Result<(), rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move { client.delete_account().await.map_err(map_client_error) })
    }
}

/// Reconstitue `expiry: DateTime<Utc>` depuis `expires_at: u64` (unix
/// seconds). Le serveur warren-api fournit l'expiry en secondes
/// (cohérent JSON), Mullvad utilise `chrono::DateTime`. Erreur seule
/// possible : `expires_at` overflow `i64` (= année > 9 milliards) →
/// renvoie `Aborted` plutôt que panic.
fn expiry_from_unix_secs(secs: u64) -> Result<chrono::DateTime<Utc>, rest::Error> {
    let secs_i64 = i64::try_from(secs).map_err(|_| rest::Error::Aborted)?;
    chrono::DateTime::from_timestamp(secs_i64, 0).ok_or(rest::Error::Aborted)
}

/// Mappe une [`warren_api_client::ClientError`] vers une
/// [`rest::Error`] Mullvad pour préserver le contrat des traits
/// [`WarrenAccountBackend`] / [`super::device_backend::WarrenDeviceBackend`].
///
/// Convention : un statut HTTP non-2xx → `ApiError(StatusCode, msg)`
/// (mappable côté caller via `map_rest_error`). Tout le reste
/// (transport down, sérialisation, clock) → `Aborted` — cohérent avec
/// le pattern Mullvad pour les pannes infrastructure.
pub(super) fn map_client_error(err: warren_api_client::ClientError) -> rest::Error {
    use warren_api_client::ClientError;
    match err {
        ClientError::ServerStatus { status, body } => {
            let code = rest::StatusCode::from_u16(status)
                .unwrap_or(rest::StatusCode::INTERNAL_SERVER_ERROR);
            let msg = if body.is_empty() {
                format!("warren-api {status}")
            } else {
                format!("warren-api {status}: {body}")
            };
            rest::Error::ApiError(code, msg)
        }
        // Transport / serde / clock → infra down.
        _ => rest::Error::Aborted,
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

    // ===================================================================
    // WarrenRemoteAccountBackend — Phase G.3 tests E2E.
    //
    // Stratégie : spawn warren-api in-process (axum::serve loopback),
    // construit un `WarrenApiClient` signé Ed25519, instancie le backend,
    // exerce chaque méthode du trait. Vérifie le mapping wire warren-api
    // ↔ `mullvad_types::AccountData` + le mapping `ClientError` ↔
    // `rest::Error::ApiError`.
    // ===================================================================

    use ed25519_dalek::SigningKey;
    use std::sync::Arc as TestArc;
    use warren_api_client::WarrenApiClient;

    /// Spawn warren-api en in-process et retourne (URL, AppState).
    /// L'`AppState` permet aux tests d'inspecter / pré-populer les
    /// stores serveur (= raccourci équivalent aux endpoints admin
    /// signés à venir en M5).
    async fn spawn_warren_api() -> (String, TestArc<warren_api::AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().expect("local addr");
        let state = warren_api::AppState::in_memory();
        let app = warren_api::build_router(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        (format!("http://{addr}"), state)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_create_returns_signing_pubkey() {
        // Régression critique : `create_account` doit retourner la
        // pubkey hex de l'identité signataire (= cohérence avec
        // `device.json` côté `warren_device_bootstrap`). Pas d'appel
        // serveur — purement local.
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[60u8; 32]);
        let expected_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let acc = backend.create_account().await.expect("create OK");
        assert_eq!(acc, expected_pubkey_hex);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_reads_subscription_expiry() {
        // Cas nominal : sub présente côté warren-api → backend.get_data()
        // retourne AccountData avec expiry reconstitué depuis expires_at.
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[61u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        // Pré-populate côté serveur (= équivalent /v1/register préalable).
        state.subscriptions.insert(&pubkey_hex, 1_700_000_000);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);
        let data = backend
            .get_data(pubkey_hex.clone())
            .await
            .expect("get_data OK");

        assert_eq!(data.id, pubkey_hex, "id == account passé en arg");
        assert_eq!(
            data.expiry.timestamp(),
            1_700_000_000_i64,
            "expiry doit refléter expires_at serveur"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_get_data_returns_apierror_404_when_no_sub() {
        // Régression critique : si le mapping ClientError → rest::Error
        // perd le statut 404, le caller (`handle_account_data_result`)
        // interprète une erreur générique au lieu de "compte inexistant"
        // → UX dégradée + état device.json incohérent.
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[62u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .get_data(pubkey_hex)
            .await
            .expect_err("must fail with 404 mapping");
        match err {
            rest::Error::ApiError(code, _) => {
                assert_eq!(code.as_u16(), 404, "404 doit transiter intact");
            }
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_removes_subscription() {
        // Cas nominal : delete_account retire la sub côté serveur.
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[63u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        state.subscriptions.insert(&pubkey_hex, 9_999_999_999);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);
        backend
            .delete_account(pubkey_hex.clone())
            .await
            .expect("delete OK");

        assert!(
            state.subscriptions.get_expiry(&pubkey_hex).is_none(),
            "sub doit avoir disparu côté serveur après delete_account"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_account_delete_returns_apierror_404_when_no_sub() {
        // Régression : si on essaie de delete une sub inexistante, le
        // backend doit propager 404 → caller peut décider de l'ignorer
        // ou la log proprement (vs erreur générique).
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[64u8; 32]);
        let pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteAccountBackend::new(client);

        let err = backend
            .delete_account(pubkey_hex)
            .await
            .expect_err("must fail 404");
        match err {
            rest::Error::ApiError(code, _) => {
                assert_eq!(code.as_u16(), 404);
            }
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }
}
