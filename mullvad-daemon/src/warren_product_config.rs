//! Warren PRODUCT/deployment constants owned by the app.
//!
//! The app (not the backend) is the Warren product, so it owns its own
//! deployment constants. These must match the production warren-api
//! deployment (its URL and signing pubkey); keep them in lockstep with
//! `warren-ios::warren_product_config` and warren-jni's `PROD_API_URL` /
//! `PROD_SERVER_PUBKEY_HEX`.
//!
//! The generic (non-product) `unix_now` helper lives in the neutral engine
//! crate `warrenguard_config` and is consumed directly from there.

/// URL of the production Warren HTTP API.
pub const WARREN_API_URL: &str = "https://api.warrenbrowse.com";

/// Ed25519 public key (64-char hex) of the production `warren-api` server
/// signing key, i.e. the key that signs the `SignedRelayList` served at
/// `GET {WARREN_API_URL}/v1/exits`. Clients pin this key when verifying the
/// fetched / baked exit list, so a compromised or impersonating API serving
/// a list signed by a different key is rejected.
pub const WARREN_SERVER_PUBKEY_HEX: &str =
    "4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e";
