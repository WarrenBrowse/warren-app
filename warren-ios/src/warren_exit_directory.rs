//! Signed Warren exit directory for the iOS relay-list path.
//!
//! `GET /v1/exits` returns a server-signed v10 `SignedRelayList`. The shared
//! `warren-discovery-core` verifier is the single place that parses and
//! signature-checks it (same crate as the desktop daemon updater and
//! warren-jni), so no parsing or verification logic is duplicated here. This
//! module adds only the iOS-specific step: projecting the verified list into
//! the Mullvad `ServerRelaysResponse` wire JSON the Swift REST stack already
//! decodes (snake_case keys, base64 `public_key`, `"cc-city"` location
//! identifiers), which keeps the Swift relay cache, selector and UI untouched.
//!
//! Freshness: an expired list is rejected (freeze/replay defense, mirroring
//! the desktop updater) and the caller keeps its cached list. The
//! anti-rollback generation gate is NOT enforced here: it needs a persisted
//! high-water mark and iOS has no daemon to hold one on this path; the
//! security-critical connect path gets its anti-rollback from the multi-hop
//! directory store (`warren_multihop_generation`). The relay list only feeds
//! the location picker UI.

use std::net::SocketAddr;

use base64::Engine as _;
use warren_discovery_core::{SignedError, WarrenRelay, verify_signed_relay_list};

/// Failure to accept a fetched exit directory. The caller keeps its
/// previously cached relay list on any of these.
#[derive(Debug, thiserror::Error)]
pub enum ExitDirectoryError {
    /// Signature, pin or format verification failed. `SignedError`'s
    /// Display is redacted, so surfacing it leaks no key material.
    #[error("exit directory verification failed: {0}")]
    Verify(#[from] SignedError),
    /// The signed `expires_at` is at or before `now` (freeze/replay
    /// defense, mirroring the desktop updater's freshness gate).
    #[error("exit directory is expired")]
    Expired,
}

/// Verifies the raw signed `/v1/exits` body against the pinned server
/// pubkey and projects it into the Mullvad `ServerRelaysResponse` wire
/// JSON consumed by the Swift REST decoder.
///
/// # Errors
/// [`ExitDirectoryError::Verify`] when the body does not verify against
/// `expected_server_pubkey`; [`ExitDirectoryError::Expired`] when the
/// signed expiry is at or before `now_unix`.
pub fn verify_and_project(
    raw: &str,
    expected_server_pubkey: Option<&str>,
    now_unix: u64,
) -> Result<String, ExitDirectoryError> {
    let verified = verify_signed_relay_list(raw, expected_server_pubkey)?;
    if verified.is_expired(now_unix) {
        return Err(ExitDirectoryError::Expired);
    }
    Ok(project(verified.relays.relays()))
}

fn project(relays: &[WarrenRelay]) -> String {
    let mut locations = serde_json::Map::new();
    let mut wg_relays: Vec<serde_json::Value> = Vec::with_capacity(relays.len());

    for relay in relays {
        let country_code = relay.location().country_code().to_ascii_lowercase();
        let location_id = format!("{}-{}", country_code, city_code(relay.location().city()));
        let (latitude, longitude) = country_centroid(&country_code);
        locations.entry(location_id.clone()).or_insert_with(|| {
            serde_json::json!({
                "country": country_display_name(&country_code),
                "city": relay.location().city(),
                "latitude": latitude,
                "longitude": longitude,
            })
        });

        // The Warren Ed25519 pubkey fills the WireGuard `public_key` slot
        // purely as a unique identifier for the Swift selector; the tunnel
        // never consumes it (config comes from the verified multi-hop
        // directory). Same convention as the desktop relay-list view.
        let pubkey_bytes: [u8; 32] = *relay.endpoint_id().as_bytes();
        let ipv4 = relay
            .endpoint_addr()
            .ip_addrs()
            .find_map(|addr| match addr {
                SocketAddr::V4(v4) => Some(v4.ip().to_string()),
                SocketAddr::V6(_) => None,
            })
            .unwrap_or_else(|| "0.0.0.0".to_owned());
        // `ipv6_addr_in` is non-optional in the Swift Codable; the
        // unspecified address mirrors the prebundled asset when the exit
        // has no v6 entry endpoint.
        let ipv6 = relay
            .endpoint_addr()
            .ip_addrs()
            .find_map(|addr| match addr {
                SocketAddr::V4(_) => None,
                SocketAddr::V6(v6) => Some(v6.ip().to_string()),
            })
            .unwrap_or_else(|| "::".to_owned());

        wg_relays.push(serde_json::json!({
            "hostname": format!("warren-{}", &hex::encode(pubkey_bytes)[..16]),
            "active": relay.is_active(),
            "owned": true,
            "provider": "warren",
            "weight": relay.weight(),
            "ipv4_addr_in": ipv4,
            "ipv6_addr_in": ipv6,
            "public_key": base64::engine::general_purpose::STANDARD.encode(pubkey_bytes),
            "location": location_id,
            "include_in_country": true,
            // Every Warren exit runs DAITA (fleet-wide since 2026-07-13)
            // and the Swift relay selector FILTERS relays on this flag
            // when the DAITA setting is on: advertising false makes every
            // DAITA-enabled install fail with noRelaysSatisfyingConstraints.
            // Derive from the signed directory if a per-exit capability
            // field ever lands there.
            "daita": true,
            "shadowsocks_extra_addr_in": [],
            "features": null,
        }));
    }

    // Gateways are decoder placeholders mirroring the prebundled
    // `relays.json`: the Warren datapath takes its addressing from the
    // tunnel config, never from these fields. The port ranges however MUST
    // be non-empty: the Swift relay selector draws a random port from them
    // and rejects every relay when they are empty (fresh installs would
    // never connect), so advertise the real unified Warren port.
    serde_json::json!({
        "locations": locations,
        "wireguard": {
            "ipv4_gateway": "10.64.0.1",
            "ipv6_gateway": "fd00::1",
            "port_ranges": [[443, 443]],
            "shadowsocks_port_ranges": [],
            "relays": wg_relays,
        },
        "bridge": {
            "shadowsocks": [],
            "relays": [],
        },
    })
    .to_string()
}

/// City part of the Swift `LocationIdentifier`. Must never contain a
/// hyphen: the Swift side splits `"cc-city"` on `-` and rejects anything
/// but exactly two components, which would fail the whole relay-list
/// decode.
fn city_code(city: &str) -> String {
    city.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Display name of a country from its ISO-3166 alpha-2 code. Kept in
/// lockstep with `mullvad-daemon::warren_relay_list_view` (warren-ios
/// cannot link the daemon crate). Falls back to the uppercased code.
fn country_display_name(code: &str) -> String {
    match code {
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

/// Approximate country centroid (latitude, longitude), same table as
/// `mullvad-daemon::warren_relay_list_view` (kept in lockstep). The
/// `(0.0, 0.0)` fallback is deliberately visible on the map so a missing
/// entry gets noticed rather than silently misplaced.
fn country_centroid(code: &str) -> (f64, f64) {
    match code {
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

#[cfg(test)]
mod tests {
    use super::*;
    use warren_discovery_core::warren_types::{ExitId, SigningKey, WarrenPubkey};
    use warren_discovery_core::{
        JsonEgress, JsonEndpoint, JsonListener, JsonLocation, JsonNode, sign_relay_list,
    };

    const FAR_FUTURE: u64 = 4_000_000_000;
    const NOW: u64 = 1_700_000_500;

    fn node(seed: u8, country: &str, city: &str, addr: &str, family: &str) -> JsonNode {
        JsonNode {
            id: hex::encode(WarrenPubkey::from_bytes([seed; 32]).as_bytes()),
            exit_id: ExitId::from_bytes([seed; 16]),
            location: JsonLocation {
                country: country.to_owned(),
                city: city.to_owned(),
            },
            weight: 100,
            active: true,
            egress: JsonEgress {
                ipv4: family == "ipv4",
                ipv6: family == "ipv6",
            },
            endpoints: vec![JsonEndpoint {
                addr: addr.to_owned(),
                family: family.to_owned(),
                listeners: vec![JsonListener {
                    port: 51820,
                    transport: "quic".to_owned(),
                    alpn: "h3".to_owned(),
                }],
            }],
            cover_domain: None,
            port_forward: None,
            tcp_fallback: None,
            last_seen_unix: None,
            stale: None,
            name: None,
            provider: None,
            virt: None,
            asn: None,
            attestation_hex: None,
            relay_descriptor: None,
            exit_descriptor: None,
            edge_cert_sha256: None,
        }
    }

    fn signed_body(key: &SigningKey, nodes: Vec<JsonNode>, expires_at: u64) -> String {
        let signed = sign_relay_list(nodes, key, 1, 1_700_000_000, expires_at);
        serde_json::to_string(&signed).expect("serialize signed v10")
    }

    fn pubkey_hex(key: &SigningKey) -> String {
        hex::encode(key.verifying_key().to_bytes())
    }

    #[test]
    fn projects_verified_list_to_mullvad_wire_shape() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![node(5, "se", "Stockholm", "198.51.100.1", "ipv4")],
            FAR_FUTURE,
        );

        let projected =
            verify_and_project(&body, Some(&pubkey_hex(&key)), NOW).expect("must verify");
        let json: serde_json::Value = serde_json::from_str(&projected).expect("valid JSON");

        let loc = &json["locations"]["se-stockholm"];
        assert_eq!(loc["country"], "Sweden");
        assert_eq!(loc["city"], "Stockholm");
        assert_eq!(loc["latitude"], 60.1);
        assert_eq!(loc["longitude"], 18.6);

        let wg = &json["wireguard"];
        assert_eq!(wg["ipv4_gateway"], "10.64.0.1");
        assert_eq!(wg["ipv6_gateway"], "fd00::1");
        // Non-empty on purpose: the Swift relay selector draws a random port
        // from these ranges and fails the whole selection when they are
        // empty, so the projection must advertise the real Warren port.
        assert_eq!(wg["port_ranges"], serde_json::json!([[443, 443]]));
        assert!(
            wg["shadowsocks_port_ranges"]
                .as_array()
                .expect("array")
                .is_empty()
        );

        let relays = wg["relays"].as_array().expect("relays array");
        assert_eq!(relays.len(), 1);
        let relay = &relays[0];
        let expected_hex = hex::encode([5u8; 32]);
        assert_eq!(relay["hostname"], format!("warren-{}", &expected_hex[..16]));
        assert_eq!(relay["active"], true);
        assert_eq!(relay["owned"], true);
        assert_eq!(relay["provider"], "warren");
        assert_eq!(relay["weight"], 100);
        assert_eq!(relay["ipv4_addr_in"], "198.51.100.1");
        assert_eq!(relay["ipv6_addr_in"], "::");
        assert_eq!(
            relay["public_key"],
            base64::engine::general_purpose::STANDARD.encode([5u8; 32])
        );
        assert_eq!(relay["location"], "se-stockholm");
        assert_eq!(relay["include_in_country"], true);
        // The Swift relay selector filters on this flag when the DAITA
        // setting is enabled; advertising false would leave a DAITA user
        // with zero selectable relays (blocked connection).
        assert_eq!(relay["daita"], true);
        assert!(
            relay["shadowsocks_extra_addr_in"]
                .as_array()
                .expect("array")
                .is_empty()
        );
        assert!(relay["features"].is_null());

        assert!(
            json["bridge"]["relays"]
                .as_array()
                .expect("array")
                .is_empty()
        );
        assert!(
            json["bridge"]["shadowsocks"]
                .as_array()
                .expect("array")
                .is_empty()
        );
    }

    #[test]
    fn rejects_tampered_body() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![node(5, "se", "Stockholm", "198.51.100.1", "ipv4")],
            FAR_FUTURE,
        )
        .replace("51820", "59999");

        let err = verify_and_project(&body, Some(&pubkey_hex(&key)), NOW)
            .expect_err("tampered body must be rejected");
        assert!(matches!(err, ExitDirectoryError::Verify(_)));
    }

    #[test]
    fn rejects_wrong_pinned_server_key() {
        let attacker = SigningKey::from_bytes(&[0x11; 32]);
        let legit = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &attacker,
            vec![node(5, "se", "Stockholm", "198.51.100.1", "ipv4")],
            FAR_FUTURE,
        );

        let err = verify_and_project(&body, Some(&pubkey_hex(&legit)), NOW)
            .expect_err("attacker-signed list must be rejected under the legit pin");
        assert!(matches!(err, ExitDirectoryError::Verify(_)));
    }

    #[test]
    fn rejects_expired_list() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![node(5, "se", "Stockholm", "198.51.100.1", "ipv4")],
            NOW - 100,
        );

        let err = verify_and_project(&body, Some(&pubkey_hex(&key)), NOW)
            .expect_err("expired list must be rejected");
        assert!(matches!(err, ExitDirectoryError::Expired));
    }

    #[test]
    fn city_code_never_contains_a_hyphen() {
        // Swift's LocationIdentifier splits on "-" and requires exactly two
        // components; a hyphenated city code would fail the whole decode.
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![node(7, "us", "New York", "198.51.100.2", "ipv4")],
            FAR_FUTURE,
        );

        let projected =
            verify_and_project(&body, Some(&pubkey_hex(&key)), NOW).expect("must verify");
        let json: serde_json::Value = serde_json::from_str(&projected).expect("valid JSON");

        assert!(json["locations"]["us-newyork"].is_object());
        assert_eq!(json["wireguard"]["relays"][0]["location"], "us-newyork");
    }

    #[test]
    fn ipv6_only_relay_keeps_required_ipv4_field() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![node(9, "de", "Frankfurt", "2001:db8::9", "ipv6")],
            FAR_FUTURE,
        );

        let projected =
            verify_and_project(&body, Some(&pubkey_hex(&key)), NOW).expect("must verify");
        let json: serde_json::Value = serde_json::from_str(&projected).expect("valid JSON");

        let relay = &json["wireguard"]["relays"][0];
        assert_eq!(relay["ipv4_addr_in"], "0.0.0.0");
        assert_eq!(relay["ipv6_addr_in"], "2001:db8::9");
    }

    #[test]
    fn baked_server_pin_is_a_valid_lowercase_ed25519_key() {
        // The pin gate is an exact string compare against the server's
        // lowercase hex; a mis-cased or off-curve baked pin would silently
        // reject every fetched directory.
        let pin = crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX;
        assert_eq!(pin, pin.to_ascii_lowercase());
        let bytes: [u8; 32] = hex::decode(pin)
            .expect("64 hex chars")
            .try_into()
            .expect("32 bytes");
        ed25519_dalek::VerifyingKey::from_bytes(&bytes).expect("on-curve Ed25519 key");
    }

    #[test]
    fn groups_relays_of_the_same_city_under_one_location() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let body = signed_body(
            &key,
            vec![
                node(1, "se", "Stockholm", "198.51.100.1", "ipv4"),
                node(2, "se", "Stockholm", "198.51.100.2", "ipv4"),
            ],
            FAR_FUTURE,
        );

        let projected =
            verify_and_project(&body, Some(&pubkey_hex(&key)), NOW).expect("must verify");
        let json: serde_json::Value = serde_json::from_str(&projected).expect("valid JSON");

        assert_eq!(
            json["locations"].as_object().expect("locations map").len(),
            1
        );
        assert_eq!(
            json["wireguard"]["relays"]
                .as_array()
                .expect("relays array")
                .len(),
            2
        );
    }
}
