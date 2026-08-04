//! Windows default-route split: a thin wrapper over the shared
//! warrenguard-winroute guard (re-exported by warrenguard-route-split as
//! `default_route_split_windows`).
//!
//! The engine guard's `install` takes only `tun_name` (no `exit_ip`): Windows
//! no longer plants any host-route exception at all (Port Fail /
//! TunnelCrack ServerIP fix; the carrier escapes via `IP_UNICAST_IF` on the
//! dial socket instead, see `warren_carrier_socket_bypass` in `lib.rs`). This
//! wrapper keeps the facade's `install(Ipv4Addr, &str)` shape so the `lib.rs`
//! call site stays OS-agnostic like the Linux/macOS guards; `exit_ip` is
//! accepted and dropped, never shaping a route.

use std::net::Ipv4Addr;

use anyhow::Result;

/// Windows default-route split guard. Wraps the shared warrenguard-winroute
/// guard; `exit_ip` is accepted only for facade parity with Linux/macOS, see
/// the module docs above.
#[derive(Debug)]
pub struct DefaultRouteSplitGuard(
    warrenguard_route_split::default_route_split_windows::DefaultRouteSplitGuard,
);

impl DefaultRouteSplitGuard {
    pub async fn install(_exit_ip: Ipv4Addr, tun_name: &str) -> Result<Self> {
        let inner =
            warrenguard_route_split::default_route_split_windows::DefaultRouteSplitGuard::install(
                tun_name,
            )
            .await?;
        Ok(Self(inner))
    }

    /// Remove the routing. Idempotent; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        self.0.uninstall().await
    }
}
