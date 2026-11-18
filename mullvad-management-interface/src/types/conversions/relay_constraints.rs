use crate::types::{FromProtobufTypeError, proto};
use mullvad_types::{
    constraints::Constraint, custom_list::Id, relay_constraints::GeographicLocationConstraint,
};
use std::str::FromStr;
use talpid_types::net::wireguard::ConnectionConfig;

impl TryFrom<&proto::WireguardConstraints>
    for mullvad_types::relay_constraints::WireguardConstraints
{
    type Error = FromProtobufTypeError;

    fn try_from(
        constraints: &proto::WireguardConstraints,
    ) -> Result<mullvad_types::relay_constraints::WireguardConstraints, Self::Error> {
        use mullvad_types::relay_constraints as mullvad_constraints;
        use talpid_types::net;

        let ip_version = constraints
            .ip_version
            .map(proto::IpVersion::try_from)
            .transpose()
            .map_err(|_| FromProtobufTypeError::invalid_argument("invalid IP protocol version"))?
            .map(net::IpVersion::from);

        Ok(mullvad_constraints::WireguardConstraints {
            ip_version: Constraint::from(ip_version),
            use_multihop: constraints.use_multihop,
            entry_location: constraints
                .entry_location
                .clone()
                .and_then(|loc| {
                    mullvad_types::relay_constraints::LocationConstraint::try_from(loc).ok()
                })
                .into(),
            entry_providers: providers_constraint_from_proto(&constraints.entry_providers),
            entry_ownership: try_ownership_constraint_from_i32(constraints.entry_ownership)?,
        })
    }
}

impl TryFrom<proto::RelaySettings> for mullvad_types::relay_constraints::RelaySettings {
    type Error = FromProtobufTypeError;

    fn try_from(
        settings: proto::RelaySettings,
    ) -> Result<mullvad_types::relay_constraints::RelaySettings, Self::Error> {
        use mullvad_types::{CustomTunnelEndpoint, relay_constraints as mullvad_constraints};

        let update_value = settings
            .endpoint
            .ok_or(FromProtobufTypeError::invalid_argument(
                "missing relay settings",
            ))?;

        match update_value {
            proto::relay_settings::Endpoint::Custom(settings) => {
                let config = settings
                    .config
                    .ok_or(FromProtobufTypeError::invalid_argument(
                        "missing relay connection config",
                    ))?;
                let config = ConnectionConfig::try_from(config)?;
                Ok(mullvad_constraints::RelaySettings::CustomTunnelEndpoint(
                    CustomTunnelEndpoint {
                        host: settings.host,
                        config,
                    },
                ))
            }

            proto::relay_settings::Endpoint::Normal(settings) => {
                let location = settings
                    .location
                    .and_then(|loc| {
                        mullvad_types::relay_constraints::LocationConstraint::try_from(loc).ok()
                    })
                    .into();
                let providers = providers_constraint_from_proto(&settings.providers);
                let ownership = try_ownership_constraint_from_i32(settings.ownership)?;

                let mut wireguard_constraints =
                    mullvad_constraints::WireguardConstraints::try_from(
                        &settings.wireguard_constraints.ok_or(
                            FromProtobufTypeError::invalid_argument(
                                "missing wireguard constraints",
                            ),
                        )?,
                    )?;

                // TODO Remove this block when the frontends support setting multihop entry filters.
                // This is needed in order to not change the current behavior (which
                // is that the ownership and providers from `RelaySettings` apply to both the entry
                // and exit multihop relays).
                {
                    wireguard_constraints.entry_ownership = ownership;
                    wireguard_constraints.entry_providers = providers.clone();
                }

                Ok(mullvad_constraints::RelaySettings::Normal(
                    mullvad_constraints::RelayConstraints {
                        location,
                        providers,
                        ownership,
                        wireguard_constraints,
                    },
                ))
            }
        }
    }
}

impl From<mullvad_types::relay_constraints::RelaySettings> for proto::RelaySettings {
    fn from(settings: mullvad_types::relay_constraints::RelaySettings) -> Self {
        use mullvad_types::relay_constraints::RelaySettings as MullvadRelaySettings;
        use proto::relay_settings;

        let endpoint = match settings {
            MullvadRelaySettings::CustomTunnelEndpoint(endpoint) => {
                relay_settings::Endpoint::Custom(proto::CustomRelaySettings {
                    host: endpoint.host,
                    config: Some(proto::WireguardConfig::from(endpoint.config)),
                })
            }
            MullvadRelaySettings::Normal(constraints) => {
                relay_settings::Endpoint::Normal(proto::NormalRelaySettings {
                    location: constraints
                        .location
                        .option()
                        .map(proto::LocationConstraint::from),
                    providers: convert_providers_constraint(&constraints.providers),
                    ownership: convert_ownership_constraint(&constraints.ownership) as i32,

                    wireguard_constraints: Some(proto::WireguardConstraints {
                        ip_version: constraints
                            .wireguard_constraints
                            .ip_version
                            .option()
                            .map(|ipv| i32::from(proto::IpVersion::from(ipv))),
                        use_multihop: constraints.wireguard_constraints.multihop(),
                        entry_location: constraints
                            .wireguard_constraints
                            .entry_location
                            .option()
                            .map(proto::LocationConstraint::from),
                        entry_providers: convert_providers_constraint(
                            &constraints.wireguard_constraints.entry_providers,
                        ),
                        entry_ownership: convert_ownership_constraint(
                            &constraints.wireguard_constraints.entry_ownership,
                        ) as i32,
                    }),
                })
            }
        };

        Self {
            endpoint: Some(endpoint),
        }
    }
}

impl From<mullvad_types::relay_constraints::TransportPort> for proto::TransportPort {
    fn from(port: mullvad_types::relay_constraints::TransportPort) -> Self {
        proto::TransportPort {
            protocol: proto::TransportProtocol::from(port.protocol) as i32,
            port: port.port.map(u32::from).option(),
        }
    }
}

impl From<mullvad_types::relay_constraints::LocationConstraint> for proto::LocationConstraint {
    fn from(location: mullvad_types::relay_constraints::LocationConstraint) -> Self {
        use mullvad_types::relay_constraints::LocationConstraint;
        match location {
            LocationConstraint::Location(location) => Self {
                r#type: Some(proto::location_constraint::Type::Location(
                    proto::GeographicLocationConstraint::from(location),
                )),
            },
            LocationConstraint::CustomList { list_id } => Self {
                r#type: Some(proto::location_constraint::Type::CustomList(
                    list_id.to_string(),
                )),
            },
        }
    }
}

impl TryFrom<proto::LocationConstraint> for mullvad_types::relay_constraints::LocationConstraint {
    type Error = FromProtobufTypeError;

    fn try_from(location: proto::LocationConstraint) -> Result<Self, Self::Error> {
        use mullvad_types::relay_constraints::LocationConstraint;
        let Some(typ) = location.r#type else {
            return Err(FromProtobufTypeError::invalid_argument(
                "Type of location constraint was not provided",
            ));
        };
        match typ {
            proto::location_constraint::Type::Location(location) => Ok(
                LocationConstraint::Location(GeographicLocationConstraint::try_from(location)?),
            ),
            proto::location_constraint::Type::CustomList(list_id) => {
                let location = LocationConstraint::CustomList {
                    list_id: Id::from_str(&list_id).map_err(|_| {
                        FromProtobufTypeError::invalid_argument("Id could not be parsed to a uuid")
                    })?,
                };
                Ok(location)
            }
        }
    }
}

impl From<GeographicLocationConstraint> for proto::GeographicLocationConstraint {
    fn from(location: mullvad_types::relay_constraints::GeographicLocationConstraint) -> Self {
        match location {
            GeographicLocationConstraint::Country(country) => Self {
                country,
                ..Default::default()
            },
            GeographicLocationConstraint::City(country, city) => Self {
                country,
                city: Some(city),
                hostname: None,
            },
            GeographicLocationConstraint::Hostname(country, city, hostname) => Self {
                country,
                city: Some(city),
                hostname: Some(hostname),
            },
        }
    }
}

impl TryFrom<proto::GeographicLocationConstraint> for GeographicLocationConstraint {
    type Error = FromProtobufTypeError;

    fn try_from(relay_location: proto::GeographicLocationConstraint) -> Result<Self, Self::Error> {
        match (
            relay_location.country,
            relay_location.city,
            relay_location.hostname,
        ) {
            (country, None, None) => Ok(GeographicLocationConstraint::Country(country)),
            (country, Some(city), None) => Ok(GeographicLocationConstraint::City(country, city)),
            (country, Some(city), Some(hostname)) => Ok(GeographicLocationConstraint::Hostname(
                country, city, hostname,
            )),
            (_country, None, Some(_hostname)) => Err(FromProtobufTypeError::invalid_argument(
                "Relay location contains hostname but no city",
            )),
        }
    }
}

impl TryFrom<proto::TransportPort> for mullvad_types::relay_constraints::TransportPort {
    type Error = FromProtobufTypeError;

    fn try_from(port: proto::TransportPort) -> Result<Self, Self::Error> {
        Ok(mullvad_types::relay_constraints::TransportPort {
            protocol: super::net::try_transport_protocol_from_i32(port.protocol)?,
            port: Constraint::from(port.port.map(|port| port as u16)),
        })
    }
}

impl From<mullvad_types::relay_constraints::RelayOverride> for proto::RelayOverride {
    fn from(r#override: mullvad_types::relay_constraints::RelayOverride) -> proto::RelayOverride {
        proto::RelayOverride {
            hostname: r#override.hostname,
            ipv4_addr_in: r#override.ipv4_addr_in.map(|addr| addr.to_string()),
            ipv6_addr_in: r#override.ipv6_addr_in.map(|addr| addr.to_string()),
        }
    }
}

impl TryFrom<proto::RelayOverride> for mullvad_types::relay_constraints::RelayOverride {
    type Error = FromProtobufTypeError;

    fn try_from(
        r#override: proto::RelayOverride,
    ) -> Result<mullvad_types::relay_constraints::RelayOverride, Self::Error> {
        Ok(mullvad_types::relay_constraints::RelayOverride {
            hostname: r#override.hostname,
            ipv4_addr_in: r#override
                .ipv4_addr_in
                .map(|addr| {
                    addr.parse().map_err(|_| {
                        FromProtobufTypeError::invalid_argument("invalid IPv4 address")
                    })
                })
                .transpose()?,
            ipv6_addr_in: r#override
                .ipv6_addr_in
                .map(|addr| {
                    addr.parse().map_err(|_| {
                        FromProtobufTypeError::invalid_argument("invalid IPv6 address")
                    })
                })
                .transpose()?,
        })
    }
}

pub fn providers_constraint_from_proto(
    providers: &[impl Into<String> + Clone],
) -> Constraint<mullvad_types::relay_constraints::Providers> {
    if !providers.is_empty() {
        Constraint::Only(
            mullvad_types::relay_constraints::Providers::new(providers.iter().cloned())
                .expect("Providers has been checked to not be empty"),
        )
    } else {
        Constraint::Any
    }
}

pub fn try_ownership_constraint_from_i32(
    ownership: i32,
) -> Result<Constraint<mullvad_types::relay_constraints::Ownership>, FromProtobufTypeError> {
    proto::Ownership::try_from(ownership)
        .map(ownership_constraint_from_proto)
        .map_err(|_| FromProtobufTypeError::invalid_argument("invalid ownership argument"))
}

pub fn ownership_constraint_from_proto(
    ownership: proto::Ownership,
) -> Constraint<mullvad_types::relay_constraints::Ownership> {
    use mullvad_types::relay_constraints::Ownership as MullvadOwnership;

    match ownership {
        proto::Ownership::Any => Constraint::Any,
        proto::Ownership::MullvadOwned => Constraint::Only(MullvadOwnership::MullvadOwned),
        proto::Ownership::Rented => Constraint::Only(MullvadOwnership::Rented),
    }
}

fn convert_providers_constraint(
    providers: &Constraint<mullvad_types::relay_constraints::Providers>,
) -> Vec<String> {
    match providers.as_ref() {
        Constraint::Any => vec![],
        Constraint::Only(providers) => Vec::from(providers.clone()),
    }
}

fn convert_ownership_constraint(
    ownership: &Constraint<mullvad_types::relay_constraints::Ownership>,
) -> proto::Ownership {
    use mullvad_types::relay_constraints::Ownership as MullvadOwnership;

    match ownership.as_ref() {
        Constraint::Any => proto::Ownership::Any,
        Constraint::Only(ownership) => match ownership {
            MullvadOwnership::MullvadOwned => proto::Ownership::MullvadOwned,
            MullvadOwnership::Rented => proto::Ownership::Rented,
        },
    }
}
