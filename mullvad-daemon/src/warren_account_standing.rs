//! Keeps the wallet's port-forward abuse standing current (warren-core doc
//! 105 §5.3, §5.4).
//!
//! Two inputs feed one [`StandingTracker`]: a poll of the signed
//! `GET /v1/account/standing`, on the same coarse cadence as the token
//! refresh and once when a tunnel is asked for, and the ban refusal the token
//! and entitlement issuers answer, reported by their refresh tasks. The second
//! is how a v7 client learns of a ban at all: it never presents its wallet to
//! an exit, so the exit's ban rejection cannot reach it.
//!
//! Every change goes to the daemon as a [`StandingUpdate`]: the daemon
//! publishes the standing, raises one event per new strike, and blocks the
//! tunnel while a ban holds. Which strikes were already announced survives a
//! restart in `<cache_dir>/warren-strike-ledger.json`, which holds digests,
//! never a case reference.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use warren_api::{AccountStandingResponse, ClientError};
use warren_standing::{Ban, Standing, StandingTracker, StandingUpdate, StrikeLedger};

/// How often the standing is asked for: the token refresh cadence, so the
/// request pattern the API sees stays the one it already sees.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(600);

/// A poll asked for on a connect is skipped when the last one is younger
/// than this: a reconnect storm must not turn into a standing storm.
pub(crate) const MIN_POKE_GAP: Duration = Duration::from_secs(60);

/// File name of the strike ledger in the daemon cache directory.
const LEDGER_FILE: &str = "warren-strike-ledger.json";

/// Wall-clock Unix seconds, the clock a ban's lapse is written in.
pub(crate) fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

type BoxFut<T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

/// Where the standing comes from: the signed API client in production.
pub(crate) trait StandingSource: Send + Sync + 'static {
    /// The wallet the next answer is about, `None` without one.
    fn wallet(&self) -> Option<String>;
    /// One signed `GET /v1/account/standing`.
    fn fetch(&self) -> BoxFut<Result<AccountStandingResponse, ClientError>>;
}

impl StandingSource for crate::warren_sdk_client::SharedWarrenApiClient {
    fn wallet(&self) -> Option<String> {
        crate::warren_sdk_client::SharedWarrenApiClient::wallet(self)
    }

    fn fetch(&self) -> BoxFut<Result<AccountStandingResponse, ClientError>> {
        let client = self.clone();
        Box::pin(async move { client.account_standing().await })
    }
}

/// What a caller gets when it asks for the standing now.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FetchError {
    /// No wallet is installed.
    #[error("no Warren wallet is installed")]
    NoWallet,
    /// The API did not answer the standing.
    #[error("the account standing could not be fetched")]
    Api(#[source] ClientError),
    /// The monitor task is gone (daemon shutting down).
    #[error("the account standing monitor has stopped")]
    Stopped,
}

enum Command {
    Poke,
    IssuanceBan { wallet: String, ban: Ban },
    FetchNow(oneshot::Sender<Result<Standing, FetchError>>),
}

/// Handle on the monitor task. Cheap to clone; every method is fire and
/// forget except [`Self::fetch_now`].
#[derive(Clone)]
pub(crate) struct StandingMonitor {
    tx: mpsc::UnboundedSender<Command>,
}

impl StandingMonitor {
    /// Starts the monitor. `on_update` runs on the monitor task for every
    /// change; `cache_dir` holds the strike ledger (`None` keeps it in memory).
    pub(crate) fn spawn(
        source: Arc<dyn StandingSource>,
        cache_dir: Option<PathBuf>,
        on_update: Arc<dyn Fn(StandingUpdate) + Send + Sync>,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let ledger_path = cache_dir.map(|dir| dir.join(LEDGER_FILE));
        tokio::spawn(run(source, ledger_path, rx, on_update));
        Self { tx }
    }

    /// Asks for a poll now, unless one ran less than [`MIN_POKE_GAP`] ago.
    pub(crate) fn poke(&self) {
        let _ = self.tx.send(Command::Poke);
    }

    /// An issuer refused `wallet` as banned.
    pub(crate) fn report_issuance_ban(&self, wallet: String, ban: Ban) {
        let _ = self.tx.send(Command::IssuanceBan { wallet, ban });
    }

    /// Polls now, whatever the last poll's age, and answers the standing.
    pub(crate) async fn fetch_now(&self) -> Result<Standing, FetchError> {
        let (reply, answer) = oneshot::channel();
        self.tx
            .send(Command::FetchNow(reply))
            .map_err(|_| FetchError::Stopped)?;
        answer.await.map_err(|_| FetchError::Stopped)?
    }
}

struct Monitor {
    source: Arc<dyn StandingSource>,
    ledger_path: Option<PathBuf>,
    tracker: StandingTracker,
    on_update: Arc<dyn Fn(StandingUpdate) + Send + Sync>,
    last_poll: Option<tokio::time::Instant>,
}

async fn run(
    source: Arc<dyn StandingSource>,
    ledger_path: Option<PathBuf>,
    mut rx: mpsc::UnboundedReceiver<Command>,
    on_update: Arc<dyn Fn(StandingUpdate) + Send + Sync>,
) {
    let ledger = ledger_path.as_deref().map(load_ledger).unwrap_or_default();
    let mut monitor = Monitor {
        source,
        ledger_path,
        tracker: StandingTracker::new(ledger),
        on_update,
        last_poll: None,
    };
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let _ = monitor.poll().await;
            }
            command = rx.recv() => match command {
                None => return,
                Some(Command::Poke) => {
                    let recent = monitor
                        .last_poll
                        .is_some_and(|at| at.elapsed() < MIN_POKE_GAP);
                    if !recent {
                        let _ = monitor.poll().await;
                    }
                }
                Some(Command::IssuanceBan { wallet, ban }) => monitor.issuance_ban(&wallet, ban),
                Some(Command::FetchNow(reply)) => {
                    let _ = reply.send(monitor.poll().await);
                }
            },
        }
    }
}

impl Monitor {
    async fn poll(&mut self) -> Result<Standing, FetchError> {
        self.last_poll = Some(tokio::time::Instant::now());
        let Some(wallet) = self.source.wallet() else {
            if let Some(update) = self.tracker.on_no_wallet() {
                (self.on_update)(update);
            }
            return Err(FetchError::NoWallet);
        };
        let response = match self.source.fetch().await {
            Ok(response) => response,
            Err(error) => {
                // The client's Display carries a status code at most, never
                // the body, which may echo identity material.
                log::warn!("Warren account standing poll failed (keeping the last): {error}");
                return Err(FetchError::Api(error));
            }
        };
        // The wallet may have changed during the fetch; the answer is about
        // the one it was signed for, which is the one asked before it.
        if self.source.wallet().as_deref() != Some(wallet.as_str()) {
            return Err(FetchError::NoWallet);
        }
        let before = self.tracker.ledger().clone();
        let update = self.tracker.on_standing(&wallet, response);
        if *self.tracker.ledger() != before {
            self.persist_ledger();
        }
        if let Some(update) = update {
            (self.on_update)(update);
        }
        self.tracker.standing().cloned().ok_or(FetchError::NoWallet)
    }

    fn issuance_ban(&mut self, wallet: &str, ban: Ban) {
        // A refusal for a wallet that is no longer installed says nothing
        // about the one that is.
        if self.source.wallet().as_deref() != Some(wallet) {
            return;
        }
        if let Some(update) = self.tracker.on_issuance_ban(wallet, ban) {
            (self.on_update)(update);
        }
    }

    fn persist_ledger(&self) {
        let Some(path) = self.ledger_path.as_deref() else {
            return;
        };
        if let Err(error) = write_atomically(path, self.tracker.ledger().to_json().as_bytes()) {
            log::warn!("Could not save the Warren strike ledger: {error}");
        }
    }
}

/// What the daemon does with the tunnel once the standing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BanAction {
    /// Enter the blocking error state with this auth-failed reason.
    Block(String),
    /// The ban that blocked the tunnel is gone: connect again.
    Reconnect,
    /// Leave the tunnel alone.
    Nothing,
}

/// Decides [`BanAction`].
///
/// `ban` is the ban in force now, `was_banned` whether one was in force
/// before this change, `secured` whether the user wants a tunnel, and
/// `blocked_for_ban` whether the tunnel already sits in the error state a ban
/// puts it in. A ban blocks only a tunnel the user asked for: a disconnected
/// client stays disconnected, and the suspension shows on its next connect.
pub(crate) fn ban_action(
    ban: Option<Ban>,
    was_banned: bool,
    secured: bool,
    blocked_for_ban: bool,
) -> BanAction {
    match ban {
        Some(ban) if secured && !blocked_for_ban => BanAction::Block(ban.auth_failed_reason()),
        None if was_banned && secured && blocked_for_ban => BanAction::Reconnect,
        _ => BanAction::Nothing,
    }
}

/// Reports `error` to `standing` when it is an issuer's ban refusal of
/// `wallet`. Answers whether it was one.
pub(crate) fn report_if_banned(
    standing: Option<&StandingMonitor>,
    wallet: &str,
    error: &warren_api::TokenClientError,
) -> bool {
    let Some(ban) = Ban::from_refresh_error(error) else {
        return false;
    };
    if let Some(standing) = standing {
        standing.report_issuance_ban(wallet.to_owned(), ban);
    }
    true
}

fn load_ledger(path: &Path) -> StrikeLedger {
    match std::fs::read_to_string(path) {
        Ok(json) => StrikeLedger::from_json(&json).unwrap_or_else(|error| {
            // The cost of starting over is one repeated warning per live
            // strike, which is better than never warning again.
            log::warn!("Ignoring the Warren strike ledger: {error}");
            StrikeLedger::new()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StrikeLedger::new(),
        Err(error) => {
            log::warn!("Could not read the Warren strike ledger: {error}");
            StrikeLedger::new()
        }
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use warren_api::{AccountStandingResponse, AccountStrike, BanReasonCode};

    use super::*;

    const WALLET: &str = "wallet-a";

    struct FakeSource {
        wallet: Mutex<Option<String>>,
        answer: Mutex<Result<AccountStandingResponse, u16>>,
        fetches: AtomicUsize,
    }

    impl FakeSource {
        fn new(answer: AccountStandingResponse) -> Arc<Self> {
            Arc::new(Self {
                wallet: Mutex::new(Some(WALLET.to_owned())),
                answer: Mutex::new(Ok(answer)),
                fetches: AtomicUsize::new(0),
            })
        }

        fn fetches(&self) -> usize {
            self.fetches.load(Ordering::SeqCst)
        }
    }

    impl StandingSource for FakeSource {
        fn wallet(&self) -> Option<String> {
            self.wallet.lock().unwrap().clone()
        }

        fn fetch(&self) -> BoxFut<Result<AccountStandingResponse, ClientError>> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            let answer = self.answer.lock().unwrap().clone();
            Box::pin(async move {
                answer.map_err(|status| ClientError::ServerStatus {
                    status,
                    body: String::new(),
                })
            })
        }
    }

    fn strike(case_reference: &str) -> AccountStrike {
        serde_json::from_value(serde_json::json!({
            "day_unix_secs": 1_790_035_200u64,
            "category": "copyright",
            "port": 51413,
            "case_reference": case_reference,
        }))
        .unwrap()
    }

    fn answer(strikes: &[&str]) -> AccountStandingResponse {
        AccountStandingResponse {
            strikes: strikes.iter().map(|r| strike(r)).collect(),
            threshold: 3,
            window_days: 90,
            ban: None,
        }
    }

    fn ban() -> Ban {
        Ban {
            reason: BanReasonCode::PortForwardingAbuse,
            banned_at_unix_secs: None,
            lapses_at_unix_secs: None,
        }
    }

    type Updates = Arc<Mutex<Vec<StandingUpdate>>>;

    fn collector() -> (Updates, Arc<dyn Fn(StandingUpdate) + Send + Sync>) {
        let updates: Updates = Arc::default();
        let sink = updates.clone();
        (updates, Arc::new(move |u| sink.lock().unwrap().push(u)))
    }

    async fn settle() {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    fn announced(updates: &Updates) -> Vec<String> {
        updates
            .lock()
            .unwrap()
            .iter()
            .flat_map(|u| u.new_strikes.iter())
            .map(|n| n.strike.case_reference.clone())
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn the_standing_is_polled_at_start_and_on_the_interval() {
        let source = FakeSource::new(answer(&[]));
        let (_updates, sink) = collector();
        let _monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;
        assert_eq!(source.fetches(), 1, "the first poll runs at start");

        tokio::time::advance(POLL_INTERVAL - Duration::from_secs(1)).await;
        settle().await;
        assert_eq!(source.fetches(), 1, "nothing before the interval");

        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
        assert_eq!(source.fetches(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_connect_polls_unless_the_last_poll_is_recent() {
        let source = FakeSource::new(answer(&[]));
        let (_updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;

        monitor.poke();
        settle().await;
        assert_eq!(source.fetches(), 1, "a poke right after a poll is skipped");

        tokio::time::advance(MIN_POKE_GAP).await;
        monitor.poke();
        settle().await;
        assert_eq!(source.fetches(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn an_issuance_ban_of_the_installed_wallet_is_published() {
        let source = FakeSource::new(answer(&[]));
        *source.answer.lock().unwrap() = Err(503);
        let (updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;

        monitor.report_issuance_ban(WALLET.to_owned(), ban());
        settle().await;

        let published = updates.lock().unwrap().last().cloned().unwrap();
        assert_eq!(published.standing, Some(Standing::ban_only(ban())));
    }

    #[tokio::test(start_paused = true)]
    async fn an_issuance_ban_of_another_wallet_is_ignored() {
        let source = FakeSource::new(answer(&[]));
        let (updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;
        let before = updates.lock().unwrap().len();

        monitor.report_issuance_ban("wallet-b".to_owned(), ban());
        settle().await;

        assert_eq!(updates.lock().unwrap().len(), before);
    }

    #[test]
    fn a_ban_blocks_a_tunnel_the_user_asked_for() {
        assert_eq!(
            ban_action(Some(ban()), false, true, false),
            BanAction::Block(ban().auth_failed_reason())
        );
    }

    #[test]
    fn a_ban_leaves_a_disconnected_client_alone() {
        assert_eq!(
            ban_action(Some(ban()), false, false, false),
            BanAction::Nothing
        );
    }

    #[test]
    fn a_tunnel_already_blocked_for_the_ban_is_not_blocked_again() {
        assert_eq!(
            ban_action(Some(ban()), true, true, true),
            BanAction::Nothing
        );
    }

    #[test]
    fn a_lifted_ban_reconnects_the_tunnel_it_blocked() {
        assert_eq!(ban_action(None, true, true, true), BanAction::Reconnect);
    }

    #[test]
    fn good_standing_leaves_the_tunnel_alone() {
        assert_eq!(ban_action(None, false, true, true), BanAction::Nothing);
        assert_eq!(ban_action(None, true, true, false), BanAction::Nothing);
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_refused_as_banned_reaches_the_standing() {
        let source = FakeSource::new(answer(&[]));
        *source.answer.lock().unwrap() = Err(503);
        let (updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;
        let refusal = warren_api::TokenClientError::Api(ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs: None,
        });

        assert!(report_if_banned(Some(&monitor), WALLET, &refusal));
        settle().await;

        assert_eq!(
            updates
                .lock()
                .unwrap()
                .last()
                .and_then(|u| u.standing.clone()),
            Some(Standing::ban_only(ban()))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn any_other_refresh_failure_is_not_reported() {
        let source = FakeSource::new(answer(&[]));
        let (updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;
        let before = updates.lock().unwrap().len();

        assert!(!report_if_banned(
            Some(&monitor),
            WALLET,
            &warren_api::TokenClientError::BadDirectoryPolicy
        ));
        settle().await;

        assert_eq!(updates.lock().unwrap().len(), before);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_now_answers_the_standing() {
        let source = FakeSource::new(answer(&["PF-1"]));
        let (_updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);

        let standing = monitor.fetch_now().await.expect("the API answered");

        assert_eq!(standing.strikes.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_now_reports_an_api_failure() {
        let source = FakeSource::new(answer(&[]));
        *source.answer.lock().unwrap() = Err(503);
        let (_updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);

        assert!(matches!(
            monitor.fetch_now().await,
            Err(FetchError::Api(ClientError::ServerStatus {
                status: 503,
                ..
            }))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn without_a_wallet_nothing_is_fetched_and_the_standing_is_forgotten() {
        let source = FakeSource::new(answer(&[]));
        let (updates, sink) = collector();
        let monitor = StandingMonitor::spawn(source.clone(), None, sink);
        settle().await;
        *source.wallet.lock().unwrap() = None;

        assert!(matches!(
            monitor.fetch_now().await,
            Err(FetchError::NoWallet)
        ));
        assert_eq!(source.fetches(), 1);
        assert_eq!(updates.lock().unwrap().last().unwrap().standing, None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_strike_is_announced_once_across_a_daemon_restart() {
        let dir = tempfile::tempdir().unwrap();
        let source = FakeSource::new(answer(&["PF-2026-0001"]));
        let (first_updates, sink) = collector();
        let _first = StandingMonitor::spawn(source.clone(), Some(dir.path().to_owned()), sink);
        settle().await;
        assert_eq!(announced(&first_updates), ["PF-2026-0001"]);

        let (second_updates, sink) = collector();
        let _second = StandingMonitor::spawn(source.clone(), Some(dir.path().to_owned()), sink);
        settle().await;

        assert!(announced(&second_updates).is_empty());
        let ledger = std::fs::read_to_string(dir.path().join(LEDGER_FILE)).unwrap();
        assert!(!ledger.contains("PF-2026-0001"), "{ledger}");
    }
}
