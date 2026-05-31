use super::Result;
use mullvad_types::settings::SettingsVersion;

/// This migration handles:
/// - Disabling the Mullvad-operated built-in API access methods (Bridge,
///   Encrypted DNS proxy and Domain fronting). Warren only talks to its own
///   API (api.warrenbrowse.com) over a direct connection; the circumvention
///   methods rely on Mullvad infrastructure (relay-list bridges, frakta.eu
///   encrypted DNS proxies, the CDN77 domain-fronting endpoint) that Warren
///   does not operate. Leaving them enabled made the daemon rotate through
///   dead Mullvad endpoints and surface TLS/certificate errors.
pub fn migrate(settings: &mut serde_json::Value) -> Result<()> {
    if !version_matches(settings) {
        return Ok(());
    }

    log::info!("Migrating settings format to V16");

    disable_mullvad_access_methods(settings);

    settings["settings_version"] = serde_json::json!(SettingsVersion::V16);

    Ok(())
}

fn disable_mullvad_access_methods(settings: &mut serde_json::Value) -> Option<()> {
    let access_methods = settings
        .get_mut("api_access_methods")
        .and_then(|methods| methods.as_object_mut())?;

    for key in ["mullvad_bridges", "encrypted_dns_proxy", "domain_fronting"] {
        if let Some(method) = access_methods
            .get_mut(key)
            .and_then(|method| method.as_object_mut())
        {
            method.insert("enabled".to_string(), serde_json::json!(false));
        }
    }

    Some(())
}

fn version_matches(settings: &serde_json::Value) -> bool {
    settings
        .get("settings_version")
        .map(|version| version == SettingsVersion::V15 as u64)
        .unwrap_or(false)
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_v15_to_v16_disables_mullvad_access_methods() {
        let mut old_settings = json!({
            "api_access_methods": {
                "direct": { "enabled": true },
                "mullvad_bridges": { "enabled": true },
                "encrypted_dns_proxy": { "enabled": true },
                "domain_fronting": { "enabled": true },
                "custom": []
            },
            "settings_version": SettingsVersion::V15 as u64
        });

        migrate(&mut old_settings).unwrap();

        let methods = &old_settings["api_access_methods"];
        assert_eq!(methods["direct"]["enabled"], json!(true));
        assert_eq!(methods["mullvad_bridges"]["enabled"], json!(false));
        assert_eq!(methods["encrypted_dns_proxy"]["enabled"], json!(false));
        assert_eq!(methods["domain_fronting"]["enabled"], json!(false));
        assert_eq!(old_settings["settings_version"], json!(SettingsVersion::V16));
    }

    #[test]
    fn test_v15_to_v16_migration_is_idempotent_on_wrong_version() {
        let mut settings = json!({
            "api_access_methods": {
                "mullvad_bridges": { "enabled": true }
            },
            "settings_version": SettingsVersion::V16 as u64
        });

        migrate(&mut settings).unwrap();

        // Version does not match V15, so nothing should change.
        assert_eq!(
            settings["api_access_methods"]["mullvad_bridges"]["enabled"],
            json!(true)
        );
    }
}
