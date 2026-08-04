//! Long-lived port-entitlement minting for the app tunnel (warren-core doc 99).
//!
//! Mirror of [`crate::warren_token_provider`] for the second credential class:
//! one [`warren_api::PortEntitlementManager`] per wallet (process-lived, keyed
//! by the ss58 address), a background refresh on a coarse timer so issuance
//! timing does not mirror anything the user does, and a provider closure the
//! tunnel hands to each port-forwarding rule.
//!
//! The provider is keyed by SLOT, not by rule: one entitlement buys one
//! forwarded port, so the tunnel's controller owns which rule holds which
//! slot and this module only answers "what does slot n hold right now".
//! An exhausted batch answers `None`, and the exit then applies its
//! configured per-client quota.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use talpid_warren_tunnel::PortEntitlementProvider;
use warren_api::{PortEntitlementManager, WarrenApiClient};
use warren_identity::WarrenIdentity;

use crate::warren_api_transport::WarrenApiTransport;
use crate::warren_sdk_client::SharedWarrenSeed;

type Manager = PortEntitlementManager<WarrenApiTransport>;

/// One manager per wallet, reused across reconnects so its per-epoch batch and
/// its slot assignments survive between sessions: a rule that reconnects must
/// present the credential the exit already spent for it, not a fresh one.
static MANAGERS: OnceLock<Mutex<HashMap<String, Arc<Manager>>>> = OnceLock::new();

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn spawn_refresh(manager: Arc<Manager>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            if let Err(e) = manager.refresh_auto(now_unix_secs()).await {
                log::warn!("Warren port-entitlement refresh failed (keeping existing): {e}");
            }
        }
    });
}

/// The entitlement provider for `seed`'s wallet against `api_url`.
pub(crate) fn provider_for(api_url: &str, seed: &SharedWarrenSeed) -> PortEntitlementProvider {
    let seed_bytes: [u8; 32] = **seed.read().expect("warren seed RwLock poisoned");
    let key = WarrenIdentity::from_seed(&seed_bytes).address();

    let map = MANAGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let manager = {
        let mut guard = map.lock().expect("entitlement manager map poisoned");
        guard
            .entry(key)
            .or_insert_with(|| {
                let client = WarrenApiClient::new(
                    api_url.to_owned(),
                    WarrenIdentity::from_seed(&seed_bytes),
                    WarrenApiTransport::new(),
                );
                let manager = Arc::new(PortEntitlementManager::new(Arc::new(client)));
                spawn_refresh(manager.clone());
                manager
            })
            .clone()
    };

    Arc::new(move |slot| manager.credential_for_slot(slot, now_unix_secs()))
}
