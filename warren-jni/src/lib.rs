// Warren VPN Android JNI bridge.
//
// Replaces upstream `mullvad-jni`. Drops the `mullvad-daemon` dependency: on
// Android the tunnel lifecycle is owned by Kotlin (`WarrenVpnService` /
// `WarrenQuinnAdapter`), not by a full state-machine running in-process. The
// JNI surface is therefore a thin set of primitives:
//
//   - logging init (`initLogger`)
//   - BIP39 mnemonic generation / import via warren-identity
//   - canonical-request Ed25519 signing for `X-Warren-*` API auth
//   - Quinn tunnel start / stop driven by Kotlin once a TUN fd is granted
//   - relay-selector queries via warren-relay-selector
//   - optional NAT-PMP port-forwarding via warren-natpmp-client
//
// All JNI exports follow the
// `Java_com_warrenbrowse_vpn_jni_WarrenJni_<method>` naming convention
// dictated by the Kotlin `WarrenJni` companion object (`android/lib/...`).
//
// NOTE (D.3 scope): this file ships the crate skeleton + dependency wiring.
// The active-tunnel state machine + relay-selector glue land in D.4 / D.6
// per `.planning/session-d-d3-warren-jni-design.md`. The exports are
// best-effort stubs whose actual implementation is tracked there.

#![cfg(target_os = "android")]

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use jnix::{
    FromJava, JnixEnv,
    jni::{
        JNIEnv,
        objects::{JClass, JObject, JString},
        sys::{jbyteArray, jint, jstring},
    },
};
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// Error / runtime state
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize logging: {0}")]
    InitializeLogging(String),

    #[error("Failed to create Tokio runtime")]
    InitTokio(#[source] std::io::Error),

    #[error("Warren identity: {0}")]
    Identity(String),

    #[error("Warren tunnel: {0}")]
    Tunnel(String),

    #[error("Tunnel already running")]
    TunnelAlreadyRunning,

    #[error("No active tunnel")]
    NoActiveTunnel,
}

/// Process-wide tokio runtime, started lazily by `initLogger` and reused
/// across JNI calls so async warren-core APIs can be driven from synchronous
/// JNI entry points.
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Active tunnel handle. Populated by `connectTunnel`, cleared by
/// `disconnectTunnel`. Only one tunnel at a time on Android (parity with
/// upstream VpnService model).
static ACTIVE_TUNNEL: Mutex<Option<TunnelHandle>> = Mutex::new(None);

/// Opaque handle stored while a tunnel is alive. The cancel sender lets
/// `disconnectTunnel` tear down the Quinn task gracefully.
struct TunnelHandle {
    _cancel_tx: oneshot::Sender<()>,
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Initialise Rust-side logging and the shared tokio runtime.
///
/// Must be called once at process start (typically from
/// `WarrenApplication.onCreate`). Idempotent calls are no-ops.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_initLogger(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    files_directory: JString<'_>,
) {
    let env = JnixEnv::from(env);
    let files_dir = pathbuf_from_java(&env, files_directory.into());

    if RUNTIME.get().is_none() {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                let _ = env.throw(format!("Failed to create Tokio runtime: {err}"));
                return;
            }
        };
        let _ = RUNTIME.set(rt);
    }

    if let Err(err) = init_log_file(&files_dir) {
        let _ = env.throw(format!("Failed to init log file: {err}"));
        return;
    }

    log_panics::init();
    log::info!("Warren JNI logger initialised in {}", files_dir.display());
}

fn init_log_file(_log_dir: &Path) -> Result<(), String> {
    // TODO (D.4): wire a file appender writing to <log_dir>/warren.log with
    // rotation. For now Android logcat capture via the default log facade is
    // sufficient for development.
    Ok(())
}

// ---------------------------------------------------------------------------
// BIP39 mnemonic + Ed25519 wallet (warren-identity)
// ---------------------------------------------------------------------------

/// Generate a fresh 12-word BIP39 English mnemonic. Returns the phrase as a
/// space-separated UTF-8 string. The mnemonic is **never persisted by Rust**
/// - the Kotlin caller is responsible for storing it via Android Keystore /
/// EncryptedSharedPreferences (D.5).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_generateMnemonic(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    // TODO (D.5): call warren_identity::Mnemonic::generate_in(Language::English, 12)
    // and return the joined phrase. Stub returns empty string for now so the
    // crate links without dragging the full identity surface in.
    match env.new_string("") {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Import an existing BIP39 mnemonic and return the derived Ed25519 public
/// key (32 bytes). The mnemonic string is **borrowed** for the call only.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_importMnemonic<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _mnemonic: JString<'local>,
) -> jbyteArray {
    // TODO (D.5): parse via warren_identity::derive_node_key + return pubkey.
    env.new_byte_array(0).unwrap_or(std::ptr::null_mut())
}

/// Sign the supplied canonical request bytes with the active wallet's
/// Ed25519 signing key. Returns a 64-byte signature, suitable for the
/// `X-Warren-Signature` header (cf. `warren-identity::auth`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_signRequest<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _canonical_message: jbyteArray,
) -> jbyteArray {
    // TODO (D.5): pull SigningKey from keystore-backed cache + sign bytes.
    env.new_byte_array(0).unwrap_or(std::ptr::null_mut())
}

// ---------------------------------------------------------------------------
// Tunnel lifecycle (warren-tunnel + warren-multihop)
// ---------------------------------------------------------------------------

/// Start a Warren Quinn tunnel.
///
/// Called by `WarrenQuinnAdapter` once Kotlin has obtained a TUN
/// `ParcelFileDescriptor` from `VpnService.Builder.establish()`. The
/// caller passes:
///   - `tun_fd`: the raw file descriptor (duplicated by Kotlin so we own
///     this lifetime).
///   - `config_json`: serde-encoded `WarrenTunnelConfig` (exit pubkey, exit
///     IP:port, optional multi-hop entry, optional DAITA spec, bypass
///     CIDRs, NAT-PMP enable, wallet pubkey).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_connectTunnel<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _tun_fd: jint,
    _config_json: JString<'local>,
) -> jint {
    let mut slot = ACTIVE_TUNNEL.lock().expect("ACTIVE_TUNNEL poisoned");
    if slot.is_some() {
        let _ = env.throw("Tunnel already running");
        return -1;
    }
    // TODO (D.4): deserialize WarrenTunnelConfig from config_json, build a
    // warren_tunnel::ClientConfig from it, spawn the Quinn pump on
    // `RUNTIME`, wire TUN <-> Quinn datagram path, and (when entry hop is
    // set) hand off to warren_multihop::MultiHopClient.
    let (cancel_tx, _cancel_rx) = oneshot::channel();
    *slot = Some(TunnelHandle {
        _cancel_tx: cancel_tx,
    });
    0
}

/// Stop the active tunnel. No-op if none is running.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_disconnectTunnel(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    if let Some(_handle) = ACTIVE_TUNNEL
        .lock()
        .expect("ACTIVE_TUNNEL poisoned")
        .take()
    {
        // Dropping `_cancel_tx` notifies the Quinn task to wind down. The
        // tokio runtime continues running for future tunnels.
    }
}

/// Returns 0 = disconnected, 1 = connecting, 2 = connected, 3 = reconnecting.
/// Matches the `WarrenTunnelState` Kotlin enum.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getTunnelStatus(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    if ACTIVE_TUNNEL
        .lock()
        .expect("ACTIVE_TUNNEL poisoned")
        .is_some()
    {
        2
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pathbuf_from_java(env: &JnixEnv<'_>, path: JObject<'_>) -> PathBuf {
    PathBuf::from(String::from_java(env, path))
}
