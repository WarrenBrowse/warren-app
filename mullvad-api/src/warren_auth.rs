//! Ed25519 signature on HTTP API requests.
//!
//! Replaces the Mullvad `Authorization: Bearer <token>` model with a
//! canonical signature of each request, proving possession of the
//! Warren private key (derived from the user's BIP39 mnemonic via
//! `warren_identity::derive_node_key`). No token lifecycle, no cache
//! to invalidate.
//!
//! **Canonical format** (see `warren-core/docs/06-auth-wallet.md` § 110):
//!
//! ```text
//! message = METHOD || "\n" || path || "\n" || timestamp || "\n" || nonce_hex || "\n" || sha256_hex(body)
//! sig     = Ed25519::sign(secret_key, message)
//! ```
//!
//! **Injected HTTP headers**:
//!
//! - `X-Warren-PubKey`    : 64-char hex (32-byte pubkey)
//! - `X-Warren-Sig`       : 128-char hex (64-byte signature)
//! - `X-Warren-Timestamp` : decimal epoch seconds
//! - `X-Warren-Nonce`     : 32-char hex (16-byte random nonce)
//!
//! **Server-side validation** (see `warren-core/crates/warren-api/`):
//! 1. `|now - timestamp| <= 60 s` (clock skew window)
//! 2. `nonce` not seen in the previous 120 s (in-RAM LRU)
//! 3. signature verifies against the pubkey
//! 4. pubkey is in the active subscription table
//!
//! **Warren no-log**: the signing_key and pubkey are NEVER logged in
//! the clear. The `Debug` impl explicitly masks them.

use ed25519_dalek::{Signer, SigningKey};
use rand::RngCore;
use sha2::{Digest, Sha256};
// Re-exported from warren-identity::auth via warren-api-client: the
// canonical wire format constants and the canonical_message builder
// MUST be identical on both sides (signer here, verifier server-side),
// otherwise no signature ever verifies. Consuming the single source of
// truth prevents silent /v1 wire divergence.
pub use warren_api_client::{
    HEADER_NONCE, HEADER_PUBKEY, HEADER_SIGNATURE, HEADER_TIMESTAMP, canonical_message,
};

/// HTTP headers produced by [`WarrenAuthSigner::sign_request`].
///
/// The caller (`mullvad-api::rest`) consumes these 4 values and
/// injects them into the `hyper::Request` using the official header
/// names: [`HEADER_PUBKEY`], [`HEADER_SIGNATURE`], [`HEADER_TIMESTAMP`],
/// [`HEADER_NONCE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarrenAuthHeaders {
    /// Signer Ed25519 pubkey, 64-char hex (32 bytes).
    pub pubkey_hex: String,
    /// Ed25519 signature over the `canonical_message`, 128-char hex (64 bytes).
    pub signature_hex: String,
    /// Unix epoch-seconds timestamp (= maximum request age on the
    /// server, which rejects if `|now - timestamp| > 60 s`).
    pub timestamp: u64,
    /// Random 32-char hex nonce (16 bytes), unique per request — used
    /// by the server to block replay attacks within the 120 s window.
    pub nonce_hex: String,
}

/// Nonce size in bytes (= 128 bits, large enough that a single client
/// can generate 2^64 requests without collisions at probability << 1%).
pub const NONCE_BYTES: usize = 16;

/// Warren signer — owns the Ed25519 private key and exposes
/// [`Self::sign_request`], which produces the 4 HTTP headers to inject
/// into a request.
///
/// **Instance lifecycle**: one signer = one Warren identity = one
/// unique pubkey. The signing key is derived once at daemon boot from
/// the user's BIP39 mnemonic (see `warren_identity::derive_node_key`)
/// and kept in RAM until shutdown or identity change (logout / import
/// of a new mnemonic).
pub struct WarrenAuthSigner {
    signing_key: SigningKey,
}

impl std::fmt::Debug for WarrenAuthSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Warren no-log: NEVER reveal the signing_key. The pubkey is
        // technically public but is still a user identifier; we mask
        // it as well for consistency.
        f.debug_struct("WarrenAuthSigner")
            .field("signing_key", &"<redacted>")
            .field("pubkey", &"<redacted>")
            .finish()
    }
}

impl WarrenAuthSigner {
    /// Builds a signer from a [`SigningKey`] derived from a user's
    /// BIP39 mnemonic (see `warren_identity::derive_node_key`).
    #[must_use]
    pub fn new(signing_key: SigningKey) -> Self {
        Self { signing_key }
    }

    /// Signer hex pubkey (64 chars). Useful for external APIs that log
    /// the `client_id` (= pubkey) without touching the signing key.
    #[must_use]
    pub fn pubkey_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Signs a request with a `timestamp` and `nonce` provided by the
    /// caller. Deterministic variant used by tests to pin a fixed
    /// input vector.
    ///
    /// **Production**: use [`Self::sign_request`], which generates
    /// `timestamp` (= now) and `nonce` (= random) automatically.
    #[must_use]
    pub fn sign_request_at(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        timestamp: u64,
        nonce: [u8; NONCE_BYTES],
    ) -> WarrenAuthHeaders {
        let nonce_hex = hex::encode(nonce);
        let body_hash_hex = hex::encode(Sha256::digest(body));
        let canonical = canonical_message(method, path, timestamp, &nonce_hex, &body_hash_hex);
        let signature = self.signing_key.sign(canonical.as_bytes());

        WarrenAuthHeaders {
            pubkey_hex: self.pubkey_hex(),
            signature_hex: hex::encode(signature.to_bytes()),
            timestamp,
            nonce_hex,
        }
    }

    /// Signs a request with `timestamp = now()` and a random nonce
    /// (cryptographically secure, via `rand::rng()`).
    ///
    /// # Panics
    ///
    /// Does not panic in practice: `SystemTime::now() < UNIX_EPOCH`
    /// would imply a clock set before 1970, which is impossible on a
    /// functional system. If the conversion fails, we fall back to
    /// `timestamp = 0`, which will be rejected server-side (clock
    /// skew), but the client does not crash.
    #[must_use]
    pub fn sign_request(&self, method: &str, path: &str, body: &[u8]) -> WarrenAuthHeaders {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut nonce = [0u8; NONCE_BYTES];
        rand::rng().fill_bytes(&mut nonce);
        self.sign_request_at(method, path, body, timestamp, nonce)
    }

    /// Applies the 4 X-Warren-* headers to an existing `hyper::Request`,
    /// using the request's HTTP method, path, and body as inputs to the
    /// `canonical_message`.
    ///
    /// **Why this signature**: factorizes the `(method, path, body)`
    /// extraction on the caller side, which must provide `body` as
    /// bytes because `hyper::Request<B>` does not expose the body
    /// without consuming it. In practice the caller (rest.rs) already
    /// has the serialized body as `Vec<u8>` before building the
    /// request.
    ///
    /// # Errors
    ///
    /// [`std::io::Error`] with [`std::io::ErrorKind::InvalidData`] if
    /// one of the produced `HeaderValue`s cannot be parsed (only a
    /// theoretical case: our values are all hex or decimal ASCII).
    pub fn apply_to_request<B>(
        &self,
        request: &mut http::Request<B>,
        body: &[u8],
    ) -> std::io::Result<()> {
        let method = request.method().as_str();
        // Path with query string. `path_and_query` returns `path?query`
        // or just `path` if no query — this is what we want to sign for
        // anti-tampering.
        let path = request
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or(request.uri().path());

        let headers = self.sign_request(method, path, body);
        let map = request.headers_mut();

        let invalid = |name: &'static str| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid header value for {name}"),
            )
        };

        map.insert(
            HEADER_PUBKEY,
            http::HeaderValue::from_str(&headers.pubkey_hex).map_err(|_| invalid(HEADER_PUBKEY))?,
        );
        map.insert(
            HEADER_SIGNATURE,
            http::HeaderValue::from_str(&headers.signature_hex)
                .map_err(|_| invalid(HEADER_SIGNATURE))?,
        );
        map.insert(
            HEADER_TIMESTAMP,
            http::HeaderValue::from_str(&headers.timestamp.to_string())
                .map_err(|_| invalid(HEADER_TIMESTAMP))?,
        );
        map.insert(
            HEADER_NONCE,
            http::HeaderValue::from_str(&headers.nonce_hex).map_err(|_| invalid(HEADER_NONCE))?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    /// Fixed key for reproducible test vectors. Seed = `[7u8; 32]`,
    /// derived pubkey is deterministic.
    fn fixed_signer() -> WarrenAuthSigner {
        WarrenAuthSigner::new(SigningKey::from_bytes(&[7u8; 32]))
    }

    #[test]
    fn pubkey_hex_is_64_chars() {
        // Format audit: Ed25519 pubkey = 32 bytes = 64 hex chars.
        let signer = fixed_signer();
        assert_eq!(signer.pubkey_hex().len(), 64);
        assert!(signer.pubkey_hex().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn signature_hex_is_128_chars() {
        // Format audit: Ed25519 signature = 64 bytes = 128 hex chars.
        let h = fixed_signer().sign_request_at("GET", "/v1/exits", b"", 1_700_000_000, [0u8; 16]);
        assert_eq!(h.signature_hex.len(), 128);
        assert!(h.signature_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn nonce_hex_is_32_chars() {
        // Format audit: 16-byte nonce = 32 hex chars.
        let h = fixed_signer().sign_request_at("GET", "/v1/exits", b"", 1_700_000_000, [0u8; 16]);
        assert_eq!(h.nonce_hex.len(), 32);
    }

    #[test]
    fn signature_is_deterministic_with_fixed_inputs() {
        // Vector test: identical (key, method, path, body, timestamp,
        // nonce) -> identical signature, guaranteed. Frozen-wire-format
        // guardrail — any format regression breaks this test.
        let signer = fixed_signer();
        let h1 = signer.sign_request_at(
            "POST",
            "/v1/port-forward/request",
            b"{}",
            1_700_000_000,
            [1u8; 16],
        );
        let h2 = signer.sign_request_at(
            "POST",
            "/v1/port-forward/request",
            b"{}",
            1_700_000_000,
            [1u8; 16],
        );
        assert_eq!(h1.signature_hex, h2.signature_hex);
        assert_eq!(h1.pubkey_hex, h2.pubkey_hex);
    }

    #[test]
    fn signature_changes_when_method_changes() {
        let signer = fixed_signer();
        let post = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);
        let get = signer.sign_request_at("GET", "/v1/x", b"", 1, [0u8; 16]);
        assert_ne!(
            post.signature_hex, get.signature_hex,
            "method MUST affect the signature (cross-method anti-replay)"
        );
    }

    #[test]
    fn signature_changes_when_path_changes() {
        let signer = fixed_signer();
        let a = signer.sign_request_at("POST", "/v1/a", b"", 1, [0u8; 16]);
        let b = signer.sign_request_at("POST", "/v1/b", b"", 1, [0u8; 16]);
        assert_ne!(
            a.signature_hex, b.signature_hex,
            "path MUST affect the signature (cross-endpoint anti-replay)"
        );
    }

    #[test]
    fn signature_changes_when_body_changes() {
        let signer = fixed_signer();
        let empty = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);
        let body = signer.sign_request_at("POST", "/v1/x", b"{\"k\":1}", 1, [0u8; 16]);
        assert_ne!(
            empty.signature_hex, body.signature_hex,
            "body MUST affect the signature (anti-tampering)"
        );
    }

    #[test]
    fn signature_changes_when_timestamp_changes() {
        let signer = fixed_signer();
        let t1 = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);
        let t2 = signer.sign_request_at("POST", "/v1/x", b"", 2, [0u8; 16]);
        assert_ne!(t1.signature_hex, t2.signature_hex);
    }

    #[test]
    fn signature_changes_when_nonce_changes() {
        let signer = fixed_signer();
        let n1 = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);
        let n2 = signer.sign_request_at("POST", "/v1/x", b"", 1, [1u8; 16]);
        assert_ne!(n1.signature_hex, n2.signature_hex);
    }

    #[test]
    fn signature_verifies_with_pubkey_and_canonical_message() {
        // E2E: we rebuild the canonical message on the "server" side
        // and verify the signature with the pubkey extracted from the
        // headers. This is exactly what the axum middleware in
        // `warren-api` does to authenticate an incoming request.
        let signer = fixed_signer();
        let body = b"{\"exit_pubkey\":\"abc\"}";
        let h = signer.sign_request_at(
            "POST",
            "/v1/port-forward/request",
            body,
            1_700_000_000,
            [42u8; 16],
        );

        // Server-side reconstruction:
        let body_hash_hex = hex::encode(Sha256::digest(body));
        let canonical = canonical_message(
            "POST",
            "/v1/port-forward/request",
            1_700_000_000,
            &h.nonce_hex,
            &body_hash_hex,
        );

        let pubkey_bytes: [u8; 32] = hex::decode(&h.pubkey_hex)
            .expect("pubkey hex valid")
            .try_into()
            .expect("32 bytes");
        let verifying_key =
            ed25519_dalek::VerifyingKey::from_bytes(&pubkey_bytes).expect("valid pubkey on curve");

        let sig_bytes: [u8; 64] = hex::decode(&h.signature_hex)
            .expect("sig hex valid")
            .try_into()
            .expect("64 bytes");
        let signature = Signature::from_bytes(&sig_bytes);

        verifying_key
            .verify(canonical.as_bytes(), &signature)
            .expect("signature must verify");
    }

    #[test]
    fn signature_does_not_verify_with_tampered_path() {
        // Security: if an attacker modifies the path in transit (e.g.
        // MITM), the signature must no longer verify. Tests the
        // "one bit changed = rejected" contract.
        let signer = fixed_signer();
        let h = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);

        // Reconstruction with forged path:
        let body_hash_hex = hex::encode(Sha256::digest(b""));
        let tampered = canonical_message("POST", "/v1/admin", 1, &h.nonce_hex, &body_hash_hex);

        let pubkey_bytes: [u8; 32] = hex::decode(&h.pubkey_hex).unwrap().try_into().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pubkey_bytes).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(&h.signature_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        assert!(
            vk.verify(tampered.as_bytes(), &sig).is_err(),
            "signature MUST NOT verify with a forged path"
        );
    }

    #[test]
    fn nonce_is_random_each_call_to_sign_request() {
        // Anti-replay: two consecutive calls must generate different
        // nonces. Collision probability on a 16-byte random nonce =
        // 1 / 2^128.
        let signer = fixed_signer();
        let a = signer.sign_request("GET", "/v1/x", b"");
        let b = signer.sign_request("GET", "/v1/x", b"");
        assert_ne!(a.nonce_hex, b.nonce_hex, "nonces must be unique");
    }

    #[test]
    fn debug_does_not_leak_signing_key() {
        // Warren no-log: Debug must NEVER reveal the signing_key or
        // pubkey, not even partially.
        let signer = fixed_signer();
        let s = format!("{signer:?}");
        assert!(s.contains("<redacted>"));
        assert!(!s.contains(&signer.pubkey_hex()[..10])); // do not reveal the first 10 chars of the pubkey
    }

    #[test]
    fn apply_to_request_injects_all_four_headers() {
        // Verifies that `apply_to_request` adds exactly the 4
        // X-Warren-* headers expected by the server, with the correct
        // value format (hex size for pubkey/sig/nonce, decimal ASCII
        // for timestamp).
        let signer = fixed_signer();
        let mut req = http::Request::builder()
            .method("POST")
            .uri("/v1/port-forward/request?dry=true")
            .body(())
            .expect("build request");

        signer
            .apply_to_request(&mut req, b"{\"exit_pubkey\":\"abc\"}")
            .expect("apply_to_request must succeed");

        let h = req.headers();
        let pk = h.get(HEADER_PUBKEY).expect("pubkey present");
        let sig = h.get(HEADER_SIGNATURE).expect("sig present");
        let ts = h.get(HEADER_TIMESTAMP).expect("ts present");
        let nonce = h.get(HEADER_NONCE).expect("nonce present");

        assert_eq!(pk.to_str().unwrap().len(), 64);
        assert_eq!(sig.to_str().unwrap().len(), 128);
        assert_eq!(nonce.to_str().unwrap().len(), 32);
        // Timestamp must be a valid decimal u64.
        ts.to_str()
            .unwrap()
            .parse::<u64>()
            .expect("timestamp must be a valid u64");
    }

    #[test]
    fn apply_to_request_signs_with_path_and_query() {
        // Anti-tampering: the signature must also cover the `?query`.
        // A malicious proxy that strips a `?dry=true` must not produce
        // a still-valid signature.
        let signer = fixed_signer();
        let mut req_with_q = http::Request::builder()
            .method("GET")
            .uri("/v1/exits?region=eu")
            .body(())
            .unwrap();
        let mut req_without_q = http::Request::builder()
            .method("GET")
            .uri("/v1/exits")
            .body(())
            .unwrap();

        // We pin the same timestamp + nonce so we can isolate the
        // effect of the path. To do that we go through
        // `sign_request_at` directly and compare with what
        // `apply_to_request` would inject.
        let h_with = signer.sign_request_at("GET", "/v1/exits?region=eu", b"", 1, [0u8; 16]);
        let h_without = signer.sign_request_at("GET", "/v1/exits", b"", 1, [0u8; 16]);
        assert_ne!(h_with.signature_hex, h_without.signature_hex);

        // We use the `req_*` values only to confirm that
        // `apply_to_request` does not crash on either path-only or
        // path+query variants.
        signer.apply_to_request(&mut req_with_q, b"").unwrap();
        signer.apply_to_request(&mut req_without_q, b"").unwrap();
    }

    #[test]
    fn body_hash_uses_sha256_for_empty_body() {
        // Cryptographic vector test: sha256("") =
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // Guarantees we have not silently mutated the hash algorithm.
        let h = fixed_signer().sign_request_at("GET", "/v1/x", b"", 0, [0u8; 16]);
        // Reconstruction: if the signature verifies against the
        // expected canonical message containing the standard sha256,
        // then the algorithm really is sha256.
        let expected_body_hash_hex =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let canonical = canonical_message("GET", "/v1/x", 0, &h.nonce_hex, expected_body_hash_hex);

        let pubkey_bytes: [u8; 32] = hex::decode(&h.pubkey_hex).unwrap().try_into().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pubkey_bytes).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(&h.signature_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        vk.verify(canonical.as_bytes(), &sig)
            .expect("body hash MUST be sha256(b\"\") = e3b0c44...");
    }

    /// Wire format regression - any divergence between the local
    /// canonical_message (pre-refactor) and the warren-identity::auth
    /// canonical_message (post-refactor) would cause every signed API
    /// request to be rejected server-side. The hardcoded expected
    /// string below was captured against the pre-refactor local impl;
    /// it MUST keep producing the same bytes after wiring through
    /// warren-api-client.
    #[test]
    fn canonical_message_matches_hardcoded_reference_vector() {
        let method = "POST";
        let path = "/v1/port-forward/request?dry=true";
        let timestamp: u64 = 1_700_000_000;
        let nonce_hex = "0102030405060708090a0b0c0d0e0f10";
        let body = b"{\"exit_pubkey\":\"abc\"}";
        let body_hash_hex = hex::encode(Sha256::digest(body));

        let actual = canonical_message(method, path, timestamp, nonce_hex, &body_hash_hex);

        // Expected = concatenation with literal '\n' separators in this
        // exact order: METHOD\npath\ntimestamp\nnonce\nbody_hash. Frozen
        // wire format /v1, must not change.
        let expected = format!(
            "POST\n\
             /v1/port-forward/request?dry=true\n\
             1700000000\n\
             0102030405060708090a0b0c0d0e0f10\n\
             {body_hash_hex}"
        );

        assert_eq!(
            actual, expected,
            "canonical_message MUST produce the exact reference vector - any change to the wire format is a /v1 breaking change"
        );
    }

    /// HEADER_* constants are the single source of truth - changing
    /// any of these strings is a /v1 breaking change because both
    /// server middleware and signer use them verbatim as HTTP header
    /// names.
    #[test]
    fn header_constants_match_v1_wire_names() {
        assert_eq!(HEADER_PUBKEY, "X-Warren-PubKey");
        assert_eq!(HEADER_SIGNATURE, "X-Warren-Sig");
        assert_eq!(HEADER_TIMESTAMP, "X-Warren-Timestamp");
        assert_eq!(HEADER_NONCE, "X-Warren-Nonce");
    }
}
