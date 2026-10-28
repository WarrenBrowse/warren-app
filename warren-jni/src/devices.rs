//! The account's devices (warren-core doc 115) on Android: this install as a
//! device of its account, registering as `android` with the app's product
//! version, and the process's [`DeviceRegistry`], which the token and
//! entitlement mints tell of a revocation and the JNI exports answer from.

use std::path::Path;
use std::sync::OnceLock;

use warren_api::DevicePlatform;
use warren_standing::device::InstallDevice;
use warren_standing::device_registry::DeviceRegistry;

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
    let _ = DEVICES.set(DeviceRegistry::new(Some(install_device_in(dir))));
}

/// The process's registry, with no install device until [`install_in`] ran.
#[cfg_attr(
    not(all(target_os = "android", feature = "tunnel")),
    expect(dead_code, reason = "reached from the Android JNI exports only")
)]
pub(crate) fn registry() -> &'static DeviceRegistry {
    DEVICES.get_or_init(|| DeviceRegistry::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;

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
