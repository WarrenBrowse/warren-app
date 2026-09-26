//! Port-forwarding entitlements as every client mints and presents them
//! (warren-core doc 99, doc 105): the one piece the desktop daemon, the
//! Android tunnel and the iOS tunnel each used to carry a copy of.
//!
//! One [`PortEntitlementManager`] per wallet, process-lived: reused across
//! reconnects so a rule that reconnects presents the entitlement the exit
//! already spent for it, instead of buying a second port. A background task
//! tops the batch up on a coarse timer, so issuance timing mirrors neither the
//! user enabling port forwarding nor the session the entitlement is spent on.
//!
//! The source is keyed by SLOT, not by rule: one entitlement buys one
//! forwarded port, so the caller owns which rule holds which slot
//! ([`RuleSlots`]) and this module only answers "what does slot n present
//! right now". Both legs of a TCP+UDP pair are one port, and the engine asks
//! the source once per refresh cycle for the pair.
//!
//! An exhausted batch, an unreachable API and a wallet the issuer refuses all
//! answer `None`. The Map request then goes out without an envelope, and the
//! exit refuses it: the attribution tag inside the envelope is mandatory for a
//! forwarded port (doc 105). A ban the issuer answers goes to the ban sink.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use warren_api::{HttpTransport, PortEntitlementManager, TokenClientError, WarrenApiClient};

/// Unix-seconds clock seam. The clock is a system boundary: tests drive epochs
/// deterministically, production wires the system clock.
pub type NowFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Told of every refresh failure with the wallet it concerns; answers whether
/// it was a ban, which the caller then enforces.
pub type BanSink = Arc<dyn Fn(&[u8; 32], &TokenClientError) -> bool + Send + Sync>;

/// What one rule presents on a NAT-PMP cycle, or `None` for a bare request.
/// Structurally the engine's `warrenguard_natpmp_client::CredentialProvider`,
/// spelled out here so this crate stays free of the engine.
pub type CredentialSource = Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>;

/// What slot `n` of a wallet's batch presents right now.
pub type SlotSource = Arc<dyn Fn(usize) -> Option<Vec<u8>> + Send + Sync>;

/// How often the batch is topped up, the session-token mint's cadence.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(600);

/// How long a mobile tunnel's first mapping request waits for the mint before
/// going out bare ([`await_first_credential`]). Sized off the observed
/// cold-start cost of the v7 token mint on Android's protected transport
/// (~2.5 s on the emulator), with margin for a slow mobile network.
pub const FIRST_CREDENTIAL_GRACE: Duration = Duration::from_secs(8);

/// How often that wait looks at the slot again.
pub const FIRST_CREDENTIAL_POLL: Duration = Duration::from_millis(250);

/// The slot the refresh log reports the stock of. Probing assigns it its
/// credential right after the refresh, which is idempotent for the epoch: the
/// first rule of every client draws it, so the first request of the next
/// session no longer races the mint.
const PROBE_SLOT: usize = 0;

/// Process-lived registry of one [`PortEntitlementManager`] per wallet.
pub struct EntitlementMint<T> {
    now: NowFn,
    managers: Mutex<HashMap<[u8; 32], Arc<PortEntitlementManager<T>>>>,
    ban_sink: Option<BanSink>,
    runtime: Option<tokio::runtime::Handle>,
}

impl<T: HttpTransport + 'static> EntitlementMint<T> {
    pub fn new(now: NowFn) -> Self {
        Self {
            now,
            managers: Mutex::new(HashMap::new()),
            ban_sink: None,
            runtime: None,
        }
    }

    /// Reports every refresh failure to `sink`: the entitlement issuer refuses
    /// a banned wallet the way the token issuer does.
    #[must_use]
    pub fn with_ban_sink(mut self, sink: BanSink) -> Self {
        self.ban_sink = Some(sink);
        self
    }

    /// Runs the background refresh on `runtime` rather than on the caller's.
    /// For a caller whose runtime is shorter-lived than the mint: a refresh
    /// spawned on a tunnel's runtime dies with that tunnel, and the manager,
    /// which outlives it, would never be topped up again.
    #[must_use]
    pub fn with_runtime(mut self, runtime: tokio::runtime::Handle) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// The slot source of `wallet_pubkey`. First sight of a wallet builds its
    /// manager (via `make_client`, which owns the wallet identity) and starts
    /// its background refresh; later calls reuse both, so the factory runs at
    /// most once per wallet and process.
    pub fn slot_source(
        &self,
        wallet_pubkey: [u8; 32],
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> SlotSource {
        let manager = self.manager(wallet_pubkey, make_client);
        let now = self.now.clone();
        Arc::new(move |slot| manager.credential_for_slot(slot, now()))
    }

    /// What rule `slot` of `wallet_pubkey` presents, per [`Self::slot_source`].
    pub fn credential_source(
        &self,
        wallet_pubkey: [u8; 32],
        slot: usize,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> CredentialSource {
        bind_slot(self.slot_source(wallet_pubkey, make_client), slot)
    }

    fn manager(
        &self,
        wallet_pubkey: [u8; 32],
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> Arc<PortEntitlementManager<T>> {
        let mut managers = self.managers.lock().unwrap_or_else(PoisonError::into_inner);
        managers
            .entry(wallet_pubkey)
            .or_insert_with(|| {
                let manager = Arc::new(PortEntitlementManager::new(Arc::new(make_client())));
                let refresh = refresh_forever(
                    manager.clone(),
                    self.now.clone(),
                    wallet_pubkey,
                    self.ban_sink.clone(),
                );
                match &self.runtime {
                    Some(runtime) => {
                        runtime.spawn(refresh);
                    }
                    None => {
                        tokio::spawn(refresh);
                    }
                }
                manager
            })
            .clone()
    }
}

/// What rule `slot` presents, read from `source` on every cycle.
pub fn bind_slot(source: SlotSource, slot: usize) -> CredentialSource {
    Arc::new(move || source(slot))
}

/// The first tick fires immediately (stock as soon as a wallet is seen), then
/// every [`REFRESH_INTERVAL`]. The manager only mints epochs it has not
/// attempted yet, so in steady state a tick costs one unsigned directory
/// fetch.
async fn refresh_forever<T: HttpTransport + 'static>(
    manager: Arc<PortEntitlementManager<T>>,
    now: NowFn,
    wallet_pubkey: [u8; 32],
    ban_sink: Option<BanSink>,
) {
    let mut tick = tokio::time::interval(REFRESH_INTERVAL);
    loop {
        tick.tick().await;
        let n = now();
        match manager.refresh_auto(n).await {
            // Presence only, never the credential: a refresh that answered 200
            // and stocked nothing (the issuer already served this account this
            // epoch, or refused it) is otherwise indistinguishable from one
            // that did, and shows up only as a Map request the exit refuses.
            Ok(()) => log::info!(
                "Warren port-entitlement refresh ok (slot stocked={})",
                manager.credential_for_slot(PROBE_SLOT, n).is_some()
            ),
            // A ban refusal goes to the standing, which blocks the tunnel;
            // anything else is transient, the batch keeps vending what it
            // already holds and the next tick retries. The error chain carries
            // no credential or seed material.
            Err(e) => {
                if ban_sink
                    .as_ref()
                    .is_some_and(|sink| sink(&wallet_pubkey, &e))
                {
                    log::warn!("Warren port-entitlement refresh refused: the account is banned");
                } else {
                    log::warn!("Warren port-entitlement refresh failed (keeping existing): {e}");
                }
            }
        }
    }
}

/// Waits for `source` to vend a credential, giving up after `grace`.
///
/// The engine reads the credential once per NAT-PMP cycle, and the first cycle
/// starts milliseconds after the handshake. A mobile tunnel process is often
/// created at connect, so its mint is cold and that first request would go
/// out bare, which the exit refuses. The wait runs off the datapath (the tunnel
/// already carries traffic) and it is bounded: a mint that never lands must
/// delay the mapping, never hang it.
pub async fn await_first_credential(
    source: &CredentialSource,
    grace: Duration,
    poll: Duration,
) -> Option<Vec<u8>> {
    let deadline = tokio::time::Instant::now() + grace;
    loop {
        if let Some(credential) = source() {
            return Some(credential);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(poll).await;
    }
}

/// Wraps `source` so `presented` records whether the last request carried an
/// entitlement, which is what tells a refused entitlement from a missing one
/// ([`crate::PortRefusal::of_request`]).
pub fn recording_presence(
    source: CredentialSource,
    presented: Arc<AtomicBool>,
) -> CredentialSource {
    Arc::new(move || {
        let credential = source();
        presented.store(credential.is_some(), Ordering::Relaxed);
        credential
    })
}

/// Which slot each live rule holds: the lowest free one, freed with the rule.
///
/// Two live rules never share a slot, or the exit would read them as one
/// port. Reusing a freed slot rather than growing matters because the batch
/// is bounded: slots that only grew would run past it after a few rule
/// changes. A rule rebuilt on the same epoch (a re-bind, a reconnect) gets its
/// old slot back and presents the entitlement the exit already spent for it.
#[derive(Clone, Default)]
pub struct RuleSlots {
    taken: Arc<Mutex<BTreeSet<usize>>>,
}

impl RuleSlots {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the lowest free slot, held until the lease drops.
    #[must_use]
    pub fn acquire(&self) -> SlotLease {
        let mut taken = self.taken.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = (0..).find(|n| !taken.contains(n)).unwrap_or(usize::MAX);
        taken.insert(slot);
        SlotLease {
            slot,
            taken: Arc::clone(&self.taken),
        }
    }
}

/// A rule's hold on its slot ([`RuleSlots::acquire`]).
#[derive(Debug)]
pub struct SlotLease {
    slot: usize,
    taken: Arc<Mutex<BTreeSet<usize>>>,
}

impl SlotLease {
    #[must_use]
    pub fn slot(&self) -> usize {
        self.slot
    }
}

impl Drop for SlotLease {
    fn drop(&mut self) {
        self.taken
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.slot);
    }
}

#[cfg(test)]
mod tests;
