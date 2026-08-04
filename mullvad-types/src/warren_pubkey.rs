//! `WarrenPubKey` newtype that replaces the `pub type AccountNumber
//! = String` alias ([`crate::account`]) with a strictly validated
//! representation of an Ed25519 pubkey.
//!
//! **Format**: a Warren **SS58 address** (Substrate/Polkadot address
//! format, network prefix [`WARREN_SS58_PREFIX`] = `13295`). Every
//! address is 47-49 characters and starts with `wb` (Warren Browse),
//! e.g. `wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB`. Any other
//! shape is rejected at construction (`parse, don't validate`).
//!
//! **Why a newtype**: (1) avoid confusion with other `String`s (see
//! rule `type-newtype-ids` rust-skills); (2) guarantee that a
//! `WarrenPubKey` is *always* decodable into `[u8; 32]` without
//! re-validation downstream; (3) replace `AccountNumber` across the
//! API + daemon chain (see `warren-core/docs/06-auth-wallet.md`).
//!
//! **Codec note**: the canonical SS58 codec lives in `warren_contract::ss58`
//! (the shared client<->server contract crate) and is byte-for-byte compatible
//! with `@polkadot/util-crypto` v14. To keep this foundational crate
//! free of the heavy identity/keyring dependency, the minimal
//! encode/decode is mirrored here in [`ss58`]; both are pinned to the
//! same reference vectors so they cannot drift.

use std::fmt;
use std::str::FromStr;

/// Warren SS58 network prefix (`13295`), re-exported from the shared
/// codec crate. Chosen so every address starts with `wb`.
pub use warren_contract::ss58::WARREN_SS58_PREFIX;

// The SS58 codec is the shared `warren-contract` crate - the single
// source of truth, also re-exported as `warren_identity::ss58` on the
// backend. No checksum logic is duplicated in this crate.
use warren_contract::ss58;

/// Warren pubkey - validated wrapper around a Warren SS58 address
/// (`wb…`) representing an Ed25519 pubkey (32 bytes).
///
/// Construct via [`Self::from_str`] or [`Self::from_bytes`] (the only
/// validated paths). The inner field stays `pub(crate)` to forbid
/// non-validated instantiation from outside the crate.
///
/// **Serde serialization**: `#[serde(transparent)]` - a JSON string.
/// The `Deserialize` impl re-validates via [`Self::from_str`] so that a
/// corrupt payload fails deserialization rather than silently producing
/// an invalid `WarrenPubKey`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct WarrenPubKey(pub(crate) String);

impl<'de> serde::Deserialize<'de> for WarrenPubKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Parsing error for [`WarrenPubKey`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    /// The string is not valid base58.
    #[error("not a valid base58 string")]
    NotBase58,
    /// The decoded length is not that of a 32-byte SS58 account address.
    #[error("not a 32-byte SS58 account address")]
    BadLength,
    /// The address encodes a non-Warren SS58 network (prefix ≠ 13295).
    #[error("not a Warren SS58 address (wrong network prefix)")]
    WrongNetwork,
    /// The SS58 checksum does not match (corrupt or mistyped address).
    #[error("SS58 checksum mismatch")]
    BadChecksum,
}

impl From<ss58::Ss58Error> for ParseError {
    fn from(e: ss58::Ss58Error) -> Self {
        match e {
            ss58::Ss58Error::BadBase58 => ParseError::NotBase58,
            ss58::Ss58Error::BadLength => ParseError::BadLength,
            ss58::Ss58Error::WrongNetwork => ParseError::WrongNetwork,
            ss58::Ss58Error::BadChecksum => ParseError::BadChecksum,
        }
    }
}

impl WarrenPubKey {
    /// Returns the underlying SS58 address string (`wb…`).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the 32 represented bytes. Always `Ok` because
    /// construction guarantees a valid SS58 address; we return a
    /// `Result` to stay compliant with `# Errors`.
    ///
    /// # Errors
    ///
    /// [`ParseError`] if the internal representation has been mutated
    /// out-of-API (= internal bug, never in normal use).
    pub fn to_bytes(&self) -> Result<[u8; 32], ParseError> {
        Ok(ss58::decode(&self.0)?)
    }

    /// Build from 32 raw bytes (e.g. an
    /// `ed25519_dalek::VerifyingKey::as_bytes()`), encoding them as a
    /// Warren SS58 address.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(ss58::encode(bytes))
    }
}

impl FromStr for WarrenPubKey {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Decode validates base58 + length + network prefix + checksum.
        ss58::decode(s)?;
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for WarrenPubKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Reference Warren SS58 address: the all-zero 32-byte pubkey under
    /// prefix 13295, as produced by @polkadot/util-crypto v14.
    const VALID_SS58: &str = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB";

    #[test]
    fn from_str_accepts_valid_warren_address() {
        let pk = WarrenPubKey::from_str(VALID_SS58).expect("valid SS58 must parse");
        assert_eq!(pk.as_str(), VALID_SS58);
    }

    #[test]
    fn from_str_rejects_non_base58() {
        // `0 O I l` are not in the base58 alphabet.
        let err = WarrenPubKey::from_str("0OIl not base58").expect_err("must fail");
        assert_eq!(err, ParseError::NotBase58);
    }

    #[test]
    fn from_str_rejects_wrong_length() {
        let err = WarrenPubKey::from_str("abc").expect_err("too short must fail");
        assert_eq!(err, ParseError::BadLength);
    }

    #[test]
    fn from_str_rejects_foreign_network() {
        // Same 0x11… bytes encoded with SS58 prefix 1000 (not Warren).
        let foreign = "vjdfteK8ZU3Lg6jotudWVQk1eGD7GEnb46Xv5JdmKpQD2WB1r";
        let err = WarrenPubKey::from_str(foreign).expect_err("foreign network must fail");
        assert_eq!(err, ParseError::WrongNetwork);
    }

    #[test]
    fn from_str_rejects_empty_string() {
        let err = WarrenPubKey::from_str("").expect_err("empty must fail");
        assert_eq!(err, ParseError::BadLength);
    }

    #[test]
    fn display_returns_underlying_address() {
        let pk = WarrenPubKey::from_str(VALID_SS58).unwrap();
        assert_eq!(format!("{pk}"), VALID_SS58);
    }

    #[test]
    fn from_bytes_matches_reference_vector() {
        // The all-zero pubkey must encode to the pinned @polkadot vector.
        let pk = WarrenPubKey::from_bytes(&[0u8; 32]);
        assert_eq!(pk.as_str(), VALID_SS58);
    }

    #[test]
    fn to_bytes_roundtrips_via_from_bytes() {
        let bytes = [0xab; 32];
        let pk = WarrenPubKey::from_bytes(&bytes);
        let back = pk.to_bytes().expect("valid pubkey must decode");
        assert_eq!(back, bytes, "bytes -> SS58 -> bytes must be identity");
        assert_eq!(WarrenPubKey::from_bytes(&back), pk);
    }

    #[test]
    fn from_bytes_produces_wb_prefixed_address() {
        let pk = WarrenPubKey::from_bytes(&[0xab; 32]);
        assert!(pk.as_str().starts_with("wb"), "got {}", pk.as_str());
        assert!((47..=49).contains(&pk.as_str().len()));
    }

    #[test]
    fn serde_roundtrips_through_json() {
        let pk = WarrenPubKey::from_str(VALID_SS58).unwrap();
        let json = serde_json::to_string(&pk).expect("serialize must succeed");
        assert_eq!(json, format!("\"{VALID_SS58}\""));
        let pk2: WarrenPubKey = serde_json::from_str(&json).expect("deserialize must succeed");
        assert_eq!(pk, pk2);
    }

    #[test]
    fn serde_deserialize_rejects_invalid_address() {
        // A corrupt payload must be rejected at deserialization.
        let res: Result<WarrenPubKey, _> = serde_json::from_str("\"not-an-address\"");
        assert!(res.is_err(), "invalid JSON address must fail");
    }

    #[test]
    fn equality_and_hash_treat_same_address_as_equal() {
        use std::collections::HashSet;
        let pk1 = WarrenPubKey::from_str(VALID_SS58).unwrap();
        let pk2 = WarrenPubKey::from_str(VALID_SS58).unwrap();
        assert_eq!(pk1, pk2);
        let mut set = HashSet::new();
        set.insert(pk1);
        assert!(set.contains(&pk2), "same address must hash identical");
    }
}
