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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use talpid_warren_tunnel::{SESSION_TOKEN_LEN, SessionTokenSource, make_session_token_provider};
use warren_api::{BlindingKey, HttpTransport, SerialLease, TokenManager, WarrenApiClient};
use warren_identity::WarrenIdentity;

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

fn spawn_refresh(manager: Arc<Manager>) {
    // The manager mints only epochs it has not minted yet.
    tokio::spawn(refresh_forever(move || {
        let manager = Arc::clone(&manager);
        async move {
            match manager.refresh(now_unix_secs()).await {
                Ok(_) => true,
                Err(e) => {
                    log::warn!("Warren v7 token refresh failed (keeping existing tokens): {e}");
                    false
                }
            }
        }
    }));
}

/// The v7 token source for `seed`'s wallet against `api_url`. Builds (and
/// starts refreshing) a manager the first time a wallet is seen; reuses it
/// after. The providers it opens never mint.
pub(crate) fn source_for(api_url: &str, seed: &SharedWarrenSeed) -> SessionTokenSource {
    let seed_bytes = seed.read().unwrap_or_else(PoisonError::into_inner);
    let identity = WarrenIdentity::from_seed(&seed_bytes);
    let key = identity.address();

    let map = MANAGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let manager = {
        let mut guard = map.lock().expect("token manager map poisoned");
        guard
            .entry(key)
            .or_insert_with(|| {
                let client =
                    WarrenApiClient::new(api_url.to_owned(), identity, WarrenApiTransport::new());
                let manager = Arc::new(TokenManager::new(
                    Arc::new(client),
                    BlindingKey::session(&seed_bytes),
                ));
                spawn_refresh(manager.clone());
                manager
            })
            .clone()
    };
    session_source(manager, Arc::new(now_unix_secs))
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
        let mut outcomes = outcomes.to_vec().into_iter();
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
}
