// Wallet primitives wrapping `warren-identity` (BIP39 + Ed25519).
//
// This module is intentionally **not** `target_os = "android"`-gated so the
// logic can be unit-tested on the host. The JNI surface that calls into it
// lives in `lib.rs` and is the only Android-gated piece.
//
// Three primitives, mapped 1:1 to the JNI exports declared in `lib.rs`:
//
//   - `generate_mnemonic()` -> fresh 12-word English BIP39 phrase
//   - `pubkey_from_mnemonic(...)` -> derives the Ed25519 verifying key
//   - `sign_message(...)` -> Ed25519 signs an arbitrary byte blob with the
//     key derived from the supplied mnemonic
//
// All three are stateless. The Kotlin caller is responsible for caching the
// mnemonic in Android Keystore + EncryptedSharedPreferences and
// passing it back per signing call. Holding the SigningKey long-living
// inside the JNI library would put a wallet secret in app memory for the
// entire session lifetime - we avoid that.

use ed25519_dalek::SigningKey;
use warren_identity::{derive_node_key, seed_from_mnemonic};

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
}

impl From<warren_identity::MnemonicError> for WalletError {
    fn from(e: warren_identity::MnemonicError) -> Self {
        WalletError::InvalidMnemonic(e.to_string())
    }
}

/// Generate a fresh 12-word English BIP39 mnemonic.
///
/// Per warren-core convention, 12 words give 128 bits of entropy - enough
/// for an Ed25519 derivation that itself only consumes 256 bits, and the
/// usability bump over 24 words is significant (writing 12 words by hand
/// is realistic on mobile, 24 is not).
#[must_use]
pub fn generate_mnemonic() -> String {
    bip39::Mnemonic::generate(12)
        .expect("BIP39 12-word generation never fails for a valid word count")
        .to_string()
}

/// Derive the Ed25519 verifying key (public key) from a BIP39 mnemonic.
///
/// Returns the raw 32-byte public key. Encode it with
/// [`pubkey_ss58_from_mnemonic`] for the canonical string form (the
/// `X-Warren-PubKey` header, the Kotlin wallet repository identifier),
/// or sign with the matching [`signing_key_from_mnemonic`].
pub fn pubkey_from_mnemonic(mnemonic: &str) -> Result<[u8; 32], WalletError> {
    let seed = seed_from_mnemonic(mnemonic)?;
    let key = derive_node_key(&seed);
    Ok(key.verifying_key().to_bytes())
}

/// Convenience wrapper: derive the Ed25519 verifying key from `mnemonic`
/// and return its **Warren SS58 address** (`wb…`, network prefix 13295).
///
/// This is the canonical string form of the Warren wallet identity - the
/// value carried in the `X-Warren-PubKey` request header, copied to the
/// clipboard, and stored by the Kotlin wallet repository. The same
/// algorithm (`warren_identity::ss58`) is used by the daemon and the
/// backend verifier, so the address round-trips byte-for-byte.
pub fn pubkey_ss58_from_mnemonic(mnemonic: &str) -> Result<String, WalletError> {
    let pubkey = pubkey_from_mnemonic(mnemonic)?;
    Ok(warren_identity::ss58::encode(&pubkey))
}

/// Derive the Ed25519 [`SigningKey`] from a BIP39 mnemonic.
///
/// Same derivation chain as [`pubkey_from_mnemonic`] - the two are
/// guaranteed to produce key pairs that agree (`signing.verifying_key() ==
/// pubkey`). Used by the tunnel session bootstrap to feed the multi-hop
/// client (`MultiHopClient`) setup.
pub fn signing_key_from_mnemonic(mnemonic: &str) -> Result<SigningKey, WalletError> {
    let seed = seed_from_mnemonic(mnemonic)?;
    Ok(derive_node_key(&seed))
}

/// The key the wallet's session-token batches are blinded with, from the
/// same BIP39 seed [`signing_key_from_mnemonic`] derives from.
///
/// It must be the SEED, never the signing key: every client of a wallet (the
/// desktop app, the extension, another phone) derives its batch this way, and
/// the issuer serves one batch per account and epoch to whoever sends it bit
/// for bit (warren-core doc 103 section 11). One-way: it does not give back
/// the seed.
pub fn session_blinding_from_mnemonic(
    mnemonic: &str,
) -> Result<warren_api::BlindingKey, WalletError> {
    let seed = seed_from_mnemonic(mnemonic)?;
    Ok(warren_api::BlindingKey::session(&seed))
}

/// The key the wallet's port-entitlement batches are blinded with, from the
/// same BIP39 seed, for the same reason as [`session_blinding_from_mnemonic`]:
/// a restarted tunnel process, or another device of the wallet, is served
/// the batch the account already holds instead of `already_issued`.
pub fn entitlement_blinding_from_mnemonic(
    mnemonic: &str,
) -> Result<warren_api::BlindingKey, WalletError> {
    let seed = seed_from_mnemonic(mnemonic)?;
    Ok(warren_api::BlindingKey::port_entitlement(&seed))
}

/// Sign `message` with the Ed25519 signing key derived from `mnemonic`.
///
/// Returns the raw 64-byte signature. The signing key never escapes this
/// function: it is dropped (and zeroized, since `SigningKey` carries
/// `ZeroizeOnDrop` via `ed25519-dalek` 2.x `zeroize` feature) the moment
/// the sign call returns.
pub fn sign_message(mnemonic: &str, message: &[u8]) -> Result<[u8; 64], WalletError> {
    use ed25519_dalek::Signer;

    let seed = seed_from_mnemonic(mnemonic)?;
    let key = derive_node_key(&seed);
    Ok(key.sign(message).to_bytes())
}

/// Build the canonical message that Warren signs for the
/// `X-Warren-Signature` header, then sign it with `mnemonic`.
///
/// The canonical bytes are produced by
/// `warren_identity::canonical_message` so this helper is the
/// single source of truth on the client side: Kotlin (or Swift, on iOS)
/// callers do not duplicate the byte-stable string concatenation, which
/// keeps wire-format drift impossible from the app layer.
///
/// Field semantics mirror the `X-Warren-*` headers:
///   - `method`: HTTP verb, uppercase (`GET`, `POST`, ...)
///   - `path`: URL path with leading `/` (no host)
///   - `timestamp`: Unix epoch seconds
///   - `nonce_hex`: 16-byte random nonce, hex-encoded (no `0x`)
///   - `body_hash_hex`: SHA-256 of the request body, hex-encoded
///     (empty-string SHA-256 for GET / empty body)
pub fn sign_canonical_request(
    mnemonic: &str,
    method: &str,
    path: &str,
    timestamp: u64,
    nonce_hex: &str,
    body_hash_hex: &str,
) -> Result<[u8; 64], WalletError> {
    let msg = warren_identity::canonical_message(method, path, timestamp, nonce_hex, body_hash_hex);
    sign_message(mnemonic, msg.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_mnemonic_has_twelve_words() {
        let phrase = generate_mnemonic();
        assert_eq!(
            phrase.split_whitespace().count(),
            12,
            "BIP39 12-word phrase expected, got {phrase:?}"
        );
    }

    /// Answers the port-entitlement directory, records the blinded batch of
    /// the issue request and refuses it, so a test sees what a key sends.
    struct BatchRecorder {
        key: warrenguard_token::IssuerSecretKey,
        blinded: parking_lot::Mutex<Vec<String>>,
    }

    impl BatchRecorder {
        fn new() -> Self {
            use rand010::SeedableRng;
            let mut rng = rand010::rngs::StdRng::seed_from_u64(7);
            Self {
                key: warrenguard_token::IssuerSecretKey::generate(&mut rng).unwrap(),
                blinded: parking_lot::Mutex::new(Vec::new()),
            }
        }

        fn directory(&self) -> warren_api::TokenIssuerDirectory {
            let pk = self.key.public_key();
            let attribution = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]);
            warren_api::TokenIssuerDirectory {
                issuer_name: "api.warrenbrowse.com".to_owned(),
                token_type: 2,
                epoch_secs: 3600,
                context_label: "warren/session-token/v1".to_owned(),
                quota_per_epoch: 5,
                prefetch_epochs: 48,
                keys: vec![warren_api::TokenIssuerKey {
                    epoch: 100,
                    token_key_id: pk.key_id().to_hex(),
                    spki_b64: data_encoding::BASE64URL_NOPAD.encode(&pk.to_spki()),
                    not_before: 100 * 3600,
                    not_after: 101 * 3600,
                }],
                attribution_verifying_key_hex: Some(
                    warren_api::PubkeyHex::try_from(
                        hex::encode(attribution.verifying_key().as_bytes()).as_str(),
                    )
                    .unwrap(),
                ),
                route_admission: None,
            }
        }
    }

    impl warren_api::HttpTransport for &BatchRecorder {
        async fn execute(
            &self,
            request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            let issue: warren_api::TokenIssueRequest =
                serde_json::from_slice(&request.body).unwrap();
            *self.blinded.lock() = issue.epochs[0].blinded.clone();
            Ok(warren_api::HttpResponse::new(503, Vec::new()))
        }
    }

    /// The batch `key` sends for epoch 100.
    fn batch_of(key: &warren_api::BlindingKey) -> Vec<String> {
        let recorder = BatchRecorder::new();
        let client = warren_api::WarrenApiClient::new(
            "https://api.example.test",
            warren_identity::WarrenIdentity::from_seed(&[1; 32]),
            &recorder,
        );
        let _ = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(warren_api::mint_tokens(
                &client,
                &recorder.directory(),
                &[100],
                key,
            ));
        recorder.blinded.lock().clone()
    }

    #[test]
    fn the_entitlement_key_is_derived_from_the_seed_every_client_derives_from() {
        // Every client of the wallet must send this batch, or it is refused
        // `already_issued` for 48 h. The node key differs from the seed.
        const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = seed_from_mnemonic(MNEMONIC).unwrap();

        let sent = batch_of(&entitlement_blinding_from_mnemonic(MNEMONIC).unwrap());

        assert_eq!(sent.len(), 5, "the whole batch was sent");
        assert_eq!(
            sent,
            batch_of(&warren_api::BlindingKey::port_entitlement(&seed))
        );
        assert_ne!(
            sent,
            batch_of(&warren_api::BlindingKey::port_entitlement(
                &derive_node_key(&seed).to_bytes()
            ))
        );
    }

    #[test]
    fn generated_mnemonics_are_unique() {
        let a = generate_mnemonic();
        let b = generate_mnemonic();
        assert_ne!(a, b, "two consecutive generate_mnemonic() calls collided");
    }

    #[test]
    fn pubkey_ss58_is_a_warren_address() {
        let phrase = generate_mnemonic();
        let addr = pubkey_ss58_from_mnemonic(&phrase).unwrap();
        assert!(addr.starts_with("wb"), "expected a wb… address, got {addr}");
        // 47-49 base58 chars for a 32-byte account with a 2-byte prefix.
        assert!((47..=49).contains(&addr.len()), "unexpected length: {addr}");
    }

    #[test]
    fn pubkey_ss58_matches_byte_form() {
        let phrase = generate_mnemonic();
        let bytes = pubkey_from_mnemonic(&phrase).unwrap();
        let addr = pubkey_ss58_from_mnemonic(&phrase).unwrap();
        assert_eq!(addr, warren_identity::ss58::encode(&bytes));
        // And it decodes back to exactly those bytes.
        assert_eq!(warren_identity::ss58::decode(&addr).unwrap(), bytes);
    }

    #[test]
    fn pubkey_is_deterministic() {
        let phrase = generate_mnemonic();
        let p1 = pubkey_from_mnemonic(&phrase).expect("derive 1");
        let p2 = pubkey_from_mnemonic(&phrase).expect("derive 2");
        assert_eq!(p1, p2);
    }

    #[test]
    fn pubkey_differs_between_mnemonics() {
        let p1 = pubkey_from_mnemonic(&generate_mnemonic()).unwrap();
        let p2 = pubkey_from_mnemonic(&generate_mnemonic()).unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn invalid_mnemonic_is_rejected() {
        let err = pubkey_from_mnemonic("not a real mnemonic at all just garbage").unwrap_err();
        assert!(matches!(err, WalletError::InvalidMnemonic(_)));
    }

    #[test]
    fn sign_verify_roundtrip() {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let phrase = generate_mnemonic();
        let pubkey_bytes = pubkey_from_mnemonic(&phrase).unwrap();
        let pubkey = VerifyingKey::from_bytes(&pubkey_bytes).unwrap();
        let msg = b"GET\n/v1/exits\n42\nabcd1234\nff00";

        let sig_bytes = sign_message(&phrase, msg).unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        pubkey.verify(msg, &sig).expect("signature must verify");
    }

    #[test]
    fn sign_tampered_message_fails_verification() {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let phrase = generate_mnemonic();
        let pubkey_bytes = pubkey_from_mnemonic(&phrase).unwrap();
        let pubkey = VerifyingKey::from_bytes(&pubkey_bytes).unwrap();

        let sig_bytes = sign_message(&phrase, b"original").unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        let tampered = b"original-tampered";
        assert!(pubkey.verify(tampered, &sig).is_err());
    }

    #[test]
    fn sign_canonical_request_verifies() {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let phrase = generate_mnemonic();
        let pubkey_bytes = pubkey_from_mnemonic(&phrase).unwrap();
        let pubkey = VerifyingKey::from_bytes(&pubkey_bytes).unwrap();

        let sig_bytes =
            sign_canonical_request(&phrase, "GET", "/v1/exits", 42, "abcd1234", "ff00").unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        // Reconstruct the canonical message exactly as warren-identity
        // builds it, then verify against the pubkey.
        let expected =
            warren_identity::canonical_message("GET", "/v1/exits", 42, "abcd1234", "ff00");
        pubkey.verify(expected.as_bytes(), &sig).expect(
            "canonical-request signature must verify against the warren-identity-built message",
        );
    }

    /// Wire vector: a fixed mnemonic must always derive the same pubkey.
    /// A change to this frozen hex means the HKDF salt/info or the key
    /// derivation drifted, which breaks every existing account, so the auth
    /// schema version must be bumped deliberately alongside it.
    #[test]
    fn fixed_mnemonic_derives_stable_pubkey() {
        // Official BIP39 all-zero-entropy test vector (12 words).
        const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        const EXPECTED_PUBKEY_HEX: &str =
            "01fcbb300c7212762f44992dfe581af2a48dc6f304317f509334838a87d2b58c";
        let pubkey = pubkey_from_mnemonic(PHRASE).unwrap();
        assert_eq!(hex::encode(pubkey), EXPECTED_PUBKEY_HEX);
    }
}
