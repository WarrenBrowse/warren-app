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
}
