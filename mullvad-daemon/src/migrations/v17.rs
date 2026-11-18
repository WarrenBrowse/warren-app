use super::{Error, Result};
use mullvad_types::settings::SettingsVersion;

/// The built-in API access methods that reached the API through a third party
/// (bridges, encrypted DNS proxies, domain fronting) no longer exist: only the
/// direct connection and the user's custom proxies remain.
///
/// This migration drops the `mullvad_bridges`, `encrypted_dns_proxy` and
/// `domain_fronting` entries of `api_access_methods`, and turns the direct
/// method back on when nothing enabled is left, so the daemon always has a way
/// to reach the API.
pub fn migrate(settings: &mut serde_json::Value) -> Result<()> {
    if !version_matches(settings) {
        return Ok(());
    }

    log::info!("Migrating settings format to V18");

    if let Some(access_methods) = settings
        .as_object_mut()
        .ok_or(Error::InvalidSettingsContent)?
        .get_mut("api_access_methods")
        .and_then(serde_json::Value::as_object_mut)
    {
        for removed in ["mullvad_bridges", "encrypted_dns_proxy", "domain_fronting"] {
            access_methods.remove(removed);
        }
        let custom_enabled = access_methods
            .get("custom")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|custom| custom.iter().any(is_enabled));
        if !custom_enabled
            && let Some(direct) = access_methods
                .get_mut("direct")
                .and_then(serde_json::Value::as_object_mut)
        {
            direct.insert("enabled".to_owned(), serde_json::Value::Bool(true));
        }
    }

    settings["settings_version"] = serde_json::json!(SettingsVersion::V18);

    Ok(())
}

fn is_enabled(method: &serde_json::Value) -> bool {
    method
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn version_matches(settings: &serde_json::Value) -> bool {
    settings
        .get("settings_version")
        .map(|version| version == SettingsVersion::V17 as u64)
        .unwrap_or(false)
}

#[cfg(test)]
mod test {
    use super::migrate;
    use mullvad_types::{
        access_method::AccessMethodSetting,
        settings::{Settings, SettingsVersion},
    };
    use serde_json::json;
    use talpid_types::net::proxy::{Shadowsocks, ShadowsocksCipher};

    /// Settings as the V17 daemon wrote them on macOS, with the three
    /// third-party built-in methods present and disabled.
    const V17_SETTINGS: &str = include_str!("v17_settings.json");

    const REMOVED: [&str; 3] = ["mullvad_bridges", "encrypted_dns_proxy", "domain_fronting"];

    fn v17_settings() -> serde_json::Value {
        serde_json::from_str(V17_SETTINGS).unwrap()
    }

    fn custom_shadowsocks(enabled: bool) -> serde_json::Value {
        let proxy = Shadowsocks::new(
            "192.0.2.10:443".parse::<std::net::SocketAddr>().unwrap(),
            ShadowsocksCipher::new("aes-256-gcm").unwrap(),
            "secret".to_owned(),
        );
        let mut method = AccessMethodSetting::new("Office".to_owned(), true, proxy.into());
        method.enabled = enabled;
        serde_json::to_value(method).unwrap()
    }

    #[test]
    fn drops_the_removed_built_in_methods_of_a_real_v17_file() {
        let mut settings = v17_settings();

        migrate(&mut settings).unwrap();

        let methods = settings["api_access_methods"].as_object().unwrap();
        for removed in REMOVED {
            assert!(!methods.contains_key(removed), "{removed} survived");
        }
        assert_eq!(methods["direct"]["enabled"], json!(true));
        assert_eq!(
            settings["settings_version"],
            json!(SettingsVersion::V18 as u64)
        );
        let settings: Settings = serde_json::from_value(settings).unwrap();
        assert_eq!(settings.api_access_methods.cardinality(), 1);
        assert!(settings.api_access_methods.direct().enabled());
    }

    #[test]
    fn keeps_custom_methods() {
        let mut settings = v17_settings();
        let custom = custom_shadowsocks(true);
        settings["api_access_methods"]["custom"] = json!([custom.clone()]);

        migrate(&mut settings).unwrap();

        assert_eq!(settings["api_access_methods"]["custom"], json!([custom]));
        let settings: Settings = serde_json::from_value(settings).unwrap();
        assert_eq!(settings.api_access_methods.iter_custom().count(), 1);
    }

    #[test]
    fn falls_back_to_direct_when_only_a_removed_method_was_enabled() {
        let mut settings = v17_settings();
        settings["api_access_methods"]["direct"]["enabled"] = json!(false);
        settings["api_access_methods"]["mullvad_bridges"]["enabled"] = json!(true);
        settings["api_access_methods"]["custom"] = json!([custom_shadowsocks(false)]);

        migrate(&mut settings).unwrap();

        assert_eq!(
            settings["api_access_methods"]["direct"]["enabled"],
            json!(true)
        );
    }

    #[test]
    fn leaves_direct_off_when_an_enabled_custom_method_remains() {
        let mut settings = v17_settings();
        settings["api_access_methods"]["direct"]["enabled"] = json!(false);
        settings["api_access_methods"]["custom"] = json!([custom_shadowsocks(true)]);

        migrate(&mut settings).unwrap();

        assert_eq!(
            settings["api_access_methods"]["direct"]["enabled"],
            json!(false)
        );
    }

    #[test]
    fn leaves_other_versions_alone() {
        let mut original = v17_settings();
        original["settings_version"] = json!(SettingsVersion::V16 as u64);
        let mut settings = original.clone();

        migrate(&mut settings).unwrap();

        assert_eq!(settings, original);
    }
}
