//! Linux default-route split: a thin wrapper over the shared warren-core
//! recipe (`warren_client::default_route_split`).
//!
//! The `ip rule` + dedicated-table-100 recipe (the two `/1` halves, the exit-IP
//! bypass at pref 50, the tun lookup at pref 51, the synchronous `Drop`
//! cleanup, and the ownership-scoped crash recovery) lives once in warren-core
//! and is shared with the standalone CLI. The only desktop-daemon-specific
//! addition is the split-tunnel **fwmark** bypass: traffic the firewall marks
//! for excluded apps must reach the `main` table (physical NIC) instead of the
//! TUN. We inject that here via warren-core's `split_tunnel_fwmark` parameter,
//! so no part of the recipe is duplicated; this wrapper only supplies the
//! Warren-specific mark and keeps the facade's `install(Ipv4Addr, &str)` shape.

use std::net::Ipv4Addr;

use anyhow::Result;

/// Firewall mark the nftables split-tunnel rules apply (as packet `meta mark`)
/// to traffic from excluded processes. Mirrors `mullvad_types::TUNNEL_FWMARK`
/// (kept as a local literal to avoid a `mullvad-types` dependency edge from
/// this crate). Hex form so it matches the `ip rule show` display.
const SPLIT_TUNNEL_FWMARK: &str = "0x6d6f6c65";

/// Linux default-route split guard. Wraps the shared warren-core guard and
/// injects the desktop daemon's split-tunnel fwmark bypass. Exposes the same
/// `install(Ipv4Addr, &str)` / `uninstall(self)` / `Drop` shape as the
/// macOS/Windows guards so the parent facade stays OS-agnostic.
#[derive(Debug)]
pub struct DefaultRouteSplitGuard(warren_client::default_route_split::DefaultRouteSplitGuard);

impl DefaultRouteSplitGuard {
    /// Install the split-default routing for `tun_name`, with the exit-IP
    /// bypass and the split-tunnel fwmark bypass. The synchronous teardown on
    /// `Drop` and the ownership-scoped crash recovery come from the wrapped
    /// warren-core guard.
    pub async fn install(exit_ip: Ipv4Addr, tun_name: &str) -> Result<Self> {
        let inner = warren_client::default_route_split::DefaultRouteSplitGuard::install(
            exit_ip,
            tun_name,
            &[],
            Some(SPLIT_TUNNEL_FWMARK),
        )
        .await?;
        Ok(Self(inner))
    }

    /// Remove the routing. Idempotent and best-effort; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        self.0.uninstall().await
    }
}
