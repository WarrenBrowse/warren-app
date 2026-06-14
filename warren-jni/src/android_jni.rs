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
// NOTE (D.3 scope): wallet primitives (`generateMnemonic`,
// `importMnemonic`, `signRequest`) call into real warren-identity code via
// the pure-rust [`crate::wallet`] module - those work today and are
// covered by host tests. The tunnel lifecycle JNI exports remain stubs;
// see `.planning/session-d-d3-warren-jni-design.md` for the D.4 plan.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

// Fix M-3: parking_lot::Mutex never poisons on panic, unlike std::sync::Mutex.
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

// Native-side error taxonomy reserved for the D.4 tunnel lifecycle work.
// Variants are unused until the connectTunnel body actually produces them;
// keeping the enum here documents the surface the JNI callers should expect
// to receive via `throw`. The `expect` form (rather than `allow`) makes
// `cargo clippy -D warnings` fail loudly the day a variant goes from "still
// unused" to "wrongly removed".
#[expect(dead_code, reason = "D.4 tunnel lifecycle work in progress")]
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
/// EncryptedSharedPreferences (D.5).
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
        // Fix L-2: wrap the raw mnemonic in Zeroizing so its heap allocation is
        // wiped when it goes out of scope (guaranteed even on early-return paths).
        use zeroize::Zeroizing;

        if tun_fd < 0 {
            let _ = jnix_env.throw(format!("invalid tun_fd: {tun_fd}"));
            return -1;
        }
        // SAFETY: Kotlin passes a freshly `detachFd()`-ed fd whose ownership
        // is now transferred to us. The fd is closed when AndroidTun drops.
        let owned: OwnedFd = unsafe { OwnedFd::from_raw_fd(tun_fd) };
        let tun = match warren_tunnel::AndroidTun::from_fd(owned) {
            Ok(t) => t,
            Err(e) => {
                let _ = jnix_env.throw(format!("AndroidTun::from_fd failed: {e}"));
                return -1;
            }
        };

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
                let _ = jnix_env
                    .throw("initLogger() must be called before connectTunnel()");
                return -1;
            }
        };

        // Fix L-2: derive the signing key synchronously at the JNI boundary,
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

/// Installs the process-wide [`warren_tunnel::socket_protect`] protector
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
    let protector: warren_tunnel::socket_protect::SocketProtector =
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
    warren_tunnel::socket_protect::set_protector(protector);
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
/// D.6 wired: fetches `GET /v1/exits` via `warren-api-client`, verifies
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

/// Hardcoded fallback used when the live fetch fails. Schema lined up
/// with the Kotlin `RelayInfo` data class. `exit_id` + `exit_pubkey_hex`
/// are 32-char (16-byte) operator-assigned identifiers, NOT Ed25519
/// pubkeys (which would be 64-char).
const FALLBACK_RELAYS_JSON: &str = r#"[{"exit_id":"2921abad869e94064b56cf48c8da3631","exit_pubkey_hex":"2921abad869e94064b56cf48c8da3631","endpoint":"warren-exit-1.warren.brown:443","country":"DE","city":"Falkenstein","active":true,"weight":100}]"#;

/// Fetch + verify the signed relay list, projecting the result to the
/// Kotlin-side JSON shape. Returns the fallback on any error.
#[cfg(target_os = "android")]
fn fetch_relays_or_fallback() -> String {
    let runtime = match RUNTIME.get() {
        Some(rt) => rt,
        None => {
            log::warn!("listRelays called before initLogger; returning fallback");
            return FALLBACK_RELAYS_JSON.to_owned();
        }
    };

    // `/v1/exits` is unsigned (response is server-signed); use the
    // unsigned-only client constructor so the API surface documents
    // the no-sign contract and we never accidentally emit a request
    // signed with a deterministic zero key.
    let client = warren_api_client::WarrenApiClient::new_unsigned(PROD_API_URL.to_owned());

    let raw = match runtime.block_on(client.list_exits()) {
        Ok(s) => s,
        Err(_) => {
            log::warn!("listRelays: GET /v1/exits failed; returning fallback");
            return FALLBACK_RELAYS_JSON.to_owned();
        }
    };

    let signed = match warren_relay_selector::verify_signed_relay_list(
        &raw,
        PROD_SERVER_PUBKEY_HEX,
    ) {
        Ok(s) => s,
        Err(_) => {
            log::warn!("listRelays: signature verify failed; returning fallback");
            return FALLBACK_RELAYS_JSON.to_owned();
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

    serde_json::to_string(&projected).unwrap_or_else(|_| FALLBACK_RELAYS_JSON.to_owned())
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn fetch_relays_or_fallback() -> String {
    FALLBACK_RELAYS_JSON.to_owned()
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

/// Submit a problem report (D.6). Called by the Android
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
    let signing_key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|e| format!("invalid mnemonic: {e}"))?;
    let client = warren_api_client::WarrenApiClient::new(PROD_API_URL.to_owned(), signing_key);
    let req = warren_api_client::SupportReportRequest {
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
    Client(warren_api_client::ClientError),
}

/// Build the `{"ok":false,...}` envelope, surfacing `status` on a server
/// non-2xx so Kotlin can distinguish 404 (no subscription) from a real
/// failure. Mirrors the iOS `err_client_json` (warren-ios account FFI).
#[cfg(target_os = "android")]
fn subscription_error_envelope(err: &SubscriptionFetchError) -> String {
    use warren_api_client::ClientError;
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
    let signing_key = crate::wallet::signing_key_from_mnemonic(mnemonic)
        .map_err(|e| SubscriptionFetchError::Setup(format!("invalid mnemonic: {e}")))?;
    let client = warren_api_client::WarrenApiClient::new(PROD_API_URL.to_owned(), signing_key);
    let resp = runtime
        .block_on(client.get_subscription())
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
            log::warn!("redeemVoucher failed");
            serde_json::json!({"ok": false, "error": e}).to_string()
        }
    };
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "android")]
fn redeem_voucher_inner(mnemonic: &str, voucher_secret: &str) -> Result<u64, String> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| "initLogger must be called before redeemVoucher".to_owned())?;
    let pubkey_ss58 = crate::wallet::pubkey_ss58_from_mnemonic(mnemonic)
        .map_err(|e| format!("invalid mnemonic: {e}"))?;
    let pubkey = warren_api_client::PubkeySs58::try_from(pubkey_ss58.as_str())
        .map_err(|e| format!("invalid pubkey: {e}"))?;
    let client = warren_api_client::WarrenApiClient::new_unsigned(PROD_API_URL.to_owned());
    let req = warren_api_client::RegisterAccountRequest {
        pubkey_ss58: pubkey,
        voucher_secret: voucher_secret.to_owned(),
        referral_code: None,
    };
    let resp = runtime
        .block_on(client.register_with_voucher(&req))
        .map_err(|e| format!("register failed: {e}"))?;
    Ok(resp.expires_at)
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
