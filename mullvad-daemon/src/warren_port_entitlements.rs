//! Port-entitlement minting for the app tunnel (warren-core doc 99, doc 105).
//!
//! The mint is `warren_standing::entitlements`, shared with Android and iOS:
//! one manager per wallet, process-lived, topped up on a coarse timer. This
//! module wires it to the daemon's wallet, transport and standing monitor, and
//! hands the tunnel a SLOT source: the tunnel's controller owns which rule
//! holds which slot. An exhausted batch answers `None`, the exit refuses the
//! rule's bare Map request, and the controller says so on the rule and asks
//! again later. A refused entitlement moves the rule's slot to another one of
//! the batch, which the wallet's other devices also hold.

use std::sync::{Arc, OnceLock};

use talpid_warren_tunnel::PortEntitlementProvider;
use warren_api::{BlindingKey, WarrenApiClient};
use warren_identity::WarrenIdentity;
use warren_standing::entitlements::EntitlementMint;

use crate::warren_account_standing::StandingMonitor;
use crate::warren_api_transport::WarrenApiTransport;
use crate::warren_sdk_client::SharedWarrenSeed;
use crate::warren_wallet_activity::WalletActivity;

/// Process-lived, so a rule that reconnects presents the entitlement the exit
/// already spent for it rather than a fresh one.
static MINT: OnceLock<EntitlementMint<WarrenApiTransport>> = OnceLock::new();

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The entitlement provider for `seed`'s wallet against `api_url`. A ban the
/// issuer answers goes to `standing`, the daemon's one monitor. The mint's
/// refresh runs only while `activity` lets the wallet's API work run.
pub(crate) fn provider_for(
    api_url: &str,
    seed: &SharedWarrenSeed,
    standing: Option<&StandingMonitor>,
    activity: &WalletActivity,
) -> PortEntitlementProvider {
    let seed_bytes: [u8; 32] = **seed.read().expect("warren seed RwLock poisoned");
    let wallet_pubkey = WarrenIdentity::from_seed(&seed_bytes).public_key();
    // Derived from the wallet, so a daemon restart is served the batch the
    // account already holds (warren-core doc 99 section 4 bis).
    let blinding = BlindingKey::port_entitlement(&seed_bytes);

    let mint = MINT.get_or_init(|| {
        let standing = standing.cloned();
        EntitlementMint::new(Arc::new(now_unix_secs))
            .with_ban_sink(Arc::new(move |wallet, error| {
                crate::warren_account_standing::report_if_banned(
                    standing.as_ref(),
                    &warren_identity::ss58::encode(wallet),
                    error,
                )
            }))
            .with_activity(activity.subscribe())
    });
    let api_url = api_url.to_owned();
    mint.slot_source(wallet_pubkey, blinding, move || {
        crate::warren_api_clock::shared().attach(WarrenApiClient::new(
            api_url,
            WarrenIdentity::from_seed(&seed_bytes),
            WarrenApiTransport::new(),
        ))
    })
}
