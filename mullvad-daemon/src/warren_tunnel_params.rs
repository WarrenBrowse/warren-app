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
use talpid_warren_tunnel::{BypassCidr, MultiHopConfig, NatPmpConfig, WarrenTunnelParameters};
use warren_relay_selector::warren_types::WarrenPubkey;
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
/// `multi_hop` is the optional output of
/// [`crate::warren_multi_hop::load_from_settings_dir`]: when `Some` the
/// dispatcher in `talpid-warren-tunnel` spins up a multi-hop session;
/// when `None` it stays on the legacy single-hop path. The exit
/// selection still runs in both cases so the firewall and routing
/// layers have a consistent `exit_addr` shape, even though the
/// multi-hop dispatcher derives the actual exit identity from
/// [`MultiHopConfig::exit`].
///
/// # Errors
///
/// Returns [`AssembleError::Selector`] if no relay matches the query.
pub fn assemble_for_attempt(
    selector: &DaemonWarrenRelaySelector,
    signing_key: SigningKey,
    query: &WarrenRelayQuery,
    retry_attempt: u32,
    multi_hop: Option<MultiHopConfig>,
    nat_pmp: Option<NatPmpConfig>,
    bypass_cidrs: Vec<BypassCidr>,
) -> Result<WarrenTunnelParameters, AssembleError> {
    let selection = selector.select_for_attempt(query, retry_attempt)?;
    Ok(WarrenTunnelParameters {
        exit_addr: selection.endpoint_addr,
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
        multi_hop,
        // The reconnect observer is wired by the caller
        // (`ParametersGenerator::produce_warren_tunnel_params`) after
        // assembly so the relay-selection logic stays decoupled from
        // the daemon-side status cache.
        on_reconnect: None,
        // NAT-PMP config originates from user settings (M4.H.F UI). The
        // caller threads it as-is; `None` keeps the legacy behaviour
        // (no refresh loop spawned).
        nat_pmp,
        // The NAT-PMP observer is wired by the caller
        // (`ParametersGenerator::produce_warren_tunnel_params`) after
        // assembly so the relay-selection logic stays decoupled from
        // the daemon-side status cache.
        nat_pmp_observer: None,
        // User-supplied bypass CIDRs (M4.H.G --bypass-cidr). The
        // daemon-side runtime in `talpid-warren-tunnel` consumes this
        // list to install extra `ip rule add to <cidr> lookup main`
        // rules alongside the standard split-default routes. Empty
        // (default) preserves the M4.E.D behaviour.
        bypass_cidrs,
        // M5.B.1 DAITA v2 opt-in. assemble() always starts at `false`;
        // the caller (`ParametersGenerator::produce_warren_tunnel_params`)
        // mutates this field post-assemble based on the live
        // `wireguard.daita.enabled` Mullvad setting. Mirrors the
        // post-assemble wiring of `on_reconnect` and the NAT-PMP
        // observer.
        enable_daita: false,
    })
}

/// M5.B.2 auto-failover variant: when the previous tunnel attempt is
/// known to have used `excluded_pubkey` and failed, pick an
/// alternative relay via
/// [`warren_relay_selector::WarrenRelaySelector::select_failover_alternative`]
/// (same-country preference, global fallback). The rest of the
/// returned [`WarrenTunnelParameters`] is identical to
/// [`assemble_for_attempt`] - the failover only affects relay
/// selection, never the signing key, features, multi-hop config or
/// the other fields.
///
/// `retry_attempt` seeds the internal RNG so successive failures of
/// the same exit explore the alternative pool deterministically (a
/// reconnect storm on a dead pop will land on a different fallback
/// every time without re-trying the broken one).
///
/// # Errors
///
/// Returns [`AssembleError::Selector`] when no alternative relay
/// exists (single-relay deployment or every other relay filtered out
/// by the query).
#[expect(
    clippy::too_many_arguments,
    reason = "Mirror of assemble_for_attempt with one extra discriminator (excluded_pubkey); splitting into a struct would obscure the call site in tunnel.rs and decouple it visually from the sibling function."
)]
pub fn assemble_failover_for_attempt(
    selector: &DaemonWarrenRelaySelector,
    signing_key: SigningKey,
    query: &WarrenRelayQuery,
    retry_attempt: u32,
    excluded_pubkey: WarrenPubkey,
    multi_hop: Option<MultiHopConfig>,
    nat_pmp: Option<NatPmpConfig>,
    bypass_cidrs: Vec<BypassCidr>,
) -> Result<WarrenTunnelParameters, AssembleError> {
    let excluded = selector
        .relay_by_pubkey(&excluded_pubkey)
        .ok_or(SelectorError::NoRelayMatch)?;
    let alternative =
        selector
            .inner()
            .select_failover_alternative_for_attempt(query, excluded, retry_attempt)?;
    Ok(WarrenTunnelParameters {
        exit_addr: alternative.endpoint_addr().clone(),
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
        multi_hop,
        on_reconnect: None,
        nat_pmp,
        nat_pmp_observer: None,
        bypass_cidrs,
        enable_daita: false,
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

        let params = assemble_for_attempt(
            &selector,
            key,
            &WarrenRelayQuery::any(),
            0,
            None,
            None,
            Vec::new(),
        )
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
        assert!(
            params.multi_hop.is_none(),
            "assemble with multi_hop=None must yield single-hop params"
        );
        assert!(
            params.on_reconnect.is_none(),
            "assemble must leave on_reconnect at None so the caller \
             (ParametersGenerator::produce_warren_tunnel_params) is the \
             single place that wires the WarrenStatusCache observer"
        );
        assert!(
            params.nat_pmp.is_none(),
            "assemble with nat_pmp=None must yield params with no NAT-PMP \
             refresh loop wired (legacy behaviour preserved)"
        );
    }

    #[test]
    fn assemble_propagates_selector_error_when_no_match() {
        // If the query matches no relay, the upstream selector error
        // must propagate via `AssembleError::Selector` (no remap that
        // would mask the cause).
        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let query = WarrenRelayQuery::any().with_location(LocationConstraint::Country("zz".into()));

        let err = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &query,
            0,
            None,
            None,
            Vec::new(),
        )
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
            None,
            None,
            Vec::new(),
        )
        .unwrap();
        let params_b = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            42,
            None,
            None,
            Vec::new(),
        )
        .unwrap();

        assert_eq!(params_a.exit_addr.id, params_b.exit_addr.id);
        assert_eq!(
            params_a.signing_key.verifying_key().to_bytes(),
            params_b.signing_key.verifying_key().to_bytes()
        );
    }

    #[test]
    fn assemble_with_multi_hop_some_wires_into_params() {
        // When the daemon passes a `Some(MultiHopConfig)`, it must be
        // forwarded verbatim onto `params.multi_hop`. The dispatcher
        // downstream (talpid-warren-tunnel::start) reads that field to
        // decide single-hop vs multi-hop. A regression that dropped
        // the field would silently downgrade every multi-hop user to
        // single-hop without any error surfacing.
        use talpid_warren_tunnel::{
            ExitId, MultiHopConfig, MultiHopExitDescriptor, MultiHopRelayDescriptor,
        };

        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let mh = MultiHopConfig {
            relay: MultiHopRelayDescriptor {
                relay_id: [0xa1; 16],
                relay_ed25519_pubkey: [0xa2; 32],
                endpoint: "198.51.100.10:443".parse().unwrap(),
                signature: [0xa3; 64],
            },
            exit: MultiHopExitDescriptor {
                exit_id: ExitId([0xb1; 16]),
                exit_ed25519_pubkey: [0xb2; 32],
                exit_x25519_multihop_pubkey: [0xb3; 32],
                endpoint: "198.51.100.20:443".parse().unwrap(),
                signature: [0xb4; 64],
            },
            operational_pubkey: SigningKey::from_bytes(&[0xc1; 32]).verifying_key(),
            enable_gso: false,
            use_warren_obfuscation: true,
        };

        let params = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            0,
            Some(mh.clone()),
            None,
            Vec::new(),
        )
        .expect("must assemble multi-hop params");

        let wired = params
            .multi_hop
            .expect("multi_hop must be forwarded onto params");
        assert_eq!(
            wired.relay.endpoint, mh.relay.endpoint,
            "relay endpoint must round-trip"
        );
        assert_eq!(
            wired.exit.endpoint, mh.exit.endpoint,
            "exit endpoint must round-trip"
        );
        assert_eq!(
            wired.operational_pubkey.to_bytes(),
            mh.operational_pubkey.to_bytes(),
            "operational pubkey must round-trip"
        );
        assert_eq!(wired.enable_gso, mh.enable_gso);
        assert_eq!(wired.use_warren_obfuscation, mh.use_warren_obfuscation);
    }

    #[test]
    fn assemble_with_nat_pmp_some_wires_into_params() {
        // When the daemon passes a `Some(NatPmpConfig)`, it must be
        // forwarded verbatim onto `params.nat_pmp`. The daemon-side
        // `NatPmpManager` reads that field to decide whether to spawn
        // a refresh loop once the tunnel is up. A regression dropping
        // the field would silently disable port-forwarding for every
        // user without any error surfacing (= product differentiator
        // missed).
        use talpid_warren_tunnel::{NatPmpConfig, NatPmpProto};

        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
        };

        let params = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            0,
            None,
            Some(cfg.clone()),
            Vec::new(),
        )
        .expect("must assemble nat-pmp params");

        let wired = params
            .nat_pmp
            .expect("nat_pmp must be forwarded onto params");
        assert_eq!(wired, cfg, "NatPmpConfig must round-trip verbatim");
    }

    #[test]
    fn assemble_propagates_bypass_cidrs_into_params() {
        // The daemon-side settings store carries a Vec<BypassCidr>
        // populated by either a future UI flow or the CLI. assemble()
        // must forward that list verbatim onto
        // `params.bypass_cidrs` so the talpid-warren-tunnel routing
        // installer sees the user's intent on the next tunnel start.
        // A regression that drops the field would silently route LAN
        // traffic through the tunnel and break inbound SSH on
        // dual-NIC hosts (the exact M4.H.G fix).
        use std::net::Ipv4Addr;
        use talpid_warren_tunnel::BypassCidr;

        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let cidrs = vec![
            BypassCidr {
                network: Ipv4Addr::new(192, 168, 0, 0),
                prefix: 16,
            },
            BypassCidr {
                network: Ipv4Addr::new(10, 0, 0, 0),
                prefix: 8,
            },
        ];

        let params = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            0,
            None,
            None,
            cidrs.clone(),
        )
        .expect("must assemble bypass-cidr params");

        assert_eq!(
            params.bypass_cidrs, cidrs,
            "bypass_cidrs must round-trip verbatim from assemble() into params",
        );
    }

    #[test]
    fn assemble_default_empty_bypass_cidrs_yields_empty_params_field() {
        // Caller passes an empty Vec (the daemon default before any
        // user supplies a bypass): params.bypass_cidrs must also be
        // empty. This is the M4.E.D anti-regression: zero-bypass
        // callers must produce the historic "no extra rules" routing.
        let list = WarrenRelayList::new(vec![fixture_relay(1, "se")]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let params = assemble_for_attempt(
            &selector,
            fixture_signing_key(),
            &WarrenRelayQuery::any(),
            0,
            None,
            None,
            Vec::new(),
        )
        .expect("must assemble params");

        assert!(
            params.bypass_cidrs.is_empty(),
            "an empty bypass list must produce empty params.bypass_cidrs, got {:?}",
            params.bypass_cidrs,
        );
    }

    #[test]
    fn nat_pmp_config_default_disabled_matches_off_invariant() {
        // `NatPmpConfig::default_disabled()` must produce a config that
        // the daemon treats equivalently to `None`. The daemon checks
        // `enabled == false` to short-circuit, so the field must be
        // false; the other fields' values don't matter but are stable
        // for diffing purposes.
        use talpid_warren_tunnel::{NatPmpConfig, NatPmpProto};

        let cfg = NatPmpConfig::default_disabled();
        assert!(!cfg.enabled, "default_disabled() must yield enabled=false");
        assert_eq!(cfg.lifetime_secs, NatPmpConfig::DEFAULT_LIFETIME_SECS);
        assert_eq!(cfg.protocol, NatPmpProto::Udp);
        assert_eq!(cfg.suggested_external_port, 0);
        assert_eq!(cfg.internal_port, 0);
    }

    #[test]
    fn nat_pmp_config_default_enabled_uses_one_hour_lifetime() {
        // The UI's first-time toggle creates `default_enabled()`. The
        // lifetime must default to the longest interval the exit-side
        // allocator grants (1h = 3600s) so renewals happen at most
        // every 30 min - keeping the no-log control plane quiet.
        use talpid_warren_tunnel::NatPmpConfig;

        let cfg = NatPmpConfig::default_enabled();
        assert!(cfg.enabled);
        assert_eq!(cfg.lifetime_secs, 3600);
    }
}
