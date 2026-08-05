//! Cross-OS facade for default-route split policy routing.
//!
//! The tunnel adapter needs a single `DefaultRouteSplitGuard` type
//! regardless of the host OS so the `WarrenTunnelMonitor` field type
//! compiles everywhere. The per-OS implementation differs because the
//! routing primitives differ:
//!
//! - **Linux**: dedicated table 100 + `ip rule` priorities, the carrier's own escape keyed on its
//!   `SO_MARK` rather than the exit destination (Port Fail / TunnelCrack ServerIP fix). The
//!   recipe lives once in `warrenguard_route_split::default_route_split` (table 100, the `0.0.0.0/1` +
//!   `128.0.0.0/1` halves, the `fwmark <WARREN_TUNNEL_FWMARK> lookup main` carrier bypass at pref
//!   50, TUN lookup at pref 51, synchronous `Drop` teardown, ownership-scoped crash recovery). The
//!   thin [`linux`] wrapper only injects the desktop daemon's split-tunnel **fwmark** bypass
//!   (`fwmark <TUNNEL_FWMARK> lookup main pref 49`, so excluded apps egress the physical NIC, a
//!   *different* mark from the carrier's) via warrenguard-route-split's `split_tunnel_fwmark`
//!   parameter. The standalone CLI passes `--bypass-cidr` (a `to <cidr> lookup main pref 49`
//!   rule) through the same parameter slot instead; both coexist at pref 49 with distinct
//!   selectors. No part of the recipe is duplicated. The carrier socket itself is marked in
//!   `lib.rs` (`warren_carrier_socket_bypass`), before this split lands.
//! - **macOS**: `0.0.0.0/1` + `128.0.0.0/1 -interface <tun>` in the global table (route-specificity
//!   over the system default), re-exported verbatim from
//!   `warrenguard_route_split::default_route_split_macos`, which plants no exit-IP exception of its
//!   own (same fix as Linux, upstream). The carrier's escape is `IP_BOUND_IF` on the dial socket
//!   (`lib.rs`, `warren_carrier_socket_bypass` + `discover_warren_phys_ifindex`), applied before
//!   this split lands, exactly like Windows, and mandatory: a discovery failure aborts the connect
//!   instead of dialing the carrier unmarked.
//! - **Windows**: native `/1` + `/1` split via the Win32 IP Helper API (`warrenguard-winroute`),
//!   re-exported through the thin [`windows`] wrapper. Plants no host-route exception at all; the
//!   carrier's escape is `IP_UNICAST_IF` on the dial socket (`lib.rs`,
//!   `warren_carrier_socket_bypass` + `discover_warren_phys_ifindex`), applied before this split
//!   lands.
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

// macOS uses the upstream warrenguard-route-split port directly. Identical install
// signature (`install(Ipv4Addr, &str) -> Result<Self>`) so the lib.rs
// dispatch is OS-agnostic at the call site. See
// `warrenguard_route_split::default_route_split_macos` for the recipe + tests.
#[cfg(target_os = "macos")]
pub use warrenguard_route_split::default_route_split_macos::DefaultRouteSplitGuard;

// Windows: the engine guard dropped `exit_ip` entirely (no host-route
// exception at all, socket-level `IP_UNICAST_IF` bypass instead), so the
// thin in-crate wrapper restores the facade's `install(Ipv4Addr, &str)`
// shape, mirroring the Linux wrapper. See `windows::DefaultRouteSplitGuard`
// and `warrenguard_route_split::default_route_split_windows` for the recipe
// + tests.
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::DefaultRouteSplitGuard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod stub;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use stub::DefaultRouteSplitGuard;

// IPv6 split-default routing. Each desktop OS now ships a real leak-safe
// v6 guard from `warrenguard_route_split` (Linux: `::/1` + `8000::/1` in the
// dedicated table; macOS: `route -inet6` halves; Windows: native Win32 IP
// Helper API halves), all with the same `install(Option<Ipv6Addr>, &str)` shape so
// this facade stays OS-agnostic at the call site. Truly exotic targets
// (not Linux/macOS/Windows) fall back to a stub whose `install` bails -
// the firewall keeps native v6 blocked there regardless (no leak).
#[cfg(target_os = "linux")]
pub use linux::DefaultRouteSplitV6Guard;
#[cfg(target_os = "macos")]
pub use warrenguard_route_split::default_route_split_macos::DefaultRouteSplitV6Guard;
#[cfg(target_os = "windows")]
pub use warrenguard_route_split::default_route_split_windows::DefaultRouteSplitV6Guard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use v6_stub::DefaultRouteSplitV6Guard;

// The complement of the guard above, for the tunnel that carries NO IPv6:
// declare global v6 unreachable so the host stops preferring a path that
// cannot work. Only macOS needs it. Every platform blocks native v6 in its
// firewall, so none of them leaks, but they do not fail alike:
//
// - Linux drops in the nftables output hook, which returns EPERM from the
//   syscall, and Windows blocks at the WFP ALE layer, which fails `connect()`
//   immediately. Both let Happy Eyeballs fall back to IPv4 at once.
// - Android routes `::/0` into a TUN with no v6 address, so the OS advertises
//   the network as IPv4-only and its resolver stops offering AAAA at all;
//   iOS captures `::/0` behind a ULA, which RFC 6724 deprioritises.
// - macOS blocks with pf `block return out`, and the RST it synthesises races
//   `connect()` to completion: the socket can report connected and fail only
//   on the first `send()`, by which point the application is committed to the
//   v6 address and never tries the v4 one.
//
// So macOS gets the real guard and the others a no-op: not an unsupported
// target, a platform whose own block already fails fast.
#[cfg(target_os = "macos")]
pub use warrenguard_route_split::default_route_split_macos::Ipv6UnreachableGuard;

#[cfg(not(target_os = "macos"))]
pub use v6_unreachable_noop::Ipv6UnreachableGuard;

#[cfg(not(target_os = "macos"))]
mod v6_unreachable_noop {
    /// No-op on every platform whose firewall already fails blocked IPv6
    /// synchronously. Keeps the monitor field type and the install call site
    /// OS-agnostic.
    #[derive(Debug)]
    pub struct Ipv6UnreachableGuard;

    impl Ipv6UnreachableGuard {
        pub async fn install() -> anyhow::Result<Self> {
            Ok(Self)
        }

        pub async fn uninstall(self) -> anyhow::Result<()> {
            Ok(())
        }
    }
}

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
/// These routes are installed via the `route`/`ip` CLI or the native Win32 IP
/// Helper API out-of-band, so the talpid `RouteManager`'s `clear_routes()` cannot see
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
    {
        warrenguard_route_split::default_route_split_macos::force_cleanup_all();
        warrenguard_route_split::default_route_split_macos::force_cleanup_all_v6();
        // A leaked reject half would keep global IPv6 down after the tunnel is
        // gone, so it is swept on the same reset paths. Stateless (the reject
        // flag is the ownership proof), so it also reclaims what a crashed
        // predecessor left behind.
        warrenguard_route_split::default_route_split_macos::force_cleanup_all_v6_unreachable();
        // macOS v6 teardown repairs: talpid's restore can miss BOTH the
        // unscoped v6 default (its tracked best-v6 is empty at teardown; the
        // kernel re-adds an RA default only at the next RA, so native v6 dies
        // for minutes) and the primary interface's IFSCOPE default (dual-homed:
        // the primary-scoped system DNS resolver is stranded). Re-add whichever
        // is missing (leak-guarded: a no-op while a tunnel still carries
        // traffic).
        //
        // The talpid `RouteManager` clears the tunnel routes ASYNCHRONOUSLY
        // relative to this reset path, so an immediate repair usually still
        // sees v6 egress on the utun and correctly bails via the leak guard.
        // Retry on a short schedule: the guard makes every attempt safe (a
        // reconnect that raced in turns them into no-ops) and the repair is
        // idempotent, so late attempts on an already-healed table do nothing.
        warrenguard_route_split::default_route_split_macos::restore_primary_defaults_v6();
        std::thread::spawn(|| {
            for delay_secs in [1, 3, 6] {
                std::thread::sleep(std::time::Duration::from_secs(delay_secs));
                warrenguard_route_split::default_route_split_macos::restore_primary_defaults_v6();
            }
        });
    }
    #[cfg(target_os = "linux")]
    warrenguard_route_split::default_route_split::force_cleanup_all();
    #[cfg(target_os = "windows")]
    // Static reclaimer: no live tunnel metadata here (it runs after an unclean
    // exit where no guard survived), so sweep the engine's default adapter
    // alias, which is exactly the case that default exists for.
    warrenguard_route_split::default_route_split_windows::force_cleanup_all(
        warrenguard_route_split::default_route_split_windows::WARREN_TUN_ALIAS,
    );
}

#[cfg(test)]
mod facade_tests {
    use super::{DefaultRouteSplitGuard, DefaultRouteSplitV6Guard, Ipv6UnreachableGuard};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn v6_unreachable_api_surface_matches_lib_rs_call_site() {
        // Same compile-time pin for the guard held while the tunnel carries no
        // v6: the real macOS impl and the no-op used elsewhere must keep the
        // argument-free `install() -> Result<Self>` + `uninstall(self) ->
        // Result<()>` shape the lib.rs call site relies on.
        let _exercise = async {
            let guard: anyhow::Result<Ipv6UnreachableGuard> = Ipv6UnreachableGuard::install().await;
            if let Ok(g) = guard {
                let _: anyhow::Result<()> = g.uninstall().await;
            }
        };
        let _ = &_exercise;
    }

    // Anti-regression: the facade must expose the same `install
    // (Ipv4Addr, &str) -> Result<Self>` shape and a `uninstall(self)
    // -> Result<()>` shape, so the OS-agnostic call site in `lib.rs`
    // stays compatible. If the per-OS impl signature drifts (e.g. an
    // extra arg lands in `warrenguard_route_split::default_route_split_macos::install`),
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
            // the Linux wrapper or the warrenguard-route-split macOS/Windows ports diverge
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
