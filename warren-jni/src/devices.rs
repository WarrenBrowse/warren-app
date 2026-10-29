//! The account's devices (warren-core doc 115) on Android: this install as a
//! device of its account, registering as `android` with the app's product
//! version, and the process's [`DeviceRegistry`], which the token and
//! entitlement mints tell of a revocation and the JNI exports answer from.

use std::path::Path;
use std::sync::OnceLock;

use warren_api::DevicePlatform;
use warren_standing::device::InstallDevice;
use warren_standing::device_registry::DeviceRegistry;

use crate::redial::SessionStatus;
use crate::status_watch::StatusCell;

static DEVICES: OnceLock<DeviceRegistry> = OnceLock::new();

/// The install's device, its identity kept in `dir` beside the token
/// rotation.
pub(crate) fn install_device_in(dir: &Path) -> InstallDevice {
    InstallDevice::new(dir, DevicePlatform::Android, mullvad_version::VERSION)
}

/// Keeps the install's device identity in `dir`. The first caller wins, like
/// the files directory it comes from.
#[cfg_attr(
    not(all(target_os = "android", feature = "tunnel")),
    expect(dead_code, reason = "reached from the Android JNI exports only")
)]
pub(crate) fn install_in(dir: &Path) {
    registry().install(install_device_in(dir));
}

/// The process's registry, with no install device until [`install_in`] ran:
/// one reached before then takes the device once it is installed.
pub(crate) fn registry() -> &'static DeviceRegistry {
    DEVICES.get_or_init(|| DeviceRegistry::new(None))
}

/// Resolves once `registry` revokes this device, the session's status then
/// reading [`SessionStatus::DeviceRevoked`]: a running tunnel ends on it as it
/// does on a ban, since the tokens already minted would otherwise keep it
/// admitted for their whole prefetch horizon.
pub(crate) async fn session_revoked(registry: &DeviceRegistry, status: &StatusCell) {
    registry.revocation_arrives().await;
    status.store(SessionStatus::DeviceRevoked as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_revocation_ends_the_session_on_its_own_status() {
        let registry = DeviceRegistry::new(None);
        let status = StatusCell::new(SessionStatus::Connected as i32);
        let ended = session_revoked(&registry, &status);
        tokio::pin!(ended);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut ended)
                .await
                .is_err(),
            "a device still registered keeps its session"
        );

        registry.on_revoked();
        ended.await;

        assert_eq!(
            status.load(),
            10,
            "WarrenQuinnAdapter reads STATUS_DEVICE_REVOKED as this value"
        );
    }

    #[test]
    fn the_install_registers_as_android_with_the_product_version() {
        let dir = tempfile::tempdir().expect("tempdir");

        let registration = install_device_in(dir.path()).registration();

        assert_eq!(registration.platform, DevicePlatform::Android);
        assert_eq!(
            registration.app_version,
            warren_standing::device::registry_app_version(mullvad_version::VERSION)
        );
    }
}
