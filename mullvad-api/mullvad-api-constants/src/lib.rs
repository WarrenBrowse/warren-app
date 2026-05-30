use std::net::{IpAddr, Ipv4Addr};

pub mod env {
    pub const API_HOST_VAR: &str = "MULLVAD_API_HOST";
    pub const API_ADDR_VAR: &str = "MULLVAD_API_ADDR";
    pub const API_FORCE_DIRECT_VAR: &str = "MULLVAD_API_FORCE_DIRECT";
    pub const DISABLE_TLS_VAR: &str = "MULLVAD_API_DISABLE_TLS";
}

pub const API_HOST_DEFAULT: &str = "api.warrenbrowse.com";
/// Warren does not pin a hardcoded API IP (unlike upstream Mullvad, which
/// pinned `api.mullvad.net` to a fixed address for censorship resistance).
/// The default host is resolved via DNS at startup; this unspecified sentinel
/// is only a last-resort fallback when resolution fails.
pub const API_IP_DEFAULT: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
pub const API_PORT_DEFAULT: u16 = 443;
