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

/// URL of the Warren HTTP API for the compiled product environment
/// (`WARREN_PRODUCT_ENV`: prod | staging | beta, prod when unset).
pub const WARREN_API_URL: &str = warren_product_env::API_URL;

/// Ed25519 public key (64-char hex) of the production `warren-api` server
/// signing key, i.e. the key that signs the `SignedRelayList` served at
/// `GET {WARREN_API_URL}/v1/exits`. Clients pin this key when verifying the
/// fetched / baked exit list, so a compromised or impersonating API serving
/// a list signed by a different key is rejected.
pub const WARREN_SERVER_PUBKEY_HEX: &str =
    "4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e";

#[cfg(test)]
mod product_env_drift_gate {
    //! Lockstep gates for the compiled product environment. The canonical
    //! anchor for PROD stays `warren_contract::product` (owned by the
    //! contract repo); non-prod environments are app-local and anchored in
    //! `warren-product-env`, so this gate pins the prod case against the
    //! contract and every case against the environment table.

    use warren_product_env::ProductEnv;

    #[test]
    fn api_url_matches_the_compiled_environment() {
        assert_eq!(
            super::WARREN_API_URL,
            warren_product_env::API_URL,
            "the daemon API URL must come from the compiled product environment"
        );
        if warren_product_env::CURRENT == ProductEnv::Prod {
            assert_eq!(
                super::WARREN_API_URL,
                warren_contract::product::API_URL,
                "a prod build's API URL drifted from the canonical contract anchor"
            );
        }
    }

    #[test]
    fn server_pubkey_is_environment_independent_and_matches_the_contract() {
        // Every environment is an alias of the same signed stack, so the
        // signing key never varies per environment.
        assert_eq!(
            super::WARREN_SERVER_PUBKEY_HEX,
            warren_contract::product::SERVER_PUBKEY_HEX,
            "the pinned server pubkey drifted from the canonical contract anchor"
        );
    }
}

#[cfg(test)]
mod ios_endpoint_drift_gate {
    //! The iOS API endpoint config is a hand-maintained pair of xcconfig files,
    //! not generated: `ios/Configurations/Api.xcconfig.template` takes the API
    //! host from `ios/Configurations/ProductEnv.xcconfig`, whose per-environment
    //! table is selected by `WARREN_PRODUCT_ENV` (prod by default). The single
    //! source of truth for product anchors is `warren_contract::product`. This
    //! gate runs in the routine Warren test scope and fails the moment the
    //! shipped iOS API host drifts from that anchor, so a shipped iOS build
    //! pointing back at Mullvad's upstream endpoint cannot silently return.
    //! (`warren-product-env`'s lockstep suite holds the whole table to the
    //! environment crate; this gate holds the Release build's resolved host to
    //! the contract.)

    /// The iOS API xcconfig template, resolved relative to this crate.
    const API_XCCONFIG_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ios/Configurations/Api.xcconfig.template"
    );

    /// The iOS product-environment xcconfig (tracked, no template).
    const PRODUCT_ENV_XCCONFIG_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ios/Configurations/ProductEnv.xcconfig"
    );

    /// Host component of an `https://host[:port]/...` URL.
    fn url_host(url: &str) -> &str {
        url.trim_start_matches("https://")
            .trim_start_matches("http://")
            .split(['/', ':'])
            .next()
            .unwrap_or("")
    }

    /// Value of a `KEY = ...` line in the xcconfig (the exact key, no
    /// `[config=...]` condition), trimmed.
    fn value<'a>(xcconfig: &'a str, key: &str) -> Option<&'a str> {
        xcconfig.lines().find_map(|line| {
            let rest = line.trim().strip_prefix(key)?.trim_start();
            Some(rest.strip_prefix('=')?.trim())
        })
    }

    /// True iff some line sets `key` under a `[config=Release]` condition.
    fn has_release_override(xcconfig: &str, key: &str) -> bool {
        let needle = format!("{key}[config=Release]");
        xcconfig
            .lines()
            .any(|line| line.trim().starts_with(&needle))
    }

    #[test]
    fn ios_release_api_host_matches_the_product_anchor() {
        let api = std::fs::read_to_string(API_XCCONFIG_PATH)
            .expect("iOS Api.xcconfig.template must exist at the expected path");
        let product_env = std::fs::read_to_string(PRODUCT_ENV_XCCONFIG_PATH)
            .expect("iOS ProductEnv.xcconfig must exist at the expected path");

        // The API host follows the product environment, never a build
        // configuration of its own.
        assert_eq!(
            value(&api, "API_HOST_NAME"),
            Some("$(WARREN_API_HOST)"),
            "API_HOST_NAME must resolve through WARREN_API_HOST in Api.xcconfig.template"
        );
        assert!(
            !has_release_override(&api, "API_HOST_NAME"),
            "API_HOST_NAME must not be overridden per build configuration"
        );
        // The Release configuration resolves the default environment.
        let default_env = value(&product_env, "WARREN_PRODUCT_ENV")
            .expect("WARREN_PRODUCT_ENV must have an unconditional default in ProductEnv.xcconfig");
        assert!(
            !has_release_override(&product_env, "WARREN_PRODUCT_ENV"),
            "the Release configuration must build the default product environment"
        );
        let resolved_api_host = value(&product_env, &format!("WARREN_API_HOST_{default_env}"))
            .unwrap_or_else(|| {
                panic!("WARREN_API_HOST_{default_env} must be present in ProductEnv.xcconfig")
            });

        let anchor_host = url_host(warren_contract::product::API_URL);

        assert_eq!(
            resolved_api_host, anchor_host,
            "iOS Release API host ({resolved_api_host}) drifted from the product anchor \
             warren_contract::product::API_URL host ({anchor_host}); update ProductEnv.xcconfig \
             (or the anchor) so they match"
        );
    }

    #[test]
    fn ios_xcconfig_ships_no_mullvad_infrastructure() {
        for path in [API_XCCONFIG_PATH, PRODUCT_ENV_XCCONFIG_PATH] {
            let xcconfig = std::fs::read_to_string(path)
                .expect("the iOS xcconfig must exist at the expected path");
            assert!(
                !xcconfig.to_ascii_lowercase().contains("mullvad"),
                "{path}: the iOS xcconfig must not reference Mullvad infrastructure: every \
                 endpoint derives from the Warren product anchors, never mullvad.net / a \
                 Mullvad-pinned host"
            );
        }
    }
}
