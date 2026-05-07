//! Bootstrap d'un `device.json` local à partir des artefacts Warren
//! (mnémonique BIP39 + signing key Ed25519 dérivée).
//!
//! Invoqué au boot du daemon en mode [`crate::warren_account_mode`] pour
//! éviter l'appel réseau `create_device → api.mullvad.net` quand
//! `AccountsProxy` n'a pas encore migré vers `warren-api`. Le device
//! généré est une **identité POC** : il n'est pas reconnu par l'infra
//! Mullvad standard, ne fonctionne qu'avec un exit Warren custom.
//!
//! Politique d'idempotence stricte :
//! - Si un `device.json` `LoggedIn` existe déjà ET sa pubkey matche la
//!   signing key courante : on **ne touche rien** ([`BootstrapOutcome::AlreadyConsistent`]).
//! - Si un `device.json` existe mais sa pubkey diffère : on **n'écrase
//!   pas** ([`BootstrapOutcome::SkippedMismatch`]) — pourrait masquer une
//!   logout volontaire ou un changement d'identité non-reconnu.
//! - Si un `device.json` `LoggedOut`/`Revoked` existe : on **n'écrase pas
//!   non plus** ([`BootstrapOutcome::SkippedExisting`]) — laisse le user
//!   passer par `mullvad account login` explicite.
//!
//! Cette politique no-overwrite empêche la perte silencieuse d'identité.

use std::path::Path;

use chrono::Utc;
use ed25519_dalek::SigningKey;
use mullvad_types::warren_pubkey::WarrenPubKey;
use mullvad_types::wireguard::{AssociatedAddresses, WireguardData};
use talpid_types::net::wireguard::PrivateKey as WgPrivateKey;

use crate::device::{
    DEVICE_CACHE_FILENAME, PrivateAccountAndDevice, PrivateDevice, PrivateDeviceState,
};

/// Outcome possible de [`ensure_local_device`].
#[derive(Debug, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// Device a été créé et écrit (`device.json` était absent).
    Created,
    /// Device existant matche déjà la signing key — rien fait.
    AlreadyConsistent,
    /// Device existant a une pubkey différente — non-écrasé pour
    /// préserver l'identité.
    SkippedMismatch,
    /// Device existant en état `LoggedOut` ou `Revoked` — non-écrasé
    /// pour préserver une logout/revocation explicite.
    SkippedExisting,
}

/// Erreurs du bootstrap.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// Erreur I/O lors de l'écriture atomique.
    #[error("io error at {0}: {1}")]
    Io(String, #[source] std::io::Error),
    /// Erreur de sérialisation JSON.
    #[error("failed to serialize device state: {0}")]
    Serialize(#[source] serde_json::Error),
}

/// Crée un `device.json` cohérent avec la `signing_key` Warren si aucun
/// device n'existe encore dans `settings_dir`.
///
/// Le caller (boot du daemon) doit avoir au préalable chargé la signing
/// key via [`crate::warren_signer::load_or_create_signing_key`] et
/// vérifié [`crate::warren_account_mode::is_enabled`].
///
/// # Errors
///
/// [`BootstrapError::Io`] si le write atomique du `device.json` échoue.
/// [`BootstrapError::Serialize`] si la sérialisation JSON échoue (ne
/// devrait jamais arriver avec les types `serde::Serialize`-derived
/// utilisés ici).
pub fn ensure_local_device(
    settings_dir: &Path,
    signing_key: &SigningKey,
) -> Result<BootstrapOutcome, BootstrapError> {
    let pubkey = warren_pubkey_from_signing_key(signing_key);
    let device_path = settings_dir.join(DEVICE_CACHE_FILENAME);

    if let Some(existing) = read_existing_state(&device_path) {
        return Ok(classify_existing(&existing, &pubkey));
    }

    let state = PrivateDeviceState::LoggedIn(PrivateAccountAndDevice {
        pubkey: pubkey.clone(),
        device: generate_local_device(&pubkey),
    });

    write_atomic(&device_path, &state)?;
    log::info!(
        "Warren local account: bootstrapped device.json at {} (pubkey {})",
        device_path.display(),
        pubkey
    );
    Ok(BootstrapOutcome::Created)
}

fn warren_pubkey_from_signing_key(sk: &SigningKey) -> WarrenPubKey {
    WarrenPubKey::from_bytes(sk.verifying_key().as_bytes())
}

fn read_existing_state(path: &Path) -> Option<PrivateDeviceState> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn classify_existing(
    existing: &PrivateDeviceState,
    expected_pubkey: &WarrenPubKey,
) -> BootstrapOutcome {
    match existing {
        PrivateDeviceState::LoggedIn(account) if account.pubkey == *expected_pubkey => {
            BootstrapOutcome::AlreadyConsistent
        }
        PrivateDeviceState::LoggedIn(account) => {
            log::warn!(
                "Warren local account: device.json pubkey ({}) ≠ mnemonic-derived pubkey ({}); not overwriting",
                account.pubkey,
                expected_pubkey
            );
            BootstrapOutcome::SkippedMismatch
        }
        PrivateDeviceState::LoggedOut | PrivateDeviceState::Revoked => {
            BootstrapOutcome::SkippedExisting
        }
    }
}

fn generate_local_device(pubkey: &WarrenPubKey) -> PrivateDevice {
    let bytes = pubkey
        .to_bytes()
        .expect("WarrenPubKey constructed from 32 valid bytes is always decodable");
    let pubkey_hex = pubkey.as_str();
    let now = Utc::now();
    PrivateDevice {
        id: format!("warren-local-{}", &pubkey_hex[..12]),
        name: format!("warren-local-{}", &pubkey_hex[..6]),
        wg_data: WireguardData {
            private_key: WgPrivateKey::new_from_random(),
            addresses: derive_addresses(&bytes),
            created: now,
        },
        hijack_dns: false,
        created: now,
    }
}

/// Dérive un `AssociatedAddresses` à partir des octets de la pubkey
/// pour éviter les collisions IP triviales entre plusieurs clients
/// POC partageant un même exit. Plages :
/// - IPv4 dans `10.64.0.0/10` (compat Mullvad — la plage qu'utilise
///   l'API standard en production).
/// - IPv6 dans `fc00:bbbb::/32` (ULA arbitraire POC).
fn derive_addresses(bytes: &[u8; 32]) -> AssociatedAddresses {
    let v4_str = format!("10.64.{}.{}/32", bytes[0], bytes[1].max(1));
    let v6_str = format!(
        "fc00:bbbb::{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}/128",
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9]
    );
    AssociatedAddresses {
        ipv4_address: v4_str
            .parse()
            .expect("hardcoded 10.64.X.Y/32 is always a valid Ipv4Network"),
        ipv6_address: v6_str
            .parse()
            .expect("hardcoded fc00:bbbb::.../128 is always a valid Ipv6Network"),
    }
}

fn write_atomic(path: &Path, state: &PrivateDeviceState) -> Result<(), BootstrapError> {
    let json = serde_json::to_vec_pretty(state).map_err(BootstrapError::Serialize)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| BootstrapError::Io(tmp.display().to_string(), e))?;
    std::fs::rename(&tmp, path).map_err(|e| BootstrapError::Io(path.display().to_string(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tempdir isolé par test — pid + nanos + counter atomique pour
    /// éviter les collisions entre tests parallèles.
    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-device-bootstrap-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    fn fixed_signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn read_state(path: &std::path::Path) -> PrivateDeviceState {
        let raw = std::fs::read_to_string(path).expect("device.json exists");
        serde_json::from_str(&raw).expect("device.json is valid")
    }

    #[test]
    fn ensure_local_device_creates_logged_in_with_pubkey_derived_from_signing_key() {
        // Régression critique : le device.json créé par bootstrap
        // DOIT avoir une pubkey égale à la pubkey Ed25519 de la
        // signing_key courante. Sinon le tunnel Warren ne pourra
        // pas signer l'auth handshake exit (pubkey mismatch).
        let dir = isolated_tempdir();
        let sk = fixed_signing_key(7);
        let expected_pubkey = WarrenPubKey::from_bytes(sk.verifying_key().as_bytes());

        let outcome = ensure_local_device(&dir, &sk).expect("first bootstrap");

        assert_eq!(outcome, BootstrapOutcome::Created);
        let state = read_state(&dir.join(DEVICE_CACHE_FILENAME));
        match state {
            PrivateDeviceState::LoggedIn(account) => {
                assert_eq!(
                    account.pubkey, expected_pubkey,
                    "device.json pubkey doit dériver de la signing_key"
                );
                assert!(
                    account.device.id.starts_with("warren-local-"),
                    "device id doit identifier le bootstrap POC"
                );
            }
            other => panic!("expected LoggedIn, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_local_device_is_idempotent_and_preserves_wireguard_private_key() {
        // Régression critique : un re-boot du daemon NE DOIT PAS
        // re-générer une nouvelle wg private_key, sinon (a) l'exit
        // perdrait la session établie, (b) toutes les rotations de
        // clés Wireguard seraient invalidées. L'idempotence se
        // mesure sur le matériel cryptographique, pas sur un simple
        // "device.json existe".
        let dir = isolated_tempdir();
        let sk = fixed_signing_key(11);

        ensure_local_device(&dir, &sk).expect("first bootstrap");
        let pk_first = match read_state(&dir.join(DEVICE_CACHE_FILENAME)) {
            PrivateDeviceState::LoggedIn(a) => a.device.wg_data.private_key.to_bytes(),
            _ => panic!("expected LoggedIn after first bootstrap"),
        };

        let outcome = ensure_local_device(&dir, &sk).expect("second bootstrap");
        assert_eq!(outcome, BootstrapOutcome::AlreadyConsistent);

        let pk_second = match read_state(&dir.join(DEVICE_CACHE_FILENAME)) {
            PrivateDeviceState::LoggedIn(a) => a.device.wg_data.private_key.to_bytes(),
            _ => panic!("expected LoggedIn after second bootstrap"),
        };
        assert_eq!(
            pk_first, pk_second,
            "wg private_key DOIT survivre au re-boot du daemon"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_local_device_does_not_overwrite_logged_in_with_different_pubkey() {
        // Régression critique : si le user a déjà un device.json
        // valide pour une autre identité (= changement de mnémonique
        // ou logout-relogin avec une autre source), on NE DOIT PAS
        // écraser silencieusement. Sinon perte d'identité utilisateur.
        let dir = isolated_tempdir();

        // Setup : bootstrap avec une première signing_key.
        let sk_initial = fixed_signing_key(1);
        ensure_local_device(&dir, &sk_initial).expect("initial bootstrap");
        let initial_state_raw =
            std::fs::read_to_string(dir.join(DEVICE_CACHE_FILENAME)).expect("read initial");

        // Act : re-bootstrap avec une signing_key différente.
        let sk_other = fixed_signing_key(2);
        let outcome = ensure_local_device(&dir, &sk_other).expect("second bootstrap");

        // Assert : (a) outcome = SkippedMismatch, (b) le fichier n'a
        // pas été modifié byte-pour-byte.
        assert_eq!(outcome, BootstrapOutcome::SkippedMismatch);
        let after_state_raw =
            std::fs::read_to_string(dir.join(DEVICE_CACHE_FILENAME)).expect("read after");
        assert_eq!(
            initial_state_raw, after_state_raw,
            "device.json DOIT rester intact en cas de mismatch pubkey"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
