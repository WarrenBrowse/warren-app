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
//   - optional NAT-PMP port-forwarding via warrenguard-natpmp-client
//
// All JNI exports follow the
// `Java_com_warrenbrowse_vpn_jni_WarrenJni_<method>` naming convention
// dictated by the Kotlin `WarrenJni` companion object (`android/lib/...`).
//
// NOTE: wallet primitives (`generateMnemonic`,
// `importMnemonic`, `signRequest`) call into real warren-identity code via
// the pure-rust [`crate::wallet`] module - those work today and are
// covered by host tests. The tunnel lifecycle JNI exports remain stubs.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

// parking_lot::Mutex never poisons on panic, unlike std::sync::Mutex.
// If a JNI thread panics while holding ACTIVE_TUNNEL, subsequent calls can
// still acquire the lock rather than crashing with "ACTIVE_TUNNEL poisoned".
use parking_lot::Mutex;

use jnix::{
    FromJava, JnixEnv,
    jni::{
        JNIEnv,
        objects::{JClass, JObject, JString, JValue},
        sys::{jbyteArray, jint, jlong, jstring},
    },
};
use tokio::sync::oneshot;
// Every mnemonic that crosses the JNI boundary is wrapped in Zeroizing so its
// heap allocation is wiped on drop, on every path including early returns.
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// Error / runtime state
// ---------------------------------------------------------------------------

// Native-side error taxonomy reserved for the tunnel lifecycle work.
// Variants are unused until the connectTunnel body actually produces them;
// keeping the enum here documents the surface the JNI callers should expect
// to receive via `throw`. The `expect` form (rather than `allow`) makes
// `cargo clippy -D warnings` fail loudly the day a variant goes from "still
// unused" to "wrongly removed".
#[expect(dead_code, reason = "tunnel lifecycle work in progress")]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize logging: {0}")]
    InitializeLogging(String),

    #[error("Failed to create Tokio runtime")]
    InitTokio(#[source] std::io::Error),

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

/// The shared runtime, once `initLogger` created it.
pub(crate) fn runtime() -> Option<&'static tokio::runtime::Runtime> {
    RUNTIME.get()
}

/// Where `initLogger` put the Rust log files, once it ran: the problem
/// report collector reads them from there.
static RUST_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The Rust log directory, once `initLogger` ran.
pub(crate) fn rust_log_dir() -> Option<PathBuf> {
    RUST_LOG_DIR.get().cloned()
}

/// Active tunnel handle. Populated by `connectTunnel`, cleared by
/// `disconnectTunnel`. Only one tunnel at a time on Android (parity with
/// upstream VpnService model).
static ACTIVE_TUNNEL: Mutex<Option<TunnelHandle>> = Mutex::new(None);

/// The active session status (cf. [`crate::redial::SessionStatus`]) and the
/// generation counter Kotlin waits on. Read by [`getTunnelStatus`] without
/// taking the `ACTIVE_TUNNEL` mutex; every sibling fact Kotlin reads on the
/// same wake (the datapath verdicts, the NAT-PMP mapping, the recovery
/// counter) bumps this cell when it changes, so one `awaitStatusChange` covers
/// them all.
static SESSION_STATUS: crate::status_watch::StatusCell = crate::status_watch::StatusCell::new(0);

/// Count of automatic in-session recoveries since process start, read by
/// Kotlin through `getAutoRecoveryCount` and combined with the Kotlin-side
/// accounting of its own automatic reconnects (blackhole retry loop,
/// handover). Fed by the session supervisor's reconnect observer: one bump per
/// redial that lands, never on the initial connect.
static AUTO_RECOVERY_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// The supervisor's per-redial observer: advances the counter
/// [`Java_..._getAutoRecoveryCount`] publishes and wakes the Kotlin waiter,
/// since a redial that lands has no status edge of its own on this side.
#[cfg(feature = "tunnel")]
pub(crate) fn auto_recovery_observer() -> warrenguard_transport::supervisor::ReconnectObserver {
    std::sync::Arc::new(|| {
        AUTO_RECOVERY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        SESSION_STATUS.bump();
    })
}

/// Latest datapath verdict of the engine's goodput prober, polled by Kotlin
/// through `getPathHealth`. Encoded as [`PATH_HEALTH_HEALTHY`],
/// [`PATH_HEALTH_DEGRADED_LARGE`] or [`PATH_HEALTH_DEGRADED_BOTH`].
///
/// Without this, a wedged datapath (transport alive, nothing crossing it) is
/// invisible above the FFI: the app kept telling the user "You are protected"
/// on a tunnel that carried nothing.
static PATH_HEALTH: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(PATH_HEALTH_HEALTHY);

/// Paired probes deliver at both size classes.
pub(crate) const PATH_HEALTH_HEALTHY: i32 = 0;
/// Large probes die while small ones survive (last-mile shrink / brownout).
pub(crate) const PATH_HEALTH_DEGRADED_LARGE: i32 = 1;
/// Both probe sizes die while the session stays up: a wedged datapath.
pub(crate) const PATH_HEALTH_DEGRADED_BOTH: i32 = 2;

/// Publish a datapath verdict for Kotlin to poll. Called from
/// [`crate::tunnel`] by the task watching the supervisor's health channel.
#[cfg(feature = "tunnel")]
pub(crate) fn set_path_health(value: i32) {
    PATH_HEALTH.store(value, std::sync::atomic::Ordering::Relaxed);
    SESSION_STATUS.bump();
}

/// Clear the verdict on teardown so a stale degraded value never colours the
/// next session (or a disconnected UI).
pub(crate) fn reset_path_health() {
    PATH_HEALTH.store(PATH_HEALTH_HEALTHY, std::sync::atomic::Ordering::Relaxed);
    SESSION_STATUS.bump();
}

/// Latest "Reduced MTU" verdict of the session sampler, as the `jint` Kotlin
/// reads through `getEffectiveMtu` (see [`crate::path_metrics`]): the usable
/// inner payload once the live path measured below the TUN MTU, `0` while it
/// carries full-size packets. Feeds the connect screen's "Reduced MTU (n)"
/// chip, which on desktop comes from the negotiated endpoint and must not be
/// confused with the user-set MTU.
static EFFECTIVE_MTU: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Publish the sampler's verdict for Kotlin to read; wakes the waiter only
/// when the value moved, since the sampler ticks every few seconds.
#[cfg(feature = "tunnel")]
pub(crate) fn set_effective_mtu(verdict: Option<u16>) {
    let code = crate::path_metrics::effective_mtu_code(verdict);
    if EFFECTIVE_MTU.swap(code, std::sync::atomic::Ordering::Relaxed) != code {
        SESSION_STATUS.bump();
    }
}

/// Clear the verdict on teardown so the next session (or a disconnected UI)
/// never shows the previous path's reduction.
#[cfg(feature = "tunnel")]
pub(crate) fn reset_effective_mtu() {
    set_effective_mtu(None);
}

/// Latest in-tunnel egress verdict: `true` while the exit answers the client
/// and forwards nothing to the internet. Its own cell rather than a second
/// writer on [`PATH_HEALTH`], because the two publishers run on unrelated
/// cadences and either would erase the other's verdict; `getPathHealth`
/// composes them (see [`crate::egress_probe::compose_path_health`]).
static EGRESS_DEAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Publish an egress verdict for Kotlin to poll. Called from the probe task
/// spawned by [`crate::tunnel`].
#[cfg(feature = "tunnel")]
pub(crate) fn set_egress_dead(dead: bool) {
    EGRESS_DEAD.store(dead, std::sync::atomic::Ordering::Relaxed);
    SESSION_STATUS.bump();
}

/// Clear the egress verdict.
///
/// Deliberately NOT reset by the per-session teardown guard that clears
/// [`PATH_HEALTH`]: this verdict ENDS the session it fires on, and a flag
/// cleared by that very teardown would be gone before the Kotlin waiter reads
/// the drop. It survives until a fresh connect attempt re-arms the probe, or
/// the user tears the tunnel down.
pub(crate) fn reset_egress_dead() {
    EGRESS_DEAD.store(false, std::sync::atomic::Ordering::Relaxed);
    SESSION_STATUS.bump();
}

/// Latest NAT-PMP port-forwarding status as a JSON string, read by Kotlin
/// via `getNatPmpStatus()` on every status wake. Empty = idle (no active
/// mapping). The session-side NAT-PMP refresh loop writes transitions here via
/// [`set_natpmp_status`]; teardown clears it via [`reset_natpmp_status`].
/// `parking_lot::Mutex::new` is const, so no lazy init is needed.
static NATPMP_STATUS: Mutex<String> = Mutex::new(String::new());

/// Store the latest NAT-PMP status JSON. Called from the session refresh
/// loop. `pub(crate)` so [`crate::tunnel`] can reach it.
#[cfg_attr(not(all(target_os = "android", feature = "tunnel")), allow(dead_code))]
pub(crate) fn set_natpmp_status(json: String) {
    *NATPMP_STATUS.lock() = json;
    SESSION_STATUS.bump();
}

/// Reset the NAT-PMP status to idle (empty). Called on teardown.
#[cfg_attr(not(all(target_os = "android", feature = "tunnel")), allow(dead_code))]
pub(crate) fn reset_natpmp_status() {
    NATPMP_STATUS.lock().clear();
    SESSION_STATUS.bump();
}

/// Opaque handle stored while a tunnel is alive. The cancel sender lets
/// `disconnectTunnel` tear down the Quinn task gracefully. With the
/// `tunnel` feature on, we additionally pin the spawned task handle so
/// it stays alive after the JNI entry returns.
struct TunnelHandle {
    _cancel_tx: oneshot::Sender<()>,
    #[cfg(feature = "tunnel")]
    task: tokio::task::JoinHandle<()>,
}

/// The task of the session `disconnectTunnel` last wound down, parked here so
/// `awaitTunnelClosed` can wait for it to actually finish.
///
/// Dropping a `JoinHandle` only DETACHES the task, so the old teardown
/// returned while the session was still winding down. Dialling the next
/// session in that window registers it on the exit while the previous one
/// still holds the sticky inner IP, and the exit's takeover rule then decides
/// which of the two keeps the downlink. The desktop daemon has never had this
/// race: its `DisconnectingState` waits for the tunnel close event before
/// reconnecting.
#[cfg(feature = "tunnel")]
static CLOSING_TASK: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

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

    // The files dir doubles as the home of the persisted v7 token bundle
    // (app-private, allowBackup=false); captured here because no other JNI
    // call carries it.
    #[cfg(feature = "tunnel")]
    crate::token_provider::set_app_files_dir(&files_dir);

    log_panics::init();
    log::info!("Warren JNI logger initialised in {}", files_dir.display());
}

fn init_log_file(files_dir: &Path) -> Result<(), String> {
    // Every Rust record goes to logcat (debug and up, tag `WarrenJni`, the
    // `adb logcat -s WarrenJni:V` view) AND to a rotating file under the app's
    // files dir (info and up), which is what a problem report can carry: a
    // reboot empties logcat, and the failure a user reports usually happened
    // before they thought of reporting it. Idempotent: a second call keeps
    // the first logger (a process has one), so re-triggering `initLogger`
    // after a service restart never panics.
    // A second call (the VPN service after the application, or a process
    // that re-triggers the init) must touch nothing: opening the sink again
    // would rotate the live file out from under the logger already writing it.
    if RUST_LOG_DIR.get().is_some() {
        return Ok(());
    }
    let dir = files_dir.join(crate::rust_log::RUST_LOG_DIR_NAME);
    let sink = crate::rust_log::FileSink::open(&dir)
        .map_err(|e| format!("rust log file: {:?}", e.kind()))?;
    let logcat = android_logger::AndroidLogger::new(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("WarrenJni"),
    );
    let tee = crate::rust_log::Tee { logcat, file: sink };
    if log::set_boxed_logger(Box::new(tee)).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
    let _ = RUST_LOG_DIR.set(dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// BIP39 mnemonic + Ed25519 wallet (warren-identity)
// ---------------------------------------------------------------------------

/// Generate a fresh 12-word BIP39 English mnemonic. Returns the phrase as a
/// space-separated UTF-8 string. The mnemonic is **never persisted by Rust** -
/// the Kotlin caller is responsible for storing it via Android Keystore /
/// EncryptedSharedPreferences.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_generateMnemonic(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let phrase = crate::wallet::generate_mnemonic();
    match env.new_string(phrase) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Import an existing BIP39 mnemonic and return the derived Ed25519 public
/// key (32 bytes). The mnemonic string is **borrowed** for the call only.
/// On parse error, throws a Java exception and returns null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_importMnemonic<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
) -> jbyteArray {
    let env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&env, mnemonic));
    let pubkey = match crate::wallet::pubkey_from_mnemonic(&phrase) {
        Ok(p) => p,
        Err(e) => {
            let _ = env.throw(e.to_string());
            return std::ptr::null_mut();
        }
    };
    new_byte_array_from(&env, &pubkey)
}

/// Convenience wrapper: derive the wallet pubkey and return it as a
/// Warren **SS58 address** (`wb…`, network prefix 13295). This is the
/// canonical string form of the Warren wallet identity - the value the
/// `X-Warren-PubKey` request header carries and the Kotlin wallet
/// repository stores, so the caller can pass the result straight through.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_mnemonicPubkeySs58<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
) -> jstring {
    let env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&env, mnemonic));
    let ss58_str = match crate::wallet::pubkey_ss58_from_mnemonic(&phrase) {
        Ok(s) => s,
        Err(e) => {
            let _ = env.throw(e.to_string());
            return std::ptr::null_mut();
        }
    };
    match env.new_string(ss58_str) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Sign `canonicalMessage` bytes with the Ed25519 signing key derived from
/// `mnemonic`. Returns a 64-byte signature suitable for the
/// `X-Warren-Signature` header (cf. `warren-identity::auth`).
///
/// The mnemonic is passed per-call rather than cached in a static so the
/// secret never lives in Rust memory beyond the signing call.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_signRequest<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    canonical_message: jbyteArray,
) -> jbyteArray {
    let env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&env, mnemonic));
    let msg = match env.convert_byte_array(canonical_message) {
        Ok(v) => v,
        Err(e) => {
            let _ = env.throw(format!("convert_byte_array failed: {e}"));
            return std::ptr::null_mut();
        }
    };
    let sig = match crate::wallet::sign_message(&phrase, &msg) {
        Ok(s) => s,
        Err(e) => {
            let _ = env.throw(e.to_string());
            return std::ptr::null_mut();
        }
    };
    new_byte_array_from(&env, &sig)
}

/// Build the canonical `X-Warren-*` request message from its 5 fields and
/// sign it with the key derived from `mnemonic`. Returns the 64-byte
/// signature.
///
/// This is the recommended entry point for API request authentication:
/// it keeps the canonical byte-format ownership in Rust
/// (`warren_identity::auth::canonical_message`), so a future schema bump
/// only needs a single touch site, not one per client platform.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_signCanonicalRequest<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    method: JString<'local>,
    path: JString<'local>,
    timestamp: jlong,
    nonce_hex: JString<'local>,
    body_hash_hex: JString<'local>,
) -> jbyteArray {
    let env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&env, mnemonic));
    let method = String::from_java(&env, method);
    let path = String::from_java(&env, path);
    let nonce_hex = String::from_java(&env, nonce_hex);
    let body_hash_hex = String::from_java(&env, body_hash_hex);
    if timestamp < 0 {
        let _ = env.throw(format!("timestamp must be >= 0, got {timestamp}"));
        return std::ptr::null_mut();
    }
    let sig = match crate::wallet::sign_canonical_request(
        &phrase,
        &method,
        &path,
        timestamp as u64,
        &nonce_hex,
        &body_hash_hex,
    ) {
        Ok(s) => s,
        Err(e) => {
            let _ = env.throw(e.to_string());
            return std::ptr::null_mut();
        }
    };
    new_byte_array_from(&env, &sig)
}

/// Allocate a Java `byte[]` and copy `bytes` into it. Returns a null pointer
/// (and leaves any pending JVM exception unchanged) on allocation failure.
fn new_byte_array_from(env: &JnixEnv<'_>, bytes: &[u8]) -> jbyteArray {
    let len = bytes.len() as i32;
    let arr = match env.new_byte_array(len) {
        Ok(a) => a,
        Err(_) => return std::ptr::null_mut(),
    };
    // SAFETY: `bytes` are read-only and len matches the allocation. The
    // sign-cast goes through `as i8` to satisfy the `&[i8]` JNI signature
    // without touching the underlying bytes.
    let i8_view: &[i8] =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<i8>(), bytes.len()) };
    if env.set_byte_array_region(arr, 0, i8_view).is_err() {
        return std::ptr::null_mut();
    }
    arr
}

// ---------------------------------------------------------------------------
// Tunnel lifecycle (warren-tunnel + warrenguard-multihop)
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
    vpn_service: JObject<'local>,
    tun_fd: jint,
    mnemonic: JString<'local>,
    config_json: JString<'local>,
) -> jint {
    // parking_lot::Mutex::lock() returns the guard directly - no poisoning,
    // no Result to unwrap.
    let mut slot = ACTIVE_TUNNEL.lock();
    if slot.is_some() {
        let _ = env.throw("Tunnel already running");
        return -1;
    }

    let jnix_env = JnixEnv::from(env);
    let config_str = String::from_java(&jnix_env, config_json);

    #[cfg(feature = "tunnel")]
    {
        use std::os::fd::{FromRawFd, OwnedFd};

        use crate::redial::SessionStatus;

        if tun_fd < 0 {
            let _ = jnix_env.throw(format!("invalid tun_fd: {tun_fd}"));
            return -1;
        }
        let config = match crate::tunnel::parse_config(&config_str) {
            Ok(c) => c,
            Err(e) => {
                let _ = jnix_env.throw(format!("invalid tunnel config JSON: {e}"));
                return -1;
            }
        };

        let runtime = match RUNTIME.get() {
            Some(rt) => rt,
            None => {
                let _ = jnix_env.throw("initLogger() must be called before connectTunnel()");
                return -1;
            }
        };

        // SAFETY: Kotlin passes a freshly `detachFd()`-ed fd whose ownership
        // is now transferred to us. The fd is closed when AndroidTun drops.
        let owned: OwnedFd = unsafe { OwnedFd::from_raw_fd(tun_fd) };
        // `AndroidTun::from_fd` registers the fd with tokio's reactor through
        // `AsyncFd::new`, which panics ("there is no reactor running, must be
        // called from the context of a Tokio 1.x runtime") unless a runtime is
        // entered on the current thread. The JNI thread is not a runtime
        // worker, so enter the shared RUNTIME for the registration. Do NOT move
        // from_fd back before this guard, it crashes the whole process.
        let tun = {
            let _runtime_guard = runtime.enter();
            match warrenguard_transport::AndroidTun::from_fd(owned) {
                Ok(t) => t,
                Err(e) => {
                    let _ = jnix_env.throw(format!("AndroidTun::from_fd failed: {e}"));
                    return -1;
                }
            }
        };

        // Derive the signing key synchronously at the JNI boundary,
        // before any async task is spawned.  The mnemonic is wrapped in
        // Zeroizing so it is wiped immediately after key derivation - it never
        // enters the async task nor persists for the session lifetime.
        let mnemonic_zeroing = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
        let signing_key = match crate::wallet::signing_key_from_mnemonic(&mnemonic_zeroing) {
            Ok(k) => k,
            Err(e) => {
                let _ = jnix_env.throw(format!("wallet key derive failed: {e}"));
                return -1;
            }
        };
        // mnemonic_zeroing is dropped (and zeroized) here.
        drop(mnemonic_zeroing);

        // Register the VpnService socket protector before the session can
        // bind its Quinn socket. Without this the tunnel's own UDP socket is
        // routed into the TUN it creates (a loop) and the handshake never
        // leaves the device. The protector attaches the calling tokio thread
        // to the JVM and invokes `VpnService.protect(int)`.
        match register_socket_protector(&jnix_env, vpn_service) {
            Ok(()) => {}
            Err(e) => {
                let _ = jnix_env.throw(format!("failed to install socket protector: {e}"));
                return -1;
            }
        }

        let (cancel_tx, cancel_rx) = oneshot::channel();
        // Reset status to Connecting before spawning so the Kotlin side reads
        // it deterministically right after this JNI returns.
        SESSION_STATUS.store(SessionStatus::Connecting as i32);

        let task = runtime.spawn(crate::tunnel::run_session(
            tun,
            signing_key,
            config,
            &SESSION_STATUS,
            cancel_rx,
        ));

        *slot = Some(TunnelHandle {
            _cancel_tx: cancel_tx,
            task,
        });
        // The VpnService routes are installed by now: whatever the API pool
        // holds was opened on the physical network and is dead.
        retire_api_transport();
        0
    }
    #[cfg(not(feature = "tunnel"))]
    {
        let _ = (vpn_service, tun_fd, mnemonic, config_str);
        let _ = jnix_env.throw("warren-jni built without the `tunnel` feature");
        -1
    }
}

/// Installs the process-wide [`warrenguard_transport::socket_protect`] protector
/// backed by `VpnService.protect`. The protector captures the JVM handle
/// and a global ref to the service so the tunnel's Quinn socket can be
/// protected from any tokio worker thread: it attaches that thread to the
/// JVM and calls `VpnService.protect(int)` on the fd, returning the
/// boolean result. Re-registered per connect so it tracks the current
/// service instance.
#[cfg(feature = "tunnel")]
fn register_socket_protector(
    env: &JnixEnv<'_>,
    vpn_service: JObject<'_>,
) -> Result<(), jnix::jni::errors::Error> {
    use std::os::fd::RawFd;
    use std::sync::Arc;

    let vm = env.get_java_vm()?;
    let service_ref = env.new_global_ref(vpn_service)?;
    let protector: warrenguard_transport::socket_protect::SocketProtector =
        Arc::new(move |fd: RawFd| -> bool {
            let guard = match vm.attach_current_thread() {
                Ok(g) => g,
                Err(e) => {
                    log::error!("socket protect: attach_current_thread failed: {e}");
                    return false;
                }
            };
            match guard.call_method(
                service_ref.as_obj(),
                "protect",
                "(I)Z",
                &[JValue::Int(fd as jint)],
            ) {
                Ok(v) => v.z().unwrap_or(false),
                Err(e) => {
                    log::error!("VpnService.protect call failed: {e}");
                    false
                }
            }
        });
    warrenguard_transport::socket_protect::set_protector(protector);
    Ok(())
}

/// Stop the active tunnel. No-op if none is running.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_disconnectTunnel(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    // parking_lot::Mutex::lock() returns the guard directly - no poisoning,
    // no Result to unwrap.
    if let Some(_handle) = ACTIVE_TUNNEL.lock().take() {
        // Park the task so `awaitTunnelClosed` can wait for the wind-down to
        // finish; dropping the handle here would only detach it.
        #[cfg(feature = "tunnel")]
        {
            *CLOSING_TASK.lock() = Some(_handle.task);
        }
        // Dropping `_cancel_tx` notifies the Quinn task to wind down. The
        // task itself flips `SESSION_STATUS` back to Disconnected when its
        // `tokio::select!` falls through; we also do it here for symmetry
        // so the status flip is observable even before the task wakes.
        SESSION_STATUS.store(crate::redial::SessionStatus::Disconnected as i32);
        reset_natpmp_status();
        reset_path_health();
        reset_egress_dead();
        // The routes go with the interface: connections pooled through the
        // tunnel are dead the same way the pre-TUN ones were at connect.
        retire_api_transport();
    }
}

/// Report an underlying-network handover (Wi-Fi to cellular and back) to the
/// migration watchdog, which rebinds the live QUIC endpoint and revalidates the
/// path instead of re-handshaking. When the path cannot be recovered the
/// watchdog ends the session, so `getTunnelStatus()` reports `Disconnected` and
/// the Kotlin fail-closed policy runs its own handover fallback: this front-runs
/// that policy, it never replaces it.
///
/// Handle-free on purpose: the Kotlin `NetworkCallback` holds no tunnel handle,
/// and only one tunnel runs at a time. A no-op when no session is running.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_notifyNetworkChanged(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    #[cfg(feature = "tunnel")]
    crate::migration::notify_path_change();
    // The API pool's connections were opened on the network just left.
    retire_api_transport();
}

/// Returns the latest NAT-PMP port-forwarding status as a JSON string.
/// Shape: `{"state":"idle"|"requesting"|"mapped"|"rate_limited"|"failed",
/// "external_port":u16?, "lifetime_secs":u32?, "retry_after_secs":u16?,
/// "reason":"..."?}`. Read by Kotlin alongside `getTunnelStatus()` on every
/// status wake.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getNatPmpStatus(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let json = {
        let guard = NATPMP_STATUS.lock();
        if guard.is_empty() {
            "{\"state\":\"idle\"}".to_owned()
        } else {
            guard.clone()
        }
    };
    match env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The compiled product environment's anchor table
/// (`crate::product::product_anchors_json`), so Kotlin can hold the flavor's
/// `BuildConfig` copies of the scheme, the application id and the hosts to
/// the Rust reference. Pure: needs neither the logger nor the runtime.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_productAnchorsJson(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    match env.new_string(crate::product::product_anchors_json()) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns 0 = disconnected, 1 = connecting, 2 = connected, 3 = reconnecting.
/// Matches the `WarrenTunnelState` Kotlin enum. Reads `SESSION_STATUS`
/// without taking the `ACTIVE_TUNNEL` mutex, so a read on every wake is
/// cheap.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getTunnelStatus(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    SESSION_STATUS.load()
}

/// Blocks until the status generation differs from `last_seen` (a status
/// edge, a datapath verdict, a NAT-PMP transition or a landed redial), or
/// `timeout_ms` elapsed, and returns the generation the caller has now seen.
/// A caller that passes back what it received sleeps between changes; one
/// that passes a stale value returns at once, so nothing is ever missed
/// between two reads.
///
/// The wait parks the calling Java thread on the shared runtime, the same
/// way `awaitTunnelClosed` does: call it from a thread that may block, never
/// from the main thread. Without a runtime (no `initLogger` yet) the call
/// sleeps for the timeout so a caller cannot spin.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_awaitStatusChange(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    last_seen: jlong,
    timeout_ms: jint,
) -> jlong {
    let timeout = std::time::Duration::from_millis(timeout_ms.max(0) as u64);
    let last_seen = last_seen.max(0) as u64;
    let Some(runtime) = RUNTIME.get() else {
        std::thread::sleep(timeout);
        return SESSION_STATUS.generation() as jlong;
    };
    runtime.block_on(async {
        tokio::time::timeout(timeout, SESSION_STATUS.changed_since(last_seen))
            .await
            .unwrap_or_else(|_| SESSION_STATUS.generation())
    }) as jlong
}

/// Returns the number of automatic in-session recoveries (redial success
/// after a session loss) since process start. Monotonic; read by Kotlin
/// alongside `getTunnelStatus()` on every status wake to drive the
/// "Reconnections" detail row.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getAutoRecoveryCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    AUTO_RECOVERY_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Waits for the session `disconnectTunnel` wound down to actually finish,
/// up to `timeout_ms`. Returns `1` when it is gone (or there was nothing to
/// wait for), `0` on timeout.
///
/// Callers dial the next session only after this returns: registering a new
/// session while the previous one still holds the sticky inner IP is what made
/// an in-place exit switch black-hole, because the exit then has two live
/// claims on one downlink route and its takeover rule picks one.
#[cfg(feature = "tunnel")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_awaitTunnelClosed(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    timeout_ms: jint,
) -> jint {
    let Some(task) = CLOSING_TASK.lock().take() else {
        return 1;
    };
    let Some(runtime) = RUNTIME.get() else {
        return 1;
    };
    let timeout = std::time::Duration::from_millis(timeout_ms.max(0) as u64);
    match runtime.block_on(async { tokio::time::timeout(timeout, task).await }) {
        Ok(_) => 1,
        Err(_) => {
            log::warn!("awaitTunnelClosed: previous session still winding down after the timeout");
            0
        }
    }
}

/// Returns the engine's current datapath verdict: `0` healthy, `1` large
/// frames dying (last-mile shrink), `2` both probe sizes dying (a wedged
/// datapath), `3` the exit answers the client but forwards nothing to the
/// internet. Read by Kotlin alongside `getTunnelStatus()` on every status
/// wake so the UI can stop claiming protection on a tunnel that carries
/// nothing.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getPathHealth(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    let goodput = PATH_HEALTH.load(std::sync::atomic::Ordering::Relaxed);
    let egress_dead = EGRESS_DEAD.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(feature = "tunnel")]
    {
        crate::egress_probe::compose_path_health(goodput, egress_dead)
    }
    #[cfg(not(feature = "tunnel"))]
    {
        let _ = egress_dead;
        goodput
    }
}

/// Returns the "Reduced MTU" verdict: the usable inner payload in bytes once
/// the live path measured below the TUN MTU, `0` while it carries full-size
/// packets. Read by Kotlin alongside `getTunnelStatus()` on every status
/// wake.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getEffectiveMtu(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    EFFECTIVE_MTU.load(std::sync::atomic::Ordering::Relaxed)
}

/// Returns a JSON-encoded array of [`RelayInfo`] objects describing the
/// available Warren exits. The Kotlin side parses this into a list of
/// `RelayInfo` and feeds the relay selector / location picker UI.
///
/// Fetches `GET /v1/exits` via the SDK's `warren-api` client, verifies
/// the embedded signature against the pinned server pubkey, and
/// projects each `JsonRelay` to the JSON shape Kotlin expects. The
/// fetch runs on the shared Tokio runtime initialised by `initLogger`.
///
/// On any failure (network, signature, server pubkey mismatch) the
/// JNI surface returns a single-entry fallback pointing at the
/// known-good `warren-exit-1` so the user is never left with an
/// empty picker on a flaky network. A `log::warn!` records the
/// failure for the operator.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_listRelays(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let json = fetch_relays_or_fallback();
    match env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The raw signed multi-hop directory (`GET /v1/multihop/directory`) as its
/// verbatim JSON, or an empty string when none is available.
///
/// Served from the process cache while the copy is under an hour old and
/// still valid (the daemon's refresh cadence, `crate::directory_cache`), so
/// an exit switch dials without a round trip; fetched otherwise. Called from
/// Kotlin BEFORE the VpnService TUN is established so a fetch egresses the
/// physical network. A fetch issued from inside the tunnel session (after the
/// TUN is up) is captured by the half-open tunnel and blackholed, which is
/// exactly why `run_multi_hop_session` must NOT fetch this itself but receive
/// the blob through the tunnel config. The blob's signature, version and
/// expiry are verified Rust-side in `run_multi_hop_session` on every dial.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_fetchMultihopDirectory(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let raw = fetch_multihop_directory_raw();
    match env.new_string(raw) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The exit a location pin dials, chosen over the relay list Kotlin hands
/// back in the `listRelays` schema, with the shared
/// `warren_discovery_core::pick_exit` rule (`crate::exit_pin`). Returns
/// `{"index":n}` or `{"index":null}`. Pure: needs neither the logger nor
/// the runtime, and logs no value of either input.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_resolveExitPin<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    pin_json: JString<'local>,
    relays_json: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let pin = String::from_java(&jnix_env, pin_json);
    let relays = String::from_java(&jnix_env, relays_json);
    let json = crate::exit_pin::resolve_exit_pin_json(&pin, &relays);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The exit an unpinned dial goes to
/// (`crate::exit_pin::resolve_automatic_exit`): the shared pick among the
/// active rows of `exit_country`, and among every active row when that
/// country has none. An empty `exit_country` means none preferred. Same
/// answer shape as `resolveExitPin`. Pure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_resolveAutomaticExit<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    exit_country: JString<'local>,
    relays_json: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let exit_country = String::from_java(&jnix_env, exit_country);
    let relays = String::from_java(&jnix_env, relays_json);
    let json = crate::exit_pin::resolve_automatic_exit_json(&exit_country, &relays);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The exit a drop retry moves to (`crate::exit_pin::resolve_failover_exit`):
/// same JSON shapes and answer as `resolveExitPin`; an empty `exit_country`
/// means none preferred. Pure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_resolveFailoverExit<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    pin_json: JString<'local>,
    exit_country: JString<'local>,
    relays_json: JString<'local>,
    failed_exit_pubkey_hex: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let pin = String::from_java(&jnix_env, pin_json);
    let exit_country = String::from_java(&jnix_env, exit_country);
    let relays = String::from_java(&jnix_env, relays_json);
    let failed = String::from_java(&jnix_env, failed_exit_pubkey_hex);
    let json = crate::exit_pin::resolve_failover_exit_json(&pin, &exit_country, &relays, &failed);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// The one reqwest stack every API call that leaves on an ordinary socket
/// rides (the unsigned client, the signed subscription read, the network
/// descriptor): one connection pool and one root store per process instead
/// of one per call. Retired at every TUN transition and handover
/// ([`retire_api_transport`]) so no pooled connection outlives the network
/// it was opened on; see `crate::api_transport`.
#[cfg(target_os = "android")]
static API_TRANSPORT: crate::api_transport::TransportSlot<
    warren_api::reqwest_transport::ReqwestTransport,
> = crate::api_transport::TransportSlot::new(warren_api::reqwest_transport::ReqwestTransport::new);

#[cfg(target_os = "android")]
type ApiTransport =
    crate::api_transport::SharedTransport<warren_api::reqwest_transport::ReqwestTransport>;

/// A handle on the shared stack; a client built around it keeps working
/// across retirements.
#[cfg(target_os = "android")]
const fn api_transport() -> ApiTransport {
    crate::api_transport::SharedTransport::new(&API_TRANSPORT)
}

/// Drops the pooled API connections. Called on every TUN transition and
/// network handover: a TCP flow opened on the physical network dies silently
/// once the VpnService routes carry it through the exit (the server sees a
/// new source address), and the first request that reused it waited out the
/// whole 15 s transport timeout, twice per exit switch.
#[cfg(target_os = "android")]
fn retire_api_transport() {
    API_TRANSPORT.retire();
}

#[cfg(target_os = "android")]
static UNSIGNED_CLIENT: OnceLock<warren_api::WarrenApiClient<ApiTransport>> = OnceLock::new();

/// The shared `WarrenApiClient` for the unsigned, no-mnemonic-available
/// endpoints (`GET /v1/exits`, `GET /v1/multihop/directory`). The identity
/// is only used by SIGNED methods, so a fixed placeholder is harmless here
/// and documents the no-sign contract (mirrors warren-core's
/// `WarrenApiClient::new_unsigned` zero-key sentinel).
#[cfg(target_os = "android")]
fn unsigned_warren_client() -> &'static warren_api::WarrenApiClient<ApiTransport> {
    UNSIGNED_CLIENT.get_or_init(|| {
        warren_api::WarrenApiClient::new(
            PRODUCT_API_URL.to_owned(),
            warren_identity::WarrenIdentity::from_seed(&[0u8; 32]),
            api_transport(),
        )
    })
}

/// The last good directory and when it was fetched; see `crate::directory_cache`.
#[cfg(target_os = "android")]
static DIRECTORY_CACHE: crate::directory_cache::DirectoryCache =
    crate::directory_cache::DirectoryCache::new();

/// Raw signed multi-hop directory (cached or fetched), or empty string when
/// nothing usable exists.
#[cfg(target_os = "android")]
fn fetch_multihop_directory_raw() -> String {
    let Some(runtime) = RUNTIME.get() else {
        log::warn!("fetchMultihopDirectory called before initLogger; returning empty");
        return String::new();
    };
    let client = unsigned_warren_client();
    runtime
        .block_on(DIRECTORY_CACHE.fetch_or_cached(client, now_unix_secs(), verify_directory))
        .unwrap_or_default()
}

/// The verification `tunnel::run_multi_hop_session` runs at dial time, on the
/// same pins, so the cache never remembers a blob the dial would reject.
#[cfg(target_os = "android")]
fn verify_directory(raw: &str) -> bool {
    let server_pins: Vec<&str> = SERVER_PUBKEY_HEX.into_iter().collect();
    match warren_discovery_core::verify_multihop_directory_any(
        raw,
        &server_pins,
        &[crate::tunnel::WARREN_MULTIHOP_ROOT_PUBKEY_HEX],
    ) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("fetchMultihopDirectory: directory verify failed: {e}");
            false
        }
    }
}

/// Seconds since the Unix epoch, 0 on a clock before it (the cache then
/// reads every copy as stale and refetches, the daemon's rule for a clock
/// it cannot trust).
#[cfg(target_os = "android")]
fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn fetch_multihop_directory_raw() -> String {
    String::new()
}

// The API URL + server pubkey live in the host-compiled `crate::product`
// module (per product environment, drift-gated against the contract).
use crate::product::{PRODUCT_API_URL, SERVER_PUBKEY_HEX};

/// Empty relay catalogue. Returned when no relay list is available (the live
/// fetch failed AND nothing was cached yet). An empty list makes the connect
/// flow fail closed and honestly ("No relay available") instead of handing the
/// tunnel a non-connectable placeholder. Do NOT replace this with a hardcoded
/// relay carrying a fake `exit_pubkey_hex`: a 16-byte operator id is not a
/// 32-byte pubkey, so the tunnel rejects it with "invalid exit pubkey" and the
/// app wedges in a misleading BLOCKED state.
const EMPTY_RELAYS_JSON: &str = "[]";

/// Last successfully fetched + verified relay list, already projected to the
/// Kotlin schema. Reused when a later fetch fails (e.g. the kill-switch is
/// blocking the network during a reconnect, so `GET /v1/exits` cannot land):
/// returning the last good list keeps the real 64-hex exit pubkeys instead of
/// falling back to nothing. In-memory only; empty until the first success.
#[cfg(target_os = "android")]
static LAST_GOOD_RELAYS: Mutex<Option<String>> = Mutex::new(None);

/// The cached relay list if a previous fetch ever succeeded, else an empty
/// catalogue. Never a non-connectable placeholder.
#[cfg(target_os = "android")]
fn cached_relays_or_empty() -> String {
    LAST_GOOD_RELAYS
        .lock()
        .clone()
        .unwrap_or_else(|| EMPTY_RELAYS_JSON.to_owned())
}

/// Fetch + verify the signed relay list, projecting the result to the
/// Kotlin-side JSON shape. On any error returns the last good list (if any),
/// else an empty catalogue.
#[cfg(target_os = "android")]
fn fetch_relays_or_fallback() -> String {
    let runtime = match RUNTIME.get() {
        Some(rt) => rt,
        None => {
            log::warn!("listRelays called before initLogger; returning cached/empty");
            return cached_relays_or_empty();
        }
    };

    // `/v1/exits` is unsigned (response is server-signed); use the
    // placeholder-identity client so the code path never accidentally
    // relies on a real mnemonic being available here.
    let client = unsigned_warren_client();

    let raw = match runtime.block_on(client.list_exits()) {
        Ok(s) => s,
        Err(_) => {
            log::warn!("listRelays: GET /v1/exits failed; returning cached/empty");
            return cached_relays_or_empty();
        }
    };

    let signed = match warren_discovery_core::verify_signed_relay_list(&raw, SERVER_PUBKEY_HEX) {
        Ok(s) => s,
        Err(e) => {
            // reason_code() is a stable secret-free category (Display is a
            // human sentence that can drift across contract versions), so
            // the label is greppable and aggregatable across releases.
            log::warn!(
                "listRelays: exit directory verify failed: reason={}; returning cached/empty",
                e.reason_code()
            );
            return cached_relays_or_empty();
        }
    };

    // Project each WarrenRelay to the Kotlin schema: { exit_id,
    // exit_pubkey_hex, endpoint, country, city, active, weight }.
    // Pick the first ip_addr entry as the endpoint (the schema is
    // single-endpoint on the Kotlin side until multi-endpoint
    // failover lands).
    let projected: Vec<serde_json::Value> = signed
        .relays
        .relays()
        .iter()
        .map(|r| {
            let endpoint = r
                .endpoint_addr()
                .ip_addrs()
                .next()
                .map(|sa| sa.to_string())
                .unwrap_or_else(|| "0.0.0.0:443".to_owned());
            serde_json::json!({
                "exit_id": r.exit_id().to_hex(),
                "exit_pubkey_hex": r.endpoint_id().to_hex(),
                "endpoint": endpoint,
                "country": r.location().country_code(),
                "city": r.location().city(),
                "active": r.is_active(),
                "weight": r.weight(),
            })
        })
        .collect();

    match serde_json::to_string(&projected) {
        Ok(json) => {
            // Cache the freshly verified list so a later fetch failure (e.g. a
            // reconnect while the kill-switch blocks the network) can reuse the
            // real exit pubkeys instead of falling back to an empty catalogue.
            *LAST_GOOD_RELAYS.lock() = Some(json.clone());
            json
        }
        Err(_) => cached_relays_or_empty(),
    }
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn fetch_relays_or_fallback() -> String {
    EMPTY_RELAYS_JSON.to_owned()
}

// ---------------------------------------------------------------------------
// App version check (signed update manifest)
// ---------------------------------------------------------------------------

/// Both verdicts of the signed update manifest from one fetch, as
/// `{"supported":bool,"latest":"x.y.z"}` (`latest` empty when no stable
/// release above the running one exists).
///
/// The Kotlin side feeds `supported` into `VersionInfo.isSupported`, which
/// drives the "unsupported version" banner and the forced-update gate, and
/// `latest` into the sideload-only "update available" notification. The
/// manifest is `android.json` over the Let's-Encrypt-pinned host, verified
/// against the embedded trusted pubkey with the exact desktop verifier
/// (`mullvad-update`), then read by `crate::version_check`.
///
/// Fail-open on support and fail-closed on the prompt: any failure (runtime
/// not ready, unparseable version, network, signature) answers
/// `{"supported":true,"latest":""}`, so a flaky network never locks the user
/// out and never shows an update that may not exist. Blocks on a network
/// fetch, so it must be invoked off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_fetchVersionInfo<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    current_version: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let current = String::from_java(&jnix_env, current_version);
    let verdict = fetch_version_verdict(&current);
    let json = serde_json::json!({
        "supported": verdict.supported,
        "latest": verdict.latest.map(|v| v.to_string()).unwrap_or_default(),
    })
    .to_string();
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// One fetch and verification of the manifest, then both verdicts.
fn fetch_version_verdict(current_version: &str) -> crate::version_check::VersionVerdict {
    use crate::version_check::{VersionVerdict, version_verdict};
    use mullvad_update::api::{HttpVersionInfoProvider, MetaRepositoryPlatform};
    use mullvad_update::version::MIN_VERIFY_METADATA_VERSION;

    let runtime = match RUNTIME.get() {
        Some(rt) => rt,
        None => {
            log::warn!("fetchVersionInfo called before initLogger; assuming supported, no update");
            return VersionVerdict::UNKNOWN;
        }
    };
    let current: mullvad_version::Version = match current_version.parse() {
        Ok(version) => version,
        Err(err) => {
            log::warn!("fetchVersionInfo: unparseable version '{current_version}': {err}");
            return VersionVerdict::UNKNOWN;
        }
    };
    let response = match runtime.block_on(HttpVersionInfoProvider::get_versions_for_platform(
        MetaRepositoryPlatform::Android,
        MIN_VERIFY_METADATA_VERSION,
    )) {
        Ok(response) => response,
        Err(_) => {
            log::warn!(
                "fetchVersionInfo: manifest fetch/verify failed; assuming supported, no update"
            );
            return VersionVerdict::UNKNOWN;
        }
    };
    version_verdict(&current, &response.signed)
}

/// Fetch the wallet's subscription status (signed `GET /v1/subscription`).
/// Returns a JSON object `{"ok": true, "expires_at": <unix secs>}` on
/// success or `{"ok": false, "error": "..."}` on failure. The mnemonic
/// derives the signing key at the boundary; it is not retained.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getSubscription<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let json = match get_subscription_inner(&phrase) {
        Ok(expires_at) => serde_json::json!({"ok": true, "expires_at": expires_at}).to_string(),
        Err(e) => {
            // Do not log the error chain: a 4xx body could echo request
            // context. The structured envelope still returns to Kotlin.
            log::warn!("getSubscription failed");
            subscription_error_envelope(&e)
        }
    };
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// A subscription fetch failure, kept structured so the boundary can
/// surface a server `status`. A 404 is not a real error: it means "no
/// subscription bound yet" (a fresh wallet), which Kotlin maps to the Unix
/// epoch the same way iOS / desktop do.
#[cfg(target_os = "android")]
enum SubscriptionFetchError {
    /// Failed before reaching the server (runtime / mnemonic): no status.
    Setup(String),
    /// The signed request reached the server and it answered non-2xx (or a
    /// transport error occurred). `ServerStatus` carries the HTTP status.
    Client(warren_api::ClientError),
}

/// Build the `{"ok":false,...}` envelope, surfacing `status` on a server
/// non-2xx so Kotlin can distinguish 404 (no subscription) from a real
/// failure. Mirrors the iOS `err_client_json` (warren-ios account FFI).
#[cfg(target_os = "android")]
fn subscription_error_envelope(err: &SubscriptionFetchError) -> String {
    use warren_api::ClientError;
    match err {
        SubscriptionFetchError::Client(client_err) => {
            let mut obj = serde_json::Map::new();
            obj.insert("ok".to_owned(), serde_json::Value::Bool(false));
            obj.insert(
                "error".to_owned(),
                serde_json::Value::String(format!("get_subscription failed: {client_err}")),
            );
            // Surface the HTTP status on a server non-2xx so Kotlin maps 404
            // (no subscription bound yet) to the epoch, not a hard failure.
            if let ClientError::ServerStatus { status, .. } = client_err {
                obj.insert("status".to_owned(), serde_json::json!(status));
            }
            serde_json::Value::Object(obj).to_string()
        }
        SubscriptionFetchError::Setup(msg) => {
            serde_json::json!({"ok": false, "error": msg}).to_string()
        }
    }
}

#[cfg(target_os = "android")]
fn get_subscription_inner(mnemonic: &str) -> Result<u64, SubscriptionFetchError> {
    let runtime = RUNTIME.get().ok_or_else(|| {
        SubscriptionFetchError::Setup("initLogger must be called before getSubscription".to_owned())
    })?;
    let identity = warren_identity::WarrenIdentity::from_mnemonic(mnemonic)
        .map_err(|e| SubscriptionFetchError::Setup(format!("invalid mnemonic: {e}")))?;
    // The identity is per call (the mnemonic never lingers); the HTTP stack
    // under it is the process-wide one.
    let client =
        warren_api::WarrenApiClient::new(PRODUCT_API_URL.to_owned(), identity, api_transport());
    let resp = runtime
        .block_on(client.subscription())
        .map_err(SubscriptionFetchError::Client)?;
    Ok(resp.expires_at)
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn get_subscription_inner(_mnemonic: &str) -> Result<u64, SubscriptionFetchError> {
    Err(SubscriptionFetchError::Setup(
        "getSubscription is Android-only".to_owned(),
    ))
}

/// Host-build mirror of the Android enum so the `not(target_os = "android")`
/// stub above type-checks. Only the `Setup` arm is constructed off-device.
#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
enum SubscriptionFetchError {
    Setup(String),
}

/// Fetch the public `GET /v1/network` environment descriptor
/// (unauthenticated display data: environment label, degraded flag,
/// default bandwidth cap, payments flag). Returns
/// `{"ok":true,"environment":...,"degraded":...,"default_rate_bps":...,
/// "payments_enabled":...}` or `{"ok":false}` on any failure, including
/// an API that predates the endpoint; Kotlin treats both as "no info".
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_fetchNetworkInfo<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let json = fetch_network_info_json();
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "android")]
fn fetch_network_info_json() -> String {
    use warren_api::transport::{HttpRequest, HttpTransport, Method};
    let fail = || serde_json::json!({"ok": false}).to_string();
    let Some(runtime) = RUNTIME.get() else {
        return fail();
    };
    let transport = api_transport();
    let request = HttpRequest {
        method: Method::Get,
        url: format!("{PRODUCT_API_URL}/v1/network"),
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    match runtime.block_on(transport.execute(request)) {
        Ok(resp) if resp.status == 200 => {
            match serde_json::from_slice::<warren_contract::dto::NetworkInfoResponse>(&resp.body) {
                Ok(info) => serde_json::json!({
                    "ok": true,
                    "environment": info.environment,
                    "degraded": info.degraded,
                    "default_rate_bps": info.default_rate_bps,
                    "payments_enabled": info.payments_enabled,
                })
                .to_string(),
                Err(_) => fail(),
            }
        }
        // 404 = the API predates the endpoint: same "no info" answer as
        // a transport failure, without error noise.
        _ => fail(),
    }
}

/// The verified operator broadcast notices held for this process, and the
/// generation high-water mark that guards them. In memory only, like the
/// daemon's: a notice is a live statement, never read off a disk cache.
static NOTICES: parking_lot::Mutex<crate::notices::NoticesState> =
    parking_lot::Mutex::new(crate::notices::NoticesState::new());

/// One fetch of the operator broadcast notices (`GET /v1/notices` on the API
/// host), verified against the pinned server key with the anti-rollback and
/// freshness rules of [`crate::notices`]. Returns
/// `{"notices":[{"id":..,"message":..,"level":"info"|"warning"|"error"}],
/// "fetch":"ok"|"rejected"|"transport"}`: the list is what the banner must
/// show right now, already filtered for the envelope expiry, each notice's own
/// TTL and `current_version`, so Kotlin renders it verbatim as plain text.
///
/// It rides the shared API transport, so the request leaves through the tunnel
/// whenever one is up, like every other `/v1` call this app makes; the route
/// is public and unauthenticated, and nothing about the caller is sent.
/// Kotlin owns the cadence (five minutes in the foreground, a fetch on
/// resume). Blocks on a network GET: invoke off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_noticesFetch<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    current_version: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let current = String::from_java(&jnix_env, current_version);
    let json = notices_fetch(&current);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

fn notices_fetch(current_version: &str) -> String {
    use crate::notices::{Fetched, Refresh, envelope};
    use warren_api::transport::{HttpRequest, HttpTransport, Method};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // A version this build cannot state is `None`, which withholds every
    // range-targeted notice: a targeted message shown to an untargeted client
    // is worse than one not shown.
    let client_version = Some(current_version).filter(|v| !v.is_empty());
    let Some(runtime) = RUNTIME.get() else {
        log::warn!("notices: initLogger must run first");
        return envelope(&[], Refresh::Transport);
    };
    let transport = api_transport();
    let request = HttpRequest {
        method: Method::Get,
        url: format!("{}/v1/notices", PRODUCT_API_URL.trim_end_matches('/')),
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    let fetched = match runtime.block_on(transport.execute(request)) {
        Ok(response) if response.status == 200 => {
            Fetched::Body(String::from_utf8(response.body).unwrap_or_default())
        }
        Ok(response) => Fetched::Status(response.status),
        Err(_) => {
            log::debug!("notices: fetch failed");
            Fetched::Transport
        }
    };
    let pins: Vec<&str> = SERVER_PUBKEY_HEX.into_iter().collect();
    let mut state = NOTICES.lock();
    let refresh = state.accept(fetched, &pins);
    envelope(&state.display(now, client_version), refresh)
}

/// The verified launch announcements held for this process, and the generation
/// high-water mark that guards them. In memory only, like the daemon's: a
/// withdrawn announcement must not come back off a disk cache.
static ANNOUNCEMENTS: parking_lot::Mutex<crate::announcements::AnnouncementsState> =
    parking_lot::Mutex::new(crate::announcements::AnnouncementsState::new());

/// The campaign codes drawn for the wallet this process is running as; see
/// [`crate::announcements::SessionCodes`].
static CAMPAIGN_CODES: parking_lot::Mutex<crate::announcements::SessionCodes> =
    parking_lot::Mutex::new(crate::announcements::SessionCodes::new());

/// One fetch of the launch announcements (`GET /v1/announcements` on the API
/// host), verified against the pinned server key with the anti-rollback and
/// freshness rules of [`crate::announcements`]. Returns
/// `{"announcements":[{"id":..,"headline":..,"body":..,"level":..,"cta":..,
/// "voucher_campaign_id":..}],"fetch":"ok"|"rejected"|"transport"}`: the list
/// is what the card must show right now, already filtered for the envelope
/// expiry, each announcement's own TTL and `current_version`, with an unsafe
/// call to action already withheld, so Kotlin renders it verbatim as plain
/// text.
///
/// The route is public and unauthenticated and the document is byte-identical
/// for every caller, so nothing about the account is sent and no cadence can be
/// tied to one. A per-account code comes from the separate, wallet-signed
/// [`Java_com_warrenbrowse_vpn_jni_WarrenJni_campaignVoucher`]. Kotlin owns the
/// cadence (five minutes in the foreground, a fetch on resume). Blocks on a
/// network GET: invoke off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_announcementsFetch<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    current_version: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let current = String::from_java(&jnix_env, current_version);
    let json = announcements_fetch(&current);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

fn announcements_fetch(current_version: &str) -> String {
    use crate::announcements::{Fetched, Refresh, envelope};
    use warren_api::transport::{HttpRequest, HttpTransport, Method};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // A version this build cannot state is `None`, which withholds every
    // range-targeted announcement: a targeted card shown to an untargeted
    // client is worse than one not shown.
    let client_version = Some(current_version).filter(|v| !v.is_empty());
    let Some(runtime) = RUNTIME.get() else {
        log::warn!("announcements: initLogger must run first");
        return envelope(&[], Refresh::Transport);
    };
    let transport = api_transport();
    let request = HttpRequest {
        method: Method::Get,
        url: format!("{}/v1/announcements", PRODUCT_API_URL.trim_end_matches('/')),
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    let fetched = match runtime.block_on(transport.execute(request)) {
        Ok(response) if response.status == 200 => {
            Fetched::Body(String::from_utf8(response.body).unwrap_or_default())
        }
        Ok(response) => Fetched::Status(response.status),
        Err(_) => {
            log::debug!("announcements: fetch failed");
            Fetched::Transport
        }
    };
    let pins: Vec<&str> = SERVER_PUBKEY_HEX.into_iter().collect();
    let mut state = ANNOUNCEMENTS.lock();
    let refresh = state.accept(fetched, &pins);
    envelope(&state.display(now, client_version), refresh)
}

/// This account's code for a campaign an announcement offers
/// (`GET /v1/campaign/{campaign_id}/voucher`), signed with the wallet and sent
/// here like every other signed call. Returns `{"ok":true,"code":"..."}`,
/// `{"ok":true,"code":null}` when the account is outside the cohort (the
/// server's `404`, a normal and quiet outcome), or `{"ok":false,"code":null}`
/// when the lookup failed, which the caller retries rather than reading as
/// "you were never eligible".
///
/// The lookup is a pure server-side read that never mints and never assigns, so
/// repeating it is always safe; the answer is nevertheless held per identity for
/// the life of the process, so a five minute poll does not become a signed
/// request every five minutes. The code is a bearer token worth a month of
/// service: it goes to the account's own screen and nowhere else, never to a
/// log, an error or a problem report. Blocks on a network GET: invoke off the
/// main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_campaignVoucher<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    campaign_id: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let campaign = String::from_java(&jnix_env, campaign_id);
    let json = crate::announcements::voucher_envelope(campaign_voucher(&phrase, &campaign));
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "android")]
fn campaign_voucher(mnemonic: &str, campaign_id: &str) -> Result<Option<String>, ()> {
    use crate::announcements::Held;

    if !crate::announcements::campaign_id_is_wire_safe(campaign_id) {
        // The wallet is not even read: no code can come of an id the signed
        // path could not carry, and a card with no code still shows.
        log::debug!("campaignVoucher: the campaign id is not wire-safe, not drawn");
        return Ok(None);
    }
    let Some(runtime) = RUNTIME.get() else {
        log::warn!("campaignVoucher: initLogger must run first");
        return Err(());
    };
    let identity = warren_identity::WarrenIdentity::from_mnemonic(mnemonic).map_err(|_| {
        log::warn!("campaignVoucher: wallet could not be derived");
    })?;
    // The identity is read BEFORE anything else and the whole cache is dropped
    // when it changed: acting under a desynced identity would show one account
    // the offer drawn for another.
    let address = identity.address();
    if let Held::Known(code) = CAMPAIGN_CODES.lock().held(&address, campaign_id) {
        return Ok(code);
    }
    let client =
        warren_api::WarrenApiClient::new(PRODUCT_API_URL.to_owned(), identity, api_transport());
    let fetched = runtime
        .block_on(client.campaign_voucher(campaign_id))
        .map_err(|e| {
            // The error itself is never rendered: a server body may echo
            // identity material, and the code must never reach a log. Nothing
            // is cached either, so the next poll asks again.
            log::debug!(
                "campaignVoucher: lookup failed ({})",
                campaign_failure_class(&e)
            );
        })?;
    CAMPAIGN_CODES
        .lock()
        .record(&address, campaign_id, fetched.clone());
    Ok(fetched)
}

/// The one word a campaign-lookup failure is logged as. The `ClientError`
/// itself carries a server body, which may echo identity material.
#[cfg(target_os = "android")]
fn campaign_failure_class(error: &warren_api::ClientError) -> &'static str {
    match error {
        warren_api::ClientError::ServerStatus { .. } => "refused by the server",
        _ => "transport failed",
    }
}

// ---------------------------------------------------------------------------
// Incident telemetry (`/v1/incidents/*`)
// ---------------------------------------------------------------------------

/// The transport the two incident reports ride: the VpnService-protected one
/// when the crate carries it, because both fire while the kill-switch
/// blackhole is up (an exit-down report is posted from the retry that follows
/// a drop, a mismatch report from a modal that refused the dial). A plain
/// socket would be captured by that interface and time out; a protected one
/// leaves on the physical network, which is where the desktop daemon's own
/// reports go, its firewall holding the API open while the tunnel is down.
#[cfg(feature = "tunnel")]
type IncidentTransport = crate::protected_transport::ProtectedTransport;
#[cfg(not(feature = "tunnel"))]
type IncidentTransport = warren_api::reqwest_transport::ReqwestTransport;

/// The token bucket over `POST /v1/incidents/exit-down`, one per process like
/// the daemon's one per run. Built on first use because its epoch is an
/// `Instant`, which no `static` can name.
static EXIT_DOWN_BUDGET: Mutex<Option<crate::incidents::ExitDownReportBudget>> = Mutex::new(None);

/// Reports an exit this client gave up on (`POST /v1/incidents/exit-down`),
/// so a client-visible outage reaches `GET /v1/admin/exits/health` instead of
/// dying on the device. Signed by the wallet; the server records no signer,
/// only the exit and the count.
///
/// Best-effort and budgeted: the answer says whether the report left, and the
/// caller acts on nothing. Returns `{"ok":true}` or
/// `{"ok":false,"reason":"budget"|"malformed"|"identity"|"runtime"|"transport"|"rejected"}`.
/// Blocks on a network POST: invoke off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_reportExitDown<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    exit_pubkey_hex: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let exit_pubkey_hex = String::from_java(&jnix_env, exit_pubkey_hex);
    let outcome = report_exit_down(&phrase, &exit_pubkey_hex);
    log::info!("reportExitDown: {}", outcome_class(outcome));
    match jnix_env.new_string(crate::incidents::envelope(outcome)) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

fn report_exit_down(
    mnemonic: &str,
    exit_pubkey_hex: &str,
) -> Result<(), crate::incidents::NotSent> {
    use crate::incidents::NotSent;

    // Spent before the body is built, the daemon's order: a client whose
    // failover loop keeps producing the same report is throttled whatever the
    // report says.
    let allowed = EXIT_DOWN_BUDGET
        .lock()
        .get_or_insert_with(crate::incidents::ExitDownReportBudget::new)
        .try_acquire(std::time::Instant::now());
    if !allowed {
        return Err(NotSent::Budget);
    }
    let request = crate::incidents::exit_down_request(exit_pubkey_hex, unix_now())?;
    let (runtime, client) = incident_client(mnemonic)?;
    runtime
        .block_on(client.report_exit_down(&request))
        .map_err(client_error_class)
}

/// Reports a pinned exit key that changed (`POST /v1/incidents/pubkey-mismatch`),
/// what the desktop's "Report to Warren" button on the same modal does. Every
/// field is already public through the signed relay list, and the server
/// records no signer, so the report says what changed and not who saw it.
///
/// Returns the same envelope as [`Java_com_warrenbrowse_vpn_jni_WarrenJni_reportExitDown`],
/// minus the `budget` class (one report per user decision, so there is no
/// loop to cap). Blocks on a network POST: invoke off the main thread.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, reason = "the wire body has six fields")]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_reportPubkeyMismatch<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    exit_id_hex: JString<'local>,
    old_pubkey_hex: JString<'local>,
    new_pubkey_hex: JString<'local>,
    country_code: JString<'local>,
    city: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let request = crate::incidents::pubkey_mismatch_request(
        &String::from_java(&jnix_env, exit_id_hex),
        &String::from_java(&jnix_env, old_pubkey_hex),
        &String::from_java(&jnix_env, new_pubkey_hex),
        &String::from_java(&jnix_env, country_code),
        &String::from_java(&jnix_env, city),
        unix_now(),
    );
    let outcome = report_pubkey_mismatch(&phrase, &request);
    log::info!("reportPubkeyMismatch: {}", outcome_class(outcome));
    match jnix_env.new_string(crate::incidents::envelope(outcome)) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

fn report_pubkey_mismatch(
    mnemonic: &str,
    request: &warren_api::IncidentPubkeyMismatchRequest,
) -> Result<(), crate::incidents::NotSent> {
    let (runtime, client) = incident_client(mnemonic)?;
    runtime
        .block_on(client.report_pubkey_mismatch(request))
        .map_err(client_error_class)
}

/// The runtime and a freshly-signed client for one incident report. The
/// identity is per call (the mnemonic never lingers), and the transport is
/// built per call rather than pooled: a report fires at most a handful of
/// times per session, and the network under it has just changed.
fn incident_client(
    mnemonic: &str,
) -> Result<
    (
        &'static tokio::runtime::Runtime,
        warren_api::WarrenApiClient<IncidentTransport>,
    ),
    crate::incidents::NotSent,
> {
    use crate::incidents::NotSent;

    let runtime = runtime().ok_or(NotSent::Runtime)?;
    let identity =
        warren_identity::WarrenIdentity::from_mnemonic(mnemonic).map_err(|_| NotSent::Identity)?;
    let client = warren_api::WarrenApiClient::new(
        PRODUCT_API_URL.to_owned(),
        identity,
        IncidentTransport::new(),
    );
    Ok((runtime, client))
}

/// A client failure as one of the report classes. The error itself is never
/// rendered: a server body may echo identity material.
fn client_error_class(error: warren_api::ClientError) -> crate::incidents::NotSent {
    match error {
        warren_api::ClientError::ServerStatus { .. } => crate::incidents::NotSent::Rejected,
        _ => crate::incidents::NotSent::Transport,
    }
}

/// The one word an incident log line carries.
fn outcome_class(outcome: Result<(), crate::incidents::NotSent>) -> &'static str {
    match outcome {
        Ok(()) => "sent",
        Err(crate::incidents::NotSent::Budget) => "suppressed by the local budget",
        Err(crate::incidents::NotSent::Malformed) => "malformed",
        Err(crate::incidents::NotSent::Identity) => "no identity",
        Err(crate::incidents::NotSent::Runtime) => "runtime not up",
        Err(crate::incidents::NotSent::Transport) => "transport failed",
        Err(crate::incidents::NotSent::Rejected) => "refused by the server",
    }
}

/// Unix seconds, `0` when the device clock predates the epoch. The server
/// replaces the value with its own clock when it records, so a wrong one is
/// carried for forward compatibility rather than trusted.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// Device list/remove JNI exports were dropped: Warren's identity is the
// BIP39 wallet (one wallet = one pubkey), not a Mullvad-style per-account
// device registry. The backend has no `/v1/devices` list/delete endpoint;
// it only tracks ephemeral self-managed sessions (`/v1/session/open|close`)
// capped at MAX_DEVICES_PER_ACCOUNT concurrent, freed on disconnect/TTL, so
// there is nothing for a user to list or revoke. The Kotlin device-management
// UI was already removed (commit dc5b0b0928); this finishes that on the Rust
// side. Desktop is on the same login-state-only model.

/// Redeem a subscription voucher (`POST /v1/register`). Binds the wallet
/// pubkey to a new subscription. The request is unsigned (the pubkey is
/// carried in the body), so only the wallet pubkey is derived from the
/// mnemonic. Returns `{"ok": true, "expires_at": <unix secs>}` or
/// `{"ok": false, "error": "..."}`. The voucher secret is never logged.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_redeemVoucher<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    voucher: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let voucher_secret = String::from_java(&jnix_env, voucher);
    let json = match redeem_voucher_inner(&phrase, &voucher_secret) {
        Ok(expires_at) => serde_json::json!({"ok": true, "expires_at": expires_at}).to_string(),
        Err(e) => {
            // "purchase pending" is the expected steady state of a purchase poll
            // (webhook not landed yet), not a failure: keep it at debug so the
            // 5s poll does not spam warnings for the whole 10-minute window.
            if e == "purchase pending" {
                log::debug!("redeemVoucher: purchase pending");
            } else {
                log::warn!("redeemVoucher failed");
            }
            serde_json::json!({"ok": false, "error": e}).to_string()
        }
    };
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Detect the Warren purchase id shape: exactly 32 ASCII hex chars (after
/// trimming), lowercased. Mirrors the desktop daemon's `as_wpid` and
/// `warren_api::providers::normalize_wpid`. Anything else is a regular voucher
/// secret, so the shape alone fully determines the redeem path.
#[cfg(target_os = "android")]
fn as_wpid(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.len() == 32 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

/// Voucher secrets pulled from `GET /v1/checkout/{wpid}/voucher` whose
/// `POST /v1/register` has not landed yet, keyed by wpid. The pull consumes
/// the server-side single-use mapping, so without this a transient register
/// failure would burn a PAID voucher: every subsequent poll would re-pull and
/// 404 forever. In-memory only (mirrors the desktop daemon's
/// `pulled_unregistered`; a process death inside that window loses the secret,
/// accepted residual per doc 35).
#[cfg(target_os = "android")]
static PULLED_UNREGISTERED: Mutex<std::collections::BTreeMap<String, String>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Redeem a voucher OR claim an app-initiated purchase (doc 35).
///
/// Mirrors the desktop daemon's `submit_voucher`: a 32-hex purchase id (wpid)
/// is NOT a voucher, so we first pull the secret the payment webhook queued
/// under it (`GET /v1/checkout/{wpid}/voucher`), then redeem that secret via
/// `POST /v1/register`. A regular voucher (any other shape) is redeemed
/// directly. A wpid whose webhook has not landed yet surfaces `purchase
/// pending` so the Kotlin poll keeps trying within its own deadline.
#[cfg(target_os = "android")]
fn redeem_voucher_inner(mnemonic: &str, voucher_or_wpid: &str) -> Result<u64, String> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| "initLogger must be called before redeemVoucher".to_owned())?;
    let pubkey_ss58 = crate::wallet::pubkey_ss58_from_mnemonic(mnemonic)
        .map_err(|e| format!("invalid mnemonic: {e}"))?;
    let pubkey = warren_api::PubkeySs58::try_from(pubkey_ss58.as_str())
        .map_err(|e| format!("invalid pubkey: {e}"))?;
    let client = unsigned_warren_client();
    let wpid = as_wpid(voucher_or_wpid);

    runtime.block_on(async move {
        let voucher_secret = match &wpid {
            Some(wpid) => {
                // A previous poll may have pulled the secret then failed the
                // register: the server mapping is single-use, so this cache is
                // the only remaining copy of a paid voucher. Reuse before pull.
                let cached = PULLED_UNREGISTERED.lock().get(wpid).cloned();
                match cached {
                    Some(secret) => secret,
                    None => match client
                        .pull_pending_voucher(wpid)
                        .await
                        .map_err(|e| format!("pull pending voucher failed: {e}"))?
                    {
                        Some(secret) => {
                            PULLED_UNREGISTERED
                                .lock()
                                .insert(wpid.clone(), secret.clone());
                            secret
                        }
                        // Webhook not landed yet (or id expired): keep polling.
                        None => return Err("purchase pending".to_owned()),
                    },
                }
            }
            None => voucher_or_wpid.to_owned(),
        };

        // An EMPTY voucher asks the server to redeem its configured
        // auto-voucher (beta onboarding / refresh access): the field is
        // omitted on the wire and the server answers idempotently for a
        // wallet already holding an active auto-voucher subscription.
        let req = warren_api::RegisterAccountRequest {
            pubkey_ss58: pubkey,
            voucher_secret: if voucher_secret.is_empty() {
                None
            } else {
                Some(voucher_secret)
            },
            referral_code: None,
        };
        // The pull consumed the single-use mapping, so a transient register
        // failure would burn the paid voucher: retry transport errors a few
        // times (a server 4xx is final). On a pulled wpid the cache lets the
        // next poll keep retrying the register even past these attempts.
        let mut last_err = None;
        for attempt in 0u32..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            match client.register(&req).await {
                Ok(resp) => {
                    if let Some(wpid) = &wpid {
                        PULLED_UNREGISTERED.lock().remove(wpid);
                    }
                    return Ok(resp.expires_at);
                }
                Err(e @ warren_api::ClientError::ServerStatus { .. }) => {
                    // Final server verdict (e.g. already-redeemed means a prior
                    // attempt landed): drop the cache so the poll stops replaying.
                    if let Some(wpid) = &wpid {
                        PULLED_UNREGISTERED.lock().remove(wpid);
                    }
                    return Err(format!("register failed: {e}"));
                }
                Err(e) => last_err = Some(e),
            }
        }
        // Transport still down: keep the cached secret for the next poll.
        Err(format!(
            "register failed: {}",
            last_err.expect("loop ran at least once")
        ))
    })
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn redeem_voucher_inner(_mnemonic: &str, _voucher_secret: &str) -> Result<u64, String> {
    Err("redeemVoucher is Android-only".to_owned())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pathbuf_from_java(env: &JnixEnv<'_>, path: JObject<'_>) -> PathBuf {
    PathBuf::from(String::from_java(env, path))
}
