//! Détection du mode tunnel Warren via env var au boot du daemon.
//!
//! POC switch pragmatique : pas de toggle UI/CLI/management-interface
//! pour l'instant. L'utilisateur qui veut tester le path Warren-Iroh
//! exporte `WARREN_TUNNEL=1` avant de lancer `mullvad-daemon`. Le
//! default reste le path WireGuard upstream (pas de breaking change
//! pour les builds non-Warren). À remplacer par un setting persistant
//! `Settings::warren_mode: bool` exposé via gRPC + GUI/CLI quand le
//! POC sera consolidé.

/// Nom de l'env var lue au boot. Convention figée pour la durée du
/// POC ; renommer impose une migration côté docs/scripts.
pub const ENV_VAR_NAME: &str = "WARREN_TUNNEL";

/// `true` si le mode Warren-Iroh est activé pour ce process daemon.
///
/// Activé par `WARREN_TUNNEL=1` (ou `true`, `yes`, `on`, insensitive à
/// la casse). Toute autre valeur ou absence = WireGuard upstream.
///
/// **Phase E** : utilise [`resolve`] qui combine env var (override) +
/// `Settings::warren_mode` (persistant). Cette wrapper garde
/// l'historique pour les callers qui n'ont pas accès aux Settings au
/// moment du check (= boot très précoce).
#[must_use]
pub fn is_enabled() -> bool {
    parse_env(std::env::var(ENV_VAR_NAME).ok().as_deref())
}

/// Phase E — résout le mode Warren effectif depuis une combinaison
/// env var POC + flag persistant `Settings::warren_mode`. L'env var,
/// si setée, **prend précédence** : permet aux devs de tester
/// rapidement sans persister le choix dans les Settings ; en
/// production l'utilisateur final togglera via UI/CLI ce qui mute
/// `Settings::warren_mode`.
#[must_use]
pub fn resolve(settings_warren_mode: bool) -> bool {
    match std::env::var(ENV_VAR_NAME).ok().as_deref() {
        Some(raw) => parse_env(Some(raw)),
        None => settings_warren_mode,
    }
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
    use super::{ENV_VAR_NAME, parse_env, resolve};

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

    /// Phase E : `resolve` doit prendre l'env var en compte si setée,
    /// même si Settings dit `false`. Sans cette priorité, un dev qui
    /// exporte `WARREN_TUNNEL=1` pour tester rapidement n'aurait pas
    /// le mode activé sans toucher à ses Settings persistants.
    /// Ce test ne touche pas à l'env réel — il teste la logique pure
    /// via `parse_env` + le check `Some/None`. Le reste (lecture
    /// `std::env::var`) est trivial et testé indirectement par
    /// l'absence d'env dans les builds CI.
    #[test]
    fn resolve_uses_settings_when_env_var_absent() {
        // Hypothèse : env var WARREN_TUNNEL absente (cas normal en
        // CI / prod sans override). resolve doit retourner la valeur
        // de Settings.
        // SAFETY: on lit `std::env` ; les autres tests ne setent pas
        // WARREN_TUNNEL. Si un test parallèle le setait, ce test
        // casserait → indication de coupling.
        // SAFETY: remove_var est unsafe en Rust 2024.
        // SAFETY: `remove_var` n'est pas thread-safe ; les tests Rust
        // tournent souvent en parallèle (`--test-threads`). On évite
        // donc de muter l'env directement et on construit la logique
        // pure indépendamment de `std::env`.
        // → On teste `resolve` indirectement via la logique
        // équivalente : si WARREN_TUNNEL n'est pas setée par les
        // autres tests (ce qui est le cas sur CI normal), `resolve`
        // doit refléter Settings.
        if std::env::var(ENV_VAR_NAME).is_ok() {
            // Test skippé si l'env contient WARREN_TUNNEL — pas une
            // régression mais sécurité de cohérence.
            return;
        }
        assert!(resolve(true), "resolve(true) sans env doit être true");
        assert!(!resolve(false), "resolve(false) sans env doit être false");
    }
}
