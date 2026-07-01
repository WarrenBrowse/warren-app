//! Warren account / subscription FFI for iOS.
//!
//! Real implementations over the SDK's `warren_api::WarrenApiClient`
//! (warren-sdk-rs), the same signed warren-api client Android drives via
//! `warren-jni` and the desktop daemon drives via
//! `WarrenRemoteAccountBackend`. These replace the legacy Mullvad
//! account-number REST flows under `src/api_client/`: the Warren identity
//! is the wallet, not a Mullvad account number.
//!
//! Identity model: every call derives the `WarrenIdentity` from the
//! 32-byte wallet seed at the FFI boundary (`WarrenIdentity::from_seed`),
//! mirroring the seed-centric shape of `warren_wallet_ffi`. The seed never
//! escapes Rust and its stack copy is wrapped in `Zeroizing`. Each call
//! builds a fresh client (stateless FFI boundary, no hot-swap needed: the
//! caller always hands in the current seed).
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
//!   Swift side maps `status` to a localized message; the response body
//!   is deliberately NOT surfaced because a 4xx body can echo request
//!   context).
//!
//! Blocking: each call `block_on`s the shared iOS tokio runtime. The
//! Swift facade (`WarrenAccountClient`) invokes them off the main thread.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde_json::json;
use warren_api::reqwest_transport::ReqwestTransport;
use warren_api::{ClientError, PubkeySs58, RegisterAccountRequest, WarrenApiClient};
use warren_identity::WarrenIdentity;
use zeroize::Zeroizing;

const SEED_LEN: usize = 32;

/// Production warren-api base URL (no trailing slash). Kept in lockstep
/// with Android's `PROD_API_URL` (`warren-jni`) and the daemon's
/// `warren_product_config::WARREN_API_URL` so a signed request a phone
/// makes is verifiable by the same backend the other platforms talk to.
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

/// Builds a `WarrenApiClient` around the seed-derived identity. Cheap: no
/// hot-swap to worry about since the FFI boundary is stateless (the
/// caller hands in the current seed on every call).
fn client_for_seed(seed: &[u8; SEED_LEN]) -> WarrenApiClient<ReqwestTransport> {
    WarrenApiClient::new(
        WARREN_API_URL.to_owned(),
        WarrenIdentity::from_seed(seed),
        ReqwestTransport::new(),
    )
}

/// Signed `GET /v1/subscription`. Returns the wallet's subscription
/// expiry as `{"ok":true,"expires_at":<unix secs>}` or an error envelope.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_get_subscription(seed: *const u8) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: `seed` upholds the documented precondition.
        let Some(seed) = (unsafe { read_seed(seed) }) else {
            return err_input_json("null seed");
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
        };
        let client = client_for_seed(&seed);
        match handle.block_on(client.subscription()) {
            Ok(resp) => ok_expiry_json(resp.expires_at),
            Err(error) => err_client_json(&error),
        }
    })
}

/// Signed `POST /v1/payments/apple/init`. Mints an ephemeral payment
/// session bound to the wallet pubkey and returns the session UUID the
/// app must pass to StoreKit as the `appAccountToken`. Returns
/// `{"ok":true,"app_account_token":"<uuid>"}` or an error envelope.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_storekit_init(seed: *const u8) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: `seed` upholds the documented precondition.
        let Some(seed) = (unsafe { read_seed(seed) }) else {
            return err_input_json("null seed");
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
        };
        let client = client_for_seed(&seed);
        match handle.block_on(client.init_apple_payment()) {
            Ok(resp) => into_cstring(
                json!({ "ok": true, "app_account_token": resp.app_account_token }).to_string(),
            ),
            Err(error) => err_client_json(&error),
        }
    })
}

/// Signed `POST /v1/payments/apple/check`. Uploads the StoreKit 2
/// signed transaction JWS so the backend can verify it against Apple's
/// root CA and credit the wallet's subscription. Returns
/// `{"ok":true,"expires_at":<unix secs>}` or an error envelope. The JWS
/// is never logged.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes;
/// `jws`, when non-null, must be a valid null-terminated C string. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_storekit_check(
    seed: *const u8,
    jws: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: `seed` upholds the documented precondition.
        let Some(seed) = (unsafe { read_seed(seed) }) else {
            return err_input_json("null seed");
        };
        if jws.is_null() {
            return err_input_json("null jws");
        }
        // SAFETY: `jws` is a valid null-terminated C string (precondition).
        let jws_transaction = match unsafe { CStr::from_ptr(jws) }.to_str() {
            Ok(s) => s.to_owned(),
            Err(_) => return err_input_json("jws is not valid UTF-8"),
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
        };
        let client = client_for_seed(&seed);
        match handle.block_on(client.check_apple_payment(&jws_transaction)) {
            Ok(resp) => ok_expiry_json(resp.expires_at),
            Err(error) => err_client_json(&error),
        }
    })
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
    crate::ffi_guard(std::ptr::null_mut(), || {
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
        let identity = WarrenIdentity::from_seed(&seed);
        let pubkey_ss58 = match PubkeySs58::try_from(identity.address().as_str()) {
            Ok(pubkey) => pubkey,
            Err(error) => return err_input_json(&format!("invalid pubkey: {error}")),
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
        };
        // `register` is unsigned (the pubkey travels in the body); the
        // client identity is only used by SIGNED methods, so building it
        // from the real seed here is harmless and avoids a separate
        // unsigned-only client type.
        let client = client_for_seed(&seed);
        let req = RegisterAccountRequest {
            pubkey_ss58,
            voucher_secret,
            referral_code: None,
        };
        match handle.block_on(client.register(&req)) {
            Ok(resp) => ok_expiry_json(resp.expires_at),
            Err(error) => err_client_json(&error),
        }
    })
}

/// Signed `DELETE /v1/account`. Permanently deletes the wallet's
/// subscription server-side. Returns `{"ok":true}` or an error envelope.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes. The
/// returned pointer must be freed once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_account_delete(seed: *const u8) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: `seed` upholds the documented precondition.
        let Some(seed) = (unsafe { read_seed(seed) }) else {
            return err_input_json("null seed");
        };
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(error) => return err_input_json(&format!("runtime unavailable: {error}")),
        };
        let client = client_for_seed(&seed);
        match handle.block_on(client.delete_account()) {
            Ok(()) => into_cstring(json!({ "ok": true }).to_string()),
            Err(error) => err_client_json(&error),
        }
    })
}
