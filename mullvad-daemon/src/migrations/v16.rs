use super::{Error, Result};
use mullvad_types::settings::SettingsVersion;

/// This migration moves the split tunneling settings into the app routing
/// settings:
///
/// `"split_tunnel": { "enable_exclusions": bool, "apps": [path] }` becomes
/// `"app_routing": { "split_mode": "exclude" | "off", "excluded_apps": [path],
/// "included_apps": [], "app_exits_enabled": false, "app_exits": {} }`.
///
/// Linux persisted no `split_tunnel` (exclusion there is launch based) and
/// gets the default app routing.
pub fn migrate(settings: &mut serde_json::Value) -> Result<()> {
    if !version_matches(settings) {
        return Ok(());
    }

    log::info!("Migrating settings format to V17");

    let split_tunnel = settings
        .as_object_mut()
        .ok_or(Error::InvalidSettingsContent)?
        .remove("split_tunnel");
    let enable_exclusions = split_tunnel
        .as_ref()
        .and_then(|split_tunnel| split_tunnel.get("enable_exclusions"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let apps = split_tunnel
        .as_ref()
        .and_then(|split_tunnel| split_tunnel.get("apps"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));

    settings["app_routing"] = serde_json::json!({
        "split_mode": if enable_exclusions { "exclude" } else { "off" },
        "excluded_apps": apps,
        "included_apps": [],
        "app_exits_enabled": false,
        "app_exits": {},
    });
    settings["settings_version"] = serde_json::json!(SettingsVersion::V17);

    Ok(())
}

fn version_matches(settings: &serde_json::Value) -> bool {
    settings
        .get("settings_version")
        .map(|version| version == SettingsVersion::V16 as u64)
        .unwrap_or(false)
}

#[cfg(test)]
mod test {
    use super::migrate;
    use mullvad_types::{
        app_routing::{AppId, AppIdFlavor, SplitMode},
        settings::{CURRENT_SETTINGS_VERSION, Settings, SettingsVersion},
    };
    use serde_json::json;

    /// Settings as the V16 daemon wrote them on macOS, with one excluded app.
    const V16_SETTINGS: &str = include_str!("v16_settings.json");

    #[test]
    fn moves_enabled_exclusions_of_a_real_v16_file_into_app_routing() {
        let mut settings: serde_json::Value = serde_json::from_str(V16_SETTINGS).unwrap();

        migrate(&mut settings).unwrap();

        assert!(settings.get("split_tunnel").is_none());
        assert_eq!(
            settings["app_routing"],
            json!({
                "split_mode": "exclude",
                "excluded_apps": ["/Applications/Firefox.app/Contents/MacOS/firefox"],
                "included_apps": [],
                "app_exits_enabled": false,
                "app_exits": {},
            })
        );
        let settings: Settings = serde_json::from_value(settings).unwrap();
        assert_eq!(settings.settings_version, CURRENT_SETTINGS_VERSION);
        assert!(settings.app_routing.exclusions_active());
        let firefox = AppId::parse_as(
            AppIdFlavor::Unix,
            "/Applications/Firefox.app/Contents/MacOS/firefox",
        )
        .unwrap();
        assert!(settings.app_routing.excluded_apps.contains(&firefox));
    }

    #[test]
    fn keeps_the_apps_of_disabled_exclusions() {
        let mut settings = json!({
            "split_tunnel": { "enable_exclusions": false, "apps": ["C:\\app.exe"] },
            "settings_version": SettingsVersion::V16 as u64,
        });

        migrate(&mut settings).unwrap();

        assert_eq!(settings["app_routing"]["split_mode"], json!("off"));
        assert_eq!(
            settings["app_routing"]["excluded_apps"],
            json!(["C:\\app.exe"])
        );
    }

    #[test]
    fn gives_settings_without_split_tunneling_the_default_app_routing() {
        let mut settings = json!({ "settings_version": SettingsVersion::V16 as u64 });

        migrate(&mut settings).unwrap();

        let settings: Settings = serde_json::from_value(settings).unwrap();
        assert_eq!(settings.app_routing.split_mode, SplitMode::Off);
        assert!(settings.app_routing.excluded_apps.is_empty());
        assert_eq!(settings.settings_version, CURRENT_SETTINGS_VERSION);
    }

    #[test]
    fn leaves_other_versions_alone() {
        let original = json!({
            "split_tunnel": { "enable_exclusions": true, "apps": [] },
            "settings_version": SettingsVersion::V15 as u64,
        });
        let mut settings = original.clone();

        migrate(&mut settings).unwrap();

        assert_eq!(settings, original);
    }
}
