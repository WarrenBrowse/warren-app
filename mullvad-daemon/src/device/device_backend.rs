//! Abstract trait for the device-level operations that used to go
//! through `DevicesProxy` to `api.mullvad.net`.
//!
//! Device-side counterpart of [`super::account_backend`]: lets us
//! dispatch at boot between:
//! - [`RemoteDeviceBackend`]: thin wrap over the legacy Mullvad
//!   `DevicesProxy`. Behavior strictly identical in non-`local`.
//! - [`LocalDeviceBackend`]: stateful POC (memory-only HashMap) that
//!   serves data consistent with the bootstrapped `device.json`,
//!   without any network call.
//!
//! Scope of 5 methods: `create`, `get`, `list`, `remove`,
//! `replace_wg_key`. `LocalDeviceBackend` **never** touches
//! `device.json` — `DeviceCacher` owns the file (see
//! recon C.2 § Q8 DeviceCacher conflict). The internal HashMap is
//! only a *shadow lookup* for the simulated API requests.

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

/// Type alias for the futures returned by the trait. See doc
/// [`super::account_backend::BoxFut`] for the `'static` rationale.
pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Abstract backend for the 5 device-level operations that
/// historically went through `DevicesProxy`. See module doc for the
/// account-side counterpart.
pub trait WarrenDeviceBackend: Send + Sync {
    /// Creates a device for the given account, tied to the provided
    /// WireGuard pubkey. Returns the produced device + the
    /// `AssociatedAddresses` allocated for this client. Idempotent on
    /// the local side: a repeated call with the same wg pubkey returns
    /// the same `Device` (same `id`).
    fn create(
        &self,
        account: AccountNumber,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<(Device, AssociatedAddresses), rest::Error>>;

    /// Fetches a device by its id. Returns `Err(rest::Error::ApiError(NOT_FOUND))`
    /// if the id is unknown (= 404 on the backend side).
    fn get(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<Device, rest::Error>>;

    /// Lists all devices on the account. On the local side, contains
    /// at most the single bootstrapped device (= single-device POC
    /// per identity).
    fn list(&self, account: AccountNumber) -> BoxFut<Result<Vec<Device>, rest::Error>>;

    /// Removes a device by its id. Idempotent: an already-absent id
    /// returns `Ok(())`.
    fn remove(&self, account: AccountNumber, id: DeviceId) -> BoxFut<Result<(), rest::Error>>;

    /// Rotation of the WireGuard pubkey of an existing device. Returns
    /// the `AssociatedAddresses` of the device — which MUST remain
    /// **identical** to those initially allocated at `create`
    /// (anti-regression: if we re-derived from the new pubkey,
    /// the Warren exit tunnel would reject the client = IP mismatch).
    fn replace_wg_key(
        &self,
        account: AccountNumber,
        id: DeviceId,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>>;
}

/// Thin wrap of the Mullvad `DevicesProxy`. Direct forward, behavior
/// strictly identical to the pre-Warren-fork legacy path.
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

/// Internal entry of the `LocalDeviceBackend` HashMap. Stores the
/// public `Device` (returned by `create`/`get`/`list`) and the
/// `AssociatedAddresses` allocated (= persistent for the device
/// lifetime, independent of the wg pubkey which can rotate via
/// `replace_wg_key`).
#[derive(Debug, Clone)]
struct LocalDeviceEntry {
    device: Device,
    addresses: AssociatedAddresses,
}

/// POC backend that maintains a shadow `HashMap<DeviceId, LocalDeviceEntry>`
/// in memory. **Never** touches `device.json`: the
/// [`super::DeviceCacher`] owns the file (see recon C.2 § Q8).
///
/// The HashMap is seeded from [`PrivateDeviceState`] at boot via
/// [`Self::from_state`] so that `list` immediately returns the
/// device bootstrapped in local mode. Mutations (`create`/`remove`/
/// `replace_wg_key`) update the HashMap but the caller remains
/// responsible for disk writes via `DeviceCacher::write` —
/// consistency is guaranteed by the call-backend -> set-cacher
/// sequence in [`super::AccountManager`].
#[derive(Clone)]
pub struct LocalDeviceBackend {
    inner: Arc<Mutex<HashMap<DeviceId, LocalDeviceEntry>>>,
}

impl LocalDeviceBackend {
    /// Builds an empty backend (= no known device). Useful for
    /// operation tests that do not need a seed.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Builds a backend whose HashMap is seeded from the provided
    /// state — typically `PrivateDeviceState` read by `DeviceCacher`
    /// at boot. If the state is `LoggedIn`, the single device is inserted.
    /// Otherwise (LoggedOut/Revoked), the HashMap starts empty.
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

/// Derives a stable POC `DeviceId` from the wg pubkey (= 32 bytes).
/// Prefixed `warren-local-` to distinguish from Mullvad UUIDs.
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

/// Derives a POC `AssociatedAddresses` from the wg pubkey bytes
/// to avoid trivial IP collisions between clients on the
/// same exit. Ranges:
/// - IPv4 in `10.64.0.0/10` (Mullvad-compatible).
/// - IPv6 in `fc00:bbbb::/32` (arbitrary POC ULA).
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

            // Idempotence: if an entry already exists for this id
            // (= same wg pubkey), return the existing device. The
            // caller sometimes calls `create` multiple times during a
            // re-login after deliberate logout.
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
            // Idempotent: `remove` of an already-absent id returns Ok.
            // Consistent with the gRPC `RemoveDevice` semantics on the UI side.
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
            // **Critical regression** (see recon C.2 § 6): NEVER
            // re-derive the addresses from the new wg pubkey.
            // The Warren exit allocated a tunnel IP for this
            // identity at the initial `create` — wg key rotation must
            // not change the IP, otherwise the tunnel becomes unusable.
            entry.device.pubkey = pubkey;
            Ok(entry.addresses.clone())
        })
    }
}

/// Warren-Remote backend — Phase G.3.b — implements
/// [`WarrenDeviceBackend`] via the `warren-api-client` client that
/// talks to the warren-api server.
///
/// Enabled in `warren_mode = true && warren_local_account = false` mode
/// (= 3rd branch of the dispatch in `device/mod.rs`).
///
/// Wire mapping:
/// - [`warren_api_client::Device`] <-> [`mullvad_types::device::Device`]
///   via [`map_device_response`].
/// - [`WgPublicKey`] <-> `wg_pubkey_hex` (32 bytes <-> 64 hex chars).
/// - [`AssociatedAddresses`]: warren-api does not return them in MVP,
///   we emit a fixed stub ([`stub_associated_addresses`]). M5+
///   refinement: warren-api will allocate real IPs (10.66.x.y / fc00:bbbb::x).
/// - `replace_wg_key`: not supported (warren-api does not yet expose
///   the rotate endpoint). Returns `rest::Error::Aborted` until
///   G.3.b+ adds `PUT /v1/devices/{id}` server-side.
#[derive(Clone)]
pub struct WarrenRemoteDeviceBackend {
    client: Arc<warren_api_client::WarrenApiClient>,
}

impl WarrenRemoteDeviceBackend {
    /// Builds a backend from a `WarrenApiClient` configured at
    /// boot (= Warren signer identity + warren-api URL).
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
        id: DeviceId,
        pubkey: WgPublicKey,
    ) -> BoxFut<Result<AssociatedAddresses, rest::Error>> {
        // Phase G.5.b: signed `PUT /v1/devices/{id}`. The `id` is
        // preserved server-side (see trait contract: wg rotation
        // does NOT change the identifier). The backend returns the
        // same stub addresses (= warren-api does not provide
        // IP allocation — see `stub_associated_addresses` doc).
        let client = self.client.clone();
        let new_wg_pubkey_hex = hex::encode(pubkey.as_bytes());
        Box::pin(async move {
            client
                .rotate_device_wg_key(&id, &new_wg_pubkey_hex)
                .await
                .map_err(super::account_backend::map_client_error)?;
            Ok(stub_associated_addresses())
        })
    }
}

/// Maps a `warren_api_client::Device` (wire JSON) to
/// [`mullvad_types::device::Device`].
///
/// Possible errors:
/// - `wg_pubkey_hex` invalid (not 32 bytes after decode) -> `Aborted`.
/// - `created_at` overflows `i64` (= year > 9 billion) -> `Aborted`.
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

/// Stub `AssociatedAddresses` — warren-api does not provide IP
/// allocation in MVP. Fixed Mullvad-style addresses so that
/// `PrivateDevice::try_from_device` on the caller side can build
/// a valid device. In warren-mode, the tunnel data plane goes
/// through `warren-iroh-tunnel` which does NOT use these WireGuard
/// IPs — so the absence of real allocation is not a blocker for
/// the MVP chain.
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
        // Critical regression: the `Device.pubkey` returned by
        // `create` MUST be the wg pubkey provided as input, otherwise
        // the `try_from_device` path on the caller side
        // (PrivateDevice::try_from_device, device/mod.rs:228) fails
        // with `Error::InvalidDevice` which crashes the login flow.
        let backend = LocalDeviceBackend::empty();
        let pubkey = fixed_wg_pubkey(7);

        let (device, _addrs) = backend
            .create("acc".to_owned(), pubkey.clone())
            .await
            .expect("create must succeed locally");

        assert_eq!(
            device.pubkey.as_bytes(),
            pubkey.as_bytes(),
            "Device.pubkey MUST match the input wg pubkey (otherwise try_from_device fails)"
        );
    }

    #[tokio::test]
    async fn local_create_idempotent_for_same_wg_pubkey() {
        // Critical regression: a re-login (= same mnemonic -> same
        // wg privkey regenerated the same way? no, wg privkey is
        // random on each generate_for_account; but if the caller
        // calls `create` twice with the same pubkey, we must not
        // create two divergent entries). Indirect test of
        // HashMap idempotence by pubkey.
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
            "idempotent create must return the same DeviceId for the same wg pubkey"
        );
        assert_eq!(
            a1.ipv4_address, a2.ipv4_address,
            "addresses must not change between two idempotent creates"
        );
    }

    #[tokio::test]
    async fn local_list_returns_seeded_device_after_init_from_state() {
        // Critical regression: if the seed from `PrivateDeviceState`
        // does not work, `mullvad-cli device list` would display 0
        // devices while the user just bootstrapped their mnemonic
        // = UX breakage. The test simulates this path by building a
        // LoggedIn state and asserts that `list` sees it.
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

        assert_eq!(devices.len(), 1, "list must see the seeded device");
        assert_eq!(devices[0].id, "warren-local-abcdef00");
    }

    #[tokio::test]
    async fn local_get_returns_not_found_after_remove() {
        // Critical regression: if `remove` were a no-op, the state
        // machine would never trigger `revoke_device` after a
        // logout = serious bug (user stays "logged in" perpetually).
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
            "get after remove must return NOT_FOUND, got {result:?}"
        );
    }

    #[tokio::test]
    async fn local_replace_wg_key_changes_pubkey_but_preserves_addresses() {
        // Critical regression D2/C.2: if `replace_wg_key` re-derived
        // the addresses from the new pubkey, the Warren exit tunnel
        // would reject the client (IP mismatch on the exit allocation
        // side). This is exactly the invisible bug this test
        // shields — without the `addresses_before == addresses_after`
        // assertion, the regression would go unnoticed until prod deployment.
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
            "replace_wg_key MUST preserve ipv4 (otherwise exit IP mismatch)"
        );
        assert_eq!(
            addresses_before.ipv6_address, addresses_after.ipv6_address,
            "replace_wg_key MUST preserve ipv6"
        );

        // But the device pubkey must have rotated:
        let device_after = backend.get("acc".to_owned(), device.id).await.unwrap();
        assert_eq!(
            device_after.pubkey.as_bytes(),
            pubkey_new.as_bytes(),
            "Device.pubkey must reflect the new wg pubkey post-rotate"
        );
        assert_ne!(
            device_after.pubkey.as_bytes(),
            pubkey_initial.as_bytes(),
            "Device.pubkey must NOT be the old pubkey"
        );
    }

    // ===================================================================
    // WarrenRemoteDeviceBackend — Phase G.3.b tests E2E.
    // See account_backend::tests for the spawn_warren_api pattern.
    // ===================================================================

    use ed25519_dalek::SigningKey;
    use std::sync::Arc as TestArc;
    use warren_api_client::WarrenApiClient;

    /// 64-char hex wg_pubkey derived from a fixed seed.
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
        // Nominal case: create sends signed POST /v1/devices, the server
        // upserts the device, the client maps it to mullvad_types::Device.
        // The returned addresses are stubs (warren-api does not allocate
        // them in MVP).
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

        // The wg pubkey of the returned Device == the one sent.
        assert_eq!(device.pubkey.as_bytes(), wg_pubkey.as_bytes());
        // ID 32 hex chars (see compute_device_id on warren-api side).
        assert_eq!(device.id.len(), 32);
        // The server does have the device.
        let server_devices = state.devices.list_for_owner(&owner_pubkey_hex);
        assert_eq!(server_devices.len(), 1);
        assert_eq!(server_devices[0].id, device.id);
        // Stub addresses (to be refined M5+).
        assert_eq!(
            addresses.ipv4_address.to_string(),
            "10.66.0.1/32",
            "expected v4 stub in MVP"
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
        // Pre-populates via the warren-api store directly (= equivalent
        // to a prior `create`, test shortcut).
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
        // Critical regression: if we lose the 404 in the mapping, the
        // caller (`AccountManager::handle_get_device`) cannot
        // distinguish "device removed/revoked" from a transport error.
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
        // Anti-cross-tenant: list signed by A contains only A's
        // devices (ensured by the server via identity middleware).
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
        assert_eq!(list.len(), 1, "A must see only its own devices");
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
            "device must have disappeared from the server-side store"
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
    async fn warren_remote_device_replace_wg_key_rotates_and_preserves_id() {
        // Phase G.5.b: wg rotation via warren-api preserves the
        // server-side device_id (= Mullvad contract), and the backend
        // returns the addresses (MVP stub).
        let (api_url, state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[82u8; 32]);
        let owner_pubkey_hex = hex::encode(key.verifying_key().as_bytes());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let server_device =
            state
                .devices
                .register(&owner_pubkey_hex, &fixed_wg_pubkey_hex(83), false, now);

        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);
        let new_wg_pubkey = WgPrivateKey::from([84u8; 32]).public_key();

        let addresses = backend
            .replace_wg_key(
                "acc".to_owned(),
                server_device.id.clone(),
                new_wg_pubkey.clone(),
            )
            .await
            .expect("rotate OK");
        // Stub addresses (MVP).
        assert_eq!(addresses.ipv4_address.to_string(), "10.66.0.1/32");

        // The server-side device has the new wg_pubkey, and the id
        // is preserved.
        let updated = state
            .devices
            .get_for_owner(&owner_pubkey_hex, &server_device.id)
            .expect("device still present");
        assert_eq!(updated.id, server_device.id, "id MUST be preserved");
        assert_eq!(updated.wg_pubkey_hex, hex::encode(new_wg_pubkey.as_bytes()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warren_remote_device_replace_wg_key_returns_apierror_404_unknown_id() {
        // Regression: if the id does not exist, the mapping must
        // propagate 404 (= caller can detect "device revoked" and
        // trigger re-login).
        let (api_url, _state) = spawn_warren_api().await;
        let key = SigningKey::from_bytes(&[85u8; 32]);
        let client = WarrenApiClient::new(api_url, key);
        let backend = WarrenRemoteDeviceBackend::new(client);
        let new_pubkey = WgPrivateKey::from([86u8; 32]).public_key();

        let err = backend
            .replace_wg_key(
                "acc".to_owned(),
                "deadbeefdeadbeefdeadbeefdeadbeef".to_owned(),
                new_pubkey,
            )
            .await
            .expect_err("must fail 404");
        match err {
            rest::Error::ApiError(code, _) => assert_eq!(code.as_u16(), 404),
            other => panic!("expected ApiError(404, _), got {other:?}"),
        }
    }
}
