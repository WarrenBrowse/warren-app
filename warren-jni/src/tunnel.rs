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

use serde::Deserialize;
use tokio::sync::oneshot;
use warren_protocol::{WarrenExitAddr, WarrenPubkey};
use warren_tunnel::{AndroidTun, ClientTunnel, pump_bidirectional};

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
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
    pub entry_hop: Option<serde_json::Value>,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
    pub daita: Option<serde_json::Value>,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
    pub bypass_cidrs: Option<Vec<String>>,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
    pub nat_pmp_enabled: Option<bool>,
    #[expect(dead_code, reason = "config field for D.4 step 3+ wiring")]
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
    let client = ClientTunnel::with_signing_key(&signing);

    log::info!("Quinn connect: {} via {}", exit_socket, config.exit_pubkey_hex);
    let session = match client.connect(target).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Quinn connect failed: {e}");
            status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
            return;
        }
    };

    log::info!(
        "Tunnel up: assigned ipv4={} ipv6={:?} mtu={}",
        session.assigned_ipv4(),
        session.assigned_ipv6(),
        session.assigned_max_mtu()
    );
    status.store(SessionStatus::Connected as i32, Ordering::SeqCst);

    let conn = session.clone_conn();
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

    status.store(SessionStatus::Disconnected as i32, Ordering::SeqCst);
}

/// Parse the JSON config blob handed in by the Kotlin caller.
pub fn parse_config(json: &str) -> Result<WarrenTunnelConfig, TunnelStartError> {
    serde_json::from_str(json).map_err(TunnelStartError::InvalidConfig)
}
