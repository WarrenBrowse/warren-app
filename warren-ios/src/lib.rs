#![cfg(target_os = "ios")]
// AUDIT_COMPLET.md M-4: the blanket
// `#![allow(clippy::undocumented_unsafe_blocks)]` was removed.
// The single current `unsafe` site (`get_string` below) carries an
// explicit `// Safety:` comment; any future `unsafe` block added in
// this crate must do the same.
use libc::c_char;
use std::ffi::CStr;
use std::sync::OnceLock;
use tokio::runtime::{Builder, Handle, Runtime};

// Warren-side FFI modules (skeletons; implementations land in Session C.3.deep
// follow-up once the warren-tunnel / warren-identity / warren-multihop /
// warren-natpmp-client integration is wired through Swift wrappers).
mod warren_multihop_ffi;
mod warren_natpmp_ffi;
mod warren_tunnel_ffi;
mod warren_wallet_ffi;

// Upstream Mullvad API client retained transitionally. Each call site here
// still calls into `mullvad-api` (account number flows, device management,
// problem reports, StoreKit). Migrating those flows to `warren-api-client`
// is a dedicated follow-up (~16 source files): the Warren wallet auth uses
// `X-Warren-*` canonical-message signatures (see warren-core/crates/warren-
// identity::auth), which is a separate flow from Mullvad's account-number
// token model.
mod api_client;
mod logging;

#[repr(C)]
pub struct ProxyHandle {
    pub context: *mut std::ffi::c_void,
    pub port: u16,
}

static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

fn warren_ios_runtime() -> Result<Handle, String> {
    match RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| ToString::to_string(&error))
    }) {
        Ok(runtime) => Ok(runtime.handle().clone()),
        Err(error) => Err(error.clone()),
    }
}

/// Try to convert a C string to an owned [String]. if `ptr` is null, an empty [String] is
/// returned.
///
/// # Safety
/// - `ptr` must uphold all safety invariants as required by [CStr::from_ptr].
unsafe fn get_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // Safety: See function doc comment.
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().map(ToOwned::to_owned).unwrap_or_default()
}
