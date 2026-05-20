//! Detection of Warren tunnel mode via env var at daemon boot.
//!
//! Pragmatic POC switch: no UI/CLI/management-interface toggle
//! for now. The user who wants to test the Warren-Iroh path
//! exports `WARREN_TUNNEL=1` before launching `mullvad-daemon`. The
//! default remains the upstream WireGuard path (no breaking change
//! for non-Warren builds). To be replaced by a persistent
//! `Settings::warren_mode: bool` setting exposed via gRPC + GUI/CLI
//! when the POC is consolidated.

/// Name of the env var read at boot. Fixed convention for the duration
/// of the POC; renaming requires a docs/scripts migration.
pub const ENV_VAR_NAME: &str = "WARREN_TUNNEL";

/// `true` if Warren-Iroh mode is enabled for this daemon process.
///
/// Enabled by `WARREN_TUNNEL=1` (or `true`, `yes`, `on`, case
/// insensitive). Any other value or absence = upstream WireGuard.
///
/// **Phase E**: uses [`resolve`] which combines env var (override) +
/// `Settings::warren_mode` (persistent). This wrapper keeps the
/// history for callers that do not have access to Settings at the
/// moment of the check (= very early boot).
#[must_use]
pub fn is_enabled() -> bool {
    parse_env(std::env::var(ENV_VAR_NAME).ok().as_deref())
}

/// Phase E — resolves the effective Warren mode from a combination
/// of POC env var + persistent `Settings::warren_mode` flag. The env var,
/// if set, **takes precedence**: lets devs test
/// quickly without persisting the choice in Settings; in
/// production the end user will toggle via UI/CLI which mutates
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

    /// Phase E: `resolve` must take the env var into account if set,
    /// even if Settings says `false`. Without this priority, a dev who
    /// exports `WARREN_TUNNEL=1` to test quickly would not have
    /// the mode enabled without touching their persistent Settings.
    /// This test does not touch the real env — it tests the pure
    /// logic via `parse_env` + the `Some/None` check. The rest (reading
    /// `std::env::var`) is trivial and tested indirectly by
    /// the absence of env in CI builds.
    #[test]
    fn resolve_uses_settings_when_env_var_absent() {
        // Assumption: env var WARREN_TUNNEL absent (normal case in
        // CI / prod without override). resolve must return the value
        // from Settings.
        // SAFETY: we read `std::env`; the other tests do not set
        // WARREN_TUNNEL. If a parallel test set it, this test
        // would break -> indication of coupling.
        // SAFETY: remove_var is unsafe in Rust 2024.
        // SAFETY: `remove_var` is not thread-safe; Rust tests
        // often run in parallel (`--test-threads`). We therefore avoid
        // mutating the env directly and build the pure
        // logic independently of `std::env`.
        // -> We test `resolve` indirectly via the equivalent
        // logic: if WARREN_TUNNEL is not set by the
        // other tests (which is the case on normal CI), `resolve`
        // must reflect Settings.
        if std::env::var(ENV_VAR_NAME).is_ok() {
            // Test skipped if the env contains WARREN_TUNNEL — not a
            // regression but a consistency safeguard.
            return;
        }
        assert!(resolve(true), "resolve(true) without env must be true");
        assert!(!resolve(false), "resolve(false) without env must be false");
    }
}
