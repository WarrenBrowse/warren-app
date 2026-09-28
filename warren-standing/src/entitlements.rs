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
//! The batch is derived from the wallet ([`BlindingKey::port_entitlement`]),
//! so a restarted process (a daemon restart, an app update, a reboot) and
//! another device of the wallet are served the batch the account holds
//! instead of `already_issued` for the rest of the 48 h prefetch window. They
//! hold the SAME entitlements, and an exit leases one serial to one port
//! fleet-wide, so a rule whose presented entitlement is refused reports it
//! ([`RuleCredential::on_refused`]) and moves to one no other rule of this
//! process holds. The cap is the batch: five ports per subscriber, shared
//! across its devices.
//!
//! An exhausted batch, an unreachable API and a wallet the issuer refuses all
//! answer `None`. The Map request then goes out without an envelope, and the
//! exit refuses it: the attribution tag inside the envelope is mandatory for a
//! forwarded port (doc 105). A ban the issuer answers goes to the ban sink.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use warren_api::{
    BlindingKey, HttpTransport, PortEntitlementManager, TokenClientError, WarrenApiClient,
};

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

/// A wallet's batch as the rules of one process see it, by slot.
pub trait SlotBatch: Send + Sync {
    /// What slot `slot` presents right now, or `None` for a bare request.
    fn credential(&self, slot: usize) -> Option<Vec<u8>>;
    /// An exit refused `presented`, what `slot` sent: the slot moves to an
    /// entitlement no other slot holds.
    fn refused(&self, slot: usize, presented: &[u8]);
    /// A rule takes `slot`. Answers the claim its release names.
    fn hold(&self, slot: usize) -> u64;
    /// The rule that took `slot` with `claim` is gone: the slot's entitlement
    /// is free for a refused rule to move to. Ignored when another rule took
    /// the slot since, whose entitlement it would take away.
    fn release(&self, slot: usize, claim: u64);
}

/// A batch that answers from a function and cannot move a slot: fixed
/// answers, as a test stands in for the mint.
impl<F: Fn(usize) -> Option<Vec<u8>> + Send + Sync> SlotBatch for F {
    fn credential(&self, slot: usize) -> Option<Vec<u8>> {
        self(slot)
    }

    fn refused(&self, _slot: usize, _presented: &[u8]) {}

    fn hold(&self, _slot: usize) -> u64 {
        0
    }

    fn release(&self, _slot: usize, _claim: u64) {}
}

/// What each slot of a wallet's batch presents, shared by the rules.
pub type SlotSource = Arc<dyn SlotBatch>;

/// The mint's own batch: one manager per wallet, read at the mint's clock,
/// and which rule last took each slot.
struct MintedSlots<T> {
    manager: Arc<PortEntitlementManager<T>>,
    now: NowFn,
    holders: Mutex<Holders>,
}

#[derive(Default)]
struct Holders {
    next: u64,
    by_slot: HashMap<usize, u64>,
}

impl<T: HttpTransport + 'static> SlotBatch for MintedSlots<T> {
    fn credential(&self, slot: usize) -> Option<Vec<u8>> {
        self.manager.credential_for_slot(slot, (self.now)())
    }

    fn refused(&self, slot: usize, presented: &[u8]) {
        self.manager.mark_refused(slot, presented, (self.now)());
    }

    fn hold(&self, slot: usize) -> u64 {
        let mut holders = self.holders.lock().unwrap_or_else(PoisonError::into_inner);
        holders.next = holders.next.wrapping_add(1);
        let claim = holders.next;
        holders.by_slot.insert(slot, claim);
        claim
    }

    fn release(&self, slot: usize, claim: u64) {
        let mut holders = self.holders.lock().unwrap_or_else(PoisonError::into_inner);
        if holders.by_slot.get(&slot) == Some(&claim) {
            holders.by_slot.remove(&slot);
            self.manager.release(slot);
        }
    }
}

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
    managers: Mutex<HashMap<[u8; 32], Arc<MintedSlots<T>>>>,
    ban_sink: Option<BanSink>,
    runtime: Option<tokio::runtime::Handle>,
    activity: Option<tokio::sync::watch::Receiver<bool>>,
}

impl<T: HttpTransport + 'static> EntitlementMint<T> {
    pub fn new(now: NowFn) -> Self {
        Self {
            now,
            managers: Mutex::new(HashMap::new()),
            ban_sink: None,
            runtime: None,
            activity: None,
        }
    }

    /// Reports every refresh failure to `sink`: the entitlement issuer refuses
    /// a banned wallet the way the token issuer does.
    #[must_use]
    pub fn with_ban_sink(mut self, sink: BanSink) -> Self {
        self.ban_sink = Some(sink);
        self
    }

    /// Refreshes only while `activity` reads `true`, and holds each refresh
    /// back while it reads `false`: a client whose account logged out, with
    /// the wallet still loaded, must not keep asking the issuer for it. A
    /// refresh held back runs as soon as `activity` reads `true` again. With
    /// no `activity`, or once its sender is gone, the refresh always runs.
    #[must_use]
    pub fn with_activity(mut self, activity: tokio::sync::watch::Receiver<bool>) -> Self {
        self.activity = Some(activity);
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
    /// manager (via `make_client`, which owns the wallet identity, minting
    /// the batches `blinding` derives) and starts its background refresh;
    /// later calls reuse both, so the factory runs at most once per wallet
    /// and process.
    ///
    /// `blinding` is the wallet's [`BlindingKey::port_entitlement`] key, from
    /// the same seed as the client's identity: the issuer serves an account
    /// only the batch it first signed for it.
    pub fn slot_source(
        &self,
        wallet_pubkey: [u8; 32],
        blinding: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> SlotSource {
        self.slots(wallet_pubkey, blinding, make_client)
    }

    /// The entitlement of rule `slot` of `wallet_pubkey`, per
    /// [`Self::slot_source`].
    pub fn rule_credential(
        &self,
        wallet_pubkey: [u8; 32],
        slot: usize,
        blinding: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> RuleCredential {
        RuleCredential::new(self.slot_source(wallet_pubkey, blinding, make_client), slot)
    }

    fn slots(
        &self,
        wallet_pubkey: [u8; 32],
        blinding: BlindingKey,
        make_client: impl FnOnce() -> WarrenApiClient<T>,
    ) -> Arc<MintedSlots<T>> {
        let mut managers = self.managers.lock().unwrap_or_else(PoisonError::into_inner);
        managers
            .entry(wallet_pubkey)
            .or_insert_with(|| {
                let manager = Arc::new(PortEntitlementManager::new(
                    Arc::new(make_client()),
                    blinding,
                ));
                let refresh = refresh_forever(
                    manager.clone(),
                    self.now.clone(),
                    wallet_pubkey,
                    self.ban_sink.clone(),
                    self.activity.clone(),
                );
                match &self.runtime {
                    Some(runtime) => {
                        runtime.spawn(refresh);
                    }
                    None => {
                        tokio::spawn(refresh);
                    }
                }
                Arc::new(MintedSlots {
                    manager,
                    now: self.now.clone(),
                    holders: Mutex::new(Holders::default()),
                })
            })
            .clone()
    }
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
    mut activity: Option<tokio::sync::watch::Receiver<bool>>,
) {
    let mut tick = tokio::time::interval(REFRESH_INTERVAL);
    loop {
        tick.tick().await;
        if let Some(activity) = activity.as_mut()
            && !*activity.borrow_and_update()
        {
            log::info!("Warren port-entitlement refresh paused: no account is logged in");
            // A sender gone reads as no gate at all, per `with_activity`.
            let _ = activity.wait_for(|active| *active).await;
            log::info!("Warren port-entitlement refresh resumed");
        }
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

/// One live rule's entitlement: what the rule presents on each cycle, whether
/// its last request carried one, and the report of an exit's refusal.
///
/// Dropping it releases the rule's slot in the batch, so a refused rule can
/// move to the entitlement it held, unless another rule took the slot since:
/// the last reference to a rule can go after its successor was built on the
/// same slot (a refresh loop's task is dropped late), and that release would
/// take the successor's entitlement away. Its [`Self::provider`] answers
/// nothing from then on: a refresh loop still winding down must not take the
/// slot back.
pub struct RuleCredential {
    rule: Arc<Rule>,
}

struct Rule {
    source: SlotSource,
    slot: usize,
    /// What the release names, so a rule that took the slot since keeps it.
    claim: u64,
    /// The last credential presented, `None` for a bare request, and whether
    /// the rule is gone. One lock, so a cycle in flight and the release never
    /// interleave.
    state: Mutex<RuleState>,
}

#[derive(Default)]
struct RuleState {
    presented: Option<Vec<u8>>,
    released: bool,
}

impl Rule {
    fn state(&self) -> std::sync::MutexGuard<'_, RuleState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl RuleCredential {
    /// The entitlement a rule holding `slot` presents from `source`.
    #[must_use]
    pub fn new(source: SlotSource, slot: usize) -> Self {
        let claim = source.hold(slot);
        Self {
            rule: Arc::new(Rule {
                source,
                slot,
                claim,
                state: Mutex::new(RuleState::default()),
            }),
        }
    }

    /// What the engine asks once per refresh cycle: the slot's entitlement at
    /// the moment of asking, so an epoch rollover reaches the next renewal.
    #[must_use]
    pub fn provider(&self) -> CredentialSource {
        let rule = Arc::clone(&self.rule);
        Arc::new(move || {
            let mut state = rule.state();
            if state.released {
                return None;
            }
            let credential = rule.source.credential(rule.slot);
            state.presented.clone_from(&credential);
            credential
        })
    }

    /// Whether the last request carried an entitlement, which is what tells
    /// a refused entitlement from a missing one
    /// ([`crate::PortRefusal::of_request`]).
    #[must_use]
    pub fn presented(&self) -> bool {
        self.rule.state().presented.is_some()
    }

    /// The exit refused the rule's last request (NAT-PMP `NotAuthorized`).
    /// When it carried an entitlement, the slot moves to one no other rule
    /// holds, so the next request does not meet the same verdict. Answers
    /// whether it carried one.
    pub fn on_refused(&self) -> bool {
        let presented = self.rule.state().presented.clone();
        match presented {
            Some(credential) => {
                self.rule.source.refused(self.rule.slot, &credential);
                true
            }
            None => false,
        }
    }
}

impl Drop for RuleCredential {
    fn drop(&mut self) {
        let mut state = self.rule.state();
        state.released = true;
        state.presented = None;
        self.rule.source.release(self.rule.slot, self.rule.claim);
    }
}

/// Which slot each live rule holds: the lowest free one, freed with the rule.
///
/// Two live rules never share a slot, or the exit would read them as one
/// port. Reusing a freed slot rather than growing matters because the batch
/// is bounded: slots that only grew would run past it after a few rule
/// changes. A rule rebuilt on the same epoch (a re-bind, a reconnect) gets its
/// old slot back and presents the entitlement of that slot's place, which is
/// the one the exit already spent for it unless the old rule had moved off
/// another device's serial.
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
