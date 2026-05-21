//! Daemon-side wrapper around the
//! [`warren_relay_selector::WarrenRelaySelector`] crate.
//!
//! Encapsulates the state of the `WarrenRelayList` on the daemon side
//! (will later be populated by a periodic fetch to the API; for the
//! POC, loaded from `<cache_dir>/warren-relays.json`), and exposes a
//! stable API for the `ParametersGenerator`. The wrapper returns only
//! the Iroh components (`EndpointId` + `EndpointAddr`); final
//! assembly into `WarrenTunnelParameters` (with `signing_key`,
//! `n_connections`, `features`) is done by
//! [`crate::warren_tunnel_params::assemble_for_attempt`].
//!
//! Dedicated module for two reasons: testable in isolation, and does
//! not import `talpid-warren-tunnel` in the wrapper's public API.

use std::path::Path;
use std::sync::Arc;

use warren_relay_selector::warren_types::{ExitId, WarrenExitAddr, WarrenPubkey};
use warren_relay_selector::{
    SelectorError, SignedError, WarrenRelay, WarrenRelayList, WarrenRelayQuery,
    WarrenRelaySelector, verify_signed_relay_list,
};

/// Errors when loading `warren-relays.json` at boot.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// I/O on the `warren-relays.json` file (path present but
    /// unreadable).
    #[error("failed to read warren relays at {0}: {1}")]
    Io(String, #[source] std::io::Error),

    /// The JSON is invalid, does not have the supported version, or
    /// the server signature does not verify. Wire format v2 mandatory
    /// post-F3 fork audit (see `warren_relay_selector::signed`).
    #[error("invalid warren-relays.json at {0}: {1}")]
    Json(String, #[source] SignedError),
}

/// Minimal selection output: the two fields needed to build a
/// `WarrenTunnelParameters` on the caller side. Cloneable so the
/// caller can keep a copy before producing the tunnel parameters.
///
/// Note: post-Quinn migration, `WarrenExitAddr.id` carries the same
/// Ed25519 pubkey as `endpoint_id`. The pair is kept here to preserve
/// the caller's accessor pattern; long-term the duplicate can be
/// dropped once consumers read from `endpoint_addr.id` directly.
#[derive(Debug, Clone)]
pub struct WarrenSelection {
    /// Ed25519 identity of the selected Warren exit.
    pub endpoint_id: WarrenPubkey,

    /// Operator-assigned 16-byte stable identifier for this exit
    /// (signed v3 relay-list field). The Session A.4 TOFU pubkey
    /// pinning verify hook keys its lookup on this value so a
    /// legitimate Ed25519 rotation stays detectable across reconnects.
    pub exit_id: ExitId,

    /// Candidate addresses of the exit (UDP IPv4/IPv6).
    pub endpoint_addr: WarrenExitAddr,

    /// Session H.6: forensic snapshot threaded through to the TOFU
    /// pin so the renderer modal + `/v1/incidents/pubkey-mismatch`
    /// report carry the user-readable location, not just the pubkey
    /// fingerprint. ISO 3166 alpha-2 code, lower case (matches the
    /// signed relay-list `Location::country_code`).
    pub country_code: String,
    /// Session H.6: free-form city label captured at selection time.
    pub city: String,
}

impl From<&WarrenRelay> for WarrenSelection {
    fn from(relay: &WarrenRelay) -> Self {
        Self {
            endpoint_id: relay.endpoint_id(),
            exit_id: relay.exit_id(),
            endpoint_addr: relay.endpoint_addr().clone(),
            country_code: relay.location().country_code().to_owned(),
            city: relay.location().city().to_owned(),
        }
    }
}

/// Daemon-side wrapper around `WarrenRelaySelector`.
///
/// Holds an `Arc<WarrenRelaySelector>` to allow thread-safe sharing
/// between the tunnel state machine and the gRPC management
/// interface (future).
#[derive(Debug, Clone)]
pub struct DaemonWarrenRelaySelector {
    inner: Arc<WarrenRelaySelector>,
    /// Raw list kept separately to allow the caller (daemon boot)
    /// to convert it to Mullvad-format `RelayList` and
    /// broadcast it to the Electron GUI via `notify_relay_list`. See
    /// `warren_relay_list_view::to_mullvad_relay_list`.
    list: Arc<WarrenRelayList>,
}

/// Name of the file containing the locally bootstrapped
/// `WarrenRelayList`. Fixed convention: a future move imposes a
/// cache migration. To be replaced by a periodic fetch to
/// `mullvad-api` once the Warren endpoint is available.
pub const WARREN_RELAYS_FILENAME: &str = "warren-relays.json";

impl DaemonWarrenRelaySelector {
    /// Builds a wrapper from a [`WarrenRelayList`].
    #[must_use]
    pub fn new(relays: WarrenRelayList) -> Self {
        let list = Arc::new(relays.clone());
        Self {
            inner: Arc::new(WarrenRelaySelector::new(relays)),
            list,
        }
    }

    /// Read access to the raw `WarrenRelayList` (= what was
    /// passed to [`Self::new`] or loaded from the cache). Used by
    /// the daemon boot to broadcast the list to the GUI via
    /// [`crate::warren_relay_list_view`].
    #[must_use]
    pub fn list(&self) -> &WarrenRelayList {
        &self.list
    }

    /// Read access to the inner [`WarrenRelaySelector`]. Exposed so
    /// callers (M5.B.2 failover) can invoke selector methods that the
    /// wrapper does not directly mirror (e.g.
    /// `select_failover_alternative`).
    #[must_use]
    pub fn inner(&self) -> &WarrenRelaySelector {
        &self.inner
    }

    /// Returns the relay whose `endpoint_id` matches `pubkey`, or
    /// `None` if the list has no such entry. M5.B.2 failover uses this
    /// to resolve the "previously failed exit's pubkey" back into the
    /// full [`WarrenRelay`] needed by
    /// [`WarrenRelaySelector::select_failover_alternative`].
    #[must_use]
    pub fn relay_by_pubkey(&self, pubkey: &WarrenPubkey) -> Option<&WarrenRelay> {
        self.list
            .relays()
            .iter()
            .find(|r| r.endpoint_id() == *pubkey)
    }

    /// Loads the `WarrenRelayList` from `<cache_dir>/warren-relays.json`.
    ///
    /// No-fail policy at boot: if the file is absent or
    /// unreadable, returns a wrapper with an empty list + log warn,
    /// to allow the daemon to keep booting in WG mode. The
    /// state machine will see an empty `WarrenRelayList` and return
    /// `NoRelayMatch` on the first selection — expected behavior:
    /// the user is not in Warren mode.
    ///
    /// # Errors
    ///
    /// Returns an error only if the file exists but
    /// contains invalid JSON (= silent corruption to signal
    /// explicitly). The caller (daemon boot) may choose to
    /// fall back to an empty list via `unwrap_or_else`.
    pub fn load_from_cache_dir(cache_dir: &Path) -> Result<Self, LoadError> {
        Self::load_from_cache_dir_with_pin(cache_dir, None)
    }

    /// Variant of [`Self::load_from_cache_dir`] with **server pubkey
    /// pinning**. If `expected_server_pubkey_hex` is `Some(hex)`,
    /// rejects any list signed by a different pubkey (=
    /// MITM-on-bootstrap protection). If `None`, TOFU mode: accepts any
    /// self-consistent signature (useful for the first fetch or tests).
    ///
    /// Expected format: v2 signed Ed25519 (see
    /// [`warren_relay_selector::verify_signed_relay_list`]). The
    /// unsigned v1 format is **rejected** post-F3 audit (anti-downgrade
    /// attack — an attacker serving an unsigned v1 could
    /// substitute the list without detection).
    ///
    /// # Errors
    ///
    /// - [`LoadError::Io`] if the file exists but is unreadable.
    /// - [`LoadError::Json`] if the JSON is invalid, version != 2,
    ///   the server pubkey differs from the pin, or the signature
    ///   does not verify.
    pub fn load_from_cache_dir_with_pin(
        cache_dir: &Path,
        expected_server_pubkey_hex: Option<&str>,
    ) -> Result<Self, LoadError> {
        let path = cache_dir.join(WARREN_RELAYS_FILENAME);
        if !path.exists() {
            log::info!(
                "Warren relays file not found at {} — booting with empty relay list",
                path.display()
            );
            return Ok(Self::new(WarrenRelayList::default()));
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| LoadError::Io(path.display().to_string(), e))?;
        let list = verify_signed_relay_list(&raw, expected_server_pubkey_hex)
            .map_err(|e| LoadError::Json(path.display().to_string(), e))?;
        log::info!(
            "Loaded {} Warren relays from {} (signature verified)",
            list.len(),
            path.display()
        );
        Ok(Self::new(list))
    }

    /// Selects a relay for the `retry_attempt` attempt and
    /// returns its Iroh components.
    ///
    /// Mirror API of
    /// [`mullvad_relay_selector::RelaySelector::get_relay`] on the
    /// WireGuard side — eases dispatch via
    /// `ParametersGenerator::generate(retry_attempt, ...)`.
    ///
    /// # Errors
    ///
    /// Returns [`SelectorError::NoRelayMatch`] if no active relay
    /// with `weight > 0` satisfies the constraints.
    pub fn select_for_attempt(
        &self,
        query: &WarrenRelayQuery,
        retry_attempt: u32,
    ) -> Result<WarrenSelection, SelectorError> {
        self.inner
            .select_for_attempt(query, retry_attempt)
            .map(WarrenSelection::from)
    }
}

#[cfg(test)]
mod tests {
    use warren_relay_selector::{Location, LocationConstraint, WarrenRelay};

    use super::*;

    fn endpoint_id(seed: u8) -> WarrenPubkey {
        WarrenPubkey::from_bytes([seed; 32])
    }

    fn relay(seed: u8, country: &str, addr_str: &str) -> WarrenRelay {
        let id = endpoint_id(seed);
        let addr = WarrenExitAddr::new(id).with_ip_addr(addr_str.parse().unwrap());
        WarrenRelay::new(
            id,
            ExitId::from_bytes([seed; 16]),
            addr,
            Location::new(country, "_"),
            100,
            true,
        )
    }

    #[test]
    fn daemon_selector_returns_warren_components_for_unconstrained_query() {
        // The wrapper must delegate to the upstream crate and return a
        // `WarrenSelection` with the two fields needed downstream by
        // `WarrenTunnelParameters`.
        let list = WarrenRelayList::new(vec![relay(1, "se", "198.51.100.1:51820")]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let selection = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .expect("must select the only available relay");

        assert_eq!(selection.endpoint_id, endpoint_id(1));
        assert!(
            selection
                .endpoint_addr
                .ip_addrs()
                .any(|s| s.to_string() == "198.51.100.1:51820"),
            "endpoint_addr must contain the source IP"
        );
    }

    #[test]
    fn daemon_selector_propagates_location_constraint() {
        // The wrapper must honor the query's geo constraint. Asking
        // for FR must never return SE.
        let list = WarrenRelayList::new(vec![
            relay(1, "se", "198.51.100.1:51820"),
            relay(2, "fr", "198.51.100.2:51820"),
        ]);
        let selector = DaemonWarrenRelaySelector::new(list);

        let query = WarrenRelayQuery::any().with_location(LocationConstraint::Country("fr".into()));
        for attempt in 0..10 {
            let selection = selector
                .select_for_attempt(&query, attempt)
                .expect("must select FR relay");
            assert_eq!(
                selection.endpoint_id,
                endpoint_id(2),
                "attempt {attempt} must always return the FR relay"
            );
        }
    }

    #[test]
    fn daemon_selector_returns_error_when_no_match() {
        // With an empty list, the upstream error must propagate
        // verbatim (no silent remap).
        let selector = DaemonWarrenRelaySelector::new(WarrenRelayList::new(vec![]));
        assert!(matches!(
            selector.select_for_attempt(&WarrenRelayQuery::any(), 0),
            Err(SelectorError::NoRelayMatch)
        ));
    }

    #[test]
    fn load_from_cache_dir_returns_empty_list_when_file_absent() {
        // On first boot, the file does not exist -> wrapper with
        // empty list, no error. Allows the daemon to start
        // without necessarily having a Warren RelayList.
        let dir = isolated_tempdir();
        let selector = DaemonWarrenRelaySelector::load_from_cache_dir(&dir)
            .expect("must succeed without file");
        assert!(matches!(
            selector.select_for_attempt(&WarrenRelayQuery::any(), 0),
            Err(SelectorError::NoRelayMatch)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_cache_dir_parses_v2_signed_json_emitted_by_warren_api() {
        // F3 fork audit: warren-api `/v1/exits` returns a
        // **signed v2** format (`SignedRelayList` with server_pubkey + Ed25519
        // signature). The daemon must parse it and verify the signature
        // — not accept unsigned v1. Frozen format: if serde changes
        // the order of v2 fields, this test (and any existing
        // installation) breaks -> `/v3` rotation mandatory.
        use ed25519_dalek::SigningKey;
        use warren_relay_selector::{JsonRelay as SignedJsonRelay, sign_relay_list};

        let dir = isolated_tempdir();

        // Fixed server signing key for the test (deterministic).
        let server_key = SigningKey::from_bytes(&[0xab; 32]);
        let relay_pubkey = WarrenPubkey::from_bytes([5u8; 32]);
        let relay_pubkey_hex = hex::encode(relay_pubkey.as_bytes());

        let signed = sign_relay_list(
            vec![SignedJsonRelay {
                endpoint_id: relay_pubkey_hex,
                exit_id: ExitId::from_bytes([0xe1; 16]),
                ip_addrs: vec!["198.51.100.1:51820".to_owned()],
                country: "se".to_owned(),
                city: "Stockholm".to_owned(),
                weight: 100,
                active: true,
            }],
            &server_key,
            1_700_000_000,
        );
        let json = serde_json::to_string(&signed).expect("serialize signed v3");
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), &json).expect("write file");

        let selector = DaemonWarrenRelaySelector::load_from_cache_dir(&dir).expect("must parse v2");
        let selection = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .expect("must find the relay");
        assert_eq!(selection.endpoint_id, relay_pubkey);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_cache_dir_rejects_v2_with_tampered_relay_signature() {
        // Anti-MITM: an attacker who serves their own signed list OR
        // modifies a relay without re-signing must see the daemon
        // refuse to load (= falls back to empty list / error,
        // tunnel remains impossible rather than connecting to an
        // attacker).
        use ed25519_dalek::SigningKey;
        use warren_relay_selector::{JsonRelay as SignedJsonRelay, sign_relay_list};

        let dir = isolated_tempdir();
        let server_key = SigningKey::from_bytes(&[0xab; 32]);
        let relay_pubkey_hex = hex::encode(WarrenPubkey::from_bytes([5u8; 32]).as_bytes());

        let mut signed = sign_relay_list(
            vec![SignedJsonRelay {
                endpoint_id: relay_pubkey_hex,
                exit_id: ExitId::from_bytes([0xe2; 16]),
                ip_addrs: vec!["198.51.100.1:51820".to_owned()],
                country: "se".to_owned(),
                city: "Stockholm".to_owned(),
                weight: 100,
                active: true,
            }],
            &server_key,
            1_700_000_000,
        );
        // Tamper the port (= MITM re-routing to its relay) without
        // re-signing.
        signed.relays[0].ip_addrs = vec!["198.51.100.1:9999".to_owned()];
        let json = serde_json::to_string(&signed).expect("serialize tampered");
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), &json).expect("write");

        let result = DaemonWarrenRelaySelector::load_from_cache_dir(&dir);
        assert!(
            matches!(result, Err(LoadError::Json(_, _))),
            "tampered relay must produce LoadError::Json (signature verify fail)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_cache_dir_rejects_v1_unsigned_legacy_format() {
        // Anti-rollback: an attacker serving an unsigned v1 must
        // be rejected. v1 has been deprecated (see F3 fork audit) and
        // the daemon must refuse to ingest it (otherwise downgrade attack).
        let dir = isolated_tempdir();
        let pubkey_hex = hex::encode(WarrenPubkey::from_bytes([5u8; 32]).as_bytes());
        let json_v1 = format!(
            r#"{{"version":1,"relays":[{{"endpoint_id":"{pubkey_hex}","ip_addrs":["198.51.100.1:51820"],"country":"se","city":"Stockholm","weight":100,"active":true}}]}}"#
        );
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), &json_v1).expect("write v1");

        let result = DaemonWarrenRelaySelector::load_from_cache_dir(&dir);
        assert!(
            matches!(result, Err(LoadError::Json(_, _))),
            "v1 unsigned format must be rejected post-F3 (got {result:?})"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_cache_dir_returns_json_error_on_corrupt_file() {
        // If the file exists but contains invalid JSON, we
        // raise a typed error rather than silence it (silent
        // corruption would mask a bug).
        let dir = isolated_tempdir();
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), "not valid json").expect("write");

        let result = DaemonWarrenRelaySelector::load_from_cache_dir(&dir);
        assert!(matches!(result, Err(LoadError::Json(_, _))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tempdir isolated per test (pid + nanos + atomic counter).
    fn isolated_tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("warren-relay-selector-{pid}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }
}
