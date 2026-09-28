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
//! read (right after a daemon start) anchors with it rather than not at all.
//! Every refresh round is announced ([`credentials_for`]), so a tunnel that
//! started without the key or without tokens takes them up when they arrive
//! (`talpid_warren_tunnel::app_routes::main_anchor`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use talpid_warren_tunnel::app_routes::admission::remember_route_admission;
pub(crate) use talpid_warren_tunnel::app_routes::admission::{
    DirectoryRouteAdmission, REMEMBERED_ROUTE_ADMISSION, RouteKemTrust,
};
pub(crate) use talpid_warren_tunnel::app_routes::tokens::session_source;
use talpid_warren_tunnel::app_routes::tokens::{
    ISSUED_ELSEWHERE_WARNING, IssuedElsewhereNotice, announce_refresh, refresh_announcing_mints,
    refresh_forever,
};
use talpid_warren_tunnel::{
    SessionTokenSource,
    app_routes::{Credentials, RouteAdmissionSource},
};
use tokio::sync::watch;
use warren_api::{BlindingKey, TokenManager, WarrenApiClient};
use warren_identity::WarrenIdentity;

use crate::warren_account_standing::StandingMonitor;
use crate::warren_api_transport::WarrenApiTransport;
use crate::warren_sdk_client::SharedWarrenSeed;
use crate::warren_wallet_activity::WalletActivity;

type Manager = TokenManager<WarrenApiTransport>;

/// One manager per wallet, reused across reconnects so its RAM token store,
/// its once-per-epoch issuance bookkeeping and its held serials survive
/// between sessions, with the announcement of each of its refresh rounds.
static MANAGERS: OnceLock<Mutex<HashMap<String, Wallet>>> = OnceLock::new();

#[derive(Clone)]
struct Wallet {
    manager: Arc<Manager>,
    credentials: watch::Sender<Credentials>,
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn spawn_refresh(
    manager: Arc<Manager>,
    credentials: watch::Sender<Credentials>,
    wallet: String,
    standing: Option<StandingMonitor>,
    remembered_at: Option<PathBuf>,
    activity: WalletActivity,
) {
    // The manager mints only epochs it has not minted yet.
    let notice = Arc::new(IssuedElsewhereNotice::default());
    tokio::spawn(refresh_forever(move || {
        let manager = Arc::clone(&manager);
        let notice = Arc::clone(&notice);
        let credentials = credentials.clone();
        let wallet = wallet.clone();
        let standing = standing.clone();
        let remembered_at = remembered_at.clone();
        let activity = activity.clone();
        async move {
            // The round also reads the route admission block, so a logged-out
            // daemon does neither.
            activity
                .park_while_inactive("Warren v7 token refresh")
                .await;
            let refreshed = refresh_announcing_mints(&manager, &credentials, &now_unix_secs).await;
            if let Some(path) = remembered_at.as_deref()
                && manager.epoch_at(now_unix_secs()).is_some()
            {
                remember_route_admission(path, manager.route_admission().as_ref());
            }
            announce_refresh(&credentials, &manager, now_unix_secs());
            if notice.due(&manager, now_unix_secs()) {
                log::warn!("{ISSUED_ELSEWHERE_WARNING}");
            }
            match refreshed {
                Ok(_) => {
                    log::debug!("Warren v7 token refresh round succeeded");
                    true
                }
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
    activity: &WalletActivity,
) -> SessionTokenSource {
    session_source(
        wallet_for(api_url, seed, standing, trust, activity).manager,
        Arc::new(now_unix_secs),
    )
}

/// The announcement of each refresh round of `seed`'s wallet, from the same
/// manager as [`source_for`]'s.
pub(crate) fn credentials_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
    activity: &WalletActivity,
) -> watch::Receiver<Credentials> {
    wallet_for(api_url, seed, standing, trust, activity)
        .credentials
        .subscribe()
}

/// Route admission by anchor for `seed`'s wallet against `api_url`, as the
/// token directory the wallet's manager last fetched announces it, its key
/// signed by one of `trust`'s pins. The same manager as [`source_for`]'s.
pub(crate) fn route_admission_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
    activity: &WalletActivity,
) -> Arc<dyn RouteAdmissionSource> {
    Arc::new(DirectoryRouteAdmission::new(
        wallet_for(api_url, seed, standing, trust, activity).manager,
        trust.clone(),
        Arc::new(now_unix_secs),
    ))
}

/// The wallet's manager, built and refreshed from its first use, while
/// `activity` lets the wallet's API work run.
fn wallet_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    trust: &RouteKemTrust,
    activity: &WalletActivity,
) -> Wallet {
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
            let credentials = watch::Sender::new(Credentials::default());
            spawn_refresh(
                manager.clone(),
                credentials.clone(),
                key,
                standing.cloned(),
                trust.remembered_at.clone(),
                activity.clone(),
            );
            Wallet {
                manager,
                credentials,
            }
        })
        .clone()
}
