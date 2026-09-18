//! Incident telemetry FFI for iOS: the signed report a user files when an exit
//! serves a key other than the one this device pinned.
//!
//! iOS counterpart of Android's `warren-jni` `reportPubkeyMismatch`. The exit
//! key-change alert's "Report to Warren" button opened a static FAQ page on
//! this client, so the report the button names never left the device and the
//! operator feed never learned of the mismatch, while the desktop daemon and
//! Android both post it.
//!
//! The request shape, the validation and the outcome envelope are the shared
//! [`warren_incidents`] crate's, so neither mobile client can drift from the
//! other; this layer only reads the FFI inputs, signs and POSTs on the shared
//! iOS runtime, and hands back a JSON envelope `CString`.
//!
//! Memory ownership: the returned heap `CString` MUST be freed once via
//! `warren_wallet_free_mnemonic` (type-agnostic: it reclaims any `CString` this
//! crate produces).
//!
//! No-log: the seed never leaves this module, the server's body is never
//! rendered (it may echo identity material), and a failure is reported as one
//! of the shared crate's coarse classes.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

use warren_api::ReqwestTransport;
use warren_identity::WarrenIdentity;
use warren_incidents::NotSent;
use zeroize::Zeroizing;

const SEED_LEN: usize = 32;

/// Files the signed pubkey-mismatch report for the exit the user was just
/// warned about.
///
/// Returns the shared envelope, `{"ok":true}` or `{"ok":false,"reason":"…"}`,
/// as a heap `CString` the caller frees once with
/// `warren_wallet_free_mnemonic`. Never null except on allocation failure.
///
/// # Safety
///
/// - `seed` points to at least 32 readable bytes, or is null.
/// - Every other pointer is a valid NUL-terminated UTF-8 C string, or null.
/// - None of the pointers need outlive the call: everything is copied.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_report_pubkey_mismatch(
    seed: *const u8,
    exit_id_hex: *const c_char,
    old_pubkey_hex: *const c_char,
    new_pubkey_hex: *const c_char,
    country_code: *const c_char,
    city: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (
            Some(seed),
            Some(exit_id_hex),
            Some(old_pubkey_hex),
            Some(new_pubkey_hex),
            Some(country_code),
            Some(city),
        ) = (
            unsafe { read_seed(seed) },
            unsafe { cstr_to_string(exit_id_hex) },
            unsafe { cstr_to_string(old_pubkey_hex) },
            unsafe { cstr_to_string(new_pubkey_hex) },
            unsafe { cstr_to_string(country_code) },
            unsafe { cstr_to_string(city) },
        )
        else {
            return envelope_cstring(Err(NotSent::Malformed));
        };

        let request = warren_incidents::pubkey_mismatch_request(
            &exit_id_hex,
            &old_pubkey_hex,
            &new_pubkey_hex,
            &country_code,
            &city,
            now_unix(),
        );
        envelope_cstring(send(&seed, &request))
    })
}

/// Signs and POSTs one report. The identity is built per call so the seed
/// never lingers, and the transport per call because the network under it has
/// just changed, the same reasoning `warren-jni`'s `incident_client` gives.
fn send(
    seed: &[u8; SEED_LEN],
    request: &warren_api::IncidentPubkeyMismatchRequest,
) -> Result<(), NotSent> {
    let handle = crate::warren_ios_runtime().map_err(|_| NotSent::Runtime)?;
    let identity = WarrenIdentity::from_seed(seed);
    let client = warren_api::WarrenApiClient::new(
        warren_product_env::API_URL.to_owned(),
        identity,
        ReqwestTransport::new(),
    );
    handle
        .block_on(client.report_pubkey_mismatch(request))
        .map_err(client_error_class)
}

/// A client failure as one of the shared report classes. The error itself is
/// never rendered: a server body may echo identity material.
fn client_error_class(error: warren_api::ClientError) -> NotSent {
    match error {
        warren_api::ClientError::ServerStatus { .. } => NotSent::Rejected,
        _ => NotSent::Transport,
    }
}

fn envelope_cstring(outcome: Result<(), NotSent>) -> *mut c_char {
    CString::new(warren_incidents::envelope(outcome))
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// # Safety
///
/// `seed` points to at least [`SEED_LEN`] readable bytes, or is null.
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

/// # Safety
///
/// `p` is a valid NUL-terminated C string, or null.
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
