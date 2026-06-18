//! Daemon-side loader for the Warren multi-hop config.
//!
//! For M4.H.B the daemon does not yet fetch signed multi-hop
//! descriptors from `warren-backend-api`; they are minted out-of-band by
//! the ops operator (`wapi admin-mint-relay-descriptor` /
//! `wapi admin-mint-exit-descriptor`) and dropped in
//! `<settings_dir>/warren-multihop.json`. The loader reads that file,
//! verifies both descriptors against the carried operational pubkey,
//! and returns a [`MultiHopConfig`] ready to be embedded in a
//! [`talpid_warren_tunnel::WarrenTunnelParameters`].
//!
//! Future revisions (M4.H.C+) will replace the file loader with a
//! periodic fetch against a yet-to-be-added `warren-backend-api`
//! endpoint. The file path stays valid as a developer-side override.

use std::path::Path;

use ed25519_dalek::VerifyingKey;
use serde::Deserialize;
use talpid_warren_tunnel::{MultiHopConfig, MultiHopExitDescriptor, MultiHopRelayDescriptor};
use warren_multihop::{
    ExitPkiError, RelayPkiError, verify_exit_descriptor, verify_relay_descriptor,
};

/// File name expected under `<settings_dir>` for the multi-hop config.
pub const MULTI_HOP_FILENAME: &str = "warren-multihop.json";

/// Errors raised by [`load_from_settings_dir`]. A missing file is
/// **not** an error: it is the common case where the user has not
/// opted into multi-hop. Only structural problems (unreadable file,
/// malformed JSON, PKI rejection) surface as `Err`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file exists but could not be read (perms, FS error).
    #[error("read {0}: {1}")]
    Io(String, #[source] std::io::Error),
    /// The file contents are not valid JSON or do not match the
    /// expected schema.
    #[error("parse {0}: {1}")]
    Json(String, #[source] serde_json::Error),
    /// The carried `operational_pubkey_hex` is not a valid Ed25519
    /// pubkey (wrong length, non-hex).
    #[error("operational_pubkey_hex is not a valid Ed25519 pubkey")]
    OperationalPubkey,
    /// The relay descriptor signature does not verify against the
    /// operational pubkey.
    #[error("relay descriptor PKI rejection: {0}")]
    RelayPki(#[from] RelayPkiError),
    /// The exit descriptor signature does not verify against the
    /// operational pubkey.
    #[error("exit descriptor PKI rejection: {0}")]
    ExitPki(#[from] ExitPkiError),
}

/// Wire schema for `<settings_dir>/warren-multihop.json`.
///
/// `operational_pubkey_hex` is the trust anchor; the two descriptors
/// embed Ed25519 signatures that this loader verifies against it
/// before yielding a [`MultiHopConfig`].
#[derive(Debug, Clone, Deserialize)]
struct MultiHopFileConfig {
    operational_pubkey_hex: String,
    #[serde(default = "default_enable_gso")]
    enable_gso: bool,
    #[serde(default = "default_use_warren_obfuscation")]
    use_warren_obfuscation: bool,
    relay: MultiHopRelayDescriptor,
    exit: MultiHopExitDescriptor,
}

fn default_enable_gso() -> bool {
    true
}

fn default_use_warren_obfuscation() -> bool {
    true
}

/// Read `<settings_dir>/warren-multihop.json`, verify both descriptor
/// signatures, and return the resulting [`MultiHopConfig`].
///
/// Returns:
/// - `Ok(None)` when the file is absent (= the user has not opted
///   into multi-hop, common case).
/// - `Ok(Some(config))` when the file is present and both descriptors
///   verify.
/// - `Err(_)` when the file exists but is unreadable, malformed, or
///   fails PKI verification. The caller should propagate this as a
///   loud error rather than silently fall back to single-hop, because
///   the user explicitly dropped a config file that is now broken.
///
/// # Errors
///
/// See [`Error`].
pub fn load_from_settings_dir(settings_dir: &Path) -> Result<Option<MultiHopConfig>, Error> {
    let path = settings_dir.join(MULTI_HOP_FILENAME);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|e| Error::Io(path.display().to_string(), e))?;
    let parsed: MultiHopFileConfig =
        serde_json::from_str(&raw).map_err(|e| Error::Json(path.display().to_string(), e))?;
    let op_bytes_vec =
        hex::decode(&parsed.operational_pubkey_hex).map_err(|_| Error::OperationalPubkey)?;
    let op_bytes: [u8; 32] = op_bytes_vec
        .try_into()
        .map_err(|_| Error::OperationalPubkey)?;
    let operational_pubkey =
        VerifyingKey::from_bytes(&op_bytes).map_err(|_| Error::OperationalPubkey)?;

    verify_relay_descriptor(&operational_pubkey, &parsed.relay)?;
    verify_exit_descriptor(&operational_pubkey, &parsed.exit)?;

    Ok(Some(MultiHopConfig {
        relay: parsed.relay,
        exit: parsed.exit,
        operational_pubkey,
        // Manual config file carries no directory geo; the daemon falls
        // back to the single-hop relay-list lookup for the location label.
        exit_country: String::new(),
        exit_city: String::new(),
        enable_gso: parsed.enable_gso,
        use_warren_obfuscation: parsed.use_warren_obfuscation,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use ed25519_dalek::{Signer, SigningKey};
    use warren_multihop::{
        ExitId, exit_descriptor_signing_payload, relay_descriptor_signing_payload,
    };

    use super::*;

    fn isolated_tempdir() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-multihop-load-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    fn well_signed_file(
        op: &SigningKey,
        with_tampered_relay_sig: bool,
        with_tampered_exit_sig: bool,
    ) -> String {
        let relay_id = [0x11; 16];
        let relay_pubkey = [0x22; 32];
        let relay_payload = relay_descriptor_signing_payload(&relay_id, &relay_pubkey);
        let mut relay_sig_bytes = op.sign(&relay_payload).to_bytes();
        if with_tampered_relay_sig {
            relay_sig_bytes[0] ^= 0xff;
        }

        let exit_id = ExitId::from_bytes([0x33; 16]);
        let exit_x25519 = [0x44; 32];
        let exit_payload = exit_descriptor_signing_payload(exit_id, &exit_x25519);
        let mut exit_sig_bytes = op.sign(&exit_payload).to_bytes();
        if with_tampered_exit_sig {
            exit_sig_bytes[0] ^= 0xff;
        }

        let op_hex = hex::encode(op.verifying_key().to_bytes());
        let relay_id_hex = hex::encode(relay_id);
        let relay_pubkey_hex = hex::encode(relay_pubkey);
        let relay_sig_hex = hex::encode(relay_sig_bytes);
        let exit_id_hex = hex::encode(exit_id.as_bytes());
        let exit_ed_hex = hex::encode([0x55; 32]);
        let exit_x_hex = hex::encode(exit_x25519);
        let exit_sig_hex = hex::encode(exit_sig_bytes);

        format!(
            r#"{{
  "operational_pubkey_hex": "{op_hex}",
  "enable_gso": false,
  "use_warren_obfuscation": false,
  "relay": {{
    "relay_id": "{relay_id_hex}",
    "relay_ed25519_pubkey": "{relay_pubkey_hex}",
    "endpoint": "198.51.100.10:443",
    "signature": "{relay_sig_hex}"
  }},
  "exit": {{
    "exit_id": "{exit_id_hex}",
    "exit_ed25519_pubkey": "{exit_ed_hex}",
    "exit_x25519_multihop_pubkey": "{exit_x_hex}",
    "endpoint": "198.51.100.20:443",
    "signature": "{exit_sig_hex}"
  }}
}}"#
        )
    }

    #[test]
    fn load_returns_none_when_file_absent() {
        // Common case: the user has not opted into multi-hop. The
        // loader must NOT error and the daemon falls back to
        // single-hop transparently.
        let dir = isolated_tempdir();
        let cfg = load_from_settings_dir(&dir).expect("must not error on missing file");
        assert!(cfg.is_none(), "missing file must yield Ok(None)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_some_for_well_signed_descriptors() {
        // Happy path: a valid signed config returns a MultiHopConfig
        // with the operational pubkey wired in.
        let dir = isolated_tempdir();
        let op = SigningKey::from_bytes(&[0xaa; 32]);
        let json = well_signed_file(&op, false, false);
        std::fs::write(dir.join(MULTI_HOP_FILENAME), &json).expect("write");

        let cfg = load_from_settings_dir(&dir)
            .expect("well-signed config must verify")
            .expect("config must be Some");
        assert_eq!(
            cfg.operational_pubkey.to_bytes(),
            op.verifying_key().to_bytes()
        );
        assert!(!cfg.enable_gso, "enable_gso must round-trip from file");
        assert!(
            !cfg.use_warren_obfuscation,
            "use_warren_obfuscation must round-trip from file"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_tampered_relay_signature() {
        // Tampering one byte of the relay signature must surface as
        // a typed PKI error rather than a silent acceptance. A silent
        // accept here would let a hostile mint be loaded as if it
        // were genuine.
        let dir = isolated_tempdir();
        let op = SigningKey::from_bytes(&[0xbb; 32]);
        let json = well_signed_file(&op, true, false);
        std::fs::write(dir.join(MULTI_HOP_FILENAME), &json).expect("write");

        let err = load_from_settings_dir(&dir).expect_err("tampered relay sig must reject");
        assert!(matches!(err, Error::RelayPki(_)), "got {err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_tampered_exit_signature() {
        // Same as above but on the exit descriptor.
        let dir = isolated_tempdir();
        let op = SigningKey::from_bytes(&[0xcc; 32]);
        let json = well_signed_file(&op, false, true);
        std::fs::write(dir.join(MULTI_HOP_FILENAME), &json).expect("write");

        let err = load_from_settings_dir(&dir).expect_err("tampered exit sig must reject");
        assert!(matches!(err, Error::ExitPki(_)), "got {err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_malformed_operational_pubkey() {
        // A non-hex or wrong-length operational pubkey must produce a
        // typed `OperationalPubkey` error before any signature
        // verification runs.
        let dir = isolated_tempdir();
        let json = r#"{
  "operational_pubkey_hex": "not-hex",
  "relay": {"relay_id": "00", "relay_ed25519_pubkey": "00", "endpoint": "1.2.3.4:443", "signature": "00"},
  "exit": {"exit_id": "00", "exit_ed25519_pubkey": "00", "exit_x25519_multihop_pubkey": "00", "endpoint": "1.2.3.4:443", "signature": "00"}
}"#;
        std::fs::write(dir.join(MULTI_HOP_FILENAME), json).expect("write");

        let err = load_from_settings_dir(&dir).expect_err("non-hex op pubkey must reject");
        // It can surface either as OperationalPubkey (if hex parse
        // succeeds but length is wrong) or as Json (if the relay/exit
        // sub-fields fail length parse first). Either way it must NOT
        // be a silent Ok.
        assert!(matches!(err, Error::OperationalPubkey | Error::Json(_, _)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_corrupt_json() {
        // Garbage in the file must produce a typed Json error rather
        // than a panic.
        let dir = isolated_tempdir();
        std::fs::write(dir.join(MULTI_HOP_FILENAME), "not json").expect("write");

        let err = load_from_settings_dir(&dir).expect_err("garbage must reject");
        assert!(matches!(err, Error::Json(_, _)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn defaults_apply_when_optional_fields_omitted() {
        // Omitting `enable_gso` / `use_warren_obfuscation` must yield
        // production defaults (both `true`). The TOML schema for
        // ops-minted files routinely omits these knobs.
        let dir = isolated_tempdir();
        let op = SigningKey::from_bytes(&[0xdd; 32]);
        let relay_id = [0x11; 16];
        let relay_pubkey = [0x22; 32];
        let relay_payload = relay_descriptor_signing_payload(&relay_id, &relay_pubkey);
        let relay_sig = op.sign(&relay_payload).to_bytes();
        let exit_id = ExitId::from_bytes([0x33; 16]);
        let exit_x25519 = [0x44; 32];
        let exit_payload = exit_descriptor_signing_payload(exit_id, &exit_x25519);
        let exit_sig = op.sign(&exit_payload).to_bytes();

        let json = format!(
            r#"{{
  "operational_pubkey_hex": "{}",
  "relay": {{
    "relay_id": "{}",
    "relay_ed25519_pubkey": "{}",
    "endpoint": "198.51.100.10:443",
    "signature": "{}"
  }},
  "exit": {{
    "exit_id": "{}",
    "exit_ed25519_pubkey": "{}",
    "exit_x25519_multihop_pubkey": "{}",
    "endpoint": "198.51.100.20:443",
    "signature": "{}"
  }}
}}"#,
            hex::encode(op.verifying_key().to_bytes()),
            hex::encode(relay_id),
            hex::encode(relay_pubkey),
            hex::encode(relay_sig),
            hex::encode(exit_id.as_bytes()),
            hex::encode([0x55; 32]),
            hex::encode(exit_x25519),
            hex::encode(exit_sig),
        );
        std::fs::write(dir.join(MULTI_HOP_FILENAME), &json).expect("write");

        let cfg = load_from_settings_dir(&dir)
            .expect("must succeed")
            .expect("must be Some");
        assert!(cfg.enable_gso, "default must be true");
        assert!(cfg.use_warren_obfuscation, "default must be true");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
