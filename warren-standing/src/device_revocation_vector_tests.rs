//! Replays the shared golden vector `vectors/device_revocation_v1.json`
//! through the device registry the Android and iOS apps share: only the
//! genuine statement forgets this install's identity and marks the
//! revocation verified, which is what lets the app forget the wallet. Every
//! other case still ends the session, keeps the identity, and stays
//! unverified.

use ed25519_dalek::SigningKey;
use serde_json::Value;
use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use warren_api::{DevicePlatform, WarrenApiClient};
use warren_identity::WarrenIdentity;

use crate::device::{DEVICE_FILE, InstallDevice};
use crate::device_registry::DeviceRegistry;

fn vector() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vectors/device_revocation_v1.json"
    );
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path} ({e}); run `git submodule update --init vectors`"));
    serde_json::from_str(&raw).expect("device_revocation_v1.json parses")
}

/// Answers every request with a 403 carrying `body`.
#[derive(Clone)]
struct Refusing(String);

impl HttpTransport for Refusing {
    async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(403, self.0.clone().into_bytes()))
    }
}

/// A registry whose install is the vector's device, and a client of the
/// vector's wallet whose every call is refused with `refusal`.
fn revoked_install(
    v: &Value,
    dir: &std::path::Path,
    refusal: &str,
) -> (DeviceRegistry, WarrenApiClient<Refusing>) {
    let kept = serde_json::json!({
        "device_id": v["device_id"]["b64url"],
        "name": "vector device",
    });
    std::fs::write(dir.join(DEVICE_FILE), kept.to_string()).expect("device file");
    let registry = DeviceRegistry::new(Some(InstallDevice::new(
        dir,
        DevicePlatform::Android,
        "1.1.55",
    )));
    let mut seed = [0u8; 32];
    hex::decode_to_slice(
        v["wallet"]["signing_seed_hex"].as_str().expect("seed"),
        &mut seed,
    )
    .expect("seed hex");
    let client = WarrenApiClient::new(
        "https://api.example.test",
        WarrenIdentity::from_signing_key(SigningKey::from_bytes(&seed)),
        Refusing(refusal.to_owned()),
    );
    (registry, client)
}

#[tokio::test]
async fn the_genuine_statement_forgets_the_identity() {
    let v = vector();
    let dir = tempfile::tempdir().expect("tempdir");
    let (registry, client) =
        revoked_install(&v, dir.path(), v["refusal_json"].as_str().expect("refusal"));
    assert_eq!(
        registry.this().map(String::from).as_deref(),
        v["device_id"]["b64url"].as_str(),
        "the install asks as the vector's device"
    );

    registry.list_json(&client).await;

    assert!(registry.revoked());
    assert!(registry.revocation_verified());
    assert!(!dir.path().join(DEVICE_FILE).exists());
}

#[tokio::test]
async fn every_refused_case_keeps_the_identity() {
    let v = vector();
    let cases = v["invalid"].as_array().expect("invalid cases");
    assert!(!cases.is_empty());
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let dir = tempfile::tempdir().expect("tempdir");
        let (registry, client) = revoked_install(
            &v,
            dir.path(),
            case["refusal_json"].as_str().expect("refusal"),
        );

        registry.list_json(&client).await;

        assert!(registry.revoked(), "{name}: the session still ends");
        assert!(!registry.revocation_verified(), "{name}");
        assert!(
            dir.path().join(DEVICE_FILE).exists(),
            "{name}: identity kept"
        );
    }
}
