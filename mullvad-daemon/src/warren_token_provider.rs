//! Long-lived v7 anonymous-token minting for the app tunnel (Privacy Pass,
//! warren-core doc 64).
//!
//! Holds one [`warren_api::TokenManager`] per wallet (process-lived, keyed by
//! the ss58 address), spawns a background refresh task the first time a wallet
//! is seen (mint the current epoch + prefetch horizon on a coarse timer, never
//! at connect, so issuance timing does not mirror session timing), and hands
//! the tunnel a provider closure.
//!
//! v7 is the default: [`provider_for`] is set on every assembled
//! [`talpid_warren_tunnel::WarrenTunnelParameters`]. On token exhaustion (or a
//! logged-out sentinel wallet with no subscription) the provider returns an
//! empty stack and the supervisor falls back to the v6 wallet-signed path, so
//! the tunnel always works.
//!
//! An issuer that refuses the wallet as banned is reported to the account
//! standing monitor, which blocks the tunnel with the suspension before any
//! exit is dialed (warren-core doc 105 §5.3).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use talpid_warren_tunnel::{SessionTokenProvider, make_session_token_provider};
use warren_api::{TokenManager, WarrenApiClient};
use warren_identity::WarrenIdentity;

use crate::warren_account_standing::StandingMonitor;
use crate::warren_api_transport::WarrenApiTransport;
use crate::warren_sdk_client::SharedWarrenSeed;

type Manager = TokenManager<WarrenApiTransport>;

/// One manager per wallet, reused across reconnects so its RAM token store and
/// its once-per-epoch issuance bookkeeping survive between sessions.
static MANAGERS: OnceLock<Mutex<HashMap<String, Arc<Manager>>>> = OnceLock::new();

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn spawn_refresh(manager: Arc<Manager>, wallet: String, standing: Option<StandingMonitor>) {
    tokio::spawn(async move {
        // First tick fires immediately (top up before the first connect), then
        // every 10 min. The manager mints only epochs it has not minted yet.
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            if let Err(e) = manager.refresh_auto(now_unix_secs()).await {
                log::warn!("Warren v7 token refresh failed (keeping existing tokens): {e}");
                crate::warren_account_standing::report_if_banned(standing.as_ref(), &wallet, &e);
            }
        }
    });
}

/// The v7 token provider for `seed`'s wallet against `api_url`. Builds (and
/// starts refreshing) a manager the first time a wallet is seen; reuses it
/// after. The returned closure pops one token per session and never mints.
/// A ban the issuer answers goes to `standing`.
pub(crate) fn provider_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
) -> SessionTokenProvider {
    let seed_bytes: [u8; 32] = **seed.read().expect("warren seed RwLock poisoned");
    let key = WarrenIdentity::from_seed(&seed_bytes).address();

    let map = MANAGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let manager = {
        let mut guard = map.lock().expect("token manager map poisoned");
        guard
            .entry(key.clone())
            .or_insert_with(|| {
                let client = WarrenApiClient::new(
                    api_url.to_owned(),
                    WarrenIdentity::from_seed(&seed_bytes),
                    WarrenApiTransport::new(),
                );
                let manager = Arc::new(TokenManager::new(Arc::new(client)));
                spawn_refresh(manager.clone(), key, standing.cloned());
                manager
            })
            .clone()
    };

    make_session_token_provider(Arc::new(move || {
        manager.take_current_stack(now_unix_secs())
    }))
}
