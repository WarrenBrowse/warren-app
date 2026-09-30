//! Route admission by anchor as the control plane announces it in its token
//! directory (warren-core doc 107), read by the tunnel through
//! [`RouteAdmissionSource`](crate::RouteAdmissionSource). The key is used only
//! under the signature of a server key the client pins (doc 107 section 6.5,
//! [`RouteKemTrust`]). The last signed block is kept in the client's cache
//! directory, so a tunnel started before a run's first directory read anchors
//! with it rather than not at all: the engine takes a main session's anchor
//! only when its supervisor is built, so a key that arrives later reaches the
//! next tunnel only.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use warren_api::{HttpTransport, RouteAdmission, TokenManager};
use warren_contract::dto::RouteAdmissionInfo;

use crate::RouteAdmissionSource;

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What the client trusts to sign the route KEM key of the token directory,
/// and where it keeps the last signed block across a restart.
#[derive(Clone, Debug, Default)]
pub struct RouteKemTrust {
    /// The API server keys the client pins (the same pins its relay list and
    /// multi-hop directory are verified against). Empty: no route admission.
    pub server_pins: Vec<String>,
    /// The file the last signed block is kept in. `None` keeps nothing.
    pub remembered_at: Option<PathBuf>,
}

/// The file, in the client's cache directory, that keeps the last signed
/// route admission block.
pub const REMEMBERED_ROUTE_ADMISSION: &str = "warren-route-admission.json";

/// Keeps `admission`'s signed block at `path`, or removes the file when the
/// directory offers no route admission. Best effort: a block that could not
/// be kept only costs the next start its anchor until the first
/// directory read.
pub fn remember_route_admission(path: &Path, admission: Option<&RouteAdmission>) {
    let Some(admission) = admission else {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::debug!("Could not forget the route admission block: {error}"),
        }
        return;
    };
    let Ok(block) = serde_json::to_vec(admission.info()) else {
        return;
    };
    if std::fs::read(path).is_ok_and(|kept| kept == block) {
        return;
    }
    let temporary = path.with_extension("json.tmp");
    let written =
        std::fs::write(&temporary, &block).and_then(|()| std::fs::rename(&temporary, path));
    if let Err(error) = written {
        log::debug!("Could not keep the route admission block: {error}");
    }
}

/// The block kept at `path`, trusted again only as far as its signature:
/// verified against `server_pins` at `now`, like a block just fetched.
pub fn remembered_route_admission(
    path: &Path,
    server_pins: &[String],
    now: u64,
) -> Option<RouteAdmission> {
    let info: RouteAdmissionInfo = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let pins: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    RouteAdmission::from_info(&info, &pins, now).ok()
}

/// Route admission as a manager's last directory announced it: read at each
/// call, so a refresh that finds the server turned it on or off reaches the
/// next tunnel and the next route dial, and never past the validity the
/// server signed. Before the manager's first directory read, the block kept
/// by an earlier run stands in, verified again.
pub struct DirectoryRouteAdmission<T> {
    manager: Arc<TokenManager<T>>,
    trust: RouteKemTrust,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// The kept block, read and verified once: a client that cannot reach
    /// the API asks for the key before every route dial, and the file only
    /// changes with a refresh, which supersedes it.
    kept: OnceLock<Option<RouteAdmission>>,
    /// Whether the manager has read the directory in this run. `None` reads
    /// it off the manager's epoch length, which only a directory read sets
    /// in a process that never restores a token bundle (the daemon).
    directory_read: Option<Arc<AtomicBool>>,
}

impl<T: HttpTransport> DirectoryRouteAdmission<T> {
    pub fn new(
        manager: Arc<TokenManager<T>>,
        trust: RouteKemTrust,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            manager,
            trust,
            now,
            kept: OnceLock::new(),
            directory_read: None,
        }
    }

    /// Told by `read` whether the manager has read the directory in this run,
    /// for a client that restores a token bundle at start (Android), whose
    /// manager knows its epoch length before any directory read.
    #[must_use]
    pub fn with_directory_read(mut self, read: Arc<AtomicBool>) -> Self {
        self.directory_read = Some(read);
        self
    }

    /// Over `manager` alone, with nothing kept: the manager's own pins decide.
    pub fn of(manager: Arc<TokenManager<T>>) -> Self {
        Self::new(manager, RouteKemTrust::default(), Arc::new(now_unix_secs))
    }

    fn current(&self) -> Option<RouteAdmission> {
        let now = (self.now)();
        let read = match &self.directory_read {
            Some(read) => read.load(Ordering::Acquire),
            // The daemon never restores a token bundle, so a known epoch
            // length means the manager has read the directory in this run.
            None => self.manager.epoch_at(now).is_some(),
        };
        if read {
            return self.manager.route_admission_at(now);
        }
        self.kept
            .get_or_init(|| {
                let path = self.trust.remembered_at.as_deref()?;
                remembered_route_admission(path, &self.trust.server_pins, now)
            })
            .clone()
            .filter(|admission| now < admission.valid_until())
    }
}

impl<T: HttpTransport + Send + Sync> RouteAdmissionSource for DirectoryRouteAdmission<T> {
    fn kem(&self) -> Option<warrenguard_multihop::RouteKemPublicKey> {
        self.current().map(|admission| admission.kem().clone())
    }

    fn offers_routes(&self, exit_id: &[u8; 16]) -> bool {
        self.current()
            .is_some_and(|admission| admission.offers_routes(exit_id))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use warren_api::{BlindingKey, TokenManager, WarrenApiClient};
    use warren_identity::WarrenIdentity;

    use super::{
        DirectoryRouteAdmission, REMEMBERED_ROUTE_ADMISSION, RouteKemTrust,
        remember_route_admission,
    };
    use crate::RouteAdmissionSource;

    const EPOCH_SECS: u64 = 3600;
    const EPOCH: u64 = 100;
    const NOW: u64 = EPOCH * EPOCH_SECS + 5;

    /// An issuer whose directory carries `route_admission` and no key to
    /// mint with: a refresh reads the block and asks for nothing else.
    struct Directory(Option<serde_json::Value>);

    impl warren_api::HttpTransport for Directory {
        async fn execute(
            &self,
            request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            assert!(request.url.ends_with("/v1/tokens/keys"), "{}", request.url);
            let mut body = serde_json::json!({
                "issuer_name": "api.example.test",
                "token_type": 2,
                "epoch_secs": EPOCH_SECS,
                "context_label": "warren/session-token/v1",
                "quota_per_epoch": 3,
                "prefetch_epochs": 0,
                "keys": [],
            });
            if let Some(block) = &self.0 {
                body["route_admission"] = block.clone();
            }
            Ok(warren_api::HttpResponse::new(
                200,
                serde_json::to_vec(&body).unwrap(),
            ))
        }
    }

    /// The API server key: it signs the route KEM key, which it also derives.
    const SERVER_SEED: [u8; 32] = [0x71; 32];

    fn server_pin() -> String {
        hex::encode(
            ed25519_dalek::SigningKey::from_bytes(&SERVER_SEED)
                .verifying_key()
                .as_bytes(),
        )
    }

    fn route_kem() -> warrenguard_multihop::RouteKemPublicKey {
        kem_of(&SERVER_SEED)
    }

    fn kem_of(seed: &[u8; 32]) -> warrenguard_multihop::RouteKemPublicKey {
        warrenguard_multihop::RouteKemSecretKey::derive(seed, 1)
            .unwrap()
            .public_key()
            .clone()
    }

    /// The block a server with route admission on serves for `kem`, signed
    /// by the server key until `valid_until`.
    fn signed_block(
        kem: &warrenguard_multihop::RouteKemPublicKey,
        valid_until: u64,
    ) -> serde_json::Value {
        let mut info = warren_contract::dto::RouteAdmissionInfo {
            version: 1,
            kem_key_id: 1,
            kem_pubkey_hex: warren_contract::dto::PubkeyHex::try_from(
                hex::encode(kem.to_bytes()).as_str(),
            )
            .unwrap(),
            max_routes_per_anchor: 32,
            exit_ids_hex: vec![warren_contract::dto::ExitId::from_bytes([9; 16])],
            kem_signature: None,
        };
        info.kem_signature = Some(warren_contract::route_kem::sign(
            &info,
            &ed25519_dalek::SigningKey::from_bytes(&SERVER_SEED),
            valid_until,
        ));
        serde_json::to_value(info).unwrap()
    }

    fn manager_over(block: Option<serde_json::Value>) -> Arc<TokenManager<Directory>> {
        let seed = [0x42; 32];
        Arc::new(
            TokenManager::new(
                Arc::new(WarrenApiClient::new(
                    "https://api.example.test",
                    WarrenIdentity::from_seed(&seed),
                    Directory(block),
                )),
                BlindingKey::session(&seed),
            )
            .with_server_pubkey_pins([server_pin()]),
        )
    }

    fn admission_over(
        manager: Arc<TokenManager<Directory>>,
        remembered_at: Option<std::path::PathBuf>,
    ) -> DirectoryRouteAdmission<Directory> {
        admission_at(manager, remembered_at, Arc::new(AtomicU64::new(NOW)))
    }

    fn admission_at(
        manager: Arc<TokenManager<Directory>>,
        remembered_at: Option<std::path::PathBuf>,
        clock: Arc<AtomicU64>,
    ) -> DirectoryRouteAdmission<Directory> {
        DirectoryRouteAdmission::new(
            manager,
            RouteKemTrust {
                server_pins: vec![server_pin()],
                remembered_at,
            },
            Arc::new(move || clock.load(Ordering::Relaxed)),
        )
    }

    async fn admission_after_refresh(
        block: Option<serde_json::Value>,
    ) -> DirectoryRouteAdmission<Directory> {
        let manager = manager_over(block);
        manager.refresh(NOW).await.expect("the directory is read");
        admission_over(manager, None)
    }

    #[tokio::test]
    async fn the_tunnel_is_handed_the_route_admission_the_directory_announces() {
        let admission = admission_after_refresh(Some(signed_block(&route_kem(), NOW + 60))).await;

        assert_eq!(
            admission.kem().map(|kem| kem.to_bytes()),
            Some(route_kem().to_bytes())
        );
        assert!(admission.offers_routes(&[9; 16]));
        assert!(!admission.offers_routes(&[8; 16]));
    }

    #[tokio::test]
    async fn a_route_kem_key_no_pinned_server_key_signed_is_not_handed_to_the_tunnel() {
        let mut substituted = signed_block(&route_kem(), NOW + 60);
        substituted["kem_pubkey_hex"] = hex::encode(kem_of(&[0x72; 32]).to_bytes()).into();
        let mut unsigned = signed_block(&route_kem(), NOW + 60);
        unsigned.as_object_mut().unwrap().remove("kem_signature");

        for block in [substituted, unsigned] {
            let admission = admission_after_refresh(Some(block)).await;

            assert!(admission.kem().is_none());
            assert!(!admission.offers_routes(&[9; 16]));
        }
    }

    #[tokio::test]
    async fn without_route_admission_in_the_directory_no_route_is_offered() {
        let admission = admission_after_refresh(None).await;

        assert!(admission.kem().is_none());
        assert!(!admission.offers_routes(&[9; 16]));
    }

    /// The block an earlier run kept, written the way a refresh keeps it.
    async fn kept_block(dir: &std::path::Path, block: serde_json::Value) -> std::path::PathBuf {
        let path = dir.join(REMEMBERED_ROUTE_ADMISSION);
        let manager = manager_over(Some(block));
        manager.refresh(NOW).await.expect("the directory is read");
        remember_route_admission(&path, manager.route_admission().as_ref());
        path
    }

    #[tokio::test]
    async fn before_the_first_directory_read_the_block_an_earlier_run_kept_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let path = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;

        let admission = admission_over(manager_over(None), Some(path));

        assert_eq!(
            admission.kem().map(|kem| kem.to_bytes()),
            Some(route_kem().to_bytes()),
            "a tunnel started before this run's first directory read anchors"
        );
        assert!(admission.offers_routes(&[9; 16]));
    }

    #[tokio::test]
    async fn a_kept_block_is_trusted_only_as_far_as_its_signature() {
        let dir = tempfile::tempdir().unwrap();
        let expired = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;
        let expired_admission = admission_at(
            manager_over(None),
            Some(expired.clone()),
            Arc::new(AtomicU64::new(NOW + 60)),
        );
        let substituted = dir.path().join("substituted.json");
        let mut block: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&expired).unwrap()).unwrap();
        block["kem_pubkey_hex"] = hex::encode(kem_of(&[0x72; 32]).to_bytes()).into();
        std::fs::write(&substituted, serde_json::to_vec(&block).unwrap()).unwrap();

        assert!(expired_admission.kem().is_none(), "an expired signature");
        assert!(
            admission_over(manager_over(None), Some(substituted))
                .kem()
                .is_none(),
            "a key written over the kept one"
        );
    }

    #[tokio::test]
    async fn a_key_is_withdrawn_once_its_signature_ends_whichever_block_it_came_from() {
        let dir = tempfile::tempdir().unwrap();
        let path = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;
        let fetched = manager_over(Some(signed_block(&route_kem(), NOW + 60)));
        fetched.refresh(NOW).await.expect("the directory is read");
        let clock = Arc::new(AtomicU64::new(NOW));
        let from_the_kept_block = admission_at(manager_over(None), Some(path), Arc::clone(&clock));
        let from_the_directory = admission_at(fetched, None, Arc::clone(&clock));
        assert!(from_the_kept_block.kem().is_some() && from_the_directory.kem().is_some());

        clock.store(NOW + 60, Ordering::Relaxed);

        assert!(from_the_kept_block.kem().is_none(), "kept block");
        assert!(
            from_the_directory.kem().is_none(),
            "a directory that cannot be fetched again does not stretch the signature"
        );
    }

    #[tokio::test]
    async fn the_first_directory_read_supersedes_and_rewrites_the_kept_block() {
        let dir = tempfile::tempdir().unwrap();
        let path = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;
        let manager = manager_over(None);
        manager.refresh(NOW).await.expect("the directory is read");

        let admission = admission_over(Arc::clone(&manager), Some(path.clone()));

        assert!(
            admission.kem().is_none(),
            "a server that turned route admission off is followed at once"
        );
        remember_route_admission(&path, manager.route_admission().as_ref());
        assert!(
            !path.exists(),
            "and the next run does not anchor with the old key"
        );
    }

    #[tokio::test]
    async fn a_client_that_restores_its_tokens_reads_the_kept_block_until_its_first_directory_read()
    {
        let dir = tempfile::tempdir().unwrap();
        let path = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;
        let restored = manager_over(None);
        let bundle = warren_api::PersistedTokens::from_json(
            &serde_json::json!({ "epoch_secs": EPOCH_SECS, "epochs": {} }).to_string(),
        )
        .expect("a bundle");
        let _ = restored.restore_persisted(&bundle);
        let read = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let admission = admission_over(Arc::clone(&restored), Some(path))
            .with_directory_read(Arc::clone(&read));

        let before = admission.kem().map(|kem| kem.to_bytes());
        read.store(true, Ordering::Release);
        let after = admission.kem();

        assert_eq!(
            before,
            Some(route_kem().to_bytes()),
            "a restored epoch length is not a directory read"
        );
        assert!(after.is_none(), "the directory this run read offers none");
    }
}
