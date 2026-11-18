// The iOS-specific runtime + API-client code only compiles on `target_os =
// "ios"`.  The FFI modules (`warren_tunnel_ffi`, `warren_wallet_ffi`) are
// also included under `test` so that unit tests can run on the macOS host
// without an iOS cross-compilation toolchain - their deps (bip39,
// ed25519-dalek, warren-identity, zeroize, std) are all cross-platform.
#![cfg(any(target_os = "ios", test))]

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
// Signed launch-announcement verification (`GET /v1/announcements`). Pure
// verify-and-render (the fetch lives in Swift, like the update manifest);
// compiled under `test` so its signature, anti-rollback and freshness tests
// run on the host.
#[cfg(any(target_os = "ios", test))]
mod warren_announcements_ffi;
// Community-forum wallet login: the pure request-construction +
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
#[cfg(any(all(target_os = "ios", feature = "tunnel"), test))]
mod warren_port_entitlements;

// The port-forward standing stores of the extension and the app.
// Cross-platform so the host suite runs its tests.
mod warren_standing_ffi;

// iOS-only modules that reference libc, tokio, mullvad-api, etc.
#[cfg(target_os = "ios")]
mod warren_account_ffi;
// The account's devices: the install's registration and
// revocation, and the list, rename and revoke calls.
#[cfg(any(target_os = "ios", test))]
mod warren_devices_ffi;
#[cfg(target_os = "ios")]
mod warren_forum_ffi;
// The signed pubkey-mismatch report the exit key-change alert files.
mod warren_incidents_ffi;
// The licence text and the source revisions of the licences screen.
mod warren_licenses_ffi;
#[cfg(target_os = "ios")]
mod warren_multihop_ffi;
// The public environment descriptor the beta badge reads.
#[cfg(target_os = "ios")]
mod warren_network_info_ffi;

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

#[cfg(all(test, target_os = "ios"))]
mod runtime_tests {
    /// The packet tunnel extension runs under a memory ceiling and already
    /// has the tunnel's own runtime; a worker per core here would add one
    /// thread per core of the device for API calls that are rare.
    #[test]
    fn the_shared_runtime_runs_two_workers() {
        let handle = super::warren_ios_runtime().expect("runtime");
        assert_eq!(handle.metrics().num_workers(), 2);
    }
}

#[cfg(target_os = "ios")]
fn warren_ios_runtime() -> Result<Handle, String> {
    match RUNTIME.get_or_init(|| {
        // Two workers, not one per core: the API calls it serves are rare, and
        // the packet tunnel extension runs it beside the tunnel's own runtime
        // under a memory ceiling.
        Builder::new_multi_thread()
            .worker_threads(2)
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
/// returning `sentinel` instead of unwinding across the C ABI: since Rust
/// 1.81 an unwind out of an `extern "C"` fn aborts the process, which in the
/// packet tunnel extension drops the tunnel. Every Warren `extern "C"` entry
/// point runs its body through it.
#[cfg(any(target_os = "ios", test))]
pub(crate) fn ffi_guard<R>(sentinel: R, body: impl FnOnce() -> R) -> R {
    let guarded = std::panic::AssertUnwindSafe(|| {
        #[cfg(test)]
        panic_probe::fire_if_armed();
        body()
    });
    match std::panic::catch_unwind(guarded) {
        Ok(value) => value,
        Err(_) => {
            #[cfg(test)]
            panic_probe::record_caught();
            sentinel
        }
    }
}

/// Lets a test make the body of a real entry point panic, and tell whether
/// the guard caught it: an entry point that skips [`ffi_guard`] never panics
/// under the probe and never records a catch, and one whose guard stopped
/// catching aborts the test binary.
#[cfg(test)]
pub(crate) mod panic_probe {
    use std::cell::Cell;

    thread_local! {
        static ARMED: Cell<bool> = const { Cell::new(false) };
        static CAUGHT: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn fire_if_armed() {
        if ARMED.with(Cell::get) {
            panic!("panic injected into an FFI body");
        }
    }

    pub(crate) fn record_caught() {
        CAUGHT.with(|caught| caught.set(true));
    }

    /// Runs `call` with the next guarded body set to panic, and returns what
    /// it returned with whether a guard caught the panic.
    pub(crate) fn panicking<R>(call: impl FnOnce() -> R) -> (R, bool) {
        CAUGHT.with(|caught| caught.set(false));
        ARMED.with(|armed| armed.set(true));
        let value = call();
        ARMED.with(|armed| armed.set(false));
        (value, CAUGHT.with(Cell::get))
    }

    #[test]
    fn a_panic_in_a_guarded_body_returns_the_sentinel() {
        let (value, caught) = panicking(|| super::ffi_guard(-2, || 0));
        assert_eq!(value, -2);
        assert!(caught);
    }

    #[test]
    fn a_guarded_body_that_returns_passes_its_value_through() {
        assert_eq!(super::ffi_guard(-2, || 7), 7);
    }
}
