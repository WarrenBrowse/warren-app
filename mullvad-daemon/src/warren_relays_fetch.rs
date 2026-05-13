//! Bootstrap fetcher pour `<cache_dir>/warren-relays.json`.
//!
//! Au boot du daemon en mode Warren, on tente un `GET {api_url}/v1/exits`
//! (endpoint public, cf. `warren-api/src/handlers.rs` § `list_exits`),
//! et on écrit la réponse brute dans le cache. La vérification de la
//! signature serveur Ed25519 (format v2) est faite ensuite par
//! `DaemonWarrenRelaySelector::load_from_cache_dir`.
//!
//! Best-effort : si le fetch échoue (réseau down, DNS, TLS, 5xx, JSON
//! invalide), on log un warn et on laisse l'ancien cache en place. La
//! state machine retournera `NoRelayMatch` si aucun cache valide
//! n'existe, comportement attendu = l'utilisateur n'est pas encore
//! connectable au réseau Warren.

use std::path::Path;
use std::time::Duration;

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const RELAYS_FILENAME: &str = "warren-relays.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("server returned non-success status {0}")]
    Status(u16),

    #[error("response body is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to write {0}: {1}")]
    Io(String, #[source] std::io::Error),
}

/// Fetch `/v1/exits` depuis `api_url` et écrit la réponse dans
/// `<cache_dir>/warren-relays.json`. Retourne le nombre d'octets écrits
/// en cas de succès.
///
/// Le body est vérifié syntaxiquement (`serde_json::Value`) avant
/// écriture pour ne pas corrompre un cache valide existant avec une
/// réponse non-JSON (page d'erreur Caddy, redirect HTML, etc.). La
/// vérification cryptographique (signature Ed25519 serveur) reste de la
/// responsabilité du loader aval.
pub async fn fetch_and_cache_relays(api_url: &str, cache_dir: &Path) -> Result<usize, Error> {
    let url = format!("{}/v1/exits", api_url.trim_end_matches('/'));
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Status(status.as_u16()));
    }
    let body = resp.text().await?;
    let _: serde_json::Value = serde_json::from_str(&body)?;
    let path = cache_dir.join(RELAYS_FILENAME);
    std::fs::write(&path, &body).map_err(|e| Error::Io(path.display().to_string(), e))?;
    Ok(body.len())
}
