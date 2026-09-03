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
//! crate produces). Envelope shapes match Android's `WarrenJni.forumLogin`:
//! `{"ok":true}` / `{"ok":false,"error":"subscription-required"}` /
//! `{"ok":false,"error":"clock-skew"}` / `{"ok":false,"error":"error"}`.
//!
//! Blocking: `block_on`s the shared iOS tokio runtime; the Swift facade invokes
//! it off the main thread.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};
use warren_identity::WarrenIdentity;
use zeroize::Zeroizing;

use crate::forum::{self, FailReason, ForumLoginOutcome};

const SEED_LEN: usize = 32;

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
/// Derives the `WarrenIdentity` from the 32-byte wallet `seed`, builds the
/// signed `POST /v1/forum/login` request (host allowlist + sid shape checked in
/// `crate::forum`), sends it, and returns the outcome envelope. Any input,
/// build, or transport failure collapses to `{"ok":false,"error":"error"}`; a
/// server 403 maps to `subscription-required`. Nothing about the request (seed,
/// sid, signature, nonce) is ever logged.
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

        let identity = WarrenIdentity::from_seed(&seed);
        let signed = match forum::build_signed_request(&identity, &sid, &host) {
            Ok(req) => req,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build)),
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Runtime)),
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
