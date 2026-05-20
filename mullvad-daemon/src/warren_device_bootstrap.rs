//! Bootstrap of a local `device.json` from the Warren artifacts
//! (BIP39 mnemonic + derived Ed25519 signing key).
//!
//! Invoked at daemon boot in [`crate::warren_account_mode`] mode to
//! avoid the network call `create_device -> api.mullvad.net` when
//! `AccountsProxy` has not yet migrated to `warren-api`. The
//! generated device is a **POC identity**: it is not recognized by
//! the standard Mullvad infra, and only works with a custom Warren exit.
//!
//! Strict idempotence policy:
//! - If a `device.json` `LoggedIn` already exists AND its pubkey matches
//!   the current signing key: we **touch nothing** ([`BootstrapOutcome::AlreadyConsistent`]).
//! - If a `device.json` exists but its pubkey differs: we **do not
//!   overwrite** ([`BootstrapOutcome::SkippedMismatch`]) — could mask a
//!   deliberate logout or an unrecognized identity change.
//! - If a `device.json` `LoggedOut`/`Revoked` exists: we **do not overwrite
//!   either** ([`BootstrapOutcome::SkippedExisting`]) — let the user
//!   go through an explicit `mullvad account login`.
//!
//! This no-overwrite policy prevents silent identity loss.

use std::path::Path;

use chrono::Utc;
use ed25519_dalek::SigningKey;
use mullvad_types::warren_pubkey::WarrenPubKey;
use mullvad_types::wireguard::{AssociatedAddresses, WireguardData};
use talpid_types::net::wireguard::PrivateKey as WgPrivateKey;

use crate::device::{
    DEVICE_CACHE_FILENAME, PrivateAccountAndDevice, PrivateDevice, PrivateDeviceState,
};

/// Possible outcome of [`ensure_local_device`].
#[derive(Debug, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// Device was created and written (`device.json` was absent).
    Created,
    /// Existing device already matches the signing key — nothing done.
    AlreadyConsistent,
    /// Existing device has a different pubkey — not overwritten to
    /// preserve identity.
    SkippedMismatch,
    /// Existing device in `LoggedOut` or `Revoked` state — not
    /// overwritten to preserve an explicit logout/revocation.
    SkippedExisting,
}

/// Bootstrap errors.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// I/O error during the atomic write.
    #[error("io error at {0}: {1}")]
    Io(String, #[source] std::io::Error),
    /// JSON serialization error.
    #[error("failed to serialize device state: {0}")]
    Serialize(#[source] serde_json::Error),
}

/// Creates a `device.json` consistent with the Warren `signing_key` if
/// no device exists yet in `settings_dir`.
///
/// The caller (daemon boot) must have previously loaded the signing
/// key via [`crate::warren_signer::load_or_create_signing_key`] and
/// checked [`crate::warren_account_mode::is_enabled`].
///
/// # Errors
///
/// [`BootstrapError::Io`] if the atomic write of `device.json` fails.
/// [`BootstrapError::Serialize`] if JSON serialization fails (should
/// never happen with the `serde::Serialize`-derived types used here).
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
                "Warren local account: device.json pubkey ({}) != mnemonic-derived pubkey ({}); not overwriting",
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

/// Derives an `AssociatedAddresses` from the pubkey bytes to
/// avoid trivial IP collisions between multiple POC clients
/// sharing the same exit. Ranges:
/// - IPv4 in `10.64.0.0/10` (Mullvad-compatible — the range used by
///   the standard API in production).
/// - IPv6 in `fc00:bbbb::/32` (arbitrary POC ULA).
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

    /// Tempdir isolated per test — pid + nanos + atomic counter to
    /// avoid collisions between parallel tests.
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
        // Critical regression: the device.json created by bootstrap
        // MUST have a pubkey equal to the Ed25519 pubkey of the
        // current signing_key. Otherwise the Warren tunnel cannot
        // sign the exit auth handshake (pubkey mismatch).
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
                    "device.json pubkey must derive from the signing_key"
                );
                assert!(
                    account.device.id.starts_with("warren-local-"),
                    "device id must identify the POC bootstrap"
                );
            }
            other => panic!("expected LoggedIn, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_local_device_is_idempotent_and_preserves_wireguard_private_key() {
        // Critical regression: a daemon re-boot MUST NOT
        // regenerate a new wg private_key, otherwise (a) the exit
        // would lose the established session, (b) all Wireguard
        // key rotations would be invalidated. Idempotence is
        // measured on the cryptographic material, not on a simple
        // "device.json exists".
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
            "wg private_key MUST survive a daemon re-boot"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_local_device_does_not_overwrite_logged_in_with_different_pubkey() {
        // Critical regression: if the user already has a valid
        // device.json for another identity (= mnemonic change
        // or logout-relogin with a different source), we MUST NOT
        // overwrite silently. Otherwise user identity is lost.
        let dir = isolated_tempdir();

        // Setup: bootstrap with a first signing_key.
        let sk_initial = fixed_signing_key(1);
        ensure_local_device(&dir, &sk_initial).expect("initial bootstrap");
        let initial_state_raw =
            std::fs::read_to_string(dir.join(DEVICE_CACHE_FILENAME)).expect("read initial");

        // Act: re-bootstrap with a different signing_key.
        let sk_other = fixed_signing_key(2);
        let outcome = ensure_local_device(&dir, &sk_other).expect("second bootstrap");

        // Assert: (a) outcome = SkippedMismatch, (b) the file was
        // not modified byte-for-byte.
        assert_eq!(outcome, BootstrapOutcome::SkippedMismatch);
        let after_state_raw =
            std::fs::read_to_string(dir.join(DEVICE_CACHE_FILENAME)).expect("read after");
        assert_eq!(
            initial_state_raw, after_state_raw,
            "device.json MUST remain intact on pubkey mismatch"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
