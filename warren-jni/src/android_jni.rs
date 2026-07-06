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
        sys::{jbyteArray, jint, jstring},
    },
};
use tokio::sync::oneshot;

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

/// Active tunnel handle. Populated by `connectTunnel`, cleared by
/// `disconnectTunnel`. Only one tunnel at a time on Android (parity with
/// upstream VpnService model).
static ACTIVE_TUNNEL: Mutex<Option<TunnelHandle>> = Mutex::new(None);

/// Atomic mirror of the active session status (cf.
/// [`crate::tunnel::SessionStatus`]). Read by [`getTunnelStatus`] without
/// taking the `ACTIVE_TUNNEL` mutex, so polling from Kotlin is cheap.
static SESSION_STATUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Latest NAT-PMP port-forwarding status as a JSON string, polled by
/// Kotlin via `getNatPmpStatus()`. Empty = idle (no active mapping). The
/// session-side NAT-PMP refresh loop writes transitions here via
/// [`set_natpmp_status`]; teardown clears it via [`reset_natpmp_status`].
/// `parking_lot::Mutex::new` is const, so no lazy init is needed.
static NATPMP_STATUS: Mutex<String> = Mutex::new(String::new());

/// Store the latest NAT-PMP status JSON. Called from the session refresh
/// loop. `pub(crate)` so [`crate::tunnel`] can reach it.
#[cfg_attr(not(all(target_os = "android", feature = "tunnel")), allow(dead_code))]
pub(crate) fn set_natpmp_status(json: String) {
    *NATPMP_STATUS.lock() = json;
}

/// Reset the NAT-PMP status to idle (empty). Called on teardown.
#[cfg_attr(not(all(target_os = "android", feature = "tunnel")), allow(dead_code))]
pub(crate) fn reset_natpmp_status() {
    NATPMP_STATUS.lock().clear();
}

/// Opaque handle stored while a tunnel is alive. The cancel sender lets
/// `disconnectTunnel` tear down the Quinn task gracefully. With the
/// `tunnel` feature on, we additionally pin the spawned task handle so
/// it stays alive after the JNI entry returns.
struct TunnelHandle {
    _cancel_tx: oneshot::Sender<()>,
    #[cfg(feature = "tunnel")]
    _task: tokio::task::JoinHandle<()>,
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
    // Bridge the `log` crate to logcat. Once initialised, `log::info!`
    // from any Rust dep (warren-identity, warren-tunnel once enabled,
    // etc.) shows up under `adb logcat -s WarrenJni:V`. The
    // `init_once` form is idempotent so JNI callers can re-trigger
    // `initLogger` (e.g. after a process restart) without panicking.
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("WarrenJni"),
    );
    // Problem reports read native logs on demand via `collectReport`
    // (logcat dump of our own tag). A future enhancement could add a
    // rotating file appender at <log_dir>/warren.log for retention beyond
    // the logcat ring buffer, but that is not required for reports to work.
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
    let phrase = String::from_java(&env, mnemonic);
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
    let phrase = String::from_java(&env, mnemonic);
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
    let phrase = String::from_java(&env, mnemonic);
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
    timestamp: jnix::jni::sys::jlong,
    nonce_hex: JString<'local>,
    body_hash_hex: JString<'local>,
) -> jbyteArray {
    let env = JnixEnv::from(env);
    let phrase = String::from_java(&env, mnemonic);
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

/// Sign and submit a forum-login challenge for `sid` to the connect `host`
/// (`POST /v1/forum/login`).
///
/// Mirrors the desktop daemon's SignForumLogin plus its POST: only the opaque
/// `sid` and the connect `host` cross the boundary. The mnemonic derives the
/// signing key for this call and is not retained; the wallet signature never
/// surfaces to Kotlin because the request is signed AND sent here (like every
/// other signed JNI call). `host` is checked against a hard allowlist so a
/// hostile deep link cannot redirect the signed request. Returns `{"ok":true}`
/// on acceptance, `{"ok":false,"error":"subscription-required"}` when the wallet
/// has never subscribed (HTTP 403), or `{"ok":false,"error":"error"}`. The
/// mnemonic, sid, signature and nonce are never logged.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_forumLogin<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    sid: JString<'local>,
    host: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = String::from_java(&jnix_env, mnemonic);
    let sid = String::from_java(&jnix_env, sid);
    let host = String::from_java(&jnix_env, host);
    let outcome = forum_login(&phrase, &sid, &host);
    let json = crate::forum::envelope(outcome);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Drive the signed forum-login POST: build the signed request (host allowlist +
/// wire bytes, all in `crate::forum`), execute it on the shared runtime through
/// the reqwest transport, and map the HTTP status to an outcome. Never logs any
/// request material; a failure to build or send collapses to `Failed`.
fn forum_login(mnemonic: &str, sid: &str, host: &str) -> crate::forum::ForumLoginOutcome {
    use crate::forum::ForumLoginOutcome;
    use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};

    let Some(runtime) = RUNTIME.get() else {
        log::warn!("forumLogin: initLogger must run first");
        return ForumLoginOutcome::Failed;
    };
    let signed = match crate::forum::build_signed_request(mnemonic, sid, host) {
        Ok(req) => req,
        Err(_) => {
            // No values: the cause could otherwise echo sid/host/mnemonic state.
            log::warn!("forumLogin: could not build signed request");
            return ForumLoginOutcome::Failed;
        }
    };
    let request = HttpRequest {
        method: Method::Post,
        url: signed.url,
        headers: signed.headers,
        body: signed.body,
        use_sni: true,
    };
    match runtime.block_on(ReqwestTransport::new().execute(request)) {
        Ok(response) => crate::forum::outcome_for_status(response.status),
        Err(_) => {
            log::warn!("forumLogin: transport error");
            ForumLoginOutcome::Failed
        }
    }
}

/// Best-effort: tell the connect provider the user declined the forum login
/// (`POST /v1/session/<sid>/cancel`) so the waiting browser page unblocks
/// instead of polling to timeout. Unsigned (no wallet material); mirrors the
/// desktop `cancelForumLogin`. Failures are ignored: the server session expires
/// on its own in 10 minutes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_forumLoginCancel<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    sid: JString<'local>,
    host: JString<'local>,
) {
    let jnix_env = JnixEnv::from(env);
    let sid = String::from_java(&jnix_env, sid);
    let host = String::from_java(&jnix_env, host);
    forum_cancel(&sid, &host);
}

fn forum_cancel(sid: &str, host: &str) {
    use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};

    let Some(url) = crate::forum::build_cancel_url(sid, host) else {
        return;
    };
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let request = HttpRequest {
        method: Method::Post,
        url,
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    // Best-effort: a failed cancel just means the browser polls to timeout.
    let _ = runtime.block_on(ReqwestTransport::new().execute(request));
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
        use std::sync::atomic::Ordering;
        // Wrap the raw mnemonic in Zeroizing so its heap allocation is
        // wiped when it goes out of scope (guaranteed even on early-return paths).
        use zeroize::Zeroizing;

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
        // Reset status to Connecting before spawning so the Kotlin side can
        // poll it deterministically right after this JNI returns.
        SESSION_STATUS.store(1, Ordering::SeqCst);

        let task = runtime.spawn(crate::tunnel::run_session(
            tun,
            signing_key,
            config,
            &SESSION_STATUS,
            cancel_rx,
        ));

        *slot = Some(TunnelHandle {
            _cancel_tx: cancel_tx,
            _task: task,
        });
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
        // Dropping `_cancel_tx` notifies the Quinn task to wind down. The
        // task itself flips `SESSION_STATUS` back to Disconnected when its
        // `tokio::select!` falls through; we also do it here for symmetry
        // so the status flip is observable even before the task wakes.
        SESSION_STATUS.store(0, std::sync::atomic::Ordering::SeqCst);
        reset_natpmp_status();
    }
}

/// Returns the latest NAT-PMP port-forwarding status as a JSON string.
/// Shape: `{"state":"idle"|"requesting"|"mapped"|"rate_limited"|"failed",
/// "external_port":u16?, "lifetime_secs":u32?, "retry_after_secs":u16?,
/// "reason":"..."?}`. Polled by Kotlin alongside `getTunnelStatus()`.
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

/// Returns 0 = disconnected, 1 = connecting, 2 = connected, 3 = reconnecting.
/// Matches the `WarrenTunnelState` Kotlin enum. Reads `SESSION_STATUS`
/// without taking the `ACTIVE_TUNNEL` mutex so polling from Kotlin is
/// cheap.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getTunnelStatus(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    SESSION_STATUS.load(std::sync::atomic::Ordering::SeqCst)
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

/// Fetch the raw signed multi-hop directory (`GET /v1/multihop/directory`) and
/// return its verbatim JSON, or an empty string on any failure.
///
/// Called from Kotlin BEFORE the VpnService TUN is established so the request
/// egresses the physical network. A fetch issued from inside the tunnel
/// session (after the TUN is up) is captured by the half-open tunnel and
/// blackholed - which is exactly why `run_multi_hop_session` must NOT fetch
/// this itself but receive the blob through the tunnel config. The blob's
/// signature + version are verified Rust-side in `run_multi_hop_session`.
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

/// Builds a `WarrenApiClient` for the unsigned, no-mnemonic-available
/// endpoints (`GET /v1/exits`, `GET /v1/multihop/directory`). The identity
/// is only used by SIGNED methods, so a fixed placeholder is harmless here
/// and documents the no-sign contract (mirrors warren-core's
/// `WarrenApiClient::new_unsigned` zero-key sentinel).
#[cfg(target_os = "android")]
fn unsigned_warren_client()
-> warren_api::WarrenApiClient<warren_api::reqwest_transport::ReqwestTransport> {
    warren_api::WarrenApiClient::new(
        PROD_API_URL.to_owned(),
        warren_identity::WarrenIdentity::from_seed(&[0u8; 32]),
        warren_api::reqwest_transport::ReqwestTransport::new(),
    )
}

/// Raw signed multi-hop directory, or empty string on failure.
#[cfg(target_os = "android")]
fn fetch_multihop_directory_raw() -> String {
    let Some(runtime) = RUNTIME.get() else {
        log::warn!("fetchMultihopDirectory called before initLogger; returning empty");
        return String::new();
    };
    let client = unsigned_warren_client();
    match runtime.block_on(client.fetch_multihop_directory()) {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            log::warn!("fetchMultihopDirectory: no directory published (404)");
            String::new()
        }
        Err(e) => {
            log::warn!("fetchMultihopDirectory: fetch failed: {e}");
            String::new()
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn fetch_multihop_directory_raw() -> String {
    String::new()
}

/// Production warren-api base URL. The Android build does NOT support
/// `--api-override` runtime switching (that path is gated behind the
/// `api-override` Cargo feature for dev/staging builds only).
pub(crate) const PROD_API_URL: &str = "https://api.warrenbrowse.com";

/// Production server signing pubkey hex (64 lowercase chars). The
/// signed relay list MUST be signed by this key; any other pubkey is
/// rejected. The companion seed lives on the prod warren-api Docker
/// volume at `/var/lib/docker/volumes/warren_warren-api-data/_data/api-signing.key`
/// (read-only access requires hcloud `warren` context SSH to
/// `warren-backend-api`).
///
/// Verified against the live `GET /v1/exits` response on
/// 2026-05-23 - the embedded `server_pubkey_hex` field matches this
/// constant byte-for-byte, and deriving the verifying key from the
/// on-disk seed via `ed25519_dalek::SigningKey::from_bytes` produces
/// the same hex.
///
/// Rotation procedure: bump this constant, push a new app build,
/// THEN swap the seed file on the server. Doing it in the reverse
/// order locks existing clients out of `/v1/exits` until they update.
pub(crate) const PROD_SERVER_PUBKEY_HEX: Option<&str> =
    Some("4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e");

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

    let signed = match warren_discovery_core::verify_signed_relay_list(&raw, PROD_SERVER_PUBKEY_HEX)
    {
        Ok(s) => s,
        Err(_) => {
            log::warn!("listRelays: signature verify failed; returning cached/empty");
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

/// Return whether the running app version is still supported, i.e. allowed to
/// keep running. `1` = supported, `0` = must force-update.
///
/// The Kotlin side feeds this into `VersionInfo.isSupported`, which drives the
/// "unsupported version" banner (deep-linking to the store) and the forced-update
/// gate. This reuses the exact desktop verifier (`mullvad-update`): it fetches
/// `android.json` over the Let's-Encrypt-pinned host, verifies the ed25519
/// signature against the embedded trusted pubkey, then applies the shared
/// `minimum_supported_version` rule (`is_current_version_supported`).
///
/// Fail-open: any transient failure (runtime not ready, unparseable version,
/// network, signature) returns `1` so a flaky network never locks the user out.
/// Only a successfully verified manifest can mark the running version
/// unsupported.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_checkVersionSupported<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    current_version: JString<'local>,
) -> jint {
    let jnix_env = JnixEnv::from(env);
    let current = String::from_java(&jnix_env, current_version);
    if check_app_version_supported(&current) {
        1
    } else {
        0
    }
}

fn check_app_version_supported(current_version: &str) -> bool {
    use mullvad_update::api::{HttpVersionInfoProvider, MetaRepositoryPlatform};
    use mullvad_update::version::{MIN_VERIFY_METADATA_VERSION, is_current_version_supported};

    let runtime = match RUNTIME.get() {
        Some(rt) => rt,
        None => {
            log::warn!("checkVersionSupported called before initLogger; assuming supported");
            return true;
        }
    };
    let current: mullvad_version::Version = match current_version.parse() {
        Ok(version) => version,
        Err(err) => {
            log::warn!("checkVersionSupported: unparseable version '{current_version}': {err}");
            return true;
        }
    };
    let response = match runtime.block_on(HttpVersionInfoProvider::get_versions_for_platform(
        MetaRepositoryPlatform::Android,
        MIN_VERIFY_METADATA_VERSION,
    )) {
        Ok(response) => response,
        Err(_) => {
            log::warn!("checkVersionSupported: manifest fetch/verify failed; assuming supported");
            return true;
        }
    };
    is_current_version_supported(&current, &response.signed)
}

/// Return the version STRING of the newest stable release strictly greater than
/// `current_version`, or an EMPTY string when there is none / on ANY error.
///
/// This drives the sideload-only "update available" in-app notification. Unlike
/// [`checkVersionSupported`] (which is fail-OPEN so a flaky network never locks
/// a user out), this prompt is fail-CLOSED: we would rather show no update than
/// a false one, so every failure path collapses to `""`. The Kotlin side maps
/// `""` to "no update known".
///
/// The fetch + verify discipline mirrors `checkVersionSupported`: same pinned
/// host, same ed25519 verification against the embedded trusted pubkey, same
/// shared tokio runtime.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_latestAvailableVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    current_version: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let current = String::from_java(&jnix_env, current_version);
    let latest = latest_available_version(&current).unwrap_or_default();
    match jnix_env.new_string(latest) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Fetch + verify the signed manifest and return the newest stable version
/// strictly above `current_version`, or `None` on any error / when none exists.
fn latest_available_version(current_version: &str) -> Option<String> {
    use mullvad_update::api::{HttpVersionInfoProvider, MetaRepositoryPlatform};
    use mullvad_update::version::MIN_VERIFY_METADATA_VERSION;

    let runtime = match RUNTIME.get() {
        Some(rt) => rt,
        None => {
            log::warn!("latestAvailableVersion called before initLogger; no update");
            return None;
        }
    };
    let current: mullvad_version::Version = match current_version.parse() {
        Ok(version) => version,
        Err(err) => {
            log::warn!("latestAvailableVersion: unparseable version '{current_version}': {err}");
            return None;
        }
    };
    let response = match runtime.block_on(HttpVersionInfoProvider::get_versions_for_platform(
        MetaRepositoryPlatform::Android,
        MIN_VERIFY_METADATA_VERSION,
    )) {
        Ok(response) => response,
        Err(_) => {
            log::warn!("latestAvailableVersion: manifest fetch/verify failed; no update");
            return None;
        }
    };
    newest_stable_above(&current, &response.signed).map(|v| v.to_string())
}

/// Pure selector: the maximum release version in `signed` that is stable
/// (no pre-stable qualifier, not a dev build) AND strictly greater than
/// `current`. Returns `None` when no such release exists.
///
/// Kept free of any I/O so it is unit-testable without a network or runtime.
fn newest_stable_above(
    current: &mullvad_version::Version,
    signed: &mullvad_update::format::response::Response,
) -> Option<mullvad_version::Version> {
    signed
        .releases
        .iter()
        .map(|release| &release.version)
        .filter(|version| version.pre_stable.is_none() && !version.is_dev())
        .filter(|version| *version > current)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
}

/// Submit a problem report. Called by the Android
/// `ProblemReportRepository` once the user taps "Send". The Kotlin
/// side passes:
///   - `mnemonic`: BIP39 phrase to derive the signing key.
///   - `user_message`: free-form user description.
///   - `redacted_logs`: pre-redacted log bundle (or "" when the
///     user unchecked "Include logs").
///   - `app_version`, `platform`: free-form context strings.
///
/// Returns a JSON object `{"ok": bool, "reference_id": "...",
/// "error": "..."}`. `reference_id` is present iff `ok == true`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_sendProblemReport<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    user_message: JString<'local>,
    redacted_logs: JString<'local>,
    app_version: JString<'local>,
    platform: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = String::from_java(&jnix_env, mnemonic);
    let user_message = String::from_java(&jnix_env, user_message);
    let redacted_logs = String::from_java(&jnix_env, redacted_logs);
    let app_version = String::from_java(&jnix_env, app_version);
    let platform = String::from_java(&jnix_env, platform);

    let result = send_problem_report_inner(
        &phrase,
        &user_message,
        &redacted_logs,
        &app_version,
        &platform,
    );
    let json = match result {
        Ok(reference_id) => {
            serde_json::json!({"ok": true, "reference_id": reference_id}).to_string()
        }
        Err(e) => {
            // Audit follow-up: do NOT embed the reqwest/hyper error
            // chain in the logcat line. A 4xx response body from
            // warren-api can carry a fragment of the user_message /
            // redacted_logs back to the client; logging it would
            // re-leak that content under the `WarrenJni` logcat tag,
            // which any app holding READ_LOGS can read. The full
            // string is still returned to Kotlin via JSON.
            log::warn!("sendProblemReport failed");
            serde_json::json!({"ok": false, "error": e}).to_string()
        }
    };
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "android")]
fn send_problem_report_inner(
    mnemonic: &str,
    user_message: &str,
    redacted_logs: &str,
    app_version: &str,
    platform: &str,
) -> Result<String, String> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| "initLogger must be called before sendProblemReport".to_owned())?;
    let identity = warren_identity::WarrenIdentity::from_mnemonic(mnemonic)
        .map_err(|e| format!("invalid mnemonic: {e}"))?;
    let client = warren_api::WarrenApiClient::new(
        PROD_API_URL.to_owned(),
        identity,
        warren_api::reqwest_transport::ReqwestTransport::new(),
    );
    let req = warren_api::SupportReportRequest {
        user_message: user_message.to_owned(),
        redacted_logs: redacted_logs.to_owned(),
        app_version: app_version.to_owned(),
        platform: platform.to_owned(),
    };
    let resp = runtime
        .block_on(client.submit_support_report(&req))
        .map_err(|e| format!("submit failed: {e}"))?;
    Ok(resp.reference_id)
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn send_problem_report_inner(
    _mnemonic: &str,
    _user_message: &str,
    _redacted_logs: &str,
    _app_version: &str,
    _platform: &str,
) -> Result<String, String> {
    Err("sendProblemReport is Android-only".to_owned())
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
    let phrase = String::from_java(&jnix_env, mnemonic);
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
    let client = warren_api::WarrenApiClient::new(
        PROD_API_URL.to_owned(),
        identity,
        warren_api::reqwest_transport::ReqwestTransport::new(),
    );
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
    let phrase = String::from_java(&jnix_env, mnemonic);
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

        let req = warren_api::RegisterAccountRequest {
            pubkey_ss58: pubkey,
            voucher_secret,
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

/// Collect a log bundle for a problem report by dumping THIS app's own
/// logcat for the `WarrenJni` tag. This is read on demand (only when the
/// user files a report), so it never touches the boot-critical logger
/// init. An app can read its own UID's logs without `READ_LOGS`, and the
/// codebase never logs secrets (mnemonics / keys are redacted at the
/// boundary), so the dump is safe to ship. Returns an empty string when
/// logcat is unavailable; the Kotlin side already handles that shape.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_collectReport(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let logs = collect_native_logs();
    match env.new_string(&logs) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Dump the last lines of this process's own logcat for the `WarrenJni`
/// tag. `-d` dumps and exits (non-blocking); `-t` caps the line count so
/// the bundle stays bounded; `WarrenJni:V *:S` keeps only our tag. Any
/// failure degrades to whatever was captured (or empty), never a panic.
fn collect_native_logs() -> String {
    const MAX_LINES: &str = "4000";
    match std::process::Command::new("logcat")
        .args(["-d", "-v", "time", "-t", MAX_LINES, "WarrenJni:V", "*:S"])
        .output()
    {
        Ok(out) => {
            if !out.status.success() {
                log::warn!("collectReport: logcat exited non-zero");
            }
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        Err(_) => {
            log::warn!("collectReport: logcat unavailable");
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pathbuf_from_java(env: &JnixEnv<'_>, path: JObject<'_>) -> PathBuf {
    PathBuf::from(String::from_java(env, path))
}

#[cfg(test)]
mod version_select_tests {
    use super::newest_stable_above;
    use mullvad_update::format::release::Release;
    use mullvad_update::format::response::Response;

    fn version(s: &str) -> mullvad_version::Version {
        s.parse().expect("test version must parse")
    }

    fn release(s: &str) -> Release {
        Release {
            version: version(s),
            changelog: String::new(),
            installers: Vec::new(),
            rollout: mullvad_update::version::rollout::Rollout::complete(),
        }
    }

    fn response(versions: &[&str]) -> Response {
        Response {
            releases: versions.iter().map(|v| release(v)).collect(),
            ..Response::default()
        }
    }

    #[test]
    fn newer_stable_present_returns_some() {
        let signed = response(&["1.0.0", "1.2.0", "1.3.0"]);
        let got = newest_stable_above(&version("1.0.0"), &signed);
        assert_eq!(got, Some(version("1.3.0")));
    }

    #[test]
    fn only_same_or_older_returns_none() {
        let signed = response(&["1.0.0", "0.9.0"]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn empty_release_list_returns_none() {
        let signed = response(&[]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn pre_release_and_dev_above_current_are_ignored() {
        // A higher beta and a higher dev build are present, but neither is a
        // stable release, so no update should be reported.
        let signed = response(&["1.1.0-beta1", "1.2.0-dev-abc123"]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn picks_max_stable_ignoring_higher_prerelease() {
        // 1.4.0-beta1 sorts above 1.3.0 but is not stable; the newest STABLE
        // strictly above current is 1.3.0.
        let signed = response(&["1.3.0", "1.4.0-beta1", "1.1.0"]);
        assert_eq!(
            newest_stable_above(&version("1.0.0"), &signed),
            Some(version("1.3.0"))
        );
    }
}
