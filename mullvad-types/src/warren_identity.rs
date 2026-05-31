//! A Warren identity is the user's wallet account, identified solely by
//! its Ed25519 public key (SS58 `wb…` address). Warren has no
//! per-device binding and no per-device WireGuard keys: every client of
//! one account shares the same wallet identity; the QUIC tunnel
//! authenticates with the wallet key alone.

use crate::warren_pubkey::WarrenPubKey;
use serde::{Deserialize, Serialize};

/// Warren identity — the Ed25519 wallet pubkey of the logged-in user.
///
/// The `pubkey` is the Warren user identifier (Ed25519, SS58 `wb…`),
/// derived from the BIP39 mnemonic via `warren_identity::derive_node_key`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarrenIdentity {
    /// Warren user identifier — Ed25519 pubkey derived from
    /// the BIP39 mnemonic via `warren_identity::derive_node_key`.
    pub pubkey: WarrenPubKey,
}

impl WarrenIdentity {
    /// Create a new identity from the wallet pubkey.
    #[must_use]
    pub fn new(pubkey: WarrenPubKey) -> Self {
        Self { pubkey }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn fixture_pubkey() -> WarrenPubKey {
        // Warren SS58 address of the all-zero 32-byte pubkey (prefix 13295).
        WarrenPubKey::from_str("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
            .expect("fixture SS58 must be valid")
    }

    #[test]
    fn new_constructs_with_given_fields() {
        let pk = fixture_pubkey();
        let identity = WarrenIdentity::new(pk.clone());
        assert_eq!(identity.pubkey, pk);
    }

    #[test]
    fn serde_roundtrips_through_json() {
        // Serialization produces a JSON exploitable by the daemon.
        // Deserialization validates the `pubkey` (= rejected if SS58
        // corrupt, see `warren_pubkey` tests).
        let identity = WarrenIdentity::new(fixture_pubkey());
        let json = serde_json::to_string(&identity).expect("serialize");
        let parsed: WarrenIdentity = serde_json::from_str(&json).expect("deserialize must succeed");
        let rejson = serde_json::to_string(&parsed).expect("re-serialize");
        assert_eq!(json, rejson, "JSON roundtrip must be stable");
    }

    #[test]
    fn serde_rejects_invalid_pubkey() {
        // Security: a corrupt cache must not produce an identity with
        // an invalid pubkey (which would crash later when we try to sign).
        let bad_json = r#"{ "pubkey": "not-a-valid-ss58-address" }"#;
        let res: Result<WarrenIdentity, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "invalid pubkey must fail deserialization");
    }
}
