//! Warren PRODUCT/deployment constants owned by the app.
//!
//! Copied verbatim from `warren-core/crates/warren-config/src/lib.rs` so
//! warren-ios does not depend on warren-core. Kept in lockstep with
//! `mullvad-daemon::warren_product_config` and warren-jni's
//! `PROD_SERVER_PUBKEY_HEX`.

/// Ed25519 public key (64-char hex) of the production `warren-api` server
/// signing key. Used as the multi-hop directory's envelope server-key pin
/// (defense-in-depth on top of the root-anchored operational certificate).
pub const WARREN_SERVER_PUBKEY_HEX: &str =
    "4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e";
