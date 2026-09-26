//! Port-forwarding entitlements for the iOS tunnel (warren-core doc 99,
//! doc 105): an exit refuses a NAT-PMP Map request that carries no entitlement
//! envelope, so without this the iOS forwarded port is refused everywhere.
//!
//! The mint is `warren_standing::entitlements`, shared with the desktop daemon
//! and Android: one manager per wallet, process-lived, topped up on the same
//! coarse timer as the session tokens. Here it is wired to the extension's
//! wallet, transport, runtime and standing, and each rule draws the lowest
//! free slot of its tunnel's table for as long as it lives.
//!
//! The table is per tunnel. A tunnel runs one rule, so its rule always draws
//! slot 0, like Android's, and a rule rebuilt on a new inner address drops its
//! lease before the next is taken and re-presents the envelope the exit
//! already spent for its port. A process-wide table would hand a new tunnel
//! slot 1 whenever the previous tunnel's rule outlived its start, spending a
//! second entitlement of the batch on the same port.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use warren_standing::entitlements::{
    CredentialSource, RuleSlots, SlotLease, SlotSource, bind_slot, recording_presence,
};

/// What one live rule presents, and the slot it holds while it lives.
pub(crate) struct RuleEntitlement {
    /// Freed when the rule is dropped.
    pub(crate) lease: SlotLease,
    /// Read by the engine once per refresh cycle, for both legs of a pair.
    pub(crate) source: CredentialSource,
    /// Whether the last request carried an envelope: an exit's refusal means
    /// something else with and without one.
    pub(crate) presented: Arc<AtomicBool>,
}

/// The entitlement of a new rule, on the lowest slot `slots` has free.
pub(crate) fn rule_entitlement(slots: &RuleSlots, entitlements: SlotSource) -> RuleEntitlement {
    let lease = slots.acquire();
    let presented = Arc::new(AtomicBool::new(false));
    let source = recording_presence(bind_slot(entitlements, lease.slot()), presented.clone());
    RuleEntitlement {
        lease,
        source,
        presented,
    }
}

#[cfg(all(target_os = "ios", feature = "tunnel"))]
pub(crate) use ios::provider_for;

#[cfg(all(target_os = "ios", feature = "tunnel"))]
mod ios {
    use std::sync::{Arc, OnceLock};

    use ed25519_dalek::SigningKey;
    use warren_api::WarrenApiClient;
    use warren_api::reqwest_transport::ReqwestTransport;
    use warren_identity::WarrenIdentity;
    use warren_standing::entitlements::{EntitlementMint, SlotSource};

    static MINT: OnceLock<EntitlementMint<ReqwestTransport>> = OnceLock::new();

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// The entitlement slots of `signing_key`'s wallet, minted by the same
    /// identity as the session tokens. The refresh runs on the process
    /// runtime: a tunnel's own runtime is shut down by `warren_tunnel_stop`,
    /// and the manager outlives it. A ban the issuer answers goes to the
    /// extension's standing, which blocks the tunnel.
    pub(crate) fn provider_for(signing_key: SigningKey) -> SlotSource {
        let mint = MINT.get_or_init(|| {
            let mint = EntitlementMint::new(Arc::new(now_unix_secs)).with_ban_sink(Arc::new(
                |wallet, error| {
                    crate::warren_standing_ffi::tunnel_store().on_refresh_error(
                        wallet,
                        error,
                        now_unix_secs(),
                    )
                },
            ));
            match crate::warren_ios_runtime() {
                Ok(runtime) => mint.with_runtime(runtime),
                Err(_) => mint,
            }
        });
        let wallet_pubkey = signing_key.verifying_key().to_bytes();
        mint.slot_source(wallet_pubkey, move || {
            WarrenApiClient::new(
                warren_product_env::API_URL.to_owned(),
                WarrenIdentity::from_signing_key(signing_key),
                ReqwestTransport::new(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use warren_standing::entitlements::{RuleSlots, SlotSource};

    use super::rule_entitlement;

    /// Slot `n` holds envelope `[n; 4]`, or nothing past the batch of two.
    fn batch_of_two() -> SlotSource {
        Arc::new(|slot| (slot < 2).then(|| vec![u8::try_from(slot).unwrap(); 4]))
    }

    #[test]
    fn a_rule_presents_the_envelope_of_its_own_slot() {
        let slots = RuleSlots::new();
        let first = rule_entitlement(&slots, batch_of_two());
        let second = rule_entitlement(&slots, batch_of_two());

        assert_eq!((first.source)(), Some(vec![0; 4]));
        assert_eq!(
            (second.source)(),
            Some(vec![1; 4]),
            "two live rules never present one envelope"
        );
    }

    /// The re-bind path: the old rule is dropped before the new one is built,
    /// so the new one draws the same slot and presents the envelope the exit
    /// already spent for the port rather than buying a second one.
    #[test]
    fn a_rule_rebuilt_after_its_predecessor_draws_its_slot_again() {
        let slots = RuleSlots::new();
        let before = rule_entitlement(&slots, batch_of_two());
        let spent = (before.source)();

        drop(before);
        let after = rule_entitlement(&slots, batch_of_two());

        assert_eq!(after.lease.slot(), 0);
        assert_eq!((after.source)(), spent);
    }

    #[test]
    fn a_rule_records_whether_its_last_request_carried_an_envelope() {
        let slots = RuleSlots::new();
        let _first = rule_entitlement(&slots, batch_of_two());
        let _second = rule_entitlement(&slots, batch_of_two());
        let third = rule_entitlement(&slots, batch_of_two());

        assert_eq!((third.source)(), None, "slot 2 is past the batch");
        assert!(!third.presented.load(Ordering::Relaxed));

        let slots = RuleSlots::new();
        let carried = rule_entitlement(&slots, batch_of_two());
        assert!((carried.source)().is_some());
        assert!(carried.presented.load(Ordering::Relaxed));
    }
}
