//! Wrapper daemon-side autour de la crate
//! [`warren_relay_selector::WarrenRelaySelector`].
//!
//! Encapsule l'état de la `WarrenRelayList` côté daemon (sera alimenté
//! plus tard par un fetch périodique vers l'API ; pour le POC, chargée
//! depuis `<cache_dir>/warren-relays.json`), et expose une API stable
//! pour le `ParametersGenerator`. Le wrapper retourne uniquement les
//! composants Iroh (`EndpointId` + `EndpointAddr`) ; l'assemblage
//! final en `WarrenTunnelParameters` (avec `signing_key`,
//! `n_connections`, `features`) est fait par
//! [`crate::warren_tunnel_params::assemble_for_attempt`].
//!
//! Module dédié pour deux raisons : testable en isolation, et n'importe
//! pas `talpid-warren-tunnel` côté API publique du wrapper.

use std::path::Path;
use std::sync::Arc;

use warren_relay_selector::warren_types::{WarrenExitAddr, WarrenPubkey};
use warren_relay_selector::{
    SelectorError, SignedError, WarrenRelay, WarrenRelayList, WarrenRelayQuery,
    WarrenRelaySelector, verify_signed_relay_list,
};

/// Erreurs du chargement de `warren-relays.json` au boot.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// I/O sur le fichier `warren-relays.json` (chemin présent mais
    /// illisible).
    #[error("failed to read warren relays at {0}: {1}")]
    Io(String, #[source] std::io::Error),

    /// Le JSON est invalide, n'a pas la version supportée, ou la
    /// signature serveur ne vérifie pas. Format wire v2 obligatoire
    /// post-F3 fork audit (cf. `warren_relay_selector::signed`).
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

    /// Candidate addresses of the exit (UDP IPv4/IPv6).
    pub endpoint_addr: WarrenExitAddr,
}

impl From<&WarrenRelay> for WarrenSelection {
    fn from(relay: &WarrenRelay) -> Self {
        Self {
            endpoint_id: relay.endpoint_id(),
            endpoint_addr: relay.endpoint_addr().clone(),
        }
    }
}

/// Wrapper daemon-side autour du `WarrenRelaySelector`.
///
/// Détient un `Arc<WarrenRelaySelector>` pour permettre un partage
/// thread-safe entre le tunnel state machine et le management
/// interface gRPC (futur).
#[derive(Debug, Clone)]
pub struct DaemonWarrenRelaySelector {
    inner: Arc<WarrenRelaySelector>,
    /// Liste brute conservée à part pour permettre au caller (boot
    /// daemon) de la convertir en `RelayList` Mullvad-format et la
    /// broadcaster à la GUI Electron via `notify_relay_list`. Cf.
    /// `warren_relay_list_view::to_mullvad_relay_list`.
    list: Arc<WarrenRelayList>,
}

/// Nom du fichier qui contient la `WarrenRelayList` bootstrappée
/// localement. Convention figée : déplacement futur impose une
/// migration cache. À remplacer par un fetch périodique vers
/// `mullvad-api` quand l'endpoint Warren sera disponible.
pub const WARREN_RELAYS_FILENAME: &str = "warren-relays.json";

impl DaemonWarrenRelaySelector {
    /// Construit un wrapper depuis une [`WarrenRelayList`].
    #[must_use]
    pub fn new(relays: WarrenRelayList) -> Self {
        let list = Arc::new(relays.clone());
        Self {
            inner: Arc::new(WarrenRelaySelector::new(relays)),
            list,
        }
    }

    /// Accès lecture à la `WarrenRelayList` brute (= ce qui a été
    /// passé à [`Self::new`] ou chargé depuis le cache). Utilisé par
    /// le boot daemon pour broadcaster la liste à la GUI via
    /// [`crate::warren_relay_list_view`].
    #[must_use]
    pub fn list(&self) -> &WarrenRelayList {
        &self.list
    }

    /// Charge la `WarrenRelayList` depuis `<cache_dir>/warren-relays.json`.
    ///
    /// Politique no-fail au boot : si le fichier est absent ou
    /// illisible, retourne un wrapper avec une liste vide + log warn,
    /// pour permettre au daemon de continuer à booter en mode WG. Le
    /// state machine verra une `WarrenRelayList` vide et retournera
    /// `NoRelayMatch` à la première sélection — comportement attendu :
    /// l'utilisateur n'est pas en mode Warren.
    ///
    /// # Errors
    ///
    /// Retourne une erreur uniquement si le fichier existe mais
    /// contient un JSON invalide (= corruption silencieuse à signaler
    /// explicitement). Le caller (boot daemon) peut choisir de
    /// fallback sur une liste vide via `unwrap_or_else`.
    pub fn load_from_cache_dir(cache_dir: &Path) -> Result<Self, LoadError> {
        Self::load_from_cache_dir_with_pin(cache_dir, None)
    }

    /// Variante de [`Self::load_from_cache_dir`] avec **pin de la
    /// pubkey serveur**. Si `expected_server_pubkey_hex` est `Some(hex)`,
    /// refuse toute liste signée par une autre pubkey (= protection
    /// MITM-on-bootstrap). Si `None`, mode TOFU : accepte toute
    /// signature self-cohérente (utile pour le 1er fetch ou les tests).
    ///
    /// Format attendu : v2 signé Ed25519 (cf.
    /// [`warren_relay_selector::verify_signed_relay_list`]). Le format
    /// v1 non-signé est **rejeté** post-F3 audit (anti-downgrade
    /// attack — un attaquant qui sert un v1 sans signature pourrait
    /// substituer la liste sans détection).
    ///
    /// # Errors
    ///
    /// - [`LoadError::Io`] si le fichier existe mais est illisible.
    /// - [`LoadError::Json`] si le JSON est invalide, version != 2, la
    ///   pubkey serveur diffère du pin, ou la signature ne vérifie pas.
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

    /// Sélectionne un relay pour la tentative `retry_attempt` et
    /// retourne ses composants Iroh.
    ///
    /// API miroir de la
    /// [`mullvad_relay_selector::RelaySelector::get_relay`] côté
    /// WireGuard — facilite le dispatch par
    /// `ParametersGenerator::generate(retry_attempt, ...)`.
    ///
    /// # Errors
    ///
    /// Retourne [`SelectorError::NoRelayMatch`] si aucun relay actif
    /// avec `weight > 0` ne satisfait les contraintes.
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
        WarrenRelay::new(id, addr, Location::new(country, "_"), 100, true)
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
        // Au premier boot, le fichier n'existe pas → wrapper avec
        // liste vide, pas d'erreur. Permet au daemon de démarrer
        // sans nécessairement avoir une RelayList Warren.
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
        // F3 fork audit : warren-api `/v1/exits` retourne un format
        // **v2 signé** (`SignedRelayList` avec server_pubkey + signature
        // Ed25519). Le daemon doit le parser et vérifier la signature
        // — pas accepter du v1 non-signé. Format figé : si serde change
        // l'ordre des fields v2, ce test (et toute installation
        // existante) casse → rotation `/v3` obligatoire.
        use ed25519_dalek::SigningKey;
        use warren_relay_selector::{JsonRelay as SignedJsonRelay, sign_relay_list};

        let dir = isolated_tempdir();

        // Server signing key fixe pour le test (déterministe).
        let server_key = SigningKey::from_bytes(&[0xab; 32]);
        let relay_pubkey = WarrenPubkey::from_bytes([5u8; 32]);
        let relay_pubkey_hex = hex::encode(relay_pubkey.as_bytes());

        let signed = sign_relay_list(
            vec![SignedJsonRelay {
                endpoint_id: relay_pubkey_hex,
                ip_addrs: vec!["198.51.100.1:51820".to_owned()],
                country: "se".to_owned(),
                city: "Stockholm".to_owned(),
                weight: 100,
                active: true,
            }],
            &server_key,
            1_700_000_000,
        );
        let json = serde_json::to_string(&signed).expect("serialize signed v2");
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
        // Anti-MITM : un attaquant qui sert sa propre liste signée OU
        // qui modifie un relay sans re-signer doit voir le daemon
        // refuser de charger (= falls back vers liste vide / erreur,
        // tunnel reste impossible plutôt que de connecter à un
        // attaquant).
        use ed25519_dalek::SigningKey;
        use warren_relay_selector::{JsonRelay as SignedJsonRelay, sign_relay_list};

        let dir = isolated_tempdir();
        let server_key = SigningKey::from_bytes(&[0xab; 32]);
        let relay_pubkey_hex = hex::encode(WarrenPubkey::from_bytes([5u8; 32]).as_bytes());

        let mut signed = sign_relay_list(
            vec![SignedJsonRelay {
                endpoint_id: relay_pubkey_hex,
                ip_addrs: vec!["198.51.100.1:51820".to_owned()],
                country: "se".to_owned(),
                city: "Stockholm".to_owned(),
                weight: 100,
                active: true,
            }],
            &server_key,
            1_700_000_000,
        );
        // Tamper le port (= MITM qui re-route vers son relais) sans
        // re-signer.
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
        // Anti-rollback : un attaquant qui sert un v1 non-signé doit
        // être rejeté. v1 a été déprécié (cf. F3 fork audit) et le
        // daemon doit refuser de l'ingérer (sinon downgrade attack).
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
        // Si le fichier existe mais contient un JSON invalide, on
        // remonte une erreur typée plutôt que de silencer (la
        // corruption silencieuse masquerait un bug).
        let dir = isolated_tempdir();
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), "not valid json").expect("write");

        let result = DaemonWarrenRelaySelector::load_from_cache_dir(&dir);
        assert!(matches!(result, Err(LoadError::Json(_, _))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tempdir isolé par test (pid + nanos + counter atomique).
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
