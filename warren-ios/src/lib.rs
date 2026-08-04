// The iOS-specific runtime + API-client code only compiles on `target_os =
// "ios"`.  The FFI modules (`warren_tunnel_ffi`, `warren_wallet_ffi`) are
// also included under `test` so that unit tests can run on the macOS host
// without an iOS cross-compilation toolchain - their deps (bip39,
// ed25519-dalek, warren-identity, zeroize, std) are all cross-platform.
#![cfg(any(target_os = "ios", test))]
// The blanket
// `#![allow(clippy::undocumented_unsafe_blocks)]` was removed.
// The single current `unsafe` site (`get_string` below) carries an
// explicit `// Safety:` comment; any future `unsafe` block added in
// this crate must do the same.

// Warren-side FFI modules.  `warren_tunnel_ffi` and `warren_wallet_ffi`
// compile on any platform; the others use iOS-only libc / tokio / mullvad
// deps and stay iOS-gated.
#[cfg(any(target_os = "ios", test))]
mod warren_tunnel_ffi;
// v7 anonymous-token provider for the multi-hop tunnel. Uses warrenguard +
// warren-api, pulled by the `tunnel` feature, so it is gated like the data
// plane it feeds.
#[cfg(all(target_os = "ios", feature = "tunnel"))]
mod warren_token_provider;
// Multi-hop directory verification + circuit selection. Needs the
// warrenguard-multihop descriptor types pulled by the `tunnel` feature, and is
// consumed only by the tunnel data plane; also compiled under `test` (with the
// feature) so the selection tests run on the host.
#[cfg(all(feature = "tunnel", any(target_os = "ios", test)))]
mod warren_multihop_directory;
// Warren PRODUCT/deployment constants (`WARREN_SERVER_PUBKEY_HEX`) the app
// owns directly instead of depending on warren-core's `warren-config`
// crate. Consumed by `warren_multihop_directory` (tunnel) and by the
// `api_client` relay-list fetch, so it compiles on every iOS build.
#[cfg(any(target_os = "ios", test))]
mod warren_product_config;
// Signed `/v1/exits` verification + projection to the Mullvad relay-list
// wire shape the Swift REST stack decodes. Pure logic (verification lives
// in warren-discovery-core); compiled under `test` so it runs on the host,
// and on iOS where `api_client` serves it to Swift.
#[cfg(any(target_os = "ios", test))]
mod warren_exit_directory;
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
// Signed update-manifest verification backing the forced-update gate. Pure
// verify-and-evaluate (the manifest fetch lives in Swift); compiled under
// `test` so its fixture tests run on the host.
#[cfg(any(target_os = "ios", test))]
mod warren_version_ffi;
// Community-forum wallet login (doc 55): the pure request-construction +
// validation + outcome mapping (host-testable); the network POST that consumes
// it is iOS-gated in `warren_forum_ffi`.
#[cfg(any(target_os = "ios", test))]
mod forum;
// Pure, host-testable NAT-PMP helpers (port-follow resolution + event
// projection). Compiled under `test` so the host suite runs them, and on the
// iOS tunnel path where `warren_tunnel_ffi` consumes them. Same gating as
// `warren_multihop_generation`.
#[cfg(any(all(target_os = "ios", feature = "tunnel"), test))]
mod warren_natpmp_ffi;

// iOS-only modules that reference libc, tokio, mullvad-api, etc.
#[cfg(target_os = "ios")]
mod warren_account_ffi;
#[cfg(target_os = "ios")]
mod warren_forum_ffi;
#[cfg(target_os = "ios")]
mod warren_multihop_ffi;

// Upstream Mullvad API client retained transitionally. Each call site here
// still calls into `mullvad-api` (account number flows, device management,
// problem reports, StoreKit). Migrating those flows to the SDK's
// `warren-api` client (warren-sdk-rs) is a dedicated follow-up (~16 source
// files): the Warren wallet auth uses `X-Warren-*` canonical-message
// signatures (see warren-sdk-rs/crates/warren-identity::signing), which is
// a separate flow from Mullvad's account-number token model.
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
