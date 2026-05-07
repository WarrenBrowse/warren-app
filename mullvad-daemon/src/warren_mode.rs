//! Warren fork — Phase 4.F.4 : détection du mode tunnel Warren.
//!
//! POC switch pragmatique : pas de toggle UI/CLI/management-interface
//! pour l'instant, juste une env var lue au boot du daemon. Le user
//! qui veut tester le path Warren-Iroh exporte `WARREN_TUNNEL=1` avant
//! de lancer `mullvad-daemon`. Le default reste WireGuard upstream
//! (pas de breaking change pour les builds non-Warren).
//!
//! Ce module sera remplacé en Phase future par un setting persistant
//! (`Settings::warren_mode: bool`) exposé via gRPC + GUI/CLI quand le
//! POC sera consolidé.

/// Nom de l'env var lue au boot. Convention figée pour la durée du
/// POC ; renommer impose une migration côté docs/scripts.
pub const ENV_VAR_NAME: &str = "WARREN_TUNNEL";

/// `true` si le mode Warren-Iroh est activé pour ce process daemon.
///
/// Activé par `WARREN_TUNNEL=1` (ou `true`, `yes`, `on`, insensitive à
/// la casse). Toute autre valeur ou absence = WireGuard upstream.
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
        assert!(!parse_env(None));
    }

    #[test]
    fn parse_env_accepts_truthy_values() {
        for v in ["1", "true", "yes", "on", "TRUE", "Yes", " on "] {
            assert!(parse_env(Some(v)), "should accept {v:?}");
        }
    }

    #[test]
    fn parse_env_rejects_falsy_or_unknown_values() {
        for v in ["0", "false", "no", "off", "", "warren", "tunnel"] {
            assert!(!parse_env(Some(v)), "should reject {v:?}");
        }
    }
}
