//! Long-lived v7 anonymous-token minting for the app tunnel (Privacy Pass,
//! warren-core doc 64).
//!
//! Holds one [`warren_api::TokenManager`] per wallet (process-lived, keyed by
//! the ss58 address), spawns a background refresh task the first time a wallet
//! is seen (mint the current epoch + prefetch horizon on a coarse timer, never
//! at each connect, so issuance timing does not mirror session timing; a
//! failed round is retried sooner), and hands the tunnel a
//! [`SessionTokenSource`].
//!
//! The batch is derived from the wallet seed, so every client of the wallet
//! holds the same tokens, and an exit leases each serial to one live session in
//! the whole fleet. A session is therefore handed the whole current-epoch batch
//! and walks it: the engine redials leading with the next token when the exit
//! refuses one. Nothing is consumed, so a reconnect costs no token.
//!
//! Every session of the tunnel (the main one and each route session) opens a
//! provider of its own, which holds the serial it leads with
//! ([`TokenManager::claim`]) until the session's supervisor drops it. Another
//! session of this daemon never leads with a held serial, and a session that
//! redials leads with its own serial again, which the exit renews in place.
//! The hold follows the lead, not the serial the exit finally admitted: the
//! engine does not report which token of a walked stack it was admitted on, so
//! after a refusal the hold sits on the refused serial. That is why a held
//! serial goes to the tail of a stack and never out of it: it may be the very
//! serial the exit admitted this session on, the only one it would renew.
//!
//! v7 is the default: [`source_for`] is set on every assembled
//! [`talpid_warren_tunnel::WarrenTunnelParameters`]. With no token this epoch
//! (or a logged-out sentinel wallet with no subscription) the main session's
//! stack is empty and the supervisor falls back to the v6 wallet-signed path,
//! so the tunnel always works; a route session is then unavailable.
//!
//! An issuer that refuses the wallet as banned is reported to the account
//! standing monitor, which blocks the tunnel with the suspension before any
//! exit is dialed (warren-core doc 105 §5.3).
//!
//! The same manager reads the route admission block of the token directory
//! on each refresh (warren-core doc 107): [`route_admission_for`] hands the
//! tunnel the key its main session anchors with and the exits that admit
//! routes by anchor, as the last directory announced them. The key is used
//! only under the signature of a server key the daemon pins (doc 107 section
//! 6.5, [`RouteKemTrust`]). The last signed block is kept in the daemon's
//! cache directory, so a tunnel started before this run's first directory
//! read (right after a daemon start) anchors with it rather than not at all:
//! the engine takes a main session's anchor only when its supervisor is
//! built, so a key that arrives later reaches the next tunnel only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use talpid_warren_tunnel::{
    SESSION_TOKEN_LEN, SessionTokenSource, app_routes::RouteAdmissionSource,
    make_session_token_provider,
};
use warren_api::{
    BlindingKey, HttpTransport, RouteAdmission, SerialLease, TokenManager, WarrenApiClient,
};
use warren_contract::dto::RouteAdmissionInfo;
use warren_identity::WarrenIdentity;

use crate::warren_account_standing::StandingMonitor;
use crate::warren_api_transport::WarrenApiTransport;
use crate::warren_sdk_client::SharedWarrenSeed;

type Manager = TokenManager<WarrenApiTransport>;

/// One manager per wallet, reused across reconnects so its RAM token store,
/// its once-per-epoch issuance bookkeeping and its held serials survive
/// between sessions.
static MANAGERS: OnceLock<Mutex<HashMap<String, Arc<Manager>>>> = OnceLock::new();

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The coarse refresh period, and the first wait after a failed round. A
/// failure doubles the wait, up to the period: the first round runs when the
/// first tunnel starts, and while its firewall is still connecting the API
/// cannot be reached, which left route sessions without a token for the
/// whole period.
const REFRESH_PERIOD: Duration = Duration::from_secs(600);
const FIRST_RETRY: Duration = Duration::from_secs(15);

/// Runs `refresh` now and after each wait: the period once a round succeeds,
/// a shorter one after a failure.
async fn refresh_forever<F, R>(mut refresh: F)
where
    F: FnMut() -> R,
    R: std::future::Future<Output = bool>,
{
    let mut retry = FIRST_RETRY;
    loop {
        let wait = if refresh().await {
            retry = FIRST_RETRY;
            REFRESH_PERIOD
        } else {
            let wait = retry;
            retry = (retry * 2).min(REFRESH_PERIOD);
            wait
        };
        tokio::time::sleep(wait).await;
    }
}

/// What the daemon trusts to sign the route KEM key of the token directory,
/// and where it keeps the last signed block across a restart.
#[derive(Clone, Debug, Default)]
pub(crate) struct RouteKemTrust {
    /// The API server keys the daemon pins (the same pins its relay list and
    /// multi-hop directory are verified against). Empty: no route admission.
    pub server_pins: Vec<String>,
    /// The file the last signed block is kept in. `None` keeps nothing.
    pub remembered_at: Option<PathBuf>,
}

/// The file, in the daemon's cache directory, that keeps the last signed
/// route admission block.
pub(crate) const REMEMBERED_ROUTE_ADMISSION: &str = "warren-route-admission.json";

/// Keeps `admission`'s signed block at `path`, or removes the file when the
/// directory offers no route admission. Best effort: a block that could not
/// be kept only costs the next daemon start its anchor until the first
/// directory read.
fn remember_route_admission(path: &Path, admission: Option<&RouteAdmission>) {
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
fn remembered_route_admission(
    path: &Path,
    server_pins: &[String],
    now: u64,
) -> Option<RouteAdmission> {
    let info: RouteAdmissionInfo = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let pins: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    RouteAdmission::from_info(&info, &pins, now).ok()
}

fn spawn_refresh(
    manager: Arc<Manager>,
    wallet: String,
    standing: Option<StandingMonitor>,
    remembered_at: Option<PathBuf>,
) {
    // The manager mints only epochs it has not minted yet.
    tokio::spawn(refresh_forever(move || {
        let manager = Arc::clone(&manager);
        let wallet = wallet.clone();
        let standing = standing.clone();
        let remembered_at = remembered_at.clone();
        async move {
            let refreshed = manager.refresh(now_unix_secs()).await;
            if let Some(path) = remembered_at.as_deref()
                && manager.epoch_at(now_unix_secs()).is_some()
            {
                remember_route_admission(path, manager.route_admission().as_ref());
            }
            match refreshed {
                Ok(_) => true,
                Err(e) => {
                    log::warn!("Warren v7 token refresh failed (keeping existing tokens): {e}");
                    crate::warren_account_standing::report_if_banned(
                        standing.as_ref(),
                        &wallet,
                        &e,
                    );
                    false
                }
            }
        }
    }));
}

/// The v7 token source for `seed`'s wallet against `api_url`. Builds (and
/// starts refreshing) a manager the first time a wallet is seen; reuses it
/// after. The providers it opens never mint. A ban the issuer answers goes
/// to `standing`.
pub(crate) fn source_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
) -> SessionTokenSource {
    session_source(
        manager_for(api_url, seed, standing, trust),
        Arc::new(now_unix_secs),
    )
}

/// Route admission by anchor for `seed`'s wallet against `api_url`, as the
/// token directory the wallet's manager last fetched announces it, its key
/// signed by one of `trust`'s pins. The same manager as [`source_for`]'s.
pub(crate) fn route_admission_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
) -> Arc<dyn RouteAdmissionSource> {
    Arc::new(DirectoryRouteAdmission::new(
        manager_for(api_url, seed, standing, trust),
        trust.clone(),
        Arc::new(now_unix_secs),
    ))
}

/// Route admission as a manager's last directory announced it: read at each
/// call, so a refresh that finds the server turned it on or off reaches the
/// next tunnel and the next route dial, and never past the validity the
/// server signed. Before the manager's first directory read, the block kept
/// by an earlier run stands in, verified again.
pub(crate) struct DirectoryRouteAdmission<T> {
    manager: Arc<TokenManager<T>>,
    trust: RouteKemTrust,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// The kept block, read and verified once: a daemon that cannot reach
    /// the API asks for the key before every route dial, and the file only
    /// changes with a refresh, which supersedes it.
    kept: OnceLock<Option<RouteAdmission>>,
}

impl<T: HttpTransport> DirectoryRouteAdmission<T> {
    pub(crate) fn new(
        manager: Arc<TokenManager<T>>,
        trust: RouteKemTrust,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            manager,
            trust,
            now,
            kept: OnceLock::new(),
        }
    }

    /// Over `manager` alone, with nothing kept: the manager's own pins decide.
    #[cfg(test)]
    pub(crate) fn of(manager: Arc<TokenManager<T>>) -> Self {
        Self::new(manager, RouteKemTrust::default(), Arc::new(now_unix_secs))
    }

    fn current(&self) -> Option<RouteAdmission> {
        let now = (self.now)();
        // The daemon never restores a token bundle, so a known epoch length
        // means the manager has read the directory in this run.
        if self.manager.epoch_at(now).is_some() {
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

/// The wallet's manager, built and refreshed from its first use.
fn manager_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
) -> Arc<Manager> {
    let seed_bytes = seed.read().unwrap_or_else(PoisonError::into_inner);
    let identity = WarrenIdentity::from_seed(&seed_bytes);
    let key = identity.address();

    let map = MANAGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("token manager map poisoned");
    guard
        .entry(key.clone())
        .or_insert_with(|| {
            let client =
                WarrenApiClient::new(api_url.to_owned(), identity, WarrenApiTransport::new());
            let manager = Arc::new(
                TokenManager::new(Arc::new(client), BlindingKey::session(&seed_bytes))
                    .with_server_pubkey_pins(trust.server_pins.iter().cloned()),
            );
            spawn_refresh(
                manager.clone(),
                key,
                standing.cloned(),
                trust.remembered_at.clone(),
            );
            manager
        })
        .clone()
}

/// Opens, for each session, a provider over `manager`'s batch that holds the
/// serial the session leads with.
pub(crate) fn session_source<T: HttpTransport + Send + Sync + 'static>(
    manager: Arc<TokenManager<T>>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
) -> SessionTokenSource {
    Arc::new(move || {
        let session = SessionLead {
            manager: Arc::clone(&manager),
            now: Arc::clone(&now),
            lead: Mutex::new(None),
        };
        make_session_token_provider(Arc::new(move || session.stack()))
    })
}

/// One session's view of its wallet's batch.
struct SessionLead<T> {
    manager: Arc<TokenManager<T>>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// The token this session led with last, and its hold on the serial.
    lead: Mutex<Option<([u8; SESSION_TOKEN_LEN], SerialLease)>>,
}

impl<T: HttpTransport> SessionLead<T> {
    /// The stack for one establishment of this session: the epoch's whole
    /// batch, led by the token it led with before when no other session holds
    /// it, else by the first one no other session of the process holds, and
    /// ending with the serials other sessions hold.
    fn stack(&self) -> Vec<[u8; SESSION_TOKEN_LEN]> {
        let now = (self.now)();
        let mut lead = self.lead.lock().unwrap_or_else(PoisonError::into_inner);
        // Released first, so the manager hands it back in the stack.
        let previous = lead.take().map(|(token, _released)| token);
        let mut stack = self.manager.session_stack(now);
        if let Some(previous) = previous
            && let Some(at) = stack.iter().position(|token| *token == previous)
        {
            stack[..=at].rotate_right(1);
        }
        // Another session may have claimed a token of this stack since the
        // manager built it: the first one still free leads.
        for at in 0..stack.len() {
            if let Some(lease) = self.manager.claim(&stack[at]) {
                *lead = Some((stack[at], lease));
                stack[..=at].rotate_right(1);
                break;
            }
        }
        let held: Vec<_> = epoch_batch(&self.manager, now)
            .into_iter()
            .filter(|token| !stack.contains(token))
            .collect();
        stack.extend(held);
        stack
    }
}

/// Every token of the epoch `now` falls in, held or not, read from a copy of
/// the store: nothing is consumed.
fn epoch_batch<T: HttpTransport>(
    manager: &TokenManager<T>,
    now: u64,
) -> Vec<[u8; SESSION_TOKEN_LEN]> {
    let Some(mut copy) = manager.export_persistable() else {
        return Vec::new();
    };
    std::iter::from_fn(|| copy.take_current_stack(now).pop()).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use talpid_warren_tunnel::{SESSION_TOKEN_LEN, SessionTokenProvider, SessionTokenSource};
    use warren_api::{BlindingKey, PersistedTokens, TokenManager, WarrenApiClient};
    use warren_identity::WarrenIdentity;

    use super::session_source;
    use crate::warren_api_transport::WarrenApiTransport;

    const EPOCH_SECS: u64 = 3600;
    const EPOCH: u64 = 100;
    const NOW: u64 = EPOCH * EPOCH_SECS + 5;

    /// A token the store accepts: the Privacy Pass type, then bytes that make
    /// each token's serial its own. Never presented to an exit.
    fn token(fill: u8) -> [u8; SESSION_TOKEN_LEN] {
        let mut bytes = [fill; SESSION_TOKEN_LEN];
        bytes[..2].copy_from_slice(&2u16.to_be_bytes());
        bytes
    }

    /// The wallet's batches, as a restored bundle (nothing reaches the
    /// issuer), behind a clock the test moves.
    fn source_over(epochs: &[(u64, &[u8])]) -> (SessionTokenSource, Arc<AtomicU64>) {
        let seed = [0x42; 32];
        let manager = TokenManager::new(
            Arc::new(WarrenApiClient::new(
                "https://api.example.test",
                WarrenIdentity::from_seed(&seed),
                WarrenApiTransport::new(),
            )),
            BlindingKey::session(&seed),
        );
        let bundle = serde_json::json!({
            "epoch_secs": EPOCH_SECS,
            "epochs": epochs
                .iter()
                .map(|(epoch, fills)| {
                    let tokens: Vec<String> = fills
                        .iter()
                        .map(|&fill| URL_SAFE_NO_PAD.encode(token(fill)))
                        .collect();
                    (epoch.to_string(), serde_json::json!(tokens))
                })
                .collect::<serde_json::Map<_, _>>(),
        });
        let _ = manager
            .restore_persisted(&PersistedTokens::from_json(&bundle.to_string()).expect("a bundle"));
        let clock = Arc::new(AtomicU64::new(NOW));
        let read = Arc::clone(&clock);
        let source = session_source(
            Arc::new(manager),
            Arc::new(move || read.load(Ordering::SeqCst)),
        );
        (source, clock)
    }

    fn batch_of_three() -> SessionTokenSource {
        source_over(&[(EPOCH, &[1, 2, 3])]).0
    }

    fn lead(provider: &SessionTokenProvider) -> [u8; SESSION_TOKEN_LEN] {
        provider().first().expect("a token").0
    }

    /// When each round of a refresh task runs, in seconds from its start,
    /// given whether each round succeeds.
    async fn refresh_rounds(outcomes: &[bool]) -> Vec<u64> {
        let start = tokio::time::Instant::now();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut outcomes = Vec::from(outcomes).into_iter();
        let task = tokio::spawn(super::refresh_forever(move || {
            let _ = tx.send(start.elapsed().as_secs());
            let outcome = outcomes.next().unwrap_or(true);
            async move { outcome }
        }));
        let mut rounds = Vec::new();
        while rounds.len() < 5 {
            rounds.push(rx.recv().await.expect("the task runs"));
        }
        task.abort();
        rounds
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_refresh_is_retried_soon_and_sooner_than_the_period() {
        let rounds = refresh_rounds(&[false, false, true, true, true]).await;

        assert_eq!(rounds, [0, 15, 45, 645, 1245]);
    }

    #[tokio::test(start_paused = true)]
    async fn retries_back_off_up_to_the_period() {
        let rounds = refresh_rounds(&[false; 5]).await;

        assert_eq!(rounds, [0, 15, 45, 105, 225]);
    }

    #[test]
    fn a_session_is_handed_the_whole_batch_and_spends_none_of_it() {
        let open = batch_of_three();
        let session = open();

        assert_eq!(session().len(), 3);
        assert_eq!(session().len(), 3, "a redial is handed the batch again");
    }

    #[test]
    fn no_session_leads_with_the_serial_another_session_holds() {
        let open = batch_of_three();
        let main = open();
        let route = open();

        let main_lead = lead(&main);
        let route_stack = route();

        assert_ne!(route_stack[0].0, main_lead);
        assert_eq!(
            route_stack.last().map(|token| token.0),
            Some(main_lead),
            "a held serial goes last"
        );
    }

    #[test]
    fn a_redial_keeps_the_serial_another_session_holds_in_its_stack() {
        let open = batch_of_three();
        let main = open();
        let _ = main();
        let route = open();
        let route_lead = lead(&route);

        let redial = main();

        assert!(
            redial.iter().any(|token| token.0 == route_lead),
            "the exit may have admitted this session on the serial the other one leads with"
        );
    }

    #[test]
    fn a_redialling_session_leads_with_its_own_serial_again() {
        let open = batch_of_three();
        let first = open();
        let _ = first();
        let session = open();
        let own = lead(&session);
        drop(first);

        assert_eq!(
            lead(&session),
            own,
            "a freed serial ahead of it is not taken"
        );
    }

    #[test]
    fn a_session_that_ends_frees_its_serial() {
        let open = batch_of_three();
        let ended = open();
        let freed = lead(&ended);
        let live = open();
        let _ = live();
        drop(ended);

        let next = open();

        assert_eq!(lead(&next), freed);
    }

    #[test]
    fn a_new_epoch_leads_with_a_token_of_that_epoch() {
        let (open, clock) = source_over(&[(EPOCH, &[1, 2, 3]), (EPOCH + 1, &[4, 5, 6])]);
        let session = open();
        let _ = session();

        clock.store((EPOCH + 1) * EPOCH_SECS + 5, Ordering::SeqCst);
        let next = session();

        let next_epoch = [token(4), token(5), token(6)];
        assert_eq!(next.len(), 3);
        assert!(next.iter().all(|t| next_epoch.contains(&t.0)));
    }

    #[test]
    fn a_wallet_without_a_token_this_epoch_hands_an_empty_stack() {
        let open = source_over(&[]).0;

        assert!(open()().is_empty());
    }

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
            Ok(warren_api::HttpResponse {
                status: 200,
                body: serde_json::to_vec(&body).unwrap(),
            })
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
    ) -> super::DirectoryRouteAdmission<Directory> {
        admission_at(manager, remembered_at, Arc::new(AtomicU64::new(NOW)))
    }

    fn admission_at(
        manager: Arc<TokenManager<Directory>>,
        remembered_at: Option<std::path::PathBuf>,
        clock: Arc<AtomicU64>,
    ) -> super::DirectoryRouteAdmission<Directory> {
        super::DirectoryRouteAdmission::new(
            manager,
            super::RouteKemTrust {
                server_pins: vec![server_pin()],
                remembered_at,
            },
            Arc::new(move || clock.load(Ordering::Relaxed)),
        )
    }

    async fn admission_after_refresh(
        block: Option<serde_json::Value>,
    ) -> super::DirectoryRouteAdmission<Directory> {
        let manager = manager_over(block);
        manager.refresh(NOW).await.expect("the directory is read");
        admission_over(manager, None)
    }

    #[tokio::test]
    async fn the_tunnel_is_handed_the_route_admission_the_directory_announces() {
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;

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
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;
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
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;

        let admission = admission_after_refresh(None).await;

        assert!(admission.kem().is_none());
        assert!(!admission.offers_routes(&[9; 16]));
    }

    /// The block an earlier run kept, written the way a refresh keeps it.
    async fn kept_block(dir: &std::path::Path, block: serde_json::Value) -> std::path::PathBuf {
        let path = dir.join(super::REMEMBERED_ROUTE_ADMISSION);
        let manager = manager_over(Some(block));
        manager.refresh(NOW).await.expect("the directory is read");
        super::remember_route_admission(&path, manager.route_admission().as_ref());
        path
    }

    #[tokio::test]
    async fn before_the_first_directory_read_the_block_an_earlier_run_kept_is_used() {
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;
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
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;
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
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;
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
        use talpid_warren_tunnel::app_routes::RouteAdmissionSource;
        let dir = tempfile::tempdir().unwrap();
        let path = kept_block(dir.path(), signed_block(&route_kem(), NOW + 60)).await;
        let manager = manager_over(None);
        manager.refresh(NOW).await.expect("the directory is read");

        let admission = admission_over(Arc::clone(&manager), Some(path.clone()));

        assert!(
            admission.kem().is_none(),
            "a server that turned route admission off is followed at once"
        );
        super::remember_route_admission(&path, manager.route_admission().as_ref());
        assert!(
            !path.exists(),
            "and the next run does not anchor with the old key"
        );
    }
}
