//! Linux default-route split: a thin wrapper over the shared warrenguard-route-split
//! recipe (`warrenguard_route_split::default_route_split`).
//!
//! The `ip rule` + dedicated-table-100 recipe (the two `/1` halves, the
//! carrier's own `fwmark <WARREN_TUNNEL_FWMARK> lookup main` bypass at pref
//! 50, the tun lookup at pref 51, the synchronous `Drop` cleanup, and the
//! ownership-scoped crash recovery) lives once in warrenguard-route-split and
//! is shared with the standalone CLI. Keying the carrier's escape on its
//! socket mark, not the exit destination, is the Port Fail / TunnelCrack
//! ServerIP fix; the mark is set on the dial socket in `lib.rs`
//! (`warren_carrier_socket_bypass`), not here. The only desktop-daemon-specific
//! addition in THIS wrapper is a *different*, second fwmark bypass: traffic
//! the firewall marks for excluded apps must reach the `main` table (physical
//! NIC) instead of the TUN. We inject that here via warrenguard-route-split's
//! `split_tunnel_fwmark` parameter, so no part of the recipe is duplicated;
//! this wrapper only supplies the Warren-specific mark and keeps the facade's
//! `install(Ipv4Addr, &str)` shape (`exit_ip` is accepted for that parity but
//! no longer shapes any route, see the wrapped guard's own docs). The v6 guard
//! ([`DefaultRouteSplitV6Guard`]) wraps warrenguard-route-split the same way
//! and injects the same excluded-app fwmark, so excluded-app traffic egresses
//! the physical NIC on both families.

use std::net::{Ipv4Addr, Ipv6Addr};

use anyhow::Result;
use warrenguard_route_split::default_route_split::{IncludeFwmark, TunLookupScope};

/// Firewall mark the nftables split-tunnel rules apply (as packet `meta mark`)
/// to traffic from excluded processes. Mirrors `mullvad_types::TUNNEL_FWMARK`
/// (kept as a local literal to avoid a `mullvad-types` dependency edge from
/// this crate). Hex form so it matches the `ip rule show` display.
const SPLIT_TUNNEL_FWMARK: &str = "0x6d6f6c65";

/// What the tunnel lookup captures, and the exclusion bypass that goes with
/// it. Exclusion and include-only never coexist: an include-only tunnel drops
/// the excluded-app bypass, so its pref-49 rule can never send marked traffic
/// around the tunnel.
fn route_selection(include_only: bool) -> Result<(Option<&'static str>, TunLookupScope)> {
    if include_only {
        let mark = IncludeFwmark::new(talpid_types::split_tunnel::INCLUDE_FWMARK)?;
        Ok((None, TunLookupScope::Marked(mark)))
    } else {
        Ok((Some(SPLIT_TUNNEL_FWMARK), TunLookupScope::AllTraffic))
    }
}

/// Linux default-route split guard. Wraps the shared warrenguard-route-split guard and
/// injects the desktop daemon's split-tunnel fwmark bypass. Exposes the same
/// `install(Ipv4Addr, &str)` / `uninstall(self)` / `Drop` shape as the
/// macOS/Windows guards so the parent facade stays OS-agnostic.
#[derive(Debug)]
pub struct DefaultRouteSplitGuard(
    warrenguard_route_split::default_route_split::DefaultRouteSplitGuard,
);

impl DefaultRouteSplitGuard {
    /// Install the split-default routing for `tun_name`: the carrier's own
    /// fwmark bypass (wrapped guard) plus the excluded-app split-tunnel
    /// fwmark bypass (this wrapper). `exit_ip` no longer shapes any route,
    /// see the module docs. The synchronous teardown on `Drop` and the
    /// ownership-scoped crash recovery come from the wrapped
    /// warrenguard-route-split guard.
    ///
    /// With `include_only`, only traffic carrying the include mark enters the
    /// tunnel; the firewall marks it and drops it anywhere else.
    pub async fn install(exit_ip: Ipv4Addr, tun_name: &str, include_only: bool) -> Result<Self> {
        let (split_tunnel_fwmark, scope) = route_selection(include_only)?;
        let inner =
            warrenguard_route_split::default_route_split::DefaultRouteSplitGuard::install_scoped(
                exit_ip,
                tun_name,
                &[],
                split_tunnel_fwmark,
                scope,
            )
            .await?;
        Ok(Self(inner))
    }

    /// Remove the routing. Idempotent and best-effort; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        self.0.uninstall().await
    }
}

/// Linux IPv6 default-route split guard. Wraps the shared warrenguard-route-split v6 guard
/// and injects the same split-tunnel fwmark bypass as the v4 wrapper, so
/// excluded-app traffic over IPv6 also egresses the physical NIC instead of the
/// TUN. Same `install(Option<Ipv6Addr>, &str)` shape as the macOS/Windows v6
/// guards so the parent facade stays OS-agnostic.
#[derive(Debug)]
pub struct DefaultRouteSplitV6Guard(
    warrenguard_route_split::default_route_split::DefaultRouteSplitV6Guard,
);

impl DefaultRouteSplitV6Guard {
    /// Install the v6 split-default routing for `tun_name`, plus the
    /// split-tunnel fwmark bypass on the v6 rule database. Teardown on `Drop`
    /// comes from the wrapped warrenguard-route-split guard.
    pub async fn install(
        exit_ip_v6: Option<Ipv6Addr>,
        tun_name: &str,
        include_only: bool,
    ) -> Result<Self> {
        let (split_tunnel_fwmark, scope) = route_selection(include_only)?;
        let inner =
            warrenguard_route_split::default_route_split::DefaultRouteSplitV6Guard::install_scoped(
                exit_ip_v6,
                tun_name,
                split_tunnel_fwmark,
                scope,
            )
            .await?;
        Ok(Self(inner))
    }

    /// Remove the v6 routing. Idempotent and best-effort; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        self.0.uninstall().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_only_scopes_the_lookup_to_the_include_mark_and_drops_the_exclusion_bypass() {
        let (bypass, scope) = route_selection(true).unwrap();

        assert_eq!(bypass, None);
        assert_eq!(
            scope,
            TunLookupScope::Marked(
                IncludeFwmark::new(talpid_types::split_tunnel::INCLUDE_FWMARK).unwrap()
            )
        );
    }

    #[test]
    fn a_full_tunnel_keeps_the_exclusion_bypass_and_captures_everything() {
        let (bypass, scope) = route_selection(false).unwrap();

        assert_eq!(bypass, Some(SPLIT_TUNNEL_FWMARK));
        assert_eq!(scope, TunLookupScope::AllTraffic);
    }
}
