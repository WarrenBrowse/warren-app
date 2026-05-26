//! Union enum of tunnel parameters for the two backends (WireGuard
//! upstream + Warren) - the abstraction stored by `ConnectingState`
//! and `ConnectedState`.
//!
//! The accessor methods (`get_tunnel_endpoint`, `get_next_hop_endpoints`,
//! `get_exit_hop_endpoint`) are the only way the state machine reads
//! params: no WG-specific field (`private_key`, `addresses`, `daita`,
//! ...) is consumed outside of `WireguardMonitor::start` which gets the
//! concrete WG variant.

use std::net::SocketAddr;

use talpid_types::net::wireguard::TunnelParameters as WireguardTunnelParameters;

use talpid_types::net::{Endpoint, TransportProtocol, TunnelEndpoint};
use talpid_warren_tunnel::WarrenTunnelParameters;

/// Lightweight snapshot of Warren tunnel endpoint data needed by the
/// state machine (firewall rules, GUI transitions, relay display).
///
/// Intentionally does NOT carry `SigningKey` or other secret material:
/// `WarrenTunnelParameters` is moved into the tunnel thread; this
/// clone-safe summary is what `BackendParams::Warren` stores.
#[derive(Debug, Clone)]
pub(crate) struct WarrenBackendInfo {
    pub exit_candidates: Vec<SocketAddr>,
    pub relay_endpoint: Option<SocketAddr>,
    pub exit_endpoint: Option<SocketAddr>,
    pub enable_daita: bool,
}

impl WarrenBackendInfo {
    pub fn from_params(p: &WarrenTunnelParameters) -> Self {
        let (relay_endpoint, exit_endpoint) = match &p.multi_hop {
            Some(mh) => (Some(mh.relay.endpoint), Some(mh.exit.endpoint)),
            None => (None, None),
        };
        Self {
            exit_candidates: p.exit_addr.ip_addrs().collect(),
            relay_endpoint,
            exit_endpoint,
            enable_daita: p.enable_daita,
        }
    }
}

/// Typed tunnel parameters, agnostic of the underlying backend.
///
/// Stored in `ConnectingState` and `ConnectedState`. The variants
/// expose only what the state machine needs (firewall, transitions,
/// GUI display); backend-specific consumption lives downstream in
/// `TunnelMonitor::start{,_warren_tunnel}`.
#[derive(Debug, Clone)]
pub(crate) enum BackendParams {
    Wireguard(WireguardTunnelParameters),
    Warren(WarrenBackendInfo),
}

impl BackendParams {
    /// `TunnelEndpoint` published in `TunnelStateTransition::Connecting`
    /// then `Connected` (consumed by the GUI / management interface to
    /// display the current exit).
    pub fn get_tunnel_endpoint(&self) -> TunnelEndpoint {
        match self {
            Self::Wireguard(p) => p.get_tunnel_endpoint(),
            Self::Warren(p) => warren_tunnel_endpoint(p),
        }
    }

    /// UDP endpoints to allow through the firewall pre-handshake (the
    /// client -> next-hop outbound UDP path).
    ///
    /// - Wireguard: the peer endpoint (or the obfuscator endpoints if
    ///   active).
    /// - Warren single-hop: every candidate IP of the exit (client
    ///   dials the exit directly).
    /// - Warren multi-hop: only the relay endpoint (the client never
    ///   sends UDP to the exit directly; the relay forwards the QUIC
    ///   datagrams on a separate C2 connection).
    pub fn get_next_hop_endpoints(&self) -> Vec<Endpoint> {
        match self {
            Self::Wireguard(p) => p.get_next_hop_endpoints(),
            Self::Warren(info) => match info.relay_endpoint {
                Some(relay) => vec![Endpoint::from_socket_address(
                    relay,
                    TransportProtocol::Udp,
                )],
                None => info
                    .exit_candidates
                    .iter()
                    .map(|&addr| Endpoint::from_socket_address(addr, TransportProtocol::Udp))
                    .collect(),
            },
        }
    }

    /// Secondary exit endpoint for the WireGuard multihop path
    /// (Windows split-tunnel only). Warren does not support multihop:
    /// returns `None`.
    #[cfg(target_os = "windows")]
    pub fn get_exit_hop_endpoint(&self) -> Option<Endpoint> {
        match self {
            Self::Wireguard(p) => p.get_exit_hop_endpoint(),
            Self::Warren(_) => None,
        }
    }
}

/// Build the `TunnelEndpoint` published in the state transition for a
/// Warren tunnel.
///
/// - Single-hop: `endpoint` = first candidate IP of the exit (typically
///   v4) over UDP, `entry_endpoint` = `None`.
/// - Multi-hop: `endpoint` = the exit descriptor's advertised endpoint
///   (= the conceptual peer the user is exiting through), and
///   `entry_endpoint` = `Some(relay.endpoint)` (= the first hop the
///   client actually dials). Mirrors the Wireguard multi-hop convention
///   so the GUI displays the entry/exit pair consistently across
///   backends.
fn warren_tunnel_endpoint(info: &WarrenBackendInfo) -> TunnelEndpoint {
    use std::net::{IpAddr, Ipv4Addr};

    let (endpoint_addr, entry_endpoint) = match (info.exit_endpoint, info.relay_endpoint) {
        (Some(exit_ep), Some(relay_ep)) => (
            exit_ep,
            Some(Endpoint::from_socket_address(
                relay_ep,
                TransportProtocol::Udp,
            )),
        ),
        _ => (
            info.exit_candidates
                .first()
                .copied()
                .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)),
            None,
        ),
    };

    TunnelEndpoint {
        endpoint: Endpoint::from_socket_address(endpoint_addr, TransportProtocol::Udp),
        quantum_resistant: false,
        obfuscation: None,
        entry_endpoint,
        tunnel_interface: None,
        daita: info.enable_daita,
        tunnel_type: talpid_types::net::TunnelType::Warren,
    }
}

#[cfg(test)]
mod tests {
    //! Security regression: validate that the accessors produce the
    //! right data for the Warren path. A mapping bug on
    //! `exit_addr.ip_addrs()` -> firewall would cause either a leak or
    //! the inability to complete the QUIC handshake.

    use std::net::SocketAddr;

    use talpid_types::net::TransportProtocol;

    use super::{BackendParams, WarrenBackendInfo};

    fn fixture_warren(addrs: &[&str]) -> WarrenBackendInfo {
        WarrenBackendInfo {
            exit_candidates: addrs.iter().map(|s| s.parse().unwrap()).collect(),
            relay_endpoint: None,
            exit_endpoint: None,
            enable_daita: false,
        }
    }

    fn fixture_warren_multi_hop(
        relay_endpoint: &str,
        exit_endpoint: &str,
    ) -> WarrenBackendInfo {
        let mut info = fixture_warren(&["203.0.113.99:443"]);
        info.relay_endpoint = Some(relay_endpoint.parse().unwrap());
        info.exit_endpoint = Some(exit_endpoint.parse().unwrap());
        info
    }

    #[test]
    fn warren_get_next_hop_endpoints_maps_all_ip_candidates_as_udp() {
        // Critical regression: if we forgot to map the candidate
        // `ip_addrs` to allowed firewall endpoints, the QUIC handshake
        // would not get through and no tunnel could be established.
        let params = fixture_warren(&["198.51.100.7:51820", "[2001:db8::7]:51820"]);
        let backend = BackendParams::Warren(params);

        let endpoints = backend.get_next_hop_endpoints();
        assert_eq!(endpoints.len(), 2, "must expose every candidate IP");

        let mut found_v4 = false;
        let mut found_v6 = false;
        for ep in &endpoints {
            assert_eq!(
                ep.protocol,
                TransportProtocol::Udp,
                "Warren is QUIC over UDP, never TCP"
            );
            match ep.address {
                SocketAddr::V4(_) => found_v4 = true,
                SocketAddr::V6(_) => found_v6 = true,
            }
        }
        assert!(found_v4, "IPv4 candidate must be present");
        assert!(found_v6, "IPv6 candidate must be present");
    }

    #[test]
    fn warren_get_next_hop_endpoints_empty_when_no_candidates() {
        // Edge case: if the selector returned an exit_addr with no
        // candidate IP (degraded case), the firewall authorizes
        // nothing. That is the safe behaviour (no-leak by default).
        let params = fixture_warren(&[]);
        let backend = BackendParams::Warren(params);

        assert!(
            backend.get_next_hop_endpoints().is_empty(),
            "no firewall leak when no candidates"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn warren_get_exit_hop_endpoint_returns_none() {
        // Critical Windows split-tunnel regression: Warren does not
        // support multihop. Returning `Some` by mistake would lead
        // the Windows firewall to try to authorize a non-existent
        // secondary exit.
        let params = fixture_warren(&["198.51.100.7:51820"]);
        let backend = BackendParams::Warren(params);
        assert!(backend.get_exit_hop_endpoint().is_none());
    }

    #[test]
    fn warren_get_tunnel_endpoint_uses_first_candidate_as_udp() {
        // The GUI displays this endpoint to the user. Returning
        // `0.0.0.0:0` or TCP by mistake would mislead the UI.
        let params = fixture_warren(&["198.51.100.42:9999", "[2001:db8::42]:9999"]);
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.protocol, TransportProtocol::Udp);
        assert_eq!(
            te.endpoint.address,
            "198.51.100.42:9999".parse::<SocketAddr>().unwrap(),
            "first candidate IP exposed"
        );
        assert!(
            te.entry_endpoint.is_none(),
            "Warren has no multihop entry endpoint"
        );
    }

    #[test]
    fn warren_multi_hop_get_next_hop_endpoints_returns_only_relay_endpoint() {
        // Critical firewall regression: on the multi-hop path the
        // client never sends UDP to the exit directly; the relay
        // forwards datagrams on a separate C2 connection. Opening the
        // exit candidate IPs in the firewall would punch unnecessary
        // holes and break the threat model (the only IP the daemon
        // sends to on the data plane is the relay).
        let params = fixture_warren_multi_hop("198.51.100.10:443", "198.51.100.20:443");
        let backend = BackendParams::Warren(params);

        let endpoints = backend.get_next_hop_endpoints();
        assert_eq!(
            endpoints.len(),
            1,
            "multi-hop must expose exactly one next-hop (the relay), got {endpoints:?}"
        );
        let only = &endpoints[0];
        assert_eq!(only.protocol, TransportProtocol::Udp);
        assert_eq!(
            only.address,
            "198.51.100.10:443".parse::<SocketAddr>().unwrap(),
            "next-hop must be the relay endpoint, not the exit"
        );
    }

    #[test]
    fn warren_multi_hop_get_tunnel_endpoint_uses_exit_with_relay_entry() {
        // GUI mirror of the Wireguard multi-hop convention: `endpoint`
        // is the conceptual exit peer (= the IP the user effectively
        // exits through), `entry_endpoint` is the first hop the
        // client actually dials (= the relay). A regression that
        // swaps these would confuse the UI relay-pair display and the
        // "where am I exiting from" panel.
        let params = fixture_warren_multi_hop("198.51.100.10:443", "198.51.100.20:443");
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.protocol, TransportProtocol::Udp);
        assert_eq!(
            te.endpoint.address,
            "198.51.100.20:443".parse::<SocketAddr>().unwrap(),
            "`endpoint` must surface the exit descriptor endpoint"
        );
        let entry = te
            .entry_endpoint
            .expect("multi-hop must publish an entry_endpoint");
        assert_eq!(entry.protocol, TransportProtocol::Udp);
        assert_eq!(
            entry.address,
            "198.51.100.10:443".parse::<SocketAddr>().unwrap(),
            "`entry_endpoint` must surface the relay descriptor endpoint"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn warren_multi_hop_get_exit_hop_endpoint_returns_none() {
        // The Windows split-tunnel exit-hop accessor stays `None` for
        // Warren even in multi-hop mode: Warren's "exit hop" is
        // already reflected in `get_tunnel_endpoint().endpoint`, and
        // the Wireguard-style secondary peer notion does not apply
        // (the relay-exit C2 hop is internal to the data plane, not
        // visible to the Windows firewall).
        let params = fixture_warren_multi_hop("198.51.100.10:443", "198.51.100.20:443");
        let backend = BackendParams::Warren(params);
        assert!(backend.get_exit_hop_endpoint().is_none());
    }

    #[test]
    fn warren_get_tunnel_endpoint_falls_back_to_unspecified_when_empty() {
        // Symmetric edge case: if there is no candidate IP, return a
        // UNSPECIFIED:0 placeholder rather than panic. This robustness
        // lets the Connecting transition still fire; the downstream
        // `WarrenTunnelMonitor::start` then fails cleanly with a visible
        // error.
        let params = fixture_warren(&[]);
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.address.port(), 0);
        assert!(te.endpoint.address.ip().is_unspecified());
    }
}
