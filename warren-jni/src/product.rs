//! Warren product/deployment constants for the Android client.
//!
//! Host-compiled (unlike `android_jni`) so the drift gate below runs in the
//! routine `cargo test` scope. The Android build does NOT support runtime
//! API switching in release builds (the `api-override` Cargo feature is
//! dev/staging only), so the compiled product environment is the mechanism
//! that points a beta APK at the beta backend: the Gradle `beta` flavor
//! exports `WARREN_PRODUCT_ENV=beta` to the cargo build.

/// warren-api base URL of the compiled product environment
/// (`WARREN_PRODUCT_ENV`: prod | staging | beta, prod when unset).
pub const PRODUCT_API_URL: &str = warren_product_env::API_URL;

/// Server signing pubkey hex (64 lowercase chars). The signed relay list
/// MUST be signed by this key; any other pubkey is rejected. The same key
/// signs every product environment (they alias one stack), so this does
/// not vary with `WARREN_PRODUCT_ENV`. The companion seed lives on the
/// prod warren-api Docker volume at
/// `/var/lib/docker/volumes/warren_warren-api-data/_data/api-signing.key`
/// (read-only access requires hcloud `warren` context SSH to
/// `warren-backend-api`).
///
/// Rotation procedure: bump this constant, push a new app build,
/// THEN swap the seed file on the server. Doing it in the reverse
/// order locks existing clients out of `/v1/exits` until they update.
pub const SERVER_PUBKEY_HEX: Option<&str> =
    Some("4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e");

#[cfg(test)]
mod product_env_drift_gate {
    use warren_product_env::ProductEnv;

    #[test]
    fn api_url_matches_the_compiled_environment() {
        assert_eq!(super::PRODUCT_API_URL, warren_product_env::API_URL);
        if warren_product_env::CURRENT == ProductEnv::Prod {
            assert_eq!(
                super::PRODUCT_API_URL,
                warren_contract::product::API_URL,
                "a prod build's API URL drifted from the canonical contract anchor"
            );
        }
    }

    #[test]
    fn server_pubkey_is_environment_independent_and_matches_the_contract() {
        assert_eq!(
            super::SERVER_PUBKEY_HEX,
            Some(warren_contract::product::SERVER_PUBKEY_HEX),
            "the pinned server pubkey drifted from the canonical contract anchor"
        );
    }
}
