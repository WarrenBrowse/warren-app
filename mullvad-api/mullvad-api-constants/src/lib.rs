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

/// Optional **pinned** IP for the API host — the single switch for the
/// bootstrap-privacy hardening (parity with upstream Mullvad).
///
/// When `Some(ip)`:
/// - the daemon dials the API at `ip` directly → **no DNS query** for
///   `api.warrenbrowse.com` at startup (the ISP never sees the hostname), and
/// - the TLS **SNI is omitted** (see `mullvad-api/src/tls_stream.rs`) → the
///   hostname never appears in cleartext on the wire either.
///
/// An on-path observer then sees only a TLS connection to a bare IP, exactly
/// like Mullvad's Direct method. Certificate validation still binds to
/// `api.warrenbrowse.com` (the SAN), so security is unchanged.
///
/// ⚠️ REQUIRES a server-side prerequisite: the API must be reachable on a
/// dedicated, stable IP whose TLS endpoint presents the `api.warrenbrowse.com`
/// certificate **without** SNI (Caddy `default_sni api.warrenbrowse.com`).
/// Today the API shares a Caddy vhost with checkout/admin and needs SNI to
/// select the cert, so this MUST stay `None` until that split is live. With
/// `None`, behaviour is unchanged (system DNS + SNI).
///
/// Resilience follow-up (Phase 2, not yet wired): DNS fallback when the pinned
/// IP is unreachable, plus in-tunnel address-cache refresh so a server IP
/// change self-heals without an app update. Until then, only pin a stable
/// (e.g. Hetzner floating) IP.
pub const API_PINNED_IP: Option<IpAddr> = None;
