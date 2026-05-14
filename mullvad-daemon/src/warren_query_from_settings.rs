//! Conversion `mullvad_types::RelaySettings` → `WarrenRelayQuery`.
//!
//! Pure function : pas d'I/O. Mappe l'UI Mullvad (countries/cities,
//! custom lists, hostname) sur la grammaire de filtrage Warren (qui
//! ne supporte que `Any`, `Country`, ou `(Country, City)` — pas de
//! provider, ownership, multihop, obfuscation, ni DAITA).
//!
//! Comportement des cas non-mappables :
//! - `CustomList` (référence vers une `CustomListsSettings`) → fallback
//!   `Any`. Les custom lists ne sont pas wired sur le path Warren ; un
//!   user qui en sélectionne une verra "tous les exits actifs".
//! - `Hostname(country, city, _)` → on conserve `(country, city)` et on
//!   drop le hostname (Warren n'a pas la notion d'host individuel exposé
//!   à l'UI).
//! - `RelaySettings::CustomTunnelEndpoint(_)` → `Any` (cas edge ;
//!   l'utilisateur ne devrait pas utiliser un custom endpoint en mode
//!   Warren, c'est un cul-de-sac fonctionnel).

use mullvad_types::constraints::Constraint;
use mullvad_types::relay_constraints::{
    GeographicLocationConstraint, LocationConstraint as MullvadLocation, RelaySettings,
};
use warren_relay_selector::{LocationConstraint as WarrenLocation, WarrenRelayQuery};

/// Convertit `RelaySettings` (Mullvad) en `WarrenRelayQuery` consommée
/// par `DaemonWarrenRelaySelector::select_for_attempt`.
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
                // Warren ne distingue pas les hosts d'une ville : on
                // restreint à la `(country, city)` du hostname.
                GeographicLocationConstraint::Hostname(cc, city, _) => WarrenLocation::City {
                    country_code: cc.clone(),
                    city: city.clone(),
                },
            },
            // Les custom lists Mullvad pointent vers une `CustomListsSettings`
            // séparée — pas de résolution pure ici. Fallback `Any` =
            // comportement défensif. À étendre quand on wire le résolveur.
            Constraint::Only(MullvadLocation::CustomList { .. }) => WarrenLocation::Any,
        },
        // Le custom tunnel endpoint cible un host spécifique (IP/port +
        // pubkey) — sans rapport avec la sélection Warren basée sur des
        // exits enrollés via warren-api. On laisse `Any` pour ne pas
        // bloquer le user accidentellement.
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
