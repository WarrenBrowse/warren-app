//! ADR 36 client-side drain reactor: proactive make-before-break migration.
//!
//! When an operator drains an exit for maintenance, the exit emits a sealed
//! `ExitDraining` advisory mid-session on the live downlink. warren-core
//! decodes it in the production downlink pump and publishes it on the
//! supervisor's `ExitDrainingChannel`. This reactor consumes that advisory
//! and proactively migrates the tunnel OFF the draining exit BEFORE its
//! hard-close deadline, so the user never sees the abrupt drop.
//!
//! Migration mechanism: escalate through the pump-error channel, exactly
//! like [`crate::migration_watchdog`]. The state machine rebuilds the tunnel
//! with `retry_attempt > 0`, which routes through
//! `assemble_failover_for_attempt` and EXCLUDES the just-drained exit
//! (`warren_last_exit_pubkey`), so the rebuild lands on a DIFFERENT exit
//! (same country preferred). The exit's own hard close at the deadline stays
//! the backstop for any straggler (it surfaces as the ordinary pump-recv
//! error escalation); this reactor is the proactive, gap-free path.
//!
//! Anti-stampede: every client attached to a drained exit receives the
//! advisory within a few seconds of one another. Reconnecting them all at
//! once would hammer the relay and the alternative exit. So the reactor
//! waits a per-client jittered delay before escalating, spreading the herd
//! across a window bounded so the migration still completes before the
//! deadline (minus a safety margin).

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

/// IO surface consumed by [`run_drain_reactor`]. Implemented for production
/// by [`RealDrainReactorIo`] and by a fake in the unit tests.
pub(crate) trait DrainReactorIo {
    /// Resolve on the next drain advisory, or `None` when the channel closes
    /// (tunnel teardown): the loop exits.
    async fn next_advisory(&mut self) -> Option<ExitDrainAdvisory>;
    /// Current wall-clock unix seconds.
    fn now_unix(&self) -> u64;
    /// Uniform draw in `[0.0, 1.0)` for the anti-stampede jitter.
    fn jitter_fraction(&mut self) -> f64;
    /// Sleep for the jitter window.
    async fn sleep(&mut self, dur: Duration);
    /// Escalate to the state machine (surfaces as a pump error → tunnel
    /// rebuild that excludes the drained exit).
    fn escalate(&mut self, msg: String);
}

/// Consume drain advisories and proactively migrate off the draining exit.
///
/// One escalation per reactor: the escalation tears down and rebuilds the
/// whole tunnel (fresh supervisor + pumps + a new reactor), so after
/// escalating we return and let the rebuilt tunnel install its own reactor.
pub(crate) async fn run_drain_reactor<I: DrainReactorIo>(io: &mut I) {
    let Some(advisory) = io.next_advisory().await else {
        return;
    };
    let delay = jitter_delay(
        advisory.deadline_unix_secs,
        io.now_unix(),
        io.jitter_fraction(),
    );
    log::info!(
        "Warren drain reactor: exit draining (reason={}, deadline={}); \
         migrating to an alternative exit in {:?} (make-before-break)",
        advisory.reason_code,
        advisory.deadline_unix_secs,
        delay
    );
    io.sleep(delay).await;
    io.escalate(format!(
        "exit draining (reason={}); proactively migrating to an alternative exit \
         before the maintenance deadline",
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

    struct FakeIo {
        advisories: VecDeque<Option<ExitDrainAdvisory>>,
        now: u64,
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

    #[tokio::test]
    async fn reacts_to_advisory_with_one_jittered_escalation() {
        let mut io = FakeIo {
            advisories: VecDeque::from([Some(ExitDrainAdvisory {
                deadline_unix_secs: 1_000,
                reason_code: 7,
            })]),
            now: 940, // 60s budget => usable 55 => window capped at 20
            fraction: 0.5,
            slept: Vec::new(),
            escalations: Vec::new(),
        };

        run_drain_reactor(&mut io).await;

        assert_eq!(
            io.escalations.len(),
            1,
            "a single drain advisory must trigger exactly one migration escalation"
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
    }

    #[tokio::test]
    async fn channel_close_exits_without_escalation() {
        let mut io = FakeIo {
            advisories: VecDeque::new(), // no advisory: channel closed at teardown
            now: 0,
            fraction: 0.0,
            slept: Vec::new(),
            escalations: Vec::new(),
        };

        run_drain_reactor(&mut io).await;

        assert!(
            io.escalations.is_empty(),
            "a closed drain channel (teardown) must never escalate"
        );
        assert!(io.slept.is_empty(), "no advisory => no jitter sleep");
    }
}
