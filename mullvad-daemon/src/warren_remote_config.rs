//! Testable extraction of `WarrenApiConfig` resolution from
//! Settings + env var + signing key loader.
//!
//! Pure function: no side effects, no I/O. The caller
//! (`Daemon::start` in `lib.rs`) loads env + signing_key + settings
//! then calls [`resolve`] with those already-resolved values.
//!
//! Covers 5 cases (see tests):
//! 1. `warren_mode = false` -> None (preserve Mullvad upstream).
//! 2. `local_account_mode = true` -> None (LocalBackend takes over).
//! 3. `warren_mode && !local_account_mode && Some(url) && Some(key)` -> Some.
//! 4. URL absent from both env AND settings -> None (Mullvad fallback).
//! 5. signing_key absent (mnemonic not bootstrapped) -> None.

use ed25519_dalek::SigningKey;

use crate::device::WarrenApiConfig;

/// Resolves the warren-api config based on flags + URL sources and
/// signing key. Pure function: the caller injects `env_url` (= via
/// `std::env::var("WARREN_API_URL").ok()`) and `signing_key` (=
/// `warren_signer::load_or_create_signing_key(...)`) already resolved.
///
/// URL priority: `env_url` > `settings_url`. No mixing: if the env
/// is set but empty, it is treated as an explicit value (= deliberately
/// unset, overrides settings to None).
#[must_use]
pub(crate) fn resolve(
    warren_mode_active: bool,
    local_account_mode: bool,
    settings_url: Option<String>,
    env_url: Option<String>,
    signing_key: Option<SigningKey>,
) -> Option<WarrenApiConfig> {
    if !warren_mode_active || local_account_mode {
        return None;
    }

    let url = env_url.or(settings_url)?;
    if url.is_empty() {
        return None;
    }
    let signing_key = signing_key?;

    Some(WarrenApiConfig { url, signing_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn warren_mode_disabled_returns_none() {
        // Critical regression: a user who disables warren_mode must
        // NOT see the warren-remote backend active. Everything must
        // fall back to legacy Mullvad upstream.
        let cfg = resolve(
            false, // warren_mode_active = false
            false,
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            Some(fixed_signing_key()),
        );
        assert!(cfg.is_none(), "warren_mode=false MUST disable remote");
    }

    #[test]
    fn local_account_mode_takes_priority_over_warren_remote() {
        // Regression: if warren_mode + warren_local_account are both
        // true, `LocalAccountBackend` must take over (= stateless POC
        // path), not warren-remote.
        let cfg = resolve(
            true,
            true, // local_account_mode = true
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            Some(fixed_signing_key()),
        );
        assert!(
            cfg.is_none(),
            "local_account_mode=true MUST short-circuit warren-remote"
        );
    }

    #[test]
    fn happy_path_returns_config_with_url_and_key() {
        let cfg = resolve(
            true,
            false,
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            Some(fixed_signing_key()),
        );
        let cfg = cfg.expect("must produce a config");
        assert_eq!(cfg.url, "https://api.warrenbrowse.com");
        assert_eq!(cfg.signing_key.to_bytes(), [7u8; 32]);
    }

    #[test]
    fn env_url_overrides_settings_url() {
        // Spec priority: env > settings. Allows a dev to point
        // at `http://localhost:8080` without touching the persistent
        // production user config.
        let cfg = resolve(
            true,
            false,
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
    fn no_url_anywhere_returns_none() {
        // Edge case: warren_mode is on but no URL configured.
        // Rather than panicking or pointing at a default URL
        // (= MITM risk), we fall back to Mullvad upstream with a warn
        // on the caller side.
        let cfg = resolve(true, false, None, None, Some(fixed_signing_key()));
        assert!(cfg.is_none(), "no URL MUST return None (no default)");
    }

    #[test]
    fn empty_url_returns_none() {
        // Edge case: a user has unset their URL via `mullvad-cli warren
        // api-url unset` which sends "" on the wire -> Settings.warren_api_url
        // == Some("") after serde (per proto3). We want to treat that
        // as unset, not as an invalid empty URL that would crash
        // reqwest.
        let cfg = resolve(
            true,
            false,
            Some(String::new()),
            None,
            Some(fixed_signing_key()),
        );
        assert!(cfg.is_none(), "empty URL MUST return None");
    }

    #[test]
    fn no_signing_key_returns_none() {
        // Edge case: warren_mode active + URL configured but no
        // mnemonic loaded (= identity absent). We MUST NOT
        // build a client with a dummy key: we fall back to
        // Mullvad upstream with a warn.
        let cfg = resolve(
            true,
            false,
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            None, // signing_key absent
        );
        assert!(cfg.is_none(), "no signing_key MUST return None");
    }
}
