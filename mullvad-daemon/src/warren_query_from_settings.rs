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
use warren_relay_selector::{LocationConstraint as WarrenLocation, WarrenRelayQuery};

/// Converts `RelaySettings` (Mullvad) into a `WarrenRelayQuery` consumed
/// by `DaemonWarrenRelaySelector::select_for_attempt`.
#[must_use]
pub fn relay_settings_to_warren_query(rs: &RelaySettings) -> WarrenRelayQuery {
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
    WarrenRelayQuery::any().with_location(location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mullvad_types::relay_constraints::RelayConstraints;

    fn settings_with_location(loc: Constraint<MullvadLocation>) -> RelaySettings {
        let constraints = RelayConstraints {
            location: loc,
            ..RelayConstraints::default()
        };
        RelaySettings::Normal(constraints)
    }

    #[test]
    fn any_yields_any() {
        let rs = settings_with_location(Constraint::Any);
        let q = relay_settings_to_warren_query(&rs);
        let any = WarrenRelayQuery::any();
        assert_eq!(format!("{q:?}"), format!("{any:?}"));
    }

    #[test]
    fn country_constraint_is_mapped() {
        let rs = settings_with_location(Constraint::Only(MullvadLocation::Location(
            GeographicLocationConstraint::Country("fr".into()),
        )));
        let q = relay_settings_to_warren_query(&rs);
        let expected = WarrenRelayQuery::any().with_location(WarrenLocation::Country("fr".into()));
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }

    #[test]
    fn city_constraint_is_mapped() {
        let rs = settings_with_location(Constraint::Only(MullvadLocation::Location(
            GeographicLocationConstraint::City("fr".into(), "par".into()),
        )));
        let q = relay_settings_to_warren_query(&rs);
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
        let q = relay_settings_to_warren_query(&rs);
        let expected = WarrenRelayQuery::any().with_location(WarrenLocation::City {
            country_code: "se".into(),
            city: "sto".into(),
        });
        assert_eq!(format!("{q:?}"), format!("{expected:?}"));
    }
}
