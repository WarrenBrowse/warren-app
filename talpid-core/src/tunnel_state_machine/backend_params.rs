//! Enum union des paramètres de tunnel pour les deux backends
//! (WireGuard upstream + Warren-Iroh) — abstraction stockée par les
//! états `ConnectingState` et `ConnectedState`.
//!
//! Les méthodes accessor (`get_tunnel_endpoint`,
//! `get_next_hop_endpoints`, `get_exit_hop_endpoint`) sont la seule
//! façon dont le state machine consomme les paramètres : aucun champ
//! WG-spécifique (`private_key`, `addresses`, `daita`, …) n'est lu
//! hors de `WireguardMonitor::start` qui reçoit le variant WG concret.

use std::net::SocketAddr;

use talpid_types::net::wireguard::TunnelParameters as WireguardTunnelParameters;
use talpid_types::net::{Endpoint, TransportProtocol, TunnelEndpoint};
use talpid_warren_iroh::WarrenIrohParameters;

/// Paramètres typés du tunnel à établir, agnostiques du backend.
///
/// Stockés dans les états `ConnectingState` et `ConnectedState`.
/// Les variants exposent uniquement ce dont le state machine a besoin
/// (firewall, transitions, affichage GUI) — la consommation
/// backend-spécifique est faite en aval, dans
/// `TunnelMonitor::start{,_warren_iroh}`.
#[derive(Debug, Clone)]
pub(crate) enum BackendParams {
    Wireguard(WireguardTunnelParameters),
    Warren(WarrenIrohParameters),
}

impl BackendParams {
    /// `TunnelEndpoint` à publier dans `TunnelStateTransition::Connecting`
    /// puis `Connected` (consommé par la GUI / management interface
    /// pour afficher l'exit en cours / actuel).
    pub fn get_tunnel_endpoint(&self) -> TunnelEndpoint {
        match self {
            Self::Wireguard(p) => p.get_tunnel_endpoint(),
            Self::Warren(p) => warren_tunnel_endpoint(p),
        }
    }

    /// Liste des endpoints UDP à autoriser au firewall pre-handshake
    /// (path UDP outgoing du client → exit). Pour Warren, c'est
    /// l'ensemble des candidates `endpoint_addr.ip_addrs()` ; pour
    /// WireGuard, c'est le peer endpoint (ou les endpoints
    /// d'obfuscateur si actif).
    pub fn get_next_hop_endpoints(&self) -> Vec<Endpoint> {
        match self {
            Self::Wireguard(p) => p.get_next_hop_endpoints(),
            Self::Warren(p) => p
                .exit_addr
                .ip_addrs()
                .copied()
                .map(|addr| Endpoint::from_socket_address(addr, TransportProtocol::Udp))
                .collect(),
        }
    }

    /// Endpoint d'exit secondaire pour le path multihop WireGuard
    /// (Windows split-tunnel only). Warren ne supporte pas le
    /// multihop : retourne `None`.
    #[cfg(target_os = "windows")]
    pub fn get_exit_hop_endpoint(&self) -> Option<Endpoint> {
        match self {
            Self::Wireguard(p) => p.get_exit_hop_endpoint(),
            Self::Warren(_) => None,
        }
    }
}

/// Construit le `TunnelEndpoint` exposé en transition pour un tunnel
/// Warren. La GUI affichera la première IP candidate de l'exit
/// (généralement v4 en priorité) avec protocol UDP.
fn warren_tunnel_endpoint(params: &WarrenIrohParameters) -> TunnelEndpoint {
    use std::net::{IpAddr, Ipv4Addr};

    let socket_addr = params
        .exit_addr
        .ip_addrs()
        .next()
        .copied()
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
    //! Régression sécurité : valider que les accessors produisent les
    //! bonnes données pour le path Warren — un bug de mapping
    //! `endpoint_addr.ip_addrs()` → firewall causerait un leak ou
    //! l'impossibilité de handshake QUIC.

    use std::net::SocketAddr;

    use ed25519_dalek::SigningKey;
    use iroh::{EndpointAddr, SecretKey};
    use talpid_types::net::TransportProtocol;
    use talpid_warren_iroh::WarrenIrohParameters;

    use super::BackendParams;

    fn fixture_warren(addrs: &[&str]) -> WarrenIrohParameters {
        let exit_secret = SecretKey::from_bytes(&[7u8; 32]);
        let exit_id = exit_secret.public();
        let mut addr = EndpointAddr::new(exit_id);
        for s in addrs {
            addr = addr.with_ip_addr(s.parse::<SocketAddr>().unwrap());
        }
        WarrenIrohParameters {
            exit_id,
            exit_addr: addr,
            signing_key: SigningKey::from_bytes(&[9u8; 32]),
            n_connections: 1,
            features: 0,
        }
    }

    #[test]
    fn warren_get_next_hop_endpoints_maps_all_ip_candidates_as_udp() {
        // Régression critique : si on oubliait de mapper les ip_addrs
        // candidates Iroh, le firewall ne laisserait pas passer le
        // handshake QUIC → tunnel impossible à établir.
        let params = fixture_warren(&["198.51.100.7:51820", "[2001:db8::7]:51820"]);
        let backend = BackendParams::Warren(params);

        let endpoints = backend.get_next_hop_endpoints();
        assert_eq!(endpoints.len(), 2, "doit exposer toutes les IPs candidate");

        let mut found_v4 = false;
        let mut found_v6 = false;
        for ep in &endpoints {
            assert_eq!(
                ep.protocol,
                TransportProtocol::Udp,
                "Iroh est en QUIC = UDP, jamais TCP"
            );
            match ep.address {
                SocketAddr::V4(_) => found_v4 = true,
                SocketAddr::V6(_) => found_v6 = true,
            }
        }
        assert!(found_v4, "IPv4 candidate doit être présente");
        assert!(found_v6, "IPv6 candidate doit être présente");
    }

    #[test]
    fn warren_get_next_hop_endpoints_empty_when_no_candidates() {
        // Edge case : si le selector retourne un EndpointAddr sans
        // aucune IP (cas dégradé), le firewall n'autorise rien — c'est
        // le comportement sûr (no-leak by default).
        let params = fixture_warren(&[]);
        let backend = BackendParams::Warren(params);

        assert!(
            backend.get_next_hop_endpoints().is_empty(),
            "pas de leak via firewall si pas de candidates"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn warren_get_exit_hop_endpoint_returns_none() {
        // Régression critique Windows split-tunnel : Warren ne
        // supporte pas le multihop. Si on retournait Some par erreur,
        // le firewall Windows tenterait d'autoriser un exit secondaire
        // inexistant → règle inopérante mais surtout bug de typage
        // pour les implémenteurs split-tunnel.
        let params = fixture_warren(&["198.51.100.7:51820"]);
        let backend = BackendParams::Warren(params);
        assert!(backend.get_exit_hop_endpoint().is_none());
    }

    #[test]
    fn warren_get_tunnel_endpoint_uses_first_candidate_as_udp() {
        // Pertinent : la GUI affiche cet endpoint à l'utilisateur.
        // Si on retournait un `0.0.0.0:0` ou TCP par erreur, l'UI
        // serait trompeuse (= signal "tunnel établi vers nulle part").
        let params = fixture_warren(&["198.51.100.42:9999", "[2001:db8::42]:9999"]);
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.protocol, TransportProtocol::Udp);
        assert_eq!(
            te.endpoint.address,
            "198.51.100.42:9999".parse::<SocketAddr>().unwrap(),
            "première IP candidate exposée"
        );
        assert!(
            te.entry_endpoint.is_none(),
            "Warren n'a pas de multihop entry"
        );
    }

    #[test]
    fn warren_get_tunnel_endpoint_falls_back_to_unspecified_when_empty() {
        // Edge case symétrique : si pas d'IP candidate, on retourne
        // un placeholder UNSPECIFIED:0 plutôt que panic. Cette
        // robustesse permet à la transition Connecting de quand même
        // partir, et le `WarrenIrohMonitor::start` qui suit échouera
        // proprement avec une erreur visible.
        let params = fixture_warren(&[]);
        let backend = BackendParams::Warren(params);

        let te = backend.get_tunnel_endpoint();
        assert_eq!(te.endpoint.address.port(), 0);
        assert!(te.endpoint.address.ip().is_unspecified());
    }
}
