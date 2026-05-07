//! Warren fork — Phase 4.E : wrapper daemon-side autour de la crate
//! [`warren_relay_selector::WarrenRelaySelector`].
//!
//! Rôle : encapsuler l'état de la `WarrenRelayList` côté daemon (sera
//! alimenté plus tard par un fetch périodique vers l'API), exposer une
//! API stable pour le `ParametersGenerator` (Phase 4.F future). Le
//! wrapper retourne uniquement les composants Iroh (`EndpointId` +
//! `EndpointAddr`) ; l'assemblage final en `WarrenIrohParameters` (avec
//! `signing_key`, `n_connections`, `features`) est fait par le caller
//! quand il a accès au `warren_signer` et à la config.
//!
//! **Pourquoi un module dédié** : (a) testable en isolation, (b)
//! n'importe pas `talpid-warren-iroh` côté API publique du wrapper
//! (Phase 4.F.1 utilise le wrapper sans charger talpid-warren-iroh).

use std::path::Path;
use std::sync::Arc;

use warren_relay_selector::iroh_types::{EndpointAddr, EndpointId};
use warren_relay_selector::{
    JsonError, SelectorError, WarrenRelay, WarrenRelayList, WarrenRelayQuery, WarrenRelaySelector,
};

/// Erreurs du chargement de `warren-relays.json` au boot.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// I/O sur le fichier `warren-relays.json` (chemin présent mais
    /// illisible).
    #[error("failed to read warren relays at {0}: {1}")]
    Io(String, #[source] std::io::Error),

    /// Le JSON est invalide ou ne respecte pas le schéma v1.
    #[error("invalid warren-relays.json at {0}: {1}")]
    Json(String, #[source] JsonError),
}

/// Sortie minimale de la sélection : les seuls deux champs
/// nécessaires pour construire un `WarrenIrohParameters` côté caller.
///
/// Cloneable (les deux types Iroh le sont) pour permettre au caller
/// d'en garder une copie avant d'en faire un `WarrenIrohParameters`.
#[derive(Debug, Clone)]
pub struct WarrenSelection {
    /// Identité Ed25519 de l'exit Warren sélectionné.
    pub endpoint_id: EndpointId,

    /// Adresses candidate de l'exit (UDP IPv4/IPv6 + relay url
    /// optionnel).
    pub endpoint_addr: EndpointAddr,
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
}

/// Nom du fichier qui contient la `WarrenRelayList` bootstrappée
/// localement. Convention figée : déplacement futur impose une
/// migration cache. Sera remplacé Phase 2.F par un fetch périodique
/// vers `mullvad-api`.
pub const WARREN_RELAYS_FILENAME: &str = "warren-relays.json";

impl DaemonWarrenRelaySelector {
    /// Construit un wrapper depuis une [`WarrenRelayList`].
    #[must_use]
    pub fn new(relays: WarrenRelayList) -> Self {
        Self {
            inner: Arc::new(WarrenRelaySelector::new(relays)),
        }
    }

    /// Charge la `WarrenRelayList` depuis `<cache_dir>/warren-relays.json`.
    ///
    /// Politique no-fail au boot : si le fichier est absent ou
    /// illisible, retourne un wrapper avec une liste vide + log warn,
    /// pour permettre au daemon de continuer à booter en mode WG. Le
    /// dispatch Phase 4.F.5 verra une `WarrenRelayList` vide et
    /// retournera `NoRelayMatch` à la première sélection — comportement
    /// attendu : l'utilisateur n'est pas en mode Warren.
    ///
    /// # Errors
    ///
    /// Retourne une erreur uniquement si le fichier existe mais
    /// contient un JSON invalide (= corruption silencieuse à signaler
    /// explicitement). Le caller (boot daemon) peut choisir de
    /// fallback sur une liste vide via `unwrap_or_else`.
    pub fn load_from_cache_dir(cache_dir: &Path) -> Result<Self, LoadError> {
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
        let list = WarrenRelayList::from_json_str(&raw)
            .map_err(|e| LoadError::Json(path.display().to_string(), e))?;
        log::info!(
            "Loaded {} Warren relays from {}",
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
    use warren_relay_selector::iroh_types::{EndpointAddr, SecretKey};
    use warren_relay_selector::{Location, LocationConstraint, WarrenRelay};

    use super::*;

    fn endpoint_id(seed: u8) -> EndpointId {
        SecretKey::from_bytes(&[seed; 32]).public()
    }

    fn relay(seed: u8, country: &str, addr_str: &str) -> WarrenRelay {
        let id = endpoint_id(seed);
        let addr = EndpointAddr::new(id).with_ip_addr(addr_str.parse().unwrap());
        WarrenRelay::new(id, addr, Location::new(country, "_"), 100, true)
    }

    #[test]
    fn daemon_selector_returns_iroh_components_for_unconstrained_query() {
        // Phase 4.E : le wrapper doit déléguer correctement à la crate
        // upstream et retourner un `WarrenSelection` avec les deux
        // champs Iroh attendus par `WarrenIrohParameters` côté
        // talpid-warren-iroh.
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
            "endpoint_addr doit contenir l'IP source"
        );
    }

    #[test]
    fn daemon_selector_propagates_location_constraint() {
        // Le wrapper doit honorer les contraintes de la query (filtrage
        // géo). Si on demande FR, on ne doit pas tomber sur SE.
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
                "attempt {attempt} doit toujours retourner le relay FR"
            );
        }
    }

    #[test]
    fn daemon_selector_returns_error_when_no_match() {
        // Phase 4.E : si la liste est vide, l'erreur upstream doit
        // remonter telle quelle (pas de remap silencieux).
        let selector = DaemonWarrenRelaySelector::new(WarrenRelayList::new(vec![]));
        assert!(matches!(
            selector.select_for_attempt(&WarrenRelayQuery::any(), 0),
            Err(SelectorError::NoRelayMatch)
        ));
    }

    #[test]
    fn load_from_cache_dir_returns_empty_list_when_file_absent() {
        // Phase 4.F.3 : au premier boot, le fichier n'existe pas →
        // wrapper avec liste vide, pas d'erreur. Permet au daemon de
        // démarrer sans nécessairement avoir une RelayList Warren.
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
    fn load_from_cache_dir_parses_valid_json_file() {
        // Phase 4.F.3 : si le fichier existe et contient un JSON v1
        // valide, le wrapper doit charger les relays correctement.
        let dir = isolated_tempdir();
        let pubkey_hex = hex::encode(SecretKey::from_bytes(&[5u8; 32]).public().as_bytes());
        let json = format!(
            r#"{{"version":1,"relays":[{{"endpoint_id":"{pubkey_hex}","ip_addrs":["198.51.100.1:51820"],"country":"se","city":"Stockholm","weight":100,"active":true}}]}}"#
        );
        std::fs::write(dir.join(WARREN_RELAYS_FILENAME), &json).expect("write file");

        let selector = DaemonWarrenRelaySelector::load_from_cache_dir(&dir).expect("must parse");
        let selection = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .expect("must find the relay");
        let expected_id = SecretKey::from_bytes(&[5u8; 32]).public();
        assert_eq!(selection.endpoint_id, expected_id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_cache_dir_returns_json_error_on_corrupt_file() {
        // Phase 4.F.3 : si le fichier existe mais contient un JSON
        // invalide, on remonte une erreur typée plutôt que de silencer
        // (la corruption silencieuse masquerait un bug).
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

    #[test]
    fn daemon_selector_is_cloneable_for_shared_use() {
        // Le wrapper est conçu pour être partagé entre threads (tunnel
        // state machine + gRPC management interface). Vérifie que
        // Clone produit deux handles vers la même liste sous-jacente.
        let list = WarrenRelayList::new(vec![relay(1, "se", "198.51.100.1:51820")]);
        let selector = DaemonWarrenRelaySelector::new(list);
        let cloned = selector.clone();

        let s1 = selector
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .unwrap();
        let s2 = cloned
            .select_for_attempt(&WarrenRelayQuery::any(), 0)
            .unwrap();
        assert_eq!(s1.endpoint_id, s2.endpoint_id);
    }
}
