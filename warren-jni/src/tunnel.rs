// Warren Quinn tunnel session bootstrap.
//
// Spawned from `Java_..._connectTunnel`. Owns the async lifecycle:
//
//   1. Derives the wallet `SigningKey` from the supplied mnemonic.
//   2. Builds a [`warren_protocol::WarrenExitAddr`] from the JSON config
//      (exit pubkey hex + UDP endpoint).
//   3. Calls `ClientTunnel::with_signing_key(...).connect(addr).await`,
//      which performs the QUIC dial + `Setup` / `SetupAck` handshake.
//   4. Spawns `pump_bidirectional(tun, conn)` to wire IP packets between
//      the Android TUN fd and the Quinn datagram channel until either
//      side fails or the cancel oneshot fires.
//
// All work happens on the shared [`crate::android_jni::RUNTIME`] tokio
// runtime so the JNI entry point can return synchronously while the
// session lives on. Status transitions surface through [`SessionStatus`]
// behind an atomic so [`Java_..._getTunnelStatus`] can poll them without
// further locking.

#![cfg(all(target_os = "android", feature = "tunnel"))]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicI32, Ordering};
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
use warren_protocol::{WarrenExitAddr, WarrenPubkey};
use warren_tunnel::{
    AndroidTun, ClientTunnel, DaitaPool, DaitaState, pump_bidirectional,
    pump_bidirectional_with_daita,
};

/// JSON config parsed from the Kotlin side. Field names mirror
/// `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt`.
/// Optional fields (multi-hop entry, DAITA spec, etc.) are accepted but not
/// wired yet - they are placeholders for D.4 step 3+ surface alignment.
#[derive(Debug, Deserialize)]
pub struct WarrenTunnelConfig {
    pub exit_pubkey_hex: String,
    pub exit_endpoint: String,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
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
    /// Client-side toggle: when present we opt the handshake into DAITA via
    /// `ClientTunnel::with_daita(true)`. The exit decides whether to honour
    /// the request by shipping a `SetupAck::daita_spec`; if it does, we
    /// instantiate `DaitaState` from it and switch to
    /// `pump_bidirectional_with_daita`. If the exit declines (no pool
    /// configured) we fall back to a plain pump - no error.
    pub daita: Option<serde_json::Value>,
    /// Client opt-in for the NAT-PMP refresh loop. When `Some(true)`
    /// the session spawns a `warren_natpmp_client::spawn_refresh_loop_from_addr`
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "IPv6 routing enforced Android-side; client-side filtering is a follow-up"
        )
    )]
    pub enable_ipv6: Option<bool>,
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
    /// DNS options. DNS routing into the tunnel is enforced Android-side via
    /// `VpnService.Builder.addDnsServer`; content-blocking flags are honoured
    /// by the exit DNS forwarder. Accepted here for schema parity.
    #[serde(default)]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "DNS routing enforced Android-side; exit-side blocking is a follow-up"
        )
    )]
    pub dns: Option<serde_json::Value>,
}

/// Tunnel session status reported back to Kotlin via
/// `WarrenJni.getTunnelStatus()`. Encoded as an `i32` rather than an enum
/// to match the existing JNI int contract.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
    Reconnecting = 3,
}

/// Errors surfaced from the synchronous JNI entry path. Currently only
/// `JsonParse` actually fires - mnemonic / pubkey / endpoint failures
/// happen inside the spawned `run_session` task and log there. The other
/// variants are kept to document the surface the caller should eventually
/// branch on once a richer error-channel makes it back across JNI.
///
/// The `clippy::enum_variant_names` allow exists because the "what failed"
/// narration is the variant's purpose; renaming each to drop the common
/// `Mnemonic` / `Config` / `ExitPubkey` etc. suffixes would lose that.
#[expect(dead_code, reason = "richer error reporting wired in D.4 step 3")]
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
/// `status` atomic is updated as the session progresses; the `cancel_rx`
/// oneshot is selected against the pump so a `WarrenJni.disconnectTunnel()`
/// call from Kotlin terminates the task promptly. `status` is held as a
/// `'static` reference because the JNI side parks the underlying atomic
/// in a `static AtomicI32`; this keeps `run_session` `'static`-bounded
/// for `runtime.spawn(...)` without an extra `Arc` allocation.
///
/// Fix L-2: the function now accepts a pre-derived `signing_key` instead of
/// a raw mnemonic string.  Key derivation happens at the JNI boundary
/// (synchronously, before this task is spawned) and the mnemonic is
/// zeroized there - it never crosses into the async lifetime.
pub async fn run_session(
    tun: AndroidTun,
    signing_key: SigningKey,
    config: WarrenTunnelConfig,
    status: &'static AtomicI32,
    cancel_rx: oneshot::Receiver<()>,
) {
    status.store(SessionStatus::Connecting as i32, Ordering::SeqCst);

    let signing = signing_key;

    // Multi-hop path: when the client opted into a separate entry relay we
    // route through `warren_client::MultiHopClient` (HPKE to the exit via the
    // entry relay). Fail closed - never silently downgrade an opted-in
    // multi-hop request to single-hop, that would be a privacy downgrade.
    //
    // NOTE: this path is wired and compiles, but the Kotlin builder does not
    // populate `entry_hop` yet (single-hop only ships today). Activate it by
    // setting `entryHop` in `WarrenTunnelConfigBuilder` AFTER on-device
    // verification of the multi-hop data plane.
    if config.entry_hop.is_some() {
        run_multi_hop_session(tun, signing, config, status, cancel_rx).await;
        return;
    }

    let exit_pubkey = match WarrenPubkey::from_hex(&config.exit_pubkey_hex) {
        Ok(p) => p,
        Err(e) => {
            log::error!("invalid exit pubkey: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    let exit_socket: SocketAddr = match config.exit_endpoint.parse() {
        Ok(s) => s,
        Err(e) => {
            log::error!("invalid exit endpoint {:?}: {e}", config.exit_endpoint);
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    let target = WarrenExitAddr::new(exit_pubkey).with_ip_addr(exit_socket);
    let daita_requested = config.daita.is_some();
    let client = ClientTunnel::with_signing_key(&signing).with_daita(daita_requested);

    log::info!(
        "Quinn connect: {} via {} (daita_requested={})",
        exit_socket,
        config.exit_pubkey_hex,
        daita_requested
    );
    let session = match client.connect(target).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Quinn connect failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    log::info!(
        "Tunnel up: assigned ipv4={} ipv6={:?} mtu={} daita_spec={}",
        session.assigned_ipv4(),
        session.assigned_ipv6(),
        session.assigned_max_mtu(),
        session.daita_spec().is_some()
    );
    status.store(SessionStatus::Connected as i32, Ordering::SeqCst);

    // D.6 NAT-PMP wiring: if the client opted in, spawn the refresh
    // loop with the assigned tunnel inner IPv4 as bind addr so the
    // request egresses through the tunnel. The loop runs alongside
    // the bidirectional pump and is cancelled implicitly when the
    // session task exits (the cancel oneshot inside the manager fires
    // on drop). Currently the assigned external port surfaces only
    // via INFO log; piping it back to Kotlin is a future iteration.
    let _nat_pmp_guard = maybe_spawn_nat_pmp(&config, session.assigned_ipv4());

    // Build the DAITA state from the exit-supplied spec. If the client
    // requested DAITA but the exit declined (returned daita_spec=None),
    // we silently fall back to the plain pump - this matches the
    // warren-tunnel contract.
    let daita_state = match session.daita_spec() {
        Some(spec) if daita_requested => match DaitaState::from_config(spec, Instant::now()) {
            Ok(state) => {
                log::info!(
                    "DAITA framework built from exit spec (machines={})",
                    spec.machine_specs.len()
                );
                Some(state)
            }
            Err(e) => {
                log::error!("DAITA state init failed: {e}; falling back to plain pump");
                None
            }
        },
        _ => None,
    };

    let conn = session.clone_conn();
    // Branch the select! body on whether DAITA is on so the future type
    // is single-arm per compile (avoids boxing or a `Pin<Box<dyn Future>>`).
    if let Some(state) = daita_state {
        tokio::select! {
            _ = cancel_rx => {
                log::info!("Tunnel cancelled by Kotlin");
            }
            result = pump_bidirectional_with_daita(tun, conn, state) => {
                if let Err(e) = result {
                    log::error!("pump_bidirectional_with_daita exited with error: {e}");
                } else {
                    log::info!("pump_bidirectional_with_daita exited cleanly");
                }
            }
        }
    } else {
        tokio::select! {
            _ = cancel_rx => {
                log::info!("Tunnel cancelled by Kotlin");
            }
            result = pump_bidirectional(tun, conn) => {
                if let Err(e) = result {
                    log::error!("pump_bidirectional exited with error: {e}");
                } else {
                    log::info!("pump_bidirectional exited cleanly");
                }
            }
        }
    }

    status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
}

/// Warren multi-hop directory root signing key (baked pin). Mirrors
/// `mullvad-daemon::warren_multi_hop_directory::WARREN_MULTIHOP_ROOT_PUBKEY_BAKED`.
/// The directory envelope must chain to this root in addition to the server pin.
const WARREN_MULTIHOP_ROOT_PUBKEY_HEX: &str =
    "33cd9279ad06d1ee884235e763b876fa70598094944bdcfb82375bd9aaa67b08";

/// Drive a multi-hop tunnel (entry relay -> exit) from start to teardown.
///
/// Mirrors the desktop reference (`talpid-warren-tunnel::start_multi_hop`) but
/// without the auto-reconnect supervisor: a single `MultiHopClient` for one
/// entry/exit pair, pumped against the Android TUN. Selection uses the signed
/// multi-hop directory (`GET /v1/multihop/directory`), the only source of the
/// signed relay descriptor, the exit X25519 HPKE key, and the operational
/// trust anchor (none of which the `/v1/exits` list carries).
async fn run_multi_hop_session(
    tun: AndroidTun,
    signing: SigningKey,
    config: WarrenTunnelConfig,
    status: &'static AtomicI32,
    cancel_rx: oneshot::Receiver<()>,
) {
    use std::sync::Arc;

    let want_exit = match WarrenPubkey::from_hex(&config.exit_pubkey_hex) {
        Ok(p) => p,
        Err(e) => {
            log::error!("multi-hop: invalid exit pubkey: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
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
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };
    let server_pins: Vec<&str> = crate::android_jni::PROD_SERVER_PUBKEY_HEX
        .into_iter()
        .collect();
    let dir = match warren_relay_selector::verify_multihop_directory_any(
        &raw,
        &server_pins,
        &[WARREN_MULTIHOP_ROOT_PUBKEY_HEX],
    ) {
        Ok(d) => d,
        Err(e) => {
            log::error!("multi-hop: directory verify failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    // Select the exit node (must match the chosen exit) and a distinct entry
    // node (the entry relay must differ from the exit's node).
    let exit_node = match dir
        .nodes
        .iter()
        .find(|n| WarrenPubkey::from_bytes(n.exit.exit_ed25519_pubkey) == want_exit)
    {
        Some(n) => n,
        None => {
            log::error!("multi-hop: chosen exit not in directory; failing closed");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };
    let entry_node = match dir
        .nodes
        .iter()
        .filter(|n| n.relay.relay_id != exit_node.relay.relay_id)
        .find(|n| {
            want_entry
                .as_ref()
                .is_none_or(|w| WarrenPubkey::from_bytes(n.relay.relay_ed25519_pubkey) == *w)
        })
        .or_else(|| {
            dir.nodes
                .iter()
                .find(|n| n.relay.relay_id != exit_node.relay.relay_id)
        }) {
        Some(n) => n,
        None => {
            log::error!("multi-hop: no distinct entry relay available; failing closed");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    let bind_addr: SocketAddr = "0.0.0.0:0".parse().expect("static bind addr");
    log::info!(
        "multi-hop connect: entry {} -> exit {} (daita_requested={})",
        entry_node.relay.endpoint,
        config.exit_pubkey_hex,
        config.daita.is_some(),
    );
    let mh = match warren_client::multi_hop::MultiHopClient::connect_with_warren_obfuscation(
        &entry_node.relay,
        exit_node.exit.exit_id,
        &exit_node.exit.exit_x25519_multihop_pubkey,
        &dir.operational_pubkey,
        &signing,
        bind_addr,
        /* enable_gso */ false,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("multi-hop connect failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    // HPKE session setup + IP negotiation over the control stream. Required
    // before the data plane carries packets (the exit allocates the inner IP).
    // The reply is the HPKE-sealed `IpAssign`; we MUST learn the allocated
    // IPv4 because the exit drops every uplink packet whose source is not that
    // address (anti-spoof gate in `warren_exit_core::multihop`).
    // Request a dual-stack v6 only when the user enabled IPv6 (the Kotlin TUN
    // plan assigns the local v6 address under the same flag). The exit MAY
    // still decline (ipv6=None in the reply); we then stay v4-only and v6 is
    // blackholed, never leaked.
    let wants_ipv6 = config.enable_ipv6.unwrap_or(false);
    let assign_reply = match mh.setup_over_stream(Some(&signing), wants_ipv6).await {
        Ok(reply) => reply,
        Err(e) => {
            log::error!("multi-hop setup_over_stream failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    // Decode the exit-allocated inner addresses. Fail closed if the IPv4 is
    // missing or malformed: without it the data plane would be silently
    // blackholed. The IPv6 is optional (the exit's capability echo).
    let (assigned_ipv4, assigned_ipv6) = match warren_multihop::try_decode_control(&assign_reply) {
        Ok(Some(warren_multihop::WarrenControlMessage::IpAssign { ipv4, ipv6, .. })) => {
            (Ipv4Addr::from(ipv4), ipv6.map(Ipv6Addr::from))
        }
        other => {
            log::error!("multi-hop: expected IpAssign reply, got {other:?}; failing closed");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    log::info!(
        "multi-hop tunnel up (assigned inner ipv4={assigned_ipv4} ipv6={assigned_ipv6:?})"
    );
    status.store(SessionStatus::Connected as i32, Ordering::SeqCst);

    // The Android VpnService TUN is fixed on LOCAL_TUN_IPV4/IPV6 (set in Kotlin
    // and immutable after establish()), so wrap it in a 1:1 NAT that presents
    // the exit-assigned source on uplink and restores the local address on
    // downlink. The v6 remap is active only when the exit granted v6 (else v6
    // stays blackholed). Android equivalent of the desktop RealTun::reassign_*.
    let v6_pair = assigned_ipv6.map(|a| (LOCAL_TUN_IPV6, a));
    let tun = crate::remap_tun::RemapTun::new(tun, LOCAL_TUN_IPV4, assigned_ipv4, v6_pair);

    // NAT-PMP over multi-hop: bind the refresh socket to LOCAL_TUN_IPV4 so the
    // request routes through the tunnel (RemapTun rewrites it to the assigned
    // source) to the exit gateway. Guard lives until this task returns.
    let _nat_pmp_guard = maybe_spawn_nat_pmp(&config, LOCAL_TUN_IPV4);

    // DAITA over multi-hop: build a local DaitaState from the curated pool
    // (the exit accepts + emits 0xFF dummy frames on the multi-hop path) and
    // run the DAITA-aware pump. The plain pump is used when DAITA is off or the
    // requested machine is unknown.
    let daita_state = build_multihop_daita_state(&config);
    let pump = Arc::new(mh);
    let pump_result = tokio::select! {
        _ = cancel_rx => {
            log::info!("multi-hop tunnel cancelled by Kotlin");
            Ok(())
        }
        result = warren_client::multi_hop_pump::pump_multi_hop_bidirectional_with_daita(
            pump, tun, daita_state,
        ) => result,
    };
    if let Err(e) = pump_result {
        log::error!("multi-hop pump exited with error: {e}");
    } else {
        log::info!("multi-hop pump exited cleanly");
    }

    status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
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
struct NatPmpGuard {
    refresh: warren_natpmp_client::RefreshLoopHandle,
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

fn maybe_spawn_nat_pmp(
    config: &WarrenTunnelConfig,
    bind_ipv4: std::net::Ipv4Addr,
) -> Option<NatPmpGuard> {
    if !config.nat_pmp_enabled.unwrap_or(false) {
        return None;
    }
    let server = warren_natpmp_client::default_server_addr();
    // Bind the refresh socket to the TUN's own inner IPv4 so the request
    // egresses through the tunnel (to the exit gateway 10.66.0.1:5351), not the
    // underlying Wi-Fi/mobile interface. On the multi-hop path this is
    // LOCAL_TUN_IPV4 (the address the Android interface actually holds); the
    // RemapTun then rewrites it to the exit-assigned source, same as data.
    let bind_addr = std::net::IpAddr::V4(bind_ipv4);
    // Map the client-selected protocol; default to UDP for unknown values.
    let proto = match config.nat_pmp_protocol.as_deref() {
        Some("tcp") | Some("TCP") => warren_natpmp_client::MapProtocol::Tcp,
        _ => warren_natpmp_client::MapProtocol::Udp,
    };
    // The internal port is informative only on the PCP/NAT-PMP wire; the
    // client does not own a specific local port, so request 0. The
    // suggested external port comes from the user (0 = gateway picks).
    let suggested_external_port = config.nat_pmp_external_port.unwrap_or(0);
    let lifetime_secs = config.nat_pmp_lifetime_secs.unwrap_or(3600);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<warren_natpmp_client::NatPmpEvent>();
    let refresh = warren_natpmp_client::spawn_refresh_loop_from_addr(
        server,
        proto,
        0,
        suggested_external_port,
        lifetime_secs,
        tx,
        Some(bind_addr),
    );
    // Publish the initial "requesting" state so Kotlin shows progress
    // immediately after the toggle takes effect.
    crate::android_jni::set_natpmp_status(r#"{"state":"requesting"}"#.to_owned());
    let drain = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            log::info!("NAT-PMP event from Android tunnel: {event:?}");
            if let Some(json) = natpmp_event_json(&event) {
                crate::android_jni::set_natpmp_status(json);
            }
        }
    });
    log::info!("NAT-PMP refresh loop spawned (server={server}, bind_addr={bind_addr})");
    Some(NatPmpGuard { refresh, drain })
}

/// Project a [`warren_natpmp_client::NatPmpEvent`] to the JSON status
/// shape polled by Kotlin's `getNatPmpStatus()`. Returns `None` for
/// events that do not change the user-visible state (e.g. `Cancelled`,
/// where teardown already resets the status). The `Failed` variant only
/// surfaces the stable `reason` category, never the raw error string, so
/// no diagnostic detail leaks across the JNI boundary.
fn natpmp_event_json(event: &warren_natpmp_client::NatPmpEvent) -> Option<String> {
    use warren_natpmp_client::NatPmpEvent;
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
    }

    #[test]
    fn missing_required_field_is_an_error() {
        // Drop `exit_pubkey_hex`: serde must reject (the field has no default).
        let json = r#"{"exit_endpoint":"1.2.3.4:443"}"#;
        assert!(parse_config(json).is_err());
    }
}
