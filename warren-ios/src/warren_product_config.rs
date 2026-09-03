//! Warren PRODUCT/deployment constants owned by the app.
//!
//! The server pubkey is copied verbatim from
//! `warren-core/crates/warren-config/src/lib.rs` so warren-ios does not depend
//! on warren-core, and kept in lockstep with
//! `mullvad-daemon::warren_product_config` and warren-jni's
//! `SERVER_PUBKEY_HEX`. Every per-environment anchor comes from the
//! `warren-product-env` crate and reaches Swift as one JSON table.

use std::ffi::{CString, c_char};

/// Ed25519 public key (64-char hex) of the production `warren-api` server
/// signing key. Used as the multi-hop directory's envelope server-key pin
/// (defense-in-depth on top of the root-anchored operational certificate).
pub const WARREN_SERVER_PUBKEY_HEX: &str =
    "4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e";

/// The compiled product environment's anchor table as a heap-allocated JSON
/// C string: one object whose keys are the columns of
/// `fixtures/client-rules/product_env.json` (`deep_link_scheme`,
/// `connect_host`, `forum_public_url`, `api_url`, `application_id`, ...), so
/// Swift reads the scheme and the hosts from the Rust reference instead of
/// spelling them again. Null only if the table could not be rendered, which
/// the crate's own tests rule out.
///
/// The returned pointer must be passed to `warren_product_anchors_free`
/// exactly once.
#[unsafe(no_mangle)]
pub extern "C" fn warren_product_anchors() -> *mut c_char {
    match CString::new(warren_product_env::CURRENT.anchors_json()) {
        Ok(json) => json.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a string previously returned by `warren_product_anchors`. No-op on
/// null.
///
/// # Safety
/// `ptr` must have been returned by `warren_product_anchors` and must not
/// have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_product_anchors_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr` came from `CString::into_raw` and is not yet freed (fn precondition).
    drop(unsafe { CString::from_raw(ptr) });
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{warren_product_anchors, warren_product_anchors_free};

    /// The table Swift decodes is the compiled environment's row, read back
    /// through the same two calls Swift makes.
    #[test]
    fn the_ffi_table_is_the_compiled_environments_row() {
        let ptr = warren_product_anchors();
        assert!(!ptr.is_null());
        // SAFETY: a non-null pointer from `warren_product_anchors` is a valid
        // C string until freed below.
        let json = unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .expect("the table is UTF-8")
            .to_owned();
        // SAFETY: freed exactly once, after the copy above.
        unsafe { warren_product_anchors_free(ptr) };

        let table: serde_json::Value = serde_json::from_str(&json).expect("the table is JSON");
        assert_eq!(table["name"], warren_product_env::ENV_NAME);
        assert_eq!(table["api_url"], warren_product_env::API_URL);
        assert_eq!(
            table["deep_link_scheme"],
            warren_product_env::DEEP_LINK_SCHEME
        );
        assert_eq!(table["connect_host"], warren_product_env::CONNECT_HOST);
        assert_eq!(
            table["forum_public_url"],
            warren_product_env::FORUM_PUBLIC_URL
        );
    }

    #[test]
    fn freeing_a_null_table_is_a_no_op() {
        // SAFETY: null is the documented no-op input.
        unsafe { warren_product_anchors_free(std::ptr::null_mut()) };
    }
}
