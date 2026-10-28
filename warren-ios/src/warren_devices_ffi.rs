//! The account's devices (warren-core doc 115) on iOS.
//!
//! iOS runs two processes, and both reach the registry: the Network
//! Extension's token and entitlement refreshes register the install and learn
//! a revocation, and the app lists, renames and revokes devices. Each process
//! configures its own [`DeviceRegistry`] with [`warren_device_configure`],
//! over the same identity file in the App Group container, so both ask as the
//! same device. A process that never configured it asks as no device.
//!
//! The list, rename and revoke calls answer the JSON envelopes of
//! `warren_standing::device_registry`; each returned pointer is freed once
//! with `warren_wallet_free_mnemonic`. They block on the shared runtime: the
//! Swift side calls them off the main thread.

use std::path::Path;
use std::sync::OnceLock;

use warren_standing::device::{DevicePlatform, InstallDevice};
use warren_standing::device_registry::DeviceRegistry;

static DEVICES: OnceLock<DeviceRegistry> = OnceLock::new();

/// The install's device, its identity kept in `dir` (the App Group
/// container, beside the token rotation), registering as `ios` with
/// `app_version`.
pub(crate) fn install_device_in(dir: &Path, app_version: &str) -> InstallDevice {
    InstallDevice::new(dir, DevicePlatform::Ios, app_version)
}

/// Configures this process's registry. The first call wins: the identity
/// directory and the version do not change while a process runs.
#[cfg_attr(
    not(target_os = "ios"),
    expect(dead_code, reason = "called over the iOS FFI")
)]
fn configure(dir: &Path, app_version: &str) {
    registry().install(install_device_in(dir, app_version));
}

/// The process's registry, with no install device until it was configured.
pub(crate) fn registry() -> &'static DeviceRegistry {
    DEVICES.get_or_init(|| DeviceRegistry::new(None))
}

#[cfg(target_os = "ios")]
mod ffi {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    use warren_api::WarrenApiClient;
    use warren_api::reqwest_transport::ReqwestTransport;
    use warren_identity::WarrenIdentity;
    use warren_standing::device_registry::failure;
    use zeroize::Zeroizing;

    use super::{configure, registry};

    const SEED_LEN: usize = 32;

    /// # Safety
    /// `ptr`, when non-null, must be a valid null-terminated C string.
    unsafe fn read_str(ptr: *const c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        // SAFETY: `ptr` is a valid null-terminated C string (precondition).
        unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .ok()
            .map(str::to_owned)
    }

    fn into_cstring(s: String) -> *mut c_char {
        CString::new(s).map_or(std::ptr::null_mut(), CString::into_raw)
    }

    /// Runs one registry call as the wallet of `seed` and answers its
    /// envelope.
    ///
    /// # Safety
    /// `seed`, when non-null, must point to at least 32 readable bytes.
    unsafe fn device_call<F, Fut>(seed: *const u8, call: F) -> *mut c_char
    where
        F: FnOnce(WarrenApiClient<ReqwestTransport>) -> Fut,
        Fut: std::future::Future<Output = String>,
    {
        if seed.is_null() {
            return into_cstring(failure("no_wallet"));
        }
        let mut bytes = Zeroizing::new([0u8; SEED_LEN]);
        // SAFETY: `seed` points to at least SEED_LEN readable bytes (precondition).
        unsafe { std::ptr::copy_nonoverlapping(seed, bytes.as_mut_ptr(), SEED_LEN) };
        let Ok(runtime) = crate::warren_ios_runtime() else {
            return into_cstring(failure("unavailable"));
        };
        let client = warren_standing::server_clock::attach(WarrenApiClient::new(
            warren_product_env::API_URL.to_owned(),
            WarrenIdentity::from_seed(&bytes),
            ReqwestTransport::new(),
        ));
        into_cstring(runtime.block_on(call(client)))
    }

    /// Configures this process's device: its identity kept in `state_dir`
    /// (the App Group container, so the app and the extension ask as one
    /// device), registering as `ios` with `app_version`
    /// (`CFBundleShortVersionString`). Call it once per process, before the
    /// first tunnel start or registry call; later calls change nothing.
    ///
    /// # Safety
    /// Both arguments, when non-null, must be valid null-terminated C strings.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn warren_device_configure(
        state_dir: *const c_char,
        app_version: *const c_char,
    ) {
        crate::ffi_guard((), || {
            // SAFETY: the arguments uphold the documented precondition.
            let (Some(dir), Some(version)) = (unsafe { read_str(state_dir) }, unsafe {
                read_str(app_version)
            }) else {
                return;
            };
            if !dir.is_empty() {
                configure(std::path::Path::new(&dir), &version);
            }
        });
    }

    /// The account's devices, this one marked:
    /// `{"ok":true,"devices":[...],"older_client_seen_on":"YYYY-MM-DD"|null}`
    /// or `{"ok":false,"error":...}`.
    ///
    /// # Safety
    /// `seed`, when non-null, must point to at least 32 readable bytes. The
    /// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn warren_device_list(seed: *const u8) -> *mut c_char {
        crate::ffi_guard(std::ptr::null_mut(), || {
            // SAFETY: `seed` upholds the documented precondition.
            unsafe {
                device_call(
                    seed,
                    |client| async move { registry().list_json(&client).await },
                )
            }
        })
    }

    /// Renames the account's device `device_id`:
    /// `{"ok":true,"this_device":bool}` or `{"ok":false,"error":...}`.
    ///
    /// # Safety
    /// `seed`, when non-null, must point to at least 32 readable bytes;
    /// `device_id` and `name`, when non-null, must be valid null-terminated C
    /// strings. The returned pointer must be freed once via
    /// `warren_wallet_free_mnemonic`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn warren_device_rename(
        seed: *const u8,
        device_id: *const c_char,
        name: *const c_char,
    ) -> *mut c_char {
        crate::ffi_guard(std::ptr::null_mut(), || {
            // SAFETY: the arguments uphold the documented precondition.
            let (Some(id), Some(name)) =
                (unsafe { read_str(device_id) }, unsafe { read_str(name) })
            else {
                return into_cstring(failure("invalid_device"));
            };
            // SAFETY: `seed` upholds the documented precondition.
            unsafe {
                device_call(seed, |client| async move {
                    registry().rename_json(&client, &id, &name).await
                })
            }
        })
    }

    /// Revokes the account's device `device_id`:
    /// `{"ok":true,"this_device":bool}` or `{"ok":false,"error":...}`.
    /// Revoking this device is a logout, after which
    /// [`warren_device_revoked`] reads `true`.
    ///
    /// # Safety
    /// `seed`, when non-null, must point to at least 32 readable bytes;
    /// `device_id`, when non-null, must be a valid null-terminated C string.
    /// The returned pointer must be freed once via
    /// `warren_wallet_free_mnemonic`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn warren_device_revoke(
        seed: *const u8,
        device_id: *const c_char,
    ) -> *mut c_char {
        crate::ffi_guard(std::ptr::null_mut(), || {
            // SAFETY: the argument upholds the documented precondition.
            let Some(id) = (unsafe { read_str(device_id) }) else {
                return into_cstring(failure("invalid_device"));
            };
            // SAFETY: `seed` upholds the documented precondition.
            unsafe {
                device_call(seed, |client| async move {
                    registry().revoke_json(&client, &id).await
                })
            }
        })
    }

    /// Whether the registry revoked this device in this process since
    /// [`warren_device_forget_revocation`]. In the extension the tunnel also
    /// ends on it with `EventDeviceRevoked`.
    #[unsafe(no_mangle)]
    pub extern "C" fn warren_device_revoked() -> bool {
        crate::ffi_guard(false, || registry().revoked())
    }

    /// The revocation was taken in (the user left the revoked screen, or
    /// logged in again).
    #[unsafe(no_mangle)]
    pub extern "C" fn warren_device_forget_revocation() {
        crate::ffi_guard((), || registry().forget_revocation());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_registers_as_ios_with_the_version_the_app_reports() {
        let dir = tempfile::tempdir().expect("tempdir");

        let registration = install_device_in(dir.path(), "2026.3").registration();

        assert_eq!(registration.platform, DevicePlatform::Ios);
        assert_eq!(registration.app_version, "2026.3");
    }
}
