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

/// The anchor tables of every environment that OUTRANKS `current`, strongest
/// first, as a JSON array of the objects `warren_product_anchors` renders one
/// of. `[]` for prod, which outranks everything.
fn higher_priority_anchors_json(current: warren_product_env::ProductEnv) -> String {
    let rows = warren_product_env::environments_with_priority_over(current)
        .into_iter()
        .map(warren_product_env::ProductEnv::anchors_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{rows}]")
}

/// The anchor tables of the environments that outrank the compiled one,
/// strongest first, as a heap-allocated JSON array C string.
///
/// This is the one table Swift cannot derive from `warren_product_anchors`:
/// coexistence needs the URL scheme of ANOTHER install to look for it with
/// `canOpenURL`, and the compiled row only ever names this build. The order
/// and the membership are `warren_product_env::PRECEDENCE`, so no client
/// spells a foreign scheme on its own. Prod gets `[]` and watches nothing.
///
/// The returned pointer must be passed to `warren_product_anchors_free`
/// exactly once.
#[unsafe(no_mangle)]
pub extern "C" fn warren_higher_priority_product_anchors() -> *mut c_char {
    match CString::new(higher_priority_anchors_json(warren_product_env::CURRENT)) {
        Ok(json) => json.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a string previously returned by `warren_product_anchors` or
/// `warren_higher_priority_product_anchors`. No-op on null.
///
/// # Safety
/// `ptr` must have been returned by one of those two renderers and must not
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

    use warren_product_env::ProductEnv;

    use super::{
        warren_higher_priority_product_anchors, warren_product_anchors, warren_product_anchors_free,
    };

    /// Reads a table back through the same two calls Swift makes and frees it.
    fn read_and_free(ptr: *mut std::ffi::c_char) -> String {
        assert!(!ptr.is_null());
        // SAFETY: a non-null pointer from either renderer is a valid C string
        // until freed below.
        let json = unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .expect("the table is UTF-8")
            .to_owned();
        // SAFETY: freed exactly once, after the copy above.
        unsafe { warren_product_anchors_free(ptr) };
        json
    }

    /// The table Swift decodes is the compiled environment's row, read back
    /// through the same two calls Swift makes.
    #[test]
    fn the_ffi_table_is_the_compiled_environments_row() {
        let json = read_and_free(warren_product_anchors());
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

    /// The list Swift watches for is the crate's precedence order, not a
    /// second table spelled in Swift: iOS has to name ANOTHER environment's
    /// URL scheme to look for its app, and the compiled row can only ever name
    /// this build's own.
    #[test]
    fn the_higher_priority_table_is_the_precedence_list_strongest_first() {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&super::higher_priority_anchors_json(ProductEnv::Beta))
                .expect("the table is JSON");

        let names: Vec<&str> = rows
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["prod", "staging"]);
        assert_eq!(
            rows[0]["deep_link_scheme"],
            ProductEnv::Prod.deep_link_scheme()
        );
        assert_eq!(
            rows[1]["deep_link_scheme"],
            ProductEnv::Staging.deep_link_scheme()
        );
    }

    /// Prod outranks everything, so it watches nothing and its stand-down can
    /// never fire.
    #[test]
    fn prod_has_no_higher_priority_table() {
        assert_eq!(super::higher_priority_anchors_json(ProductEnv::Prod), "[]");
    }

    /// The FFI renders the compiled environment's list, read back the way
    /// Swift reads it.
    #[test]
    fn the_higher_priority_ffi_renders_the_compiled_environments_list() {
        let json = read_and_free(warren_higher_priority_product_anchors());
        assert_eq!(
            json,
            super::higher_priority_anchors_json(warren_product_env::CURRENT)
        );
    }

    #[test]
    fn freeing_a_null_table_is_a_no_op() {
        // SAFETY: null is the documented no-op input.
        unsafe { warren_product_anchors_free(std::ptr::null_mut()) };
    }
}
