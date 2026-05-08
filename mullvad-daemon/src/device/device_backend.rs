//! Trait abstrait Warren fork — Phase C.2 — pour les opérations
//! device-level qui partaient en `DevicesProxy` vers `api.mullvad.net`.
//!
//! Pendant device-side du [`super::account_backend`] : permet d'aiguiller
//! au boot entre :
//! - [`RemoteDeviceBackend`] : thin wrap sur l'`DevicesProxy` Mullvad
//!   historique. Comportement strictement identique en non-`local`.
//! - [`LocalDeviceBackend`] : POC stateful (HashMap mémoire-only) qui
//!   sert des données cohérentes avec le `device.json` bootstrappé,
//!   sans aucun appel réseau.
//!
//! Périmètre 5 méthodes : `create`, `get`, `list`, `remove`,
//! `replace_wg_key`. `LocalDeviceBackend` ne touche **jamais** au
//! `device.json` — c'est `DeviceCacher` qui possède le fichier (cf.
//! recon C.2 § Q8 conflit DeviceCacher). Le HashMap interne est
//! seulement un *shadow lookup* pour les requêtes API simulées.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use mullvad_api::DevicesProxy;
use mullvad_api::rest;
use mullvad_types::account::AccountNumber;
use mullvad_types::device::{Device, DeviceId};
use mullvad_types::wireguard::AssociatedAddresses;
use std::future::Future;
use talpid_types::net::wireguard::PublicKey as WgPublicKey;

use super::PrivateDeviceState;

/// Type alias pour les futures retournées par le trait. Cf. doc
/// [`super::account_backend::BoxFut`] pour le rationale `'static`.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Backend abstrait pour les 5 opérations device-level qui passaient
/// historiquement par `DevicesProxy`. Cf. doc module pour le pendant
/// account-side.
pub trait WarrenDeviceBackend: Send + Sync {
    /// Crée un device pour le compte donné, lié à la pubkey WireGuard
    /// fournie. Retourne le device produit + les `AssociatedAddresses`
    /// allouées pour ce client. Idempotent côté local : un appel répété
    /// avec la même pubkey wg retourne le même `Device` (même `id`).
    fn create(
        &self,
        account: AccountNumber,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<(Device, AssociatedAddresses), rest::Error>>;

    /// Récupère un device par son id. Retourne `Err(rest::Error::ApiError(NOT_FOUND))`
    /// si l'id n'est pas connu (= 404 côté backend).
    fn get(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<Device, rest::Error>>;

    /// Liste tous les devices du compte. Côté local, contient au plus
    /// l'unique device bootstrappé (= POC single-device par identité).
    fn list(&self, account: AccountNumber) -> BoxFut<Result<Vec<Device>, rest::Error>>;

    /// Supprime un device par son id. Idempotent : un id déjà absent
    /// retourne `Ok(())`.
    fn remove(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<(), rest::Error>>;

    /// Rotation de la pubkey WireGuard d'un device existant. Retourne
    /// les `AssociatedAddresses` du device — qui DOIVENT rester
    /// **identiques** à celles initialement allouées au `create`
    /// (anti-régression : si on re-derivait depuis la nouvelle pubkey,
    /// le tunnel exit Warren rejetterait le client = mismatch IP).
    fn replace_wg_key(
        &self,
        account: AccountNumber,
        id: DeviceId,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>>;
}

/// Wrap fin de l'`DevicesProxy` Mullvad. Forward direct, comportement
/// strictement identique au path historique pré-Warren-fork.
#[derive(Clone)]
pub struct RemoteDeviceBackend {
    proxy: DevicesProxy,
}

impl RemoteDeviceBackend {
    #[must_use]
    pub fn new(proxy: DevicesProxy) -> Self {
        Self { proxy }
    }
}

impl WarrenDeviceBackend for RemoteDeviceBackend {
    fn create(
        &self,
        account: AccountNumber,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<(Device, AssociatedAddresses), rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.create(account, pubkey).await })
    }

    fn get(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<Device, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.get(account, id).await })
    }

    fn list(&self, account: AccountNumber) -> BoxFut<Result<Vec<Device>, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.list(account).await })
    }

    fn remove(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<(), rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.remove(account, id).await })
    }

    fn replace_wg_key(
        &self,
        account: AccountNumber,
        id: DeviceId,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>> {
        let proxy = self.proxy.clone();
        Box::pin(async move { proxy.replace_wg_key(account, id, pubkey).await })
    }
}

/// Entry interne du HashMap `LocalDeviceBackend`. Stocke le `Device`
/// public (renvoyé par `create`/`get`/`list`) et les `AssociatedAddresses`
/// allouées (= persistantes pour la durée de vie du device, indépendantes
/// de la pubkey wg qui peut tourner via `replace_wg_key`).
#[derive(Debug, Clone)]
struct LocalDeviceEntry {
    device: Device,
    addresses: AssociatedAddresses,
}

/// Backend POC qui maintient un shadow `HashMap<DeviceId, LocalDeviceEntry>`
/// en mémoire. Ne touche **jamais** au `device.json` : c'est le
/// [`super::DeviceCacher`] qui possède le fichier (cf. recon C.2 § Q8).
///
/// Le HashMap est seedé depuis [`PrivateDeviceState`] au boot via
/// [`Self::from_state`] pour que `list` retourne immédiatement le device
/// bootstrappé en mode local. Les mutations (`create`/`remove`/
/// `replace_wg_key`) mettent à jour le HashMap mais le caller reste
/// responsable de l'écriture disque via `DeviceCacher::write` —
/// cohérence garantie par la séquence appel-backend → set-cacher dans
/// [`super::AccountManager`].
#[derive(Clone)]
pub struct LocalDeviceBackend {
    inner: Arc<Mutex<HashMap<DeviceId, LocalDeviceEntry>>>,
}

impl LocalDeviceBackend {
    /// Construit un backend vide (= aucun device connu). Utile pour
    /// les tests d'opérations qui n'ont pas besoin de seed.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Construit un backend dont le HashMap est seedé depuis l'état
    /// fourni — typiquement `PrivateDeviceState` lu par `DeviceCacher`
    /// au boot. Si l'état est `LoggedIn`, l'unique device est inséré.
    /// Sinon (LoggedOut/Revoked), le HashMap démarre vide.
    #[must_use]
    pub fn from_state(state: &PrivateDeviceState) -> Self {
        let backend = Self::empty();
        if let PrivateDeviceState::LoggedIn(account) = state {
            let device = Device {
                id: account.device.id.clone(),
                name: account.device.name.clone(),
                pubkey: account.device.wg_data.private_key.public_key(),
                hijack_dns: account.device.hijack_dns,
                created: account.device.created,
            };
            let entry = LocalDeviceEntry {
                device: device.clone(),
                addresses: account.device.wg_data.addresses.clone(),
            };
            backend
                .inner
                .lock()
                .expect("poisoned mutex (only one path locks)")
                .insert(device.id.clone(), entry);
        }
        backend
    }
}

/// Dérive un `DeviceId` POC stable à partir de la pubkey wg (= 32 bytes).
/// Préfixé `warren-local-` pour distinguer des UUID Mullvad.
fn derive_device_id(pubkey: &WgPublicKey) -> DeviceId {
    let bytes = pubkey.as_bytes();
    let hex_short = bytes[..8]
        .iter()
        .fold(String::with_capacity(16), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        });
    format!("warren-local-{hex_short}")
}

/// Dérive un `AssociatedAddresses` POC depuis les bytes de la pubkey
/// wg pour éviter les collisions IP triviales entre clients sur un
/// même exit. Plages :
/// - IPv4 dans `10.64.0.0/10` (compat Mullvad).
/// - IPv6 dans `fc00:bbbb::/32` (ULA arbitraire POC).
fn derive_addresses(pubkey: &WgPublicKey) -> AssociatedAddresses {
    let bytes = pubkey.as_bytes();
    let v4_str = format!("10.64.{}.{}/32", bytes[0], bytes[1].max(1));
    let v6_str = format!(
        "fc00:bbbb::{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}/128",
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9]
    );
    AssociatedAddresses {
        ipv4_address: v4_str
            .parse()
            .expect("hardcoded 10.64.X.Y/32 is always a valid Ipv4Network"),
        ipv6_address: v6_str
            .parse()
            .expect("hardcoded fc00:bbbb::.../128 is always a valid Ipv6Network"),
    }
}

impl WarrenDeviceBackend for LocalDeviceBackend {
    fn create(
        &self,
        _account: AccountNumber,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<(Device, AssociatedAddresses), rest::Error>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let id = derive_device_id(&pubkey);
            let mut guard = inner
                .lock()
                .expect("poisoned mutex (single-thread Tokio in tests)");

            // Idempotence : si une entry existe déjà pour ce id (=
            // même pubkey wg), retourner le device existant. Le caller
            // appelle parfois `create` plusieurs fois lors d'un re-login
            // après logout volontaire.
            if let Some(entry) = guard.get(&id) {
                return Ok((entry.device.clone(), entry.addresses.clone()));
            }

            let now = Utc::now();
            let addresses = derive_addresses(&pubkey);
            let device = Device {
                id: id.clone(),
                name: format!("warren-local-{}", &id[13..21]),
                pubkey,
                hijack_dns: false,
                created: now,
            };
            let entry = LocalDeviceEntry {
                device: device.clone(),
                addresses: addresses.clone(),
            };
            guard.insert(id, entry);
            Ok((device, addresses))
        })
    }

    fn get(&self, _account: AccountNumber, id: DeviceId) -> BoxFut<Result<Device, rest::Error>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.lock().expect("poisoned mutex");
            guard
                .get(&id)
                .map(|entry| entry.device.clone())
                .ok_or_else(|| {
                    rest::Error::ApiError(
                        rest::StatusCode::NOT_FOUND,
                        mullvad_api::DEVICE_NOT_FOUND.to_owned(),
                    )
                })
        })
    }

    fn list(&self, _account: AccountNumber) -> BoxFut<Result<Vec<Device>, rest::Error>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.lock().expect("poisoned mutex");
            let devices = guard
                .values()
                .map(|entry| entry.device.clone())
                .collect::<Vec<_>>();
            Ok(devices)
        })
    }

    fn remove(&self, _account: AccountNumber, id: DeviceId) -> BoxFut<Result<(), rest::Error>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let mut guard = inner.lock().expect("poisoned mutex");
            // Idempotent : `remove` d'un id déjà absent retourne Ok.
            // Cohérent avec sémantique gRPC `RemoveDevice` côté UI.
            guard.remove(&id);
            Ok(())
        })
    }

    fn replace_wg_key(
        &self,
        _account: AccountNumber,
        id: DeviceId,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let mut guard = inner.lock().expect("poisoned mutex");
            let entry = guard.get_mut(&id).ok_or_else(|| {
                rest::Error::ApiError(
                    rest::StatusCode::NOT_FOUND,
                    mullvad_api::DEVICE_NOT_FOUND.to_owned(),
                )
            })?;
            // **Régression critique** (cf. recon C.2 § 6) : ne JAMAIS
            // re-dériver les addresses depuis la nouvelle pubkey wg.
            // L'exit Warren a alloué une IP de tunnel pour cette
            // identité au `create` initial — la rotation de clé wg ne
            // doit pas changer l'IP, sinon le tunnel devient inutilisable.
            entry.device.pubkey = pubkey;
            Ok(entry.addresses.clone())
        })
    }
}

/// Backend Warren-Remote — Phase G.3.b — implémente
/// [`WarrenDeviceBackend`] via le client `warren-api-client` qui parle
/// au serveur warren-api.
///
/// Activé en mode `warren_mode = true && warren_local_account = false`
/// (= 3e branche du dispatch dans `device/mod.rs`).
///
/// Mapping wire :
/// - [`warren_api_client::Device`] ↔ [`mullvad_types::device::Device`]
///   via [`map_device_response`].
/// - [`WgPublicKey`] ↔ `wg_pubkey_hex` (32 bytes ↔ 64 chars hex).
/// - [`AssociatedAddresses`] : warren-api ne les retourne pas en MVP,
///   on émet un stub fixe ([`stub_associated_addresses`]). Raffinement
///   M5+ : warren-api allouera de vraies IPs (10.66.x.y / fc00:bbbb::x).
/// - `replace_wg_key` : non supporté (warren-api n'expose pas encore
///   l'endpoint rotate). Retourne `rest::Error::Aborted` jusqu'à ce
///   que G.3.b+ ajoute `PUT /v1/devices/{id}` côté serveur.
#[derive(Clone)]
pub struct WarrenRemoteDeviceBackend {
    client: Arc<warren_api_client::WarrenApiClient>,
}

impl WarrenRemoteDeviceBackend {
    /// Construit un backend depuis un `WarrenApiClient` configuré au
    /// boot (= identité Warren signataire + URL warren-api).
    #[must_use]
    pub fn new(client: warren_api_client::WarrenApiClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl WarrenDeviceBackend for WarrenRemoteDeviceBackend {
    fn create(
        &self,
        _account: AccountNumber,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<(Device, AssociatedAddresses), rest::Error>> {
        let client = self.client.clone();
        let wg_pubkey_hex = hex::encode(pubkey.as_bytes());
        Box::pin(async move {
            let req = warren_api_client::RegisterDeviceRequest {
                wg_pubkey_hex,
                hijack_dns: false,
            };
            let server_device = client
                .register_device(&req)
                .await
                .map_err(super::account_backend::map_client_error)?;
            let device = map_device_response(server_device)?;
            Ok((device, stub_associated_addresses()))
        })
    }

    fn get(&self, _account: AccountNumber, id: DeviceId) -> BoxFut<Result<Device, rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            let server_device = client
                .get_device(&id)
                .await
                .map_err(super::account_backend::map_client_error)?;
            map_device_response(server_device)
        })
    }

    fn list(&self, _account: AccountNumber) -> BoxFut<Result<Vec<Device>, rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            let server_devices = client
                .list_devices()
                .await
                .map_err(super::account_backend::map_client_error)?;
            server_devices
                .into_iter()
                .map(map_device_response)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn remove(&self, _account: AccountNumber, id: DeviceId) -> BoxFut<Result<(), rest::Error>> {
        let client = self.client.clone();
        Box::pin(async move {
            client
                .delete_device(&id)
                .await
                .map_err(super::account_backend::map_client_error)
        })
    }

    fn replace_wg_key(
        &self,
        _account: AccountNumber,
        _id: DeviceId,
        _pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>> {
        // MVP G.3 : warren-api n'expose pas encore `PUT /v1/devices/{id}`.
        // Cf. doc struct pour roadmap. `Aborted` est la convention
        // Mullvad pour "API non disponible" (= cohérent avec
        // `RemoteAccountBackend::delete_account` hors Android).
        Box::pin(async move { Err(rest::Error::Aborted) })
    }
}

/// Mappe une `warren_api_client::Device` (wire JSON) vers
/// [`mullvad_types::device::Device`].
///
/// Erreurs possibles :
/// - `wg_pubkey_hex` invalide (pas 32 bytes après decode) → `Aborted`.
/// - `created_at` overflow `i64` (= année > 9 milliards) → `Aborted`.
fn map_device_response(d: warren_api_client::Device) -> Result<Device, rest::Error> {
    let wg_bytes = hex::decode(&d.wg_pubkey_hex).map_err(|_| rest::Error::Aborted)?;
    let wg_array: [u8; 32] = wg_bytes.try_into().map_err(|_| rest::Error::Aborted)?;
    let pubkey = WgPublicKey::from(wg_array);
    let secs_i64 = i64::try_from(d.created_at).map_err(|_| rest::Error::Aborted)?;
    let created = chrono::DateTime::from_timestamp(secs_i64, 0).ok_or(rest::Error::Aborted)?;
    Ok(Device {
        id: d.id,
        name: d.name,
        pubkey,
        hijack_dns: d.hijack_dns,
        created,
    })
}

/// Stub `AssociatedAddresses` — warren-api ne fournit pas d'allocation
/// IP en MVP. Adresses fixes Mullvad-style pour que `PrivateDevice::try_from_device`
/// côté caller puisse construire un device valide. En mode warren-mode,
/// le tunnel data plane passe par `warren-iroh-tunnel` qui n'utilise
/// PAS ces IPs WireGuard — donc l'absence d'allocation réelle n'est
/// pas un blocker pour la chaîne MVP.
fn stub_associated_addresses() -> AssociatedAddresses {
    AssociatedAddresses {
        ipv4_address: "10.66.0.1/32".parse().expect("valid v4 stub"),
        ipv6_address: "fc00:bbbb::1/128".parse().expect("valid v6 stub"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talpid_types::net::wireguard::PrivateKey as WgPrivateKey;

    fn fixed_wg_pubkey(seed: u8) -> WgPublicKey {
        WgPrivateKey::from([seed; 32]).public_key()
    }

    #[tokio::test]
    async fn local_create_returns_device_with_input_wg_pubkey() {
        // Régression critique : le `Device.pubkey` retourné par
        // `create` DOIT être la pubkey wg fournie en entrée, sinon le
        // path `try_from_device` côté caller (PrivateDevice::try_from_device,
        // device/mod.rs:228) échoue avec `Error::InvalidDevice` qui
        // crash le flow login.
        let backend = LocalDeviceBackend::empty();
        let pubkey = fixed_wg_pubkey(7);

        let (device, _addrs) = backend
            .create("acc".to_owned(), pubkey.clone())
            .await
            .expect("create doit succeed en local");

        assert_eq!(
            device.pubkey.as_bytes(),
            pubkey.as_bytes(),
            "Device.pubkey DOIT matcher la pubkey wg input (sinon try_from_device échoue)"
        );
    }

    #[tokio::test]
    async fn local_create_idempotent_for_same_wg_pubkey() {
        // Régression critique : un re-login (= même mnémonique → même
        // wg privkey régénérée la même façon ? non, wg privkey est
        // random à chaque generate_for_account ; mais si le caller
        // appelle deux fois `create` avec la même pubkey, on ne doit
        // pas créer deux entries divergentes). Test indirect de
        // l'idempotence du HashMap par pubkey.
        let backend = LocalDeviceBackend::empty();
        let pubkey = fixed_wg_pubkey(11);

        let (d1, a1) = backend
            .create("acc".to_owned(), pubkey.clone())
            .await
            .unwrap();
        let (d2, a2) = backend
            .create("acc".to_owned(), pubkey.clone())
            .await
            .unwrap();

        assert_eq!(
            d1.id, d2.id,
            "create idempotent doit retourner le même DeviceId pour la même pubkey wg"
        );
        assert_eq!(
            a1.ipv4_address, a2.ipv4_address,
            "les addresses ne doivent pas changer entre deux create idempotents"
        );
    }

    #[tokio::test]
    async fn local_list_returns_seeded_device_after_init_from_state() {
        // Régression critique : si le seed depuis `PrivateDeviceState`
        // ne fonctionne pas, `mullvad-cli device list` afficherait 0
        // devices alors que le user vient de bootstrapper sa mnémonique
        // = panne UX. Le test simule ce path en construisant un état
        // LoggedIn et asserte que `list` le voit bien.
        let pubkey = fixed_wg_pubkey(13);
        let priv_key = WgPrivateKey::from([13u8; 32]);
        let addresses = derive_addresses(&pubkey);
        let device_priv = super::super::PrivateDevice {
            id: "warren-local-abcdef00".to_owned(),
            name: "test-device".to_owned(),
            wg_data: mullvad_types::wireguard::WireguardData {
                private_key: priv_key,
                addresses,
                created: Utc::now(),
            },
            hijack_dns: false,
            created: Utc::now(),
        };
        let state = PrivateDeviceState::LoggedIn(super::super::PrivateAccountAndDevice {
            pubkey: mullvad_types::warren_pubkey::WarrenPubKey::from_bytes(&[7u8; 32]),
            device: device_priv,
        });

        let backend = LocalDeviceBackend::from_state(&state);
        let devices = backend.list("ignored".to_owned()).await.unwrap();

        assert_eq!(devices.len(), 1, "list doit voir le device seedé");
        assert_eq!(devices[0].id, "warren-local-abcdef00");
    }

    #[tokio::test]
    async fn local_get_returns_not_found_after_remove() {
        // Régression critique : si `remove` était no-op, le state
        // machine ne déclencherait jamais `revoke_device` après un
        // logout = bug grave (user reste "logged in" perpétuellement).
        let backend = LocalDeviceBackend::empty();
        let pubkey = fixed_wg_pubkey(17);
        let (device, _) = backend.create("acc".to_owned(), pubkey).await.unwrap();

        backend
            .remove("acc".to_owned(), device.id.clone())
            .await
            .unwrap();

        let result = backend.get("acc".to_owned(), device.id).await;
        assert!(
            matches!(
                result,
                Err(rest::Error::ApiError(rest::StatusCode::NOT_FOUND, _))
            ),
            "get après remove doit retourner NOT_FOUND, got {result:?}"
        );
    }

    #[tokio::test]
    async fn local_replace_wg_key_changes_pubkey_but_preserves_addresses() {
        // Régression critique D2/C.2 : si `replace_wg_key` re-dérivait
        // les addresses depuis la nouvelle pubkey, le tunnel exit
        // Warren rejetterait le client (mismatch IP côté allocation
        // exit). C'est exactement le bug invisible que ce test
        // shielde — sans assertion `addresses_before == addresses_after`,
        // la régression passerait inaperçue jusqu'au déploiement prod.
        let backend = LocalDeviceBackend::empty();
        let pubkey_initial = fixed_wg_pubkey(19);
        let (device, addresses_before) = backend
            .create("acc".to_owned(), pubkey_initial.clone())
            .await
            .unwrap();

        let pubkey_new = fixed_wg_pubkey(29);
        let addresses_after = backend
            .replace_wg_key("acc".to_owned(), device.id.clone(), pubkey_new.clone())
            .await
            .unwrap();

        assert_eq!(
            addresses_before.ipv4_address, addresses_after.ipv4_address,
            "replace_wg_key DOIT préserver l'ipv4 (sinon mismatch IP exit)"
        );
        assert_eq!(
            addresses_before.ipv6_address, addresses_after.ipv6_address,
            "replace_wg_key DOIT préserver l'ipv6"
        );

        // Mais la pubkey du device doit avoir bien tourné :
        let device_after = backend.get("acc".to_owned(), device.id).await.unwrap();
        assert_eq!(
            device_after.pubkey.as_bytes(),
            pubkey_new.as_bytes(),
            "Device.pubkey doit refléter la nouvelle pubkey wg post-rotate"
        );
        assert_ne!(
            device_after.pubkey.as_bytes(),
            pubkey_initial.as_bytes(),
            "Device.pubkey ne doit PAS être l'ancienne pubkey"
        );
    }

    // ===================================================================
    // WarrenRemoteDeviceBackend — Phase G.3.b tests E2E.
    // Cf. account_backend::tests pour le pattern spawn_warren_api.
    // ===================================================================

    use ed25519_dalek::SigningKey;
    use std::sync::Arc as TestArc;
    use warren_api_client::WarrenApiClient;

    /// Wg_pubkey hex 64 chars dérivée d'une seed fixe.
    fn fixed_wg_pubkey_hex(seed: u8) -> String {
        hex::encode(WgPrivateKey::from([seed; 32]).public_key().as_bytes())
    }

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
    async fn warren_remote_device_create_inserts_device_and_returns_stub_addresses() {
        // Cas nominal : create envoie POST /v1/devices signé, le serveur
        // upsert le device, le client le mappe en mullvad_types::Device.
        // Les addresses retournées sont les stubs (warren-api ne les
        // alloue pas en MVP).
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[70u8; 32]);
        let owner_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);

        let wg_pubkey = WgPrivateKey::from([71u8; 32]).public_key();
        let (device, addresses) = backend
            .create("acc".to_owned(), wg_pubkey.clone())
            .await
            .expect("create OK");

        // Le pubkey wg du Device retourné == celui envoyé.
        assert_eq!(device.pubkey.as_bytes(), wg_pubkey.as_bytes());
        // ID 32 hex chars (cf. compute_device_id côté warren-api).
        assert_eq!(device.id.len(), 32);
        // Le serveur a bien le device.
        let server_devices = state.devices.list_for_owner(&owner_pubkey_hex);
        assert_eq!(server_devices.len(), 1);
        assert_eq!(server_devices[0].id, device.id);
        // Stub addresses (à raffiner M5+).
        assert_eq!(
            addresses.ipv4_address.to_string(),
            "10.66.0.1/32",
            "stub v4 attendu en MVP"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_get_returns_existing_device() {
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[72u8; 32]);
        let owner_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Pré-popule via le store warren-api directement (= équivalent
        // d'un `create` préalable, raccourci test).
        let server_device =
            state
                .devices
                .register(&owner_pubkey_hex, &fixed_wg_pubkey_hex(73), false, now);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);
        let got = backend
            .get("acc".to_owned(), server_device.id.clone())
            .await
            .expect("get OK");
        assert_eq!(got.id, server_device.id);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_get_returns_apierror_404_for_unknown_id() {
        // Régression critique : si on perd le 404 dans le mapping, le
        // caller (`AccountManager::handle_get_device`) ne peut pas
        // distinguer "device supprimé/révoqué" d'une erreur transport.
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[74u8; 32]);
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);

        let err = backend
            .get(
                "acc".to_owned(),
                "deadbeefdeadbeefdeadbeefdeadbeef".to_owned(),
            )
            .await
            .expect_err("must fail 404");
        match err {
            rest::Error::ApiError(code, _) => assert_eq!(code.as_u16(), 404),
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_list_returns_only_owners_devices() {
        // Anti-cross-tenant : list signé par A ne contient que les
        // devices de A (assuré par le serveur via identity middleware).
        let (api_url, state) = spawn_warren_api().await;
        let key_a = SigningKey::from_bytes(&[75u8; 32]);
        let pubkey_a = hex::encode(key_a.verifying_key().as_bytes());
        let key_b = SigningKey::from_bytes(&[76u8; 32]);
        let pubkey_b = hex::encode(key_b.verifying_key().as_bytes());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        state
            .devices
            .register(&pubkey_a, &fixed_wg_pubkey_hex(77), false, now);
        state
            .devices
            .register(&pubkey_b, &fixed_wg_pubkey_hex(78), false, now);

        let client_a = WarrenApiClient::new(api_url, key_a);
        let backend_a = WarrenRemoteDeviceBackend::new(client_a);
        let list = backend_a.list("acc".to_owned()).await.expect("list OK");
        assert_eq!(list.len(), 1, "A doit voir seulement ses propres devices");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_remove_deletes_device_in_store() {
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[79u8; 32]);
        let owner_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let server_device =
            state
                .devices
                .register(&owner_pubkey_hex, &fixed_wg_pubkey_hex(80), false, now);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);
        backend
            .remove("acc".to_owned(), server_device.id)
            .await
            .expect("remove OK");

        assert!(
            state.devices.list_for_owner(&owner_pubkey_hex).is_empty(),
            "device doit avoir disparu du store côté serveur"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_remove_returns_apierror_404_for_unknown_id() {
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[81u8; 32]);
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);

        let err = backend
            .remove(
                "acc".to_owned(),
                "deadbeefdeadbeefdeadbeefdeadbeef".to_owned(),
            )
            .await
            .expect_err("must fail 404");
        match err {
            rest::Error::ApiError(code, _) => assert_eq!(code.as_u16(), 404),
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_replace_wg_key_returns_aborted_unsupported() {
        // En MVP G.3, warren-api n'expose pas l'endpoint rotate. Le
        // backend retourne explicitement `Aborted` plutôt que de panic
        // ou de prétendre succeed silencieusement. À updater dès que
        // `PUT /v1/devices/{id}` côté serveur est ajouté (Phase G.5+).
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[82u8; 32]);
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);

        let new_pubkey = WgPrivateKey::from([83u8; 32]).public_key();
        let err = backend
            .replace_wg_key("acc".to_owned(), "anyid".to_owned(), new_pubkey)
            .await
            .expect_err("must fail unsupported");
        match err {
            rest::Error::Aborted => {}
            other => panic!("expected Aborted, got {other:?}"),
        }
    }
}
