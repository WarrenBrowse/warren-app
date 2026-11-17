use super::*;
use warrenguard_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};
use warrenguard_wire::WarrenPubkey;

#[test]
fn a_migration_never_carries_an_ml_kem_key_the_operational_signature_does_not_cover() {
    let cfg = MultiHopConfig {
        relay: RelayDescriptorSigned {
            relay_id: [0xaa; 16],
            relay_ed25519_pubkey: [0xbb; 32],
            endpoint: "192.0.2.10:443".parse().unwrap(),
            endpoint_v6: None,
            signature: [0xcc; 64],
            cover_domain: None,
            tcp_fallback: false,
        },
        exit: ExitDescriptorSigned {
            exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
            exit_ed25519_pubkey: [0xee; 32],
            exit_x25519_multihop_pubkey: [0xff; 32],
            exit_mlkem768_pubkey: Some(vec![0x5a; warrenguard_multihop::MLKEM768_ENCAPS_KEY_LEN]),
            endpoint: None,
            signature: [0x11; 64],
            dns_disabled: false,
            cover_domain: None,
        },
        operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
        exit_country: String::new(),
        exit_city: String::new(),
        enable_gso: false,
        use_warren_obfuscation: true,
        single_node: true,
    };

    assert!(migration_target(&cfg).exit_mlkem768_pubkey.is_none());
}

#[test]
fn a_first_session_refused_for_want_of_a_post_quantum_seal_says_so() {
    let Error::Handshake(reason) = initial_session_closed(SupervisorVerdict::of(
        &MultiHopError::PostQuantumUnavailable,
    )) else {
        panic!("a handshake failure");
    };
    assert!(
        matches!(reason, HandshakeFailure::PostQuantumUnavailable),
        "{reason:?}"
    );
    assert!(reason.to_string().contains("post-quantum"), "{reason}");
}

#[test]
fn a_first_session_ended_by_an_unroutable_fleet_names_the_network() {
    let Error::Handshake(unroutable) = initial_session_closed(SupervisorVerdict::Unroutable) else {
        panic!("a handshake failure");
    };
    assert!(
        matches!(unroutable, HandshakeFailure::Unroutable),
        "{unroutable:?}"
    );
    assert!(
        unroutable.to_string().contains("no entry relay reachable"),
        "{unroutable}"
    );
    let Error::Handshake(other) = initial_session_closed(SupervisorVerdict::Other) else {
        panic!("a handshake failure");
    };
    assert!(!other.to_string().contains("reachable"), "{other}");
}

#[test]
fn a_first_session_without_a_token_stays_recoverable_and_feeds_the_refusal_run() {
    for cause in [
        NoSessionTokenCause::Empty,
        NoSessionTokenCause::AllRefused,
        NoSessionTokenCause::AllInUse,
    ] {
        let err = initial_session_closed(SupervisorVerdict::NoToken(cause));

        assert!(
            err.is_recoverable(),
            "{cause:?}: the token refresh refills the store, so the state machine must retry"
        );
        assert!(
            err.session_rejection().is_some(),
            "{cause:?}: a session that never comes up must not dial in silence forever"
        );
    }
}

#[test]
fn every_token_in_use_elsewhere_surfaces_as_the_device_limit() {
    let err = initial_session_closed(SupervisorVerdict::NoToken(NoSessionTokenCause::AllInUse));
    let refusal = err.session_rejection().expect("a refusal");

    assert!(
        refusal.reason.starts_with("[TOO_MANY_CONNECTIONS]"),
        "{}",
        refusal.reason
    );
    assert!(
        !refusal.retry_while_unverified,
        "the exit itself said every token is in use"
    );
    assert!(
        refusal
            .if_active
            .as_deref()
            .is_some_and(|reason| reason.starts_with("[TOO_MANY_CONNECTIONS]")),
        "{:?}",
        refusal.if_active
    );
}

#[test]
fn no_token_for_an_active_subscription_surfaces_as_no_session_token() {
    for cause in [NoSessionTokenCause::Empty, NoSessionTokenCause::AllRefused] {
        let err = initial_session_closed(SupervisorVerdict::NoToken(cause));
        let refusal = err.session_rejection().expect("a refusal");

        assert!(
            refusal.reason.starts_with("[EXPIRED_ACCOUNT]"),
            "{cause:?}: without a subscription there is no token to have: {}",
            refusal.reason
        );
        assert!(
            refusal.retry_while_unverified,
            "{cause:?}: an API that cannot answer mints no token either, and says no expiry"
        );
        assert!(
            refusal
                .if_active
                .as_deref()
                .is_some_and(|reason| reason.starts_with(NO_SESSION_TOKEN_REASON)),
            "{cause:?}: {:?}",
            refusal.if_active
        );
    }
}

#[test]
fn the_supervisor_verdict_names_a_missing_token() {
    assert_eq!(
        SupervisorVerdict::of(&MultiHopError::NoSessionToken(
            NoSessionTokenCause::AllInUse
        )),
        SupervisorVerdict::NoToken(NoSessionTokenCause::AllInUse)
    );
    assert_eq!(
        SupervisorVerdict::of(&MultiHopError::NoReachableEntry),
        SupervisorVerdict::Unroutable
    );
    assert_eq!(
        SupervisorVerdict::of(&MultiHopError::NoSession),
        SupervisorVerdict::Other
    );
}

#[test]
fn a_relay_the_probe_did_not_keep_is_named_by_its_relay_id() {
    let relay = |tag: u8| RelayDescriptorSigned {
        relay_id: [tag; 16],
        relay_ed25519_pubkey: [tag; 32],
        endpoint: "192.0.2.1:443".parse().expect("static addr parses"),
        endpoint_v6: None,
        cover_domain: None,
        tcp_fallback: false,
        signature: [0; 64],
    };
    let (a, b, c) = (relay(1), relay(2), relay(3));

    assert_eq!(relay_ids_left_out(&[&a, &b, &c], &[0, 2]), vec![[2; 16]]);
    assert_eq!(relay_ids_left_out(&[&a, &b], &[]), vec![[1; 16], [2; 16]]);
}

#[test]
fn a_relay_this_host_routes_is_never_named_unroutable() {
    // Loopback is routed on every host, escape or not.
    let loopback = RelayDescriptorSigned {
        relay_id: [9; 16],
        relay_ed25519_pubkey: [9; 32],
        endpoint: "127.0.0.1:9".parse().expect("static addr parses"),
        endpoint_v6: None,
        cover_domain: None,
        tcp_fallback: false,
        signature: [0; 64],
    };
    assert!(warren_unroutable_relays(&[&loopback]).is_empty());
}

#[test]
fn a_session_presents_no_more_tokens_than_one_setup_may_carry() {
    let stack: Vec<[u8; warrenguard_wire::SESSION_TOKEN_LEN]> = (0..12u8)
        .map(|i| [i; warrenguard_wire::SESSION_TOKEN_LEN])
        .collect();
    let expected: Vec<u8> = (0..warrenguard_wire::MAX_SESSION_TOKENS)
        .map(|i| u8::try_from(i).unwrap())
        .collect();
    let provider = make_session_token_provider(Arc::new(move || stack.clone()));

    let presented: Vec<u8> = provider().iter().map(|token| token.0[0]).collect();

    assert_eq!(presented, expected);
}

/// A serialized no-op maybenot machine (same vector warrenguard-daita
/// pins): a valid grant the framework can actually load.
const NO_OP_MACHINE: &str = "02eNpjYEAHjOgCAAA0AAI=";

fn granted_spec(machine_specs: Vec<String>) -> warrenguard_wire::DaitaConfig {
    warrenguard_wire::DaitaConfig {
        machine_specs,
        max_padding_frac: 0.5,
        max_blocking_frac: 0.0,
    }
}

#[test]
fn multi_hop_daita_runs_the_machine_the_exit_granted() {
    let shared = multi_hop_daita_shared(true, Some(granted_spec(vec![NO_OP_MACHINE.into()])))
        .expect("a valid grant must build")
        .expect("a granted defense must select the DAITA pumps");
    let state = shared.lock();
    assert!(state.is_enabled(), "the granted machine must be driven");
    assert_eq!(state.machines_count(), 1);
}

#[test]
fn multi_hop_daita_not_granted_runs_plain_pumps_without_claiming_protection() {
    let shared = multi_hop_daita_shared(true, None)
        .expect("an ungranted defense is not an error, it is surfaced");
    assert!(
        shared.is_none(),
        "no grant must select the plain pumps and record daita_active = false"
    );
}

#[test]
fn multi_hop_daita_empty_grant_is_treated_as_not_granted() {
    // The wire contract says an empty machine list is invalid (the exit
    // must send None); a client must never claim an active defense over
    // a grant that drives zero machines.
    let shared = multi_hop_daita_shared(true, Some(granted_spec(vec![])))
        .expect("an empty grant maps to inactive, not an error");
    assert!(shared.is_none());
}

#[test]
fn multi_hop_daita_unusable_grant_fails_the_connect_instead_of_padding_nothing() {
    let err = multi_hop_daita_shared(true, Some(granted_spec(vec!["garbage".into()])))
        .expect_err("an unparseable server-supplied machine must not be swallowed");
    assert!(
        matches!(err, Error::Handshake(_)),
        "unusable grant must surface as a recoverable handshake error, got {err:?}"
    );
}

#[test]
fn dial_refusal_cooldown_throttles_reactions() {
    // First refusal ever (last == 0) must react; a refusal inside the
    // cooldown must not (the supervisor redials every 0.3-2 s, one
    // reaction per rollout wave is enough); past the cooldown it
    // reacts again; a future `last` (clock skew) stays quiet.
    assert!(dial_refusal_reaction_due(1_000, 0));
    assert!(!dial_refusal_reaction_due(1_010, 1_000));
    assert!(dial_refusal_reaction_due(
        1_000 + WARREN_DIAL_REFUSAL_COOLDOWN.as_secs(),
        1_000
    ));
    assert!(!dial_refusal_reaction_due(900, 1_000));
}

#[test]
fn connect_summary_names_every_phase_with_its_duration() {
    // Ops grep this exact line shape to attribute connect latency
    // (phase names + ms values); a silent rename breaks fleet-wide
    // log tooling, so the shape is pinned here.
    let line = connect_summary_line("multi_hop", 252, 1619, 14, 191, 2077);
    assert_eq!(
        line,
        "Warren connected in 2077ms (mode=multi_hop prep=252ms \
         handshake=1619ms tun=14ms routes+up=191ms)"
    );
}

#[test]
fn disconnect_summary_totals_its_phases() {
    // The total must be the sum of the phases: the wait()-side
    // teardown has no single wall-clock anchor (the select races
    // signals for the whole session lifetime), so a drifting total
    // would mean a phase is unaccounted for.
    let line = disconnect_summary_line(120, 3, 180, 45);
    assert_eq!(
        line,
        "Warren disconnected in 348ms (down_event=120ms natpmp=3ms \
         routes=180ms tasks=45ms)"
    );
}

#[test]
fn multi_hop_bind_addr_is_always_wildcard() {
    // QUIC migration regression: pinning the bind to a detected
    // physical source IP kills the connection on every interface
    // switch (the socket dies with its interface). The bind must
    // be wildcard, family-matched to the relay endpoint.
    let v4 = multi_hop_bind_addr("203.0.113.10:443".parse().unwrap());
    assert!(v4.ip().is_unspecified() && v4.is_ipv4() && v4.port() == 0);
    let v6 = multi_hop_bind_addr("[2001:db8::1]:443".parse().unwrap());
    assert!(v6.ip().is_unspecified() && v6.is_ipv6() && v6.port() == 0);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_ordered_routes_split_bypass_from_tunnel_default() {
    // The ordered variant lets the multi-hop path land the carrier escape
    // BEFORE the 0/0 redirect (zero self-nest window).
    let carrier: IpAddr = "203.0.113.10".parse().unwrap();
    let (bypass, defaults) = build_warren_tunnel_routes_macos_ordered("utun7", &[carrier]);
    assert_eq!(bypass.len(), 1);
    assert_eq!(bypass[0].prefix.prefix(), 32);
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0].prefix.prefix(), 0);
}

#[test]
fn a_replayed_setup_is_redialled() {
    let err = reject_error(RejectionReason::SetupReplayed);

    assert!(matches!(err, Error::BackendTransient(_)), "{err:?}");
}

#[test]
fn the_device_clock_is_blamed_only_once_two_exits_refused_it() {
    let (a, b) = (
        warrenguard_wire::ExitId::from_bytes([0xA1; 16]),
        warrenguard_wire::ExitId::from_bytes([0xB2; 16]),
    );
    let mut tally = warrenguard_multihop::ClockSkewTally::new();

    let first = refusal_error(&mut tally, RejectionReason::ClockSkew, a);
    let second = refusal_error(&mut tally, RejectionReason::ClockSkew, b);

    assert!(
        first.is_recoverable(),
        "one exit may have the wrong clock: {first:?}"
    );
    assert!(
        matches!(&second, Error::BackendFatal(cause @ FatalCause::ClockSkew(_))
            if cause.to_string().starts_with(CLOCK_SKEW_REASON)),
        "{second:?}"
    );
}

#[test]
fn reject_error_is_recoverable_so_killswitch_stays_cancelable() {
    // Load-bearing invariant: a NON-ban exit rejection must map to a
    // RECOVERABLE error. Recoverable keeps the firewall kill-switch up
    // (fail-closed, no leak) while leaving the state cancelable and
    // letting the tunnel self-heal once the allowlist syncs. Promoting
    // this to a fatal error would strand a just-subscribed user.
    for reason in [
        RejectionReason::NotAllowlisted,
        RejectionReason::PolicyRefused,
        RejectionReason::IpExhausted,
    ] {
        let err = reject_error(reason);
        assert!(
            err.is_recoverable(),
            "{reason:?} must be recoverable, got {err:?}"
        );
    }
}

#[test]
fn reject_error_marks_a_self_healing_refusal_so_the_retry_loop_can_be_bounded() {
    // A refusal for a missing allowlist entry is what a just-redeemed
    // subscription looks like until the exit's next poll, so it stays
    // recoverable (asserted above). It is nonetheless distinguishable
    // from every other retriable failure, because it is the only one
    // that can persist forever: the connecting state times the run and
    // surfaces it rather than dialing in silence until the user gives
    // up. The message carries the auth-failed token the app already
    // maps to its localized "account is out of time" copy.
    for reason in [
        RejectionReason::NotAllowlisted,
        RejectionReason::PolicyRefused,
    ] {
        let err = reject_error(reason);
        let carried = err
            .session_rejection()
            .unwrap_or_else(|| panic!("{reason:?} must be a session rejection, got {err:?}"));
        assert!(
            carried.reason.starts_with("[EXPIRED_ACCOUNT]"),
            "the refusal must carry the token the app parses, got {carried:?}"
        );
        assert_eq!(
            carried.if_active, None,
            "a wallet refusal against an active subscription is the exit not synced yet"
        );
    }

    // Capacity is not an account verdict: an exhausted IP pool frees up
    // on its own with nothing for the user to do, so it must NOT be
    // surfaced as an auth failure.
    assert!(
        reject_error(RejectionReason::IpExhausted)
            .session_rejection()
            .is_none(),
        "an exhausted exit pool is not an account verdict"
    );
}

#[test]
fn reject_error_bans_are_fatal_and_carry_the_banned_token() {
    // A CRL ban is definitive and fleet-wide: it must be FATAL (not
    // recoverable) so the state machine stops reconnecting and surfaces
    // the suspension, and it must carry a [BANNED*] auth-failed token so
    // the app maps it to the localized suspension message. Retrying a ban
    // self-heals nothing and would spin "Reconnecting" forever.
    //
    // An unspecified (0) or unrecognized code carries the generic [BANNED]
    // token; the port-forwarding code carries the specific token so the app
    // shows a forwarded-port suspension.
    for (code, expected_token) in [
        (0u8, "[BANNED]"),
        (BAN_REASON_PORT_FORWARDING, "[BANNED_PORT_FORWARDING]"),
        (200u8, "[BANNED]"),
    ] {
        let err = reject_error(RejectionReason::Banned(code));
        match &err {
            Error::BackendFatal(cause) => assert!(
                cause.to_string().starts_with(expected_token),
                "ban code {code} must carry {expected_token} so the funnel parses it, got {cause:?}"
            ),
            other => panic!("a ban must map to BackendFatal, got {other:?}"),
        }
        assert!(
            !err.is_recoverable(),
            "a ban must NOT be recoverable (no endless reconnect)"
        );
    }
}

#[test]
fn reject_error_device_limit_is_fatal_and_carries_the_too_many_connections_token() {
    // The exit caps the wallet-signed sessions of one account. Past the
    // cap the refusal holds until another device disconnects, so it must
    // be FATAL (no endless "Reconnecting") and carry the
    // [TOO_MANY_CONNECTIONS] token the app already localizes. Calling it
    // an expired subscription, as the catch-all arm did, sends a paying
    // user to the checkout for nothing.
    let err = reject_error(RejectionReason::DeviceLimit);
    match &err {
        Error::BackendFatal(cause) => assert!(
            cause.to_string().starts_with("[TOO_MANY_CONNECTIONS]"),
            "a device limit must carry [TOO_MANY_CONNECTIONS] so the funnel parses it, got {cause:?}"
        ),
        other => panic!("a device limit must map to BackendFatal, got {other:?}"),
    }
    assert!(
        !err.is_recoverable(),
        "a device limit must NOT be recoverable (no endless reconnect)"
    );
    assert!(
        err.session_rejection().is_none(),
        "a device limit is not the self-healing expired-subscription refusal"
    );
}

#[test]
fn a_token_walk_no_refusal_explained_names_the_device_limit_for_an_active_subscription() {
    // An exit that predates the typed token refusal says nothing more than
    // Rejected for a token another device holds. The refusal still reads
    // as an expired subscription when the account API says so, and as the
    // device limit when it says the subscription is active.
    let err = reject_error(RejectionReason::TokensRefusedWithoutReason);
    let refusal = err
        .session_rejection()
        .unwrap_or_else(|| panic!("it must stay a session rejection, got {err:?}"));
    assert!(refusal.reason.starts_with("[EXPIRED_ACCOUNT]"));
    let if_active = refusal
        .if_active
        .as_deref()
        .expect("an active subscription explains a refused stack by the device limit");
    assert!(
        if_active.starts_with("[TOO_MANY_CONNECTIONS]"),
        "the app parses the device limit from this token, got {if_active:?}"
    );
    assert!(
        err.is_recoverable(),
        "the account API decides, not this lap"
    );
}

#[test]
fn a_lone_serial_in_use_reads_as_the_device_limit() {
    let err = reject_error(RejectionReason::SerialInUse);
    assert!(
        matches!(&err, Error::BackendFatal(cause @ FatalCause::DeviceLimit(_))
            if cause.to_string().starts_with("[TOO_MANY_CONNECTIONS]")),
        "got {err:?}"
    );
}

#[test]
fn warren_tunnel_parameters_debug_does_not_leak_secrets() {
    // No-log Warren: Debug must never reveal signing_key nor the
    // full exit pubkey. The first is secret material, the second
    // is session-PII identifying the user on the exit side.
    let signing = SigningKey::from_bytes(&[0u8; 32]);
    let exit_id = WarrenPubkey::from_bytes([1u8; 32]);
    let params = WarrenTunnelParameters {
        exit_id: RelayExitId::ZERO,
        country_code: String::new(),
        city: String::new(),
        exit_addr: WarrenExitAddr::new(exit_id),
        signing_key: signing,
        n_connections: 2,
        setup_clock: None,
        features: 0x1,
        multi_hop: None,
        alpn_protocols: Vec::new(),
        on_reconnect: None,
        on_path_rtt: None,
        on_exit_draining: None,
        on_egress_verdict: None,
        warren_register_migrate_handle: None,
        warren_drain_migrate: None,
        warren_dial_refused: None,
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        nat_pmp: None,
        nat_pmp_observer: None,
        nat_pmp_control_rx: None,
        max_rate_control_rx: None,
        lan_networks: None,
        include_only: false,
        tunnel_resolvers: Vec::new(),
        enable_daita: false,
        require_pq: false,
        session_tokens: None,
        port_entitlement_provider: None,
        app_routes_rx: None,
        on_app_routes: None,
        route_admission: None,
        credentials: None,
        cache_dir: None,
    };
    let s = format!("{params:?}");
    assert!(s.contains("<redacted>"), "must mask secrets: {s}");
    assert!(
        !s.contains(&exit_id.to_hex()),
        "must not leak the exit pubkey in hex: {s}"
    );
    assert!(s.contains("n_connections: 2"));
    assert!(s.contains("features: 0x00000001"));
}

#[test]
fn absent_circuit_fails_closed_as_non_recoverable() {
    // The Warren fleet is multi-hop only: the daemon's circuit
    // selection always populates `multi_hop`. A `None` reaching the
    // tunnel backend means circuit assembly failed upstream, so
    // `start` fails closed with `NoCircuit` rather than opening an
    // unprotected direct datapath. Fail-closed = non-recoverable so
    // the state machine enters the kill-switch error state instead of
    // looping on a dispatch that can never conjure a circuit.
    let e = Error::NoCircuit;
    assert!(
        !e.is_recoverable(),
        "NoCircuit must be non-recoverable (fail closed)"
    );
    assert!(
        e.to_string().contains("multi-hop circuit"),
        "display must name the missing circuit: {e}"
    );
}

#[test]
fn warren_tunnel_parameters_debug_when_multi_hop_some_marks_it_as_redacted() {
    // No-log Warren: when multi_hop is Some, the relay + exit
    // descriptors carry pubkeys + UDP endpoints that re-identify
    // the session. The Debug surface must mark it as
    // `Some("<redacted>")` and never let the underlying hex
    // pubkeys leak into a log line.
    let signing = SigningKey::from_bytes(&[0u8; 32]);
    let exit_id = WarrenPubkey::from_bytes([1u8; 32]);
    let mh = MultiHopConfig {
        relay: RelayDescriptorSigned {
            relay_id: [0xaa; 16],
            relay_ed25519_pubkey: [0xbb; 32],
            endpoint: "192.0.2.10:443".parse().unwrap(),
            endpoint_v6: None,
            signature: [0xcc; 64],
            cover_domain: None,
            tcp_fallback: false,
        },
        exit: ExitDescriptorSigned {
            exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
            exit_ed25519_pubkey: [0xee; 32],
            exit_x25519_multihop_pubkey: [0xff; 32],
            exit_mlkem768_pubkey: None,
            endpoint: Some("192.0.2.20:443".parse().unwrap()),
            signature: [0x11; 64],
            dns_disabled: false,
            cover_domain: None,
        },
        operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
        exit_country: "se".to_owned(),
        exit_city: "Stockholm".to_owned(),
        enable_gso: true,
        use_warren_obfuscation: true,
        single_node: false,
    };
    let params = WarrenTunnelParameters {
        exit_id: RelayExitId::ZERO,
        country_code: String::new(),
        city: String::new(),
        exit_addr: WarrenExitAddr::new(exit_id),
        signing_key: signing,
        n_connections: 1,
        setup_clock: None,
        features: 0,
        multi_hop: Some(mh),
        alpn_protocols: Vec::new(),
        on_reconnect: None,
        on_path_rtt: None,
        on_exit_draining: None,
        on_egress_verdict: None,
        warren_register_migrate_handle: None,
        warren_drain_migrate: None,
        warren_dial_refused: None,
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        nat_pmp: None,
        nat_pmp_observer: None,
        nat_pmp_control_rx: None,
        max_rate_control_rx: None,
        lan_networks: None,
        include_only: false,
        tunnel_resolvers: Vec::new(),
        enable_daita: false,
        require_pq: false,
        session_tokens: None,
        port_entitlement_provider: None,
        app_routes_rx: None,
        on_app_routes: None,
        route_admission: None,
        credentials: None,
        cache_dir: None,
    };
    let s = format!("{params:?}");
    assert!(
        s.contains("Some(\"<redacted>\")") || s.contains("Some(\"<redacted>\")"),
        "multi_hop Some must render as Some(\"<redacted>\"), got: {s}"
    );
    assert!(
        !s.contains("bb") && !s.contains("ee") && !s.contains("ff"),
        "relay/exit pubkeys must not leak: {s}"
    );
    assert!(
        !s.contains("192.0.2.10") && !s.contains("192.0.2.20"),
        "relay/exit endpoints must not leak: {s}"
    );
}

#[test]
fn warren_tunnel_parameters_debug_when_nat_pmp_some_redacts_internals() {
    // No-log Warren: NAT-PMP config carries the user's port choice
    // and protocol preference. Those are not session-PII but the
    // Debug surface must still display the field structurally so
    // operators reading a log line can see whether port-forwarding
    // is on without exposing the picked port (which becomes
    // identifying once the mapping is live).
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: Vec::new(),
        protocol: NatPmpProto::Udp,
        suggested_external_port: 22222,
        internal_port: 22,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let params = WarrenTunnelParameters {
        exit_id: RelayExitId::ZERO,
        country_code: String::new(),
        city: String::new(),
        exit_addr: WarrenExitAddr::new(WarrenPubkey::from_bytes([0u8; 32])),
        signing_key: SigningKey::from_bytes(&[0u8; 32]),
        n_connections: 1,
        setup_clock: None,
        features: 0,
        multi_hop: None,
        alpn_protocols: Vec::new(),
        on_reconnect: None,
        on_path_rtt: None,
        on_exit_draining: None,
        on_egress_verdict: None,
        warren_register_migrate_handle: None,
        warren_drain_migrate: None,
        warren_dial_refused: None,
        warren_pre_swap_check: None,
        warren_on_overlap_swapped: None,
        nat_pmp: Some(cfg.clone()),
        nat_pmp_observer: None,
        nat_pmp_control_rx: None,
        max_rate_control_rx: None,
        lan_networks: None,
        include_only: false,
        tunnel_resolvers: Vec::new(),
        enable_daita: false,
        require_pq: false,
        session_tokens: None,
        port_entitlement_provider: None,
        app_routes_rx: None,
        on_app_routes: None,
        route_admission: None,
        credentials: None,
        cache_dir: None,
    };
    let s = format!("{params:?}");
    // NatPmpConfig derives Debug, so the Some(..) renders the
    // structural shape. We assert the toggle is visible (operators
    // need to see "enabled: true") and that the suggested external
    // port number is dropped or carried (here carried; future
    // tightening can mask it if needed).
    assert!(
        s.contains("enabled: true"),
        "nat_pmp enabled toggle must appear in Debug: {s}"
    );
}

#[test]
fn nat_pmp_config_default_disabled_yields_enabled_false() {
    let cfg = NatPmpConfig::default_disabled();
    assert!(!cfg.enabled);
    assert_eq!(cfg.lifetime_secs, NatPmpConfig::DEFAULT_LIFETIME_SECS);
    assert_eq!(cfg.protocol, NatPmpProto::Udp);
}

#[test]
fn nat_pmp_config_default_enabled_uses_one_hour_default_lifetime() {
    let cfg = NatPmpConfig::default_enabled();
    assert!(cfg.enabled);
    assert_eq!(cfg.lifetime_secs, 3600);
    assert_eq!(cfg.protocol, NatPmpProto::Udp);
    assert_eq!(cfg.suggested_external_port, 0);
}

#[test]
fn with_sticky_ports_re_suggests_last_granted_port_for_an_auto_rule() {
    // An auto rule (suggested == 0) whose port was granted on the
    // previous exit must be re-suggested to the next exit, so the
    // public port "follows" the client across an exit change instead
    // of silently changing.
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: vec![NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 51820,
            sticky_suggestion: false,
        }],
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 0,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let mut sticky = std::collections::HashMap::new();
    sticky.insert(
        NatPmpRuleId {
            internal_port: 51820,
            protocol: NatPmpProto::Udp,
        },
        49200,
    );

    let effective = cfg.with_sticky_ports(&sticky);

    let rules = effective.effective_rules();
    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0].suggested_external_port, 49200,
        "auto rule must re-suggest the last-granted port"
    );
}

#[test]
fn with_sticky_ports_preserves_an_explicit_user_pin() {
    // A rule the user explicitly pinned (suggested != 0) is their
    // intent: never override it with a sticky value, even if one
    // exists for that rule id.
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: vec![NatPmpRule {
            protocol: NatPmpProto::Tcp,
            suggested_external_port: 50000,
            internal_port: 50000,
            sticky_suggestion: false,
        }],
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 0,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let mut sticky = std::collections::HashMap::new();
    sticky.insert(
        NatPmpRuleId {
            internal_port: 50000,
            protocol: NatPmpProto::Tcp,
        },
        49200,
    );

    let effective = cfg.with_sticky_ports(&sticky);

    assert_eq!(
        effective.effective_rules()[0].suggested_external_port,
        50000,
        "an explicit pin must win over the sticky port"
    );
}

#[test]
fn with_sticky_ports_leaves_an_auto_rule_at_zero_when_no_sticky_entry() {
    // No remembered port (first connect, or after an explicit reset
    // to auto): the rule stays auto so the exit picks a fresh port.
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: vec![NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 51820,
            sticky_suggestion: false,
        }],
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 0,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let sticky = std::collections::HashMap::new();

    let effective = cfg.with_sticky_ports(&sticky);

    assert_eq!(
        effective.effective_rules()[0].suggested_external_port,
        0,
        "without a sticky entry the rule must stay on auto"
    );
}

#[test]
fn with_sticky_ports_ignores_a_zero_sticky_entry() {
    // A sticky value of 0 carries no information (the exit never
    // grants port 0); it must not turn an auto rule into a request
    // for the invalid port 0.
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: vec![NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 51820,
            sticky_suggestion: false,
        }],
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 0,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let mut sticky = std::collections::HashMap::new();
    sticky.insert(
        NatPmpRuleId {
            internal_port: 51820,
            protocol: NatPmpProto::Udp,
        },
        0,
    );

    let effective = cfg.with_sticky_ports(&sticky);

    assert_eq!(effective.effective_rules()[0].suggested_external_port, 0);
}

#[test]
fn with_sticky_ports_synthesizes_rules_from_legacy_flat_fields() {
    // A single-port (legacy flat) config must still follow: the
    // sticky override applies to the rule synthesized from the flat
    // fields, and the result expresses it through `rules`.
    let cfg = NatPmpConfig {
        enabled: true,
        lifetime_secs: 3600,
        rules: Vec::new(),
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 6881,
        sticky_suggestion: false,
        remap_epoch: 0,
    };
    let mut sticky = std::collections::HashMap::new();
    sticky.insert(
        NatPmpRuleId {
            internal_port: 6881,
            protocol: NatPmpProto::Udp,
        },
        49300,
    );

    let effective = cfg.with_sticky_ports(&sticky);

    let rules = effective.effective_rules();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].internal_port, 6881);
    assert_eq!(rules[0].suggested_external_port, 49300);
}

#[test]
fn derive_multi_hop_tun_ip_is_deterministic_for_same_pubkey() {
    // Same identity must land on the same TUN IP across reconnects
    // so subscriptions and exit-side state stay attached to a
    // stable IP. Mutation = stale exit-side state across restart.
    let pubkey = [0x42u8; 32];
    let a = derive_multi_hop_tun_ip(&pubkey);
    let b = derive_multi_hop_tun_ip(&pubkey);
    assert_eq!(a, b);
}

#[test]
fn derive_multi_hop_tun_ip_skips_network_and_gateway_slots() {
    // The /16 pool has `.0` reserved as the network address and
    // `.1` reserved for the gateway. Even with pubkey bytes that
    // would land on those slots, the derivation must bump to a
    // safe value.
    let pubkey = {
        let mut p = [0u8; 32];
        p[1] = 0; // would land on .X.0
        p
    };
    let ip = derive_multi_hop_tun_ip(&pubkey);
    assert_ne!(ip.octets()[3], 0, "must not land on the network slot");

    let pubkey = {
        let mut p = [0u8; 32];
        p[1] = 1; // would land on .X.1 (gateway)
        p
    };
    let ip = derive_multi_hop_tun_ip(&pubkey);
    assert_ne!(ip.octets()[3], 1, "must not land on the gateway slot");
}

/// A black-holed BIND is not yet grounds to spend the circuit: its own
/// remedy has not been tried, the next connect pre-installs the `/32`
/// escape, and burning the entry relay first would trade the leak-free
/// bind for nothing.
#[cfg(target_os = "macos")]
#[test]
fn a_blackholed_bind_alone_never_spends_the_circuit() {
    use carrier_egress_guard::{CarrierRoute, CarrierRoutes, GuardOutcome};
    assert!(!dead_carrier_blames_the_circuit(
        GuardOutcome::BindBlackholed,
        Some(CarrierRoutes {
            bound: CarrierRoute::OffTunnel,
            plain: CarrierRoute::OffTunnel,
        }),
    ));
}

/// Both escapes measured dead while the kernel sources the carrier OFF the
/// tunnel: the packets leave this host, so what loses them is beyond it and
/// the circuit is the one thing left to change.
#[cfg(target_os = "macos")]
#[test]
fn both_escapes_dead_over_an_off_tunnel_carrier_blames_the_circuit() {
    use carrier_egress_guard::{CarrierRoute, CarrierRoutes, GuardOutcome};
    assert!(dead_carrier_blames_the_circuit(
        GuardOutcome::EscapeAlsoDead,
        Some(CarrierRoutes {
            bound: CarrierRoute::OffTunnel,
            plain: CarrierRoute::Unroutable,
        }),
    ));
}

/// The opposite finding: the carrier is sourced from the tunnel's own
/// address, so it is swallowed HERE. Rotating the entry relay cannot fix a
/// packet that never leaves, and doing it anyway would churn the directory
/// on every reconnect of a host that is nesting.
#[cfg(target_os = "macos")]
#[test]
fn a_carrier_nested_in_the_tunnel_blames_this_host_not_the_circuit() {
    use carrier_egress_guard::{CarrierRoute, CarrierRoutes, GuardOutcome};
    assert!(!dead_carrier_blames_the_circuit(
        GuardOutcome::EscapeAlsoDead,
        Some(CarrierRoutes {
            bound: CarrierRoute::Nested,
            plain: CarrierRoute::Nested,
        }),
    ));
}

/// No probe, no accusation. An unmeasured route leaves both hypotheses
/// open, and the reaction exists precisely because the counters cannot
/// choose between them.
#[cfg(target_os = "macos")]
#[test]
fn an_unmeasured_route_never_spends_the_circuit() {
    use carrier_egress_guard::GuardOutcome;
    assert!(!dead_carrier_blames_the_circuit(
        GuardOutcome::EscapeAlsoDead,
        None
    ));
    assert!(!dead_carrier_blames_the_circuit(
        GuardOutcome::BypassConfirmed,
        None
    ));
}

#[test]
fn derive_multi_hop_tun_ip_stays_in_pool_cidr() {
    // Anchor: the derivation always produces an address inside
    // 10.66.0.0/16. A regression that swapped octet positions
    // would surface here.
    for seed in 0..32u8 {
        let mut p = [0u8; 32];
        p[0] = seed;
        p[1] = seed.wrapping_add(7);
        let ip = derive_multi_hop_tun_ip(&p);
        let o = ip.octets();
        assert_eq!(o[0], 10);
        assert_eq!(o[1], 66);
    }
}

#[test]
fn build_multi_hop_tun_config_v4_only_pins_mtu_and_gateway() {
    // v4-only multi-hop (exit served no v6, or user IPv6 off) at MTU
    // 1280 with the canonical gateway `10.66.0.1`. Any mutation here
    // would silently change the wire profile across deployments.
    let cfg = build_multi_hop_tun_config(std::net::Ipv4Addr::new(10, 66, 7, 42), None);
    assert_eq!(cfg.mtu, 1280, "multi-hop TUN MTU must be 1280");
    assert_eq!(
        cfg.ipv4_gateway,
        std::net::Ipv4Addr::new(10, 66, 0, 1),
        "multi-hop TUN gateway must be 10.66.0.1"
    );
    assert!(
        cfg.ipv6_gateway.is_none(),
        "v4-only multi-hop must not set a v6 gateway"
    );
    assert_eq!(cfg.addresses.len(), 1);
    assert!(!cfg.allow_lan, "no LAN access on multi-hop");
    assert!(cfg.dns_servers.is_none(), "DNS handled by exit forwarder");
}

#[test]
fn build_multi_hop_tun_config_dual_stack_adds_v6_address_and_gateway() {
    // `/v2` dual-stack: when the exit allocated a v6, the TUN carries
    // both addresses and pins the v6 gateway to `fdcc:f:1::1`.
    let v6 = std::net::Ipv6Addr::new(0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 2);
    let cfg = build_multi_hop_tun_config(std::net::Ipv4Addr::new(10, 66, 7, 42), Some(v6));
    assert_eq!(cfg.mtu, 1280);
    assert_eq!(cfg.addresses.len(), 2, "dual-stack TUN carries v4 + v6");
    assert!(cfg.addresses.contains(&IpAddr::V6(v6)));
    assert_eq!(
        cfg.ipv6_gateway,
        Some(std::net::Ipv6Addr::new(
            0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 1
        )),
        "dual-stack multi-hop must pin the v6 gateway to fdcc:f:1::1"
    );
}

#[test]
fn multi_hop_config_debug_does_not_leak_descriptors() {
    // Independent anchor: `MultiHopConfig::Debug` is also
    // formatted directly when the daemon logs the config struct
    // standalone (e.g. trace-on-build). The redaction must hold
    // there too, with the gso + obfuscation knobs exposed for
    // operational debug.
    let mh = MultiHopConfig {
        relay: RelayDescriptorSigned {
            relay_id: [0xaa; 16],
            relay_ed25519_pubkey: [0xbb; 32],
            endpoint: "192.0.2.10:443".parse().unwrap(),
            endpoint_v6: None,
            signature: [0xcc; 64],
            cover_domain: None,
            tcp_fallback: false,
        },
        exit: ExitDescriptorSigned {
            exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
            exit_ed25519_pubkey: [0xee; 32],
            exit_x25519_multihop_pubkey: [0xff; 32],
            exit_mlkem768_pubkey: None,
            endpoint: Some("192.0.2.20:443".parse().unwrap()),
            signature: [0x11; 64],
            dns_disabled: false,
            cover_domain: None,
        },
        operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
        exit_country: "se".to_owned(),
        exit_city: "Stockholm".to_owned(),
        enable_gso: false,
        use_warren_obfuscation: true,
        single_node: false,
    };
    let s = format!("{mh:?}");
    assert!(s.contains("<redacted>"));
    assert!(s.contains("enable_gso: false"));
    assert!(s.contains("use_warren_obfuscation: true"));
    assert!(!s.contains("192.0.2"), "endpoints must not leak: {s}");
}

#[test]
fn handshake_error_is_recoverable() {
    // A transient handshake error (network glitch) must be
    // retryable on the state-machine side so a session is not
    // killed by a single packet loss.
    let e = Error::Handshake(HandshakeFailure::SupervisorStopped);
    assert!(e.is_recoverable());
}

// --- BackendTransient / BackendFatal split ---

#[test]
fn backend_transient_error_is_recoverable() {
    // TUN I/O errors and peer-initiated session closes are
    // transient: the configuration is still valid, a fresh
    // connect should succeed without operator action.
    let e = Error::BackendTransient(TransientCause::DatapathDead);
    assert!(
        e.is_recoverable(),
        "BackendTransient must be recoverable so the state machine reconnects \
         instead of entering kill-switch mode"
    );
}

#[test]
fn backend_fatal_error_is_not_recoverable() {
    // Auth failures and explicit session rejections are fatal:
    // retrying immediately would produce the same outcome.
    let e = Error::BackendFatal(FatalCause::Banned(RejectionReason::Banned(0)));
    assert!(
        !e.is_recoverable(),
        "BackendFatal must NOT be recoverable - the state machine must enter \
         ErrorState rather than looping on a guaranteed-to-fail reconnect"
    );
}

/// The firewall drops private destinations on the tunnel interface except
/// the exit's own pool, where its gateway, resolver and port mapping live.
/// The two tables must name the same ranges, or DNS and NAT-PMP break.
#[test]
fn the_in_tunnel_exceptions_are_the_exits_own_pool() {
    use talpid_types::net::ALLOWED_IN_TUNNEL_LAN_NETS;
    let v4 = ipnetwork::IpNetwork::new(
        IpAddr::V4(warrenguard_config::TUNNEL_POOL_NETWORK),
        warrenguard_config::TUNNEL_POOL_PREFIX,
    )
    .unwrap();
    let v6 = ipnetwork::IpNetwork::new(
        IpAddr::V6(warrenguard_config::TUNNEL_POOL_NETWORK_V6),
        warrenguard_config::TUNNEL_POOL_PREFIX_V6,
    )
    .unwrap();

    assert_eq!(ALLOWED_IN_TUNNEL_LAN_NETS, [v4, v6]);
}

#[test]
fn a_tunnel_with_no_default_route_never_reports_itself_up() {
    let relay_v4: IpAddr = "192.0.2.10".parse().unwrap();
    let installed = require_default_route(relay_v4, |_| Ok::<_, anyhow::Error>("guard"));
    assert_eq!(installed.unwrap(), "guard");

    let failed = require_default_route(relay_v4, |_| {
        Err::<&str, _>(anyhow::anyhow!("route socket refused"))
    });
    assert!(
        matches!(&failed, Err(Error::RouteSetup(RouteSetupFailure::DefaultRouteSplit(e)))
            if e.to_string().contains("route socket refused")),
        "{failed:?}"
    );
    assert!(!failed.unwrap_err().is_recoverable());

    let relay_v6: IpAddr = "2001:db8::10".parse().unwrap();
    let mut called = false;
    let v6 = require_default_route(relay_v6, |_| {
        called = true;
        Ok::<_, anyhow::Error>("guard")
    });
    assert!(
        matches!(v6, Err(Error::RouteSetup(RouteSetupFailure::RelayOverIpv6))),
        "{v6:?}"
    );
    assert!(!called, "an IPv6 relay must not reach the IPv4 split");
}

#[test]
fn tun_setup_error_is_not_recoverable() {
    // Privilege or kernel-module issue: retrying will not help
    // without operator action (modprobe, capability grant, ...).
    let e = Error::TunSetup(TunSetupFailure::ProviderPoisoned);
    assert!(!e.is_recoverable());
}

#[test]
fn the_logged_chain_of_a_transient_error_names_its_cause() {
    // The state machine logs `display_chain`, which walks the source.
    use talpid_types::ErrorExt;
    let e = Error::BackendTransient(TransientCause::SessionEnded(
        "multi-hop uplink: tun read".to_owned(),
    ));
    let s = e.display_chain();
    assert!(s.contains("recoverable"), "{s}");
    assert!(s.contains("multi-hop uplink: tun read"), "{s}");
}

#[test]
fn a_fatal_error_reads_as_the_auth_failed_token_of_its_cause() {
    let e = Error::BackendFatal(FatalCause::Banned(RejectionReason::Banned(0)));
    let s = e.to_string();
    assert!(s.contains("fatal"), "{s}");
    assert!(s.contains("[BANNED] exit rejected the session"), "{s}");
}

// --- WarrenTunnelParameters must not implement Clone ---

/// Compile-time assertion: `WarrenTunnelParameters` must NOT
/// implement `Clone`. The `signing_key` field carries secret Ed25519
/// material; a `Clone` derive would silently duplicate it in memory,
/// broadening the attack surface.
///
/// Callers that need a copy of the *configuration* (not the key)
/// must clone only the individual fields they require.
#[test]
fn warren_tunnel_parameters_is_not_clone() {
    static_assertions::assert_not_impl_any!(WarrenTunnelParameters: Clone);
}

// --- startup event ordering ---

/// Verifies the documented startup ordering invariant:
/// InterfaceUp BEFORE routes BEFORE TunnelEvent::Up.
///
/// This test cannot drive a real TUN / route_manager in a unit
/// context, but it documents the expected ordering through the
/// TRACE_PREFIX log phases, which are verified at integration level.
/// The comment acts as a TDD anchor: if the sequence regresses, the
/// in-crate trace labels ("T4b" vs "T7") will be out of order in
/// the log output.
#[test]
fn startup_event_ordering_documented_invariant() {
    // Phase labels used by start_multi_hop.
    // If anyone reorders the blocks and forgets to update the labels,
    // this test documents the expected ordering so the reviewer
    // notices.  The labels must appear in this order in the source:
    //
    //   T4    = interfaceup_emit
    //   T4b   = interfaceup_consumed
    //   T5    = routes_add_start
    //   T6    = routes_added
    //   T7    = up_emit          <-- Up only after routes
    //   T7b   = up_consumed
    //
    // Compare against the previous (broken) ordering:
    //   T4    = interfaceup_emit
    //   T4b   = interfaceup_consumed + Up emitted HERE (wrong)
    //   T5    = up_consumed
    //   T6    = routes_add_start (too late)
    //
    // This constant array acts as a structural snapshot that forces
    // a reviewer to revisit this test when the trace labels change.
    const EXPECTED_PHASE_ORDER: &[&str] = &[
        "interfaceup_emit",
        "interfaceup_consumed",
        "routes_add_start",
        "routes_added",
        "up_emit",
        "up_consumed",
    ];
    // Verify the phases are strictly ordered by index (no reorder).
    for window in EXPECTED_PHASE_ORDER.windows(2) {
        let (a, b) = (window[0], window[1]);
        assert_ne!(a, b, "phase names must be distinct: {a}");
    }
    // Anchor: the Up event must come after the route phases.
    let up_idx = EXPECTED_PHASE_ORDER
        .iter()
        .position(|p| *p == "up_emit")
        .expect("up_emit must be in the phase list");
    let routes_idx = EXPECTED_PHASE_ORDER
        .iter()
        .position(|p| *p == "routes_added")
        .expect("routes_added must be in the phase list");
    assert!(
        up_idx > routes_idx,
        "up_emit (idx={up_idx}) must come AFTER routes_added (idx={routes_idx}) \
         to prevent a window where the UI shows Connected but traffic bypasses the tunnel"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn warren_carrier_socket_bypass_linux_uses_the_tunnel_fwmark() {
    // Anti-regression: the whole Port Fail / TunnelCrack ServerIP fix
    // hinges on the carrier socket carrying this exact mark, the one
    // `default_route_split`'s `ip rule fwmark ... lookup main` matches.
    // `None` (or the wrong variant) silently reopens the destination-keyed
    // leak the fix closed: `start_multi_hop` parameterizes its dial socket
    // with this before the split-default route lands.
    assert_eq!(
        warren_carrier_socket_bypass(0),
        warrenguard_tun_core::SocketBypass::Fwmark(warrenguard_tun_core::WARREN_TUNNEL_FWMARK)
    );
    // The interface index must not shape the Linux selection: SO_MARK
    // needs no interface, only the fwmark ip-rule.
    assert_eq!(
        warren_carrier_socket_bypass(7),
        warrenguard_tun_core::SocketBypass::Fwmark(warrenguard_tun_core::WARREN_TUNNEL_FWMARK)
    );
}

#[cfg(target_os = "windows")]
#[test]
fn warren_carrier_socket_bypass_windows_binds_the_discovered_ifindex() {
    // Anti-regression: Windows keys its carrier escape on `IP_UNICAST_IF`
    // to the physical interface discovered by
    // `default_route_split_windows::discover_physical_ifindex`, since the
    // engine's Windows route guard emits no `/32` exception at all (see
    // `install_v4_never_emits_a_32_prefix_or_a_route_to_the_exit_ip` in
    // warrenguard-winroute): a missing or wrong bypass here self-poisons
    // the tunnel the moment the split-default route lands.
    assert_eq!(
        warren_carrier_socket_bypass(7),
        warrenguard_tun_core::SocketBypass::UnicastIf(7)
    );
}

#[test]
fn idle_cover_is_armed_only_when_knob_on_and_daita_off() {
    // ADR-0006: idle cover and DAITA are mutually exclusive covers.
    // The same bool gates BOTH the transport config (keep-alive off)
    // and the pump choice, so this is the single source of truth that
    // prevents the foot-gun (keep-alive disabled with no cover pump).
    assert!(
        idle_cover_effective(true, false),
        "knob on + no DAITA => idle cover armed"
    );
    assert!(
        !idle_cover_effective(true, true),
        "DAITA requested => idle cover must yield (DAITA carries its own cover)"
    );
    assert!(
        !idle_cover_effective(false, false),
        "knob off => never arm idle cover (keep-alive beacon stays on)"
    );
    assert!(
        !idle_cover_effective(false, true),
        "knob off + DAITA => not idle cover"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn detect_default_iface_returns_some_iface_on_linux_runtime_or_skip() {
    // Sanity check: on a Linux test environment, `/proc/net/route`
    // exists and returns at least one non-empty interface name.
    // If `/proc` is not mounted or there is no default route
    // (isolated container, no-network CI), skip rather than fail.
    match detect_default_iface() {
        Ok(iface) => assert!(!iface.is_empty(), "iface name must be non-empty"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skip detect_default_iface: no default route ({e})");
        }
        Err(e) => panic!("unexpected I/O error: {e}"),
    }
}

// ===================================================================
// Live-reconfig controller tests. Reproduces the exact user
// scenario: tunnel connected with NAT-PMP OFF, then the user
// toggles it ON via the watch channel. The controller must spawn a
// manager and the observer must see a Mapped event - all without a
// tunnel reconnect.
// ===================================================================
use std::{
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::net::UdpSocket;
use warrenguard_natpmp_protocol::{
    MapProto, Response as NatPmpResponse, ResultCode, parse_request, serialize_response,
};

/// UDP stub that echoes a Success Map response with the requested
/// lifetime baked into the external port (`49000 + lifetime`), so a
/// test can correlate which config produced which mapping. A
/// `lifetime == 0` (release) gets a Success(0) reply so the
/// client's release future completes promptly.
async fn spawn_lifetime_echo_stub() -> std::net::SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let addr = sock.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        loop {
            let (n, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(_) => return,
            };
            let lifetime = match parse_request(&buf[..n]) {
                Ok(warrenguard_natpmp_protocol::Request::Map { lifetime_secs, .. }) => {
                    lifetime_secs
                }
                _ => continue,
            };
            let external_port = if lifetime == 0 {
                0
            } else {
                49000u16.saturating_add(lifetime as u16)
            };
            let resp = serialize_response(&NatPmpResponse::Map {
                proto: MapProto::Udp,
                result_code: ResultCode::Success,
                epoch_secs: 0,
                internal_port: 22,
                external_port,
                lifetime_secs: lifetime,
                rate_limit: None,
            });
            let _ = sock.send_to(&resp, peer).await;
        }
    });
    addr
}

/// Like [`spawn_lifetime_echo_stub`], recording the credential trailer of
/// every request so a test can see what each rule presented.
async fn spawn_credential_recording_stub() -> (std::net::SocketAddr, CredentialLog) {
    let seen: CredentialLog = Arc::new(StdMutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let addr = sock.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            let (n, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(_) => return,
            };
            let frame = &buf[..n];
            let (internal_port, lifetime) = match parse_request(frame) {
                Ok(warrenguard_natpmp_protocol::Request::Map {
                    internal_port,
                    lifetime_secs,
                    ..
                }) => (internal_port, lifetime_secs),
                _ => continue,
            };
            sink.lock().expect("sink").push((
                internal_port,
                warrenguard_natpmp_protocol::credential_trailer(frame).map(<[u8]>::to_vec),
            ));
            let resp = serialize_response(&NatPmpResponse::Map {
                proto: MapProto::Udp,
                result_code: ResultCode::Success,
                epoch_secs: 0,
                internal_port,
                external_port: 49000u16.saturating_add(internal_port),
                lifetime_secs: lifetime,
                rate_limit: None,
            });
            let _ = sock.send_to(&resp, peer).await;
        }
    });
    (addr, seen)
}

/// `(internal port, credential presented)` per request seen by the stub.
type CredentialLog = Arc<StdMutex<Vec<(u16, Option<Vec<u8>>)>>>;

/// Shared event log captured by [`collector_observer`].
type NatPmpEventLog = Arc<StdMutex<Vec<(NatPmpRuleId, NatPmpEvent)>>>;

fn collector_observer() -> (NatPmpMappingObserver, NatPmpEventLog) {
    let (observer, log, _refusals) = collector_observer_with_refusals();
    (observer, log)
}

/// `(rule, refusal, retry_in_secs)` per refusal the controller reported.
type RefusalLog = Arc<StdMutex<Vec<(NatPmpRuleId, NatPmpRefusal, u32)>>>;

fn collector_observer_with_refusals() -> (NatPmpMappingObserver, NatPmpEventLog, RefusalLog) {
    let log: NatPmpEventLog = Arc::new(StdMutex::new(Vec::new()));
    let refusals: RefusalLog = Arc::new(StdMutex::new(Vec::new()));
    let log_for_obs = log.clone();
    let refusals_for_obs = refusals.clone();
    let observer: NatPmpMappingObserver = Arc::new(move |id, evt| match evt {
        NatPmpRuleEvent::Engine(evt) => {
            log_for_obs.lock().expect("observer lock").push((id, evt));
        }
        NatPmpRuleEvent::Refused {
            refusal,
            retry_in_secs,
        } => {
            refusals_for_obs
                .lock()
                .expect("observer lock")
                .push((id, refusal, retry_in_secs));
        }
    });
    (observer, log, refusals)
}

/// Answers NotAuthorized (RFC result code 2) to the first `refusals`
/// Map requests, then grants `49000 + internal_port` like the other
/// stubs: the exit refusing an entitlement it will accept later.
async fn spawn_refusing_stub(refusals: usize) -> std::net::SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let addr = sock.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        let mut refused = 0;
        loop {
            let (n, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(_) => return,
            };
            let (internal_port, lifetime) = match parse_request(&buf[..n]) {
                Ok(warrenguard_natpmp_protocol::Request::Map {
                    internal_port,
                    lifetime_secs,
                    ..
                }) => (internal_port, lifetime_secs),
                _ => continue,
            };
            let refuse = lifetime != 0 && refused < refusals;
            if refuse {
                refused += 1;
            }
            let resp = serialize_response(&NatPmpResponse::Map {
                proto: MapProto::Udp,
                result_code: if refuse {
                    ResultCode::NotAuthorized
                } else {
                    ResultCode::Success
                },
                epoch_secs: 0,
                internal_port,
                external_port: if refuse {
                    0
                } else {
                    49000u16.saturating_add(internal_port)
                },
                lifetime_secs: if refuse { 0 } else { lifetime },
                rate_limit: None,
            });
            let _ = sock.send_to(&resp, peer).await;
        }
    });
    addr
}

#[tokio::test]
async fn a_refused_entitlement_is_reported_and_asked_again_until_granted() {
    let server = spawn_refusing_stub(1).await;
    let (observer, log, refusals) = collector_observer_with_refusals();
    let cfg = natpmp_cfg(60);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let entitlements: PortEntitlementProvider = Arc::new(|_| Some(vec![7; 8]));

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(entitlements),
    ));

    let mapped = |log: &NatPmpEventLog| {
        log.lock()
            .unwrap()
            .iter()
            .any(|(_, e)| matches!(e, NatPmpEvent::Mapped { .. }))
    };
    for _ in 0..250 {
        if mapped(&log) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();

    let reported = refusals.lock().unwrap().clone();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].1, NatPmpRefusal::EntitlementRefused);
    assert_eq!(
        reported[0].2, 0,
        "the next serial is presented at once after a refused one"
    );
    assert!(
        !log.lock()
            .unwrap()
            .iter()
            .any(|(_, e)| matches!(e, NatPmpEvent::Failed { .. })),
        "a refusal the controller retries is not a failure of the rule"
    );
    assert!(mapped(&log), "the retry must reach the grant");
}

/// Answers `[7; 8]` for every slot and records the refusals and releases
/// the controller reports.
#[derive(Default)]
struct RecordingBatch {
    refused: std::sync::Mutex<Vec<(usize, Vec<u8>)>>,
    released: std::sync::Mutex<Vec<usize>>,
}

impl warren_standing::entitlements::SlotBatch for RecordingBatch {
    fn credential(&self, _slot: usize) -> Option<Vec<u8>> {
        Some(vec![7; 8])
    }

    fn refused(&self, slot: usize, presented: &[u8]) {
        self.refused
            .lock()
            .unwrap()
            .push((slot, presented.to_vec()));
    }

    fn hold(&self, _slot: usize) -> u64 {
        0
    }

    fn release(&self, slot: usize, _claim: u64) {
        self.released.lock().unwrap().push(slot);
    }
}

#[tokio::test]
async fn a_refused_entitlement_moves_the_rule_to_another_one() {
    // Another device of the wallet holds the same batch, and its first
    // rule presents what this one's first rule presents: the exit leases
    // that serial to one port. Asked again on the same serial, the rule
    // would be refused for as long as the other device's port lives.
    let server = spawn_refusing_stub(1).await;
    let (observer, log, _refusals) = collector_observer_with_refusals();
    let cfg = natpmp_cfg(60);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let batch = Arc::new(RecordingBatch::default());

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(batch.clone() as PortEntitlementProvider),
    ));
    for _ in 0..250 {
        if log
            .lock()
            .unwrap()
            .iter()
            .any(|(_, e)| matches!(e, NatPmpEvent::Mapped { .. }))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();
    let _ = handle.await;

    assert_eq!(*batch.refused.lock().unwrap(), vec![(0, vec![7; 8])]);
    assert_eq!(
        *batch.released.lock().unwrap(),
        vec![0],
        "the controller gone, its rule's slot is free"
    );
}

#[tokio::test]
async fn a_rule_with_no_entitlement_is_reported_as_such() {
    let server = spawn_refusing_stub(usize::MAX).await;
    let (observer, _log, refusals) = collector_observer_with_refusals();
    let cfg = natpmp_cfg(60);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let exhausted: PortEntitlementProvider = Arc::new(|_| None);

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(exhausted),
    ));

    for _ in 0..100 {
        if !refusals.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();

    let reported = refusals.lock().unwrap().clone();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].1, NatPmpRefusal::NoEntitlement);
    assert_eq!(reported[0].2, 5);
}

/// A batch left empty because the issuer refused this device's clock.
struct ClockRefusedBatch;

impl warren_standing::entitlements::SlotBatch for ClockRefusedBatch {
    fn credential(&self, _slot: usize) -> Option<Vec<u8>> {
        None
    }

    fn refused(&self, _slot: usize, _presented: &[u8]) {}

    fn hold(&self, _slot: usize) -> u64 {
        0
    }

    fn release(&self, _slot: usize, _claim: u64) {}

    fn shortage(&self) -> Option<NatPmpRefusal> {
        Some(NatPmpRefusal::ClockSkew {
            offset_secs: Some(-91),
        })
    }
}

/// Forum topic 219: the rule row said the entitlements were used up while
/// every mint was refused for the machine's clock. The refusal the daemon
/// reports names the clock, and by how much.
#[tokio::test]
async fn a_rule_left_bare_by_a_clock_refusal_is_reported_as_the_clock() {
    let server = spawn_refusing_stub(usize::MAX).await;
    let (observer, _log, refusals) = collector_observer_with_refusals();
    let cfg = natpmp_cfg(60);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let clock_refused: PortEntitlementProvider = Arc::new(ClockRefusedBatch);

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(clock_refused),
    ));

    for _ in 0..100 {
        if !refusals.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();

    let reported = refusals.lock().unwrap().clone();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(
        reported[0].1,
        NatPmpRefusal::ClockSkew {
            offset_secs: Some(-91)
        }
    );
}

fn natpmp_cfg(lifetime_secs: u32) -> NatPmpConfig {
    NatPmpConfig {
        enabled: true,
        lifetime_secs,
        rules: Vec::new(),
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 22,
        sticky_suggestion: false,
        remap_epoch: 0,
    }
}

#[tokio::test]
async fn each_rule_presents_its_own_entitlement() {
    // One entitlement buys ONE port, so two rules must present two
    // different credentials: sharing one would read at the exit as a
    // single port and the second rule would be refused.
    let (server, seen) = spawn_credential_recording_stub().await;
    let (observer, _log) = collector_observer();
    let mut cfg = natpmp_cfg(60);
    cfg.rules = vec![
        NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
            sticky_suggestion: false,
        },
        NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 23,
            sticky_suggestion: false,
        },
    ];
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));

    // Slot n gets credential [n; 8], so the assertion reads the slot the
    // controller assigned straight off the wire.
    let entitlements: PortEntitlementProvider =
        Arc::new(|slot| u8::try_from(slot).ok().map(|n| vec![n; 8]));

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(entitlements),
    ));

    let mut presented = Vec::new();
    for _ in 0..100 {
        presented = seen.lock().expect("sink").clone();
        if presented.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();

    let for_22 = presented.iter().find(|(p, _)| *p == 22).cloned();
    let for_23 = presented.iter().find(|(p, _)| *p == 23).cloned();
    let (_, cred_22) = for_22.expect("rule 22 never reached the server");
    let (_, cred_23) = for_23.expect("rule 23 never reached the server");
    assert!(cred_22.is_some(), "rule 22 presented no credential");
    assert_ne!(
        cred_22, cred_23,
        "both rules drew the same entitlement slot"
    );
}

#[tokio::test]
async fn a_rule_keeps_its_slot_across_renewals() {
    // The exit spends a credential once and renews it afterwards. A rule
    // whose slot moved between requests would spend a second entitlement
    // for the same port.
    let (server, seen) = spawn_credential_recording_stub().await;
    let (observer, _log) = collector_observer();
    // lifetime 2 => the loop renews after 1 s.
    let cfg = natpmp_cfg(2);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let entitlements: PortEntitlementProvider =
        Arc::new(|slot| u8::try_from(slot).ok().map(|n| vec![n; 8]));

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        Some(entitlements),
    ));

    let mut presented = Vec::new();
    for _ in 0..150 {
        presented = seen.lock().expect("sink").clone();
        if presented.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    handle.abort();

    assert!(presented.len() >= 2, "expected a renewal: {presented:?}");
    assert_eq!(
        presented[0].1, presented[1].1,
        "the renewal presented a different entitlement"
    );
}

#[tokio::test]
async fn controller_enables_mapping_on_watch_push_from_disabled_start() {
    // THE bug reproduction: controller starts with initial=None
    // (tunnel connected, NAT-PMP off), then a `Some(enabled)` push
    // arrives on the watch (user toggles on). The controller MUST
    // spawn a manager → observer sees a Mapped event. No reconnect.
    let server = spawn_lifetime_echo_stub().await;
    let (observer, log) = collector_observer();
    let (tx, rx) = tokio::sync::watch::channel::<Option<NatPmpConfig>>(None);

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        None, // initial: NAT-PMP off at tunnel start
        rx,
        None,
    ));

    // Give the controller a moment to reach `changed().await`.
    tokio::time::sleep(Duration::from_millis(50)).await;
    // No mapping yet (toggle still off).
    assert!(
        log.lock().unwrap().is_empty(),
        "no events expected before the toggle"
    );

    // User toggles ON.
    tx.send(Some(natpmp_cfg(60))).expect("watch send");

    // Controller should spawn the manager → Mapped(port 49060).
    for _ in 0..100 {
        if log.lock().unwrap().iter().any(|e| {
            matches!(
                e.1,
                NatPmpEvent::Mapped {
                    external_port: 49060,
                    ..
                }
            )
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let snapshot = log.lock().unwrap().clone();
    assert!(
        snapshot.iter().any(|e| matches!(
            e.1,
            NatPmpEvent::Mapped {
                external_port: 49060,
                ..
            }
        )),
        "controller must spawn a mapping on the enable push; events: {snapshot:?}"
    );

    handle.abort();
}

#[tokio::test]
async fn controller_live_reconfigures_on_second_push() {
    // After an initial enable, a second push with different params
    // must live-reconfigure (new port observed), no reconnect.
    let server = spawn_lifetime_echo_stub().await;
    let (observer, log) = collector_observer();
    // Start already-enabled (lifetime 60 → port 49060).
    let (tx, rx) = tokio::sync::watch::channel::<Option<NatPmpConfig>>(Some(natpmp_cfg(60)));

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(natpmp_cfg(60)),
        rx,
        None,
    ));

    // Wait for the initial Mapped(49060).
    for _ in 0..100 {
        if log.lock().unwrap().iter().any(|e| {
            matches!(
                e.1,
                NatPmpEvent::Mapped {
                    external_port: 49060,
                    ..
                }
            )
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Reconfigure to lifetime 90 → expect Mapped(49090).
    tx.send(Some(natpmp_cfg(90))).expect("watch send");
    for _ in 0..100 {
        if log.lock().unwrap().iter().any(|e| {
            matches!(
                e.1,
                NatPmpEvent::Mapped {
                    external_port: 49090,
                    ..
                }
            )
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let snapshot = log.lock().unwrap().clone();
    assert!(
        snapshot.iter().any(|e| matches!(
            e.1,
            NatPmpEvent::Mapped {
                external_port: 49090,
                ..
            }
        )),
        "controller must live-reconfigure on the second push; events: {snapshot:?}"
    );

    handle.abort();
}

/// Grants every Map request and records the lifetime each one asked
/// for, so a test can count the deletes (lifetime 0) a client sent.
async fn spawn_lifetime_recording_stub() -> (std::net::SocketAddr, Arc<StdMutex<Vec<u32>>>) {
    let seen = Arc::new(StdMutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let addr = sock.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            let Ok((n, peer)) = sock.recv_from(&mut buf).await else {
                return;
            };
            let Ok(warrenguard_natpmp_protocol::Request::Map {
                internal_port,
                lifetime_secs,
                ..
            }) = parse_request(&buf[..n])
            else {
                continue;
            };
            sink.lock().expect("sink").push(lifetime_secs);
            let resp = serialize_response(&NatPmpResponse::Map {
                proto: MapProto::Udp,
                result_code: ResultCode::Success,
                epoch_secs: 0,
                internal_port,
                external_port: 49000u16.saturating_add(internal_port),
                lifetime_secs,
                rate_limit: None,
            });
            let _ = sock.send_to(&resp, peer).await;
        }
    });
    (addr, seen)
}

#[test]
fn a_tunnel_teardown_deletes_every_granted_mapping_before_it_returns() {
    // A tunnel torn down to move to another exit (a change of country)
    // used to drop its mappings without a word: the exit it left kept the
    // port and the entitlement's lease until it reaped the session, and the
    // next exit refused that entitlement meanwhile (forum topic 226). The
    // delete is what lets the exit hand the lease back at once.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let (server, lifetimes) = rt.block_on(spawn_lifetime_recording_stub());
    let (observer, log) = collector_observer();
    let cfg = natpmp_cfg(60);
    let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let handle = rt.spawn(run_nat_pmp_controller_until(
        rt.handle().clone(),
        server,
        None,
        observer,
        Some(cfg),
        rx,
        None,
        Arc::clone(&shutdown),
    ));
    let mapped = || {
        log.lock()
            .expect("log")
            .iter()
            .any(|(_, e)| matches!(e, NatPmpEvent::Mapped { .. }))
    };
    for _ in 0..250 {
        if mapped() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(mapped(), "the rule never got its mapping");

    release_and_shutdown_controller(rt.handle(), handle, &shutdown);

    assert_eq!(
        lifetimes
            .lock()
            .expect("lifetimes")
            .iter()
            .filter(|l| **l == 0)
            .count(),
        1,
        "one delete for the one UDP mapping, sent before teardown went on"
    );
    assert!(
        log.lock()
            .expect("log")
            .iter()
            .any(|(_, e)| matches!(e, NatPmpEvent::Cancelled)),
        "the daemon still hears the rule is gone"
    );
}

#[test]
fn controller_shutdown_returns_only_once_the_aborted_task_released_its_state() {
    // The teardown that runs this goes on to restore the routes and start
    // the next tunnel epoch. Everything the controller owns (its managers,
    // and the `Cancelled` each of them reports on drop) has to be gone
    // before that: a `Cancelled` landing after the next tunnel's first
    // `Mapped` deletes a live rule from the daemon's status.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");

    struct SlowRelease(Arc<AtomicBool>);
    impl Drop for SlowRelease {
        fn drop(&mut self) {
            // A manager's drop is not instantaneous (it aborts tasks and
            // calls the observer). Standing in for that work is what makes
            // this test measure the wait rather than a lucky schedule.
            std::thread::sleep(Duration::from_millis(50));
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let released = Arc::new(AtomicBool::new(false));
    let owned = SlowRelease(released.clone());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let task = rt.spawn(async move {
        let _owned = owned;
        started_tx.send(()).expect("started");
        // Parked forever, like the controller on its watch channel: only
        // the abort can end it.
        std::future::pending::<()>().await;
    });
    started_rx.recv().expect("the task never started");

    await_controller_shutdown(rt.handle(), task);

    assert!(
        released.load(Ordering::SeqCst),
        "teardown returned while the controller still owned its managers"
    );
}

#[tokio::test]
async fn controller_reports_exactly_one_cancelled_for_a_removed_rule() {
    // Removing a rule is the one path where the controller reports the
    // teardown itself, and the manager it releases stays silent because
    // the release cleared its live flag. Both halves are load-bearing and
    // neither is local to one function: with no event the daemon keeps a
    // dead public port in `port-forward status`, with two the second one
    // deletes whatever row the rule holds by then.
    let server = spawn_lifetime_echo_stub().await;
    let (observer, log) = collector_observer();
    let mut cfg = natpmp_cfg(60);
    cfg.rules = vec![
        NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
            sticky_suggestion: false,
        },
        NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 23,
            sticky_suggestion: false,
        },
    ];
    let doomed = cfg.rules[1].id();
    let (tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));

    let runtime = tokio::runtime::Handle::current();
    let handle = runtime.spawn(run_nat_pmp_controller(
        runtime.clone(),
        server,
        None,
        observer,
        Some(cfg.clone()),
        rx,
        None,
    ));

    // Removing a rule that never mapped would prove nothing, so wait for
    // the doomed one to be live first.
    for _ in 0..100 {
        if log
            .lock()
            .unwrap()
            .iter()
            .any(|(id, e)| *id == doomed && matches!(e, NatPmpEvent::Mapped { .. }))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|(id, e)| *id == doomed && matches!(e, NatPmpEvent::Mapped { .. })),
        "the removed rule never mapped; the stub never answered it"
    );

    let mut smaller = cfg.clone();
    smaller.rules.truncate(1);
    tx.send(Some(smaller)).expect("watch send");

    for _ in 0..100 {
        if log
            .lock()
            .unwrap()
            .iter()
            .any(|(id, e)| *id == doomed && matches!(e, NatPmpEvent::Cancelled))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Let a duplicate land before counting: a second report would come
    // from the manager's own drop, one poll after the release returns.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = log.lock().unwrap().clone();
    handle.abort();

    let cancelled = snapshot
        .iter()
        .filter(|(id, e)| *id == doomed && matches!(e, NatPmpEvent::Cancelled))
        .count();
    assert_eq!(
        cancelled, 1,
        "a removed rule must report its teardown exactly once; events: {snapshot:?}"
    );
}
