//! Long-lived v7 anonymous-token minting for the iOS tunnel (Privacy Pass,
//! warren-core doc 64), the Network-Extension twin of the desktop/Android daemon's
//! `warren_token_provider`.
//!
//! Holds one [`warren_api::TokenManager`] per wallet (process-lived, keyed by
//! the wallet public key), spawns a background refresh task the first time a
//! wallet is seen (mint the current epoch + prefetch horizon on a coarse timer,
//! never at connect), and hands the supervisor a provider closure.
//!
//! The identity is built from the SAME Ed25519 signing key the tunnel uses for
//! its handshake ([`WarrenIdentity::from_signing_key`], no re-derivation), so
//! the minting wallet is bit-for-bit the subscribed wallet, and the batch is
//! blinded from the wallet seed, so every client of the wallet holds the same
//! tokens. An exit leases each serial to one live session in the whole fleet:
//! every dial is handed the whole current-epoch batch and the engine walks it,
//! redialling with the next token when the exit refuses one. Nothing is
//! consumed. v7 is the default; with no token this epoch the provider returns
//! an empty stack and the supervisor falls back to the v6 wallet-signed path.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ed25519_dalek::SigningKey;
use warren_api::reqwest_transport::ReqwestTransport;
use warren_api::{BlindingKey, TokenManager, WarrenApiClient};
use warren_identity::WarrenIdentity;
use warrenguard_transport::supervisor::SessionTokenProvider;
use warrenguard_wire::SessionToken;

/// warren-api base, matching `warren_account_ffi`'s subscription/issuance host.
const WARREN_API_URL: &str = warren_product_env::API_URL;

type Manager = TokenManager<ReqwestTransport>;

/// One manager per wallet, reused across reconnects so its RAM token store and
/// once-per-epoch issuance bookkeeping survive between sessions.
static MANAGERS: OnceLock<Mutex<HashMap<[u8; 32], Arc<Manager>>>> = OnceLock::new();

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn spawn_refresh(manager: Arc<Manager>, wallet_pubkey: [u8; 32]) {
    // On the process runtime: `provider_for` runs on a tunnel's own runtime,
    // which `warren_tunnel_stop` shuts down, and the manager outlives it in
    // `MANAGERS`, so a refresh spawned there would die with the first tunnel
    // and never be started again. The next session in the same extension
    // process would then learn of no ban and top up no token.
    let refresh = async move {
        // First tick fires immediately (top up before the first connect), then
        // every 10 min. The manager mints only epochs it has not minted yet.
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            let now = now_unix_secs();
            if let Err(e) = manager.refresh(now).await {
                // The issuer refuses a banned wallet (warren-core doc 105 §5.3):
                // that goes to the standing, which blocks the tunnel before an
                // exit is dialed. A v7 client never presents its wallet to an
                // exit, so this is how it learns of the ban at all.
                if crate::warren_standing_ffi::tunnel_store().on_refresh_error(
                    &wallet_pubkey,
                    &e,
                    now,
                ) {
                    tracing::warn!("Warren v7 token refresh refused: the account is banned");
                } else {
                    tracing::warn!(error = %e, "Warren v7 token refresh failed (keeping existing tokens)");
                }
            }
        }
    };
    match crate::warren_ios_runtime() {
        Ok(runtime) => {
            runtime.spawn(refresh);
        }
        Err(_) => {
            tokio::spawn(refresh);
        }
    }
}

/// The v7 token provider for `signing_key`'s wallet, whose session blinding
/// key is `blinding`. Builds (and starts refreshing) a manager the first time
/// a wallet is seen; reuses it after. The returned closure hands each dial the
/// current batch, at most what one setup request may carry, and never mints.
pub(crate) fn provider_for(signing_key: SigningKey, blinding: BlindingKey) -> SessionTokenProvider {
    let pubkey = signing_key.verifying_key().to_bytes();

    let map = MANAGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let manager = {
        let mut guard = map.lock().expect("token manager map poisoned");
        guard
            .entry(pubkey)
            .or_insert_with(|| {
                let client = WarrenApiClient::new(
                    WARREN_API_URL.to_owned(),
                    WarrenIdentity::from_signing_key(signing_key),
                    ReqwestTransport::new(),
                );
                let manager = Arc::new(TokenManager::new(Arc::new(client), blinding));
                spawn_refresh(manager.clone(), pubkey);
                manager
            })
            .clone()
    };

    Arc::new(move || {
        manager
            .session_stack(now_unix_secs())
            .into_iter()
            .take(warrenguard_wire::MAX_SESSION_TOKENS)
            .map(SessionToken)
            .collect()
    })
}
