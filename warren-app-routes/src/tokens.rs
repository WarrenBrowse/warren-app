//! The anonymous session tokens a tunnel's sessions lead with (warren-core
//! doc 64), shared by the desktop daemon and the Android engine.
//!
//! The wallet's batch is blinded from its seed, so every client of the wallet
//! holds the same tokens an epoch, and an exit leases each serial to one live
//! session in the whole fleet. Every session of a tunnel (the main one and
//! each route session) therefore opens a provider of its own
//! ([`session_source`]), which holds the serial it leads with
//! ([`TokenManager::claim`]) until the session's supervisor drops it: another
//! session of the process never leads with a held serial, and a session that
//! redials leads with its own serial again, which the exit renews in place.
//! The hold follows the lead, not the serial the exit finally admitted: the
//! engine does not report which token of a walked stack it was admitted on,
//! so after a refusal the hold sits on the refused serial. That is why a held
//! serial goes to the tail of a stack and never out of it: it may be the very
//! serial the exit admitted this session on, the only one it would renew.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use warren_api::{HttpTransport, SerialLease, TokenClientError, TokenManager};
use warrenguard_transport::supervisor::SessionTokenProvider;
use warrenguard_wire::SESSION_TOKEN_LEN;

/// Opens a token provider for each session of a tunnel: one for the main
/// session and one for each route session it starts, so the sessions of one
/// wallet never lead with the same serial (the exit leases a serial to a
/// single live session in the whole fleet).
pub type SessionTokenSource = Arc<dyn Fn() -> SessionTokenProvider + Send + Sync>;

/// Wraps a source of serialized token bytes (one 354-byte Privacy Pass token
/// each, e.g. `warren_api::TokenManager::session_stack`) into a
/// [`SessionTokenProvider`] the multi-hop supervisor consumes. An empty stack
/// keeps the v6 wallet-signed path of a main session.
///
/// The stack is cut to the first [`warrenguard_wire::MAX_SESSION_TOKENS`]: the
/// default admission sends the whole stack in one setup request, and an exit
/// refuses to decode a setup request that carries more.
#[must_use]
pub fn make_session_token_provider(
    take_stack: Arc<dyn Fn() -> Vec<[u8; SESSION_TOKEN_LEN]> + Send + Sync>,
) -> SessionTokenProvider {
    Arc::new(move || {
        take_stack()
            .into_iter()
            .take(warrenguard_wire::MAX_SESSION_TOKENS)
            .map(warrenguard_wire::SessionToken)
            .collect()
    })
}

/// Opens, for each session, a provider over `manager`'s batch that holds the
/// serial the session leads with.
pub fn session_source<T: HttpTransport + Send + Sync + 'static>(
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

/// Announces a finished refresh round of `manager`'s credentials at `now`,
/// and whether the wallet then holds tokens for the epoch: a tunnel started
/// before either the tokens or the route admission key were at hand takes
/// them up from this instead of waiting for its next reconnect, and the first
/// tunnel of a daemon run waits for the first round to finish
/// ([`crate::main_anchor`]).
pub fn announce_refresh<T: HttpTransport>(
    credentials: &tokio::sync::watch::Sender<crate::Credentials>,
    manager: &TokenManager<T>,
    now: u64,
) {
    let has_tokens = !epoch_batch(manager, now).is_empty();
    credentials.send_modify(|announced| {
        announced.rounds = announced.rounds.wrapping_add(1);
        announced.has_tokens = has_tokens;
    });
}

/// Announces, while a refresh round is still running, that the wallet now
/// holds tokens for the epoch of `now`, and says whether it does: a round
/// also mints the later epochs of its horizon, one request each, and the
/// first tunnel of a daemon run needs the current epoch only. Rounds are not
/// counted here.
pub fn announce_minted<T: HttpTransport>(
    credentials: &tokio::sync::watch::Sender<crate::Credentials>,
    manager: &TokenManager<T>,
    now: u64,
) -> bool {
    if epoch_batch(manager, now).is_empty() {
        return false;
    }
    credentials.send_if_modified(|announced| {
        let newly = !announced.has_tokens;
        announced.has_tokens = true;
        newly
    });
    true
}

/// The coarse refresh period of a wallet's credentials, and the first wait
/// after a failed round. A failure doubles the wait, up to the period: the
/// first round runs as the first tunnel connects, and a round lost then left
/// route sessions without a token for the whole period.
pub const REFRESH_PERIOD: Duration = Duration::from_secs(600);
/// See [`REFRESH_PERIOD`].
pub const FIRST_RETRY: Duration = Duration::from_secs(15);
/// How often a running round is looked at for the current epoch's tokens,
/// which a waiting first tunnel can take before the round ends.
pub const MINTED_POLL: Duration = Duration::from_millis(100);

/// Runs `refresh` now and after each wait: [`REFRESH_PERIOD`] once a round
/// succeeds, from [`FIRST_RETRY`] doubling up to the period after a failure.
pub async fn refresh_forever<F, R>(mut refresh: F)
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

/// Runs one refresh round of `manager` and announces the current epoch's
/// tokens as soon as the round has minted them ([`announce_minted`]), every
/// [`MINTED_POLL`], without waiting for the later epochs of its horizon. The
/// round's end is the caller's to announce ([`announce_refresh`]), once what
/// it keeps of the round (the route admission block, a persisted bundle) is
/// in force.
///
/// # Errors
///
/// The round's own failure, [`TokenManager::refresh`]'s.
pub async fn refresh_announcing_mints<T: HttpTransport>(
    manager: &TokenManager<T>,
    credentials: &tokio::sync::watch::Sender<crate::Credentials>,
    now: &(dyn Fn() -> u64 + Sync),
) -> Result<(), TokenClientError> {
    let refresh = manager.refresh(now());
    tokio::pin!(refresh);
    let mut minted = false;
    loop {
        tokio::select! {
            refreshed = &mut refresh => return refreshed,
            () = tokio::time::sleep(MINTED_POLL), if !minted => {
                minted = announce_minted(credentials, manager, now());
            }
        }
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
    use warren_api::{BlindingKey, PersistedTokens, TokenManager, WarrenApiClient};
    use warren_identity::WarrenIdentity;
    use warrenguard_transport::supervisor::SessionTokenProvider;
    use warrenguard_wire::SESSION_TOKEN_LEN;

    use super::{SessionTokenSource, session_source};

    /// The issuer is never asked: every batch comes from a restored bundle.
    struct NoNetwork;

    impl warren_api::HttpTransport for NoNetwork {
        async fn execute(
            &self,
            _request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            panic!("these tests never reach the issuer")
        }
    }

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
        let manager = restored_manager(epochs);
        let clock = Arc::new(AtomicU64::new(NOW));
        let read = Arc::clone(&clock);
        let source = session_source(
            Arc::new(manager),
            Arc::new(move || read.load(Ordering::SeqCst)),
        );
        (source, clock)
    }

    /// A manager holding the wallet's batches for `epochs`.
    fn restored_manager(epochs: &[(u64, &[u8])]) -> TokenManager<NoNetwork> {
        restored_manager_over(NoNetwork, epochs)
    }

    /// A manager holding the wallet's batches for `epochs`, whose refresh
    /// rounds go to `issuer`.
    fn restored_manager_over<T: warren_api::HttpTransport>(
        issuer: T,
        epochs: &[(u64, &[u8])],
    ) -> TokenManager<T> {
        let seed = [0x42; 32];
        let manager = TokenManager::new(
            Arc::new(WarrenApiClient::new(
                "https://api.example.test",
                WarrenIdentity::from_seed(&seed),
                issuer,
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
        manager
    }

    #[test]
    fn the_first_tokens_of_a_round_are_announced_before_it_ends() {
        let (credentials, followed) = tokio::sync::watch::channel(crate::Credentials::default());

        assert!(!super::announce_minted(
            &credentials,
            &restored_manager(&[(EPOCH - 1, &[1])]),
            NOW
        ));
        assert_eq!(*followed.borrow(), crate::Credentials::default());

        assert!(super::announce_minted(
            &credentials,
            &restored_manager(&[(EPOCH, &[1])]),
            NOW
        ));
        assert_eq!(
            *followed.borrow(),
            crate::Credentials {
                rounds: 0,
                has_tokens: true
            },
            "a round in progress is not counted as finished"
        );
    }

    #[test]
    fn each_refresh_round_is_announced_with_whether_the_wallet_holds_tokens() {
        let (credentials, followed) = tokio::sync::watch::channel(crate::Credentials::default());

        super::announce_refresh(&credentials, &restored_manager(&[(EPOCH - 1, &[1])]), NOW);
        assert_eq!(
            *followed.borrow(),
            crate::Credentials {
                rounds: 1,
                has_tokens: false
            },
            "only an earlier epoch's batch"
        );

        super::announce_refresh(&credentials, &restored_manager(&[(EPOCH, &[1, 2])]), NOW);
        assert_eq!(
            *followed.borrow(),
            crate::Credentials {
                rounds: 2,
                has_tokens: true
            }
        );
    }

    /// An issuer that never answers, so a round over it keeps running.
    struct Unanswered;

    impl warren_api::HttpTransport for Unanswered {
        async fn execute(
            &self,
            _request: warren_api::HttpRequest,
        ) -> Result<warren_api::HttpResponse, warren_api::TransportError> {
            std::future::pending().await
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_running_round_announces_the_current_epochs_tokens_before_it_ends() {
        let (credentials, followed) = tokio::sync::watch::channel(crate::Credentials::default());
        let manager = restored_manager_over(Unanswered, &[(EPOCH, &[1])]);

        let round = super::refresh_announcing_mints(&manager, &credentials, &|| NOW);
        let ended = tokio::time::timeout(std::time::Duration::from_secs(1), round).await;

        assert!(ended.is_err(), "the round is still running");
        assert_eq!(
            *followed.borrow(),
            crate::Credentials {
                rounds: 0,
                has_tokens: true
            }
        );
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

    fn batch_of_three() -> SessionTokenSource {
        source_over(&[(EPOCH, &[1, 2, 3])]).0
    }

    fn lead(provider: &SessionTokenProvider) -> [u8; SESSION_TOKEN_LEN] {
        provider().first().expect("a token").0
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
}
