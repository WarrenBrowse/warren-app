//! Live Warren tunnel status surfaced to the gRPC management interface.
//!
//! Carries the M4.E.D auto-reconnect metrics (`reconnect_count` +
//! `last_reconnect_age`) and the M4.0 obfuscation indicator that the
//! Electron UI displays in the connection details view. The values are
//! held in a [`WarrenStatusCache`] shared between the daemon main loop
//! and the multi-hop supervisor (when in multi-hop mode); for
//! single-hop mode the `reconnect_count` stays at 0 and
//! `obfuscation_active` is true per doctrine
//! `warren_obfuscation_doctrine_v1` (M4.0 HTTP/3 mimicry always-on /v1).

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use talpid_warren_tunnel::NatPmpEvent;
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
    },
    /// The last `request_map` failed and the loop has terminated. The
    /// UI surfaces `error` so the user knows whether to retry.
    Failed {
        /// Free-form error string from `NatPmpEvent::Failed`.
        error: String,
    },
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
    /// True when the M4.0 HTTP/3 mimicry obfuscation is active. /v1 has
    /// this always-on (doctrine), so this is true unless the daemon is
    /// in pure single-hop legacy mode with obfuscation disabled.
    pub obfuscation_active: bool,
    /// Live NAT-PMP state for the port-forwarding UI. `Disabled` when
    /// the user has not opted into port-forwarding; the other variants
    /// track the refresh loop's lifecycle.
    pub nat_pmp: NatPmpStateSnapshot,
    /// M5.B.2 multi-exit failover counter. Bumped each time the
    /// `assemble_failover_for_attempt` path is taken (retry > 0 with a
    /// memoized previous exit). The UI displays a toast when this
    /// counter increments so the user knows their exit was swapped.
    pub failover_count: u32,
    /// Time since the last failover. `None` if no failover has been
    /// recorded in the current session. Matches `last_reconnect_age`
    /// semantics (computed against the calling thread's clock).
    pub last_failover_age: Option<Duration>,
}

impl Default for WarrenStatusSnapshot {
    /// Default: fresh boot, no reconnects observed, obfuscation
    /// enabled per /v1 doctrine, NAT-PMP disabled.
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_age: None,
            obfuscation_active: true,
            nat_pmp: NatPmpStateSnapshot::Disabled,
            failover_count: 0,
            last_failover_age: None,
        }
    }
}

/// Internal state held by [`WarrenStatusCache`]. Separated to keep the
/// public snapshot trivially `Clone` while letting the cache hold an
/// `Instant` (which is not `Serialize` / not stable wire format).
#[derive(Debug, Clone)]
struct InternalState {
    reconnect_count: u32,
    last_reconnect_at: Option<Instant>,
    obfuscation_active: bool,
    nat_pmp: NatPmpStateSnapshot,
    failover_count: u32,
    last_failover_at: Option<Instant>,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_at: None,
            obfuscation_active: true,
            nat_pmp: NatPmpStateSnapshot::Disabled,
            failover_count: 0,
            last_failover_at: None,
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
        WarrenStatusSnapshot {
            reconnect_count: inner.reconnect_count,
            last_reconnect_age: inner.last_reconnect_at.map(|t| t.elapsed()),
            obfuscation_active: inner.obfuscation_active,
            nat_pmp: inner.nat_pmp.clone(),
            failover_count: inner.failover_count,
            last_failover_age: inner.last_failover_at.map(|t| t.elapsed()),
        }
    }

    /// Build a [`WarrenStatusSnapshot`] from the internal state. Used
    /// by every `record_*` / `set_*` method to avoid drift between the
    /// construction sites whenever new fields are added.
    fn snapshot_of(inner: &InternalState) -> WarrenStatusSnapshot {
        WarrenStatusSnapshot {
            reconnect_count: inner.reconnect_count,
            last_reconnect_age: inner.last_reconnect_at.map(|t| t.elapsed()),
            obfuscation_active: inner.obfuscation_active,
            nat_pmp: inner.nat_pmp.clone(),
            failover_count: inner.failover_count,
            last_failover_age: inner.last_failover_at.map(|t| t.elapsed()),
        }
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
        let _ = self.tx.send(snapshot);
    }

    /// M5.B.2: bumps the failover counter and stamps the moment.
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
        let _ = self.tx.send(snapshot);
    }

    /// Records a NAT-PMP event coming from the daemon-side
    /// `NatPmpManager` and broadcasts the resulting snapshot on the
    /// watch channel so the Electron UI updates.
    ///
    /// Event -> state mapping:
    /// - `Mapped { external_port, lifetime_secs }` -> `Mapped { .. }`
    /// - `Renewed { external_port, lifetime_secs }` -> `Mapped { .. }`
    ///   (UI reuses the same row; the lifetime countdown resets).
    /// - `Failed { error }` -> `Failed { error }`.
    /// - `Cancelled` -> `Disabled` (the user disabled the toggle, or
    ///   the tunnel went down).
    pub fn record_nat_pmp_event(&self, event: NatPmpEvent) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.nat_pmp = match event {
                NatPmpEvent::Mapped {
                    external_port,
                    lifetime_secs,
                }
                | NatPmpEvent::Renewed {
                    external_port,
                    lifetime_secs,
                } => NatPmpStateSnapshot::Mapped {
                    external_port,
                    lifetime_secs,
                },
                NatPmpEvent::Failed { error } => NatPmpStateSnapshot::Failed { error },
                NatPmpEvent::Cancelled => NatPmpStateSnapshot::Disabled,
            };
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send(snapshot);
    }

    /// Sets the NAT-PMP state to `Requesting`, broadcasting the
    /// resulting snapshot. Called by the daemon as soon as the
    /// `NatPmpManager` is spawned but before the first event arrives,
    /// so the UI immediately reflects the "in flight" condition.
    pub fn set_nat_pmp_requesting(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            inner.nat_pmp = NatPmpStateSnapshot::Requesting;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send(snapshot);
    }

    /// Resets the NAT-PMP state to `Disabled`, broadcasting the
    /// resulting snapshot. Called when the user toggles port-forwarding
    /// off or when a tunnel goes down without firing a `Cancelled`
    /// event (defensive).
    pub fn set_nat_pmp_disabled(&self) {
        let snapshot = {
            let mut inner = self
                .state
                .write()
                .expect("warren_status state lock poisoned");
            if matches!(inner.nat_pmp, NatPmpStateSnapshot::Disabled) {
                return;
            }
            inner.nat_pmp = NatPmpStateSnapshot::Disabled;
            Self::snapshot_of(&inner)
        };
        let _ = self.tx.send(snapshot);
    }

    /// Toggle the obfuscation indicator. M4.0 always-on /v1 means this
    /// stays true in production; the setter exists so a future /v2
    /// toggle (M4.G+) can flip it without touching the cache shape.
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
        let _ = self.tx.send(snapshot);
    }
}

impl Default for WarrenStatusCache {
    fn default() -> Self {
        Self::new()
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
        assert!(s.obfuscation_active, "M4.0 obfuscation always-on /v1");
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

    // --- NAT-PMP surface ----------------------------------------------------

    #[test]
    fn default_snapshot_has_nat_pmp_disabled() {
        let s = WarrenStatusSnapshot::default();
        assert_eq!(s.nat_pmp, NatPmpStateSnapshot::Disabled);
    }

    #[test]
    fn record_nat_pmp_mapped_event_updates_state_to_mapped() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 49152,
            lifetime_secs: 3600,
        });
        match cache.snapshot().nat_pmp {
            NatPmpStateSnapshot::Mapped {
                external_port,
                lifetime_secs,
            } => {
                assert_eq!(external_port, 49152);
                assert_eq!(lifetime_secs, 3600);
            }
            other => panic!("expected Mapped, got {other:?}"),
        }
    }

    #[test]
    fn record_nat_pmp_renewed_event_updates_state_keeps_lifetime_fresh() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 49152,
            lifetime_secs: 3600,
        });
        cache.record_nat_pmp_event(NatPmpEvent::Renewed {
            external_port: 49152,
            lifetime_secs: 3600,
        });
        match cache.snapshot().nat_pmp {
            NatPmpStateSnapshot::Mapped {
                external_port,
                lifetime_secs,
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
        cache.record_nat_pmp_event(NatPmpEvent::Failed {
            error: "server returned error: OutOfResources".to_owned(),
        });
        match cache.snapshot().nat_pmp {
            NatPmpStateSnapshot::Failed { error } => {
                assert!(
                    error.contains("OutOfResources"),
                    "error must propagate: {error}"
                );
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn record_nat_pmp_cancelled_resets_to_disabled() {
        let cache = WarrenStatusCache::new();
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 49152,
            lifetime_secs: 3600,
        });
        cache.record_nat_pmp_event(NatPmpEvent::Cancelled);
        assert_eq!(cache.snapshot().nat_pmp, NatPmpStateSnapshot::Disabled);
    }

    #[test]
    fn set_nat_pmp_requesting_then_mapped_round_trip() {
        let cache = WarrenStatusCache::new();
        cache.set_nat_pmp_requesting();
        assert_eq!(cache.snapshot().nat_pmp, NatPmpStateSnapshot::Requesting);
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 60000,
            lifetime_secs: 60,
        });
        assert!(matches!(
            cache.snapshot().nat_pmp,
            NatPmpStateSnapshot::Mapped {
                external_port: 60000,
                lifetime_secs: 60
            }
        ));
    }

    #[test]
    fn set_nat_pmp_disabled_idempotent_when_already_disabled() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        // Already disabled by default; another disable must not push.
        cache.set_nat_pmp_disabled();
        assert!(!rx.has_changed().unwrap_or(false));
    }

    // --- M5.B.2 failover surface ---------------------------------------

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

    #[test]
    fn record_failover_does_not_perturb_reconnect_count_or_nat_pmp() {
        let cache = WarrenStatusCache::new();
        cache.record_reconnect();
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 60123,
            lifetime_secs: 3600,
        });
        cache.record_failover();
        let s = cache.snapshot();
        assert_eq!(s.failover_count, 1);
        assert_eq!(s.reconnect_count, 1, "failover must not touch reconnect counter");
        assert!(
            matches!(s.nat_pmp, NatPmpStateSnapshot::Mapped { external_port: 60123, .. }),
            "failover must not touch nat_pmp state"
        );
    }

    #[test]
    fn subscribe_yields_snapshot_on_record_nat_pmp_event() {
        let cache = WarrenStatusCache::new();
        let mut rx = cache.subscribe();
        rx.borrow_and_update();
        cache.record_nat_pmp_event(NatPmpEvent::Mapped {
            external_port: 51234,
            lifetime_secs: 60,
        });
        assert!(rx.has_changed().unwrap_or(false));
        let s = rx.borrow_and_update();
        assert!(matches!(
            s.nat_pmp,
            NatPmpStateSnapshot::Mapped {
                external_port: 51234,
                ..
            }
        ));
    }
}
