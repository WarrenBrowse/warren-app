//! Cross-OS facade for default-route split policy routing.
//!
//! The tunnel adapter needs a single `DefaultRouteSplitGuard` type
//! regardless of the host OS so the `WarrenTunnelMonitor` field type
//! compiles everywhere. The per-OS implementation differs because the
//! routing primitives differ:
//!
//! - **Linux**: dedicated table 100 + `ip rule` priorities + an
//!   exit-IP bypass exception. The in-crate impl in
//!   [`linux`] shells out to `ip` directly (= same recipe as
//!   `warren_client::default_route_split` upstream, kept inline here
//!   because the adapter predates the upstream crate split).
//! - **macOS**: route-specificity ordering in the global table -
//!   `<exit_ip>/32 -interface <default_iface>` (host-route exception)
//!   then `0.0.0.0/1` + `128.0.0.0/1 -interface <tun>`. Implementation
//!   re-exported verbatim from `warren_client::default_route_split_macos`
//!   so the adapter stays a thin wrapper.
//! - **Other** (currently Windows + the rest): a [`stub`] that always
//!   fails to install. Lets the `default_route_guard` field type exist
//!   so the crate compiles on every target while making it visible at
//!   runtime that traffic is **not** being captured by the tunnel
//!   (operator log surfaces the install error and the warning that no
//!   Internet traffic flows through the TUN).
//!
//! The `install(exit_ip, tun_name)` / `uninstall(self)` / `Drop` API is
//! identical on every platform so the call site in `lib.rs` does not
//! need to fan out per OS - only the cfg block that constructs the
//! guard does, and that exists already to choose between IPv4-only and
//! "no v4 next-hop, skip routing" paths.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::DefaultRouteSplitGuard;

// macOS uses the upstream warren-core port directly. Identical install
// signature (`install(Ipv4Addr, &str) -> Result<Self>`) so the lib.rs
// dispatch is OS-agnostic at the call site. See
// `warren_client::default_route_split_macos` for the recipe + tests.
#[cfg(target_os = "macos")]
pub use warren_client::default_route_split_macos::DefaultRouteSplitGuard;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod stub;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use stub::DefaultRouteSplitGuard;

#[cfg(test)]
mod facade_tests {
    use super::DefaultRouteSplitGuard;
    use std::net::Ipv4Addr;

    // Anti-regression: the facade must expose the same `install
    // (Ipv4Addr, &str) -> Result<Self>` shape and a `uninstall(self)
    // -> Result<()>` shape, so the OS-agnostic call site in `lib.rs`
    // stays compatible. If the upstream signature drifts (e.g. an
    // extra arg lands in `warren_client::default_route_split_macos::install`),
    // this test fails at compile time and the operator gets a clear
    // signal that the facade needs to be re-aligned.
    //
    // The closure is never polled (only the type-checker looks at the
    // shape), so it does not need a tokio runtime.
    #[test]
    fn api_surface_matches_lib_rs_call_site() {
        let _exercise = async {
            let exit_ip: Ipv4Addr = "10.0.0.1".parse().unwrap();
            let tun_name = "tun0";
            // Mirrors lib.rs:693 / 1061 verbatim. Fails to compile if
            // either the in-crate Linux impl or the warren-core macOS
            // port diverges from the (Ipv4Addr, &str) signature.
            let guard: anyhow::Result<DefaultRouteSplitGuard> =
                DefaultRouteSplitGuard::install(exit_ip, tun_name).await;
            // Mirrors lib.rs:wait() teardown: uninstall consumes the
            // guard and returns Result<()>. Drops the value otherwise.
            if let Ok(g) = guard {
                let _: anyhow::Result<()> = g.uninstall().await;
            }
        };
        // Black-hole the future so the compiler keeps the shape check
        // but the test is a true no-op at runtime.
        let _ = &_exercise;
    }
}
