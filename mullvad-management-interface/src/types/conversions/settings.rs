use crate::types::{FromProtobufTypeError, proto};
use mullvad_types::settings::CURRENT_SETTINGS_VERSION;

impl From<&mullvad_types::settings::Settings> for proto::Settings {
    fn from(settings: &mullvad_types::settings::Settings) -> Self {
        #[cfg(any(windows, target_os = "android", target_os = "macos"))]
        let split_tunnel = {
            let apps = settings
                .split_tunnel
                .apps
                .iter()
                .filter_map(|app| match app.clone().to_string() {
                    None => {
                        log::error!("Failed to convert application to string: {:?}", app);
                        None
                    }
                    string => string,
                })
                .collect();

            Some(proto::SplitTunnelSettings {
                enable_exclusions: settings.split_tunnel.enable_exclusions,
                apps,
            })
        };
        #[cfg(target_os = "linux")]
        let split_tunnel = None;

        Self {
            relay_settings: Some(proto::RelaySettings::from(settings.get_relay_settings())),
            allow_lan: settings.allow_lan,
            #[cfg(not(target_os = "android"))]
            lockdown_mode: settings.lockdown_mode,
            #[cfg(target_os = "android")]
            lockdown_mode: false,
            auto_connect: settings.auto_connect,
            tunnel_options: Some(proto::TunnelOptions::from(&settings.tunnel_options)),
            show_beta_releases: settings.show_beta_releases,
            obfuscation_settings: Some(proto::ObfuscationSettings::from(
                &settings.obfuscation_settings,
            )),
            split_tunnel,
            custom_lists: Some(proto::CustomListSettings::from(
                settings.custom_lists.clone(),
            )),
            api_access_methods: Some(proto::ApiAccessMethodSettings::from(
                settings.api_access_methods.clone(),
            )),
            relay_overrides: settings
                .relay_overrides
                .iter()
                .cloned()
                .map(proto::RelayOverride::from)
                .collect(),
            recents: settings.recents.clone().map(proto::Recents::from),
            update_default_location: settings.update_default_location,
            // None -> empty string on the wire (proto3 `string` has
            // no "absent"; we use "" as a sentinel for "unset").
            // Consistent with the reverse conversion on the
            // `try_from(SettingsProto)` side.
            warren_api_url: settings.warren_api_url.clone().unwrap_or_default(),
            // None -> 0 on the wire (proto3 `uint32` has no "absent";
            // 0 is outside the valid 1..=16 range so it is a safe
            // "unset" sentinel).
            warren_n_connections: u32::from(settings.warren_n_connections.unwrap_or_default()),
            // None -> 0 on the wire (0 bps is not a usable cap, so it
            // is a safe "unset" sentinel).
            warren_max_rate_bps: settings.warren_max_rate_bps.unwrap_or_default(),
            warren_multi_hop: Some(proto::WarrenMultiHopSettings::from(
                &settings.warren_multi_hop,
            )),
            warren_nat_pmp: Some(proto::NatPmpSettings::from(&settings.warren_nat_pmp)),
            warren_custom_exit: Some(proto::WarrenCustomExitSettings::from(
                &settings.warren_custom_exit,
            )),
            lan_networks: Some(proto::LanNetworks {
                networks: settings
                    .lan_networks()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                custom: settings.custom_lan_networks.is_some(),
            }),
        }
    }
}

/// The networks a client asked to share, or `None` for the built-in private ranges.
///
/// # Errors
///
/// [`FromProtobufTypeError::InvalidArgument`] for an entry that is not a network in CIDR
/// notation, or one too broad to be a local network.
pub fn custom_lan_networks(
    lan_networks: proto::LanNetworks,
) -> Result<Option<Vec<ipnetwork::IpNetwork>>, FromProtobufTypeError> {
    if !lan_networks.custom {
        return Ok(None);
    }
    let networks = lan_networks
        .networks
        .iter()
        .map(|network| {
            network.parse().map_err(|_| {
                FromProtobufTypeError::invalid_argument(format!("{network} is not a network"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    mullvad_types::settings::validate_lan_networks(networks)
        .map(Some)
        .map_err(|error| FromProtobufTypeError::invalid_argument(error.to_string()))
}

impl From<&mullvad_types::settings::WarrenMultiHopSettings> for proto::WarrenMultiHopSettings {
    fn from(value: &mullvad_types::settings::WarrenMultiHopSettings) -> Self {
        proto::WarrenMultiHopSettings {
            enabled: value.enabled,
            entry_country: value.entry_country.clone(),
            exit_country: value.exit_country.clone(),
            hpke_epoch_rotation: Some(
                prost_types::Duration::try_from(value.hpke_epoch_rotation).expect(
                    "WarrenMultiHopSettings.hpke_epoch_rotation must fit in prost Duration",
                ),
            ),
        }
    }
}

impl TryFrom<proto::WarrenMultiHopSettings> for mullvad_types::settings::WarrenMultiHopSettings {
    type Error = FromProtobufTypeError;

    fn try_from(value: proto::WarrenMultiHopSettings) -> Result<Self, Self::Error> {
        let hpke_epoch_rotation = value
            .hpke_epoch_rotation
            .map(std::time::Duration::try_from)
            .transpose()
            .map_err(|_| {
                FromProtobufTypeError::invalid_argument("invalid hpke_epoch_rotation duration")
            })?
            .unwrap_or_else(|| std::time::Duration::from_secs(4 * 60 * 60));

        Ok(mullvad_types::settings::WarrenMultiHopSettings {
            enabled: value.enabled,
            entry_country: value.entry_country,
            exit_country: value.exit_country,
            hpke_epoch_rotation,
        })
    }
}

impl From<&mullvad_types::settings::WarrenCustomExitSettings> for proto::WarrenCustomExitSettings {
    fn from(value: &mullvad_types::settings::WarrenCustomExitSettings) -> Self {
        proto::WarrenCustomExitSettings {
            enabled: value.enabled,
            endpoint: value.endpoint.clone(),
            pubkey_hex: value.pubkey_hex.clone(),
            x25519_multihop_pubkey_hex: value.x25519_multihop_pubkey_hex.clone(),
            exit_id_hex: value.exit_id_hex.clone(),
            cover_domain: value.cover_domain.clone(),
            label: value.label.clone(),
        }
    }
}

impl From<proto::WarrenCustomExitSettings> for mullvad_types::settings::WarrenCustomExitSettings {
    /// Infallible: all fields are opaque strings here. Content validity
    /// (parseable endpoint, well-formed pubkey) is checked downstream in
    /// `assemble_custom`, so a bad value persists as "inactive" rather
    /// than failing the whole settings conversion.
    fn from(value: proto::WarrenCustomExitSettings) -> Self {
        mullvad_types::settings::WarrenCustomExitSettings {
            enabled: value.enabled,
            endpoint: value.endpoint,
            pubkey_hex: value.pubkey_hex,
            x25519_multihop_pubkey_hex: value.x25519_multihop_pubkey_hex,
            exit_id_hex: value.exit_id_hex,
            cover_domain: value.cover_domain,
            label: value.label,
        }
    }
}

/// Map a `WarrenNatPmpProto` to its proto enum discriminant.
fn nat_pmp_proto_to_i32(p: mullvad_types::settings::WarrenNatPmpProto) -> i32 {
    use mullvad_types::settings::WarrenNatPmpProto;
    match p {
        WarrenNatPmpProto::Udp => proto::nat_pmp_settings::Proto::Udp as i32,
        WarrenNatPmpProto::Tcp => proto::nat_pmp_settings::Proto::Tcp as i32,
        WarrenNatPmpProto::Both => proto::nat_pmp_settings::Proto::Both as i32,
    }
}

/// Map a proto enum discriminant back to a `WarrenNatPmpProto`.
fn nat_pmp_proto_from_i32(
    v: i32,
) -> Result<mullvad_types::settings::WarrenNatPmpProto, FromProtobufTypeError> {
    use mullvad_types::settings::WarrenNatPmpProto;
    match proto::nat_pmp_settings::Proto::try_from(v) {
        Ok(proto::nat_pmp_settings::Proto::Udp) => Ok(WarrenNatPmpProto::Udp),
        Ok(proto::nat_pmp_settings::Proto::Tcp) => Ok(WarrenNatPmpProto::Tcp),
        Ok(proto::nat_pmp_settings::Proto::Both) => Ok(WarrenNatPmpProto::Both),
        Err(_) => Err(FromProtobufTypeError::invalid_argument(
            "invalid NatPmpSettings.protocol enum value",
        )),
    }
}

impl From<&mullvad_types::settings::WarrenNatPmpRule> for proto::nat_pmp_settings::Rule {
    fn from(value: &mullvad_types::settings::WarrenNatPmpRule) -> Self {
        proto::nat_pmp_settings::Rule {
            protocol: nat_pmp_proto_to_i32(value.protocol),
            suggested_external_port: u32::from(value.suggested_external_port),
            internal_port: u32::from(value.internal_port),
        }
    }
}

impl TryFrom<proto::nat_pmp_settings::Rule> for mullvad_types::settings::WarrenNatPmpRule {
    type Error = FromProtobufTypeError;

    fn try_from(value: proto::nat_pmp_settings::Rule) -> Result<Self, Self::Error> {
        let protocol = nat_pmp_proto_from_i32(value.protocol)?;
        let suggested_external_port =
            u16::try_from(value.suggested_external_port).map_err(|_| {
                FromProtobufTypeError::invalid_argument(
                    "NatPmpRule.suggested_external_port > 65535",
                )
            })?;
        let internal_port = u16::try_from(value.internal_port).map_err(|_| {
            FromProtobufTypeError::invalid_argument("NatPmpRule.internal_port > 65535")
        })?;
        Ok(mullvad_types::settings::WarrenNatPmpRule {
            protocol,
            suggested_external_port,
            internal_port,
        })
    }
}

impl From<&mullvad_types::settings::WarrenNatPmpSettings> for proto::NatPmpSettings {
    fn from(value: &mullvad_types::settings::WarrenNatPmpSettings) -> Self {
        proto::NatPmpSettings {
            enabled: value.enabled,
            lifetime_secs: value.lifetime_secs,
            rules: value
                .rules
                .iter()
                .map(proto::nat_pmp_settings::Rule::from)
                .collect(),
            protocol: nat_pmp_proto_to_i32(value.protocol),
            suggested_external_port: u32::from(value.suggested_external_port),
            internal_port: u32::from(value.internal_port),
        }
    }
}

impl TryFrom<proto::NatPmpSettings> for mullvad_types::settings::WarrenNatPmpSettings {
    type Error = FromProtobufTypeError;

    fn try_from(value: proto::NatPmpSettings) -> Result<Self, Self::Error> {
        let protocol = nat_pmp_proto_from_i32(value.protocol)?;
        // Ports are 16-bit; reject overflow rather than silently
        // truncating, otherwise a malformed gRPC call would silently
        // wrap to e.g. (65536 -> 0).
        let suggested_external_port =
            u16::try_from(value.suggested_external_port).map_err(|_| {
                FromProtobufTypeError::invalid_argument(
                    "NatPmpSettings.suggested_external_port > 65535",
                )
            })?;
        let internal_port = u16::try_from(value.internal_port).map_err(|_| {
            FromProtobufTypeError::invalid_argument("NatPmpSettings.internal_port > 65535")
        })?;
        let rules = value
            .rules
            .into_iter()
            .map(mullvad_types::settings::WarrenNatPmpRule::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(mullvad_types::settings::WarrenNatPmpSettings {
            enabled: value.enabled,
            lifetime_secs: value.lifetime_secs,
            rules,
            protocol,
            suggested_external_port,
            internal_port,
        })
    }
}

impl From<&mullvad_types::settings::DnsOptions> for proto::DnsOptions {
    fn from(options: &mullvad_types::settings::DnsOptions) -> Self {
        use proto::dns_options;

        proto::DnsOptions {
            state: match options.state {
                mullvad_types::settings::DnsState::Default => dns_options::DnsState::Default as i32,
                mullvad_types::settings::DnsState::Custom => dns_options::DnsState::Custom as i32,
            },
            default_options: Some(proto::DefaultDnsOptions {
                block_ads: options.default_options.block_ads,
                block_trackers: options.default_options.block_trackers,
                block_malware: options.default_options.block_malware,
                block_adult_content: options.default_options.block_adult_content,
                block_gambling: options.default_options.block_gambling,
                block_social_media: options.default_options.block_social_media,
            }),
            custom_options: Some(proto::CustomDnsOptions {
                addresses: options
                    .custom_options
                    .addresses
                    .iter()
                    .map(|addr| addr.to_string())
                    .collect(),
            }),
            allow_external_dns: options.allow_external_dns,
        }
    }
}

impl From<&mullvad_types::settings::TunnelOptions> for proto::TunnelOptions {
    fn from(options: &mullvad_types::settings::TunnelOptions) -> Self {
        proto::TunnelOptions {
            mtu: options.wireguard.mtu.map(u32::from),
            rotation_interval: None,
            quantum_resistant: Some(proto::QuantumResistantState::from(
                options.wireguard.quantum_resistant,
            )),
            #[cfg(daita)]
            daita: Some(proto::DaitaSettings::from(options.wireguard.daita.clone())),
            #[cfg(not(daita))]
            daita: None,
            enable_ipv6: options.generic.enable_ipv6,
            dns_options: Some(proto::DnsOptions::from(&options.dns_options)),
            userspace: options.wireguard.userspace,
        }
    }
}

impl TryFrom<proto::Settings> for mullvad_types::settings::Settings {
    type Error = FromProtobufTypeError;

    fn try_from(settings: proto::Settings) -> Result<Self, Self::Error> {
        let relay_settings =
            settings
                .relay_settings
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing relay settings",
                ))?;
        let tunnel_options =
            settings
                .tunnel_options
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing tunnel options",
                ))?;
        let obfuscation_settings =
            settings
                .obfuscation_settings
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing obfuscation settings",
                ))?;
        let custom_lists_settings =
            settings
                .custom_lists
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing custom lists settings",
                ))?;
        let api_access_methods_settings =
            settings
                .api_access_methods
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing api access methods settings",
                ))?;
        #[cfg(any(windows, target_os = "android", target_os = "macos"))]
        let split_tunnel = settings
            .split_tunnel
            .ok_or(FromProtobufTypeError::invalid_argument(
                "missing split tunnel options",
            ))?;

        Ok(Self {
            relay_settings: mullvad_types::relay_constraints::RelaySettings::try_from(
                relay_settings,
            )?,
            allow_lan: settings.allow_lan,
            custom_lan_networks: settings
                .lan_networks
                .map(custom_lan_networks)
                .transpose()?
                .flatten(),
            #[cfg(not(target_os = "android"))]
            lockdown_mode: settings.lockdown_mode,
            auto_connect: settings.auto_connect,
            tunnel_options: mullvad_types::settings::TunnelOptions::try_from(tunnel_options)?,
            relay_overrides: settings
                .relay_overrides
                .into_iter()
                .map(mullvad_types::relay_constraints::RelayOverride::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            show_beta_releases: settings.show_beta_releases,
            #[cfg(any(windows, target_os = "android", target_os = "macos"))]
            split_tunnel: mullvad_types::settings::SplitTunnelSettings::from(split_tunnel),
            obfuscation_settings: mullvad_types::relay_constraints::ObfuscationSettings::try_from(
                obfuscation_settings,
            )?,
            // NOTE: This field is set based on mullvad-types. It's not based on the actual settings
            // version.
            settings_version: CURRENT_SETTINGS_VERSION,
            custom_lists: mullvad_types::custom_list::CustomListsSettings::try_from(
                custom_lists_settings,
            )?,
            api_access_methods: mullvad_types::access_method::Settings::try_from(
                api_access_methods_settings,
            )?,
            recents: Some(vec![]),
            update_default_location: settings.update_default_location,
            // HACK: The deamon should never read this random settings blob from a random client.
            // We should look into separating the serializable settings object that pass accross
            // gRPC from the daemon's trusted settings. There are multiple fields that would not be
            // included in the serializable settings, such as the below value.
            #[cfg(not(target_os = "android"))]
            rollout_threshold_seed: None,
            // Empty string proto -> None on the mullvad_types side.
            // Lets the gRPC UI/CLI unset the field by sending "".
            warren_api_url: if settings.warren_api_url.is_empty() {
                None
            } else {
                Some(settings.warren_api_url)
            },
            // 0 on the wire -> None (unset, compiled default). Other
            // values must fit u8; range validation (1..=16) is done at
            // the SetWarrenNConnections rpc and again at
            // parameter-production time.
            warren_n_connections: match settings.warren_n_connections {
                0 => None,
                n => Some(u8::try_from(n).map_err(|_| {
                    FromProtobufTypeError::invalid_argument("warren_n_connections out of u8 range")
                })?),
            },
            // 0 on the wire -> None (unset = unlimited).
            warren_max_rate_bps: match settings.warren_max_rate_bps {
                0 => None,
                bps => Some(bps),
            },
            warren_multi_hop: settings
                .warren_multi_hop
                .map(mullvad_types::settings::WarrenMultiHopSettings::try_from)
                .transpose()?
                .unwrap_or_default(),
            warren_nat_pmp: settings
                .warren_nat_pmp
                .map(mullvad_types::settings::WarrenNatPmpSettings::try_from)
                .transpose()?
                .unwrap_or_default(),
            warren_custom_exit: settings
                .warren_custom_exit
                .map(mullvad_types::settings::WarrenCustomExitSettings::from)
                .unwrap_or_default(),
            // TOFU pinning storage is daemon-internal: never round-trip
            // through gRPC `SetSettings` (would let a gRPC client wipe
            // the pin table). Default-initialise from a gRPC update;
            // the daemon retains its own copy.
            warren_pinned_exit_pubkeys: mullvad_types::settings::WarrenPinnedExitPubkeys::default(),
            // Same reason as the pin table: the cross-environment yield is
            // daemon-internal, and it is the one record that decides whether
            // a connect is refused. Round-tripping it through `SetSettings`
            // would let any local client clear a stand-down without the
            // `ClearEnvYield` guard, which is the whole check.
            warren_env_yield: None,
        })
    }
}

#[cfg(any(windows, target_os = "android", target_os = "macos"))]
impl From<proto::SplitTunnelSettings> for mullvad_types::settings::SplitTunnelSettings {
    fn from(value: proto::SplitTunnelSettings) -> Self {
        use mullvad_types::settings::{SplitApp, SplitTunnelSettings};
        SplitTunnelSettings {
            enable_exclusions: value.enable_exclusions,
            apps: value.apps.into_iter().map(SplitApp::from).collect(),
        }
    }
}

impl TryFrom<proto::TunnelOptions> for mullvad_types::settings::TunnelOptions {
    type Error = FromProtobufTypeError;

    fn try_from(options: proto::TunnelOptions) -> Result<Self, Self::Error> {
        use talpid_types::net;

        let dns_options = options
            .dns_options
            .ok_or(FromProtobufTypeError::invalid_argument(
                "missing tunnel DNS options",
            ))?;

        Ok(Self {
            wireguard: mullvad_types::wireguard::TunnelOptions {
                mtu: options.mtu.map(|mtu| mtu as u16),
                quantum_resistant: options
                    .quantum_resistant
                    .map(mullvad_types::wireguard::QuantumResistantState::try_from)
                    .ok_or(FromProtobufTypeError::invalid_argument(
                        "missing quantum resistant state",
                    ))??,
                #[cfg(daita)]
                daita: options
                    .daita
                    .map(mullvad_types::wireguard::DaitaSettings::from)
                    .ok_or(FromProtobufTypeError::invalid_argument(
                        "missing daita settings",
                    ))?,
                userspace: options.userspace,
            },
            generic: net::GenericTunnelOptions {
                enable_ipv6: options.enable_ipv6,
            },
            dns_options: mullvad_types::settings::DnsOptions::try_from(dns_options)?,
        })
    }
}

impl TryFrom<proto::DnsOptions> for mullvad_types::settings::DnsOptions {
    type Error = FromProtobufTypeError;

    fn try_from(options: proto::DnsOptions) -> Result<Self, Self::Error> {
        use mullvad_types::settings::{
            CustomDnsOptions as MullvadCustomDnsOptions,
            DefaultDnsOptions as MullvadDefaultDnsOptions, DnsOptions as MullvadDnsOptions,
            DnsState as MullvadDnsState,
        };

        let state = match proto::dns_options::DnsState::try_from(options.state) {
            Ok(proto::dns_options::DnsState::Default) => MullvadDnsState::Default,
            Ok(proto::dns_options::DnsState::Custom) => MullvadDnsState::Custom,
            Err(_) => {
                return Err(FromProtobufTypeError::invalid_argument(
                    "invalid DNS options state",
                ));
            }
        };

        let default_options =
            options
                .default_options
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing default DNS options",
                ))?;
        let custom_options =
            options
                .custom_options
                .ok_or(FromProtobufTypeError::invalid_argument(
                    "missing default DNS options",
                ))?;

        Ok(MullvadDnsOptions {
            state,
            default_options: MullvadDefaultDnsOptions {
                block_ads: default_options.block_ads,
                block_trackers: default_options.block_trackers,
                block_malware: default_options.block_malware,
                block_adult_content: default_options.block_adult_content,
                block_gambling: default_options.block_gambling,
                block_social_media: default_options.block_social_media,
            },
            custom_options: MullvadCustomDnsOptions {
                addresses: custom_options
                    .addresses
                    .into_iter()
                    .map(|addr| {
                        addr.parse().map_err(|_| {
                            FromProtobufTypeError::invalid_argument("invalid IP address")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            },
            allow_external_dns: options.allow_external_dns,
        })
    }
}

impl From<Vec<mullvad_types::settings::Recent>> for proto::Recents {
    fn from(recents: Vec<mullvad_types::settings::Recent>) -> Self {
        proto::Recents {
            recents: recents.into_iter().map(proto::Recent::from).collect(),
        }
    }
}

impl From<mullvad_types::settings::Recent> for proto::Recent {
    fn from(recent: mullvad_types::settings::Recent) -> Self {
        match recent {
            mullvad_types::settings::Recent::Singlehop(location) => Self {
                r#type: Some(proto::recent::Type::Singlehop(location.into())),
            },
            mullvad_types::settings::Recent::Multihop { entry, exit } => Self {
                r#type: Some(proto::recent::Type::Multihop(proto::MultihopRecent {
                    entry: Some(entry.into()),
                    exit: Some(exit.into()),
                })),
            },
        }
    }
}

#[cfg(test)]
mod warren_multi_hop_conversion_tests {
    use super::*;
    use mullvad_types::settings::WarrenMultiHopSettings;
    use std::time::Duration;

    #[test]
    fn default_warren_multi_hop_settings_match_v1_doctrine() {
        let defaults = WarrenMultiHopSettings::default();
        assert!(
            !defaults.enabled,
            "warren-multihop OFF by default per doctrine `warren_multihop_doctrine_v1`"
        );
        assert!(defaults.entry_country.is_empty());
        assert!(defaults.exit_country.is_empty());
        assert_eq!(
            defaults.hpke_epoch_rotation,
            Duration::from_secs(4 * 60 * 60),
            "default HPKE epoch rotation = 4h"
        );
    }

    #[test]
    fn proto_roundtrip_preserves_all_fields() {
        let original = WarrenMultiHopSettings {
            enabled: true,
            entry_country: "fr".to_string(),
            exit_country: "de".to_string(),
            hpke_epoch_rotation: Duration::from_secs(7200),
        };
        let proto = proto::WarrenMultiHopSettings::from(&original);
        let back = WarrenMultiHopSettings::try_from(proto).expect("roundtrip must succeed");
        assert_eq!(original, back);
    }

    #[test]
    fn proto_roundtrip_with_defaults_preserves_doctrine() {
        let original = WarrenMultiHopSettings::default();
        let proto = proto::WarrenMultiHopSettings::from(&original);
        let back = WarrenMultiHopSettings::try_from(proto).expect("roundtrip");
        assert_eq!(original, back);
    }

    #[test]
    fn try_from_missing_duration_falls_back_to_4h() {
        let proto = proto::WarrenMultiHopSettings {
            enabled: true,
            entry_country: "se".to_string(),
            exit_country: "no".to_string(),
            hpke_epoch_rotation: None,
        };
        let back = WarrenMultiHopSettings::try_from(proto).expect("None rotation accepted");
        assert_eq!(back.hpke_epoch_rotation, Duration::from_secs(4 * 60 * 60));
    }

    #[test]
    fn try_from_negative_duration_is_rejected() {
        let proto = proto::WarrenMultiHopSettings {
            enabled: false,
            entry_country: String::new(),
            exit_country: String::new(),
            hpke_epoch_rotation: Some(prost_types::Duration {
                seconds: -1,
                nanos: 0,
            }),
        };
        let result = WarrenMultiHopSettings::try_from(proto);
        assert!(
            result.is_err(),
            "negative duration must be rejected as invalid"
        );
    }
}

#[cfg(test)]
mod warren_nat_pmp_conversion_tests {
    use super::*;
    use mullvad_types::settings::{WarrenNatPmpProto, WarrenNatPmpRule, WarrenNatPmpSettings};

    #[test]
    fn proto_roundtrip_preserves_multi_port_rules() {
        let original = WarrenNatPmpSettings {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![
                WarrenNatPmpRule {
                    protocol: WarrenNatPmpProto::Udp,
                    suggested_external_port: 51820,
                    internal_port: 51820,
                },
                WarrenNatPmpRule {
                    protocol: WarrenNatPmpProto::Tcp,
                    suggested_external_port: 0,
                    internal_port: 8080,
                },
                WarrenNatPmpRule {
                    protocol: WarrenNatPmpProto::Both,
                    suggested_external_port: 27015,
                    internal_port: 27015,
                },
            ],
            protocol: WarrenNatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
        };
        let proto = proto::NatPmpSettings::from(&original);
        assert_eq!(proto.rules.len(), 3);
        let back = WarrenNatPmpSettings::try_from(proto).expect("roundtrip must succeed");
        assert_eq!(original, back);
    }

    #[test]
    fn proto_roundtrip_empty_rules_preserves_defaults() {
        let original = WarrenNatPmpSettings::default();
        let proto = proto::NatPmpSettings::from(&original);
        assert!(proto.rules.is_empty());
        let back = WarrenNatPmpSettings::try_from(proto).expect("roundtrip");
        assert_eq!(original, back);
    }

    #[test]
    fn rule_port_overflow_is_rejected() {
        let proto = proto::NatPmpSettings {
            enabled: true,
            lifetime_secs: 60,
            rules: vec![proto::nat_pmp_settings::Rule {
                protocol: proto::nat_pmp_settings::Proto::Udp as i32,
                suggested_external_port: 70000, // > 65535
                internal_port: 22,
            }],
            protocol: proto::nat_pmp_settings::Proto::Udp as i32,
            suggested_external_port: 0,
            internal_port: 0,
        };
        assert!(WarrenNatPmpSettings::try_from(proto).is_err());
    }
}

#[cfg(test)]
mod lan_networks_conversion_tests {
    use super::*;
    use mullvad_types::settings::Settings;
    use talpid_types::net::ALLOWED_LAN_NETS;

    fn lan_networks(networks: &[&str], custom: bool) -> proto::LanNetworks {
        proto::LanNetworks {
            networks: networks.iter().map(|n| n.to_string()).collect(),
            custom,
        }
    }

    /// The UI shows the list the firewall is using, so an uncustomised
    /// daemon still sends the built-in ranges, marked as not custom.
    #[test]
    fn default_settings_send_the_built_in_ranges_as_not_custom() {
        let proto = proto::Settings::from(&Settings::default());
        let sent = proto.lan_networks.expect("always sent");
        assert!(!sent.custom);
        let expected: Vec<String> = ALLOWED_LAN_NETS.iter().map(|n| n.to_string()).collect();
        assert_eq!(sent.networks, expected);
    }

    #[test]
    fn a_custom_list_survives_a_settings_round_trip() {
        let original = Settings {
            custom_lan_networks: Some(vec![
                "192.168.0.0/16".parse().unwrap(),
                "400::/7".parse().unwrap(),
            ]),
            ..Default::default()
        };
        let back = Settings::try_from(proto::Settings::from(&original)).expect("round trip");
        assert_eq!(back.custom_lan_networks, original.custom_lan_networks);
    }

    /// `custom = false` is how a client resets the list, whatever it lists.
    #[test]
    fn not_custom_means_the_built_in_ranges() {
        let parsed = custom_lan_networks(lan_networks(&["400::/7"], false)).expect("valid");
        assert_eq!(parsed, None);
    }

    #[test]
    fn a_custom_list_is_validated_and_normalised() {
        let parsed =
            custom_lan_networks(lan_networks(&["192.168.1.7/24", "400::/7"], true)).expect("valid");
        assert_eq!(
            parsed,
            Some(vec![
                "192.168.1.0/24".parse().unwrap(),
                "400::/7".parse().unwrap()
            ])
        );
    }

    #[test]
    fn rejects_a_string_that_is_not_a_network() {
        let error =
            custom_lan_networks(lan_networks(&["my-server"], true)).expect_err("not a network");
        assert!(matches!(error, FromProtobufTypeError::InvalidArgument(_)));
    }

    #[test]
    fn rejects_a_network_too_broad_to_be_local() {
        let error = custom_lan_networks(lan_networks(&["0.0.0.0/0"], true))
            .expect_err("a default route is not a local network");
        assert!(matches!(error, FromProtobufTypeError::InvalidArgument(_)));
    }
}
