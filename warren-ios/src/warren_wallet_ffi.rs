//! Warren wallet FFI for iOS (BIP39 mnemonic + Ed25519 signing).
//!
//! Real implementations over `warren-identity` + `bip39`. The Swift side
//! (`ios/WarrenRustRuntime/WarrenWallet.swift`) wraps these exports behind
//! an idiomatic Swift API.
//!
//! Memory ownership : strings returned by `warren_wallet_generate_mnemonic`
//! are heap-allocated `CString`s ; the caller must free them via
//! `warren_wallet_free_mnemonic`. Buffers passed by the caller for
//! `out_seed` / `out_pubkey` / `out_signature` must be at least the
//! documented size ; the FFI writes to them on success.
//!
//! Return codes (`i32`) :
//! - `0` : success
//! - `-1` : invalid input (null pointer, malformed UTF-8, bad mnemonic, …)
//! - `-2` : internal error (signing, HKDF, …)

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use bip39::{Language, Mnemonic};
use ed25519_dalek::{Signer, SigningKey};
use warren_identity::{derive_node_key, seed_from_mnemonic};

const SEED_LEN: usize = 32;
const PUBKEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

const RC_OK: c_int = 0;
const RC_INVALID_INPUT: c_int = -1;

/// Generates a new BIP39 mnemonic with `word_count` words (12 or 24).
///
/// Returns a heap-allocated C string. Caller MUST free via
/// `warren_wallet_free_mnemonic`. Returns null on error (invalid
/// word_count or RNG failure).
///
/// # Safety
/// The returned pointer must be passed back to
/// `warren_wallet_free_mnemonic` exactly once. Reading the string after
/// freeing is undefined behaviour.
#[unsafe(no_mangle)]
pub extern "C" fn warren_wallet_generate_mnemonic(word_count: u32) -> *mut c_char {
    // BIP39 supports 12, 15, 18, 21, 24 words. Warren UI offers only 12
    // (default) and 24 (advanced).
    let count = match word_count {
        12 | 15 | 18 | 21 | 24 => word_count as usize,
        _ => return std::ptr::null_mut(),
    };
    let mnemonic = match Mnemonic::generate_in(Language::English, count) {
        Ok(m) => m.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };
    match CString::new(mnemonic) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a mnemonic string previously returned by
/// `warren_wallet_generate_mnemonic`. No-op on null.
///
/// # Safety
/// `ptr` must have been returned by `warren_wallet_generate_mnemonic`
/// and must not have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_wallet_free_mnemonic(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Reconstitute the CString to drop it (frees the heap allocation).
    drop(unsafe { CString::from_raw(ptr) });
}

/// Derives the Warren identity 32-byte seed from a BIP39 mnemonic.
///
/// `mnemonic` : null-terminated UTF-8 BIP39 phrase (12 or 24 words).
/// `out_seed` : caller-provided buffer of at least 32 bytes ; written
/// on success.
///
/// Returns `0` on success, `-1` on invalid input.
///
/// # Safety
/// `mnemonic` must point to a valid null-terminated C string.
/// `out_seed` must point to a writable buffer of at least 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_wallet_seed_from_mnemonic(
    mnemonic: *const c_char,
    out_seed: *mut u8,
) -> c_int {
    if mnemonic.is_null() || out_seed.is_null() {
        return RC_INVALID_INPUT;
    }
    let cstr = unsafe { CStr::from_ptr(mnemonic) };
    let s = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return RC_INVALID_INPUT,
    };
    let seed = match seed_from_mnemonic(s) {
        Ok(seed) => seed,
        Err(_) => return RC_INVALID_INPUT,
    };
    // Safety: out_seed is at least SEED_LEN bytes (precondition).
    unsafe {
        std::ptr::copy_nonoverlapping(seed.as_ptr(), out_seed, SEED_LEN);
    }
    // seed Zeroizing drop wipes the internal buffer.
    RC_OK
}

/// Derives the Ed25519 public key from a 32-byte seed.
///
/// `seed` : caller-provided buffer of 32 bytes.
/// `out_pubkey` : caller-provided buffer of at least 32 bytes ; written
/// on success.
///
/// Returns `0` on success, `-1` on invalid input.
///
/// # Safety
/// Both `seed` and `out_pubkey` must point to writable buffers of at
/// least 32 bytes each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_wallet_derive_pubkey(
    seed: *const u8,
    out_pubkey: *mut u8,
) -> c_int {
    if seed.is_null() || out_pubkey.is_null() {
        return RC_INVALID_INPUT;
    }
    // Safety: seed is at least SEED_LEN bytes (precondition).
    let seed_arr: [u8; SEED_LEN] = unsafe {
        let mut arr = [0u8; SEED_LEN];
        std::ptr::copy_nonoverlapping(seed, arr.as_mut_ptr(), SEED_LEN);
        arr
    };
    let signing_key = derive_node_key(&seed_arr);
    let pubkey = signing_key.verifying_key().to_bytes();
    // Safety: out_pubkey is at least PUBKEY_LEN bytes (precondition).
    unsafe {
        std::ptr::copy_nonoverlapping(pubkey.as_ptr(), out_pubkey, PUBKEY_LEN);
    }
    RC_OK
}

/// Signs an arbitrary payload with the Ed25519 signing key derived
/// from `seed`.
///
/// `seed` : 32-byte seed buffer.
/// `payload` : pointer to `payload_len` bytes.
/// `out_signature` : caller-provided buffer of at least 64 bytes ;
/// written on success.
///
/// Returns `0` on success, `-1` on invalid input, `-2` on internal
/// signing error.
///
/// # Safety
/// All pointers must be non-null and point to buffers of the documented
/// sizes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_wallet_sign(
    seed: *const u8,
    payload: *const u8,
    payload_len: usize,
    out_signature: *mut u8,
) -> c_int {
    if seed.is_null() || (payload.is_null() && payload_len != 0) || out_signature.is_null() {
        return RC_INVALID_INPUT;
    }
    // Safety: seed is SEED_LEN bytes (precondition).
    let seed_arr: [u8; SEED_LEN] = unsafe {
        let mut arr = [0u8; SEED_LEN];
        std::ptr::copy_nonoverlapping(seed, arr.as_mut_ptr(), SEED_LEN);
        arr
    };
    let signing_key: SigningKey = derive_node_key(&seed_arr);
    let payload_slice: &[u8] = if payload_len == 0 {
        &[]
    } else {
        // Safety: payload has at least payload_len bytes (precondition).
        unsafe { std::slice::from_raw_parts(payload, payload_len) }
    };
    let signature = signing_key.sign(payload_slice);
    let sig_bytes = signature.to_bytes();
    // Safety: out_signature is at least SIGNATURE_LEN bytes (precondition).
    unsafe {
        std::ptr::copy_nonoverlapping(sig_bytes.as_ptr(), out_signature, SIGNATURE_LEN);
    }
    RC_OK
}
