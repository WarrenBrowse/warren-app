//! Testable extraction of `WarrenApiConfig` resolution from
//! Settings + env var + seed loader.
//!
//! Pure function: no side effects, no I/O. The caller
//! (`Daemon::start` in `lib.rs`) loads env + seed + settings
//! then calls [`resolve`] with those already-resolved values.
//!
//! Covers these cases (see tests):
//! 1. `Some(url) && Some(seed)` -> Some.
//! 2. URL absent from both env AND settings -> compiled default.
//! 3. seed absent (mnemonic not bootstrapped) -> None.

use crate::device::WarrenApiConfig;
use crate::warren_sdk_client::SharedWarrenSeed;

/// Compiled warren-api base URL of the product environment this binary
/// was built for (`WARREN_PRODUCT_ENV`: prod | staging | beta). Used when
/// neither the `WARREN_API_URL` env var nor the persisted
/// `Settings::warren_api_url` provides a non-empty value, so the Warren
/// remote backend works out-of-the-box without the user ever running
/// `warren warren api-url set …`.
///
/// This is the canonical Warren backend over TLS, baked into the
/// release binary - it is **not** a MITM vector (an attacker cannot
/// substitute it without rebuilding the binary, and TLS authenticates
/// the host). Defaulting here is strictly safer than the historical
/// "no default → fall back to `api.mullvad.net`" behaviour, which
/// pointed the account/device chain at an endpoint Warren does not
/// operate.
pub const DEFAULT_WARREN_API_URL: &str = crate::warren_product_config::WARREN_API_URL;

/// Resolves the warren-api config based on flags + URL sources and the
/// BIP39-derived seed. Pure function: the caller injects `env_url` (=
/// via `std::env::var("WARREN_API_URL").ok()`) and `seed` (=
/// `WarrenIdentityManager::seed_handle`)
/// already resolved.
///
/// URL priority: first non-empty of `env_url` > `settings_url`,
/// otherwise the compiled [`DEFAULT_WARREN_API_URL`]. An empty value
/// (e.g. `warren warren api-url unset` persisting `Some("")`) is
/// treated as "not provided" and falls through to the next source /
/// the default, rather than disabling the Warren remote backend.
#[must_use]
pub(crate) fn resolve(
    settings_url: Option<String>,
    env_url: Option<String>,
    seed: Option<SharedWarrenSeed>,
) -> Option<WarrenApiConfig> {
    let url = api_url(settings_url, env_url);
    let seed = seed?;

    Some(WarrenApiConfig { url, seed })
}

/// The API base URL every Warren client of the daemon talks to.
///
/// A development build (the `api-override` feature) takes the first non-empty of `env_url`
/// (`WARREN_API_URL`) and `settings_url`. A release build always talks to its compiled API: the
/// signed requests carry the wallet address, and a leftover development setting or a variable
/// planted in the service environment must not send them anywhere else.
#[must_use]
pub(crate) fn api_url(settings_url: Option<String>, env_url: Option<String>) -> String {
    let configured = [env_url, settings_url]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty());
    match configured {
        Some(url) if cfg!(feature = "api-override") => url,
        Some(_) => {
            log::warn!("An API URL is configured and ignored: only development builds honour one");
            DEFAULT_WARREN_API_URL.to_owned()
        }
        None => DEFAULT_WARREN_API_URL.to_owned(),
    }
}

/// Why `SetWarrenApiUrl` refuses a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiUrlRefused {
    /// Only development builds talk to another API than their compiled one.
    ReleaseBuild,
    /// The signed requests would cross the network in clear.
    NotHttps,
}

/// Checks a value for `Settings::warren_api_url` before it is stored. An empty value clears the
/// setting and is always accepted.
pub(crate) fn check_api_url_setting(url: &str) -> Result<(), ApiUrlRefused> {
    if url.is_empty() {
        return Ok(());
    }
    if !cfg!(feature = "api-override") {
        return Err(ApiUrlRefused::ReleaseBuild);
    }
    match url.strip_prefix("https://") {
        Some(rest) if !rest.is_empty() && !rest.starts_with('/') => Ok(()),
        _ => Err(ApiUrlRefused::NotHttps),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use zeroize::Zeroizing;

    fn fixed_seed() -> SharedWarrenSeed {
        Arc::new(RwLock::new(Zeroizing::new([7u8; 32])))
    }

    #[test]
    fn happy_path_returns_config_with_url_and_seed() {
        let cfg = resolve(
            Some(DEFAULT_WARREN_API_URL.to_owned()),
            None,
            Some(fixed_seed()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(cfg.url, DEFAULT_WARREN_API_URL);
        assert_eq!(*cfg.seed.read().unwrap(), Zeroizing::new([7u8; 32]));
    }

    #[cfg(not(feature = "api-override"))]
    #[test]
    fn a_release_build_talks_to_its_compiled_api_whatever_is_configured() {
        let url = api_url(
            Some("https://elsewhere.example".to_owned()),
            Some("https://also-elsewhere.example".to_owned()),
        );

        assert_eq!(url, DEFAULT_WARREN_API_URL);
    }

    #[test]
    fn clearing_the_api_setting_is_always_accepted() {
        assert_eq!(check_api_url_setting(""), Ok(()));
    }

    #[cfg(not(feature = "api-override"))]
    #[test]
    fn a_release_build_refuses_to_change_its_api() {
        assert_eq!(
            check_api_url_setting("https://api.beta.warrenbrowse.com"),
            Err(ApiUrlRefused::ReleaseBuild)
        );
    }

    #[cfg(feature = "api-override")]
    #[test]
    fn a_development_build_refuses_a_cleartext_api() {
        assert_eq!(
            check_api_url_setting("http://127.0.0.1:8080"),
            Err(ApiUrlRefused::NotHttps)
        );
    }

    #[cfg(feature = "api-override")]
    #[test]
    fn a_development_build_accepts_another_https_api() {
        assert_eq!(check_api_url_setting("https://127.0.0.1:8443"), Ok(()));
    }

    #[cfg(feature = "api-override")]
    #[test]
    fn env_url_overrides_settings_url() {
        // Spec priority: env > settings. Allows a dev to point
        // at `http://localhost:8080` without touching the persistent
        // production user config.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            Some("http://127.0.0.1:8080".to_owned()),
            Some(fixed_seed()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(
            cfg.url, "http://127.0.0.1:8080",
            "env_url MUST take priority over settings_url"
        );
    }

    #[test]
    fn no_url_anywhere_uses_compiled_default() {
        // No URL configured anywhere. Rather than silently falling
        // back to the Mullvad upstream API (`api.mullvad.net`, which
        // Warren does not operate), we use the compiled production
        // default so the remote backend works out-of-the-box.
        let cfg = resolve(None, None, Some(fixed_seed()));
        let cfg = cfg.expect("no URL MUST now produce a config with the compiled default");
        assert_eq!(
            cfg.url, DEFAULT_WARREN_API_URL,
            "missing URL MUST default to the prod warren-api endpoint"
        );
    }

    #[test]
    fn empty_url_uses_compiled_default() {
        // A user who unset their URL via `warren warren api-url unset`
        // persists `Some("")`. We treat that as "not provided" and
        // fall through to the compiled default, not as a hard disable
        // of the Warren remote backend.
        let cfg = resolve(Some(String::new()), None, Some(fixed_seed()));
        let cfg = cfg.expect("empty URL MUST fall through to the compiled default");
        assert_eq!(
            cfg.url, DEFAULT_WARREN_API_URL,
            "empty URL MUST default to the prod warren-api endpoint"
        );
    }

    #[cfg(feature = "api-override")]
    #[test]
    fn empty_env_url_falls_through_to_settings_url() {
        // Priority is "first non-empty": an empty env var must not
        // shadow a real settings URL.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            Some(String::new()),
            Some(fixed_seed()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(
            cfg.url, "https://api.warrenbrowse.com",
            "an empty env URL MUST fall through to the non-empty settings URL"
        );
    }

    #[test]
    fn no_seed_returns_none() {
        // Edge case: URL configured but no mnemonic loaded (= identity
        // absent). We MUST NOT build a client with a dummy seed: we fall
        // back with a warn and no remote config.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            None, // seed absent
        );
        assert!(cfg.is_none(), "no seed MUST return None");
    }
}
