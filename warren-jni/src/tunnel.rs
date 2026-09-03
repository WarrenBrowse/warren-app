// Warren Quinn tunnel session bootstrap.
//
// Spawned from `Java_..._connectTunnel`. Owns the async lifecycle:
//
//   1. Derives the wallet `SigningKey` from the supplied mnemonic.
//   2. Verifies the signed multi-hop directory prefetched by Kotlin and
//      selects the circuit (a distinct entry relay for a 2-hop circuit, or
//      the exit collapsed onto itself for a 1-hop circuit).
//   3. Brings up the `MultiHopClient` (HPKE setup + IP negotiation over the
//      control stream) and pumps between the Android TUN fd and the circuit.
//      The production fleet speaks only the multi-hop wire.
//
// All work happens on the shared [`crate::android_jni::RUNTIME`] tokio
// runtime so the JNI entry point can return synchronously while the
// session lives on. Status transitions surface through [`SessionStatus`]
// behind an atomic so [`Java_..._getTunnelStatus`] can poll them without
// further locking.

#![cfg(all(target_os = "android", feature = "tunnel"))]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::time::Instant;

/// Tunnel-inner addresses the Android `VpnService` interface is configured
/// with. Mirror `WarrenTunDefaults.IPV4_ADDRESS` / `IPV6_ADDRESS` in
/// `app/.../service/WarrenTunInterfacePlan.kt`; the multi-hop data plane NATs
/// these to the exit-assigned addresses (see [`run_multi_hop_session`]).
const LOCAL_TUN_IPV4: Ipv4Addr = Ipv4Addr::new(10, 64, 0, 1);
const LOCAL_TUN_IPV6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1);

use ed25519_dalek::SigningKey;
use rand_v9::SeedableRng;
use serde::Deserialize;
use tokio::sync::oneshot;
use warrenguard_daita::{DaitaPool, DaitaState};
use warrenguard_transport::AndroidTun;
use warrenguard_transport::supervisor::SessionTokenProvider;
use warrenguard_wire::WarrenPubkey;

/// JSON config parsed from the Kotlin side. Field names mirror
/// `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt`.
/// Fields the Android layer enforces on its own (allow_lan, mtu, etc.) are
/// accepted but unread here; there is no `deny_unknown_fields`.
#[derive(Debug, Deserialize)]
pub struct WarrenTunnelConfig {
    pub exit_pubkey_hex: String,
    /// Kept for wire parity with the Kotlin payload; the multi-hop datapath
    /// resolves the exit endpoint from the signed directory, so it is unread
    /// outside the parse tests.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "exit endpoint resolved from the signed multi-hop directory"
        )
    )]
    pub exit_endpoint: String,
    #[expect(dead_code, reason = "config field reserved for later wiring")]
    pub wallet_pubkey_hex: Option<String>,
    /// Multi-hop entry relay hint. When present, `run_session` routes through
    /// `MultiHopClient` (see `run_multi_hop_session`); the value carries an
    /// optional `relay_pubkey_hex` to prefer a specific entry relay.
    pub entry_hop: Option<serde_json::Value>,
    /// Raw signed multi-hop directory, fetched by Kotlin BEFORE the TUN is
    /// established (so the request egresses the physical network). When present
    /// `run_multi_hop_session` verifies + uses it instead of fetching the
    /// directory itself, which would be blackholed by the half-open tunnel.
    #[serde(default)]
    pub multihop_directory_raw: Option<String>,
    /// Preferred entry-relay country (ISO 3166-1 alpha-2, case-insensitive).
    /// `None`/empty selects automatically. Matched against `NodeEntry.country`
    /// in `run_multi_hop_session`; an explicit `entry_hop.relay_pubkey_hex`
    /// pubkey hint takes precedence over the country.
    #[serde(default)]
    pub entry_country: Option<String>,
    /// Multi-hop topology selector, mirroring the iOS FFI `multihop_two_hop`.
    /// `Some(true)` (and `None`, the older-payload default) selects a 2-hop
    /// circuit: a DISTINCT entry relay in front of the exit. `Some(false)`
    /// selects the single-hop 1-hop circuit: the exit node is dialed as its
    /// own entry relay on the unified `:443` dispatcher, so the setup frame's
    /// exit_id names that node and it terminates locally (no second hop),
    /// mirroring iOS `select_one_hop` / the daemon `select_one_hop_circuit`.
    /// The 1-hop circuit still rides the multi-hop wire (the production fleet
    /// speaks only that), so `entry_hop` + `multihop_directory_raw` remain
    /// required. Never inferred: a 2-hop request is never silently collapsed
    /// to 1-hop (that would be a privacy downgrade).
    #[serde(default)]
    pub multihop_two_hop: Option<bool>,
    /// Client-side DAITA toggle: when present the multi-hop setup requests
    /// DAITA and the pump builds a `DaitaState` from the curated pool (see
    /// `build_multihop_daita_state`). The exit accepts and emits the `0xFF`
    /// dummy frames on the multi-hop path, so the shaping is a real defense.
    /// Absent means no DAITA request.
    pub daita: Option<serde_json::Value>,
    /// Client opt-in for the NAT-PMP refresh loop. When `Some(true)`
    /// the session spawns a `warrenguard_natpmp_client::spawn_refresh_loop_from_addr`
    /// after the handshake completes, binding the UDP socket to the
    /// assigned tunnel inner IPv4 so the request egresses through the
    /// tunnel (and not the underlying mobile-data / Wi-Fi interface).
    /// Assigned external port surfacing to Kotlin is a follow-up; for
    /// now we log it at INFO.
    pub nat_pmp_enabled: Option<bool>,
    /// NAT-PMP mapping protocol: "udp" (default) or "tcp".
    #[serde(default)]
    pub nat_pmp_protocol: Option<String>,
    /// Requested external port; `0`/absent means the gateway picks one.
    #[serde(default)]
    pub nat_pmp_external_port: Option<u16>,
    /// Requested mapping lifetime in seconds (gateway may cap it).
    #[serde(default)]
    pub nat_pmp_lifetime_secs: Option<u32>,
    /// Whether IPv6 is carried through the tunnel. Enforced Android-side at
    /// the `VpnService.Builder` layer (route + address selection); accepted
    /// here so the schema stays in sync and future client-side IPv6
    /// filtering can read it. `#[serde(default)]` keeps older payloads valid.
    #[serde(default)]
    pub enable_ipv6: Option<bool>,
    /// The TUN MTU Kotlin configured (`VpnService.Builder.setMtu`). Read only
    /// by the "Reduced MTU" sampler, which compares the live path's usable
    /// inner payload against it; absent on older payloads, where the Kotlin
    /// default applies.
    #[serde(default)]
    pub mtu: Option<u16>,
    /// App-level kill switch. Enforced Android-side (the adapter keeps a
    /// blackhole interface up when the tunnel drops). Accepted here for
    /// schema parity.
    #[serde(default)]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "lockdown enforced Android-side via blackhole interface"
        )
    )]
    pub lockdown_mode: Option<bool>,
    /// DNS options. On Android the magic-IP DNS (`100.64.0.X`) is composed and
    /// applied client-side via `VpnService.Builder.addDnsServer`, and the
    /// per-category blocking is done by the exit's kresd resolver. This field is
    /// therefore unused here; it is accepted only for schema parity with desktop.
    #[serde(default)]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "DNS applied Android-side via VpnService; exit blocking done by kresd"
        )
    )]
    pub dns: Option<serde_json::Value>,
    /// Client-side bandwidth ceiling in bits per second, enforced on
    /// both directions independently at the tunnel packet device.
    /// Absent/zero = uncapped. Applied at tunnel start (a change takes
    /// effect on the next connect, like the MTU).
    #[serde(default)]
    pub max_rate_bps: Option<u64>,
}

pub use crate::redial::SessionStatus;

/// Errors surfaced from the synchronous JNI entry path. Currently only
/// `JsonParse` actually fires - mnemonic / pubkey / endpoint failures
/// happen inside the spawned `run_session` task and log there. The other
/// variants are kept to document the surface the caller should eventually
/// branch on once a richer error-channel makes it back across JNI.
///
/// The `clippy::enum_variant_names` allow exists because the "what failed"
/// narration is the variant's purpose; renaming each to drop the common
/// `Mnemonic` / `Config` / `ExitPubkey` etc. suffixes would lose that.
#[expect(dead_code, reason = "richer error reporting wired in a follow-up")]
#[expect(
    clippy::enum_variant_names,
    reason = "shared `Invalid*` prefix is the variant's documentation purpose"
)]
#[derive(Debug, thiserror::Error)]
pub enum TunnelStartError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid config JSON: {0}")]
    InvalidConfig(#[source] serde_json::Error),
    #[error("invalid exit pubkey hex: {0}")]
    InvalidExitPubkey(String),
    #[error("invalid exit endpoint: {0}")]
    InvalidExitEndpoint(String),
}

/// Drive a single Warren Quinn tunnel from start to teardown.
///
/// Runs on the caller's tokio runtime (the JNI-shared `RUNTIME`). The
/// `status` cell is updated as the session progresses, waking the Kotlin
/// waiter on every edge; the `cancel_rx` oneshot is selected against the
/// pump so a `WarrenJni.disconnectTunnel()` call from Kotlin terminates the
/// task promptly. `status` is held as a `'static` reference because the JNI
/// side parks the cell in a `static`; this keeps `run_session`
/// `'static`-bounded for `runtime.spawn(...)` without an extra `Arc`
/// allocation.
///
/// The function accepts a pre-derived `signing_key` rather than a raw
/// mnemonic string: key derivation happens at the JNI boundary
/// (synchronously, before this task is spawned) and the mnemonic is
/// zeroized there - it never crosses into the async lifetime.
pub async fn run_session(
    tun: AndroidTun,
    signing_key: SigningKey,
    config: WarrenTunnelConfig,
    status: &'static crate::status_watch::StatusCell,
    cancel_rx: oneshot::Receiver<()>,
) {
    status.store(SessionStatus::Connecting as i32);
    // Re-arm the egress verdict: it deliberately outlives the session it ended
    // (see `android_jni::reset_egress_dead`), so a fresh connect is what clears
    // it, not the teardown it caused.
    crate::android_jni::reset_egress_dead();

    // v7 anonymous admission (default, warren-core doc 64): background-minted Privacy
    // Pass tokens are presented at setup so the exit admits the session
    // without learning the wallet; an empty stack (nothing minted yet, epoch
    // drained, no issuance) keeps the v6 wallet-signed path.
    let session_tokens = crate::token_provider::provider_for(signing_key.clone());

    // The production fleet speaks only the multi-hop wire, so a valid config
    // always carries an entry hop: the Kotlin builder sets it even when the
    // multi-hop toggle is off (a 1-hop circuit with the exit collapsed onto
    // itself). A missing entry_hop is a malformed config, never a valid
    // request, so fail closed instead of guessing.
    if config.entry_hop.is_none() {
        log::error!("multi-hop entry hop missing from config; failing closed");
        status.store(SessionStatus::Disconnected as i32);
        return;
    }
    run_multi_hop_session(tun, signing_key, session_tokens, config, status, cancel_rx).await;
}

/// Warren multi-hop directory root signing key (baked pin). Mirrors
/// `mullvad-daemon::warren_multi_hop_directory::WARREN_MULTIHOP_ROOT_PUBKEY_BAKED`.
/// The directory envelope must chain to this root in addition to the server pin.
pub(crate) const WARREN_MULTIHOP_ROOT_PUBKEY_HEX: &str =
    "33cd9279ad06d1ee884235e763b876fa70598094944bdcfb82375bd9aaa67b08";

/// Drive a multi-hop tunnel (entry relay -> exit) from start to teardown.
///
/// Mirrors the desktop reference (`talpid-warren-tunnel::start_multi_hop`) and
/// iOS: a [`MultiHopSupervisor`] owns the session across disconnects and the
/// two `supervised_pump` halves pump the Android TUN against whatever it
/// publishes. Selection uses the signed multi-hop directory
/// (`GET /v1/multihop/directory`), the only source of the signed relay
/// descriptor, the exit X25519 HPKE key, and the operational trust anchor
/// (none of which the `/v1/exits` list carries).
///
/// [`MultiHopSupervisor`]: warrenguard_transport::supervisor::MultiHopSupervisor
async fn run_multi_hop_session(
    tun: AndroidTun,
    signing: SigningKey,
    session_tokens: SessionTokenProvider,
    config: WarrenTunnelConfig,
    status: &'static crate::status_watch::StatusCell,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    use std::sync::Arc;

    use warrenguard_multihop::RejectionReason;
    use warrenguard_transport::IpAssignChannel;
    use warrenguard_transport::supervisor::{MultiHopSupervisor, SupervisorConfig};

    use crate::supervised_session::{AbortOnDrop, SessionEnd, SupervisedInputs};

    let want_exit = match WarrenPubkey::from_hex(&config.exit_pubkey_hex) {
        Ok(p) => p,
        Err(e) => {
            log::error!("multi-hop: invalid exit pubkey: {e}");
            status.store(SessionStatus::Disconnected as i32);
            return;
        }
    };
    // Optional preferred entry relay (ed25519), from the Kotlin entry_hop hint.
    let want_entry = config
        .entry_hop
        .as_ref()
        .and_then(|v| v.get("relay_pubkey_hex"))
        .and_then(|v| v.as_str())
        .and_then(|s| WarrenPubkey::from_hex(s).ok());

    // Verify + use the signed multi-hop directory. It MUST be supplied by the
    // caller (Kotlin fetches it pre-TUN via WarrenJni.fetchMultihopDirectory):
    // fetching here, after the TUN is up, would route the request into the
    // half-open tunnel and blackhole it. Fail closed if it is missing.
    let raw = match config.multihop_directory_raw.as_deref() {
        Some(raw) if !raw.is_empty() => raw.to_owned(),
        _ => {
            log::error!(
                "multi-hop: no directory supplied in config (Kotlin must prefetch it pre-TUN); \
                 failing closed"
            );
            status.store(SessionStatus::Disconnected as i32);
            return;
        }
    };
    let server_pins: Vec<&str> = crate::product::SERVER_PUBKEY_HEX.into_iter().collect();
    let dir = match warren_discovery_core::verify_multihop_directory_any(
        &raw,
        &server_pins,
        &[WARREN_MULTIHOP_ROOT_PUBKEY_HEX],
    ) {
        Ok(d) => d,
        Err(e) => {
            log::error!("multi-hop: directory verify failed: {e}");
            status.store(SessionStatus::Disconnected as i32);
            return;
        }
    };

    // Choose the circuit. Single-hop collapses the entry onto the exit node
    // (1-hop, entry == exit); two-hop picks a DISTINCT entry with precedence
    // (pubkey hint > entry country > first distinct) and fails closed. The
    // topology is EXPLICIT (`config.multihop_two_hop`); a 2-hop request is
    // never silently downgraded to 1-hop. `None`/older payloads default to
    // 2-hop, preserving the shipping Android behavior. Selection logic + its
    // fail-closed contract live in the host-tested `crate::circuit_select`.
    let two_hop = config.multihop_two_hop.unwrap_or(true);
    let views: Vec<crate::circuit_select::NodeSel<'_>> = dir
        .nodes
        .iter()
        .map(|n| crate::circuit_select::NodeSel {
            exit_ed25519: &n.exit.exit_ed25519_pubkey,
            relay_id: &n.relay.relay_id,
            relay_ed25519: &n.relay.relay_ed25519_pubkey,
            country: &n.country,
        })
        .collect();
    let (entry_idx, exit_idx) = match crate::circuit_select::select_circuit_indices(
        &views,
        want_exit.as_bytes(),
        two_hop,
        want_entry.as_ref().map(WarrenPubkey::as_bytes),
        config.entry_country.as_deref(),
    ) {
        Ok(pair) => pair,
        // No-logs: state the failure category, never the exit/entry identity.
        Err(crate::circuit_select::CircuitSelectError::ExitNotInDirectory) => {
            log::error!("multi-hop: chosen exit not in directory; failing closed");
            status.store(SessionStatus::Disconnected as i32);
            return;
        }
        Err(crate::circuit_select::CircuitSelectError::NoDistinctEntry) => {
            log::error!("multi-hop: no distinct entry relay available; failing closed");
            status.store(SessionStatus::Disconnected as i32);
            return;
        }
    };
    // For a 1-hop circuit entry_idx == exit_idx, so `entry_node.relay` is the
    // exit node's own relay descriptor: we dial that node's `:443` dispatcher
    // and forward to its own exit_id, which the dispatcher terminates locally.
    let entry_node = &dir.nodes[entry_idx];
    let exit_node = &dir.nodes[exit_idx];

    let bind_addr: SocketAddr = "0.0.0.0:0".parse().expect("static bind addr");
    // No-logs: never record the entry-relay address or the exit identity pubkey -
    // that pair would deanonymize the chosen circuit (and this tag is captured
    // into user-submitted problem reports). Only non-identifying mode booleans
    // (topology + DAITA request), never an address or a pubkey.
    log::info!(
        "multi-hop connect (two_hop={two_hop}, daita_requested={})",
        config.daita.is_some()
    );
    // Post-quantum X-Wing seal by default with automatic classical fallback.
    // The exit's ML-KEM key is passed only when its signed descriptor advertises
    // a valid one (`negotiate_pq` verifies the operational signature over the
    // key, the anti-downgrade anchor); the supervisor dials `require_pq=false`,
    // so a classical exit stays on the byte-identical X25519 seal. No exit
    // publishes a PQ descriptor yet, so this currently always takes the
    // classical path.
    #[cfg(feature = "pq-hpke")]
    let exit_mlkem768_pubkey =
        match warrenguard_multihop::negotiate_pq(&dir.operational_pubkey, &exit_node.exit, false) {
            Ok(warrenguard_multihop::PqAvailability::Available) => {
                exit_node.exit.exit_mlkem768_pubkey.clone()
            }
            _ => None,
        };
    #[cfg(not(feature = "pq-hpke"))]
    let exit_mlkem768_pubkey: Option<Vec<u8>> = None;

    // v7 anonymous admission (default, warren-core doc 64). The supervisor calls
    // the provider once per session establishment, so every redial presents a
    // FRESH token: replaying a spent serial would be refused. `presented`
    // records whether a stack actually rode the last setup, which is what lets a
    // rejection be attributed to the TOKEN instead of the subscription.
    let presented = Arc::new(AtomicBool::new(false));
    let counting_tokens: SessionTokenProvider = {
        let presented = Arc::clone(&presented);
        Arc::new(move || {
            let stack = session_tokens();
            presented.store(!stack.is_empty(), Ordering::Relaxed);
            stack
        })
    };

    // DAITA over multi-hop: build a local DaitaState from the curated pool
    // (the exit accepts + emits 0xFF dummy frames on the multi-hop path) and
    // run the DAITA-aware pumps. A disabled state keeps them padding-free.
    // Shared across redials so the machines keep their state through a blip.
    let daita: warrenguard_transport::supervised_pump::DaitaShared =
        Arc::new(parking_lot::Mutex::new(build_multihop_daita_state(&config)));
    // Client-side bandwidth ceiling (config `max_rate_bps`): wrap the
    // remapped TUN so both pump directions pass the token-bucket gate.
    // Zero/absent = uncapped (no wrapper, no per-packet cost).
    let max_rate_bps = config.max_rate_bps.filter(|bps| *bps > 0);
    if let Some(bps) = max_rate_bps {
        log::info!("multi-hop: client-side bandwidth cap active ({bps} bps per direction)");
    }

    // At most two attempts: the second drops the anonymous tokens after a
    // rejection that was a verdict on the token, not on the account.
    let mut token_provider = Some(counting_tokens);
    loop {
        let ip_assign_channel = IpAssignChannel::new();
        let supervisor_config = SupervisorConfig {
            relay: Arc::new(entry_node.relay.clone()),
            exit_id: exit_node.exit.exit_id,
            exit_x25519_multihop_pubkey: exit_node.exit.exit_x25519_multihop_pubkey,
            exit_mlkem768_pubkey: exit_mlkem768_pubkey.clone(),
            operational_pubkey: dir.operational_pubkey,
            client_signing: signing.clone(),
            bind_addr,
            // Android has no UDP GSO on its TUN path; the dial keeps it off.
            enable_gso: false,
            use_warren_obfuscation: true,
            // Android keeps the tunnel's own carrier socket out of the VPN
            // through VpnService.protect at the platform layer, so the engine's
            // fwmark/bound-interface bypass (privileges an app does not have) is
            // unused. The protector is process-wide and is applied inside the
            // dial itself, so it covers every redial the supervisor makes.
            socket_bypass: None,
            enable_daita: config.daita.is_some(),
            // Keep the fixed keep-alive beacon: this is byte-identical to the
            // pre-supervisor dial, which used the same obfuscation preset.
            idle_cover: false,
            // Same low ceiling as the desktop daemon and iOS: the default
            // 15 s HANDSHAKE cap overshoots a mobile network-change recovery,
            // parking a redial well past the moment the link returns.
            backoff: warrenguard_backoff::Backoff {
                base: std::time::Duration::from_millis(300),
                max: std::time::Duration::from_secs(2),
            },
            on_reconnect: Some(crate::android_jni::auto_recovery_observer()),
            ip_assign_channel: Some(ip_assign_channel.clone()),
            wants_ipv6: config.enable_ipv6.unwrap_or(false),
            // One connection: the mobile radio path does not benefit from
            // bonding, and each bonded dial costs a handshake.
            n_connections: 1,
            pre_swap_check: None,
            on_overlap_swapped: None,
            on_dial_refused: None,
            on_path_rtt: None,
            session_token_provider: token_provider.clone(),
        };

        let (supervisor, sessions) = MultiHopSupervisor::new(supervisor_config);
        // Publish the goodput prober's verdict for Kotlin to poll. The
        // supervisor's dead-path watches cannot see the degradation class
        // where the transport stays up while nothing crosses the datapath, so
        // without this the app tells the user "You are protected" on a tunnel
        // that carries nothing. Must be taken before `run` consumes the
        // supervisor. The task ends on its own when the channel closes at
        // teardown, and `reset_path_health` clears the last verdict there.
        let mut health_rx = supervisor.path_health_rx();
        let path_health_task = tokio::spawn(async move {
            use warrenguard_transport::path_health::PathHealth;
            loop {
                let verdict = *health_rx.borrow_and_update();
                crate::android_jni::set_path_health(match verdict {
                    PathHealth::Healthy => crate::android_jni::PATH_HEALTH_HEALTHY,
                    PathHealth::DegradedLarge => crate::android_jni::PATH_HEALTH_DEGRADED_LARGE,
                    PathHealth::DegradedBoth => crate::android_jni::PATH_HEALTH_DEGRADED_BOTH,
                });
                if health_rx.changed().await.is_err() {
                    break;
                }
            }
        });
        let _path_health_guard = PathHealthTaskGuard(path_health_task);
        // "Reduced MTU" sampler: the same post-connect probe as the desktop
        // daemon, so the chip means the same thing on both clients (the path
        // measured low, never the user's own MTU setting).
        let _effective_mtu_guard =
            spawn_effective_mtu_sampler(sessions.clone(), config.mtu.unwrap_or(DEFAULT_TUN_MTU));
        // Migration watchdog: a Wi-Fi to cellular handover rebinds the live
        // QUIC endpoint (VpnService.protect re-applied to the fresh socket
        // before quinn can send on it) and revalidates the path in about one
        // RTT, instead of the full teardown + re-handshake Kotlin used to run.
        // Fed by `WarrenJni.notifyNetworkChanged()`; when nothing recovers it
        // latches `escalated`, which ends the session so the Kotlin fail-closed
        // policy takes over.
        let (escalated_tx, escalated) = tokio::sync::watch::channel(false);
        let mut watchdog_io = crate::migration::AndroidMigrationIo::new(
            crate::migration::install_path_change_channel(),
            sessions.clone(),
            supervisor.handle(),
            escalated_tx,
        );
        // Stops feeding a watchdog whose session is gone: the closed source is
        // how `run_watchdog` exits on its own.
        let _path_channel = crate::migration::PathChannelGuard;
        // In-tunnel egress probe: the supervisor's dead-path watches and the
        // goodput prober both stay green on an exit that answers the client and
        // forwards nothing to the internet, so this is the only guard that sees
        // that class. Its verdict reaches the UI through `getPathHealth`, and
        // its escalation ends the session so Kotlin redials onto a fresh
        // circuit instead of leaving the user on a dead exit.
        let (egress_dead_tx, egress_dead) = tokio::sync::watch::channel(false);
        let _egress_probe = spawn_egress_probe(sessions.clone(), egress_dead_tx);
        let inputs = SupervisedInputs {
            sessions,
            fatal: supervisor.fatal_rx(),
            datapath_dead: supervisor.datapath_dead_rx(),
            assigns: ip_assign_channel.subscribe(),
            escalated,
            egress_dead,
        };
        // The supervisor holds the only remaining sender once this is dropped,
        // so the driver's watch closes with it at teardown.
        drop(ip_assign_channel);
        let _supervisor_task = AbortOnDrop::spawn(async move {
            if let Err(e) = supervisor.run().await {
                log::warn!("multi-hop supervisor terminated: {e}");
            }
        });
        // Aborted with the attempt: the handle it holds subscribes a watch
        // receiver, which would otherwise keep the supervisor from seeing that
        // nobody reads the tunnel any more.
        let _watchdog = AbortOnDrop::spawn(async move {
            warrenguard_transport::migration_watchdog::run_watchdog(&mut watchdog_io).await;
            log::debug!("multi-hop migration watchdog terminated");
        });

        // NAT-PMP over multi-hop: bind the refresh socket to LOCAL_TUN_IPV4 so
        // the request routes through the tunnel (RemapTun rewrites it to the
        // assigned source) to the exit gateway. Guard lives for this attempt.
        let _nat_pmp_guard = maybe_spawn_nat_pmp(&config, LOCAL_TUN_IPV4);

        // The Android VpnService TUN is fixed on LOCAL_TUN_IPV4/IPV6 (set in
        // Kotlin and immutable after establish()), so wrap it in a 1:1 NAT that
        // presents the exit-assigned source on uplink and restores the local
        // address on downlink. Built from the exit's first assignment; the v6
        // remap is active only when the exit granted v6 (else v6 stays
        // blackholed). Android equivalent of the desktop RealTun::reassign_*.
        let remap = |spec: warrenguard_transport::IpAssignSpec| {
            // No-logs: the exit-assigned inner IPs are user-linkable; report the
            // admission mode only.
            log::info!(
                "multi-hop tunnel up (v7={})",
                presented.load(Ordering::Relaxed)
            );
            let v6_pair = spec.assigned_v6.map(|a| (LOCAL_TUN_IPV6, a));
            crate::remap_tun::RemapTun::new(tun.clone(), LOCAL_TUN_IPV4, spec.assigned, v6_pair)
        };
        let daita = daita.clone();
        let outcome = match max_rate_bps {
            Some(bps) => tokio::select! {
                _ = &mut cancel_rx => None,
                end = crate::supervised_session::run_supervised(
                    inputs,
                    |spec| crate::rate_limited_tun::RateLimitedTun::new(remap(spec), bps),
                    daita,
                    status,
                    crate::supervised_session::SESSION_GRACE,
                ) => Some(end),
            },
            None => tokio::select! {
                _ = &mut cancel_rx => None,
                end = crate::supervised_session::run_supervised(
                    inputs,
                    remap,
                    daita,
                    status,
                    crate::supervised_session::SESSION_GRACE,
                ) => Some(end),
            },
        };

        match outcome {
            None => {
                log::info!("multi-hop tunnel cancelled by Kotlin");
                break;
            }
            // A rejected v7 token (spent serial, clock-skewed epoch) is a
            // verdict on the TOKEN, not the subscription, and Unauthorized is
            // terminal for Kotlin. Re-verify on the wallet-signed path before
            // surfacing anything.
            Some(SessionEnd::Rejected(RejectionReason::NotAllowlisted))
                if token_provider.is_some() && presented.load(Ordering::Relaxed) =>
            {
                log::warn!("multi-hop: exit rejected the v7 token; retrying wallet-signed");
                token_provider = None;
            }
            // The exit policy-rejected the wallet-signed setup (not
            // authorized): surface a distinct Unauthorized status so Kotlin
            // shows "subscription expired" and stops retrying, instead of a
            // generic disconnect + reconnect storm that keeps hitting the
            // same rejection.
            Some(SessionEnd::Rejected(RejectionReason::NotAllowlisted)) => {
                log::warn!("multi-hop: exit rejected setup (not authorized / subscription lapsed)");
                status.store(SessionStatus::Unauthorized as i32);
                return;
            }
            Some(end) => {
                // No-logs: `SessionEnd` carries only a failure category (and,
                // for a ban, the exit's opaque product code), never identity.
                log::info!("multi-hop session ended: {end:?}");
                break;
            }
        }
    }

    status.store(SessionStatus::Disconnected as i32);
}

/// Spawn the in-tunnel egress probe for one connect attempt.
///
/// `None` when `WARREN_EGRESS_PROBE=0` disables it. The guard aborts the task
/// with the attempt, which drops the session watch receiver it holds (a
/// leftover receiver would keep the supervisor dialing for a tunnel nobody
/// reads).
fn spawn_egress_probe(
    sessions: warrenguard_transport::supervisor::ClientWatch,
    escalate: tokio::sync::watch::Sender<bool>,
) -> Option<crate::supervised_session::AbortOnDrop> {
    use warrenguard_transport::egress_probe::{EgressProbeConfig, run_egress_probe};

    let cfg = EgressProbeConfig::from_env();
    if !cfg.enabled {
        log::info!("multi-hop: in-tunnel egress probe disabled by environment");
        return None;
    }
    // No-logs: cadence and threshold only, nothing that identifies the circuit.
    log::info!(
        "multi-hop: in-tunnel egress probe armed (startup {}s, steady {}s, {} failures)",
        cfg.startup_interval.as_secs(),
        cfg.interval.as_secs(),
        cfg.failure_threshold
    );
    let mut io = crate::egress_probe::AndroidEgressProbeIo {
        interval: cfg.interval,
        startup_interval: cfg.startup_interval,
        sessions: Some(sessions),
        verdict: Some(std::sync::Arc::new(crate::android_jni::set_egress_dead)),
        escalate: Some(escalate),
        // The production in-tunnel DNS probe against the exit resolver.
        probe: None,
        // Production reads the ACK counter off the session watch above.
        acks: None,
        acks_at_streak_start: None,
        // Same: the real downlink packet count comes off the live bundle. Only
        // a test scripts it.
        real_rx: None,
        real_rx_at_streak_start: None,
    };
    Some(crate::supervised_session::AbortOnDrop::spawn(async move {
        run_egress_probe(&mut io, cfg.failure_threshold).await;
        log::debug!("multi-hop egress probe terminated");
    }))
}

/// Build the DAITA state for the multi-hop pump from the client config.
///
/// Returns [`DaitaState::disabled`] when DAITA is off or the requested padding
/// machine is not in the curated pool (logged loudly with the valid names).
/// The exit accepts and emits the `0xFF` dummy frames on the multi-hop path
/// (`warren_exit_core::multihop` drops uplink dummies and pads the downlink),
/// so a client-built state is a real defense, not just local noise.
fn build_multihop_daita_state(config: &WarrenTunnelConfig) -> DaitaState {
    let Some(daita) = config.daita.as_ref() else {
        return DaitaState::disabled();
    };
    let machine = daita
        .get("padding_machine")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tamaraw");
    let pool = DaitaPool::default_pool();
    let mut rng = rand_v9::rngs::StdRng::from_os_rng();
    match pool.pick_named(machine, &mut rng) {
        Some(cfg) => match DaitaState::from_config(&cfg, Instant::now()) {
            Ok(state) => {
                log::info!("multi-hop DAITA enabled (machine={machine})");
                state
            }
            Err(e) => {
                log::error!("multi-hop DAITA state init failed: {e}; plain pump");
                DaitaState::disabled()
            }
        },
        None => {
            log::error!(
                "multi-hop: unknown DAITA machine {machine:?} (valid: {:?}); plain pump",
                pool.entry_names()
            );
            DaitaState::disabled()
        }
    }
}

/// Parse the JSON config blob handed in by the Kotlin caller.
pub fn parse_config(json: &str) -> Result<WarrenTunnelConfig, TunnelStartError> {
    serde_json::from_str(json).map_err(TunnelStartError::InvalidConfig)
}

/// Guard returned by [`maybe_spawn_nat_pmp`]. Drops the NAT-PMP
/// refresh loop AND aborts the event-drain task on drop. Matches the
/// daemon-side `NatPmpManager` pattern (which stores both handles).
/// `None` from [`maybe_spawn_nat_pmp`] means NAT-PMP was disabled in
/// config.
/// Stops the path-health publisher with the session and clears the last
/// verdict, so a degraded value can never colour a disconnected UI or the
/// first moments of the next session.
/// The Kotlin-side default of `WarrenTunnelConfig.mtu`
/// (`WarrenLocalSettingsRepository.MTU_MAX`), for a payload that omits it.
const DEFAULT_TUN_MTU: u16 = 1280;

/// Sample the live bundle's usable inner payload against the TUN MTU and
/// publish the "Reduced MTU" verdict for Kotlin.
///
/// DPLPMTUD needs a few RTTs to settle, so the first sample waits 4 s after
/// the dial (sampling at once would flag every network as reduced); after
/// that a 5 s cadence, republished only when the quantized verdict moves.
/// Mirrors the desktop daemon's sampler in `talpid-warren-tunnel`. Ends on its
/// own when the supervisor closes the session watch; the guard aborts it with
/// the attempt and clears the verdict.
fn spawn_effective_mtu_sampler(
    mut sessions: warrenguard_transport::supervisor::ClientWatch,
    tun_mtu: u16,
) -> EffectiveMtuGuard {
    EffectiveMtuGuard(tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        loop {
            let bundle = sessions.borrow().clone();
            if let Some(bundle) = bundle {
                crate::android_jni::set_effective_mtu(crate::path_metrics::reduced_mtu_verdict(
                    bundle.max_inner_payload(),
                    tun_mtu,
                ));
            }
            tokio::select! {
                () = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                res = sessions.changed() => {
                    if res.is_err() {
                        break;
                    }
                }
            }
        }
    }))
}

struct EffectiveMtuGuard(tokio::task::JoinHandle<()>);

impl Drop for EffectiveMtuGuard {
    fn drop(&mut self) {
        self.0.abort();
        crate::android_jni::reset_effective_mtu();
    }
}

struct PathHealthTaskGuard(tokio::task::JoinHandle<()>);

impl Drop for PathHealthTaskGuard {
    fn drop(&mut self) {
        self.0.abort();
        crate::android_jni::reset_path_health();
    }
}

struct NatPmpGuard {
    refresh: warrenguard_natpmp_client::RefreshLoopHandle,
    drain: tokio::task::JoinHandle<()>,
}

impl Drop for NatPmpGuard {
    fn drop(&mut self) {
        // Clear the published status so a stale "mapped" port does not
        // linger in the UI after the mapping is torn down.
        crate::android_jni::reset_natpmp_status();
        self.refresh.cancel();
        // Abort eagerly: the refresh-loop cancel closes the sender
        // and the drain would exit naturally on the next `recv`, but
        // an explicit abort removes the window where Kotlin observes
        // `disconnectTunnel` complete while the drain task is still
        // alive on the runtime.
        self.drain.abort();
    }
}

/// Last NAT-PMP external port granted by an exit in this process, so an
/// auto-mode forward keeps the same public port when the user changes exit:
/// the next session re-suggests it (the port "follows" the client). Mirrors
/// the desktop daemon's sticky map. `0` means nothing remembered yet.
/// Updated on every `Mapped`/`Renewed`; read when spawning a session.
static LAST_GRANTED_NATPMP_EXTERNAL_PORT: AtomicU16 = AtomicU16::new(0);

/// Transport the [`LAST_GRANTED_NATPMP_EXTERNAL_PORT`] was granted for
/// (`true` = TCP). The follow only re-suggests the remembered port when the
/// new session uses the same transport, so a UDP-to-TCP switch does not
/// collide with the client's own still-leased UDP mapping on that port.
static LAST_GRANTED_NATPMP_IS_TCP: AtomicBool = AtomicBool::new(false);

fn maybe_spawn_nat_pmp(
    config: &WarrenTunnelConfig,
    bind_ipv4: std::net::Ipv4Addr,
) -> Option<NatPmpGuard> {
    if !config.nat_pmp_enabled.unwrap_or(false) {
        return None;
    }
    let server = warrenguard_natpmp_client::default_server_addr();
    // Bind the refresh socket to the TUN's own inner IPv4 so the request
    // egresses through the tunnel (to the exit gateway 10.66.0.1:5351), not the
    // underlying Wi-Fi/mobile interface. On the multi-hop path this is
    // LOCAL_TUN_IPV4 (the address the Android interface actually holds); the
    // RemapTun then rewrites it to the exit-assigned source, same as data.
    let bind_addr = std::net::IpAddr::V4(bind_ipv4);
    // Map the client-selected protocol; default to UDP for unknown values.
    let proto = match config.nat_pmp_protocol.as_deref() {
        Some("tcp") | Some("TCP") => warrenguard_natpmp_client::MapProtocol::Tcp,
        _ => warrenguard_natpmp_client::MapProtocol::Udp,
    };
    let is_tcp = matches!(proto, warrenguard_natpmp_client::MapProtocol::Tcp);
    // The internal port is informative only on the PCP/NAT-PMP wire; the
    // client does not own a specific local port, so request 0. The
    // suggested external port is the user's pin, or (auto) the last port an
    // exit granted us this process so it follows the client across an exit
    // change (only when the transport matches; see the static's doc).
    let user_pin = config.nat_pmp_external_port.unwrap_or(0);
    let suggested_external_port = crate::natpmp_follow::effective_natpmp_suggested(
        user_pin,
        is_tcp,
        LAST_GRANTED_NATPMP_EXTERNAL_PORT.load(Ordering::Relaxed),
        LAST_GRANTED_NATPMP_IS_TCP.load(Ordering::Relaxed),
    );
    // A user-pinned port is honour-or-error; the carried-over follow is
    // best-effort and downgrades to a server pick on conflict.
    let suggestion = if user_pin != 0 {
        warrenguard_natpmp_client::SuggestionKind::Pinned
    } else {
        warrenguard_natpmp_client::SuggestionKind::Sticky
    };
    let lifetime_secs = config.nat_pmp_lifetime_secs.unwrap_or(3600);
    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<warrenguard_natpmp_client::NatPmpEvent>();
    let refresh = warrenguard_natpmp_client::spawn_refresh_loop_from_addr(
        server,
        proto,
        0,
        suggested_external_port,
        lifetime_secs,
        suggestion,
        tx,
        Some(bind_addr),
    );
    // Publish the initial "requesting" state so Kotlin shows progress
    // immediately after the toggle takes effect.
    crate::android_jni::set_natpmp_status(r#"{"state":"requesting"}"#.to_owned());
    let drain = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            log::info!("NAT-PMP event from Android tunnel: {event:?}");
            // Remember the granted external port so the next session (after
            // an exit change) re-suggests it: the public port follows the
            // client. Only a grant carries a port; 0 is never granted.
            if let warrenguard_natpmp_client::NatPmpEvent::Mapped { external_port, .. }
            | warrenguard_natpmp_client::NatPmpEvent::Renewed { external_port, .. } = &event
                && *external_port != 0
            {
                LAST_GRANTED_NATPMP_EXTERNAL_PORT.store(*external_port, Ordering::Relaxed);
                LAST_GRANTED_NATPMP_IS_TCP.store(is_tcp, Ordering::Relaxed);
            }
            if let Some(json) = natpmp_event_json(&event) {
                crate::android_jni::set_natpmp_status(json);
            }
        }
    });
    log::info!("NAT-PMP refresh loop spawned (server={server}, bind_addr={bind_addr})");
    Some(NatPmpGuard { refresh, drain })
}

/// Project a [`warrenguard_natpmp_client::NatPmpEvent`] to the JSON status
/// shape polled by Kotlin's `getNatPmpStatus()`. Returns `None` for
/// events that do not change the user-visible state (e.g. `Cancelled`,
/// where teardown already resets the status). The `Failed` variant only
/// surfaces the stable `reason` category, never the raw error string, so
/// no diagnostic detail leaks across the JNI boundary.
fn natpmp_event_json(event: &warrenguard_natpmp_client::NatPmpEvent) -> Option<String> {
    use warrenguard_natpmp_client::NatPmpEvent;
    let json = match event {
        NatPmpEvent::Mapped {
            external_port,
            lifetime_secs,
            ..
        }
        | NatPmpEvent::Renewed {
            external_port,
            lifetime_secs,
            ..
        } => serde_json::json!({
            "state": "mapped",
            "external_port": external_port,
            "lifetime_secs": lifetime_secs,
        }),
        NatPmpEvent::RateLimited { retry_after_secs } => serde_json::json!({
            "state": "rate_limited",
            "retry_after_secs": retry_after_secs,
        }),
        NatPmpEvent::Failed { reason, .. } => serde_json::json!({
            "state": "failed",
            "reason": format!("{reason:?}"),
        }),
        _ => return None,
    };
    Some(json.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_config;

    // The exact JSON the Kotlin `WarrenTunnelConfig.toWireJson()` produces for
    // a fully-populated config. Kept in sync with
    // `WarrenTunnelConfigSerializationTest` on the Android side: both pin the
    // same wire contract so a `@SerialName` / serde field-name drift fails
    // loudly on at least one side (neither uses `deny_unknown_fields`).
    const FULL_WIRE_JSON: &str = r#"{
        "exit_pubkey_hex": "abababababababababababababababababababababababababababababababab",
        "exit_endpoint": "1.2.3.4:443",
        "wallet_pubkey_hex": "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB",
        "entry_hop": {
            "relay_pubkey_hex": "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
            "relay_endpoint": "5.6.7.8:443"
        },
        "multihop_two_hop": true,
        "daita": {"padding_machine": "tamaraw", "normalize_packets": false},
        "nat_pmp_enabled": true,
        "nat_pmp_protocol": "tcp",
        "nat_pmp_external_port": 51820,
        "nat_pmp_lifetime_secs": 21600,
        "enable_ipv6": true,
        "lockdown_mode": true,
        "dns": {
            "state": "custom",
            "custom_servers": ["9.9.9.9"],
            "block_ads": true,
            "block_trackers": true,
            "block_malware": true,
            "block_adult_content": true,
            "block_gambling": true,
            "block_social_media": true
        },
        "allow_lan": true,
        "mtu": 1200
    }"#;

    #[test]
    fn parses_full_kotlin_wire_payload() {
        let cfg = parse_config(FULL_WIRE_JSON).expect("full payload must parse");
        assert_eq!(
            cfg.exit_pubkey_hex,
            "abababababababababababababababababababababababababababababababab"
        );
        assert_eq!(cfg.exit_endpoint, "1.2.3.4:443");
        assert!(cfg.entry_hop.is_some());
        assert_eq!(cfg.multihop_two_hop, Some(true));
        assert!(cfg.daita.is_some());
        assert_eq!(cfg.nat_pmp_enabled, Some(true));
        assert_eq!(cfg.nat_pmp_protocol.as_deref(), Some("tcp"));
        assert_eq!(cfg.nat_pmp_external_port, Some(51820));
        assert_eq!(cfg.nat_pmp_lifetime_secs, Some(21600));
        assert_eq!(cfg.enable_ipv6, Some(true));
        assert_eq!(cfg.lockdown_mode, Some(true));
        assert!(cfg.dns.is_some());
    }

    #[test]
    fn android_only_keys_are_accepted_and_ignored() {
        // `allow_lan` and `mtu` are enforced Android-side and absent from the
        // Rust struct; parsing must not fail on them (no deny_unknown_fields).
        let json =
            r#"{"exit_pubkey_hex":"ab","exit_endpoint":"1.2.3.4:443","allow_lan":true,"mtu":1200}"#;
        assert!(parse_config(json).is_ok());
    }

    #[test]
    fn minimal_payload_defaults_optional_fields() {
        let json = r#"{"exit_pubkey_hex":"ab","exit_endpoint":"1.2.3.4:443"}"#;
        let cfg = parse_config(json).expect("minimal payload must parse");
        assert!(cfg.entry_hop.is_none());
        assert!(cfg.daita.is_none());
        assert_eq!(cfg.nat_pmp_enabled, None);
        assert_eq!(cfg.nat_pmp_protocol, None);
        // Absent selector defaults to None, which run_multi_hop_session reads
        // as 2-hop: an older payload keeps the shipping topology.
        assert_eq!(cfg.multihop_two_hop, None);
    }

    #[test]
    fn single_hop_payload_sets_multihop_two_hop_false() {
        // The Kotlin builder emits `multihop_two_hop: false` when the user
        // turns multi-hop OFF; it still ships entry_hop + the directory so the
        // 1-hop circuit rides the multi-hop wire.
        let json = r#"{
            "exit_pubkey_hex":"ab","exit_endpoint":"1.2.3.4:443",
            "entry_hop":{},"multihop_two_hop":false
        }"#;
        let cfg = parse_config(json).expect("single-hop payload must parse");
        assert_eq!(cfg.multihop_two_hop, Some(false));
        assert!(cfg.entry_hop.is_some());
    }

    #[test]
    fn the_tun_mtu_is_read_off_the_kotlin_payload() {
        let cfg = parse_config(FULL_WIRE_JSON).expect("full payload must parse");
        assert_eq!(cfg.mtu, Some(1200));
        let minimal = parse_config(r#"{"exit_pubkey_hex":"ab","exit_endpoint":"1.2.3.4:443"}"#)
            .expect("minimal payload must parse");
        assert_eq!(minimal.mtu, None);
    }

    #[test]
    fn missing_required_field_is_an_error() {
        // Drop `exit_pubkey_hex`: serde must reject (the field has no default).
        let json = r#"{"exit_endpoint":"1.2.3.4:443"}"#;
        assert!(parse_config(json).is_err());
    }
}
