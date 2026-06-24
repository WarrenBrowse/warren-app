//! ADR 36 client-side drain reactor: proactive reconnect on exit maintenance.
//!
//! When an operator drains an exit, the exit emits a sealed `ExitDraining`
//! advisory mid-session; warren-core decodes it in the downlink pump and
//! publishes it on the supervisor's `ExitDrainingChannel`. This reactor
//! consumes it and PROACTIVELY triggers a tunnel reconnect before the exit's
//! hard-close deadline, so the drop happens in a controlled window instead of
//! as an abrupt mid-use cut.
//!
//! Exit EXCLUSION (landing the reconnect on a DIFFERENT exit) is provided by
//! the *ambient* drain path, NOT by this reactor. The backend marks the
//! draining exit inactive; the signed relay-list / multi-hop directory refresh
//! propagates `active=false`; the weighted selector then stops offering it.
//! The state-machine failover is deliberately NOT relied upon here: a
//! connected-tunnel close re-enters Connecting with `retry_attempt = 0`
//! (`connected_state.rs`), so `is_failover` never arms, and even when it does
//! `assemble_failover_for_attempt` only rewrites the single-hop fields and
//! passes the multi-hop circuit through unchanged. Plumbing true in-band
//! exit-exclusion into the multi-hop directory selection is a tracked
//! follow-up (see `docs/36`); until then this reactor is a proactive nudge,
//! and the ambient refresh + the exit's own hard close are the exclusion
//! backstops.
//!
//! Storm guard: if the proactive reconnect re-lands on the SAME still-draining
//! exit (the ambient refresh has not caught up yet), that tunnel's fresh
//! reactor would escalate again, a reconnect loop the flap detector would
//! eventually turn into a BLOCKED state (worse UX than the abrupt drop). A
//! process-global cooldown ([`DRAIN_RECONNECT_COOLDOWN`]) suppresses a second
//! drain escalation for a window, deferring to the exit hard-close + the
//! ambient refresh rather than storming.
//!
//! Anti-stampede: many clients on one exit are drained within seconds of each
//! other; the reactor waits a per-client jittered delay before escalating to
//! spread the reconnect herd across the relay + the alternative exit.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use warren_client::supervised_pump::ExitDrainAdvisory;

/// Pump-error escalation channel, shared with the pumps and the migration
/// watchdog: the first task to take the `oneshot` reports the fatal/transient
/// cause to the state machine, which rebuilds the tunnel.
type PumpErrorTx = std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

/// Floor kept between the jittered reconnect and the exit's hard-close
/// deadline, so the proactive migration finishes before the backstop close.
const DEADLINE_SAFETY_MARGIN: Duration = Duration::from_secs(5);

/// Upper bound on the anti-stampede spread, even when the operator sets a
/// very distant (or soft / `u64::MAX`) deadline.
const MAX_JITTER: Duration = Duration::from_secs(20);

/// Minimum interval between two drain-triggered reconnects, process-wide.
/// Bounds a reconnect loop when the proactive reconnect re-lands on a still
/// draining exit before the ambient relay-list refresh has excluded it.
const DRAIN_RECONNECT_COOLDOWN: Duration = Duration::from_secs(120);

/// Unix seconds of the last drain-triggered escalation, process-wide (`0` =
/// never). Shared across tunnel instances so a rebuilt tunnel's fresh reactor
/// observes a recent drain reconnect and backs off instead of storming.
static LAST_DRAIN_ESCALATION_UNIX: AtomicU64 = AtomicU64::new(0);

/// Anti-stampede delay before escalating a drain migration.
///
/// `fraction` is a uniform draw in `[0.0, 1.0)` (per-client, so the herd
/// spreads); production draws it from the RNG, tests pass it in. The window
/// is `min(MAX_JITTER, deadline - now - SAFETY_MARGIN)`, clamped at zero:
///
/// - a soft drain (`deadline == u64::MAX`, no hard deadline) caps at
///   `MAX_JITTER`,
/// - a deadline already within the safety margin yields `ZERO` (escalate
///   now: the close is imminent, no time to spread).
pub(crate) fn jitter_delay(deadline_unix_secs: u64, now_unix_secs: u64, fraction: f64) -> Duration {
    let budget = deadline_unix_secs.saturating_sub(now_unix_secs);
    let usable = budget.saturating_sub(DEADLINE_SAFETY_MARGIN.as_secs());
    let window = usable.min(MAX_JITTER.as_secs());
    let frac = fraction.clamp(0.0, 1.0);
    Duration::from_secs_f64(window as f64 * frac)
}

/// `true` when a drain reconnect fired less than [`DRAIN_RECONNECT_COOLDOWN`]
/// ago and a second one must be suppressed. `last_unix == 0` (never) is always
/// allowed. Robust to clock skew: a `last` in the future reads as not-elapsed
/// (suppress), erring on the safe side.
pub(crate) fn within_cooldown(now_unix: u64, last_unix: u64) -> bool {
    if last_unix == 0 {
        return false;
    }
    now_unix.saturating_sub(last_unix) < DRAIN_RECONNECT_COOLDOWN.as_secs() || last_unix > now_unix
}

/// IO surface consumed by [`run_drain_reactor`]. Implemented for production
/// by [`RealDrainReactorIo`] and by a fake in the unit tests.
pub(crate) trait DrainReactorIo {
    /// Resolve on the next drain advisory, or `None` when the channel closes
    /// (tunnel teardown): the loop exits.
    async fn next_advisory(&mut self) -> Option<ExitDrainAdvisory>;
    /// Current wall-clock unix seconds.
    fn now_unix(&self) -> u64;
    /// Unix seconds of the last process-wide drain escalation (`0` = never).
    fn last_drain_escalation_unix(&self) -> u64;
    /// Record that a drain escalation fired at `now_unix` (process-wide).
    fn record_drain_escalation(&mut self, now_unix: u64);
    /// Uniform draw in `[0.0, 1.0)` for the anti-stampede jitter.
    fn jitter_fraction(&mut self) -> f64;
    /// Sleep for the jitter window.
    async fn sleep(&mut self, dur: Duration);
    /// Escalate to the state machine (surfaces as a pump error → tunnel
    /// rebuild; exit exclusion is the ambient drain's job, see module docs).
    fn escalate(&mut self, msg: String);
}

/// Consume drain advisories and proactively reconnect off the draining exit.
///
/// One escalation per reactor: the escalation tears down and rebuilds the
/// whole tunnel (fresh supervisor + pumps + a new reactor), so after
/// escalating we return and let the rebuilt tunnel install its own reactor.
/// The process-wide cooldown then suppresses that fresh reactor from
/// re-escalating if the rebuild re-landed on the same still-draining exit.
pub(crate) async fn run_drain_reactor<I: DrainReactorIo>(io: &mut I) {
    let Some(advisory) = io.next_advisory().await else {
        return;
    };
    let now = io.now_unix();
    if within_cooldown(now, io.last_drain_escalation_unix()) {
        log::info!(
            "Warren drain reactor: a drain reconnect fired <{}s ago; deferring to \
             the ambient relay-list drain + the exit hard-close backstop instead of \
             reconnecting again (avoids a storm against a still-draining exit)",
            DRAIN_RECONNECT_COOLDOWN.as_secs()
        );
        return;
    }
    // Record at decision time (before the jitter sleep) so a rapidly rebuilt
    // tunnel's reactor sees this drain reconnect and backs off.
    io.record_drain_escalation(now);
    let delay = jitter_delay(advisory.deadline_unix_secs, now, io.jitter_fraction());
    log::info!(
        "Warren drain reactor: exit draining (reason={}, deadline={}); \
         reconnecting in {:?} (proactive, exclusion via ambient drain)",
        advisory.reason_code,
        advisory.deadline_unix_secs,
        delay
    );
    io.sleep(delay).await;
    io.escalate(format!(
        "exit draining (reason={}); proactively reconnecting before the maintenance deadline",
        advisory.reason_code
    ));
}

/// Production [`DrainReactorIo`]: a subscriber on the supervisor's
/// `ExitDrainingChannel` plus the shared pump-error escalation handle.
pub(crate) struct RealDrainReactorIo {
    pub drain_sub: tokio::sync::watch::Receiver<Option<ExitDrainAdvisory>>,
    pub pump_error_tx: PumpErrorTx,
}

impl DrainReactorIo for RealDrainReactorIo {
    async fn next_advisory(&mut self) -> Option<ExitDrainAdvisory> {
        loop {
            if self.drain_sub.changed().await.is_err() {
                return None;
            }
            if let Some(adv) = *self.drain_sub.borrow_and_update() {
                return Some(adv);
            }
        }
    }

    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn last_drain_escalation_unix(&self) -> u64 {
        LAST_DRAIN_ESCALATION_UNIX.load(Ordering::Relaxed)
    }

    fn record_drain_escalation(&mut self, now_unix: u64) {
        LAST_DRAIN_ESCALATION_UNIX.store(now_unix, Ordering::Relaxed);
    }

    fn jitter_fraction(&mut self) -> f64 {
        rand::random::<f64>()
    }

    async fn sleep(&mut self, dur: Duration) {
        tokio::time::sleep(dur).await;
    }

    fn escalate(&mut self, msg: String) {
        log::warn!("Warren drain reactor: escalating to the state machine: {msg}");
        if let Some(tx) = self
            .pump_error_tx
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = tx.send(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn jitter_soft_drain_caps_at_max_window() {
        // A soft drain has no hard deadline (u64::MAX): the window must still
        // be capped at MAX_JITTER, never the (astronomically large) budget.
        let d = jitter_delay(u64::MAX, 1_000, 1.0);
        assert!(
            d <= MAX_JITTER && d >= MAX_JITTER - Duration::from_secs(1),
            "soft drain must cap the jitter at MAX_JITTER, got {d:?}"
        );
    }

    #[test]
    fn jitter_imminent_deadline_is_zero() {
        // Deadline inside the safety margin (budget 3 < margin 5): no time to
        // spread, escalate immediately regardless of the fraction.
        assert_eq!(jitter_delay(1_003, 1_000, 0.9), Duration::ZERO);
    }

    #[test]
    fn jitter_scales_linearly_with_fraction() {
        // budget 30 => usable 25 => window min(25, 20) = 20.
        assert_eq!(jitter_delay(1_030, 1_000, 0.0), Duration::ZERO);
        assert_eq!(jitter_delay(1_030, 1_000, 0.5), Duration::from_secs(10));
    }

    #[test]
    fn cooldown_allows_first_escalation_and_suppresses_a_quick_second() {
        // Never escalated (0) => allowed.
        assert!(!within_cooldown(1_000, 0));
        // 30s after a drain reconnect (< 120s cooldown) => suppress.
        assert!(within_cooldown(1_030, 1_000));
        // 200s after (> cooldown) => allowed again.
        assert!(!within_cooldown(1_200, 1_000));
        // Clock skew (last in the future) => suppress, erring safe.
        assert!(within_cooldown(900, 1_000));
    }

    struct FakeIo {
        advisories: VecDeque<Option<ExitDrainAdvisory>>,
        now: u64,
        last_escalation: u64,
        fraction: f64,
        slept: Vec<Duration>,
        escalations: Vec<String>,
    }

    impl DrainReactorIo for FakeIo {
        async fn next_advisory(&mut self) -> Option<ExitDrainAdvisory> {
            self.advisories.pop_front().flatten()
        }
        fn now_unix(&self) -> u64 {
            self.now
        }
        fn last_drain_escalation_unix(&self) -> u64 {
            self.last_escalation
        }
        fn record_drain_escalation(&mut self, now_unix: u64) {
            self.last_escalation = now_unix;
        }
        fn jitter_fraction(&mut self) -> f64 {
            self.fraction
        }
        async fn sleep(&mut self, dur: Duration) {
            self.slept.push(dur);
        }
        fn escalate(&mut self, msg: String) {
            self.escalations.push(msg);
        }
    }

    fn fake_with(advisory: Option<ExitDrainAdvisory>, now: u64, last_escalation: u64) -> FakeIo {
        FakeIo {
            advisories: VecDeque::from([advisory]),
            now,
            last_escalation,
            fraction: 0.5,
            slept: Vec::new(),
            escalations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn reacts_to_advisory_with_one_jittered_escalation() {
        // 60s budget => usable 55 => window capped at 20; fraction 0.5 => 10s.
        let mut io = fake_with(
            Some(ExitDrainAdvisory {
                deadline_unix_secs: 1_000,
                reason_code: 7,
            }),
            940,
            0, // never escalated => cooldown does not suppress
        );

        run_drain_reactor(&mut io).await;

        assert_eq!(
            io.escalations.len(),
            1,
            "a single drain advisory must trigger exactly one reconnect escalation"
        );
        assert!(
            io.escalations[0].contains("draining"),
            "the escalation message must name the drain as the cause: {:?}",
            io.escalations[0]
        );
        assert_eq!(
            io.slept,
            vec![Duration::from_secs(10)],
            "must apply the anti-stampede jitter (window 20s * fraction 0.5) before escalating"
        );
        assert_eq!(
            io.last_escalation, 940,
            "the escalation must be recorded at decision time for the cooldown"
        );
    }

    #[tokio::test]
    async fn cooldown_suppresses_a_second_drain_reconnect() {
        // A drain reconnect fired 30s ago (< 120s cooldown): the fresh reactor
        // on the rebuilt tunnel must NOT escalate again (storm guard).
        let mut io = fake_with(
            Some(ExitDrainAdvisory {
                deadline_unix_secs: 2_000,
                reason_code: 0,
            }),
            1_030,
            1_000,
        );

        run_drain_reactor(&mut io).await;

        assert!(
            io.escalations.is_empty(),
            "within the cooldown the reactor must defer instead of re-escalating"
        );
        assert!(
            io.slept.is_empty(),
            "suppressed reactor must not even jitter"
        );
    }

    #[tokio::test]
    async fn channel_close_exits_without_escalation() {
        let mut io = fake_with(None, 0, 0); // no advisory: channel closed at teardown

        run_drain_reactor(&mut io).await;

        assert!(
            io.escalations.is_empty(),
            "a closed drain channel (teardown) must never escalate"
        );
        assert!(io.slept.is_empty(), "no advisory => no jitter sleep");
    }
}
