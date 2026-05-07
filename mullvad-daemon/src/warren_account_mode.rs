//! Détection du mode account local Warren via env var au boot du daemon.
//!
//! Quand `WARREN_LOCAL_ACCOUNT=1`, le daemon n'appelle plus `api.mullvad.net`
//! pour le retry-loop initial `get_data()` ni pour la validation device
//! déclenchée par la state machine après 3 retries WG. Le `device.json`
//! présent localement (créé par `warren_device_bootstrap` à partir de la
//! mnémonique) est considéré valide tel quel.
//!
//! Module sœur de [`crate::warren_mode`] : même convention de parsing
//! truthy (`1`, `true`, `yes`, `on`, casse-insensitive). À remplacer par
//! un setting persistant `Settings::warren_local_account: bool` exposé
//! via gRPC + GUI/CLI quand la migration `AccountsProxy` Mullvad →
//! `warren-api` proxy sera livrée — d'ici-là, c'est un POC switch
//! pragmatique pour permettre le bench end-to-end du fork sans backend
//! Mullvad.

/// Nom de l'env var lue au boot. Convention figée pour la durée du
/// POC ; renommer impose une migration côté docs/scripts/runbooks.
pub const ENV_VAR_NAME: &str = "WARREN_LOCAL_ACCOUNT";

/// `true` si le mode account local Warren est activé pour ce process daemon.
///
/// Activé par `WARREN_LOCAL_ACCOUNT=1` (ou `true`, `yes`, `on`, insensitive
/// à la casse). Toute autre valeur ou absence = comportement Mullvad
/// standard (appels `api.mullvad.net` pour `get_data` et `validate_device`).
#[must_use]
pub fn is_enabled() -> bool {
    parse_env(std::env::var(ENV_VAR_NAME).ok().as_deref())
}

fn parse_env(raw: Option<&str>) -> bool {
    let Some(raw) = raw else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_env;

    #[test]
    fn parse_env_returns_false_when_unset() {
        // Le default doit être false pour que les builds non-Warren
        // gardent le comportement historique (appels api.mullvad.net).
        // Si on régressait à `true` par défaut, tout le path account
        // remote serait court-circuité silencieusement = breaking.
        assert!(!parse_env(None));
    }

    #[test]
    fn parse_env_accepts_truthy_values() {
        // Couvre les variantes que les utilisateurs vont taper, y
        // compris casse + whitespace. Si l'une casse (ex. `TRUE`
        // rejeté), le user voit un mode qui ne s'active pas — bug
        // silencieux côté UX.
        for v in ["1", "true", "yes", "on", "TRUE", "Yes", " on "] {
            assert!(parse_env(Some(v)), "should accept {v:?}");
        }
    }

    #[test]
    fn parse_env_rejects_falsy_or_unknown_values() {
        // Régression critique : une mauvaise valeur (ex. `warren`,
        // `account`, ou une coquille `1.0`) ne doit JAMAIS activer
        // le mode local. Sinon le daemon zappe les appels API en
        // pensant être en mode local alors qu'il ne l'est pas.
        for v in ["0", "false", "no", "off", "", "warren", "account"] {
            assert!(!parse_env(Some(v)), "should reject {v:?}");
        }
    }
}
