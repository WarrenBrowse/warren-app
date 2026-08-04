//! Mullvad-format view of a `WarrenRelayList`.
//!
//! Converts the flat warren-relays list (`Vec<WarrenRelay>`) into a
//! hierarchical [`RelayList`] countries -> cities -> relays. Allows the
//! Electron GUI - which historically consumes the Mullvad list - to display
//! the available Warren exits in its country/city selector without
//! a major frontend refactor.
//!
//! POC limitations:
//! - Produced `WireguardRelay` values carry a `wireguard::PublicKey`
//!   built from the 32 bytes of the Warren Ed25519 pubkey. It is a
//!   fake x25519 key, never used to set up an actual WG tunnel in
//!   Warren mode (the tunnel goes through `warren-tunnel` via
//!   `talpid-warren-tunnel`); it only acts as a unique identifier for
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
use warren_discovery_core::{WarrenRelay, WarrenRelayList};

/// Builds a [`RelayList`] (upstream Mullvad format) from a
/// [`WarrenRelayList`].
///
/// Deterministic grouping by `country_code` then `city` (alphabetical
/// order via `BTreeMap`) to produce stable output
/// (useful for GUI regression tests).
#[must_use]
pub fn to_mullvad_relay_list(warren: &WarrenRelayList) -> RelayList {
    // country_code -> (display_name, city -> (display_name, Vec<&WarrenRelay>))
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
        let (lat, lng) = country_centroid(&country_code);
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
                    lat,
                    lng,
                ));
            }
            // City-level coordinates: at this POC stage every Warren
            // country hosts at most one relay, so the country centroid
            // doubles as the city centroid. When a country grows
            // multiple cities we can move to a city-level table (or
            // ship the coordinates from the signed `warren-relays.json`
            // directly - preferable long-term, drops the static table).
            cities.push(RelayListCity {
                name: city_name,
                code: city_code,
                latitude: lat,
                longitude: lng,
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
    latitude: f64,
    longitude: f64,
) -> WireguardRelay {
    // Warren pubkey = 32 bytes Ed25519, re-interpreted as a
    // `wireguard::PublicKey` to satisfy the GUI type. This key is
    // never consumed by the tunnel: `produce_warren_tunnel_params`
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
            latitude,
            longitude,
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

/// Rough slug of a city name to produce a `CityCode`
/// (`ascii_lower`, `-` instead of space, removes non
/// `[a-z0-9-]` characters). No uniqueness guarantee - `(country_code,
/// city_code)` must remain unique on the warren-api producer side.
fn slugify(s: &str) -> String {
    let lowered = s.to_lowercase().replace([' ', '_'], "-");
    lowered
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

/// Display name of a country from its ISO-3166 alpha-2 code.
/// Minimal POC table - falls back to the code in uppercase if
/// the entry is unknown (the GUI will display "FR", "SE", "US" rather
/// than "France", "Sweden", "United States"). To enrich or replace
/// with an `isocountry`/`celes` crate if needed.
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

/// Geographic centroid (latitude, longitude) of a country from its
/// ISO-3166 alpha-2 code. Values are approximate population/area
/// centers (CIA World Factbook style - accurate to a few degrees,
/// which is the right resolution for a globe-scale VPN map marker).
///
/// Fallback `(0.0, 0.0)` lands at Null Island (Gulf of Guinea); the
/// caller is free to keep the previous coordinate when this is hit,
/// but the renderer side currently overwrites unconditionally, so the
/// fallback IS visible as a marker dot off the West African coast.
/// That visible failure mode is intentional: it cues the operator
/// that a relay's country code is missing from the table here. To
/// fix, either add the entry below or bump the signed
/// `warren-relays.json` schema to carry lat/lng on every relay
/// (preferred long-term; this table is a Warren-POC stopgap).
///
/// MUST keep ISO-3166 alpha-2 codes in sync with
/// [`country_display_name`] - any country listed there should also
/// appear here so the relay list view never silently degrades to
/// Null Island for a supported country.
fn country_centroid(code: &str) -> (f64, f64) {
    // Approximate population centroids (degrees), good enough at
    // the resolution of a 320 × 493 globe canvas. Sources:
    // CIA World Factbook, OECD country profiles.
    match code.to_ascii_lowercase().as_str() {
        "fr" => (46.2, 2.2),
        "de" => (51.2, 10.5),
        "se" => (60.1, 18.6),
        "us" => (39.8, -98.6),
        "uk" | "gb" => (54.8, -2.7),
        "ca" => (56.1, -106.3),
        "nl" => (52.1, 5.3),
        "ch" => (46.8, 8.2),
        "no" => (64.6, 17.6),
        "fi" => (64.0, 26.0),
        "es" => (40.2, -3.7),
        "it" => (41.9, 12.6),
        "pl" => (51.9, 19.1),
        "ro" => (45.9, 24.9),
        "jp" => (36.2, 138.3),
        "sg" => (1.4, 103.8),
        "au" => (-25.3, 133.8),
        _ => (0.0, 0.0),
    }
}

/// Exposes [`country_centroid`] to other modules in the daemon
/// (specifically `tunnel.rs`, which uses the same lookup to populate
/// `GeoIpLocation` on the connecting path). Pub(crate) keeps the API
/// surface internal to the daemon - the table is not a stable
/// downstream contract.
#[must_use]
pub(crate) fn country_centroid_for(code: &str) -> (f64, f64) {
    country_centroid(code)
}

/// Exposes [`country_display_name`] to other modules so the
/// `GeoIpLocation` constructed at relay-selection time in
/// `tunnel.rs::produce_warren_tunnel_params` carries the same
/// human-readable country string the relay-list view ships to the
/// renderer (otherwise the map marker and the connection panel show
/// inconsistent labels - e.g. "Germany" in the panel vs "DE" in the
/// hover popup).
#[must_use]
pub(crate) fn country_display_name_pub(code: &str) -> String {
    country_display_name(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use warren_discovery_core::warren_types::WarrenPubkey;
    use warren_discovery_core::{Addr, Ingress, Listener, Location as WLocation, WarrenRelay};

    fn make_warren_relay(country: &str, city: &str, ipv4: &str, byte_seed: u8) -> WarrenRelay {
        let sk = SigningKey::from_bytes(&[byte_seed; 32]);
        let endpoint_id = WarrenPubkey::from_bytes(sk.verifying_key().to_bytes());
        WarrenRelay::from_public(
            endpoint_id,
            warren_discovery_core::warren_types::ExitId::from_bytes([byte_seed; 16]),
            WLocation::new(country, city),
            100,
            true,
            vec![Ingress::new(
                Addr::new(ipv4.parse().unwrap(), None),
                vec![Listener::new(51820, "quic", "h3")],
            )],
            true,
            false,
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
