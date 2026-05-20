//! No-op default-route split guard for platforms that do not have a
//! Warren-specific routing recipe wired yet (Windows + everything that
//! is not Linux or macOS).
//!
//! The type exists purely so the `WarrenTunnelMonitor::default_route_guard`
//! field compiles unconditionally. Calling [`DefaultRouteSplitGuard::install`]
//! always returns an error so the operator sees that no policy routing
//! is being applied (and that the tunnel is up but the host's Internet
//! traffic still flows through the original default route, not via the
//! Warren TUN). The Linux + macOS impls cover the supported platforms.
//!
//! When Windows split-default lands (Session A.2), this stub gets
//! replaced by a `pub use` of the warren-core Windows port, mirroring
//! the macOS arm in the parent module.

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
