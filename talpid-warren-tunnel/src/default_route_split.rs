//! Cross-OS facade for default-route split policy routing.
//!
//! The tunnel adapter needs a single `DefaultRouteSplitGuard` type
//! regardless of the host OS so the `WarrenTunnelMonitor` field type
//! compiles everywhere. The per-OS implementation differs because the
//! routing primitives differ:
//!
//! - **Linux**: dedicated table 100 + `ip rule` priorities + an exit-IP bypass exception. The
//!   in-crate impl in [`linux`] shells out to `ip` directly (= same recipe as
//!   `warren_client::default_route_split` upstream, kept inline here because the adapter predates
//!   the upstream crate split).
//! - **macOS**: route-specificity ordering in the global table - `<exit_ip>/32 -interface
//!   <default_iface>` (host-route exception) then `0.0.0.0/1` + `128.0.0.0/1 -interface <tun>`.
//!   Implementation re-exported verbatim from `warren_client::default_route_split_macos` so the
//!   adapter stays a thin wrapper.
//! - **Windows**: PowerShell `New-NetRoute` recipe - host-route exception on the captured default
//!   `(ifindex, NextHop)` pair, then `0.0.0.0/1` + `128.0.0.0/1 -InterfaceAlias <tun>`. Re-exported
//!   from `warren_client::default_route_split_windows`.
//! - **Other** (any target that is not Linux, macOS or Windows): a [`stub`] that always fails to
//!   install. Lets the `default_route_guard` field type exist so the crate compiles on every target
//!   while making it visible at runtime that traffic is **not** being captured by the tunnel
//!   (operator log surfaces the install error and the warning that no Internet traffic flows
//!   through the TUN).
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

// Windows: same `install(Ipv4Addr, &str) -> Result<Self>` shape as
// Linux + macOS, exposed by the warren-core PowerShell port. The
// recipe is `New-NetRoute` host-route exception + the two /1 routes
// landing on the WinTUN interface alias. See
// `warren_client::default_route_split_windows` for the recipe + tests.
#[cfg(target_os = "windows")]
pub use warren_client::default_route_split_windows::DefaultRouteSplitGuard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod stub;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use stub::DefaultRouteSplitGuard;

// IPv6 split-default routing. Each desktop OS now ships a real leak-safe
// v6 guard from `warren_client` (Linux: `::/1` + `8000::/1` in the
// dedicated table; macOS: `route -inet6` halves; Windows: `New-NetRoute`
// halves), all with the same `install(Option<Ipv6Addr>, &str)` shape so
// this facade stays OS-agnostic at the call site. Truly exotic targets
// (not Linux/macOS/Windows) fall back to a stub whose `install` bails -
// the firewall keeps native v6 blocked there regardless (no leak).
#[cfg(target_os = "linux")]
pub use warren_client::default_route_split::DefaultRouteSplitV6Guard;
#[cfg(target_os = "macos")]
pub use warren_client::default_route_split_macos::DefaultRouteSplitV6Guard;
#[cfg(target_os = "windows")]
pub use warren_client::default_route_split_windows::DefaultRouteSplitV6Guard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use v6_stub::DefaultRouteSplitV6Guard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod v6_stub {
    use std::net::Ipv6Addr;

    /// Cross-OS placeholder so the monitor field type exists on exotic
    /// targets. `install` fails loudly so such a build never believes it
    /// routed v6; the firewall keeps native v6 blocked regardless (no leak).
    #[derive(Debug)]
    pub struct DefaultRouteSplitV6Guard;

    impl DefaultRouteSplitV6Guard {
        pub async fn install(
            _exit_ip_v6: Option<Ipv6Addr>,
            _tun_name: &str,
        ) -> anyhow::Result<Self> {
            anyhow::bail!("IPv6 split-default routing not supported on this target")
        }

        pub async fn uninstall(self) -> anyhow::Result<()> {
            Ok(())
        }
    }
}

/// Force out any Warren split-default routing this process (or a crashed
/// predecessor) installed, synchronously, regardless of guard lifetime.
///
/// These routes are installed via the `route`/`ip`/PowerShell CLI
/// out-of-band, so the talpid `RouteManager`'s `clear_routes()` cannot see
/// them: the state machine's reset paths call this so a leaked split can
/// never blackhole egress. Every desktop OS carries a real, idempotent,
/// privilege-tolerant implementation:
///
/// - **Linux**: `ip rule del` the Warren priorities + flush the dedicated
///   table 100 (never touches `main`).
/// - **macOS**: reclaim the registry-tracked host route + the `/1` halves.
/// - **Windows**: `Remove-NetRoute` the `/1` halves from the ActiveStore.
///
/// This is the recovery backstop on top of each guard's synchronous `Drop`:
/// it also reclaims a split leaked by a *previous* unclean exit, where no
/// guard survives.
///
/// All three desktop reclaimers are ownership-scoped: they reclaim only
/// Warren's own artifacts (the macOS registry host route + owned `/1` halves;
/// the Linux `lookup 100` rule + the `/1` routes inside the private table 100;
/// the Windows `/1` halves), so a co-resident VPN's routing is never disturbed.
pub fn force_route_cleanup() {
    #[cfg(target_os = "macos")]
    warren_client::default_route_split_macos::force_cleanup_all();
    #[cfg(target_os = "linux")]
    warren_client::default_route_split::force_cleanup_all();
    #[cfg(target_os = "windows")]
    warren_client::default_route_split_windows::force_cleanup_all();
}

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
