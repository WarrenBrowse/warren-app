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
use talpid_warren_iroh::WarrenIrohParameters;

/// Typed tunnel parameters, agnostic of the underlying backend.
///
/// Stored in `ConnectingState` and `ConnectedState`. The variants
/// expose only what the state machine needs (firewall, transitions,
/// GUI display); backend-specific consumption lives downstream in
/// `TunnelMonitor::start{,_warren_iroh}`.
#[derive(Debug, Clone)]
pub(crate) enum BackendParams {
    Wireguard(WireguardTunnelParameters),
    Warren(WarrenIrohParameters),
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
    /// client -> exit outbound UDP path). For Warren this is the full
    /// set of `exit_addr.ip_addrs()` candidates; for WireGuard, the
    /// peer endpoint (or the obfuscator endpoints if active).
    pub fn get_next_hop_endpoints(&self) -> Vec<Endpoint> {
        match self {
            Self::Wireguard(p) => p.get_next_hop_endpoints(),
            Self::Warren(p) => p
                .exit_addr
                .ip_addrs()
                .map(|addr| Endpoint::from_socket_address(addr, TransportProtocol::Udp))
                .collect(),
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
/// Warren tunnel. The GUI displays the first candidate IP of the exit
/// (typically v4 first) over UDP.
fn warren_tunnel_endpoint(params: &WarrenIrohParameters) -> TunnelEndpoint {
    use std::net::{IpAddr, Ipv4Addr};

    let socket_addr = params
        .exit_addr
        .ip_addrs()
        .next()
        .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));

    TunnelEndpoint {
        endpoint: Endpoint::from_socket_address(socket_addr, TransportProtocol::Udp),
        quantum_resistant: false,
        obfuscation: None,
        entry_endpoint: None,
        tunnel_interface: None,
        daita: false,
    }
}

#[cfg(test)]
mod tests {
    //! Security regression: validate that the accessors produce the
    //! right data for the Warren path. A mapping bug on
    //! `exit_addr.ip_addrs()` -> firewall would cause either a leak or
    //! the inability to complete the QUIC handshake.

    use std::net::SocketAddr;

    use ed25519_dalek::SigningKey;
    use talpid_types::net::TransportProtocol;
    use talpid_warren_iroh::WarrenIrohParameters;
    use warren_protocol::{WarrenExitAddr, WarrenPubkey};

    use super::BackendParams;

    fn fixture_warren(addrs: &[&str]) -> WarrenIrohParameters {
        let exit_id = WarrenPubkey::from_bytes([7u8; 32]);
        let mut addr = WarrenExitAddr::new(exit_id);
        for s in addrs {
            addr = addr.with_ip_addr(s.parse::<SocketAddr>().unwrap());
        }
        WarrenIrohParameters {
            exit_addr: addr,
            signing_key: SigningKey::from_bytes(&[9u8; 32]),
            n_connections: 1,
            features: 0,
        }
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
    fn warren_get_tunnel_endpoint_falls_back_to_unspecified_when_empty() {
        // Symmetric edge case: if there is no candidate IP, return a
        // UNSPECIFIED:0 placeholder rather than panic. This robustness
        // lets the Connecting transition still fire; the downstream
        // `WarrenIrohMonitor::start` then fails cleanly with a visible
        // error.
        let params = fixture_warren(&[]);
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.address.port(), 0);
        assert!(te.endpoint.address.ip().is_unspecified());
    }
}
