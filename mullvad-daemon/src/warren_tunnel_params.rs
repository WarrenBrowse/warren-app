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

use std::net::SocketAddr;

use ed25519_dalek::SigningKey;
use mullvad_types::settings::WarrenCustomExitSettings;
use talpid_warren_tunnel::{BypassCidr, MultiHopConfig, NatPmpConfig, WarrenTunnelParameters};
use warren_discovery_core::warren_types::{ExitId, WarrenExitAddr, WarrenPubkey};
use warren_discovery_core::{SelectorError, WarrenRelayQuery};

use crate::warren_relay_selector::DaemonWarrenRelaySelector;

/// Errors raised while assembling Warren tunnel parameters.
#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    /// No Warren relay matched the query (or the list was empty).
    #[error("warren relay selection failed: {0}")]
    Selector(#[from] SelectorError),

    /// The persisted/advanced custom-exit override is malformed (bad
    /// endpoint or pubkey). Surfaced instead of silently falling back to
    /// the roster so the user learns their override is wrong rather than
    /// connecting somewhere they did not intend.
    #[error("invalid custom exit: {0}")]
    CustomExit(String),
}

/// Number of parallel QUIC connections. `8` captures ~95% of the
/// multi-conn benefit (the throughput curve plateaus around N=8, cf.
/// warren-core `bench/scripts/m3e-multi-conn-sweep.sh` and the
/// standalone warren-client default). Mono-conn (`1`) caps the whole
/// tunnel on a single exit-side pump task (~350 Mbps measured against
/// a cpx32 exit on 2026-06-10).
const DEFAULT_N_CONNECTIONS: u8 = 8;

/// Valid range for the parallel-connection count. Above 16 the m3e
/// sweep shows no measurable gain (N=32 adds ~5% over N=8) while the
/// exit pays one pump task per connection.
pub const N_CONNECTIONS_RANGE: std::ops::RangeInclusive<u8> = 1..=16;

/// Env var overriding the persisted `Settings::warren_n_connections`
/// (ops/debug surface, mirrors `WARREN_API_URL` precedence).
pub const N_CONNECTIONS_ENV: &str = "WARREN_N_CONNECTIONS";

/// Resolves the effective parallel-connection count:
/// env var > persisted setting > compiled default.
///
/// Invalid or out-of-range values are ignored with a warning rather
/// than clamped, so a typo cannot silently change the wire profile.
pub fn resolve_n_connections(setting: Option<u8>) -> u8 {
    resolve_n_connections_from(std::env::var(N_CONNECTIONS_ENV).ok().as_deref(), setting)
}

fn resolve_n_connections_from(env: Option<&str>, setting: Option<u8>) -> u8 {
    if let Some(raw) = env {
        match raw.trim().parse::<u8>() {
            Ok(n) if N_CONNECTIONS_RANGE.contains(&n) => {
                log::warn!("{N_CONNECTIONS_ENV}={n} overrides the persisted n_connections setting");
                return n;
            }
            _ => log::warn!(
                "ignoring invalid {N_CONNECTIONS_ENV}={raw:?} (expected integer in {:?})",
                N_CONNECTIONS_RANGE
            ),
        }
    }
    match setting {
        Some(n) if N_CONNECTIONS_RANGE.contains(&n) => n,
        Some(n) => {
            log::warn!(
                "ignoring out-of-range persisted n_connections={n}, \
                 using default {DEFAULT_N_CONNECTIONS}"
            );
            DEFAULT_N_CONNECTIONS
        }
        None => DEFAULT_N_CONNECTIONS,
    }
}

/// Assemble-time `features` baseline (`0` = IPv4-only). The real
/// user-driven bitmask is computed post-assemble by the caller via
/// [`features_for`] (mirrors how `on_reconnect` / `enable_daita` are
/// wired after assembly), so the assembler stays oblivious to live
/// settings.
const DEFAULT_FEATURES: u32 = 0;

/// Computes the `features` bitmask to advertise in the `Setup` frame
/// from the live user settings.
///
/// Today the only user-driven feature is IPv6 dual-stack: the `IPV6`
/// bit is set iff the user enabled IPv6. Both single-hop and multi-hop
/// now carry v6 - multi-hop negotiates it over the control `/v2`
/// (`IpRequestV2`/`IpAssignV2`, cf. docs/31). When the exit cannot serve
/// v6 it answers v4-only and the firewall keeps native IPv6 blocked
/// (no leak).
#[must_use]
pub fn features_for(enable_ipv6: bool) -> u32 {
    let mut features = 0;
    if enable_ipv6 {
        features |= talpid_warren_tunnel::features::IPV6;
    }
    features
}

/// Whether to negotiate in-tunnel IPv6 for this tunnel: only when the
/// user enabled IPv6 AND the selected exit attests working IPv6
/// egress.
///
/// Negotiating v6 against an exit that cannot egress it blackholes
/// every client v6 packet - the client routes `::/0` into the tunnel
/// and nothing returns (observed on a v4-only exit, 2026-06-12). When
/// the exit lacks egress we connect v4-only and the firewall keeps
/// native IPv6 blocked (no leak, no blackhole). The egress capability
/// is derived per-node from the signed list (`WarrenRelay::egress_v6`,
/// from the exit's probed per-endpoint egress flags).
#[must_use]
pub fn negotiate_in_tunnel_ipv6(enable_ipv6: bool, exit_attests_ipv6_egress: bool) -> bool {
    enable_ipv6 && exit_attests_ipv6_egress
}

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
    let alpn_protocols = selection
        .alpns
        .iter()
        .map(|a| a.clone().into_bytes())
        .collect();
    Ok(WarrenTunnelParameters {
        exit_addr: selection.endpoint_addr,
        exit_id: selection.exit_id,
        country_code: selection.country_code,
        city: selection.city,
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
        alpn_protocols,
        multi_hop,
        // The reconnect observer is wired by the caller
        // (`ParametersGenerator::produce_warren_tunnel_params`) after
        // assembly so the relay-selection logic stays decoupled from
        // the daemon-side status cache.
        on_reconnect: None,
        // ADR 36: set by `produce_warren_tunnel_params` post-assembly (it owns
        // the generator handle the callback needs); `None` here is overwritten.
        on_exit_draining: None,
        on_egress_verdict: None,
        // ADR 36 (Option A): set by `produce_warren_tunnel_params` for
        // multi-hop (it owns the generator the register callback stores into).
        warren_register_migrate_handle: None,
        // ADR 36 gap-free drain path: set by `produce_warren_tunnel_params`
        // (it owns the generator handle the hook needs).
        warren_drain_migrate: None,
        // docs/59 Lot 3: pre-swap NAT-PMP reservation + post-swap re-map,
        // wired later from the live port-forward config; None keeps the
        // unconditional-swap behaviour.
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        // NAT-PMP config originates from user settings (UI). The
        // caller threads it as-is; `None` keeps the legacy behaviour
        // (no refresh loop spawned).
        nat_pmp,
        // The NAT-PMP observer is wired by the caller
        // (`ParametersGenerator::produce_warren_tunnel_params`) after
        // assembly so the relay-selection logic stays decoupled from
        // the daemon-side status cache.
        nat_pmp_observer: None,
        // Live-reconfig channel: same logic - wired by the caller
        // after assembly so the assembler stays oblivious to runtime
        // signalling concerns. `None` here keeps the legacy
        // "params at start" behaviour for callers that don't opt
        // into live reconfig (tests, future non-daemon embedders).
        nat_pmp_control_rx: None,
        // User-supplied bypass CIDRs (--bypass-cidr). The
        // daemon-side runtime in `talpid-warren-tunnel` consumes this
        // list to install extra `ip rule add to <cidr> lookup main`
        // rules alongside the standard split-default routes. Empty
        // (default) preserves the prior behaviour.
        bypass_cidrs,
        // DAITA v2 opt-in. assemble() always starts at `false`;
        // the caller (`ParametersGenerator::produce_warren_tunnel_params`)
        // mutates this field post-assemble based on the live
        // `wireguard.daita.enabled` Mullvad setting. Mirrors the
        // post-assemble wiring of `on_reconnect` and the NAT-PMP
        // observer.
        enable_daita: false,
    })
}

/// Auto-failover variant: when the previous tunnel attempt is
/// known to have used `excluded_pubkey` and failed, pick an
/// alternative relay via
/// [`warren_discovery_core::WarrenRelaySelector::select_failover_alternative`]
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
    let alpn_protocols = alternative
        .dial_alpns()
        .into_iter()
        .map(String::into_bytes)
        .collect();
    Ok(WarrenTunnelParameters {
        exit_addr: alternative.endpoint_addr().clone(),
        exit_id: alternative.exit_id(),
        country_code: alternative.location().country_code().to_owned(),
        city: alternative.location().city().to_owned(),
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
        alpn_protocols,
        multi_hop,
        on_reconnect: None,
        // ADR 36: set by `produce_warren_tunnel_params` post-assembly.
        on_exit_draining: None,
        on_egress_verdict: None,
        // ADR 36 (Option A): set by `produce_warren_tunnel_params` for
        // multi-hop (it owns the generator the register callback stores into).
        warren_register_migrate_handle: None,
        // ADR 36 gap-free drain path: set by `produce_warren_tunnel_params`
        // (it owns the generator handle the hook needs).
        warren_drain_migrate: None,
        // docs/59 Lot 3: pre-swap NAT-PMP reservation + post-swap re-map,
        // wired later from the live port-forward config; None keeps the
        // unconditional-swap behaviour.
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        nat_pmp,
        nat_pmp_observer: None,
        nat_pmp_control_rx: None,
        bypass_cidrs,
        enable_daita: false,
    })
}

/// Assembles [`WarrenTunnelParameters`] for the advanced "custom exit"
/// override, bypassing the relay selector entirely.
///
/// The user supplied an `(endpoint, pubkey, cover_domain?)` out of band
/// (e.g. a self-hosted `warrenguard serve` node). We dial it directly:
/// no roster lookup, no location query, no failover pool, and always
/// single-hop (`multi_hop = None`) - a hand-entered exit has no relay to
/// pair with. The TOFU pin is skipped by the caller because typing the
/// key *is* the pin.
///
/// The `exit_id` is synthesised from the first 16 bytes of the pubkey:
/// custom exits are not in the signed roster, so there is no operator
/// `exit_id`, but the downstream params field is non-optional and the
/// pin path is disabled for this branch anyway. `country_code` is empty
/// and `city` carries the user's cosmetic label, if any.
///
/// # Errors
///
/// Returns [`AssembleError::CustomExit`] if the endpoint does not parse
/// as a `host:port` socket address or the pubkey is not 64 hex chars.
pub fn assemble_custom(
    custom: &WarrenCustomExitSettings,
    signing_key: SigningKey,
    nat_pmp: Option<NatPmpConfig>,
    bypass_cidrs: Vec<BypassCidr>,
) -> Result<WarrenTunnelParameters, AssembleError> {
    let endpoint: SocketAddr = custom.endpoint.trim().parse().map_err(|_| {
        AssembleError::CustomExit(format!(
            "endpoint {:?} is not a valid host:port (bracket IPv6, e.g. [2001:db8::1]:443)",
            custom.endpoint
        ))
    })?;
    let pubkey = WarrenPubkey::from_hex(custom.pubkey_hex.trim())
        .map_err(|e| AssembleError::CustomExit(format!("pubkey: {e}")))?;

    // Synthetic stable id from the pubkey prefix (see fn doc).
    let mut id_bytes = [0u8; 16];
    id_bytes.copy_from_slice(&pubkey.as_bytes()[..16]);
    let exit_id = ExitId::from_bytes(id_bytes);

    let mut exit_addr = WarrenExitAddr::from_ip_addrs(pubkey, [endpoint]);
    if let Some(domain) = custom
        .cover_domain
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        exit_addr = exit_addr.with_cover_domain(domain);
    }

    Ok(WarrenTunnelParameters {
        exit_addr,
        exit_id,
        country_code: String::new(),
        city: custom.label.clone(),
        signing_key,
        n_connections: DEFAULT_N_CONNECTIONS,
        features: DEFAULT_FEATURES,
        // Empty -> client offers the `ALPN_H3` default inside
        // `ClientTunnel`, which is exactly what a plain `warrenguard
        // serve` exit speaks. The roster-driven path threads exit-attested
        // ALPNs here; a hand-entered exit advertises none.
        alpn_protocols: Vec::new(),
        // A custom exit is a single hand-entered node: no relay to pair
        // with, so multi-hop is structurally impossible here.
        multi_hop: None,
        on_reconnect: None,
        on_exit_draining: None,
        on_egress_verdict: None,
        warren_register_migrate_handle: None,
        // ADR 36 gap-free drain path: set by `produce_warren_tunnel_params`
        // (it owns the generator handle the hook needs).
        warren_drain_migrate: None,
        // docs/59 Lot 3: pre-swap NAT-PMP reservation + post-swap re-map,
        // wired later from the live port-forward config; None keeps the
        // unconditional-swap behaviour.
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        nat_pmp,
        nat_pmp_observer: None,
        nat_pmp_control_rx: None,
        bypass_cidrs,
        enable_daita: false,
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::warren_types::WarrenPubkey;
    use warren_discovery_core::{
        Addr, Ingress, Listener, Location, LocationConstraint, WarrenRelay, WarrenRelayList,
    };

    use super::*;

    fn fixture_signing_key() -> SigningKey {
        // Deterministic seed for the tests.
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn fixture_relay(seed: u8, country: &str) -> WarrenRelay {
        WarrenRelay::from_public(
            WarrenPubkey::from_bytes([seed; 32]),
            warren_discovery_core::warren_types::ExitId::from_bytes([seed; 16]),
            Location::new(country, "_"),
            100,
            true,
            vec![Ingress::new(
                Addr::new("198.51.100.1".parse().unwrap(), None),
                vec![Listener::new(51820, "quic", "h3")],
            )],
            true,
            false,
        )
    }

    #[test]
    fn resolve_n_connections_defaults_when_unset() {
        // No env, no setting -> compiled default. The baseline path
        // every fresh install takes.
        assert_eq!(resolve_n_connections_from(None, None), 8);
    }

    #[test]
    fn resolve_n_connections_uses_persisted_setting() {
        assert_eq!(resolve_n_connections_from(None, Some(4)), 4);
        assert_eq!(resolve_n_connections_from(None, Some(1)), 1);
        assert_eq!(resolve_n_connections_from(None, Some(16)), 16);
    }

    #[test]
    fn resolve_n_connections_rejects_out_of_range_setting() {
        // Out-of-range persisted values (hand-edited settings.json)
        // must fall back to the default, never clamp: clamping would
        // silently bind the user to a value they never chose.
        assert_eq!(resolve_n_connections_from(None, Some(0)), 8);
        assert_eq!(resolve_n_connections_from(None, Some(17)), 8);
        assert_eq!(resolve_n_connections_from(None, Some(255)), 8);
    }

    #[test]
    fn resolve_n_connections_env_overrides_setting() {
        assert_eq!(resolve_n_connections_from(Some("2"), Some(8)), 2);
        assert_eq!(resolve_n_connections_from(Some(" 12 "), None), 12);
    }

    #[test]
    fn resolve_n_connections_invalid_env_falls_back_to_setting() {
        // A broken env value must not take the default over a valid
        // persisted setting: the user's choice outranks a typo.
        assert_eq!(resolve_n_connections_from(Some("abc"), Some(4)), 4);
        assert_eq!(resolve_n_connections_from(Some("0"), Some(4)), 4);
        assert_eq!(resolve_n_connections_from(Some("64"), Some(4)), 4);
        assert_eq!(resolve_n_connections_from(Some(""), None), 8);
    }

    #[test]
    fn in_tunnel_ipv6_negotiated_only_when_exit_attests_egress() {
        // Blackhole guard (incident, 2026-06-12): a v4-only exit
        // that gets the IPV6 feature allocates an in-tunnel v6 the
        // client routes ::/0 into, but the exit cannot egress it, so
        // every v6 packet is dropped with no error. The client must
        // negotiate v6 ONLY when the selected exit attests egress.
        assert!(
            negotiate_in_tunnel_ipv6(true, true),
            "user wants v6 + exit egresses v6 => negotiate"
        );
        assert!(
            !negotiate_in_tunnel_ipv6(true, false),
            "user wants v6 but exit cannot egress => v4-only (no blackhole)"
        );
        assert!(
            !negotiate_in_tunnel_ipv6(false, true),
            "user disabled v6 => never negotiate, even if the exit could"
        );
    }

    #[test]
    fn features_sets_ipv6_bit_when_enabled() {
        // enable_ipv6 ON => advertise IPV6 so the exit allocates a tunnel
        // v6 and the client goes dual-stack. Now true for both single-hop
        // and multi-hop (the latter negotiates v6 over the control /v2).
        assert_eq!(
            features_for(true),
            talpid_warren_tunnel::features::IPV6,
            "IPv6 enabled must advertise the IPV6 feature"
        );
    }

    #[test]
    fn features_omits_ipv6_when_disabled() {
        // enable_ipv6 OFF => baseline 0 => exit allocates no v6 => the
        // firewall blocks native IPv6 (no leak, no v6 connectivity).
        assert_eq!(
            features_for(false),
            0,
            "IPv6 disabled must advertise no feature"
        );
    }

    #[test]
    fn assemble_combines_selection_signing_key_and_constants() {
        // The function must produce a `WarrenTunnelParameters` whose
        // `exit_addr` comes from the selector, whose `signing_key` is
        // passed through verbatim, and where the two constants
        // `n_connections == 8` + `features == 0` are set.
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
        assert_eq!(params.n_connections, 8);
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
                cover_domain: None,
            },
            exit: MultiHopExitDescriptor {
                exit_id: ExitId::from_bytes([0xb1; 16]),
                exit_ed25519_pubkey: [0xb2; 32],
                exit_x25519_multihop_pubkey: [0xb3; 32],
                endpoint: Some("198.51.100.20:443".parse().unwrap()),
                signature: [0xb4; 64],
                dns_disabled: false,
                cover_domain: None,
            },
            operational_pubkey: SigningKey::from_bytes(&[0xc1; 32]).verifying_key(),
            exit_country: "fr".to_owned(),
            exit_city: "Paris".to_owned(),
            enable_gso: false,
            use_warren_obfuscation: true,
            single_node: false,
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
            rules: Vec::new(),
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
            sticky_suggestion: false,
            remap_epoch: 0,
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
        // dual-NIC hosts.
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
        // empty. This is the anti-regression: zero-bypass
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

    fn fixture_custom(endpoint: &str, pubkey_hex: &str) -> WarrenCustomExitSettings {
        WarrenCustomExitSettings {
            enabled: true,
            endpoint: endpoint.to_owned(),
            pubkey_hex: pubkey_hex.to_owned(),
            cover_domain: None,
            label: String::new(),
        }
    }

    #[test]
    fn assemble_custom_dials_the_user_supplied_exit() {
        // A valid custom exit must produce params pointed at exactly the
        // entered endpoint + pubkey, single-hop, with no ALPN override
        // (so the client speaks the ALPN_H3 default a plain `warrenguard
        // serve` exit understands).
        let pubkey_hex = "ab".repeat(32);
        let custom = fixture_custom("198.51.100.7:443", &pubkey_hex);
        let params =
            assemble_custom(&custom, fixture_signing_key(), None, Vec::new()).expect("valid");

        assert_eq!(
            params.exit_addr.id,
            WarrenPubkey::from_hex(&pubkey_hex).unwrap()
        );
        assert_eq!(
            params.exit_addr.ip_addrs().collect::<Vec<_>>(),
            vec!["198.51.100.7:443".parse().unwrap()]
        );
        assert!(
            params.multi_hop.is_none(),
            "custom exit is always single-hop"
        );
        assert!(
            params.alpn_protocols.is_empty(),
            "no exit-attested ALPN override"
        );
        assert!(
            params.exit_addr.cover_domain.is_none(),
            "RPK mode when no cover domain"
        );
    }

    #[test]
    fn assemble_custom_applies_cover_domain_for_x509_mode() {
        // A non-empty cover domain selects v6 X.509 mode: it must land on
        // the exit_addr so `resolve_cover_domain` downstream dials it as
        // SNI instead of pinning the raw pubkey.
        let custom = WarrenCustomExitSettings {
            cover_domain: Some("cdn.example.com".to_owned()),
            ..fixture_custom("198.51.100.7:443", &"cd".repeat(32))
        };
        let params =
            assemble_custom(&custom, fixture_signing_key(), None, Vec::new()).expect("valid");
        assert_eq!(
            params.exit_addr.cover_domain.as_deref(),
            Some("cdn.example.com")
        );
    }

    #[test]
    fn assemble_custom_rejects_unparseable_endpoint() {
        // A bad endpoint must error loudly, never silently fall back to
        // the roster (which would connect the user somewhere they did not
        // choose).
        let custom = fixture_custom("not-a-socket-addr", &"ab".repeat(32));
        let err = assemble_custom(&custom, fixture_signing_key(), None, Vec::new()).unwrap_err();
        assert!(matches!(err, AssembleError::CustomExit(_)), "got {err:?}");
    }

    #[test]
    fn assemble_custom_rejects_malformed_pubkey() {
        // Wrong-length / non-hex pubkey is a hard error.
        let custom = fixture_custom("198.51.100.7:443", "deadbeef");
        let err = assemble_custom(&custom, fixture_signing_key(), None, Vec::new()).unwrap_err();
        assert!(matches!(err, AssembleError::CustomExit(_)), "got {err:?}");
    }
}
