use std::{
    ffi::{CString, c_char, c_void},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use shadowsocks::crypto::available_ciphers;
use talpid_types::net::proxy::{Shadowsocks, ShadowsocksCipher, Socks5Remote, SocksAuth};

use super::get_string;

/// Constructs a new IP address from a pointer containing bytes representing an IP address.
///
/// SAFETY: `addr` pointer must be non-null, aligned, and point to at least addr_len bytes
pub(crate) unsafe fn parse_ip_addr(addr: *const u8, addr_len: usize) -> Option<IpAddr> {
    match addr_len {
        4 => {
            // SAFETY: `addr` pointer must be non-null, aligned, and point to at least addr_len bytes
            let bytes = unsafe { std::slice::from_raw_parts(addr, addr_len) };
            Some(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).into())
        }
        16 => {
            // SAFETY: `addr` pointer must be non-null, aligned, and point to at least addr_len bytes
            let bytes = unsafe { std::slice::from_raw_parts(addr, addr_len) };
            let mut addr_arr = [0u8; 16];
            addr_arr.as_mut_slice().copy_from_slice(bytes);

            Some(Ipv6Addr::from(addr_arr).into())
        }
        anything_else => {
            log::error!("Bad IP address length {anything_else}");
            None
        }
    }
}

/// Converts parameters into a boxed `Shadowsocks` configuration that is safe
/// to send across the FFI boundary
///
/// # SAFETY
/// `address` must be a pointer to at least `address_len` bytes.
/// `c_password` and `c_cipher` must be pointers to null terminated strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_shadowsocks_access_method_setting(
    address: *const u8,
    address_len: usize,
    port: u16,
    c_password: *const c_char,
    c_cipher: *const c_char,
) -> *const c_void {
    // SAFETY: this function's `# Safety` section requires `address` to point to at least
    // `address_len` readable bytes, which is what `parse_ip_addr` needs; it only reads 4 or 16
    // of them and rejects any other length.
    let endpoint: SocketAddr =
        if let Some(ip_address) = unsafe { parse_ip_addr(address, address_len) } {
            SocketAddr::new(ip_address, port)
        } else {
            return std::ptr::null();
        };

    // SAFETY: this function's `# Safety` section requires `c_password` to be a NUL-terminated
    // string valid for this call (null is read as empty); `get_string` copies it before returning.
    let password = unsafe { get_string(c_password) };
    // SAFETY: this function's `# Safety` section requires `c_cipher` to be a NUL-terminated string
    // valid for this call (null is read as empty); `get_string` copies it before returning.
    let cipher = unsafe { get_string(c_cipher) };
    // A caller-supplied cipher name must not panic the FFI: an unknown
    // name fails closed with a null return instead of unwinding across
    // the C ABI.
    let Ok(cipher) = ShadowsocksCipher::new(&cipher) else {
        return std::ptr::null();
    };

    let shadowsocks_configuration = Shadowsocks {
        endpoint,
        password,
        cipher,
    };

    Box::into_raw(Box::new(shadowsocks_configuration)) as *mut c_void
}

/// Converts parameters into a boxed `Socks5Remote` configuration that is safe
///
/// to send across the FFI boundary
///
/// # SAFETY
/// `address` must be a pointer to at least `address_len` bytes.
/// `c_username` and `c_password` must be pointers to null terminated strings, or null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_socks5_access_method_setting(
    address: *const u8,
    address_len: usize,
    port: u16,
    c_username: *const c_char,
    c_password: *const c_char,
) -> *const c_void {
    // SAFETY: this function's `# Safety` section requires `address` to point to at least
    // `address_len` readable bytes, which is what `parse_ip_addr` needs; it only reads 4 or 16
    // of them and rejects any other length.
    let endpoint: SocketAddr =
        if let Some(ip_address) = unsafe { parse_ip_addr(address, address_len) } {
            SocketAddr::new(ip_address, port)
        } else {
            return std::ptr::null();
        };

    let auth = {
        if c_username.is_null() || c_password.is_null() {
            None
        } else {
            // SAFETY: `c_username` was checked non-null above, and this function's `# Safety`
            // section requires it to be a NUL-terminated string valid for this call; `get_string`
            // copies it.
            let username = unsafe { get_string(c_username) };
            // SAFETY: `c_password` was checked non-null above, and this function's `# Safety`
            // section requires it to be a NUL-terminated string valid for this call; `get_string`
            // copies it.
            let password = unsafe { get_string(c_password) };
            SocksAuth::new(username, password).ok()
        }
    };

    let socks5_configuration = Socks5Remote { endpoint, auth };
    Box::into_raw(Box::new(socks5_configuration)) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn get_shadowsocks_chipers() -> *mut libc::c_char {
    let ciphers_string = available_ciphers().join(",");
    let ciphers_c_string = CString::new(ciphers_string).unwrap_or_default();

    ciphers_c_string.into_raw()
}

/// Deallocates a CString returned by the Mullvad API client.
///
/// # Safety
///
/// `cstr_ptr` must be a pointer to a string allocated by another `mullvad_api` function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mullvad_api_cstring_drop(cstr_ptr: *mut libc::c_char) {
    // SAFETY: the caller guarantees that `cstr_ptr` came from `CString::into_raw` in this library
    // and has not been freed yet, so rebuilding the `CString` frees it exactly once with its
    // original length.
    let _ = unsafe { CString::from_raw(cstr_ptr) };
}
