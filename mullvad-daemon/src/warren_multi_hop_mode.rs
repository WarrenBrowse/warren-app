//! Daemon-side opt-in switch for Warren multi-hop tunnels.
//!
//! A single env var `WARREN_MULTI_HOP`
//! gates whether the tunnel state machine assembles a
//! [`talpid_warren_tunnel::MultiHopConfig`] (when the file at
//! `<settings_dir>/warren-multihop.json` is present) or not. The env
//! var prevents a stale config file from
//! silently bumping a daemon into multi-hop on the next restart; the
//! file alone is necessary but not sufficient.
//!
//! A user-facing UI toggle (Settings::warren_multi_hop_mode) is left
//! for a future revision; this env var is the bench-time gate.

/// Name of the env var read at boot. Convention frozen for the POC:
/// renaming requires a migration in `docs/` and bench scripts.
pub const ENV_VAR_NAME: &str = "WARREN_MULTI_HOP";

/// `true` when the user opted into multi-hop via env var.
///
/// Truthy values (case-insensitive, trim-stripped): `1`, `true`, `yes`,
/// `on`. Anything else or absent = single-hop.
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
        for v in ["0", "false", "no", "off", "", "warren", "multi-hop"] {
            assert!(!parse_env(Some(v)), "should reject {v:?}");
        }
    }
}
