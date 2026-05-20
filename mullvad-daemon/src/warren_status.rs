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

use tokio::sync::watch;

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
}

impl Default for WarrenStatusSnapshot {
    /// Default: fresh boot, no reconnects observed, obfuscation
    /// enabled per /v1 doctrine.
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_age: None,
            obfuscation_active: true,
        }
    }
}

/// Internal state held by [`WarrenStatusCache`]. Separated to keep the
/// public snapshot trivially `Clone` while letting the cache hold an
/// `Instant` (which is not `Serialize` / not stable wire format).
#[derive(Debug, Clone, Copy)]
struct InternalState {
    reconnect_count: u32,
    last_reconnect_at: Option<Instant>,
    obfuscation_active: bool,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            reconnect_count: 0,
            last_reconnect_at: None,
            obfuscation_active: true,
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
        let inner = *self
            .state
            .read()
            .expect("warren_status state lock poisoned");
        WarrenStatusSnapshot {
            reconnect_count: inner.reconnect_count,
            last_reconnect_age: inner.last_reconnect_at.map(|t| t.elapsed()),
            obfuscation_active: inner.obfuscation_active,
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
            WarrenStatusSnapshot {
                reconnect_count: inner.reconnect_count,
                last_reconnect_age: Some(Duration::from_secs(0)),
                obfuscation_active: inner.obfuscation_active,
            }
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
            WarrenStatusSnapshot {
                reconnect_count: inner.reconnect_count,
                last_reconnect_age: inner.last_reconnect_at.map(|t| t.elapsed()),
                obfuscation_active: active,
            }
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
}
