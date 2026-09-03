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

/// The compiled environment's anchor table as the JSON `productAnchorsJson`
/// hands Kotlin: one object whose keys are the columns of
/// `fixtures/client-rules/product_env.json`. The Gradle flavor spells the
/// scheme and the application id again because the manifest needs them
/// before any native code runs; Kotlin holds those copies to this table.
#[must_use]
pub fn product_anchors_json() -> String {
    warren_product_env::CURRENT.anchors_json()
}

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

    /// The table Kotlin decodes is the compiled environment's row, so a
    /// `.so` built for one environment inside an APK flavored for another is
    /// caught by the on-device comparison against `BuildConfig`.
    #[test]
    fn product_anchors_json_is_the_compiled_environments_row() {
        let table: serde_json::Value =
            serde_json::from_str(&super::product_anchors_json()).expect("the table is JSON");
        assert_eq!(table["name"], warren_product_env::ENV_NAME);
        assert_eq!(table["api_url"], super::PRODUCT_API_URL);
        assert_eq!(
            table["deep_link_scheme"],
            warren_product_env::DEEP_LINK_SCHEME
        );
        assert_eq!(table["connect_host"], warren_forum::connect_host());
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
