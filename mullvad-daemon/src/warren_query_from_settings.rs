//! Conversion `mullvad_types::RelaySettings` -> `WarrenRelayQuery`.
//!
//! Maps the Mullvad UI (countries/cities, custom lists, hostname) onto the
//! Warren filtering grammar (which only supports `Any`, `Country`, or
//! `(Country, City)` - no provider, ownership, multihop, obfuscation, nor
//! DAITA). Side-effect-free except for a `warn` log on the custom-list
//! fallback below.
//!
//! Behavior for non-mappable cases:
//! - `CustomList` (reference to a `CustomListsSettings`) -> fallback
//!   `Any`. Custom lists are not wired on the Warren path; a user
//!   who selects one will see "all active exits".
//! - `Hostname(country, city, _)` -> we keep `(country, city)` and
//!   drop the hostname (Warren has no concept of an individual host
//!   exposed to the UI).
//! - `RelaySettings::CustomTunnelEndpoint(_)` -> `Any` (edge case;
//!   the user should not be using a custom endpoint in Warren mode,
//!   it is a functional dead-end).

use mullvad_types::constraints::Constraint;
use mullvad_types::relay_constraints::{
    GeographicLocationConstraint, LocationConstraint as MullvadLocation, RelaySettings,
};
use talpid_types::net::IpVersion;
use warren_discovery_core::{
    IpAvailability, LocationConstraint as WarrenLocation, WarrenRelayQuery,
};

/// Converts `RelaySettings` (Mullvad) into a `WarrenRelayQuery` consumed
/// by `DaemonWarrenRelaySelector::select_for_attempt`.
///
/// Two VPN toggles filter the server list by endpoint family:
/// - "Device IP version" (`wireguard_constraints.ip_version`) maps to
///   [`IpAvailability`]: it restricts which family the client may dial,
///   filtering out exits without a reachable endpoint of that family.
/// - "In-tunnel IPv6" (`enable_ipv6`, from the generic tunnel options)
///   maps to `require_ipv6_egress`: when on, only exits that attest
///   working IPv6 egress are kept, so the client never negotiates
///   in-tunnel v6 against an exit that would blackhole it.
#[must_use]
pub fn relay_settings_to_warren_query(rs: &RelaySettings, enable_ipv6: bool) -> WarrenRelayQuery {
    let location = match rs {
        RelaySettings::Normal(constraints) => match &constraints.location {
            Constraint::Any => WarrenLocation::Any,
            Constraint::Only(MullvadLocation::Location(geo)) => match geo {
                GeographicLocationConstraint::Country(cc) => WarrenLocation::Country(cc.clone()),
                GeographicLocationConstraint::City(cc, city) => WarrenLocation::City {
                    country_code: cc.clone(),
                    city: city.clone(),
                },
                // Warren does not distinguish hosts within a city: we
                // restrict to the `(country, city)` of the hostname.
                GeographicLocationConstraint::Hostname(cc, city, _) => WarrenLocation::City {
                    country_code: cc.clone(),
                    city: city.clone(),
                },
            },
            // Mullvad custom lists point to a separate `CustomListsSettings`
            // not resolvable here. Until they are wired into Warren selection,
            // warn loudly rather than silently widening to every exit, so the
            // mismatch between the user's choice and the actual circuit is
            // visible in the logs.
            Constraint::Only(MullvadLocation::CustomList { .. }) => {
                log::warn!(
                    "Warren selection ignores the configured custom list and falls back to \
                     any exit: custom lists are not yet wired into the Warren relay grammar"
                );
                WarrenLocation::Any
            }
        },
        // The custom tunnel endpoint targets a specific host (IP/port +
        // pubkey) - unrelated to Warren selection based on
        // exits enrolled via warren-api. We leave `Any` to avoid
        // accidentally blocking the user.
        RelaySettings::CustomTunnelEndpoint(_) => WarrenLocation::Any,
    };

    // "Device IP version": restrict the dialable endpoint family.
    let ip_availability = match rs {
        RelaySettings::Normal(constraints) => match constraints.wireguard_constraints.ip_version {
            Constraint::Any => IpAvailability::Both,
            Constraint::Only(IpVersion::V4) => IpAvailability::Ipv4Only,
            Constraint::Only(IpVersion::V6) => IpAvailability::Ipv6Only,
        },
        RelaySettings::CustomTunnelEndpoint(_) => IpAvailability::Both,
    };

    WarrenRelayQuery::any()
        .with_location(location)
        .with_ip_availability(ip_availability)
        .with_require_ipv6_egress(enable_ipv6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mullvad_types::relay_constraints::RelayConstraints;

    use mullvad_types::relay_constraints::WireguardConstraints;

    fn settings_with_location(loc: Constraint<MullvadLocation>) -> RelaySettings {
        let constraints = RelayConstraints {
            location: loc,
            ..RelayConstraints::default()
        };
        RelaySettings::Normal(constraints)
    }

    fn settings_with_ip_version(v: Constraint<IpVersion>) -> RelaySettings {
        let constraints = RelayConstraints {
            wireguard_constraints: WireguardConstraints {
                ip_version: v,
                ..WireguardConstraints::default()
            },
            ..RelayConstraints::default()
        };
        RelaySettings::Normal(constraints)
    }

    #[test]
    fn any_yields_any() {
        let rs = settings_with_location(Constraint::Any);
        let q = relay_settings_to_warren_query(&rs, false);
        let any = WarrenRelayQuery::any();
        assert_eq!(format!("{q:?}"), format!("{any:?}"));
    }

    #[test]
    fn country_constraint_is_mapped() {
        let rs = settings_with_location(Constraint::Only(MullvadLocation::Location(
            GeographicLocationConstraint::Country("fr".into()),
        )));
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_location(WarrenLocation::Country("fr".into()));
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn city_constraint_is_mapped() {
        let rs = settings_with_location(Constraint::Only(MullvadLocation::Location(
            GeographicLocationConstraint::City("fr".into(), "par".into()),
        )));
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_location(WarrenLocation::City {
            country_code: "fr".into(),
            city: "par".into(),
        });
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn hostname_constraint_falls_back_to_city() {
        let rs = settings_with_location(Constraint::Only(MullvadLocation::Location(
            GeographicLocationConstraint::Hostname("se".into(), "sto".into(), "se1".into()),
        )));
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_location(WarrenLocation::City {
            country_code: "se".into(),
            city: "sto".into(),
        });
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn device_ip_version_v4_filters_to_ipv4_only() {
        // "Device IP version = IPv4" must restrict the dialable endpoint
        // family so v6-only exits are filtered out of the server list.
        let rs = settings_with_ip_version(Constraint::Only(IpVersion::V4));
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_ip_availability(IpAvailability::Ipv4Only);
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn device_ip_version_v6_filters_to_ipv6_only() {
        let rs = settings_with_ip_version(Constraint::Only(IpVersion::V6));
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_ip_availability(IpAvailability::Ipv6Only);
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn device_ip_version_any_keeps_both_families() {
        let rs = settings_with_ip_version(Constraint::Any);
        let q = relay_settings_to_warren_query(&rs, false);
        let expected = WarrenRelayQuery::any().with_ip_availability(IpAvailability::Both);
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn in_tunnel_ipv6_requires_v6_egress() {
        // "In-tunnel IPv6 = on" must keep only exits that attest working
        // v6 egress, so the client never blackholes in-tunnel v6.
        let rs = settings_with_location(Constraint::Any);
        let q = relay_settings_to_warren_query(&rs, true);
        let expected = WarrenRelayQuery::any().with_require_ipv6_egress(true);
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }
}
