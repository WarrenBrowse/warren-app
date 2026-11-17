//! The relay catalogue the Kotlin side reads (`WarrenJni.listRelays`): one row
//! per verified exit, in the schema `RelayInfo` decodes. Host-tested here
//! because the JNI export that serves it only compiles for Android.

use warren_discovery_core::WarrenRelay;

/// Projects each verified exit to its Kotlin row: `exit_id`,
/// `exit_pubkey_hex`, `endpoint` (the first entry address, the Kotlin schema
/// being single-endpoint), `country`, `city`, `active`, `weight`, and `name`,
/// the fleet name the exit is shown by (`null` for an unnamed node).
pub(crate) fn project_relays(relays: &[WarrenRelay]) -> Vec<serde_json::Value> {
    relays
        .iter()
        .map(|r| {
            let endpoint = r
                .endpoint_addr()
                .ip_addrs()
                .next()
                .map(|sa| sa.to_string())
                .unwrap_or_else(|| "0.0.0.0:443".to_owned());
            serde_json::json!({
                "exit_id": r.exit_id().to_hex(),
                "exit_pubkey_hex": r.endpoint_id().to_hex(),
                "endpoint": endpoint,
                "country": r.location().country_code(),
                "city": r.location().city(),
                "active": r.is_active(),
                "weight": r.weight(),
                "name": r.fleet_name(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use warren_discovery_core::warren_types::{ExitId, SigningKey, WarrenPubkey};
    use warren_discovery_core::{
        JsonEgress, JsonEndpoint, JsonListener, JsonLocation, JsonNode, sign_relay_list_v2,
        verify_signed_relay_list,
    };

    fn node(seed: u8, name: Option<&str>) -> JsonNode {
        JsonNode {
            id: hex::encode(WarrenPubkey::from_bytes([seed; 32]).as_bytes()),
            exit_id: ExitId::from_bytes([seed; 16]),
            location: JsonLocation {
                country: "FI".to_owned(),
                city: "Helsinki".to_owned(),
            },
            weight: 100,
            active: true,
            egress: JsonEgress {
                ipv4: true,
                ipv6: false,
            },
            endpoints: vec![JsonEndpoint {
                addr: format!("198.51.100.{seed}"),
                family: "ipv4".to_owned(),
                listeners: vec![JsonListener {
                    port: 443,
                    transport: "quic".to_owned(),
                    alpn: "h3".to_owned(),
                }],
            }],
            cover_domain: None,
            port_forward: None,
            tcp_fallback: None,
            last_seen_unix: None,
            stale: None,
            name: name.map(str::to_owned),
            provider: None,
            virt: None,
            asn: None,
            attestation_hex: None,
            relay_descriptor: None,
            exit_descriptor: None,
            edge_cert_sha256: None,
        }
    }

    /// The app shows an exit by the fleet name the v11 list carries, and by
    /// nothing it invents when a node has none.
    #[test]
    fn each_row_carries_the_fleet_name_or_null() {
        let key = SigningKey::from_bytes(&[0xab; 32]);
        let signed = sign_relay_list_v2(
            vec![node(5, Some("fi-hel-bveh1")), node(6, None)],
            &key,
            1,
            1_700_000_000,
            4_000_000_000,
        );
        let raw = serde_json::to_string(&signed).expect("serialize");
        let verified = verify_signed_relay_list(&raw, None).expect("verifies");

        let rows = project_relays(verified.relays.relays());

        assert_eq!(rows[0]["name"], "fi-hel-bveh1");
        assert!(rows[1]["name"].is_null());
        assert_eq!(rows[0]["city"], "Helsinki");
        assert_eq!(rows[0]["endpoint"], "198.51.100.5:443");
    }
}
