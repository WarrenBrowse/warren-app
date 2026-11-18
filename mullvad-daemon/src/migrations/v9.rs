use mullvad_types::settings::SettingsVersion;

use super::{Error, Result};

// ======================================================
// Section for vendoring types and values that
// this settings version depend on. See `mod.rs`.
type JsonSettings = serde_json::Map<String, serde_json::Value>;

// ======================================================

/// This migration onboards the Android app's split tunnel settings into the daemon's settings.
///
/// Until now, split tunneling has been completely handled client side by the Android app. This
/// includes keeping track of the setting itself (enabled / disabled) as well as all apps whose
/// traffic is supposed to be routed outside of any active tunnel. This migration reads all split
/// apps which has been stored by the Android client and writes them to the daemon's settings,
/// adding the 'split_tunnel' key to the settings object in the process.
///
/// # Note
/// This `migrate` function needs to get passed a `settings_dir` to work on Android. This is
/// because the Android client will pass the settings directory when initializing the daemon,
/// which means that we can not know ahead of time where the settings are stored.
pub fn migrate(settings: &mut serde_json::Value) -> Result<()> {
    if !version_matches(settings) {
        return Ok(());
    }

    log::info!("Migrating settings format to V10");

    let json_blob = to_settings_object(settings)?;

    // TODO: Remove this comment when closing the migration:
    // While this is an open migration, we check to see if the split tunnel apps have been migrated
    // already. If so, we don't want to run the migration code again. The call to
    // `split_tunnel_subkey_exists` can safely be removed when closing this migration.

    json_blob["settings_version"] = serde_json::json!(SettingsVersion::V10);

    Ok(())
}

fn version_matches(settings: &serde_json::Value) -> bool {
    settings
        .get("settings_version")
        .map(|version| version == SettingsVersion::V9 as u64)
        .unwrap_or(false)
}

/// Represent the settings blob for what it is: A JSON-object.
fn to_settings_object(settings: &mut serde_json::Value) -> Result<&mut JsonSettings> {
    settings
        .as_object_mut()
        .ok_or(Error::InvalidSettingsContent)
}

#[cfg(test)]
mod test {
    use super::{migrate, version_matches};

    /// Assert that tunnel type is migrated
    #[test]
    fn test_v9_to_v10_migration() {
        // TODO: Also test the case where the location is not an openvpn relay and the tunnel type is any
        let mut old_settings = serde_json::from_str(V9_SETTINGS).unwrap();

        assert!(version_matches(&old_settings));
        migrate(&mut old_settings).unwrap();
        let new_settings: serde_json::Value = serde_json::from_str(V10_SETTINGS).unwrap();

        eprintln!(
            "old_settings: {}",
            serde_json::to_string_pretty(&old_settings).unwrap()
        );
        eprintln!(
            "new_settings: {}",
            serde_json::to_string_pretty(&new_settings).unwrap()
        );

        assert_eq!(&old_settings, &new_settings);
    }

    /// This settings blob contains no constraint for tunnel type
    pub const V9_SETTINGS: &str = r#"
{
  "relay_settings": {
    "normal": {
      "location": {
        "only": {
          "location": {
            "hostname": "at-vie-ovpn-001"
          }
        }
      },
      "providers": "any",
      "ownership": "any",
      "tunnel_protocol": "any",
      "wireguard_constraints": {
        "port": "any",
        "ip_version": "any",
        "use_multihop": false,
        "entry_location": {
          "only": {
            "location": {
              "country": "se"
            }
          }
        }
      },
      "openvpn_constraints": {
        "port": "any"
      }
    }
  },
  "bridge_settings": {
    "bridge_type": "normal",
    "normal": {
      "location": "any",
      "providers": "any",
      "ownership": "any"
    },
    "custom": null
  },
  "obfuscation_settings": {
    "selected_obfuscation": "auto",
    "udp2tcp": {
      "port": "any"
    }
  },
  "bridge_state": "auto",
  "custom_lists": {
    "custom_lists": []
  },
  "api_access_methods": {
    "direct": {
      "id": "d81121bf-c942-4ca4-971f-8ea6581bc915",
      "name": "Direct",
      "enabled": true,
      "access_method": {
        "built_in": "direct"
      }
    },
    "mullvad_bridges": {
      "id": "92135711-534d-4950-963d-93e446a792e4",
      "name": "Mullvad Bridges",
      "enabled": true,
      "access_method": {
        "built_in": "bridge"
      }
    },
    "custom": []
  },
  "allow_lan": false,
  "block_when_disconnected": false,
  "auto_connect": false,
  "tunnel_options": {
    "openvpn": {
      "mssfix": null
    },
    "wireguard": {
      "mtu": null,
      "quantum_resistant": "auto",
      "rotation_interval": null
    },
    "generic": {
      "enable_ipv6": false
    },
    "dns_options": {
      "state": "default",
      "default_options": {
        "block_ads": false,
        "block_trackers": false,
        "block_malware": false,
        "block_adult_content": false,
        "block_gambling": false,
        "block_social_media": false
      },
      "custom_options": {
        "addresses": []
      }
    }
  },
  "relay_overrides": [],
  "show_beta_releases": true,
  "settings_version": 9
}
"#;

    /// This settings blob does not contain an "any" tunnel type
    pub const V10_SETTINGS: &str = r#"
{
  "relay_settings": {
    "normal": {
      "location": {
        "only": {
          "location": {
            "hostname": "at-vie-ovpn-001"
          }
        }
      },
      "providers": "any",
      "ownership": "any",
      "tunnel_protocol": "any",
      "wireguard_constraints": {
        "port": "any",
        "ip_version": "any",
        "use_multihop": false,
        "entry_location": {
          "only": {
            "location": {
              "country": "se"
            }
          }
        }
      },
      "openvpn_constraints": {
        "port": "any"
      }
    }
  },
  "bridge_settings": {
    "bridge_type": "normal",
    "normal": {
      "location": "any",
      "providers": "any",
      "ownership": "any"
    },
    "custom": null
  },
  "obfuscation_settings": {
    "selected_obfuscation": "auto",
    "udp2tcp": {
      "port": "any"
    }
  },
  "bridge_state": "auto",
  "custom_lists": {
    "custom_lists": []
  },
  "api_access_methods": {
    "direct": {
      "id": "d81121bf-c942-4ca4-971f-8ea6581bc915",
      "name": "Direct",
      "enabled": true,
      "access_method": {
        "built_in": "direct"
      }
    },
    "mullvad_bridges": {
      "id": "92135711-534d-4950-963d-93e446a792e4",
      "name": "Mullvad Bridges",
      "enabled": true,
      "access_method": {
        "built_in": "bridge"
      }
    },
    "custom": []
  },
  "allow_lan": false,
  "block_when_disconnected": false,
  "auto_connect": false,
  "tunnel_options": {
    "openvpn": {
      "mssfix": null
    },
    "wireguard": {
      "mtu": null,
      "quantum_resistant": "auto",
      "rotation_interval": null
    },
    "generic": {
      "enable_ipv6": false
    },
    "dns_options": {
      "state": "default",
      "default_options": {
        "block_ads": false,
        "block_trackers": false,
        "block_malware": false,
        "block_adult_content": false,
        "block_gambling": false,
        "block_social_media": false
      },
      "custom_options": {
        "addresses": []
      }
    }
  },
  "relay_overrides": [],
  "show_beta_releases": true,
  "settings_version": 10
}
"#;
}
