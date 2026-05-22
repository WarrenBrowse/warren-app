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

use std::net::SocketAddr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use serde::Deserialize;
use tokio::sync::oneshot;
use warren_protocol::{WarrenExitAddr, WarrenPubkey};
use warren_tunnel::{
    AndroidTun, ClientTunnel, DaitaState, pump_bidirectional, pump_bidirectional_with_daita,
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
    #[expect(dead_code, reason = "entry-hop wiring tracked under D.4 multi-hop scope")]
    pub entry_hop: Option<serde_json::Value>,
    /// Client-side toggle: when present we opt the handshake into DAITA via
    /// `ClientTunnel::with_daita(true)`. The exit decides whether to honour
    /// the request by shipping a `SetupAck::daita_spec`; if it does, we
    /// instantiate `DaitaState` from it and switch to
    /// `pump_bidirectional_with_daita`. If the exit declines (no pool
    /// configured) we fall back to a plain pump - no error.
    pub daita: Option<serde_json::Value>,
    #[expect(dead_code, reason = "Android does not surface bypass-CIDR routing yet (D.4 follow-up)")]
    pub bypass_cidrs: Option<Vec<String>>,
    /// Client opt-in for the NAT-PMP refresh loop. When `Some(true)`
    /// the session spawns a `warren_natpmp_client::spawn_refresh_loop_from_addr`
    /// after the handshake completes, binding the UDP socket to the
    /// assigned tunnel inner IPv4 so the request egresses through the
    /// tunnel (and not the underlying mobile-data / Wi-Fi interface).
    /// Assigned external port surfacing to Kotlin is a follow-up; for
    /// now we log it at INFO.
    pub nat_pmp_enabled: Option<bool>,
    #[expect(
        dead_code,
        reason = "M4.0 obfuscation lives at the QUIC transport layer (warren-tunnel transport_config); \
                  the client-side toggle is currently configured at warren-exit deploy time, not per-session"
    )]
    pub obfuscation_m40: Option<bool>,
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
pub async fn run_session(
    tun: AndroidTun,
    mnemonic: String,
    config: WarrenTunnelConfig,
    status: &'static AtomicI32,
    cancel_rx: oneshot::Receiver<()>,
) {
    status.store(SessionStatus::Connecting as i32, Ordering::SeqCst);

    let signing = match crate::wallet::signing_key_from_mnemonic(&mnemonic) {
        Ok(k) => k,
        Err(e) => {
            log::error!("wallet key derive failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

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
        exit_socket, config.exit_pubkey_hex, daita_requested
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

/// Parse the JSON config blob handed in by the Kotlin caller.
pub fn parse_config(json: &str) -> Result<WarrenTunnelConfig, TunnelStartError> {
    serde_json::from_str(json).map_err(TunnelStartError::InvalidConfig)
}

/// Guard returned by [`maybe_spawn_nat_pmp`]. Drops the NAT-PMP
/// refresh loop on drop (via the `RefreshLoopHandle`'s drop impl).
/// `None` means NAT-PMP was disabled in config or unsupported on the
/// assigned interface (IPv6-only is not yet wired).
struct NatPmpGuard {
    #[expect(dead_code, reason = "handle dropped on session teardown; field is the lifetime owner")]
    handle: warren_natpmp_client::RefreshLoopHandle,
}

fn maybe_spawn_nat_pmp(
    config: &WarrenTunnelConfig,
    assigned_ipv4: std::net::Ipv4Addr,
) -> Option<NatPmpGuard> {
    if !config.nat_pmp_enabled.unwrap_or(false) {
        return None;
    }
    let server = warren_natpmp_client::default_server_addr();
    let bind_addr = std::net::IpAddr::V4(assigned_ipv4);
    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<warren_natpmp_client::NatPmpEvent>();
    let handle = warren_natpmp_client::spawn_refresh_loop_from_addr(
        server,
        warren_natpmp_client::MapProtocol::Udp,
        // D.6 placeholder: request external mapping for the QUIC
        // datagram port. The internal port is informative only on the
        // PCP/NAT-PMP wire (the exit allocates an arbitrary external
        // port). Use 0 as "client does not own a specific local port"
        // and let the exit pick.
        0,
        0,
        3600,
        tx,
        Some(bind_addr),
    );
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            log::info!("NAT-PMP event from Android tunnel: {event:?}");
        }
    });
    log::info!(
        "NAT-PMP refresh loop spawned (server={server}, bind_addr={bind_addr})"
    );
    Some(NatPmpGuard { handle })
}
