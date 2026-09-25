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
//!
//! Include-only ("VPN only for these apps") installs no split halves: the
//! physical default route must stay the best one for every app the split
//! tunnel driver leaves alone. The tunnel gets its own default route at a
//! metric that loses to it, which only sockets bound to the tunnel address
//! (the included apps', rebound there by the driver) take, and a host route
//! to each tunnel resolver (see `super::include_only_routes`).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use anyhow::{Context, Result};
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_FOUND, ERROR_OBJECT_ALREADY_EXISTS, NO_ERROR},
    NetworkManagement::{
        IpHelper::{
            CreateIpForwardEntry2, DeleteIpForwardEntry2, InitializeIpForwardEntry,
            MIB_IPFORWARD_ROW2,
        },
        Ndis::NET_LUID_LH,
    },
    Networking::WinSock::{MIB_IPPROTO_NETMGMT, NlroManual},
};

use super::{TunnelRoute, include_only_routes};

/// Windows default-route split guard: the engine's `/1` halves, or the
/// include-only routes.
#[derive(Debug)]
pub enum DefaultRouteSplitGuard {
    FullTunnel(warrenguard_route_split::default_route_split_windows::DefaultRouteSplitGuard),
    IncludeOnly(TunnelRoutes),
}

impl DefaultRouteSplitGuard {
    pub async fn install(
        _exit_ip: Ipv4Addr,
        tun_name: &str,
        include_only: bool,
        resolvers: &[IpAddr],
    ) -> Result<Self> {
        if include_only {
            let routes = include_only_routes(Ipv4Addr::UNSPECIFIED.into(), resolvers);
            return TunnelRoutes::install(tun_name, &routes).map(Self::IncludeOnly);
        }
        let inner =
            warrenguard_route_split::default_route_split_windows::DefaultRouteSplitGuard::install(
                tun_name,
            )
            .await?;
        Ok(Self::FullTunnel(inner))
    }

    /// Remove the routing. Idempotent; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        match self {
            Self::FullTunnel(inner) => inner.uninstall().await,
            Self::IncludeOnly(routes) => {
                routes.remove();
                Ok(())
            }
        }
    }
}

/// IPv6 counterpart of [`DefaultRouteSplitGuard`].
#[derive(Debug)]
pub enum DefaultRouteSplitV6Guard {
    FullTunnel(warrenguard_route_split::default_route_split_windows::DefaultRouteSplitV6Guard),
    IncludeOnly(TunnelRoutes),
}

impl DefaultRouteSplitV6Guard {
    pub async fn install(
        exit_ip_v6: Option<Ipv6Addr>,
        tun_name: &str,
        include_only: bool,
        resolvers: &[IpAddr],
    ) -> Result<Self> {
        if include_only {
            let routes = include_only_routes(Ipv6Addr::UNSPECIFIED.into(), resolvers);
            return TunnelRoutes::install(tun_name, &routes).map(Self::IncludeOnly);
        }
        let inner =
            warrenguard_route_split::default_route_split_windows::DefaultRouteSplitV6Guard::install(
                exit_ip_v6, tun_name,
            )
            .await?;
        Ok(Self::FullTunnel(inner))
    }

    /// Remove the routing. Idempotent; see the wrapped guard.
    pub async fn uninstall(self) -> Result<()> {
        match self {
            Self::FullTunnel(inner) => inner.uninstall().await,
            Self::IncludeOnly(routes) => {
                routes.remove();
                Ok(())
            }
        }
    }
}

/// Routes on the tunnel interface, removed on drop.
pub struct TunnelRoutes {
    rows: Vec<MIB_IPFORWARD_ROW2>,
}

// The rows name the tunnel interface and public destinations only.
impl std::fmt::Debug for TunnelRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelRoutes")
            .field("routes", &self.rows.len())
            .finish()
    }
}

// SAFETY: a `MIB_IPFORWARD_ROW2` holds plain values only (addresses, a LUID,
// numbers), no pointer.
unsafe impl Send for TunnelRoutes {}

impl TunnelRoutes {
    fn install(tun_name: &str, routes: &[TunnelRoute]) -> Result<Self> {
        let luid = talpid_windows::net::luid_from_alias(tun_name)
            .context("no interface with the tunnel's alias")?;
        let mut installed = Self { rows: Vec::new() };
        for &(destination, prefix_len, metric) in routes {
            let row = route_row(luid, destination, prefix_len, metric);
            // SAFETY: `row` was initialized by `InitializeIpForwardEntry`, then
            // given a valid prefix, next hop and interface.
            let status = unsafe { CreateIpForwardEntry2(&raw const row) };
            if status != NO_ERROR && status != ERROR_OBJECT_ALREADY_EXISTS {
                // `installed` drops here and removes what went in.
                return Err(std::io::Error::from_raw_os_error(status as i32))
                    .context("failed to add an include-only route");
            }
            installed.rows.push(row);
        }
        log::info!(
            "Include-only routing installed: {} routes on the tunnel",
            installed.rows.len()
        );
        Ok(installed)
    }

    fn remove(mut self) {
        self.remove_rows();
    }

    fn remove_rows(&mut self) {
        for row in self.rows.drain(..) {
            // SAFETY: `row` is a row this guard added.
            let status = unsafe { DeleteIpForwardEntry2(&raw const row) };
            if status != NO_ERROR && status != ERROR_NOT_FOUND {
                log::warn!("Failed to remove an include-only route: {status}");
            }
        }
    }
}

impl Drop for TunnelRoutes {
    fn drop(&mut self) {
        self.remove_rows();
    }
}

/// An on-link route on the interface `luid`.
fn route_row(
    luid: NET_LUID_LH,
    destination: IpAddr,
    prefix_len: u8,
    metric: u32,
) -> MIB_IPFORWARD_ROW2 {
    // SAFETY: MIB_IPFORWARD_ROW2 holds no reference or pointer, only plain
    // values, so it may be zeroed.
    let mut row: MIB_IPFORWARD_ROW2 = unsafe { std::mem::zeroed() };
    // SAFETY: required before a row is handed to CreateIpForwardEntry2.
    unsafe { InitializeIpForwardEntry(&raw mut row) };
    let unspecified = match destination {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    row.InterfaceLuid = luid;
    row.DestinationPrefix.Prefix =
        talpid_windows::net::inet_sockaddr_from_socketaddr(SocketAddr::new(destination, 0));
    row.DestinationPrefix.PrefixLength = prefix_len;
    row.NextHop =
        talpid_windows::net::inet_sockaddr_from_socketaddr(SocketAddr::new(unspecified, 0));
    row.Metric = metric;
    row.Protocol = MIB_IPPROTO_NETMGMT;
    row.Origin = NlroManual;
    row
}
