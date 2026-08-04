//! Abstractions over operating system DNS settings.
use std::fmt;
use std::net::IpAddr;

#[cfg(target_os = "linux")]
use talpid_routing::RouteManagerHandle;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
mod imp;

#[cfg(target_os = "linux")]
pub use imp::will_use_nm;

#[cfg(windows)]
#[path = "windows/mod.rs"]
mod imp;

#[cfg(target_os = "android")]
#[path = "android.rs"]
mod imp;

pub use self::imp::Error;

/// Repair DNS state left behind by an unclean daemon exit (crash, SIGKILL),
/// without constructing a monitor. The daemon runs the same self-heals when
/// it restarts; this entry point exists for the out-of-band rescue
/// (`warren-setup reset-firewall`), which must restore name resolution too
/// when the daemon cannot come back up at all.
pub fn recover_after_crash() -> Result<(), Error> {
    imp::recover_after_crash()
}

/// DNS configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DnsConfig {
    config: InnerDnsConfig,
    /// Whether to lift the firewall's DNS leak protection (allow queries to any resolver).
    allow_external_dns: bool,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            config: InnerDnsConfig::Default,
            allow_external_dns: false,
        }
    }
}

impl DnsConfig {
    /// Use the specified addresses for DNS resolution
    pub fn from_addresses(tunnel_config: &[IpAddr], non_tunnel_config: &[IpAddr]) -> Self {
        DnsConfig {
            config: InnerDnsConfig::Override {
                tunnel_config: tunnel_config.to_owned(),
                non_tunnel_config: non_tunnel_config.to_owned(),
            },
            allow_external_dns: false,
        }
    }

    /// Lift the firewall's DNS leak protection. When enabled, the firewall stops blocking DNS to
    /// resolvers other than those in this config; queries still leave through the tunnel.
    pub fn allow_external_dns(mut self, allow: bool) -> Self {
        self.allow_external_dns = allow;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
enum InnerDnsConfig {
    /// Use gateway addresses from the tunnel config
    Default,
    /// Use the specified addresses for DNS resolution
    Override {
        /// Addresses to configure on the tunnel interface
        tunnel_config: Vec<IpAddr>,
        /// Addresses to allow on non-tunnel interface.
        /// For the most part, the tunnel state machine will not handle any of this configuration
        /// on non-tunnel interface, only allow them in the firewall.
        non_tunnel_config: Vec<IpAddr>,
    },
}

impl DnsConfig {
    pub fn resolve(
        &self,
        default_tun_config: &[IpAddr],
        #[cfg(target_os = "macos")] port: u16,
    ) -> ResolvedDnsConfig {
        match &self.config {
            InnerDnsConfig::Default => ResolvedDnsConfig {
                tunnel_config: default_tun_config.to_owned(),
                non_tunnel_config: vec![],
                allow_external_dns: self.allow_external_dns,
                #[cfg(target_os = "macos")]
                port,
            },
            InnerDnsConfig::Override {
                tunnel_config,
                non_tunnel_config,
            } => ResolvedDnsConfig {
                tunnel_config: tunnel_config.to_owned(),
                non_tunnel_config: non_tunnel_config.to_owned(),
                allow_external_dns: self.allow_external_dns,
                #[cfg(target_os = "macos")]
                port,
            },
        }
    }
}

/// DNS configuration with `DnsConfig::Default` resolved
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDnsConfig {
    /// Addresses to configure on the tunnel interface
    tunnel_config: Vec<IpAddr>,
    /// Addresses to allow on non-tunnel interface.
    /// For the most part, the tunnel state machine will not handle any of this configuration
    /// on non-tunnel interface, only allow them in the firewall.
    non_tunnel_config: Vec<IpAddr>,
    /// Whether the firewall should stop blocking DNS to resolvers other than the ones above.
    allow_external_dns: bool,
    /// Port to use
    #[cfg(target_os = "macos")]
    port: u16,
}

impl fmt::Display for ResolvedDnsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tunnel DNS: ")?;
        Self::fmt_addr_set(f, &self.tunnel_config)?;

        f.write_str(" Non-tunnel DNS: ")?;
        Self::fmt_addr_set(f, &self.non_tunnel_config)?;

        #[cfg(target_os = "macos")]
        write!(f, " Port: {}", self.port)?;

        Ok(())
    }
}

impl ResolvedDnsConfig {
    fn fmt_addr_set(f: &mut fmt::Formatter<'_>, addrs: &[IpAddr]) -> fmt::Result {
        f.write_str("{")?;
        for (i, addr) in addrs.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{addr}")?;
        }
        f.write_str("}")
    }

    /// Addresses to configure on the tunnel interface
    pub fn tunnel_config(&self) -> &[IpAddr] {
        &self.tunnel_config
    }

    /// Addresses to allow on non-tunnel interface.
    /// For the most part, the tunnel state machine will not handle any of this configuration
    /// on non-tunnel interface, only allow them in the firewall.
    pub fn non_tunnel_config(&self) -> &[IpAddr] {
        &self.non_tunnel_config
    }

    /// Whether the firewall should stop blocking DNS to resolvers other than those in this config.
    pub fn allow_external_dns(&self) -> bool {
        self.allow_external_dns
    }

    /// Consume `self` and return a vector of all addresses
    pub fn addresses(self) -> impl Iterator<Item = IpAddr> {
        self.non_tunnel_config.into_iter().chain(self.tunnel_config)
    }

    /// Return whether the config contains only (and at least one) loopback addresses, and zero
    /// non-loopback addresses
    pub fn is_loopback(&self) -> bool {
        let (loopback_addrs, non_loopback_addrs) = self
            .tunnel_config
            .iter()
            .chain(self.non_tunnel_config.iter())
            .copied()
            .partition::<Vec<_>, _>(|ip| ip.is_loopback());

        !loopback_addrs.is_empty() && non_loopback_addrs.is_empty()
    }
}

/// Sets and monitors system DNS settings. Makes sure the desired DNS servers are being used.
pub struct DnsMonitor {
    inner: imp::DnsMonitor,
}

impl DnsMonitor {
    /// Returns a new `DnsMonitor` that can set and monitor the system DNS.
    pub fn new(
        #[cfg(target_os = "linux")] handle: tokio::runtime::Handle,
        #[cfg(target_os = "linux")] route_manager: RouteManagerHandle,
    ) -> Result<Self, Error> {
        Ok(DnsMonitor {
            inner: imp::DnsMonitor::new(
                #[cfg(target_os = "linux")]
                handle,
                #[cfg(target_os = "linux")]
                route_manager,
            )?,
        })
    }

    /// Set DNS to the given servers. And start monitoring the system for changes.
    pub fn set(&mut self, interface: &str, config: ResolvedDnsConfig) -> Result<(), Error> {
        log::info!("Setting DNS servers: {config}",);
        self.inner.set(interface, config)
    }

    /// Reset system DNS settings to what it was before being set by this instance.
    /// This succeeds if the interface does not exist.
    pub fn reset(&mut self) -> Result<(), Error> {
        log::info!("Resetting DNS");
        self.inner.reset()
    }

    /// Reset DNS settings to what they were before being set by this instance.
    /// If the settings only affect a specific interface, this can be a no-op,
    /// as the interface will be destroyed.
    pub fn reset_before_interface_removal(&mut self) -> Result<(), Error> {
        log::info!("Resetting DNS");
        self.inner.reset_before_interface_removal()
    }
}

trait DnsMonitorT: Sized {
    type Error: std::error::Error;

    fn new(
        #[cfg(target_os = "linux")] handle: tokio::runtime::Handle,
        #[cfg(target_os = "linux")] route_manager: RouteManagerHandle,
    ) -> Result<Self, Self::Error>;

    fn set(&mut self, interface: &str, servers: ResolvedDnsConfig) -> Result<(), Self::Error>;

    fn reset(&mut self) -> Result<(), Self::Error>;

    fn reset_before_interface_removal(&mut self) -> Result<(), Self::Error> {
        self.reset()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::net::Ipv4Addr;

    fn resolve(config: &DnsConfig, default_tun: &[IpAddr]) -> ResolvedDnsConfig {
        config.resolve(
            default_tun,
            #[cfg(target_os = "macos")]
            53,
        )
    }

    const GATEWAY: [IpAddr; 1] = [IpAddr::V4(Ipv4Addr::new(10, 64, 0, 1))];

    /// The DNS leak protection must stay enforced unless explicitly opted out of.
    #[test]
    fn allow_external_dns_defaults_to_false() {
        assert!(!DnsConfig::default().allow_external_dns);
        assert!(!resolve(&DnsConfig::default(), &GATEWAY).allow_external_dns());
        assert!(!resolve(&DnsConfig::from_addresses(&[], &[]), &GATEWAY).allow_external_dns());
    }

    /// The opt-out flag must survive `resolve()` for both the gateway-default and override variants.
    #[test]
    fn resolve_threads_allow_external_dns() {
        let default_variant = DnsConfig::default().allow_external_dns(true);
        assert!(resolve(&default_variant, &GATEWAY).allow_external_dns());

        let custom = [IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))];
        let override_variant = DnsConfig::from_addresses(&custom, &[]).allow_external_dns(true);
        let resolved = resolve(&override_variant, &GATEWAY);
        assert!(resolved.allow_external_dns());
        assert_eq!(resolved.tunnel_config(), custom.as_slice());
    }
}
