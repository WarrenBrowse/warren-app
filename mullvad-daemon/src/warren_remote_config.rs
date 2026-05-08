//! Warren fork — Phase #4 — extraction testable de la résolution
//! `WarrenApiConfig` depuis Settings + env var + signing key loader.
//!
//! Pure function : pas de side effects, pas d'I/O. Le caller
//! (`Daemon::start` dans `lib.rs`) charge l'env + signing_key + settings
//! puis appelle [`resolve`] avec ces valeurs déjà résolues.
//!
//! Couvre 5 cas (cf. tests) :
//! 1. `warren_mode = false` → None (preserve Mullvad upstream).
//! 2. `local_account_mode = true` → None (LocalBackend prend le relais).
//! 3. `warren_mode && !local_account_mode && Some(url) && Some(key)` → Some.
//! 4. URL absente côté env ET settings → None (fallback Mullvad).
//! 5. signing_key absente (mnémonique non bootstrappée) → None.

use ed25519_dalek::SigningKey;

use crate::device::WarrenApiConfig;

/// Résoud la config warren-api selon les flags + sources d'URL et de
/// signing key. Pure function : le caller injecte `env_url` (= via
/// `std::env::var("WARREN_API_URL").ok()`) et `signing_key` (=
/// `warren_signer::load_or_create_signing_key(...)`) déjà résolus.
///
/// Priorité URL : `env_url` > `settings_url`. Pas de mélange : si l'env
/// est setée mais vide, c'est traité comme valeur explicite (= unset
/// volontaire, override settings vers None).
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
        // Régression critique : un user qui désactive warren_mode ne doit
        // PAS voir le backend warren-remote actif. Tout doit retomber
        // sur Mullvad upstream legacy.
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
        // Régression : si warren_mode + warren_local_account sont tous
        // les deux true, c'est `LocalAccountBackend` qui doit prendre
        // la main (= path POC stateless), pas warren-remote.
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
        // Priorité spec : env > settings. Permet à un dev de pointer
        // vers `http://localhost:8080` sans toucher la config persistante
        // de l'utilisateur prod.
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
        // Edge case : warren_mode est on mais aucune URL configurée.
        // Plutôt que de planter ou pointer vers une URL par défaut
        // (= risque MITM), on retombe sur Mullvad upstream avec un warn
        // côté caller.
        let cfg = resolve(true, false, None, None, Some(fixed_signing_key()));
        assert!(cfg.is_none(), "no URL MUST return None (no default)");
    }

    #[test]
    fn empty_url_returns_none() {
        // Edge case : un user a unset son URL via `mullvad-cli warren
        // api-url unset` qui envoie "" sur le wire → Settings.warren_api_url
        // == Some("") après serde (selon proto3). On veut traiter ça
        // comme unset, pas comme URL vide invalide qui crasherait
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
        // Edge case : warren_mode actif + URL configurée mais pas de
        // mnémonique chargée (= identité absente). On NE DOIT PAS
        // construire un client avec une key dummy : on retombe sur
        // Mullvad upstream avec un warn.
        let cfg = resolve(
            true,
            false,
            Some("https://api.warrenbrowse.com".to_owned()),
            None,
            None, // signing_key absente
        );
        assert!(cfg.is_none(), "no signing_key MUST return None");
    }
}
