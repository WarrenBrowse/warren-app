//! [`WarrenIdentity`] type that replaces
//! [`crate::device::AccountAndDevice`] in the Warren auth pipeline.
//!
//! **Difference from `AccountAndDevice`**:
//! - `account_number: AccountNumber` (alias `String`, unvalidated) ->
//!   `pubkey: WarrenPubKey` (validated 64-char hex newtype, see
//!   [`crate::warren_pubkey`]).
//! - Device field unchanged: we keep the same
//!   `Device { id, name, pubkey, hijack_dns, created }` structure.
//!   The `pubkey` of the Device is a
//!   `talpid_types::net::wireguard::PublicKey`; this is NOT the
//!   same as the Ed25519 `WarrenPubKey` of the Warren identity (the
//!   former is WireGuard, the latter is the Warren user identifier).

use crate::device::Device;
use crate::warren_pubkey::WarrenPubKey;
use serde::{Deserialize, Serialize};

/// Warren identity bound to a device — `(pubkey, device)` pair.
///
/// The `pubkey` is the Warren user identifier (Ed25519, 64-char hex);
/// the `device` is the WireGuard device registered on the
/// server side (the `device.pubkey` is the WireGuard pubkey, distinct).
// Upstream Mullvad `Device` does not implement `PartialEq` / `Eq` (it
// has a `chrono::DateTime` field plus `talpid_types::PublicKey` which
// does not derive them either). We stay consistent: no direct
// `PartialEq`; tests compare via JSON roundtrip.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarrenIdentity {
    /// Warren user identifier — Ed25519 pubkey derived from
    /// the BIP39 mnemonic via `warren_identity::derive_node_key`.
    pub pubkey: WarrenPubKey,
    /// WireGuard device registered for this identity (id, name,
    /// WG key, etc.).
    pub device: Device,
}

impl WarrenIdentity {
    /// Create a new identity (trivial constructor for
    /// consistency with `AccountAndDevice::new`).
    #[must_use]
    pub fn new(pubkey: WarrenPubKey, device: Device) -> Self {
        Self { pubkey, device }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::str::FromStr;
    use talpid_types::net::wireguard::PublicKey;

    /// Fixture device for tests. Arbitrary WG pubkey, only used
    /// to fill the struct.
    fn fixture_device() -> Device {
        Device {
            id: "device-id-fixture".to_owned(),
            name: "happy seagull".to_owned(),
            // PublicKey accepts a [u8; 32]
            pubkey: PublicKey::from([0u8; 32]),
            hijack_dns: false,
            created: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    fn fixture_pubkey() -> WarrenPubKey {
        // Warren SS58 address of the all-zero 32-byte pubkey (prefix 13295).
        WarrenPubKey::from_str("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
            .expect("fixture SS58 must be valid")
    }

    #[test]
    fn new_constructs_with_given_fields() {
        let pk = fixture_pubkey();
        let dev = fixture_device();
        let identity = WarrenIdentity::new(pk.clone(), dev.clone());
        assert_eq!(identity.pubkey, pk);
        assert_eq!(identity.device.id, dev.id);
    }

    #[test]
    fn serde_roundtrips_through_json() {
        // Phase 2.B.2 — serialization produces a JSON exploitable
        // by the daemon (which will persist it in device.json
        // instead of the legacy `AccountAndDevice`). Deserialization
        // validates the `pubkey` (= rejected if hex corrupt, see
        // `warren_pubkey::serde_deserialize_rejects_invalid_hex`).
        // `Device` has no PartialEq, so we compare via the re-emitted
        // serialized JSON (= wire format stability contract).
        let identity = WarrenIdentity::new(fixture_pubkey(), fixture_device());
        let json = serde_json::to_string(&identity).expect("serialize");
        let parsed: WarrenIdentity = serde_json::from_str(&json).expect("deserialize must succeed");
        let rejson = serde_json::to_string(&parsed).expect("re-serialize");
        assert_eq!(json, rejson, "JSON roundtrip must be stable");
    }

    #[test]
    fn serde_rejects_invalid_pubkey_hex() {
        // Security: a corrupt device.json must not produce
        // an identity with an invalid pubkey (which would crash
        // later when we try to sign).
        let bad_json = r#"{
            "pubkey": "not-hex",
            "device": {
                "id": "x",
                "name": "y",
                "pubkey": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "hijack_dns": false,
                "created": "2026-01-01T00:00:00Z"
            }
        }"#;
        let res: Result<WarrenIdentity, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "non-hex pubkey must fail deserialization");
    }
}
