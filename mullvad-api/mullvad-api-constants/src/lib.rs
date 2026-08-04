use std::net::{IpAddr, Ipv4Addr};

pub mod env {
    pub const API_HOST_VAR: &str = "MULLVAD_API_HOST";
    pub const API_ADDR_VAR: &str = "MULLVAD_API_ADDR";
    pub const API_FORCE_DIRECT_VAR: &str = "MULLVAD_API_FORCE_DIRECT";
    pub const DISABLE_TLS_VAR: &str = "MULLVAD_API_DISABLE_TLS";
}

/// Default API host of the compiled product environment
/// (`WARREN_PRODUCT_ENV`: prod | staging | beta, prod when unset).
pub const API_HOST_DEFAULT: &str = warren_product_env::API_HOST;
/// Warren does not pin a hardcoded API IP (unlike upstream Mullvad, which
/// pinned `api.mullvad.net` to a fixed address for censorship resistance).
/// The default host is resolved via DNS at startup; this unspecified sentinel
/// is only a last-resort fallback when resolution fails.
pub const API_IP_DEFAULT: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
pub const API_PORT_DEFAULT: u16 = 443;

/// Optional **pinned** IP for the API host - the single switch for the
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
/// STATUS: intentionally **deferred / left `None`**.
/// Full rationale + the exact resume steps live in warren-core
/// `docs/31-API-BOOTSTRAP-PRIVACY.md`. Read that before changing this.
///
/// ⚠️ REQUIRES a server-side prerequisite: the API must answer on a dedicated,
/// stable IP that serves the `api.warrenbrowse.com` certificate **without**
/// SNI. Today the API shares a Caddy host with checkout/admin, where Caddy
/// needs SNI to pick the cert; the `default_sni` workaround was tested live and
/// does NOT work for a multi-site host (caddy#6979). The reliable prerequisite
/// is a **dedicated single-vhost host** for the API. So this MUST stay `None`
/// until that host exists. With `None`, behaviour is unchanged (system DNS + SNI).
///
/// Resilience: a dead or rotated pinned IP triggers a one-shot DNS fallback at
/// startup (`seed_pinned_or_dns_fallback` in `mullvad-api`), so the app still
/// boots (it merely gives up the privacy benefit for that session). A stable
/// (e.g. floating) IP is still recommended. Mid-session self-heal via the
/// in-tunnel `api-addrs` cache refresh is a separate follow-up.
pub const API_PINNED_IP: Option<IpAddr> = None;
