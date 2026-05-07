//! Warren fork — Phase 4.E : wrapper daemon-side autour de la crate
//! [`warren_relay_selector::WarrenRelaySelector`].
//!
//! Rôle : encapsuler l'état de la `WarrenRelayList` côté daemon (sera
//! alimenté plus tard par un fetch périodique vers l'API), exposer une
//! API stable pour le `ParametersGenerator` (Phase 4.F future). Le
//! wrapper retourne uniquement les composants Iroh (`EndpointId` +
//! `EndpointAddr`) ; l'assemblage final en `WarrenIrohParameters` (avec
//! `signing_key`, `n_connections`, `features`) est fait par le caller
//! quand il a accès au `warren_signer` et à la config.
//!
//! **Pourquoi un module dédié** : (a) testable en isolation, (b)
//! n'importe pas `talpid-warren-iroh` → reste compilable même quand le
//! state machine wiring Phase 1.B n'est pas terminé.

use std::sync::Arc;

use warren_relay_selector::iroh_types::{EndpointAddr, EndpointId};
use warren_relay_selector::{
    SelectorError, WarrenRelay, WarrenRelayList, WarrenRelayQuery, WarrenRelaySelector,
};

/// Sortie minimale de la sélection : les seuls deux champs
/// nécessaires pour construire un `WarrenIrohParameters` côté caller.
///
/// Cloneable (les deux types Iroh le sont) pour permettre au caller
/// d'en garder une copie avant d'en faire un `WarrenIrohParameters`.
#[derive(Debug, Clone)]
pub struct WarrenSelection {
    /// Identité Ed25519 de l'exit Warren sélectionné.
    pub endpoint_id: EndpointId,

    /// Adresses candidate de l'exit (UDP IPv4/IPv6 + relay url
    /// optionnel).
    pub endpoint_addr: EndpointAddr,
}

impl From<&WarrenRelay> for WarrenSelection {
    fn from(relay: &WarrenRelay) -> Self {
        Self {
            endpoint_id: relay.endpoint_id(),
            endpoint_addr: relay.endpoint_addr().clone(),
        }
    }
}

/// Wrapper daemon-side autour du `WarrenRelaySelector`.
///
/// Détient un `Arc<WarrenRelaySelector>` pour permettre un partage
/// thread-safe entre le tunnel state machine et le management
/// interface gRPC (futur).
#[derive(Debug, Clone)]
pub struct DaemonWarrenRelaySelector {
    inner: Arc<WarrenRelaySelector>,
}

impl DaemonWarrenRelaySelector {
    /// Construit un wrapper depuis une [`WarrenRelayList`].
    #[must_use]
    pub fn new(relays: WarrenRelayList) -> Self {
        Self {
            inner: Arc::new(WarrenRelaySelector::new(relays)),
        }
    }

    /// Sélectionne un relay pour la tentative `retry_attempt` et
    /// retourne ses composants Iroh.
    ///
    /// API miroir de la
    /// [`mullvad_relay_selector::RelaySelector::get_relay`] côté
    /// WireGuard — facilite le dispatch par
    /// `ParametersGenerator::generate(retry_attempt, ...)`.
    ///
    /// # Errors
    ///
    /// Retourne [`SelectorError::NoRelayMatch`] si aucun relay actif
    /// avec `weight > 0` ne satisfait les contraintes.
    pub fn select_for_attempt(
        &self,
        query: &WarrenRelayQuery,
        retry_attempt: u32,
    ) -> Result<WarrenSelection, SelectorError> {
        self.inner
            .select_for_attempt(query, retry_attempt)
            .map(WarrenSelection::from)
    }
}

#[cfg(test)]
mod tests {
    use warren_relay_selector::iroh_types::{EndpointAddr, SecretKey};
    use warren_relay_selector::{Location, LocationConstraint, WarrenRelay};

    use super::*;

    fn endpoint_id(seed: u8) -> EndpointId {
        SecretKey::from_bytes(&[seed; 32]).public()
    }

    fn relay(seed: u8, country: &str, addr_str: &str) -> WarrenRelay {
        let id = endpoint_id(seed);
        let addr = EndpointAddr::new(id).with_ip_addr(addr_str.parse().unwrap());
        WarrenRelay::new(id, addr, Location::new(country, "_"), 100, true)
    }

    #[test]
    fn daemon_selector_returns_iroh_components_for_unconstrained_query() {
        // Phase 4.E : le wrapper doit déléguer correctement à la crate
        // upstream et retourner un `WarrenSelection` avec les deux
        // champs Iroh attendus par `WarrenIrohParameters` côté
        // talpid-warren-iroh.
        let list = WarrenRelayList::new(vec![relay(1, "se", "198.51.100.1:51820")]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let selection = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .expect("must select the only available relay");

        assert_eq!(selection.endpoint_id, endpoint_id(1));
        assert!(
            selection
                .endpoint_addr
                .ip_addrs()
                .any(|s| s.to_string() == "198.51.100.1:51820"),
            "endpoint_addr doit contenir l'IP source"
        );
    }

    #[test]
    fn daemon_selector_propagates_location_constraint() {
        // Le wrapper doit honorer les contraintes de la query (filtrage
        // géo). Si on demande FR, on ne doit pas tomber sur SE.
        let list = WarrenRelayList::new(vec![
            relay(1, "se", "198.51.100.1:51820"),
            relay(2, "fr", "198.51.100.2:51820"),
        ]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let query = WarrenRelayQuery::any().with_location(LocationConstraint::Country("fr".into()));
        for attempt in 0..10 {
            let selection = selector
                .select_for_attempt(&query, attempt)
                .expect("must select FR relay");
            assert_eq!(
                selection.endpoint_id,
                endpoint_id(2),
                "attempt {attempt} doit toujours retourner le relay FR"
            );
        }
    }

    #[test]
    fn daemon_selector_returns_error_when_no_match() {
        // Phase 4.E : si la liste est vide, l'erreur upstream doit
        // remonter telle quelle (pas de remap silencieux).
        let selector = DaemonWarrenRelaySelector::new(WarrenRelayList::new(vec![]));
        assert!(matches!(
            selector.select_for_attempt(&WarrenRelayQuery::any(), 0),
            Err(SelectorError::NoRelayMatch)
        ));
    }

    #[test]
    fn daemon_selector_is_cloneable_for_shared_use() {
        // Le wrapper est conçu pour être partagé entre threads (tunnel
        // state machine + gRPC management interface). Vérifie que
        // Clone produit deux handles vers la même liste sous-jacente.
        let list = WarrenRelayList::new(vec![relay(1, "se", "198.51.100.1:51820")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let cloned = selector.clone();

        let s1 = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .unwrap();
        let s2 = cloned
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .unwrap();
        assert_eq!(s1.endpoint_id, s2.endpoint_id);
    }
}
