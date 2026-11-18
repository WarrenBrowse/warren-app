use super::{Error, Result};
use mullvad_types::settings::SettingsVersion;

/// The WireGuard-era settings the Warren tunnel never read are gone: the
/// obfuscation settings, the WireGuard MTU, the userspace WireGuard switch and
/// the WireGuard allowed IPs.
///
/// This migration drops `obfuscation_settings`, `tunnel_options.wireguard.mtu`,
/// `tunnel_options.wireguard.userspace` and
/// `relay_settings.normal.wireguard_constraints.allowed_ips`. Every other
/// setting, the quantum-resistant and DAITA options included, is kept as is.
pub fn migrate(settings: &mut serde_json::Value) -> Result<()> {
    if !version_matches(settings) {
        return Ok(());
    }

    log::info!("Migrating settings format to V19");

    let root = settings
        .as_object_mut()
        .ok_or(Error::InvalidSettingsContent)?;
    root.remove("obfuscation_settings");
    if let Some(wireguard) = root
        .get_mut("tunnel_options")
        .and_then(|options| options.get_mut("wireguard"))
        .and_then(serde_json::Value::as_object_mut)
    {
        wireguard.remove("mtu");
        wireguard.remove("userspace");
    }
    if let Some(constraints) = root
        .get_mut("relay_settings")
        .and_then(|relay| relay.get_mut("normal"))
        .and_then(|normal| normal.get_mut("wireguard_constraints"))
        .and_then(serde_json::Value::as_object_mut)
    {
        constraints.remove("allowed_ips");
    }

    settings["settings_version"] = serde_json::json!(SettingsVersion::V19);

    Ok(())
}

fn version_matches(settings: &serde_json::Value) -> bool {
    settings
        .get("settings_version")
        .map(|version| version == SettingsVersion::V18 as u64)
        .unwrap_or(false)
}

#[cfg(test)]
mod test {
    use super::migrate;
    use mullvad_types::{
        relay_constraints::RelaySettings,
        settings::{CURRENT_SETTINGS_VERSION, Settings, SettingsVersion},
        wireguard::QuantumResistantState,
    };
    use serde_json::json;
    use talpid_types::net::IpVersion;

    /// Settings as the V18 daemon serializes them, with every removed field
    /// set away from its default, and the kept ones too.
    const V18_SETTINGS: &str = include_str!("v18_settings.json");

    fn v18_settings() -> serde_json::Value {
        serde_json::from_str(V18_SETTINGS).unwrap()
    }

    #[test]
    fn drops_the_wireguard_era_settings_of_a_v18_file() {
        let mut settings = v18_settings();

        migrate(&mut settings).unwrap();

        assert!(settings.get("obfuscation_settings").is_none());
        let wireguard = settings["tunnel_options"]["wireguard"].as_object().unwrap();
        assert!(!wireguard.contains_key("mtu"));
        assert!(!wireguard.contains_key("userspace"));
        let constraints = settings["relay_settings"]["normal"]["wireguard_constraints"]
            .as_object()
            .unwrap();
        assert!(!constraints.contains_key("allowed_ips"));
        assert_eq!(settings["settings_version"], json!(19));
    }

    #[test]
    fn keeps_every_other_setting() {
        let mut settings = v18_settings();

        migrate(&mut settings).unwrap();

        let settings: Settings = serde_json::from_value(settings).unwrap();
        assert_eq!(settings.settings_version, CURRENT_SETTINGS_VERSION);
        assert_eq!(
            settings.tunnel_options.wireguard.quantum_resistant,
            QuantumResistantState::Off
        );
        assert!(settings.tunnel_options.wireguard.daita.enabled);
        assert!(settings.tunnel_options.generic.enable_ipv6);
        assert!(settings.allow_lan);
        let RelaySettings::Normal(normal) = settings.relay_settings else {
            panic!("normal relay settings expected");
        };
        assert_eq!(
            normal.wireguard_constraints.ip_version,
            mullvad_types::constraints::Constraint::Only(IpVersion::V4)
        );
    }

    #[test]
    fn leaves_other_versions_alone() {
        let mut original = v18_settings();
        original["settings_version"] = json!(SettingsVersion::V17 as u64);
        let mut settings = original.clone();

        migrate(&mut settings).unwrap();

        assert_eq!(settings, original);
    }
}
