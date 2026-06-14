// The iOS-specific runtime + API-client code only compiles on `target_os =
// "ios"`.  The FFI modules (`warren_tunnel_ffi`, `warren_wallet_ffi`) are
// also included under `test` so that unit tests can run on the macOS host
// without an iOS cross-compilation toolchain - their deps (bip39,
// ed25519-dalek, warren-identity, zeroize, std) are all cross-platform.
#![cfg(any(target_os = "ios", test))]
// AUDIT_COMPLET.md M-4: the blanket
// `#![allow(clippy::undocumented_unsafe_blocks)]` was removed.
// The single current `unsafe` site (`get_string` below) carries an
// explicit `// Safety:` comment; any future `unsafe` block added in
// this crate must do the same.

// Warren-side FFI modules.  `warren_tunnel_ffi` and `warren_wallet_ffi`
// compile on any platform; the others use iOS-only libc / tokio / mullvad
// deps and stay iOS-gated.
#[cfg(any(target_os = "ios", test))]
mod warren_tunnel_ffi;
// Multi-hop directory verification + circuit selection. Needs the
// warren-multihop descriptor types pulled by the `tunnel` feature, and is
// consumed only by the tunnel data plane, so it is gated the same way.
#[cfg(all(target_os = "ios", feature = "tunnel"))]
mod warren_multihop_directory;
// Persistent anti-rollback high-water mark for the multi-hop directory
// generation. Backs the iOS tunnel path; also compiled under `test` so its
// pure std::fs round-trip tests run on the host (`cargo test`).
#[cfg(any(all(target_os = "ios", feature = "tunnel"), test))]
mod warren_multihop_generation;
// Trust-on-first-use store for exit Ed25519 pubkeys. Pure logic + JSON
// persistence; compiled under `test` so its round-trip + verdict tests run
// on the host, and on iOS where the tunnel FFI enforces the pin on connect.
#[cfg(any(target_os = "ios", test))]
mod warren_pin_store;
#[cfg(any(target_os = "ios", test))]
mod warren_wallet_ffi;

// iOS-only modules that reference libc, tokio, mullvad-api, etc.
#[cfg(target_os = "ios")]
mod warren_account_ffi;
#[cfg(target_os = "ios")]
mod warren_multihop_ffi;
#[cfg(target_os = "ios")]
mod warren_natpmp_ffi;

// Upstream Mullvad API client retained transitionally. Each call site here
// still calls into `mullvad-api` (account number flows, device management,
// problem reports, StoreKit). Migrating those flows to `warren-api-client`
// is a dedicated follow-up (~16 source files): the Warren wallet auth uses
// `X-Warren-*` canonical-message signatures (see warren-core/crates/warren-
// identity::auth), which is a separate flow from Mullvad's account-number
// token model.
#[cfg(target_os = "ios")]
mod api_client;
#[cfg(target_os = "ios")]
mod logging;

#[cfg(target_os = "ios")]
use libc::c_char;
#[cfg(target_os = "ios")]
use std::ffi::CStr;
#[cfg(target_os = "ios")]
use std::sync::OnceLock;
#[cfg(target_os = "ios")]
use tokio::runtime::{Builder, Handle, Runtime};

#[cfg(target_os = "ios")]
#[repr(C)]
pub struct ProxyHandle {
    pub context: *mut std::ffi::c_void,
    pub port: u16,
}

#[cfg(target_os = "ios")]
static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

#[cfg(target_os = "ios")]
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
#[cfg(target_os = "ios")]
unsafe fn get_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // Safety: See function doc comment.
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().map(ToOwned::to_owned).unwrap_or_default()
}

/// Runs an FFI body under `catch_unwind` so a panic fails closed by
/// returning `sentinel` instead of unwinding across the C ABI, which is
/// undefined behaviour. Used at Warren `extern "C"` entry points whose
/// bodies do non-trivial work (network, serde) where a future regression
/// could panic.
#[cfg(target_os = "ios")]
pub(crate) fn ffi_guard<R>(sentinel: R, body: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(_) => sentinel,
    }
}
