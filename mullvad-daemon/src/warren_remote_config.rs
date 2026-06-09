//! Testable extraction of `WarrenApiConfig` resolution from
//! Settings + env var + signing key loader.
//!
//! Pure function: no side effects, no I/O. The caller
//! (`Daemon::start` in `lib.rs`) loads env + signing_key + settings
//! then calls [`resolve`] with those already-resolved values.
//!
//! Covers these cases (see tests):
//! 1. `Some(url) && Some(key)` -> Some.
//! 2. URL absent from both env AND settings -> compiled default.
//! 3. signing_key absent (mnemonic not bootstrapped) -> None.

use std::sync::{Arc, RwLock};

use ed25519_dalek::SigningKey;

use crate::device::WarrenApiConfig;

/// Compiled production warren-api base URL. Used when neither the
/// `WARREN_API_URL` env var nor the persisted `Settings::warren_api_url`
/// provides a non-empty value, so the Warren remote backend works
/// out-of-the-box without the user ever running `warren warren api-url
/// set …`.
///
/// This is the canonical Warren backend over TLS, baked into the
/// release binary - it is **not** a MITM vector (an attacker cannot
/// substitute it without rebuilding the binary, and TLS authenticates
/// the host). Defaulting here is strictly safer than the historical
/// "no default → fall back to `api.mullvad.net`" behaviour, which
/// pointed the account/device chain at an endpoint Warren does not
/// operate.
pub const DEFAULT_WARREN_API_URL: &str = "https://api.warrenbrowse.com";

/// Resolves the warren-api config based on flags + URL sources and
/// signing key. Pure function: the caller injects `env_url` (= via
/// `std::env::var("WARREN_API_URL").ok()`) and `signing_key` (=
/// `warren_signer::load_or_create_signing_key(...)`) already resolved.
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
    signing_key: Option<Arc<RwLock<SigningKey>>>,
) -> Option<WarrenApiConfig> {
    // First non-empty of [env, settings], else the compiled prod
    // default - so remote mode never silently falls back to the
    // Mullvad upstream API just because no URL was configured.
    let url = [env_url, settings_url]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_WARREN_API_URL.to_owned());
    let signing_key = signing_key?;

    Some(WarrenApiConfig { url, signing_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_signing_key() -> Arc<RwLock<SigningKey>> {
        Arc::new(RwLock::new(SigningKey::from_bytes(&[7u8; 32])))
    }

    #[test]
    fn happy_path_returns_config_with_url_and_key() {
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            Some(fixed_signing_key()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(cfg.url, "https://api.warrenbrowse.com");
        assert_eq!(cfg.signing_key.read().unwrap().to_bytes(), [7u8; 32]);
    }

    #[test]
    fn env_url_overrides_settings_url() {
        // Spec priority: env > settings. Allows a dev to point
        // at `http://localhost:8080` without touching the persistent
        // production user config.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            Some("http://127.0.0.1:8080".to_owned()),
            Some(fixed_signing_key()),
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
        let cfg = resolve(None, None, Some(fixed_signing_key()));
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
        let cfg = resolve(Some(String::new()), None, Some(fixed_signing_key()));
        let cfg = cfg.expect("empty URL MUST fall through to the compiled default");
        assert_eq!(
            cfg.url, DEFAULT_WARREN_API_URL,
            "empty URL MUST default to the prod warren-api endpoint"
        );
    }

    #[test]
    fn empty_env_url_falls_through_to_settings_url() {
        // Priority is "first non-empty": an empty env var must not
        // shadow a real settings URL.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            Some(String::new()),
            Some(fixed_signing_key()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(
            cfg.url, "https://api.warrenbrowse.com",
            "an empty env URL MUST fall through to the non-empty settings URL"
        );
    }

    #[test]
    fn no_signing_key_returns_none() {
        // Edge case: URL configured but no mnemonic loaded (= identity
        // absent). We MUST NOT build a client with a dummy key: we fall
        // back with a warn and no remote config.
        let cfg = resolve(
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            None, // signing_key absent
        );
        assert!(cfg.is_none(), "no signing_key MUST return None");
    }
}
