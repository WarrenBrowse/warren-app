//! The account's devices (warren-core doc 115) as an app reaches them: this
//! install as a device of its account, whether the registry revoked it, and
//! the list, rename and revoke calls the device screens make. Shared by the
//! Android JNI and the iOS FFI, which hand its answers on as they are.
//!
//! The answers are JSON envelopes. A listed device is the contract's own
//! `DeviceView` serialization (`device_id`, `name`, `platform`,
//! `app_version`, `registered_on`, `last_seen_on`, `this_device`), so the app
//! reads what the registry answered. The list names the account's installs
//! and the days they were used: it goes to the account's own screens and never
//! to a log.

use serde_json::json;
use tokio::sync::watch;
use warren_api::{ClientError, DeviceId, HttpTransport, WarrenApiClient};

use crate::device::InstallDevice;

/// This install's device, and whether the registry revoked it.
pub struct DeviceRegistry {
    device: Option<InstallDevice>,
    revoked: watch::Sender<bool>,
}

impl DeviceRegistry {
    /// The registry calls of `device`, this install; `None` asks as no
    /// device (a client built before the install's directory is known).
    #[must_use]
    pub fn new(device: Option<InstallDevice>) -> Self {
        Self {
            device,
            revoked: watch::Sender::new(false),
        }
    }

    /// The install as a device of its account.
    #[must_use]
    pub fn device(&self) -> Option<&InstallDevice> {
        self.device.as_ref()
    }

    /// This install's device id, `None` without an install device.
    #[must_use]
    pub fn this(&self) -> Option<DeviceId> {
        self.device
            .as_ref()
            .map(|device| device.identity().device_id)
    }

    /// The registry revoked this device (from another device, or by its own
    /// logout): the identity is forgotten, so the next login registers as a
    /// new device, and [`Self::revoked`] reads `true` until the app forgets
    /// it.
    pub fn on_revoked(&self) {
        self.revoked.send_replace(true);
        if let Some(device) = &self.device
            && let Err(error) = device.forget()
        {
            log::warn!(
                "the revoked device identity could not be forgotten: {}",
                error.kind()
            );
        }
    }

    /// Whether the registry revoked this device since the app last forgot
    /// it.
    #[must_use]
    pub fn revoked(&self) -> bool {
        *self.revoked.borrow()
    }

    /// The app took the revocation in (the user left the revoked screen, or
    /// logged in again).
    pub fn forget_revocation(&self) {
        self.revoked.send_replace(false);
    }

    /// Resolves once this device is revoked, at once if it already is: a
    /// running tunnel ends its session on it, as it does on a ban.
    pub async fn revocation_arrives(&self) {
        let mut changes = self.revoked.subscribe();
        // The sender lives as long as `self`, so the wait cannot fail.
        let _ = changes.wait_for(|revoked| *revoked).await;
    }

    /// The account's devices, this one marked:
    /// `{"ok":true,"devices":[DeviceView...],"older_client_seen_on":"YYYY-MM-DD"|null}`.
    pub async fn list_json<T: HttpTransport>(&self, client: &WarrenApiClient<T>) -> String {
        match client.list_devices(self.this()).await {
            Ok(list) => json!({
                "ok": true,
                "devices": list.devices,
                "older_client_seen_on": list.older_client_seen_on,
            })
            .to_string(),
            Err(error) => self.refused(&error, "invalid_device"),
        }
    }

    /// Renames the account's device `id`: `{"ok":true,"this_device":bool}`.
    /// A rename of this device is kept for its next registration.
    pub async fn rename_json<T: HttpTransport>(
        &self,
        client: &WarrenApiClient<T>,
        id: &str,
        name: &str,
    ) -> String {
        let Ok(id) = DeviceId::try_from(id.to_owned()) else {
            return failure("invalid_device");
        };
        match client.rename_device(id, name).await {
            Ok(()) => {
                let this_device = self.this() == Some(id);
                if this_device && let Some(device) = &self.device {
                    device.keep_name(name);
                }
                json!({ "ok": true, "this_device": this_device }).to_string()
            }
            Err(error) => self.refused(&error, "invalid_name"),
        }
    }

    /// Revokes the account's device `id`: `{"ok":true,"this_device":bool}`.
    /// Revoking this device is a logout, and is taken as its revocation.
    pub async fn revoke_json<T: HttpTransport>(
        &self,
        client: &WarrenApiClient<T>,
        id: &str,
    ) -> String {
        let Ok(id) = DeviceId::try_from(id.to_owned()) else {
            return failure("invalid_device");
        };
        match client.revoke_device(id).await {
            Ok(()) => {
                let this_device = self.this() == Some(id);
                if this_device {
                    self.on_revoked();
                }
                json!({ "ok": true, "this_device": this_device }).to_string()
            }
            Err(error) => self.refused(&error, "invalid_device"),
        }
    }

    /// The envelope of a refused call. A 400 is the malformed field the call
    /// sent (`bad_request`); a `device_revoked` answer means this device was
    /// revoked, which is recorded as such.
    fn refused(&self, error: &ClientError, bad_request: &str) -> String {
        match error {
            ClientError::DeviceRevoked => {
                self.on_revoked();
                failure("revoked")
            }
            ClientError::ServerStatus { status: 404, .. } => failure("not_found"),
            ClientError::ServerStatus { status: 400, .. } => failure(bad_request),
            ClientError::ServerStatus { status, .. } => {
                log::warn!("device registry call refused by the server ({status})");
                failure("unavailable")
            }
            _ => {
                log::warn!("device registry call failed in transport");
                failure("unavailable")
            }
        }
    }
}

/// `{"ok":false,"error":code}`, the envelope of a call that could not
/// be made or was refused.
#[must_use]
pub fn failure(code: &str) -> String {
    json!({ "ok": false, "error": code }).to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::Value;
    use std::sync::Mutex;
    use warren_api::DevicePlatform;
    use warren_api::transport::{HttpRequest, HttpResponse, Method, TransportError};
    use warren_identity::WarrenIdentity;

    use super::*;

    /// The registry, answering every request with `status` and `body`
    /// (`{this}` stands for this device's id), and recording what it got.
    #[derive(Clone)]
    struct Registry {
        status: u16,
        body: String,
        seen: Arc<Mutex<Vec<(Method, String)>>>,
    }

    impl Registry {
        fn answering(status: u16, body: &str) -> Self {
            Self {
                status,
                body: body.to_owned(),
                seen: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl HttpTransport for Registry {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.seen
                .lock()
                .unwrap()
                .push((request.method, request.url));
            Ok(HttpResponse::new(
                self.status,
                self.body.clone().into_bytes(),
            ))
        }
    }

    fn client(registry: &Registry) -> WarrenApiClient<Registry> {
        WarrenApiClient::new(
            "https://api.example.test",
            WarrenIdentity::from_seed(&[0x51; 32]),
            registry.clone(),
        )
    }

    fn parsed(json: &str) -> Value {
        serde_json::from_str(json).expect("an envelope")
    }

    fn other_id() -> String {
        String::from(DeviceId::from_bytes([9; warren_api::DEVICE_ID_LEN]))
    }

    fn install_device_in(dir: &std::path::Path) -> InstallDevice {
        InstallDevice::new(dir, DevicePlatform::Ios, "1.1.55")
    }

    #[tokio::test]
    async fn a_revocation_wakes_whoever_waits_for_it() {
        let devices = Arc::new(DeviceRegistry::new(None));
        let waiting = Arc::clone(&devices);
        let woken = tokio::spawn(async move { waiting.revocation_arrives().await });
        tokio::task::yield_now().await;
        assert!(!woken.is_finished());

        devices.on_revoked();

        tokio::time::timeout(std::time::Duration::from_secs(5), woken)
            .await
            .expect("woken")
            .expect("joined");
    }

    #[tokio::test]
    async fn the_list_carries_every_field_the_registry_answered_and_marks_this_device() {
        let dir = tempfile::tempdir().expect("tempdir");
        let devices = DeviceRegistry::new(Some(install_device_in(dir.path())));
        let this = String::from(devices.this().expect("an install device"));
        let registry = Registry::answering(
            200,
            &format!(
                r#"{{"devices":[{{"device_id":"{this}","name":"calm heron","platform":"android","app_version":"1.1.55","registered_on":"2026-10-01","last_seen_on":"2026-10-28","this_device":true}}],"older_client_seen_on":"2026-10-27"}}"#
            ),
        );

        let answer = parsed(&devices.list_json(&client(&registry)).await);

        assert_eq!(answer["ok"], true);
        assert_eq!(answer["older_client_seen_on"], "2026-10-27");
        let device = &answer["devices"][0];
        assert_eq!(device["device_id"], this.as_str());
        assert_eq!(device["name"], "calm heron");
        assert_eq!(device["platform"], "android");
        assert_eq!(device["app_version"], "1.1.55");
        assert_eq!(device["registered_on"], "2026-10-01");
        assert_eq!(device["last_seen_on"], "2026-10-28");
        assert_eq!(device["this_device"], true);
        let (method, url) = registry.seen.lock().unwrap()[0].clone();
        assert_eq!(method, Method::Get);
        assert!(
            url.ends_with(&format!("/v1/devices?device_id={this}")),
            "{url}"
        );
    }

    #[tokio::test]
    async fn revoking_another_device_leaves_this_one_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let devices = DeviceRegistry::new(Some(install_device_in(dir.path())));
        let kept = devices.this();
        let registry = Registry::answering(204, "");

        let answer = parsed(&devices.revoke_json(&client(&registry), &other_id()).await);

        assert_eq!(answer, json!({ "ok": true, "this_device": false }));
        assert!(!devices.revoked());
        assert_eq!(devices.this(), kept);
        assert_eq!(registry.seen.lock().unwrap()[0].0, Method::Delete);
    }

    #[tokio::test]
    async fn revoking_this_device_is_its_revocation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let devices = DeviceRegistry::new(Some(install_device_in(dir.path())));
        let this = devices.this().expect("an install device");
        let registry = Registry::answering(204, "");

        let answer = parsed(
            &devices
                .revoke_json(&client(&registry), &String::from(this))
                .await,
        );

        assert_eq!(answer, json!({ "ok": true, "this_device": true }));
        assert!(devices.revoked());
        assert_ne!(devices.this(), Some(this), "the identity is forgotten");
    }

    #[tokio::test]
    async fn renaming_this_device_keeps_the_name_for_its_next_registration() {
        let dir = tempfile::tempdir().expect("tempdir");
        let devices = DeviceRegistry::new(Some(install_device_in(dir.path())));
        let this = String::from(devices.this().expect("an install device"));
        let registry = Registry::answering(204, "");

        let answer = parsed(
            &devices
                .rename_json(&client(&registry), &this, "kitchen phone")
                .await,
        );

        assert_eq!(answer, json!({ "ok": true, "this_device": true }));
        assert_eq!(
            install_device_in(dir.path()).identity().name,
            "kitchen phone"
        );
        assert_eq!(registry.seen.lock().unwrap()[0].0, Method::Patch);
    }

    #[tokio::test]
    async fn a_malformed_id_is_refused_before_any_request() {
        let devices = DeviceRegistry::new(None);
        let registry = Registry::answering(204, "");

        let revoked = parsed(&devices.revoke_json(&client(&registry), "short").await);
        let renamed = parsed(&devices.rename_json(&client(&registry), "short", "x").await);

        assert_eq!(revoked, json!({ "ok": false, "error": "invalid_device" }));
        assert_eq!(renamed, json!({ "ok": false, "error": "invalid_device" }));
        assert!(registry.seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn each_refusal_is_named() {
        let devices = DeviceRegistry::new(None);
        let cases = [
            (404, "", "not_found"),
            (503, "", "unavailable"),
            (400, r#"{"error":"invalid_device_name"}"#, "invalid_name"),
        ];
        for (status, body, code) in cases {
            let registry = Registry::answering(status, body);

            let answer = parsed(
                &devices
                    .rename_json(&client(&registry), &other_id(), "x")
                    .await,
            );

            assert_eq!(answer, json!({ "ok": false, "error": code }), "{status}");
        }
    }

    #[tokio::test]
    async fn a_caller_the_registry_revoked_records_its_revocation() {
        let devices = DeviceRegistry::new(None);
        let registry = Registry::answering(403, r#"{"error":"device_revoked"}"#);

        let answer = parsed(&devices.list_json(&client(&registry)).await);

        assert_eq!(answer, json!({ "ok": false, "error": "revoked" }));
        assert!(devices.revoked());
    }

    #[test]
    fn a_revocation_kotlin_took_in_is_forgotten() {
        let devices = DeviceRegistry::new(None);
        devices.on_revoked();

        devices.forget_revocation();

        assert!(!devices.revoked());
    }
}
