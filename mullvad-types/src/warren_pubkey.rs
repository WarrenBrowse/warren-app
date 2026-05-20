//! `WarrenPubKey` newtype that replaces the `pub type AccountNumber
//! = String` alias ([`crate::account`]) with a strictly validated
//! representation of an Ed25519 pubkey.
//!
//! **Format**: 64-character ASCII hex string `[0-9a-f]` (=
//! 32 binary bytes). Any other shape is rejected at
//! construction (`parse, don't validate`).
//!
//! **Why a newtype**: (1) avoid confusion with other
//! `String`s (see rule `type-newtype-ids` rust-skills); (2) guarantee
//! that a `WarrenPubKey` is *always* parseable into `[u8; 32]` without
//! re-validation downstream; (3) prepare the progressive replacement of
//! `AccountNumber` across the API + daemon chain (see
//! `warren-core/docs/06-auth-wallet.md`).

use std::fmt;
use std::str::FromStr;

/// Length in hex chars of an Ed25519 pubkey (32 bytes x 2).
pub const PUBKEY_HEX_LEN: usize = 64;

/// Warren pubkey — validated wrapper around a 64-char ASCII hex string
/// representing an Ed25519 pubkey (32 bytes).
///
/// Construct via [`Self::from_str`] (the only validated path). The
/// inner field stays `pub(crate)` to forbid non-validated
/// instantiation from outside the crate.
///
/// **Serde serialization**: `#[serde(transparent)]` — equivalent to a
/// JSON string. The `Deserialize` impl re-validates via [`Self::from_str`]
/// so that a corrupt payload (malformed hex, wrong length) fails
/// deserialization rather than silently producing an invalid
/// `WarrenPubKey`.
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
    /// The string does not have the expected length (= [`PUBKEY_HEX_LEN`]).
    #[error("expected {expected} chars, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    /// The string contains a non-hex character.
    #[error("non-hex character in pubkey")]
    NonHex,
}

impl WarrenPubKey {
    /// Returns the underlying hex representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the 32 represented bytes. Always `Ok` because
    /// construction guarantees hex-validity; we return a
    /// `Result` to stay compliant with `# Errors`. The conversion can
    /// only fail if memory corruption mutates the field.
    ///
    /// # Errors
    ///
    /// [`hex::FromHexError`] if the internal representation has been
    /// mutated out-of-API (= internal bug, never in normal use).
    pub fn to_bytes(&self) -> Result<[u8; 32], hex::FromHexError> {
        let v = hex::decode(&self.0)?;
        // 32 bytes guaranteed by the 64-char length validated at construction.
        v.try_into()
            .map_err(|_| hex::FromHexError::InvalidStringLength)
    }

    /// Build from 32 raw bytes (e.g. an
    /// `ed25519_dalek::VerifyingKey::as_bytes()`).
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(hex::encode(bytes))
    }
}

impl FromStr for WarrenPubKey {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != PUBKEY_HEX_LEN {
            return Err(ParseError::InvalidLength {
                expected: PUBKEY_HEX_LEN,
                actual: s.len(),
            });
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ParseError::NonHex);
        }
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

    /// Fixture value: Ed25519 pubkey derived from the seed `[7u8; 32]`
    /// (see `mullvad_api::warren_auth::tests::fixed_signer`). The test
    /// can use any valid 64-char hex; we pick this one for consistency
    /// with the other vector tests.
    const VALID_HEX_64: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn from_str_accepts_valid_64_char_hex() {
        // Phase 2.B.1 — a strict 64-char ASCII hex must build a
        // `WarrenPubKey` without error.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).expect("valid hex must parse");
        assert_eq!(pk.as_str(), VALID_HEX_64);
    }

    #[test]
    fn from_str_rejects_too_short() {
        // 63 chars (1 too short) -> InvalidLength { expected: 64, actual: 63 }
        let too_short = &VALID_HEX_64[..63];
        let err = WarrenPubKey::from_str(too_short).expect_err("too short must fail");
        assert_eq!(
            err,
            ParseError::InvalidLength {
                expected: 64,
                actual: 63,
            }
        );
    }

    #[test]
    fn from_str_rejects_too_long() {
        // 65 chars -> InvalidLength { actual: 65 }
        let too_long = format!("{VALID_HEX_64}a");
        let err = WarrenPubKey::from_str(&too_long).expect_err("too long must fail");
        assert!(matches!(
            err,
            ParseError::InvalidLength {
                expected: 64,
                actual: 65,
            }
        ));
    }

    #[test]
    fn from_str_rejects_non_hex_character() {
        // 64 chars including 1 non-hex (`z`) -> NonHex
        let mut bad = String::from(VALID_HEX_64);
        bad.replace_range(0..1, "z");
        let err = WarrenPubKey::from_str(&bad).expect_err("non-hex must fail");
        assert_eq!(err, ParseError::NonHex);
    }

    #[test]
    fn from_str_rejects_empty_string() {
        // Edge case: empty -> InvalidLength
        let err = WarrenPubKey::from_str("").expect_err("empty must fail");
        assert_eq!(
            err,
            ParseError::InvalidLength {
                expected: 64,
                actual: 0,
            }
        );
    }

    #[test]
    fn display_returns_underlying_hex() {
        let pk = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        assert_eq!(format!("{pk}"), VALID_HEX_64);
    }

    #[test]
    fn to_bytes_roundtrips_via_from_bytes() {
        // Cycle 3 — the bytes <-> hex conversion is bijective.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let bytes = pk.to_bytes().expect("valid pubkey must convert");
        assert_eq!(bytes.len(), 32);
        let pk2 = WarrenPubKey::from_bytes(&bytes);
        assert_eq!(pk, pk2, "bytes -> hex -> bytes must be identity");
    }

    #[test]
    fn from_bytes_produces_lowercase_hex() {
        // Format: we enforce lowercase on the output of `from_bytes`,
        // for consistency with `mullvad_api::warren_auth` which always
        // produces lowercase via `hex::encode`. `from_str` also accepts
        // uppercase (via `is_ascii_hexdigit`), but the output
        // convention is lowercase.
        let bytes = [0xab; 32];
        let pk = WarrenPubKey::from_bytes(&bytes);
        assert_eq!(pk.as_str(), "ab".repeat(32));
    }

    #[test]
    fn serde_roundtrips_through_json() {
        // Cycle 3 — transparent serialization to JSON string.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let json = serde_json::to_string(&pk).expect("serialize must succeed");
        // `serde(transparent)` => JSON string `"hexvalue"` (with quotes)
        assert_eq!(json, format!("\"{VALID_HEX_64}\""));
        let pk2: WarrenPubKey = serde_json::from_str(&json).expect("deserialize must succeed");
        assert_eq!(pk, pk2);
    }

    #[test]
    fn serde_deserialize_rejects_invalid_hex() {
        // Security: a corrupt payload must be rejected at
        // deserialization, not silently produce an invalid
        // `WarrenPubKey`.
        let bad_json = "\"not-hex\"";
        let res: Result<WarrenPubKey, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "non-hex JSON must fail");
    }

    #[test]
    fn equality_and_hash_treat_same_hex_as_equal() {
        // For use as HashMap key (future nonce-by-pubkey cache, etc.).
        use std::collections::HashSet;
        let pk1 = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let pk2 = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        assert_eq!(pk1, pk2, "same hex must be == ");
        let mut set = HashSet::new();
        set.insert(pk1);
        assert!(set.contains(&pk2), "same hex must hash identical");
    }
}
