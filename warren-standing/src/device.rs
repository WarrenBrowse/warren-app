//! This install's device in the account's registry (warren-core doc 115): the
//! identity it registers under, kept beside the token rotation in the
//! install's own state directory, and the registration every token and
//! entitlement manager of the install sends.
//!
//! The account's device list names an install by this identity, so one drawn
//! anew at every start would add a line to the list at every start. Shared by
//! the desktop daemon, Android and iOS. The file has the name and the shape of
//! the SDK's own (`warren_sdk::kept_device_identity`), the serialized
//! [`DeviceIdentity`].

use std::path::{Path, PathBuf};

use warren_api::{DeviceIdentity, DevicePlatform, DeviceRegistration, HttpTransport, TokenManager};

/// File under a client's state directory that keeps the device identity.
pub const DEVICE_FILE: &str = "device";

/// The most characters the registry takes for an app version.
const APP_VERSION_MAX: usize = 32;

/// The version an install registers with, as the registry accepts it: 1 to
/// 32 characters of `[0-9A-Za-z.+-]`. A version the registry refuses fails
/// the registration, and every issue request then answers
/// `device_unregistered`, so a build that reports an odd version string must
/// still register as something.
#[must_use]
pub fn registry_app_version(version: &str) -> String {
    let kept: String = version
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'))
        .take(APP_VERSION_MAX)
        .collect();
    if kept.is_empty() {
        "0".to_owned()
    } else {
        kept
    }
}

/// This install as a device of its account: where its identity is kept, the
/// platform it runs on and the version it runs.
#[derive(Clone, Debug)]
pub struct InstallDevice {
    path: PathBuf,
    platform: DevicePlatform,
    app_version: String,
}

impl InstallDevice {
    /// The device whose identity is kept in `dir`, registering from
    /// `platform` with `app_version` (made acceptable to the registry by
    /// [`registry_app_version`]).
    #[must_use]
    pub fn new(dir: &Path, platform: DevicePlatform, app_version: &str) -> Self {
        Self {
            path: dir.join(DEVICE_FILE),
            platform,
            app_version: registry_app_version(app_version),
        }
    }

    /// The identity kept for this install, or one drawn and kept now. A
    /// directory it cannot write to leaves this process its own draw, and an
    /// unreadable file is replaced by a new draw.
    #[must_use]
    pub fn identity(&self) -> DeviceIdentity {
        if let Some(kept) = self.read() {
            return kept;
        }
        let drawn = DeviceIdentity::generate();
        if let Err(e) = serde_json::to_vec(&drawn)
            .map_err(std::io::Error::other)
            .and_then(|json| write_whole(&self.path, &json))
        {
            log::warn!("Warren device identity not kept, the next start registers anew: {e}");
        }
        drawn
    }

    /// What this install registers as: its kept identity, its platform and
    /// its version.
    #[must_use]
    pub fn registration(&self) -> DeviceRegistration {
        self.identity()
            .registration(self.platform, self.app_version.clone())
    }

    /// Keeps `name` for the next start, once the registry answered with it
    /// (a rename made from another device of the account) or the user renamed
    /// this device. Writes nothing when the kept name already reads `name`,
    /// and never keeps a name without the identifier it names.
    pub fn keep_name(&self, name: &str) {
        let Some(mut kept) = self.read() else {
            return;
        };
        if kept.name == name {
            return;
        }
        name.clone_into(&mut kept.name);
        if let Err(e) = serde_json::to_vec(&kept)
            .map_err(std::io::Error::other)
            .and_then(|json| write_whole(&self.path, &json))
        {
            log::warn!("Warren device name not kept: {e}");
        }
    }

    /// Erases the kept identity: the device was revoked, or logged out. The
    /// next [`Self::identity`] draws a new one, which the account lists as a
    /// new device.
    ///
    /// # Errors
    ///
    /// The error of the removal; an identity that is not there is already
    /// forgotten.
    pub fn forget(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    fn read(&self) -> Option<DeviceIdentity> {
        serde_json::from_slice(&std::fs::read(&self.path).ok()?).ok()
    }
}

/// `manager`, registering and asking as `device` (warren-core doc 115).
/// Without a device it asks as no device, the way clients before the
/// registry did.
#[must_use]
pub fn with_install_device<T: HttpTransport>(
    manager: TokenManager<T>,
    device: Option<&InstallDevice>,
) -> TokenManager<T> {
    match device {
        Some(device) => manager.with_device(device.registration()),
        None => manager,
    }
}

/// Writes `bytes` to a side file then renames it over `path`, because a torn
/// write would still parse as another identity.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    std::fs::write(&partial, bytes).and_then(|()| std::fs::rename(&partial, path))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use warren_api::{BlindingKey, DevicePlatform, TokenManager, WarrenApiClient};
    use warren_identity::WarrenIdentity;

    use super::{DEVICE_FILE, InstallDevice, registry_app_version, with_install_device};

    struct NoNetwork;

    impl warren_api::HttpTransport for NoNetwork {
        async fn execute(
            &self,
            _request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            panic!("these tests never reach the API")
        }
    }

    fn bare_manager() -> TokenManager<NoNetwork> {
        let seed = [0x42; 32];
        TokenManager::new(
            Arc::new(WarrenApiClient::new(
                "https://api.example.test",
                WarrenIdentity::from_seed(&seed),
                NoNetwork,
            )),
            BlindingKey::session(&seed),
        )
    }

    #[test]
    fn a_manager_kept_over_an_install_asks_as_its_kept_device() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let kept = device(dir.path()).identity();

        let manager = with_install_device(bare_manager(), Some(&device(dir.path())));

        assert_eq!(manager.device_name(), Some(kept.name));
        assert_eq!(
            with_install_device(bare_manager(), None).device_name(),
            None
        );
    }

    fn device(dir: &std::path::Path) -> InstallDevice {
        InstallDevice::new(dir, DevicePlatform::Macos, "1.1.55")
    }

    #[test]
    fn the_next_process_registers_under_the_identity_this_one_drew() {
        let dir = tempfile::tempdir().expect("a temp dir");

        let drawn = device(dir.path()).identity();

        assert_eq!(device(dir.path()).identity(), drawn);
    }

    #[test]
    fn a_registration_carries_the_kept_identity_the_platform_and_the_version() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let identity = device(dir.path()).identity();

        let registration = device(dir.path()).registration();

        assert_eq!(registration.device_id, identity.device_id);
        assert_eq!(registration.name, identity.name);
        assert_eq!(registration.platform, DevicePlatform::Macos);
        assert_eq!(registration.app_version, "1.1.55");
    }

    #[test]
    fn a_kept_name_is_registered_at_the_next_start_under_the_same_identifier() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let drawn = device(dir.path()).identity();

        device(dir.path()).keep_name("calm heron");

        let next = device(dir.path()).registration();
        assert_eq!(next.name, "calm heron");
        assert_eq!(next.device_id, drawn.device_id);
    }

    #[test]
    fn a_name_is_never_kept_without_an_identity_to_name() {
        let dir = tempfile::tempdir().expect("a temp dir");

        device(dir.path()).keep_name("calm heron");

        assert!(!dir.path().join(DEVICE_FILE).exists());
    }

    #[test]
    fn a_forgotten_identity_gives_way_to_a_new_device() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let drawn = device(dir.path()).identity();

        device(dir.path()).forget().expect("forgotten");

        assert_ne!(device(dir.path()).identity().device_id, drawn.device_id);
    }

    #[test]
    fn forgetting_an_identity_never_kept_succeeds() {
        let dir = tempfile::tempdir().expect("a temp dir");

        assert!(device(dir.path()).forget().is_ok());
    }

    #[test]
    fn an_unreadable_identity_is_drawn_again_and_kept() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join(DEVICE_FILE), b"{\"device_id\":\"short\"}").unwrap();

        let drawn = device(dir.path()).identity();

        assert_eq!(device(dir.path()).identity(), drawn);
    }

    #[test]
    fn an_identity_that_cannot_be_kept_is_still_handed_out() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let missing = dir.path().join("never-created");

        let one = device(&missing).identity();
        let other = device(&missing).identity();

        assert_ne!(one.device_id, other.device_id);
    }

    #[test]
    fn the_registry_takes_the_version_whatever_the_build_reports() {
        assert_eq!(registry_app_version("1.1.55"), "1.1.55");
        assert_eq!(
            registry_app_version("2026.3-beta1-dev-abc123"),
            "2026.3-beta1-dev-abc123"
        );
        assert_eq!(registry_app_version(" 1.2 (45) "), "1.245");
        assert_eq!(registry_app_version(&"9".repeat(40)), "9".repeat(32));
        assert_eq!(registry_app_version(""), "0");
    }
}
