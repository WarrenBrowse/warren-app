//! No-op default-route split guard for exotic targets with no Warren-specific
//! routing recipe. Linux, macOS AND Windows are all wired in the parent module
//! (Windows via the warrenguard-route-split port), so this stub is only reached on targets
//! that are none of those three.
//!
//! The type exists purely so the `WarrenTunnelMonitor::default_route_guard`
//! field compiles unconditionally. Calling [`DefaultRouteSplitGuard::install`]
//! always returns an error so the operator sees that no policy routing
//! is being applied (and that the tunnel is up but the host's Internet
//! traffic still flows through the original default route, not via the
//! Warren TUN).

use std::net::Ipv4Addr;

use anyhow::{Result, anyhow};

#[derive(Debug)]
pub struct DefaultRouteSplitGuard {
    _priv: (),
}

impl DefaultRouteSplitGuard {
    /// Always returns an error on platforms without a Warren-specific
    /// default-route split recipe. The caller logs the error and the
    /// tunnel comes up without routing the host's default traffic.
    pub async fn install(_exit_ip: Ipv4Addr, _tun_name: &str) -> Result<Self> {
        Err(anyhow!(
            "Warren default-route split not implemented on this platform; \
             tunnel up but Internet traffic NOT captured by the TUN"
        ))
    }

    /// No-op: this stub never installs anything.
    pub async fn uninstall(self) -> Result<()> {
        Ok(())
    }
}
