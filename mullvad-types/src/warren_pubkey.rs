//! Newtype `WarrenPubKey` qui remplace l'alias `pub type AccountNumber
//! = String` ([`crate::account`]) par une représentation strictement
//! validée d'une pubkey Ed25519.
//!
//! **Format** : chaîne hex de 64 caractères ASCII `[0-9a-f]` (=
//! 32 octets binaires). Toute autre forme est refusée à la
//! construction (`parse, don't validate`).
//!
//! **Pourquoi un newtype** : (1) éviter les confusions avec d'autres
//! `String` (cf. règle `type-newtype-ids` rust-skills) ; (2) garantir
//! qu'une `WarrenPubKey` est *toujours* parseable en `[u8; 32]` sans
//! re-validation aval ; (3) préparer le remplacement progressif de
//! `AccountNumber` dans toute la chaîne API + daemon (cf.
//! `warren-core/docs/06-auth-wallet.md`).

use std::fmt;
use std::str::FromStr;

/// Longueur en chars hex d'une pubkey Ed25519 (32 octets × 2).
pub const PUBKEY_HEX_LEN: usize = 64;

/// Pubkey Warren — wrapper validé autour d'une chaîne hex 64-chars
/// ASCII représentant une pubkey Ed25519 (32 octets).
///
/// Construire via [`Self::from_str`] (la seule voie validée). Le
/// champ interne reste `pub(crate)` pour interdire l'instanciation
/// non-validée depuis l'extérieur de la crate.
///
/// **Sérialisation Serde** : `#[serde(transparent)]` — équivaut à un
/// JSON string. Le `Deserialize` re-valide via [`Self::from_str`]
/// pour qu'un payload corrompu (hex malformé, mauvaise longueur)
/// échoue à la désérialisation plutôt que de produire un
/// `WarrenPubKey` invalide silencieusement.
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

/// Erreur de parsing pour [`WarrenPubKey`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    /// La chaîne n'a pas la longueur attendue (= [`PUBKEY_HEX_LEN`]).
    #[error("expected {expected} chars, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    /// La chaîne contient un caractère qui n'est pas hex.
    #[error("non-hex character in pubkey")]
    NonHex,
}

impl WarrenPubKey {
    /// Retourne la représentation hex sous-jacente.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Retourne les 32 octets représentés. Toujours `Ok` parce que
    /// la construction garantit hex-validité ; on retourne un
    /// `Result` pour rester conforme à `# Errors`. La conversion ne
    /// peut échouer que si une corruption mémoire mute le champ.
    ///
    /// # Errors
    ///
    /// [`hex::FromHexError`] si la représentation interne a été
    /// muté hors-API (= bug interne, jamais à l'usage normal).
    pub fn to_bytes(&self) -> Result<[u8; 32], hex::FromHexError> {
        let v = hex::decode(&self.0)?;
        // 32 octets garantis par la longueur 64ch validée à la construction.
        v.try_into()
            .map_err(|_| hex::FromHexError::InvalidStringLength)
    }

    /// Construit depuis 32 octets bruts (e.g. un
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

    /// Valeur fixture : pubkey Ed25519 dérivée de la seed `[7u8; 32]`
    /// (cf. `mullvad_api::warren_auth::tests::fixed_signer`). Le test
    /// peut utiliser n'importe quelle hex 64-chars valide ; on prend
    /// celle-ci pour cohérence avec les autres vector tests.
    const VALID_HEX_64: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn from_str_accepts_valid_64_char_hex() {
        // Phase 2.B.1 — un hex 64 chars ASCII strict doit construire
        // un `WarrenPubKey` sans erreur.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).expect("valid hex must parse");
        assert_eq!(pk.as_str(), VALID_HEX_64);
    }

    #[test]
    fn from_str_rejects_too_short() {
        // 63 chars (1 de trop court) → InvalidLength { expected: 64, actual: 63 }
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
        // 65 chars → InvalidLength { actual: 65 }
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
        // 64 chars dont 1 non-hex (`z`) → NonHex
        let mut bad = String::from(VALID_HEX_64);
        bad.replace_range(0..1, "z");
        let err = WarrenPubKey::from_str(&bad).expect_err("non-hex must fail");
        assert_eq!(err, ParseError::NonHex);
    }

    #[test]
    fn from_str_rejects_empty_string() {
        // Edge case : empty → InvalidLength
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
        // Cycle 3 — la conversion bytes ↔ hex est bijective.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let bytes = pk.to_bytes().expect("valid pubkey must convert");
        assert_eq!(bytes.len(), 32);
        let pk2 = WarrenPubKey::from_bytes(&bytes);
        assert_eq!(pk, pk2, "bytes -> hex -> bytes doit être identité");
    }

    #[test]
    fn from_bytes_produces_lowercase_hex() {
        // Format : on impose lowercase à la sortie de `from_bytes`,
        // pour cohérence avec `mullvad_api::warren_auth` qui produit
        // toujours lowercase via `hex::encode`. Le `from_str` accepte
        // upper aussi (via `is_ascii_hexdigit`), mais la convention
        // de sortie est lowercase.
        let bytes = [0xab; 32];
        let pk = WarrenPubKey::from_bytes(&bytes);
        assert_eq!(pk.as_str(), "ab".repeat(32));
    }

    #[test]
    fn serde_roundtrips_through_json() {
        // Cycle 3 — sérialisation transparent vers JSON string.
        let pk = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let json = serde_json::to_string(&pk).expect("serialize must succeed");
        // `serde(transparent)` => JSON string `"hexvalue"` (avec quotes)
        assert_eq!(json, format!("\"{VALID_HEX_64}\""));
        let pk2: WarrenPubKey = serde_json::from_str(&json).expect("deserialize must succeed");
        assert_eq!(pk, pk2);
    }

    #[test]
    fn serde_deserialize_rejects_invalid_hex() {
        // Sécurité : un payload corrompu doit être rejeté à la
        // désérialisation, pas produire un `WarrenPubKey` invalide
        // silencieusement.
        let bad_json = "\"not-hex\"";
        let res: Result<WarrenPubKey, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "non-hex JSON doit échouer");
    }

    #[test]
    fn equality_and_hash_treat_same_hex_as_equal() {
        // Pour usage en HashMap key (futur cache nonce-by-pubkey, etc.).
        use std::collections::HashSet;
        let pk1 = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        let pk2 = WarrenPubKey::from_str(VALID_HEX_64).unwrap();
        assert_eq!(pk1, pk2, "même hex doit être == ");
        let mut set = HashSet::new();
        set.insert(pk1);
        assert!(set.contains(&pk2), "même hex doit hash identique");
    }
}
