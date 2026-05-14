//! Builds a complete [`talpid_warren_tunnel::WarrenTunnelParameters`]
//! from the separate building blocks (relay selector + BIP39 signing
//! key + config constants).
//!
//! Dedicated module for two reasons: it can be tested in isolation,
//! and it is the single place where parameters not picked by the
//! selector (`n_connections`, `features`) are decided.
//!
//! Caller: [`crate::tunnel::ParametersGenerator::produce_warren_tunnel_params`],
//! invoked from the tunnel state machine when Warren mode is active.

use ed25519_dalek::SigningKey;
use talpid_warren_tunnel::WarrenTunnelParameters;
use warren_relay_selector::{SelectorError, WarrenRelayQuery};

use crate::warren_relay_selector::DaemonWarrenRelaySelector;

/// Errors raised while assembling Warren tunnel parameters.
#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    /// No Warren relay matched the query (or the list was empty).
    #[error("warren relay selection failed: {0}")]
    Selector(#[from] SelectorError),
}

/// Number of parallel QUIC connections. `1` = mono-conn (baseline);
/// multi-conn (bonding) gets enabled once the benches justify the
/// extra complexity.
const DEFAULT_N_CONNECTIONS: u8 = 1;

/// `features` bitmask advertised in the Warren `Setup` frame. `0` =
/// IPv4-only baseline, no port-forward, no multipath. To be extended
/// once the UI settings surface these options.
const DEFAULT_FEATURES: u32 = 0;

/// Assembles a full [`WarrenTunnelParameters`] for the given
/// `retry_attempt`.
///
/// # Errors
///
/// Returns [`AssembleError::Selector`] if no relay matches the query.
pub fn assemble_for_attempt(
    selector: &DaemonWarrenRelaySelector,
    signing_key: SigningKey,
    query: &WarrenRelayQuery,
    retry_attempt: u32,
) -> Result<WarrenTunnelParameters, AssembleError> {
    let selection = selector.select_for_attempt(query, retry_attempt)?;
    Ok(WarrenTunnelParameters {
        exit_addr: selection.endpoint_addr,
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use warren_relay_selector::warren_types::{WarrenExitAddr, WarrenPubkey};
    use warren_relay_selector::{Location, LocationConstraint, WarrenRelay, WarrenRelayList};

    use super::*;

    fn fixture_signing_key() -> SigningKey {
        // Deterministic seed for the tests.
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn fixture_relay(seed: u8, country: &str) -> WarrenRelay {
        let id = WarrenPubkey::from_bytes([seed; 32]);
        let addr = WarrenExitAddr::new(id).with_ip_addr("198.51.100.1:51820".parse().unwrap());
        WarrenRelay::new(id, addr, Location::new(country, "_"), 100, true)
    }

    #[test]
    fn assemble_combines_selection_signing_key_and_constants() {
        // The function must produce a `WarrenTunnelParameters` whose
        // `exit_addr` comes from the selector, whose `signing_key` is
        // passed through verbatim, and where the two constants
        // `n_connections == 1` + `features == 0` are set.
        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let key = fixture_signing_key();
        let expected_pubkey = key.verifying_key().to_bytes();

        let params = assemble_for_attempt(&selector, key, &WarrenRelayQuery::any(), 0)
            .expect("must assemble valid params");

        let expected_id = WarrenPubkey::from_bytes([1u8; 32]);
        assert_eq!(params.exit_addr.id, expected_id);
        assert!(
            params
                .exit_addr
                .ip_addrs()
                .any(|s| s.to_string() == "198.51.100.1:51820"),
            "exit_addr must contain the source IP"
        );
        assert_eq!(
            params.signing_key.verifying_key().to_bytes(),
            expected_pubkey,
            "signing_key must be passed through verbatim"
        );
        assert_eq!(params.n_connections, 1);
        assert_eq!(params.features, 0);
    }

    #[test]
    fn assemble_propagates_selector_error_when_no_match() {
        // If the query matches no relay, the upstream selector error
        // must propagate via `AssembleError::Selector` (no remap that
        // would mask the cause).
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
        // Same retry_attempt + same selector + same signing_key ->
        // same assembled parameters. Critical for state-machine
        // idempotence (post-crash resume).
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

        assert_eq!(params_a.exit_addr.id, params_b.exit_addr.id);
        assert_eq!(
            params_a.signing_key.verifying_key().to_bytes(),
            params_b.signing_key.verifying_key().to_bytes()
        );
    }
}
