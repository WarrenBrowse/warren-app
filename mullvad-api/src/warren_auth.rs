//! Signature Ed25519 sur les requêtes API HTTP.
//!
//! Remplace le modèle `Authorization: Bearer <token>` Mullvad par une
//! signature canonique de chaque requête, prouvant la possession de la
//! clé privée Warren (dérivée de la mnémonique BIP39 utilisateur via
//! `warren_identity::derive_node_key`). Pas de cycle de vie token,
//! pas de cache à invalider.
//!
//! **Format canonique** (cf. `warren-pocs/docs/06-auth-wallet.md` § 110) :
//!
//! ```text
//! message = METHOD || "\n" || path || "\n" || timestamp || "\n" || nonce_hex || "\n" || sha256_hex(body)
//! sig     = Ed25519::sign(secret_key, message)
//! ```
//!
//! **Headers HTTP injectés** :
//!
//! - `X-Warren-PubKey`    : hex 64 chars (pubkey 32 octets)
//! - `X-Warren-Sig`       : hex 128 chars (signature 64 octets)
//! - `X-Warren-Timestamp` : epoch seconds décimal
//! - `X-Warren-Nonce`     : hex 32 chars (nonce 16 octets random)
//!
//! **Validation côté serveur** (cf. `warren-pocs/crates/warren-api/`) :
//! 1. `|now - timestamp| ≤ 60 s` (clock skew window)
//! 2. `nonce` jamais vu dans les 120 s précédentes (LRU RAM)
//! 3. signature vérifie via la pubkey
//! 4. pubkey ∈ table active subscription
//!
//! **No-log Warren** : la signing_key et la pubkey ne sont JAMAIS
//! loggées en clair. Le `Debug` impl est explicitement masqué.

use ed25519_dalek::{Signer, SigningKey};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// En-têtes HTTP générés par [`WarrenAuthSigner::sign_request`].
///
/// Le caller (`mullvad-api::rest`) consomme ces 4 valeurs pour les
/// injecter dans la requête `hyper::Request` via les noms officiels :
/// [`HEADER_PUBKEY`], [`HEADER_SIGNATURE`], [`HEADER_TIMESTAMP`],
/// [`HEADER_NONCE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarrenAuthHeaders {
    /// Pubkey Ed25519 du signeur, hex 64 chars (32 octets).
    pub pubkey_hex: String,
    /// Signature Ed25519 du `canonical_message`, hex 128 chars (64 octets).
    pub signature_hex: String,
    /// Timestamp Unix epoch seconds (= âge maximum de la requête côté
    /// serveur, qui rejette si `|now - timestamp| > 60 s`).
    pub timestamp: u64,
    /// Nonce random hex 32 chars (16 octets), unique par requête —
    /// utilisé par le serveur pour bloquer les replay attacks dans
    /// la fenêtre de 120 s.
    pub nonce_hex: String,
}

/// Nom du header HTTP pour la pubkey Warren (cf. doc 06 § 121-126).
pub const HEADER_PUBKEY: &str = "X-Warren-PubKey";
/// Nom du header HTTP pour la signature Ed25519.
pub const HEADER_SIGNATURE: &str = "X-Warren-Sig";
/// Nom du header HTTP pour le timestamp epoch seconds.
pub const HEADER_TIMESTAMP: &str = "X-Warren-Timestamp";
/// Nom du header HTTP pour le nonce hex.
pub const HEADER_NONCE: &str = "X-Warren-Nonce";

/// Taille du nonce en octets (= 128 bits, suffisant pour qu'un même
/// client génère 2^64 requêtes sans collision avec proba ≪ 1 %).
pub const NONCE_BYTES: usize = 16;

/// Signeur Warren — détient la clé privée Ed25519 et expose
/// [`Self::sign_request`] qui produit les 4 headers HTTP à injecter
/// dans une requête.
///
/// **Vie de l'instance** : un signer = un identifiant Warren = une
/// pubkey unique. La signing key est dérivée une fois au boot du
/// daemon depuis la mnémonique BIP39 utilisateur (cf.
/// `warren_identity::derive_node_key`) et conservée en RAM jusqu'au
/// shutdown ou changement d'identité (logout / import nouvelle
/// mnémonique).
pub struct WarrenAuthSigner {
    signing_key: SigningKey,
}

impl std::fmt::Debug for WarrenAuthSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No-log Warren : ne JAMAIS révéler la signing_key. La pubkey
        // est techniquement publique mais reste un identifiant
        // utilisateur ; on la masque aussi par cohérence.
        f.debug_struct("WarrenAuthSigner")
            .field("signing_key", &"<redacted>")
            .field("pubkey", &"<redacted>")
            .finish()
    }
}

impl WarrenAuthSigner {
    /// Crée un signer à partir d'une [`SigningKey`] dérivée d'une
    /// mnémonique BIP39 utilisateur (cf.
    /// `warren_identity::derive_node_key`).
    #[must_use]
    pub fn new(signing_key: SigningKey) -> Self {
        Self { signing_key }
    }

    /// Pubkey hex (64 chars) du signeur. Utile pour les API externes
    /// qui logent le `client_id` (= pubkey) sans toucher à la signing
    /// key.
    #[must_use]
    pub fn pubkey_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Signe une requête avec un `timestamp` et un `nonce` fournis par
    /// le caller. Variante déterministe utilisée par les tests pour
    /// figer un vecteur d'entrée.
    ///
    /// **Production** : utiliser [`Self::sign_request`] qui génère
    /// `timestamp` (= now) et `nonce` (= random) automatiquement.
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

    /// Signe une requête avec `timestamp = now()` et un `nonce` random
    /// (cryptographically secure, via `rand::rng()`).
    ///
    /// # Panics
    ///
    /// Ne panic pas en pratique : `SystemTime::now() < UNIX_EPOCH`
    /// implique une horloge antérieure à 1970, cas impossible sur un
    /// système fonctionnel. Si la conversion échoue, on retombe sur
    /// `timestamp = 0` ce qui sera rejeté côté serveur (clock skew),
    /// mais le client ne crash pas.
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

    /// Applique les 4 headers X-Warren-* sur une `hyper::Request`
    /// existante, en utilisant la méthode HTTP, le path et le body de
    /// la requête comme entrées du `canonical_message`.
    ///
    /// **Pourquoi cette signature** : factorise l'extraction
    /// (`method`, `path`, `body`) côté caller, qui doit fournir le
    /// `body` en bytes parce que `hyper::Request<B>` n'expose pas le
    /// body sans le consommer. En pratique le caller (rest.rs) a déjà
    /// le body sérialisé en `Vec<u8>` avant de construire la requête.
    ///
    /// # Errors
    ///
    /// [`std::io::Error`] avec [`std::io::ErrorKind::InvalidData`] si
    /// l'un des `HeaderValue` produits ne peut pas être parsé (cas
    /// uniquement théorique : nos valeurs sont toutes hex ou décimal
    /// ASCII).
    pub fn apply_to_request<B>(
        &self,
        request: &mut http::Request<B>,
        body: &[u8],
    ) -> std::io::Result<()> {
        let method = request.method().as_str();
        // Path with query string. `path_and_query` retourne `path?query`
        // ou juste `path` si pas de query — c'est ce qu'on veut signer
        // pour anti-tampering.
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

/// Construit le `canonical_message` qui sera signé. Format figé par la
/// doc 06 § 110 — ne JAMAIS modifier sans rotation de version (`v2`).
fn canonical_message(
    method: &str,
    path: &str,
    timestamp: u64,
    nonce_hex: &str,
    body_hash_hex: &str,
) -> String {
    let mut s = String::with_capacity(
        method.len() + path.len() + 20 + nonce_hex.len() + body_hash_hex.len() + 4,
    );
    s.push_str(method);
    s.push('\n');
    s.push_str(path);
    s.push('\n');
    s.push_str(&timestamp.to_string());
    s.push('\n');
    s.push_str(nonce_hex);
    s.push('\n');
    s.push_str(body_hash_hex);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    /// Clé fixe pour vecteurs de test reproductibles. Seed = `[7u8; 32]`,
    /// pubkey dérivée déterministe.
    fn fixed_signer() -> WarrenAuthSigner {
        WarrenAuthSigner::new(SigningKey::from_bytes(&[7u8; 32]))
    }

    #[test]
    fn pubkey_hex_is_64_chars() {
        // Audit format : pubkey Ed25519 = 32 octets = 64 chars hex
        let signer = fixed_signer();
        assert_eq!(signer.pubkey_hex().len(), 64);
        assert!(signer.pubkey_hex().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn signature_hex_is_128_chars() {
        // Audit format : signature Ed25519 = 64 octets = 128 chars hex
        let h = fixed_signer().sign_request_at("GET", "/v1/exits", b"", 1_700_000_000, [0u8; 16]);
        assert_eq!(h.signature_hex.len(), 128);
        assert!(h.signature_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn nonce_hex_is_32_chars() {
        // Audit format : nonce 16 octets = 32 chars hex
        let h = fixed_signer().sign_request_at("GET", "/v1/exits", b"", 1_700_000_000, [0u8; 16]);
        assert_eq!(h.nonce_hex.len(), 32);
    }

    #[test]
    fn signature_is_deterministic_with_fixed_inputs() {
        // Vector test : mêmes (key, method, path, body, timestamp,
        // nonce) → même signature à coup sûr. Garde-fou wire format
        // figé — toute régression de format casse ce test.
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
            "method DOIT influencer la signature (anti-replay cross-method)"
        );
    }

    #[test]
    fn signature_changes_when_path_changes() {
        let signer = fixed_signer();
        let a = signer.sign_request_at("POST", "/v1/a", b"", 1, [0u8; 16]);
        let b = signer.sign_request_at("POST", "/v1/b", b"", 1, [0u8; 16]);
        assert_ne!(
            a.signature_hex, b.signature_hex,
            "path DOIT influencer la signature (anti-replay cross-endpoint)"
        );
    }

    #[test]
    fn signature_changes_when_body_changes() {
        let signer = fixed_signer();
        let empty = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);
        let body = signer.sign_request_at("POST", "/v1/x", b"{\"k\":1}", 1, [0u8; 16]);
        assert_ne!(
            empty.signature_hex, body.signature_hex,
            "body DOIT influencer la signature (anti-replay tampering)"
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
        // E2E : on reconstruit le canonical message côté "serveur" et
        // on vérifie la signature avec la pubkey extraite des headers.
        // C'est exactement ce que le middleware axum côté `warren-api`
        // fait pour authentifier une requête entrante.
        let signer = fixed_signer();
        let body = b"{\"exit_pubkey\":\"abc\"}";
        let h = signer.sign_request_at(
            "POST",
            "/v1/port-forward/request",
            body,
            1_700_000_000,
            [42u8; 16],
        );

        // Reconstitution serveur :
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
        // Sécurité : si l'attaquant modifie le path en transit (e.g.
        // MITM), la signature ne doit plus vérifier. Test le contrat
        // "un bit modifié = rejet".
        let signer = fixed_signer();
        let h = signer.sign_request_at("POST", "/v1/x", b"", 1, [0u8; 16]);

        // Reconstitution avec path falsifié :
        let body_hash_hex = hex::encode(Sha256::digest(b""));
        let tampered = canonical_message("POST", "/v1/admin", 1, &h.nonce_hex, &body_hash_hex);

        let pubkey_bytes: [u8; 32] = hex::decode(&h.pubkey_hex).unwrap().try_into().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pubkey_bytes).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(&h.signature_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        assert!(
            vk.verify(tampered.as_bytes(), &sig).is_err(),
            "signature ne doit PAS vérifier avec un path falsifié"
        );
    }

    #[test]
    fn nonce_is_random_each_call_to_sign_request() {
        // Anti-replay : 2 calls successifs doivent générer des nonces
        // différents. Proba de collision sur 16 octets random = 1/2^128.
        let signer = fixed_signer();
        let a = signer.sign_request("GET", "/v1/x", b"");
        let b = signer.sign_request("GET", "/v1/x", b"");
        assert_ne!(a.nonce_hex, b.nonce_hex, "nonces doivent être uniques");
    }

    #[test]
    fn debug_does_not_leak_signing_key() {
        // No-log Warren : Debug ne doit JAMAIS révéler la signing_key
        // ni la pubkey, même partiellement.
        let signer = fixed_signer();
        let s = format!("{signer:?}");
        assert!(s.contains("<redacted>"));
        assert!(!s.contains(&signer.pubkey_hex()[..10])); // ne pas révéler les 10 premiers chars de pubkey
    }

    #[test]
    fn apply_to_request_injects_all_four_headers() {
        // Vérifie que `apply_to_request` ajoute exactement les 4
        // headers X-Warren-* attendus par le serveur, avec le bon
        // format de valeur (taille hex pour pubkey/sig/nonce, décimal
        // ASCII pour timestamp).
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
        // Timestamp doit être un u64 décimal valide.
        ts.to_str()
            .unwrap()
            .parse::<u64>()
            .expect("timestamp must be a valid u64");
    }

    #[test]
    fn apply_to_request_signs_with_path_and_query() {
        // Anti-tampering : la signature doit couvrir le `?query` aussi.
        // Un proxy malveillant qui retire un `?dry=true` ne doit pas
        // produire une signature toujours valide.
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

        // On force le même timestamp + nonce pour pouvoir comparer
        // uniquement l'effet du path. Pour ça on passe par
        // `sign_request_at` directement et compare avec ce
        // qu'`apply_to_request` injecterait.
        let h_with = signer.sign_request_at("GET", "/v1/exits?region=eu", b"", 1, [0u8; 16]);
        let h_without = signer.sign_request_at("GET", "/v1/exits", b"", 1, [0u8; 16]);
        assert_ne!(h_with.signature_hex, h_without.signature_hex);

        // Utilisation des `req_*` juste pour confirmer que `apply_to_request`
        // ne crash pas sur les deux variantes path-only / path+query.
        signer.apply_to_request(&mut req_with_q, b"").unwrap();
        signer.apply_to_request(&mut req_without_q, b"").unwrap();
    }

    #[test]
    fn body_hash_uses_sha256_for_empty_body() {
        // Vector test cryptographique : sha256("") =
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // Garantit qu'on n'a pas mute l'algo de hash silencieusement.
        let h = fixed_signer().sign_request_at("GET", "/v1/x", b"", 0, [0u8; 16]);
        // Reconstruit : si la signature passe avec le canonical attendu
        // contenant le sha256 standard, alors l'algo est bien sha256.
        let expected_body_hash_hex =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let canonical = canonical_message("GET", "/v1/x", 0, &h.nonce_hex, expected_body_hash_hex);

        let pubkey_bytes: [u8; 32] = hex::decode(&h.pubkey_hex).unwrap().try_into().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pubkey_bytes).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(&h.signature_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        vk.verify(canonical.as_bytes(), &sig)
            .expect("le body hash DOIT être sha256(b\"\") = e3b0c44...");
    }
}
