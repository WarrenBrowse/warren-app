//! Cross-OS facade for default-route split policy routing.
//!
//! The tunnel adapter needs a single `DefaultRouteSplitGuard` type
//! regardless of the host OS so the `WarrenTunnelMonitor` field type
//! compiles everywhere. The per-OS implementation differs because the
//! routing primitives differ:
//!
//! - **Linux**: dedicated table 100 + `ip rule` priorities + an exit-IP bypass exception. The
//!   recipe lives once in `warren_client::default_route_split` (table 100, the `0.0.0.0/1` +
//!   `128.0.0.0/1` halves, exit-IP bypass at pref 50, TUN lookup at pref 51, synchronous `Drop`
//!   teardown, ownership-scoped crash recovery). The thin [`linux`] wrapper only injects the
//!   desktop daemon's split-tunnel **fwmark** bypass (`fwmark <TUNNEL_FWMARK> lookup main pref
//!   49`, so excluded apps egress the physical NIC) via warren-core's `split_tunnel_fwmark`
//!   parameter. The standalone CLI passes `--bypass-cidr` (a `to <cidr> lookup main pref 49`
//!   rule) through the same parameter slot instead; both coexist at pref 49 with distinct
//!   selectors. No part of the recipe is duplicated.
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
pub use linux::DefaultRouteSplitV6Guard;
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
/// - **Linux**: read `ip rule show`, delete the `lookup 100` rule and the `/1`
///   routes inside table 100 by destination (never a blanket flush, never
///   `main`), sparing any foreign rule/route.
/// - **macOS**: reclaim the registry-tracked host route + the owned `/1` halves.
/// - **Windows**: `Remove-NetRoute` the `/1` halves scoped to the Warren
///   tunnel alias in the ActiveStore.
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
    use super::{DefaultRouteSplitGuard, DefaultRouteSplitV6Guard};
    use std::net::{Ipv4Addr, Ipv6Addr};

    // Anti-regression: the facade must expose the same `install
    // (Ipv4Addr, &str) -> Result<Self>` shape and a `uninstall(self)
    // -> Result<()>` shape, so the OS-agnostic call site in `lib.rs`
    // stays compatible. If the per-OS impl signature drifts (e.g. an
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
            // Mirrors the lib.rs v4 call site verbatim. Fails to compile if
            // the Linux wrapper or the warren-core macOS/Windows ports diverge
            // from the (Ipv4Addr, &str) signature.
            let guard: anyhow::Result<DefaultRouteSplitGuard> =
                DefaultRouteSplitGuard::install(exit_ip, tun_name).await;
            if let Ok(g) = guard {
                let _: anyhow::Result<()> = g.uninstall().await;
            }
        };
        let _ = &_exercise;
    }

    #[test]
    fn v6_api_surface_matches_lib_rs_call_site() {
        // Same compile-time pin for the v6 guard: every OS impl (3 + stub)
        // must keep the `install(Option<Ipv6Addr>, &str) -> Result<Self>` +
        // `uninstall(self) -> Result<()>` shape the lib.rs call site relies on.
        let _exercise = async {
            let exit_ip_v6: Option<Ipv6Addr> = Some("2a01:4f8:1::2".parse().unwrap());
            let guard: anyhow::Result<DefaultRouteSplitV6Guard> =
                DefaultRouteSplitV6Guard::install(exit_ip_v6, "tun0").await;
            if let Ok(g) = guard {
                let _: anyhow::Result<()> = g.uninstall().await;
            }
        };
        let _ = &_exercise;
    }
}
