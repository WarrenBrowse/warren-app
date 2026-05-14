//! Vue Mullvad-format d'une `WarrenRelayList`.
//!
//! Convertit la liste warren-relays (`Vec<WarrenRelay>` plat) en une
//! [`RelayList`] hiérarchique countries → cities → relays. Permet à la
//! GUI Electron — qui consomme historiquement la liste Mullvad — d'afficher
//! les exits Warren disponibles dans son sélecteur de pays/villes sans
//! refacto majeur côté frontend.
//!
//! POC limitations:
//! - Produced `WireguardRelay` values carry a `wireguard::PublicKey`
//!   built from the 32 bytes of the Warren Ed25519 pubkey. It is a
//!   fake x25519 key, never used to set up an actual WG tunnel in
//!   Warren mode (the tunnel goes through `warren-tunnel` via
//!   `talpid-warren-iroh`); it only acts as a unique identifier for
//!   the GUI.
//! - `hostname` = first 16 hex chars of the pubkey (enough to
//!   visually distinguish exits in the list).
//! - `latitude`/`longitude` = 0/0 (warren-api `/v1/exits` does not
//!   carry the coordinates). The map will stack all exits on (0,0)
//!   until we enrich with a country->coord table or a server-side
//!   geocoder.
//! - `port_ranges`/`shadowsocks_port_ranges`/`udp2tcp_ports` are
//!   empty (not used on the Warren path).

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};

use mullvad_types::location::Location;
use mullvad_types::relay_list::{
    EndpointData, Relay, RelayList, RelayListCity, RelayListCountry, WireguardRelay,
    WireguardRelayEndpointData,
};
use talpid_types::net::wireguard;
use warren_relay_selector::{WarrenRelay, WarrenRelayList};

/// Construit une [`RelayList`] (format Mullvad upstream) à partir d'une
/// [`WarrenRelayList`].
///
/// Groupement déterministe par `country_code` puis `city` (ordre
/// alphabétique via `BTreeMap`) pour produire une sortie stable
/// (utile pour les tests de regression GUI).
#[must_use]
pub fn to_mullvad_relay_list(warren: &WarrenRelayList) -> RelayList {
    // country_code → (display_name, city → (display_name, Vec<&WarrenRelay>))
    let mut by_country: BTreeMap<String, BTreeMap<String, Vec<&WarrenRelay>>> = BTreeMap::new();
    for relay in warren.relays() {
        by_country
            .entry(relay.location().country_code().to_ascii_lowercase())
            .or_default()
            .entry(relay.location().city().to_string())
            .or_default()
            .push(relay);
    }

    let mut countries: Vec<RelayListCountry> = Vec::with_capacity(by_country.len());
    for (country_code, cities_map) in by_country {
        let mut cities: Vec<RelayListCity> = Vec::with_capacity(cities_map.len());
        for (city_name, relays) in cities_map {
            let city_code = slugify(&city_name);
            let mut wg_relays: Vec<WireguardRelay> = Vec::with_capacity(relays.len());
            for r in relays {
                wg_relays.push(make_wireguard_relay(
                    r,
                    &country_code,
                    &city_name,
                    &city_code,
                ));
            }
            cities.push(RelayListCity {
                name: city_name,
                code: city_code,
                latitude: 0.0,
                longitude: 0.0,
                relays: wg_relays,
            });
        }
        countries.push(RelayListCountry {
            name: country_display_name(&country_code),
            code: country_code,
            cities,
        });
    }

    RelayList {
        countries,
        wireguard: EndpointData::default(),
    }
}

fn make_wireguard_relay(
    relay: &WarrenRelay,
    country_code: &str,
    city_name: &str,
    city_code: &str,
) -> WireguardRelay {
    // Warren pubkey = 32 bytes Ed25519, re-interpreted as a
    // `wireguard::PublicKey` to satisfy the GUI type. This key is
    // never consumed by the tunnel: `produce_warren_iroh_params`
    // pulls the pubkey directly from the `WarrenRelaySelector`, not
    // from the `RelayList` view.
    let endpoint_bytes: [u8; 32] = *relay.endpoint_id().as_bytes();
    let public_key = wireguard::PublicKey::from(endpoint_bytes);

    // Short visually distinct hostname (16 hex chars of the pubkey).
    let hostname = format!("warren-{}", &hex::encode(endpoint_bytes)[..16]);

    let ipv4 = relay
        .endpoint_addr()
        .ip_addrs()
        .find_map(|addr| match addr {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            SocketAddr::V6(_) => None,
        })
        .unwrap_or(Ipv4Addr::UNSPECIFIED);
    let ipv6 = relay
        .endpoint_addr()
        .ip_addrs()
        .find_map(|addr| match addr {
            SocketAddr::V4(_) => None,
            SocketAddr::V6(v6) => Some(*v6.ip()),
        });

    let inner = Relay {
        hostname,
        ipv4_addr_in: ipv4,
        ipv6_addr_in: ipv6,
        active: relay.is_active(),
        weight: relay.weight(),
        location: Location {
            country: country_display_name(country_code),
            country_code: country_code.to_string(),
            city: city_name.to_string(),
            city_code: city_code.to_string(),
            latitude: 0.0,
            longitude: 0.0,
        },
    };

    WireguardRelay::new(
        false,                // overridden_ipv4
        false,                // overridden_ipv6
        true,                 // include_in_country
        true,                 // owned (= shown as Warren-owned)
        "warren".to_string(), // provider
        WireguardRelayEndpointData::new(public_key),
        inner,
    )
}

/// Slug grossier d'un nom de ville pour produire un `CityCode`
/// (`ascii_lower`, `-` à la place de l'espace, supprime les caractères
/// non `[a-z0-9-]`). Pas de garantie d'unicité — les `(country_code,
/// city_code)` doivent rester uniques côté producteur warren-api.
fn slugify(s: &str) -> String {
    let lowered = s.to_lowercase().replace([' ', '_'], "-");
    lowered
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

/// Nom d'affichage d'un pays à partir de son code ISO-3166 alpha-2.
/// Table minimale POC — on tombe sur le code en majuscules si
/// l'entrée n'est pas connue (la GUI affichera "FR", "SE", "US" plutôt
/// que "France", "Sweden", "United States"). À enrichir ou remplacer
/// par un crate `isocountry`/`celes` si nécessaire.
fn country_display_name(code: &str) -> String {
    match code.to_ascii_lowercase().as_str() {
        "fr" => "France",
        "de" => "Germany",
        "se" => "Sweden",
        "us" => "USA",
        "uk" | "gb" => "United Kingdom",
        "ca" => "Canada",
        "nl" => "Netherlands",
        "ch" => "Switzerland",
        "no" => "Norway",
        "fi" => "Finland",
        "es" => "Spain",
        "it" => "Italy",
        "pl" => "Poland",
        "ro" => "Romania",
        "jp" => "Japan",
        "sg" => "Singapore",
        "au" => "Australia",
        _ => return code.to_ascii_uppercase(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use warren_relay_selector::warren_types::{WarrenExitAddr, WarrenPubkey};
    use warren_relay_selector::{Location as WLocation, WarrenRelay};

    fn make_warren_relay(country: &str, city: &str, ipv4: &str, byte_seed: u8) -> WarrenRelay {
        let sk = SigningKey::from_bytes(&[byte_seed; 32]);
        let endpoint_id = WarrenPubkey::from_bytes(sk.verifying_key().to_bytes());
        let socket: SocketAddr = format!("{}:51820", ipv4).parse().unwrap();
        let endpoint_addr = WarrenExitAddr::new(endpoint_id).with_ip_addr(socket);
        WarrenRelay::new(
            endpoint_id,
            endpoint_addr,
            WLocation::new(country, city),
            100,
            true,
        )
    }

    #[test]
    fn empty_list_produces_empty_relay_list() {
        let list = WarrenRelayList::default();
        let out = to_mullvad_relay_list(&list);
        assert!(out.countries.is_empty());
    }

    #[test]
    fn groups_relays_by_country_and_city() {
        let list = WarrenRelayList::new(vec![
            make_warren_relay("FR", "Paris", "10.0.0.1", 1),
            make_warren_relay("fr", "Paris", "10.0.0.2", 2),
            make_warren_relay("FR", "Lyon", "10.0.0.3", 3),
            make_warren_relay("SE", "Stockholm", "10.0.0.4", 4),
        ]);
        let out = to_mullvad_relay_list(&list);
        assert_eq!(out.countries.len(), 2);
        let fr = out.countries.iter().find(|c| c.code == "fr").unwrap();
        assert_eq!(fr.cities.len(), 2);
        let paris = fr.cities.iter().find(|c| c.name == "Paris").unwrap();
        assert_eq!(paris.relays.len(), 2);
        let lyon = fr.cities.iter().find(|c| c.name == "Lyon").unwrap();
        assert_eq!(lyon.relays.len(), 1);
    }

    #[test]
    fn hostname_is_unique_per_relay() {
        let list = WarrenRelayList::new(vec![
            make_warren_relay("FR", "Paris", "10.0.0.1", 1),
            make_warren_relay("FR", "Paris", "10.0.0.2", 2),
        ]);
        let out = to_mullvad_relay_list(&list);
        let fr_paris = &out.countries[0].cities[0].relays;
        assert_ne!(fr_paris[0].hostname, fr_paris[1].hostname);
    }
}
