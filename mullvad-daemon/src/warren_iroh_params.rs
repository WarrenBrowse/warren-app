//! Assemblage d'un [`talpid_warren_iroh::WarrenIrohParameters`]
//! complet à partir des briques séparées (sélecteur Iroh + signing
//! key BIP39 + constantes config).
//!
//! Module dédié pour deux raisons : testable en isolation, et point
//! unique de décision pour les paramètres non-issus de la sélection
//! (`n_connections`, `features`) qui peuvent évoluer indépendamment.
//!
//! Caller : [`crate::tunnel::ParametersGenerator::produce_warren_iroh_params`],
//! invoqué depuis le tunnel state machine quand le mode Warren est
//! actif.

use ed25519_dalek::SigningKey;
use talpid_warren_iroh::WarrenIrohParameters;
use warren_relay_selector::{SelectorError, WarrenRelayQuery};

use crate::warren_relay_selector::DaemonWarrenRelaySelector;

/// Erreurs de l'assemblage des paramètres Iroh.
#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    /// Aucun relay Warren ne satisfait la query (ou liste vide).
    #[error("warren relay selection failed: {0}")]
    Selector(#[from] SelectorError),
}

/// Nombre de connexions QUIC parallèles. `1` = mono-conn (baseline) ;
/// le multi-conn (bonding) sera activable quand les benchs justifieront
/// la complexité + la résolution des bugs perf (cf.
/// `warren-pocs/docs/13-M3-RADICAL-PERF-RESEARCH.md`).
const DEFAULT_N_CONNECTIONS: u8 = 1;

/// Bitmask `features` annoncé dans le `Setup` Warren. `0` = baseline
/// IPv4 only, pas de port-forward, pas de multipath. À étendre quand
/// les settings UI exposeront ces options.
const DEFAULT_FEATURES: u32 = 0;

/// Assemble un [`WarrenIrohParameters`] complet pour la tentative
/// `retry_attempt` donnée.
///
/// # Errors
///
/// Retourne [`AssembleError::Selector`] si le sélecteur ne trouve
/// aucun relay matchant la query.
pub fn assemble_for_attempt(
    selector: &DaemonWarrenRelaySelector,
    signing_key: SigningKey,
    query: &WarrenRelayQuery,
    retry_attempt: u32,
) -> Result<WarrenIrohParameters, AssembleError> {
    let selection = selector.select_for_attempt(query, retry_attempt)?;
    Ok(WarrenIrohParameters {
        exit_id: selection.endpoint_id,
        exit_addr: selection.endpoint_addr,
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use warren_relay_selector::iroh_types::{EndpointAddr, SecretKey};
    use warren_relay_selector::{Location, LocationConstraint, WarrenRelay, WarrenRelayList};

    use super::*;

    fn fixture_signing_key() -> SigningKey {
        // Seed déterministe pour les tests : ed25519_dalek SigningKey
        // accepte un seed [u8; 32].
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn fixture_relay(seed: u8, country: &str) -> WarrenRelay {
        let id = SecretKey::from_bytes(&[seed; 32]).public();
        let addr = EndpointAddr::new(id).with_ip_addr("198.51.100.1:51820".parse().unwrap());
        WarrenRelay::new(id, addr, Location::new(country, "_"), 100, true)
    }

    #[test]
    fn assemble_combines_selection_signing_key_and_constants() {
        // La fonction doit produire un `WarrenIrohParameters` dont
        // `exit_id` + `exit_addr` viennent du selector, `signing_key`
        // est passée telle quelle, et les 2 constantes
        // `n_connections=1` + `features=0` sont posées.
        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let key = fixture_signing_key();
        let expected_pubkey = key.verifying_key().to_bytes();

        let params = assemble_for_attempt(&selector, key, &WarrenRelayQuery::any(), 0)
            .expect("must assemble valid params");

        let expected_id = SecretKey::from_bytes(&[1u8; 32]).public();
        assert_eq!(params.exit_id, expected_id);
        assert!(
            params
                .exit_addr
                .ip_addrs()
                .any(|s| s.to_string() == "198.51.100.1:51820"),
            "exit_addr doit contenir l'IP source"
        );
        assert_eq!(
            params.signing_key.verifying_key().to_bytes(),
            expected_pubkey,
            "signing_key doit être passée telle quelle"
        );
        assert_eq!(params.n_connections, 1);
        assert_eq!(params.features, 0);
    }

    #[test]
    fn assemble_propagates_selector_error_when_no_match() {
        // Si la query ne match aucun relay, l'erreur upstream doit
        // remonter via `AssembleError::Selector` (pas de remap qui
        // masquerait la cause).
        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let query = WarrenRelayQuery::any().with_location(LocationConstraint::Country("zz".into()));

        let err = assemble_for_attempt(&selector, fixture_signing_key(), &query, 0)
            .expect_err("must fail");
        assert!(matches!(
            err,
            AssembleError::Selector(SelectorError::NoRelayMatch)
        ));
    }

    #[test]
    fn assemble_is_idempotent_for_same_attempt() {
        // Même retry_attempt + même selector + même signing_key →
        // mêmes paramètres assemblés. Crucial pour l'idempotence du
        // state machine (reprise après crash).
        let list = WarrenRelayList::new(vec![
            fixture_relay(1, "se"),
            fixture_relay(2, "fr"),
            fixture_relay(3, "de"),
        ]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let params_a = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            42,
        )
        .unwrap();
        let params_b = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            42,
        )
        .unwrap();

        assert_eq!(params_a.exit_id, params_b.exit_id);
        assert_eq!(
            params_a.signing_key.verifying_key().to_bytes(),
            params_b.signing_key.verifying_key().to_bytes()
        );
    }
}
