//! Live Warren tunnel status surfaced to the gRPC management interface.
//!
//! Carries the auto-reconnect metrics (`reconnect_count` +
//! `last_reconnect_age`) and the obfuscation indicator that the
//! Electron UI displays in the connection details view. The values are
//! held in a [`WarrenStatusCache`] shared between the daemon main loop
//! and the multi-hop supervisor (when in multi-hop mode); for
//! single-hop mode the `reconnect_count` stays at 0 and
//! `obfuscation_active` is true per doctrine
//! `warren_obfuscation_doctrine_v1` (HTTP/3 mimicry always-on /v1).

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use std::collections::HashMap;

use talpid_warren_tunnel::{NatPmpEvent, NatPmpFailureReason, NatPmpProto, NatPmpRuleId};
use tokio::sync::watch;

/// Snapshot of the NAT-PMP refresh loop, surfaced to the UI alongside
/// the connection status. `Disabled` is the default (no toggle, no
/// loop spawned); `Requesting` is set when the daemon-side
/// `NatPmpManager` is created but no event has been observed yet;
/// `Mapped` carries the public port granted by the exit; `Failed`
/// carries the last error string from `NatPmpEvent::Failed`. The state
/// is `Disabled` again as soon as the user turns the toggle off.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NatPmpStateSnapshot {
    /// Port-forwarding is OFF in the user settings (no refresh loop
    /// spawned). The Electron UI hides the public-port row entirely
    /// in this state.
    #[default]
    Disabled,
    /// Port-forwarding is ON but the first mapping has not arrived
    /// yet (initial `request_map` in flight).
    Requesting,
    /// Mapping is active. `external_port` is the public-facing port
    /// the user advertises to incoming peers; `lifetime_secs` is the
    /// last lifetime granted by the exit. The UI displays a countdown
    /// based on this value + the wall clock since the last event.
    Mapped {
        /// Allocated public port (the differentiator surface).
        external_port: u16,
        /// Granted lifetime in seconds (renewals at `secs / 2`).
        lifetime_secs: u32,
        /// Per-source rate-limit slots still available (reported by the
        /// exit). `None` when the exit sent no budget trailer. The UI
        /// warns when this is 0 or 1 and blocks the port control at 0.
        attempts_remaining: Option<u8>,
        /// Seconds until the rate-limit budget grows by one. `0` when
        /// unknown / full. Drives the "wait before next change"
        /// countdown when `attempts_remaining == 0`.
        window_reset_secs: u16,
    },
    /// The exit rate-limited the last (re)mapping request (too many port
    /// changes in a row). Recoverable: the daemon's refresh loop retries
    /// automatically after `retry_after_secs`. The UI blocks the port
    /// control and shows a countdown until then.
    RateLimited {
        /// Seconds to wait before a rate-limit slot frees.
        retry_after_secs: u16,
    },
    /// The last `request_map` failed and the loop has terminated. The
    /// UI surfaces a localised message keyed on `reason`; `error` is the
    /// raw string kept for logs / diagnostics.
    Failed {
        /// Free-form error string from `NatPmpEvent::Failed`.
        error: String,
        /// Stable, translatable failure category. Lets the renderer
        /// show e.g. "this port is already in use" for a strict
        /// suggested-port rejection instead of a raw English string.
        reason: NatPmpFailureReason,
    },
}

/// One NAT-PMP mapping surfaced to the UI, tagged with the rule it
/// belongs to. Multi-port: the UI renders one row per snapshot,
/// identified by `internal_port` + `protocol` (the rule identity). The
/// `state` is never `Disabled` here - a removed rule is dropped from the
/// list rather than marked disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatPmpMappingSnapshot {
    /// Internal port the user's application binds (the rule identity the
    /// UI keys its row on).
    pub internal_port: u16,
    /// Transport protocol of this rule.
    pub protocol: NatPmpProto,
    /// Lifecycle state of this rule's mapping.
    pub state: NatPmpStateSnapshot,
}

/// Forensic payload pushed to the UI when the
/// TOFU verify hook refuses a connect. All fields are operator
/// metadata (`exit_id` and the Ed25519 pubkeys are public via the
/// signed relay-list; the location is the snapshot pinned at TOFU
/// time). The renderer mounts the `WarrenPubKeyWarning` modal while
/// `WarrenStatusSnapshot::pubkey_mismatch_pending` is `Some`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubkeyMismatchPending {
    /// 32-character lower-case hex of the 16-byte stable `exit_id`.
    pub exit_id_hex: String,
    /// 64-character lower-case hex of the previously pinned Ed25519
    /// verifying key.
    pub pinned_pubkey_hex: String,
    /// 64-character lower-case hex of the newly observed key.
    pub observed_pubkey_hex: String,
    /// ISO 3166 alpha-2 country code captured at TOFU time. Empty
    /// when the pin pre-dates the forensic enrichment.
    pub country_code: String,
    /// City label captured at TOFU time. Empty when the pin pre-dates
    /// the forensic enrichment.
    pub city: String,
}

/// Cached copy of the public `GET /v1/network` environment descriptor
/// (see `warren_contract::dto::NetworkInfoResponse`), fetched by the
/// daemon's network-info refresh loop and surfaced to both UIs through
/// the status stream. Display data only: the compiled product
/// environment stays the authority on WHICH build this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInfoSnapshot {
    /// Environment name reported by the API, e.g. `"beta"`.
    pub environment: String,
    /// True when the service is deliberately degraded (bandwidth-capped
    /// free beta). The UIs surface the label; enforcement is exit-side.
    pub degraded: bool,
    /// Default per-subscriber bandwidth cap in bits per second, absent
    /// when the environment applies no cap.
    pub default_rate_bps: Option<u64>,
    /// False when this environment must not expose payment flows.
    pub payments_enabled: bool,
}

/// Snapshot returned by `GetWarrenStatus` rpc. Trivially cloneable so
/// the watch channel can broadcast it without locking issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarrenStatusSnapshot {
    /// Number of successful reconnects since the supervisor was last
    /// spawned. 0 in single-hop mode (no auto-reconnect supervisor).
    pub reconnect_count: u32,
    /// Time since the last successful reconnect. `None` if there has
    /// not been any reconnect yet (e.g. fresh session, single-hop, or
    /// connection currently failing).
    pub last_reconnect_age: Option<Duration>,
    /// True when the HTTP/3 mimicry obfuscation is active. /v1 has
    /// this always-on (doctrine), so this is always true.
    pub obfuscation_active: bool,
    /// Live NAT-PMP mappings for the port-forwarding UI, one entry per
    /// active rule, sorted by `internal_port` for a stable UI order.
    /// Empty when the user has not opted into port-forwarding (or all
    /// rules were removed).
    pub nat_pmp_mappings: Vec<NatPmpMappingSnapshot>,
    /// Multi-exit failover counter. Bumped each time the
    /// `assemble_failover_for_attempt` path is taken (retry > 0 with a
    /// memoized previous exit). The UI displays a toast when this
    /// counter increments so the user knows their exit was swapped.
    pub failover_count: u32,
    /// Time since the last failover. `None` if no failover has been
    /// recorded in the current session. Matches `last_reconnect_age`
    /// semantics (computed against the calling thread's clock).
    pub last_failover_age: Option<Duration>,
    /// `None` (steady state) when no TOFU mismatch is
    /// pending review; `Some` while the verify hook refused a connect
    /// because the observed Ed25519 pubkey differs from the pinned
    /// baseline. The UI mounts the `WarrenPubKeyWarning` modal until
    /// the user picks Trust / Reject / Report and the daemon clears
    /// the flag.
    pub pubkey_mismatch_pending: Option<PubkeyMismatchPending>,
    /// True while the last drain-triggered exit switch (ADR 36
    /// maintenance migration) is younger than
    /// [`MAINTENANCE_MIGRATION_TTL`]. The UI shows a self-expiring
    /// "server maintenance" banner while this holds; the daemon
    /// re-broadcasts at expiry so the banner drops without a poll.
    pub maintenance_migration_active: bool,
    /// docs/59 C5: number of maintenance migrations CANCELLED because a
    /// user-pinned port could not be reserved on any candidate exit.
    /// The client stays on the draining exit with its ports intact; the
    /// UI toasts "migration postponed, port kept" on each increment.
    pub port_migration_cancellations: u32,
    /// True while the last port-conflict cancellation is inside its
    /// display window ([`MAINTENANCE_MIGRATION_TTL`], same discipline as
    /// the maintenance banner). The recorder schedules a rebroadcast at
    /// expiry so the UI banner self-dismisses.
    pub port_migration_cancellation_active: bool,
    /// True while the offline monitor reports the host offline. Pushed
    /// on the very edge (no grace): the tunnel state machine holds
    /// Connected for its migration grace window, so without this flag
    /// the UI has no way to tell the user the network is gone until
    /// the grace expires. The UI shows an immediate "no internet
    /// connection" banner keyed on it, in every tunnel state.
    pub host_offline: bool,
    /// Doc 62 item 5: true while the in-tunnel egress liveness probe
    /// reports the exit not forwarding (N consecutive probe failures
    /// with the QUIC session otherwise alive, e.g. a drained or
    /// half-swapped exit during a fleet rollout). Cleared by a single
    /// successful probe, and whenever the tunnel leaves Connected. The
    /// UI renders the "interrupted" phase with an exit-not-forwarding
    /// cause while this holds.
    pub exit_egress_dead: bool,
    /// Latest `GET /v1/network` descriptor, `None` until the first
    /// successful fetch (or when the API predates the endpoint).
    pub network_info: Option<NetworkInfoSnapshot>,
    /// Broadcast notices the operator has published, already verified
    /// against the pinned server key and filtered for expiry and this
    /// app's version by the notices updater. Empty in the steady state.
    /// The UI shows them above every other banner: an operator-authored
    /// message is the only thing that outranks the app's own reading of
    /// the connection.
    pub notices: Vec<crate::warren_notices_updater::DisplayNotice>,
    /// Verified forum activity counts, one lowercase hex character per
    /// anonymous slot, or `None` while no fresh document is held.
    ///
    /// Published verbatim: the daemon checks the signature and the expiry,
    /// and the renderer indexes the one slot it knows is the user's. That
    /// slot lives in the renderer's sealed store beside the forum handle,
    /// so no part of the daemon, and nothing on the wire, ties the
    /// document to an account.
    pub forum_digest: Option<String>,
}

impl Default for WarrenStatusSnapshot {
    /// Default: fresh boot, no reconnects observed, obfuscation
    /// enabled per /v1 doctrine, NAT-PMP disabled.
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_age: None,
            obfuscation_active: true,
            nat_pmp_mappings: Vec::new(),
            failover_count: 0,
            last_failover_age: None,
            pubkey_mismatch_pending: None,
            maintenance_migration_active: false,
            port_migration_cancellations: 0,
            port_migration_cancellation_active: false,
            host_offline: false,
            exit_egress_dead: false,
            network_info: None,
            notices: Vec::new(),
            forum_digest: None,
        }
    }
}

/// How long a drain-triggered migration is surfaced as "maintenance in
/// progress". Matches the drained-exit avoid-set TTL
/// (`crate::tunnel::WARREN_DRAINED_EXIT_TTL_SECS`): once the avoid-set
/// entry expires the exit is selectable again, i.e. its maintenance
/// window is considered over from this client's viewpoint.
pub const MAINTENANCE_MIGRATION_TTL: Duration =
    Duration::from_secs(crate::tunnel::WARREN_DRAINED_EXIT_TTL_SECS);

/// Whether a maintenance migration recorded at `at` is still inside its
/// display window at `now`. Pure so the TTL boundary is unit-testable.
fn maintenance_active(at: Option<Instant>, now: Instant) -> bool {
    at.is_some_and(|t| now.saturating_duration_since(t) < MAINTENANCE_MIGRATION_TTL)
}

/// Internal state held by [`WarrenStatusCache`]. Separated to keep the
/// public snapshot trivially `Clone` while letting the cache hold an
/// `Instant` (which is not `Serialize` / not stable wire format).
#[derive(Debug, Clone)]
struct InternalState {
    reconnect_count: u32,
    last_reconnect_at: Option<Instant>,
    obfuscation_active: bool,
    /// Per-rule NAT-PMP mappings keyed by rule identity. Materialised
    /// into a sorted `Vec` in [`WarrenStatusCache::snapshot_of`].
    nat_pmp: HashMap<NatPmpRuleId, NatPmpStateSnapshot>,
    failover_count: u32,
    last_failover_at: Option<Instant>,
    pubkey_mismatch_pending: Option<PubkeyMismatchPending>,
    last_maintenance_migration_at: Option<Instant>,
    port_migration_cancellations: u32,
    last_port_migration_cancelled_at: Option<Instant>,
    host_offline: bool,
    exit_egress_dead: bool,
    network_info: Option<NetworkInfoSnapshot>,
    notices: Vec<crate::warren_notices_updater::DisplayNotice>,
    forum_digest: Option<String>,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_at: None,
            obfuscation_active: true,
            nat_pmp: HashMap::new(),
            failover_count: 0,
            last_failover_at: None,
            pubkey_mismatch_pending: None,
            last_maintenance_migration_at: None,
            port_migration_cancellations: 0,
            last_port_migration_cancelled_at: None,
            host_offline: false,
            exit_egress_dead: false,
            network_info: None,
            notices: Vec::new(),
            forum_digest: None,
        }
    }
}

/// Shared cache populated by the multi-hop supervisor and read by the
/// gRPC handlers. Cloneable handle around an [`RwLock`] + a watch
/// sender for push updates to the `WarrenStatusUpdates` stream.
#[derive(Clone)]
pub struct WarrenStatusCache {
    state: Arc<RwLock<InternalState>>,
    tx: watch::Sender<WarrenStatusSnapshot>,
}

impl WarrenStatusCache {
    /// Create a fresh cache with default state. The internal watch
    /// channel is initialised with the default snapshot so subscribers
    /// always get an immediate read.
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(WarrenStatusSnapshot::default());
        Self {
            state: Arc::new(RwLock::new(InternalState::default())),
            tx,
        }
    }

    /// Subscribe to push status updates. The returned receiver yields
    /// a fresh snapshot whenever the supervisor records a reconnect
    /// (or other tracked transitions).
    pub fn subscribe(&self) -> watch::Receiver<WarrenStatusSnapshot> {
        self.tx.subscribe()
    }

    /// Get the current snapshot. Cheap: snapshot of the internal
    /// state with the `Instant` materialised into a `Duration` against
    /// the calling thread's clock.
    pub fn snapshot(&self) -> WarrenStatusSnapshot {
        let inner = self
            .state
            .read()
            .expect("warren_status state lock poisoned");
        Self::snapshot_of(&inner)
    }

    /// Build a [`WarrenStatusSnapshot`] from the internal state. Used
    /// by every `record_*` / `set_*` method to avoid drift between the
    /// construction sites whenever new fields are added.
    fn snapshot_of(inner: &InternalState) -> WarrenStatusSnapshot {
        WarrenStatusSnapshot {
            reconnect_count: inner.reconnect_count,
            last_reconnect_age: inner.last_reconnect_at.map(|t| t.elapsed()),
            obfuscation_active: inner.obfuscation_active,
            nat_pmp_mappings: Self::nat_pmp_mappings_of(&inner.nat_pmp),
            failover_count: inner.failover_count,
            last_failover_age: inner.last_failover_at.map(|t| t.elapsed()),
            pubkey_mismatch_pending: inner.pubkey_mismatch_pending.clone(),
            maintenance_migration_active: maintenance_active(
                inner.last_maintenance_migration_at,
                Instant::now(),
            ),
            port_migration_cancellations: inner.port_migration_cancellations,
            port_migration_cancellation_active: maintenance_active(
                inner.last_port_migration_cancelled_at,
                Instant::now(),
            ),
            host_offline: inner.host_offline,
            exit_egress_dead: inner.exit_egress_dead,
            network_info: inner.network_info.clone(),
            notices: inner.notices.clone(),
            forum_digest: inner.forum_digest.clone(),
        }
    }

    /// Materialise the per-rule NAT-PMP map into a `Vec` sorted by
    /// `(internal_port, protocol)` so the UI sees a stable row order
    /// regardless of `HashMap` iteration order.
    fn nat_pmp_mappings_of(
        map: &HashMap<NatPmpRuleId, NatPmpStateSnapshot>,
    ) -> Vec<NatPmpMappingSnapshot> {
        let mut v: Vec<NatPmpMappingSnapshot> = map
            .iter()
            .map(|(id, state)| NatPmpMappingSnapshot {
                internal_port: id.internal_port,
                protocol: id.protocol,
                state: state.clone(),
            })
            .collect();
        v.sort_by_key(|m| {
            let proto_rank = match m.protocol {
                NatPmpProto::Udp => 0u8,
                NatPmpProto::Tcp => 1,
                NatPmpProto::Both => 2,
            };
            (m.internal_port, proto_rank)
        });
        v
    }

    /// Called by the multi-hop supervisor whenever it successfully
    /// reconnects after a transient failure. Bumps the counter and
    /// stamps the moment, then broadcasts the new snapshot to all
    /// `WarrenStatusUpdates` stream subscribers.
    pub fn record_reconnect(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.reconnect_count = inner.reconnect_count.saturating_add(1);
            inner.last_reconnect_at = Some(Instant::now());
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Bumps the failover counter and stamps the moment.
    /// Called from the tunnel-state-machine right when
    /// `assemble_failover_for_attempt` is dispatched, i.e. when a
    /// retry > 0 will exclude the previous exit. The UI toast layer
    /// observes `failover_count` increments via the watch channel.
    pub fn record_failover(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.failover_count = inner.failover_count.saturating_add(1);
            inner.last_failover_at = Some(Instant::now());
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Stamps a drain-triggered exit switch (ADR 36 maintenance
    /// migration) and broadcasts. The snapshot reports
    /// `maintenance_migration_active` until [`MAINTENANCE_MIGRATION_TTL`]
    /// elapses; the caller schedules a [`Self::rebroadcast`] at expiry so
    /// subscribers observe the flip back to inactive.
    pub fn record_maintenance_migration(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.last_maintenance_migration_at = Some(Instant::now());
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// docs/59 C5: records that a maintenance migration was cancelled
    /// because a user-pinned port could not be reserved on any candidate
    /// exit (the client stays put, ports kept), and broadcasts so the UI
    /// can toast the dedicated signal.
    pub fn record_port_migration_cancelled(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.port_migration_cancellations =
                inner.port_migration_cancellations.saturating_add(1);
            inner.last_port_migration_cancelled_at = Some(Instant::now());
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
        // Time-driven flip back to inactive, mirroring the maintenance
        // banner: rebroadcast once the display TTL elapses so the UI
        // drops the banner without polling. Skipped outside a runtime
        // (plain unit tests): the flag then just decays unobserved.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let cache = self.clone();
            handle.spawn(async move {
                tokio::time::sleep(MAINTENANCE_MIGRATION_TTL + Duration::from_secs(1)).await;
                cache.rebroadcast();
            });
        }
    }

    /// Re-broadcasts the current snapshot unconditionally. Needed for
    /// time-driven flips (the maintenance banner expiry) where no state
    /// write happens but subscribers must observe the recomputed value.
    pub fn rebroadcast(&self) {
        let snapshot = {
            let inner = self
                .state
                .read()
                .expect("warren_status state lock poisoned");
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Records a NAT-PMP event coming from the daemon-side
    /// `NatPmpManager` and broadcasts the resulting snapshot on the
    /// watch channel so the Electron UI updates.
    ///
    /// Event -> state mapping:
    /// - `Mapped { .. }` / `Renewed { .. }` -> `Mapped { .. }`
    ///   (UI reuses the same row; the lifetime countdown resets). The
    ///   rate-limit budget is carried through so the UI can warn before
    ///   the next ban.
    /// - `RateLimited { retry_after_secs }` -> `RateLimited { .. }`
    ///   (UI blocks the port control and counts down; the loop retries).
    /// - `Failed { error, reason }` -> `Failed { .. }`.
    /// - `Cancelled` -> `Disabled` (the user disabled the toggle, or
    ///   the tunnel went down).
    pub fn record_nat_pmp_event(&self, id: NatPmpRuleId, event: NatPmpEvent) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            match event {
                NatPmpEvent::Mapped {
                    external_port,
                    lifetime_secs,
                    attempts_remaining,
                    window_reset_secs,
                }
                | NatPmpEvent::Renewed {
                    external_port,
                    lifetime_secs,
                    attempts_remaining,
                    window_reset_secs,
                } => {
                    inner.nat_pmp.insert(
                        id,
                        NatPmpStateSnapshot::Mapped {
                            external_port,
                            lifetime_secs,
                            attempts_remaining,
                            window_reset_secs,
                        },
                    );
                }
                NatPmpEvent::RateLimited { retry_after_secs } => {
                    inner
                        .nat_pmp
                        .insert(id, NatPmpStateSnapshot::RateLimited { retry_after_secs });
                }
                NatPmpEvent::Failed { error, reason } => {
                    inner
                        .nat_pmp
                        .insert(id, NatPmpStateSnapshot::Failed { error, reason });
                }
                // The rule's mapping was released (toggle off, rule
                // removed, or tunnel down): drop it from the list.
                NatPmpEvent::Cancelled => {
                    inner.nat_pmp.remove(&id);
                }
            }
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Reconcile the NAT-PMP mapping list against the rules the user now
    /// wants active: drop mappings whose rule disappeared, and pre-set
    /// any newly-added rule to `Requesting` so the UI reflects the
    /// in-flight state immediately (before the manager's first event).
    /// Existing mappings (e.g. already `Mapped`) keep their state to
    /// avoid a flicker when an unrelated rule changes.
    pub fn set_nat_pmp_requesting(&self, rules: &[NatPmpRuleId]) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            let wanted: std::collections::HashSet<NatPmpRuleId> = rules.iter().copied().collect();
            inner.nat_pmp.retain(|id, _| wanted.contains(id));
            for id in rules {
                inner
                    .nat_pmp
                    .entry(*id)
                    .or_insert(NatPmpStateSnapshot::Requesting);
            }
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Clears all NAT-PMP mappings, broadcasting the resulting snapshot.
    /// Called when the user toggles port-forwarding off or when a tunnel
    /// goes down without firing a `Cancelled` event (defensive).
    pub fn set_nat_pmp_disabled(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.nat_pmp.is_empty() {
                return;
            }
            inner.nat_pmp.clear();
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Surface a TOFU pubkey-mismatch event to the UI.
    /// Mounts the `WarrenPubKeyWarning` modal on the next watch tick.
    /// Idempotent: passing the same payload again does not re-push.
    pub fn set_pubkey_mismatch_pending(&self, pending: PubkeyMismatchPending) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.pubkey_mismatch_pending.as_ref() == Some(&pending) {
                return;
            }
            inner.pubkey_mismatch_pending = Some(pending);
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Clear the pending mismatch flag (= the user
    /// picked Trust / Reject / Report from the modal). Idempotent.
    pub fn clear_pubkey_mismatch_pending(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.pubkey_mismatch_pending.is_none() {
                return;
            }
            inner.pubkey_mismatch_pending = None;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Record the offline monitor's verdict, pushed by the daemon on
    /// every connectivity edge (T+0, before any tunnel grace). The UI
    /// banner keys on the snapshot flag. Idempotent: repeated equal
    /// verdicts do not re-push (route-event storms would otherwise
    /// spam the stream).
    pub fn set_host_offline(&self, offline: bool) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.host_offline == offline {
                return;
            }
            inner.host_offline = offline;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Record the in-tunnel egress probe's verdict (doc 62 item 5).
    /// Pushed on verdict edges by the probe callback and cleared by the
    /// daemon whenever the tunnel leaves Connected (a rebuilt tunnel
    /// starts with a clean verdict). Idempotent: repeated equal
    /// verdicts do not re-push.
    pub fn set_exit_egress_dead(&self, dead: bool) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.exit_egress_dead == dead {
                return;
            }
            inner.exit_egress_dead = dead;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Record the latest `GET /v1/network` descriptor fetched by the
    /// daemon's network-info refresh loop and broadcast it to the
    /// status stream. Idempotent: re-recording an equal descriptor
    /// does not re-push (the loop refreshes periodically and the
    /// payload rarely changes).
    pub fn set_network_info(&self, info: Option<NetworkInfoSnapshot>) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.network_info == info {
                return;
            }
            inner.network_info = info;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Record the notices the client must display, as published by the
    /// notices updater after signature verification. Idempotent: the
    /// updater re-publishes on every refresh (including every `304`), so
    /// re-pushing an identical set must not wake the UI.
    ///
    /// The empty vector is a meaningful value, not a no-op: it is how an
    /// erasure, an expiry or a failed verification clears the banner.
    pub fn set_notices(&self, notices: Vec<crate::warren_notices_updater::DisplayNotice>) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.notices == notices {
                return;
            }
            inner.notices = notices;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Record the verified forum activity counts, as published by the
    /// digest updater after signature verification. Idempotent: the
    /// updater re-publishes on every cycle so freshness keeps being
    /// applied, and re-pushing an identical document must not wake the UI.
    ///
    /// `None` is a meaningful value, not a no-op: it is how an expiry, a
    /// server that stopped answering, or a failed verification clears the
    /// badge.
    pub fn set_forum_digest(&self, counts: Option<String>) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.forum_digest == counts {
                return;
            }
            inner.forum_digest = counts;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }

    /// Toggle the obfuscation indicator. Always-on /v1 means this
    /// stays true in production; the setter exists so a future /v2
    /// toggle can flip it without touching the cache shape.
    pub fn set_obfuscation_active(&self, active: bool) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if inner.obfuscation_active == active {
                return;
            }
            inner.obfuscation_active = active;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send_replace(snapshot);
    }
}

impl Default for WarrenStatusCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Coarse tunnel-state kind consumed by [`auto_recovery_step`]. The
/// daemon maps its `TunnelState` to this before each step so the
/// attribution logic stays a pure, platform-free function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelStateKind {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

/// One step of the auto-recovery attribution driving the UI
/// `reconnect_count` row alongside the supervisor's silent redials.
///
/// The supervisor observer only fires when the QUIC session is redialed
/// UNDER a live tunnel; recoveries that go through the state machine
/// (offline grace -> Error(IsOffline) -> auto reconnect, or the
/// session-liveness escalation -> Connecting -> Connected) never touch
/// it, which left the counter frozen at 0 through real network-loss
/// recoveries. This function attributes those: it arms a pending flag
/// on the state edges that only automation produces, and reports a
/// count when a Connected arrives with the flag armed.
///
/// Returns `(pending_after, count_now)`.
pub fn auto_recovery_step(
    pending: bool,
    prev: TunnelStateKind,
    next: TunnelStateKind,
    target_secured: bool,
) -> (bool, bool) {
    match next {
        // The daemon auto-retries out of a blocked error while the user
        // still wants the tunnel up; reaching Connected from here is a
        // recovery, not a user action.
        TunnelStateKind::Error if target_secured => (true, false),
        // Connected -> Connecting without a Disconnecting in between
        // only happens when the tunnel died under us (liveness
        // escalation, transient backend error). Every user-initiated
        // reconnect (button, relay change) passes through
        // Disconnecting first.
        TunnelStateKind::Connecting if prev == TunnelStateKind::Connected => (true, false),
        TunnelStateKind::Connected => (false, pending),
        // The user gave up (manual disconnect) or the target flipped:
        // whatever comes next is not an automatic recovery.
        TunnelStateKind::Disconnected => (false, false),
        _ => (pending, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_matches_v1_doctrine() {
        let s = WarrenStatusSnapshot::default();
        assert_eq!(s.reconnect_count, 0);
        assert_eq!(s.last_reconnect_age, None);
        assert!(s.obfuscation_active, "obfuscation always-on /v1");
    }

    #[test]
    fn cache_default_returns_doctrine_defaults() {
        let cache = WarrenStatusCache::new();
        let snap = cache.snapshot();
        assert_eq!(snap.reconnect_count, 0);
        assert_eq!(snap.last_reconnect_age, None);
        assert!(snap.obfuscation_active);
    }

    #[test]
    fn record_reconnect_bumps_counter_and_resets_age() {
        let cache = WarrenStatusCache::new();
        cache.record_reconnect();
        let snap = cache.snapshot();
        assert_eq!(snap.reconnect_count, 1);
        // last_reconnect_age must be very small (just set), not None.
        let age = snap
            .last_reconnect_age
            .expect("last_reconnect_age must be Some after record_reconnect");
        assert!(age < Duration::from_secs(1));
    }

    #[test]
    fn record_reconnect_increments_monotonically() {
        let cache = WarrenStatusCache::new();
        cache.record_reconnect();
        cache.record_reconnect();
        cache.record_reconnect();
        assert_eq!(cache.snapshot().reconnect_count, 3);
    }

    #[test]
    fn subscribe_yields_snapshot_on_record_reconnect() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        assert_eq!(rx.borrow().reconnect_count, 0);
        cache.record_reconnect();
        // mark_changed is exercised by Sender::send; the receiver
        // should now see the bumped count.
        assert_eq!(rx.borrow_and_update().reconnect_count, 1);
    }

    #[test]
    fn set_obfuscation_active_no_change_does_not_send() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        // Default already true; flipping to true again must NOT mark
        // the watch channel as changed (avoids spurious UI redraws).
        rx.borrow_and_update();
        cache.set_obfuscation_active(true);
        assert!(!rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn set_obfuscation_active_false_then_true_emits_two_snapshots() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_obfuscation_active(false);
        assert!(rx.has_changed().unwrap_or(false));
        let s = rx.borrow_and_update();
        assert!(!s.obfuscation_active);
    }

    #[test]
    fn reconnect_count_saturates_at_u32_max() {
        let cache = WarrenStatusCache::new();
        // Pre-set internal counter close to the cap to exercise the
        // saturating add without spending billions of cycles in a loop.
        {
            let mut inner = cache.state.write().unwrap();
            inner.reconnect_count = u32::MAX - 1;
        }
        cache.record_reconnect();
        cache.record_reconnect();
        assert_eq!(cache.snapshot().reconnect_count, u32::MAX);
    }

    // --- Exit maintenance migration (ADR 36 banner) --------------------------

    #[test]
    fn default_snapshot_has_no_maintenance_migration() {
        assert!(!WarrenStatusSnapshot::default().maintenance_migration_active);
        assert!(
            !WarrenStatusCache::new()
                .snapshot()
                .maintenance_migration_active
        );
    }

    #[test]
    fn record_maintenance_migration_activates_snapshot() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.record_maintenance_migration();
        assert!(rx.has_changed().unwrap_or(false));
        assert!(cache.snapshot().maintenance_migration_active);
    }

    #[test]
    fn maintenance_active_expires_after_ttl() {
        let now = Instant::now();
        assert!(!maintenance_active(None, now));
        assert!(maintenance_active(Some(now), now));
        let inside = now
            .checked_sub(MAINTENANCE_MIGRATION_TTL - Duration::from_secs(1))
            .expect("clock supports subtraction");
        assert!(maintenance_active(Some(inside), now));
        let expired = now
            .checked_sub(MAINTENANCE_MIGRATION_TTL + Duration::from_secs(1))
            .expect("clock supports subtraction");
        assert!(!maintenance_active(Some(expired), now));
    }

    // --- Port-conflict migration cancellation (docs 59 C5) -------------------

    #[test]
    fn default_snapshot_has_no_port_migration_cancellation() {
        assert_eq!(
            WarrenStatusSnapshot::default().port_migration_cancellations,
            0
        );
        assert_eq!(
            WarrenStatusCache::new()
                .snapshot()
                .port_migration_cancellations,
            0
        );
    }

    #[test]
    fn record_port_migration_cancelled_bumps_counter_and_broadcasts() {
        // C5: the UI toasts "migration postponed, port kept" on each
        // increment, so the record must both bump and push.
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.record_port_migration_cancelled();
        assert!(rx.has_changed().unwrap_or(false));
        assert_eq!(cache.snapshot().port_migration_cancellations, 1);
        cache.record_port_migration_cancelled();
        assert_eq!(cache.snapshot().port_migration_cancellations, 2);
    }

    #[test]
    fn record_port_migration_cancelled_opens_the_display_window() {
        // The UI banner keys on the windowed flag (same TTL discipline
        // as the maintenance banner: self-expiring, rebroadcast-driven).
        let cache = WarrenStatusCache::new();
        assert!(!cache.snapshot().port_migration_cancellation_active);
        cache.record_port_migration_cancelled();
        assert!(cache.snapshot().port_migration_cancellation_active);
    }

    #[test]
    fn record_port_migration_cancelled_saturates_at_u32_max() {
        let cache = WarrenStatusCache::new();
        {
            let mut inner = cache.state.write().unwrap();
            inner.port_migration_cancellations = u32::MAX - 1;
        }
        cache.record_port_migration_cancelled();
        cache.record_port_migration_cancelled();
        assert_eq!(cache.snapshot().port_migration_cancellations, u32::MAX);
    }

    #[test]
    fn rebroadcast_reemits_current_snapshot() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.rebroadcast();
        // The expiry timer relies on an unconditional re-send so the UI
        // observes the active -> inactive flip without a state change.
        assert!(rx.has_changed().unwrap_or(false));
    }

    // --- NAT-PMP surface ----------------------------------------------------

    /// Build a rule id for tests (UDP on the given internal port).
    fn rule(internal_port: u16) -> NatPmpRuleId {
        NatPmpRuleId {
            internal_port,
            protocol: NatPmpProto::Udp,
        }
    }

    /// The state of the single mapping in the snapshot (panics if not
    /// exactly one).
    fn only_state(cache: &WarrenStatusCache) -> NatPmpStateSnapshot {
        let mappings = cache.snapshot().nat_pmp_mappings;
        assert_eq!(mappings.len(), 1, "expected exactly one mapping");
        mappings[0].state.clone()
    }

    #[test]
    fn default_snapshot_has_no_nat_pmp_mappings() {
        let s = WarrenStatusSnapshot::default();
        assert!(s.nat_pmp_mappings.is_empty());
    }

    #[test]
    fn record_nat_pmp_mapped_event_updates_state_to_mapped() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 49152,
                lifetime_secs: 3600,
                attempts_remaining: Some(4),
                window_reset_secs: 0,
            },
        );
        let m = &cache.snapshot().nat_pmp_mappings[0];
        assert_eq!(m.internal_port, 22);
        match &m.state {
            NatPmpStateSnapshot::Mapped {
                external_port,
                lifetime_secs,
                attempts_remaining,
                window_reset_secs,
            } => {
                assert_eq!(*external_port, 49152);
                assert_eq!(*lifetime_secs, 3600);
                assert_eq!(*attempts_remaining, Some(4));
                assert_eq!(*window_reset_secs, 0);
            }
            other => panic!("expected Mapped, got {other:?}"),
        }
    }

    #[test]
    fn late_subscriber_initial_snapshot_carries_events_recorded_while_unobserved() {
        // Headless daemon (no GUI stream): the cache has ZERO receivers
        // when the refresh loop delivers Mapped. `watch::Sender::send`
        // refuses to store a value on a receiverless channel, so every
        // later one-shot `port-forward status` subscription was served
        // the stale default snapshot ("No active mappings") while the
        // exit held a granted mapping. A subscriber arriving after the
        // event must see it in the stream's initial value.
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(40000),
            NatPmpEvent::Mapped {
                external_port: 64163,
                lifetime_secs: 3600,
                attempts_remaining: None,
                window_reset_secs: 0,
            },
        );
        let rx = cache.subscribe();
        let snap = rx.borrow();
        assert_eq!(
            snap.nat_pmp_mappings.len(),
            1,
            "a late subscriber's initial snapshot must carry the mapping"
        );
        assert!(
            matches!(
                snap.nat_pmp_mappings[0].state,
                NatPmpStateSnapshot::Mapped {
                    external_port: 64163,
                    ..
                }
            ),
            "expected the Mapped state, got {:?}",
            snap.nat_pmp_mappings[0].state
        );
    }

    #[test]
    fn record_nat_pmp_two_rules_yields_two_sorted_mappings() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(8080),
            NatPmpEvent::Mapped {
                external_port: 50000,
                lifetime_secs: 60,
                attempts_remaining: None,
                window_reset_secs: 0,
            },
        );
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 49152,
                lifetime_secs: 60,
                attempts_remaining: None,
                window_reset_secs: 0,
            },
        );
        let mappings = cache.snapshot().nat_pmp_mappings;
        assert_eq!(mappings.len(), 2);
        // Sorted by internal_port ascending.
        assert_eq!(mappings[0].internal_port, 22);
        assert_eq!(mappings[1].internal_port, 8080);
    }

    #[test]
    fn record_nat_pmp_rate_limited_event_updates_state_with_retry_after() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::RateLimited {
                retry_after_secs: 47,
            },
        );
        match only_state(&cache) {
            NatPmpStateSnapshot::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, 47);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn record_nat_pmp_renewed_event_updates_state_keeps_lifetime_fresh() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 49152,
                lifetime_secs: 3600,
                attempts_remaining: Some(4),
                window_reset_secs: 0,
            },
        );
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Renewed {
                external_port: 49152,
                lifetime_secs: 3600,
                attempts_remaining: Some(3),
                window_reset_secs: 12,
            },
        );
        match only_state(&cache) {
            NatPmpStateSnapshot::Mapped {
                external_port,
                lifetime_secs,
                ..
            } => {
                assert_eq!(external_port, 49152);
                assert_eq!(lifetime_secs, 3600);
            }
            other => panic!("expected Mapped after Renewed, got {other:?}"),
        }
    }

    #[test]
    fn record_nat_pmp_failed_event_updates_state_and_surfaces_error() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Failed {
                error: "server returned error: OutOfResources".to_owned(),
                reason: NatPmpFailureReason::OutOfResources,
            },
        );
        match only_state(&cache) {
            NatPmpStateSnapshot::Failed { error, reason } => {
                assert!(
                    error.contains("OutOfResources"),
                    "error must propagate: {error}"
                );
                assert_eq!(reason, NatPmpFailureReason::OutOfResources);
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn record_nat_pmp_cancelled_removes_only_that_rule() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 49152,
                lifetime_secs: 3600,
                attempts_remaining: Some(4),
                window_reset_secs: 0,
            },
        );
        cache.record_nat_pmp_event(
            rule(8080),
            NatPmpEvent::Mapped {
                external_port: 50000,
                lifetime_secs: 3600,
                attempts_remaining: Some(4),
                window_reset_secs: 0,
            },
        );
        cache.record_nat_pmp_event(rule(22), NatPmpEvent::Cancelled);
        let mappings = cache.snapshot().nat_pmp_mappings;
        assert_eq!(mappings.len(), 1, "only rule 22 removed");
        assert_eq!(mappings[0].internal_port, 8080);
    }

    #[test]
    fn set_nat_pmp_requesting_then_mapped_round_trip() {
        let cache = WarrenStatusCache::new();
        cache.set_nat_pmp_requesting(&[rule(22)]);
        assert_eq!(only_state(&cache), NatPmpStateSnapshot::Requesting);
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 60000,
                lifetime_secs: 60,
                attempts_remaining: Some(5),
                window_reset_secs: 0,
            },
        );
        assert!(matches!(
            only_state(&cache),
            NatPmpStateSnapshot::Mapped {
                external_port: 60000,
                lifetime_secs: 60,
                ..
            }
        ));
    }

    #[test]
    fn set_nat_pmp_requesting_drops_removed_rules_and_keeps_mapped() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 60000,
                lifetime_secs: 60,
                attempts_remaining: Some(5),
                window_reset_secs: 0,
            },
        );
        // Reconcile to a set that drops rule 22 and adds rule 8080.
        cache.set_nat_pmp_requesting(&[rule(8080)]);
        let mappings = cache.snapshot().nat_pmp_mappings;
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].internal_port, 8080);
        assert_eq!(mappings[0].state, NatPmpStateSnapshot::Requesting);
    }

    #[test]
    fn set_nat_pmp_requesting_preserves_existing_mapped_state() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 60000,
                lifetime_secs: 60,
                attempts_remaining: Some(5),
                window_reset_secs: 0,
            },
        );
        // Re-asserting the same rule set must NOT flip 22 back to Requesting.
        cache.set_nat_pmp_requesting(&[rule(22)]);
        assert!(matches!(
            only_state(&cache),
            NatPmpStateSnapshot::Mapped { .. }
        ));
    }

    #[test]
    fn set_nat_pmp_disabled_idempotent_when_already_empty() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        // No mappings by default; another disable must not push.
        cache.set_nat_pmp_disabled();
        assert!(!rx.has_changed().unwrap_or(false));
    }

    // --- Auto-recovery attribution ---------------------------------

    use TunnelStateKind::*;

    #[test]
    fn offline_block_then_reconnect_counts_one_recovery() {
        // Connected -> Error(IsOffline, grace expired) -> Connecting
        // (online edge) -> Connected: the exact poka/topic-37 flow.
        let (p, c) = auto_recovery_step(false, Connected, Error, true);
        assert!(p && !c);
        let (p, c) = auto_recovery_step(p, Error, Connecting, true);
        assert!(p && !c);
        let (p, c) = auto_recovery_step(p, Connecting, Connected, true);
        assert!(!p);
        assert!(
            c,
            "reaching Connected out of an offline block is a recovery"
        );
    }

    #[test]
    fn tunnel_dying_under_us_then_recovering_counts() {
        // Session-liveness escalation: Connected -> Connecting directly
        // (no Disconnecting edge), then Connected again.
        let (p, c) = auto_recovery_step(false, Connected, Connecting, true);
        assert!(p && !c);
        let (p, c) = auto_recovery_step(p, Connecting, Connected, true);
        assert!(!p);
        assert!(c);
    }

    #[test]
    fn user_reconnect_does_not_count() {
        // Button / relay change: Connected -> Disconnecting ->
        // Connecting -> Connected. Never arms the flag.
        let (p, c) = auto_recovery_step(false, Connected, Disconnecting, true);
        assert!(!p && !c);
        let (p, c) = auto_recovery_step(p, Disconnecting, Connecting, true);
        assert!(!p && !c);
        let (p, c) = auto_recovery_step(p, Connecting, Connected, true);
        assert!(!p && !c);
    }

    #[test]
    fn manual_disconnect_clears_the_pending_flag() {
        let (p, _) = auto_recovery_step(false, Connected, Error, true);
        assert!(p);
        let (p, c) = auto_recovery_step(p, Error, Disconnected, true);
        assert!(!p && !c);
        // A later manual connect must not count as a recovery.
        let (p, c) = auto_recovery_step(p, Connecting, Connected, true);
        assert!(!p && !c);
    }

    #[test]
    fn error_while_unsecured_never_arms() {
        let (p, c) = auto_recovery_step(false, Connected, Error, false);
        assert!(!p && !c);
    }

    // --- Host offline surface -------------------------------------

    #[test]
    fn default_snapshot_reports_host_online() {
        assert!(!WarrenStatusSnapshot::default().host_offline);
        assert!(!WarrenStatusCache::new().snapshot().host_offline);
    }

    #[test]
    fn set_host_offline_pushes_edge_and_clears_on_online() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_host_offline(true);
        assert!(rx.has_changed().unwrap_or(false));
        assert!(rx.borrow_and_update().host_offline);
        cache.set_host_offline(false);
        assert!(rx.has_changed().unwrap_or(false));
        assert!(!rx.borrow_and_update().host_offline);
    }

    #[test]
    fn set_host_offline_idempotent_same_verdict() {
        // Route-event storms repeat the same verdict; the stream must
        // not be spammed with identical snapshots.
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_host_offline(false);
        assert!(!rx.has_changed().unwrap_or(false));
        cache.set_host_offline(true);
        rx.borrow_and_update();
        cache.set_host_offline(true);
        assert!(!rx.has_changed().unwrap_or(false));
    }

    // --- Exit egress surface (doc 62 item 5) -----------------------

    #[test]
    fn default_snapshot_reports_egress_alive() {
        assert!(!WarrenStatusSnapshot::default().exit_egress_dead);
        assert!(!WarrenStatusCache::new().snapshot().exit_egress_dead);
    }

    #[test]
    fn set_exit_egress_dead_pushes_edge_and_clears_on_recovery() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_exit_egress_dead(true);
        assert!(rx.has_changed().unwrap_or(false));
        assert!(rx.borrow_and_update().exit_egress_dead);
        cache.set_exit_egress_dead(false);
        assert!(rx.has_changed().unwrap_or(false));
        assert!(!rx.borrow_and_update().exit_egress_dead);
    }

    #[test]
    fn set_exit_egress_dead_idempotent_same_verdict() {
        // The probe publishes on edges, but a defensive daemon-side
        // clear (tunnel leaving Connected) may repeat the verdict; the
        // stream must not be spammed with identical snapshots.
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_exit_egress_dead(false);
        assert!(!rx.has_changed().unwrap_or(false));
        cache.set_exit_egress_dead(true);
        rx.borrow_and_update();
        cache.set_exit_egress_dead(true);
        assert!(!rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn set_exit_egress_dead_does_not_perturb_host_offline() {
        // The two "interrupted" causes are independent bits: an egress
        // verdict must never mask or clear the offline monitor's.
        let cache = WarrenStatusCache::new();
        cache.set_host_offline(true);
        cache.set_exit_egress_dead(true);
        let s = cache.snapshot();
        assert!(s.host_offline && s.exit_egress_dead);
        cache.set_exit_egress_dead(false);
        assert!(cache.snapshot().host_offline, "host_offline untouched");
    }

    // --- Network info surface --------------------------------------

    fn beta_network_info() -> NetworkInfoSnapshot {
        NetworkInfoSnapshot {
            environment: "beta".to_owned(),
            degraded: true,
            default_rate_bps: Some(20_000_000),
            payments_enabled: false,
        }
    }

    #[test]
    fn default_snapshot_has_no_network_info() {
        assert!(WarrenStatusSnapshot::default().network_info.is_none());
        assert!(WarrenStatusCache::new().snapshot().network_info.is_none());
    }

    #[test]
    fn set_network_info_pushes_payload_to_subscribers() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_network_info(Some(beta_network_info()));
        assert!(rx.has_changed().unwrap_or(false));
        let snap = rx.borrow_and_update().clone();
        assert_eq!(snap.network_info, Some(beta_network_info()));
    }

    #[test]
    fn set_network_info_idempotent_same_payload() {
        // The refresh loop re-fetches periodically; an unchanged
        // descriptor must not spam the status stream.
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.set_network_info(Some(beta_network_info()));
        rx.borrow_and_update();
        cache.set_network_info(Some(beta_network_info()));
        assert!(!rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn set_network_info_does_not_perturb_other_fields() {
        let cache = WarrenStatusCache::new();
        cache.record_reconnect();
        cache.set_network_info(Some(beta_network_info()));
        let snap = cache.snapshot();
        assert_eq!(snap.reconnect_count, 1);
        assert_eq!(snap.network_info, Some(beta_network_info()));
    }

    // --- Failover surface ---------------------------------------

    #[test]
    fn default_snapshot_has_failover_count_zero_and_age_none() {
        let s = WarrenStatusSnapshot::default();
        assert_eq!(s.failover_count, 0);
        assert_eq!(s.last_failover_age, None);
    }

    #[test]
    fn record_failover_bumps_counter_and_resets_age() {
        let cache = WarrenStatusCache::new();
        cache.record_failover();
        let snap = cache.snapshot();
        assert_eq!(snap.failover_count, 1, "first failover bumps from 0 -> 1");
        let age = snap
            .last_failover_age
            .expect("last_failover_age must be Some after record_failover");
        assert!(age < Duration::from_secs(1));
    }

    #[test]
    fn record_failover_increments_monotonically() {
        let cache = WarrenStatusCache::new();
        cache.record_failover();
        cache.record_failover();
        cache.record_failover();
        assert_eq!(cache.snapshot().failover_count, 3);
    }

    #[test]
    fn record_failover_saturates_at_u32_max() {
        let cache = WarrenStatusCache::new();
        {
            let mut inner = cache.state.write().unwrap();
            inner.failover_count = u32::MAX - 1;
        }
        cache.record_failover();
        cache.record_failover();
        assert_eq!(cache.snapshot().failover_count, u32::MAX);
    }

    #[test]
    fn subscribe_yields_snapshot_on_record_failover() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.record_failover();
        assert!(rx.has_changed().unwrap_or(false));
        let s = rx.borrow_and_update();
        assert_eq!(s.failover_count, 1);
    }

    // --- Pubkey-mismatch surface --------------------

    fn make_mismatch() -> PubkeyMismatchPending {
        PubkeyMismatchPending {
            exit_id_hex: "aa".repeat(16),
            pinned_pubkey_hex: "bb".repeat(32),
            observed_pubkey_hex: "cc".repeat(32),
            country_code: "fr".to_owned(),
            city: "Paris".to_owned(),
        }
    }

    #[test]
    fn default_snapshot_has_no_pubkey_mismatch_pending() {
        assert!(
            WarrenStatusSnapshot::default()
                .pubkey_mismatch_pending
                .is_none()
        );
    }

    #[test]
    fn set_pubkey_mismatch_pending_pushes_payload_and_clears_on_clear() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        let mismatch = make_mismatch();
        cache.set_pubkey_mismatch_pending(mismatch.clone());
        assert!(rx.has_changed().unwrap_or(false));
        let snap_set = rx.borrow_and_update().clone();
        assert_eq!(snap_set.pubkey_mismatch_pending.as_ref(), Some(&mismatch));
        cache.clear_pubkey_mismatch_pending();
        assert!(rx.has_changed().unwrap_or(false));
        let snap_clear = rx.borrow_and_update().clone();
        assert!(snap_clear.pubkey_mismatch_pending.is_none());
    }

    #[test]
    fn set_pubkey_mismatch_pending_idempotent_same_payload() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        let mismatch = make_mismatch();
        cache.set_pubkey_mismatch_pending(mismatch.clone());
        rx.borrow_and_update();
        cache.set_pubkey_mismatch_pending(mismatch);
        assert!(!rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn clear_pubkey_mismatch_pending_idempotent_when_already_clear() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.clear_pubkey_mismatch_pending();
        assert!(!rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn record_failover_does_not_perturb_reconnect_count_or_nat_pmp() {
        let cache = WarrenStatusCache::new();
        cache.record_reconnect();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 60123,
                lifetime_secs: 3600,
                attempts_remaining: Some(4),
                window_reset_secs: 0,
            },
        );
        cache.record_failover();
        let s = cache.snapshot();
        assert_eq!(s.failover_count, 1);
        assert_eq!(
            s.reconnect_count, 1,
            "failover must not touch reconnect counter"
        );
        assert!(
            matches!(
                s.nat_pmp_mappings.first().map(|m| &m.state),
                Some(NatPmpStateSnapshot::Mapped {
                    external_port: 60123,
                    ..
                })
            ),
            "failover must not touch nat_pmp state"
        );
    }

    #[test]
    fn subscribe_yields_snapshot_on_record_nat_pmp_event() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.record_nat_pmp_event(
            rule(22),
            NatPmpEvent::Mapped {
                external_port: 51234,
                lifetime_secs: 60,
                attempts_remaining: Some(5),
                window_reset_secs: 0,
            },
        );
        assert!(rx.has_changed().unwrap_or(false));
        let s = rx.borrow_and_update();
        assert!(matches!(
            s.nat_pmp_mappings.first().map(|m| &m.state),
            Some(NatPmpStateSnapshot::Mapped {
                external_port: 51234,
                ..
            })
        ));
    }
}
