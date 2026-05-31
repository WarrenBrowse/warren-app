//! Detection of Warren local account mode via env var at daemon boot.
//!
//! When `WARREN_LOCAL_ACCOUNT=1`, the daemon does not call `api.mullvad.net`
//! for the initial `get_data()` retry-loop; the wallet identity derived
//! from the local mnemonic is used as-is.
//!
//! Same truthy parsing convention as the other Warren boot env vars
//! (`1`, `true`, `yes`, `on`, case-insensitive). To be replaced by
//! a persistent `Settings::warren_local_account: bool` setting exposed
//! via gRPC + GUI/CLI when the `AccountsProxy` Mullvad ->
//! `warren-api` proxy migration is delivered — until then, it's a
//! pragmatic POC switch to allow end-to-end benchmarking of the fork
//! without a Mullvad backend.

/// Name of the env var read at boot. Fixed convention for the duration
/// of the POC; renaming requires a docs/scripts/runbooks migration.
pub const ENV_VAR_NAME: &str = "WARREN_LOCAL_ACCOUNT";

/// `true` if Warren local account mode is enabled for this daemon process.
///
/// Enabled by `WARREN_LOCAL_ACCOUNT=1` (or `true`, `yes`, `on`,
/// case-insensitive). Any other value or absence = standard Mullvad
/// behavior (`api.mullvad.net` calls for `get_data` and `validate_device`).
///
/// **Phase E**: see [`resolve`] for the env + Settings combination.
#[must_use]
pub fn is_enabled() -> bool {
    parse_env(std::env::var(ENV_VAR_NAME).ok().as_deref())
}

/// Phase E — resolves the effective local account mode from the POC env var
/// combined with the persistent `Settings::warren_local_account` flag. The env
/// var, if set, **takes precedence**: it lets devs test quickly without
/// persisting the choice in Settings.
#[must_use]
pub fn resolve(settings_warren_local_account: bool) -> bool {
    match std::env::var(ENV_VAR_NAME).ok().as_deref() {
        Some(raw) => parse_env(Some(raw)),
        None => settings_warren_local_account,
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
    use super::parse_env;

    #[test]
    fn parse_env_returns_false_when_unset() {
        // The default must be false so that non-Warren builds
        // keep the legacy behavior (api.mullvad.net calls).
        // If we regressed to `true` by default, the whole remote
        // account path would be silently short-circuited = breaking.
        assert!(!parse_env(None));
    }

    #[test]
    fn parse_env_accepts_truthy_values() {
        // Covers the variants users will type, including
        // case + whitespace. If one breaks (e.g. `TRUE`
        // rejected), the user sees a mode that does not activate —
        // silent UX bug.
        for v in ["1", "true", "yes", "on", "TRUE", "Yes", " on "] {
            assert!(parse_env(Some(v)), "should accept {v:?}");
        }
    }

    #[test]
    fn parse_env_rejects_falsy_or_unknown_values() {
        // Critical regression: a bad value (e.g. `warren`,
        // `account`, or a typo `1.0`) must NEVER enable
        // local mode. Otherwise the daemon skips API calls thinking
        // it's in local mode when it is not.
        for v in ["0", "false", "no", "off", "", "warren", "account"] {
            assert!(!parse_env(Some(v)), "should reject {v:?}");
        }
    }

    /// Phase E: `resolve` returns the Settings value when the env
    /// var is absent (normal prod case). Regression: if we always
    /// read `false` (= ignore Settings), a user who toggled
    /// the mode via UI/CLI would never see it enabled.
    #[test]
    fn resolve_uses_settings_when_env_var_absent() {
        if std::env::var(super::ENV_VAR_NAME).is_ok() {
            return;
        }
        assert!(
            super::resolve(true),
            "resolve(true) without env must be true"
        );
        assert!(
            !super::resolve(false),
            "resolve(false) without env must be false"
        );
    }
}
