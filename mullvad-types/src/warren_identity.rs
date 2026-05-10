//! Type [`WarrenIdentity`] qui remplace
//! [`crate::device::AccountAndDevice`] dans le pipeline auth Warren.
//!
//! **Différence avec `AccountAndDevice`** :
//! - `account_number: AccountNumber` (alias `String` non validé) →
//!   `pubkey: WarrenPubKey` (newtype hex 64ch validé, cf.
//!   [`crate::warren_pubkey`]).
//! - Champ Device inchangé : on conserve la même structure
//!   `Device { id, name, pubkey, hijack_dns, created }`. La `pubkey`
//!   du Device est une `talpid_types::net::wireguard::PublicKey` ;
//!   ce n'est PAS la même que la `WarrenPubKey` Ed25519 de
//!   l'identité Warren (la première est WireGuard, la seconde est
//!   l'identifiant utilisateur Warren).

use crate::device::Device;
use crate::warren_pubkey::WarrenPubKey;
use serde::{Deserialize, Serialize};

/// Identité Warren liée à un device — paire `(pubkey, device)`.
///
/// La `pubkey` est l'identifiant utilisateur Warren (Ed25519 hex
/// 64ch) ; le `device` est le device WireGuard enregistré côté
/// serveur (le `device.pubkey` est la pubkey WireGuard, distincte).
// `Device` upstream Mullvad n'implémente pas `PartialEq` / `Eq` (champ
// `chrono::DateTime` + `talpid_types::PublicKey` qui ne le derivent
// pas non plus). On reste cohérent : pas de `PartialEq` direct, les
// tests comparent via roundtrip JSON.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarrenIdentity {
    /// Identifiant utilisateur Warren — pubkey Ed25519 dérivée de
    /// la mnémonique BIP39 via `warren_identity::derive_node_key`.
    pub pubkey: WarrenPubKey,
    /// Device WireGuard enregistré pour cette identité (id, nom,
    /// clé WG, etc.).
    pub device: Device,
}

impl WarrenIdentity {
    /// Crée une nouvelle identité (constructeur trivial pour
    /// cohérence avec `AccountAndDevice::new`).
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

    /// Fixture device pour tests. Pubkey WG arbitraire, ne sert
    /// qu'à remplir la struct.
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
        WarrenPubKey::from_str("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            .expect("fixture hex must be valid")
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
        // Phase 2.B.2 — la sérialisation produit un JSON exploitable
        // par le daemon (qui le persistera dans device.json à la
        // place de l'ancien `AccountAndDevice`). La déserialisation
        // valide la `pubkey` (= rejet si hex corrompu, cf.
        // `warren_pubkey::serde_deserialize_rejects_invalid_hex`).
        // `Device` n'a pas PartialEq, donc on compare via le JSON
        // sérialisé ré-émis (= contrat de stabilité du wire format).
        let identity = WarrenIdentity::new(fixture_pubkey(), fixture_device());
        let json = serde_json::to_string(&identity).expect("serialize");
        let parsed: WarrenIdentity = serde_json::from_str(&json).expect("deserialize must succeed");
        let rejson = serde_json::to_string(&parsed).expect("re-serialize");
        assert_eq!(json, rejson, "JSON roundtrip doit être stable");
    }

    #[test]
    fn serde_rejects_invalid_pubkey_hex() {
        // Sécurité : un device.json corrompu ne doit pas produire
        // une identité avec pubkey invalide (qui crasherait plus
        // tard quand on essaiera de signer).
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
        assert!(res.is_err(), "pubkey non-hex doit faire échouer la deser");
    }
}
