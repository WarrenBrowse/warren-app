//! Warren account / subscription FFI for iOS.
//!
//! Real implementations over `warren-api-client`, the same signed
//! warren-api client Android drives via `warren-jni` and the desktop
//! daemon drives via `WarrenRemoteAccountBackend`. These replace the
//! legacy Mullvad account-number REST flows under `src/api_client/`:
//! the Warren identity is the wallet, not a Mullvad account number.
//!
//! Identity model: every call derives the Ed25519 signing key from the
//! 32-byte wallet seed at the FFI boundary (`derive_node_key`), mirroring
//! the seed-centric shape of `warren_wallet_ffi`. The seed never escapes
//! Rust and its stack copy is wrapped in `Zeroizing`.
//!
//! Wire model (lockstep with Android `warren-jni` + daemon backend):
//! - subscription: signed `GET /v1/subscription`  -> `{ expires_at }`
//! - voucher:      unsigned `POST /v1/register`    -> `{ expires_at }`
//! - delete:       signed `DELETE /v1/account`
//!
//! Memory ownership: each call returns a heap `CString` holding a JSON
//! envelope. The caller MUST free it via `warren_wallet_free_mnemonic`
//! (documented type-agnostic: it reclaims any `CString` this crate
//! produces). Envelope shapes:
//! - `{"ok":true,"expires_at":<u64>}`            (subscription / voucher)
//! - `{"ok":true}`                               (delete)
//! - `{"ok":false,"error":"<msg>"}`              (input / transport error)
//! - `{"ok":false,"error":"<msg>","status":<u16>}` (server non-2xx; the
//!    Swift side maps `status` to a localized message; the response body
//!    is deliberately NOT surfaced because a 4xx body can echo request
//!    context).
//!
//! Blocking: each call `block_on`s the shared iOS tokio runtime. The
//! Swift facade (`WarrenAccountClient`) invokes them off the main thread.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde_json::json;
use warren_api_client::{ClientError, PubkeySs58, RegisterAccountRequest, WarrenApiClient};
use warren_identity::{derive_node_key, ss58};
use zeroize::Zeroizing;

const SEED_LEN: usize = 32;

/// Production warren-api base URL (no trailing slash). Kept in lockstep
/// with Android's `PROD_API_URL` (`warren-jni`) and the daemon's
/// `DEFAULT_WARREN_API_URL` so a signed request a phone makes is
/// verifiable by the same backend the other platforms talk to.
const WARREN_API_URL: &str = "https://api.warrenbrowse.com";

/// Reads a 32-byte seed into a zeroizing buffer. Returns `None` when
/// `seed` is null.
///
/// # Safety
/// `seed`, when non-null, must point to at least `SEED_LEN` readable
/// bytes.
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

/// Allocates a heap `CString` from `s`, returning null when `s` contains
/// an interior NUL (never the case for our JSON envelopes).
fn into_cstring(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// `{"ok":true,"expires_at":<u64>}` envelope.
fn ok_expiry_json(expires_at: u64) -> *mut c_char {
    into_cstring(json!({ "ok": true, "expires_at": expires_at }).to_string())
}

/// `{"ok":false,"error":"<msg>"}` envelope for input / runtime errors.
fn err_input_json(msg: &str) -> *mut c_char {
    into_cstring(json!({ "ok": false, "error": msg }).to_string())
}

/// `{"ok":false,...}` envelope for a [`ClientError`]. The server status
/// is surfaced (when present) so Swift can map it to a localized message;
/// the raw response body is never surfaced.
fn err_client_json(err: &ClientError) -> *mut c_char {
    let value = match err {
        ClientError::ServerStatus { status, .. } => {
            json!({ "ok": false, "error": err.to_string(), "status": status })
        }
        _ => json!({ "ok": false, "error": err.to_string() }),
    };
    into_cstring(value.to_string())
}

/// Signed `GET /v1/subscription`. Returns the wallet's subscription
/// expiry as `{"ok":true,"expires_at":<unix secs>}` or an error envelope.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_get_subscription(seed: *const u8) -> *mut c_char {
    // SAFETY: `seed` upholds the documented precondition.
    let Some(seed) = (unsafe { read_seed(seed) }) else {
        return err_input_json("null seed");
    };
    let signing_key = derive_node_key(&seed);
    let handle = match crate::warren_ios_runtime() {
        Ok(handle) => handle,
        Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
    };
    let client = WarrenApiClient::new(WARREN_API_URL.to_owned(), signing_key);
    match handle.block_on(client.get_subscription()) {
        Ok(resp) => ok_expiry_json(resp.expires_at),
        Err(error) => err_client_json(&error),
    }
}

/// Unsigned `POST /v1/register`. Binds the wallet pubkey to a new
/// subscription via a voucher secret. Returns
/// `{"ok":true,"expires_at":<unix secs>}` or an error envelope. The
/// voucher secret is never logged.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes;
/// `voucher`, when non-null, must be a valid null-terminated C string.
/// The returned pointer must be freed once via
/// `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_redeem_voucher(
    seed: *const u8,
    voucher: *const c_char,
) -> *mut c_char {
    // SAFETY: `seed` upholds the documented precondition.
    let Some(seed) = (unsafe { read_seed(seed) }) else {
        return err_input_json("null seed");
    };
    if voucher.is_null() {
        return err_input_json("null voucher");
    }
    // SAFETY: `voucher` is a valid null-terminated C string (precondition).
    let voucher_secret = match unsafe { CStr::from_ptr(voucher) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return err_input_json("voucher is not valid UTF-8"),
    };
    // The pubkey travels in the request body (the endpoint is unsigned),
    // so we derive only the SS58 address, never sending a signature.
    let signing_key = derive_node_key(&seed);
    let address = ss58::encode(&signing_key.verifying_key().to_bytes());
    let pubkey_ss58 = match PubkeySs58::try_from(address.as_str()) {
        Ok(pubkey) => pubkey,
        Err(error) => return err_input_json(&format!("invalid pubkey: {error}")),
    };
    let handle = match crate::warren_ios_runtime() {
        Ok(handle) => handle,
        Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
    };
    let client = WarrenApiClient::new_unsigned(WARREN_API_URL.to_owned());
    let req = RegisterAccountRequest {
        pubkey_ss58,
        voucher_secret,
        referral_code: None,
    };
    match handle.block_on(client.register_with_voucher(&req)) {
        Ok(resp) => ok_expiry_json(resp.expires_at),
        Err(error) => err_client_json(&error),
    }
}

/// Signed `DELETE /v1/account`. Permanently deletes the wallet's
/// subscription server-side. Returns `{"ok":true}` or an error envelope.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_delete(seed: *const u8) -> *mut c_char {
    // SAFETY: `seed` upholds the documented precondition.
    let Some(seed) = (unsafe { read_seed(seed) }) else {
        return err_input_json("null seed");
    };
    let signing_key = derive_node_key(&seed);
    let handle = match crate::warren_ios_runtime() {
        Ok(handle) => handle,
        Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
    };
    let client = WarrenApiClient::new(WARREN_API_URL.to_owned(), signing_key);
    match handle.block_on(client.delete_account()) {
        Ok(()) => into_cstring(json!({ "ok": true }).to_string()),
        Err(error) => err_client_json(&error),
    }
}
