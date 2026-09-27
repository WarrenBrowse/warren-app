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
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use talpid_warren_tunnel::app_routes::admission::remember_route_admission;
pub(crate) use talpid_warren_tunnel::app_routes::admission::{
    DirectoryRouteAdmission, REMEMBERED_ROUTE_ADMISSION, RouteKemTrust,
};
pub(crate) use talpid_warren_tunnel::app_routes::tokens::session_source;
use talpid_warren_tunnel::{SessionTokenSource, app_routes::RouteAdmissionSource};
use warren_api::{BlindingKey, TokenManager, WarrenApiClient};
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

#[cfg(test)]
mod tests {
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
}
