//! Community-forum wallet login FFI for iOS (`POST /v1/forum/login`, doc 55).
//!
//! iOS counterpart of Android's `warren-jni` `forumLogin`: signs AND POSTs the
//! challenge entirely in Rust, so only the opaque `sid` and the connect `host`
//! cross the boundary and the wallet signature never surfaces to Swift. The
//! wire bytes + validation + outcome mapping live in the host-tested
//! [`crate::forum`] module; this layer only reads the FFI inputs, executes the
//! POST on the shared iOS runtime through the reqwest transport, and returns a
//! JSON envelope `CString`.
//!
//! Memory ownership: the returned heap `CString` MUST be freed once via
//! `warren_wallet_free_mnemonic` (type-agnostic: it reclaims any `CString` this
//! crate produces). Envelope shapes match Android's `WarrenJni.forumLogin`,
//! the `login` table of `fixtures/client-rules/forum_outcomes.json`:
//! `{"ok":true,"handle":"..","notify_slot":n}` (both additive) /
//! `{"ok":false,"error":"subscription-required"|"clock-skew"|"expired"}` /
//! `{"ok":false,"error":"error","reason":"<class>"}`.
//!
//! Blocking: `block_on`s the shared iOS tokio runtime; the Swift facade invokes
//! it off the main thread.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::Duration;

use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};
use warren_identity::WarrenIdentity;
use zeroize::Zeroizing;

use crate::forum::{self, FailReason, ForumLoginOutcome, SessionPreflight};

const SEED_LEN: usize = 32;

/// The connect and total timeouts of the status read, the SDK transport's own
/// (5 s connect, 15 s total), so the preflight can never outlast the POST it
/// precedes.
const PREFLIGHT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PREFLIGHT_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

/// Reads the session's status once before signing (`GET /v1/session/{sid}/status`,
/// unsigned). The `Date` header of that TLS-authenticated answer is the
/// trusted clock a device that never synchronised its own is corrected
/// against, which turns the 2026-08-18 class (every attempt refused by the
/// broker's 60 s window) into a login that works; a 404 names a dead session
/// before a signature is spent. The classing is the shared crate's, the one
/// Android applies. Any failure of the read itself is `Unknown`: the request
/// is then stamped with the device clock and the provider decides, as before
/// the preflight existed. Nothing about the request is logged.
async fn preflight(sid: &str, host: &str) -> SessionPreflight {
    let Some(url) = forum::build_status_url(sid, host) else {
        return SessionPreflight::Unknown;
    };
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(PREFLIGHT_CONNECT_TIMEOUT)
        .timeout(PREFLIGHT_TOTAL_TIMEOUT)
        .build()
    else {
        return SessionPreflight::Unknown;
    };
    match client.get(&url).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let date = response
                .headers()
                .get(reqwest::header::DATE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let device_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let verdict = forum::classify_status_preflight(status, date.as_deref(), device_now);
            if let SessionPreflight::Pending { offset_secs } = verdict
                && offset_secs.abs() > 30
            {
                log::warn!(
                    "forumLogin: device clock is {offset_secs} s off the connect host, correcting"
                );
            }
            verdict
        }
        Err(_) => {
            log::warn!("forumLogin: status preflight failed (transport), signing anyway");
            SessionPreflight::Unknown
        }
    }
}

/// Reads a 32-byte seed into a zeroizing buffer. `None` when `seed` is null.
///
/// # Safety
/// `seed`, when non-null, must point to at least `SEED_LEN` readable bytes.
unsafe fn read_seed(seed: *const u8) -> Option<Zeroizing<[u8; SEED_LEN]>> {
    if seed.is_null() {
        return None;
    }
    let mut buf = Zeroizing::new([0u8; SEED_LEN]);
    // SAFETY: `seed` points to at least SEED_LEN readable bytes (precondition).
    unsafe {
        std::ptr::copy_nonoverlapping(seed, buf.as_mut_ptr(), SEED_LEN);
    }
    Some(buf)
}

/// Reads a NUL-terminated C string into an owned `String`, or `None` if null or
/// not valid UTF-8.
///
/// # Safety
/// `p`, when non-null, must be a valid NUL-terminated C string.
unsafe fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a valid NUL-terminated C string (precondition).
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

/// Allocates the JSON envelope `CString` for `outcome`. `forum::envelope`
/// builds JSON from fixed tokens and a proquint handle, so it never carries an
/// interior NUL and this never fails in practice.
fn envelope_cstring(outcome: ForumLoginOutcome) -> *mut c_char {
    match CString::new(forum::envelope(&outcome)) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Sign and submit a forum-login challenge for `sid` to the connect `host`.
///
/// Derives the `WarrenIdentity` from the 32-byte wallet `seed`, reads the
/// session's status once (a dead session is `expired` without a signature
/// spent; the answer's `Date` corrects the device clock), builds the signed
/// `POST /v1/forum/login` request at the corrected time (host allowlist + sid
/// shape checked in `crate::forum`), sends it, and returns the outcome
/// envelope: `{"ok":true,...}` with the forum identity the broker handed back,
/// `subscription-required` on 403, `clock-skew` on connect's 401 token,
/// `expired` on 404, `error` with a `reason` class for anything else (input,
/// build, runtime, transport, an unnamed status). Nothing about the request
/// (seed, sid, signature, nonce) is ever logged.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes; `sid` and
/// `host` must be valid NUL-terminated C strings. The returned pointer must be
/// freed exactly once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_login(
    seed: *const u8,
    sid: *const c_char,
    host: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(seed), Some(sid), Some(host)) = (
            unsafe { read_seed(seed) },
            unsafe { cstr_to_string(sid) },
            unsafe { cstr_to_string(host) },
        ) else {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        };

        if !warren_forum::is_allowed_connect_host(&host) || !warren_forum::is_valid_sid(&sid) {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        }
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Runtime)),
        };
        let offset_secs = match handle.block_on(preflight(&sid, &host)) {
            SessionPreflight::Pending { offset_secs } => offset_secs,
            SessionPreflight::Gone => return envelope_cstring(ForumLoginOutcome::Expired),
            SessionPreflight::Unknown => 0,
        };
        let Some(timestamp) = forum::timestamp_with_offset(offset_secs) else {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        };
        let identity = WarrenIdentity::from_seed(&seed);
        let signed = match forum::build_signed_request_at(&identity, &sid, &host, timestamp) {
            Ok(req) => req,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build)),
        };

        let request = HttpRequest {
            method: Method::Post,
            url: signed.url,
            headers: signed.headers,
            body: signed.body,
            use_sni: true,
        };
        let outcome = match handle.block_on(ReqwestTransport::new().execute(request)) {
            Ok(response) => forum::outcome_for_response(response.status, &response.body),
            Err(_) => ForumLoginOutcome::Failed(FailReason::Transport),
        };
        envelope_cstring(outcome)
    })
}

/// Best-effort: notify the connect `host` that the user declined the forum login
/// for `sid` (`POST /v1/session/<sid>/cancel`), so the waiting browser page
/// unblocks instead of polling to timeout. Unsigned (no seed / wallet material);
/// mirrors the desktop `cancelForumLogin`. Failures are ignored (connect drops
/// a login session on its own after 5 minutes, the `pending_ttl_secs.login` of
/// `fixtures/client-rules/forum_link.json`). Blocking; call off the main thread.
///
/// # Safety
/// `sid` and `host` must be valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_cancel(sid: *const c_char, host: *const c_char) {
    crate::ffi_guard((), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(sid), Some(host)) = (unsafe { cstr_to_string(sid) }, unsafe {
            cstr_to_string(host)
        }) else {
            return;
        };
        let Some(url) = forum::build_cancel_url(&sid, &host) else {
            return;
        };
        let Ok(handle) = crate::warren_ios_runtime() else {
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
        let _ = handle.block_on(ReqwestTransport::new().execute(request));
    })
}
