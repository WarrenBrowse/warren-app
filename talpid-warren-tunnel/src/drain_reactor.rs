//! ADR 36 client-side drain reactor: proactive reconnect on exit maintenance.
//!
//! When an operator drains an exit, the exit emits a sealed `ExitDraining`
//! advisory mid-session; warrenguard-transport decodes it in the downlink pump and
//! publishes it on the supervisor's `ExitDrainingChannel`. This reactor
//! consumes it and PROACTIVELY triggers a tunnel reconnect before the exit's
//! hard-close deadline, so the drop happens in a controlled window instead of
//! as an abrupt mid-use cut.
//!
//! Exit EXCLUSION (landing the reconnect on a DIFFERENT exit) is driven by an
//! IN-BAND avoid-set, NOT the state-machine failover. Before escalating, the
//! reactor reports the draining exit id through the `on_exit_draining` callback
//! to the daemon, which records it in a TTL'd avoid-set that the multi-hop
//! directory selection filters out (`warren_multi_hop_directory::valid_circuits`).
//! So the reconnect re-selects a circuit that excludes the drained exit. The
//! state-machine failover is deliberately NOT relied upon: a connected-tunnel
//! close re-enters Connecting with `retry_attempt = 0` (`connected_state.rs`),
//! so `is_failover` never arms, and `assemble_failover_for_attempt` only
//! rewrites the root relay-selection fields and passes the multi-hop circuit
//! through unchanged. The ambient relay-list refresh (backend marks the exit inactive)
//! remains the long-term authority + the backstop when no callback is wired.
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
//!
//! One reactor serves the tunnel's whole life. A gap-free migration keeps the
//! tunnel, so the exit it lands on can drain in turn (a rollout drains every
//! node, one after the other), and that drain must move the session again
//! before its own deadline. The reactor therefore charges each advisory to
//! the exit that sealed it, which the engine names on the notice, skips one
//! from an exit the session already left ([`crate::exit_in_use`]), and acts
//! on each notice once: the exit re-sends its advisory every few seconds
//! until the session has left. It returns only after escalating a rebuild,
//! whose fresh tunnel brings its own reactor.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// The anti-stampede policy (jitter window, cooldown, avoid TTL) is engine
// owned: this reactor keeps only the app plumbing around it.
use warrenguard_transport::drain_policy::{
    DRAIN_RECONNECT_COOLDOWN, jitter_delay, within_cooldown,
};
use warrenguard_transport::supervised_pump::ExitDrainNotice;

use crate::exit_in_use::ExitInUse;
// Pump-error escalation channel, shared with the pumps and the migration
// watchdog: the first task to take the `oneshot` reports the fatal/transient
// cause to the state machine, which rebuilds the tunnel.
use crate::reconnect_signal::PumpErrorTx;

/// Unix seconds of the last drain-triggered escalation, process-wide (`0` =
/// never). Shared across tunnel instances so a rebuilt tunnel's fresh reactor
/// observes a recent drain reconnect and backs off instead of storming.
static LAST_DRAIN_ESCALATION_UNIX: AtomicU64 = AtomicU64::new(0);

/// Notices remembered as acted on, so the re-sends of their advisory are
/// skipped. The latest few cover every exit a session can cross in one
/// rollout; the bound stops an exit that keeps varying its advisory from
/// growing the list.
const ACTED_ON_MEMORY: usize = 8;

/// IO surface consumed by [`run_drain_reactor`]. Implemented for production
/// by [`RealDrainReactorIo`] and by a fake in the unit tests.
pub(crate) trait DrainReactorIo {
    /// Resolve on the next drain notice, or `None` when the channel closes
    /// (tunnel teardown): the loop exits.
    async fn next_notice(&mut self) -> Option<ExitDrainNotice>;
    /// The exit of the session the tunnel carries its traffic over now,
    /// `None` while the supervisor publishes none.
    fn exit_in_use(&mut self) -> Option<[u8; 16]>;
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
    /// rebuild that re-selects the multi-hop circuit).
    fn escalate(&mut self, msg: String);
    /// Report the draining exit to the daemon avoid-set so the move triggered
    /// by [`Self::try_migrate`] or [`Self::escalate`] excludes it and lands on
    /// a DIFFERENT exit (ADR 36 true exit-exclusion). No-op when no callback
    /// is wired (exclusion then relies on the ambient relay-list refresh).
    fn report_drained_exit(&mut self, exit: [u8; 16]);
    /// Attempt a GAP-FREE cross-exit migration off the draining `exit` (ADR
    /// 36): select a non-drained circuit and swap the supervisor onto it via
    /// `SupervisorHandle::migrate_to` (make-before-break, NO tunnel rebuild).
    /// Returns `true` if the migration was initiated (the tunnel stays up, so
    /// the reactor skips [`Self::escalate`]), `false` to fall back to the
    /// break-before-make escalation. Default `false`: with no migration wired,
    /// the reactor escalates. The avoid-set recorded by
    /// [`Self::report_drained_exit`] is what the migration's circuit selection
    /// excludes, so the swap lands on a different exit.
    async fn try_migrate(&mut self, _exit: [u8; 16]) -> bool {
        false
    }
}

/// Consume drain advisories and proactively move the session off each exit
/// that drains while the tunnel is on it (see the module docs).
pub(crate) async fn run_drain_reactor<I: DrainReactorIo>(io: &mut I) {
    let mut acted_on: VecDeque<ExitDrainNotice> = VecDeque::with_capacity(ACTED_ON_MEMORY);
    while let Some(notice) = io.next_notice().await {
        let draining = *notice.exit_id.as_bytes();
        if io.exit_in_use().is_some_and(|in_use| in_use != draining) {
            log::debug!(
                "Warren drain reactor: advisory from an exit this tunnel already left; ignored"
            );
            continue;
        }
        if acted_on.contains(&notice) {
            continue;
        }
        if acted_on.len() == ACTED_ON_MEMORY {
            acted_on.pop_front();
        }
        acted_on.push_back(notice);
        let advisory = notice.advisory;
        // Record the drained exit in the avoid-set UNCONDITIONALLY, before the
        // cooldown gate: even when the cooldown suppresses a second reconnect,
        // the directory selection must still exclude this exit (a different
        // exit that also drains within the cooldown window must not be
        // re-picked).
        io.report_drained_exit(draining);
        let now = io.now_unix();
        if within_cooldown(now, io.last_drain_escalation_unix()) {
            log::info!(
                "Warren drain reactor: a drain reconnect fired <{}s ago; recorded the \
                 exit in the avoid-set but deferring the reconnect to the exit hard-close \
                 backstop (avoids a storm against a still-draining exit)",
                DRAIN_RECONNECT_COOLDOWN.as_secs()
            );
            continue;
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
        // Prefer a GAP-FREE cross-exit migration (ADR 36): the supervisor
        // make-before-breaks onto a non-drained circuit (the avoid-set recorded
        // above excludes the draining exit), so the tunnel never drops. Only if
        // that is not possible (no migration wired, or no non-drained exit
        // available) do we fall back to the break-before-make rebuild.
        if io.try_migrate(draining).await {
            log::info!(
                "Warren drain reactor: gap-free cross-exit migration initiated \
                 (make-before-break); tunnel stays up, no rebuild"
            );
            continue;
        }
        // The drained exit was already recorded in the avoid-set above, so the
        // reconnect this escalation triggers re-selects a circuit that excludes it.
        io.escalate(format!(
            "exit draining (reason={}); proactively reconnecting to a different exit \
             before the maintenance deadline",
            advisory.reason_code
        ));
        return;
    }
}

/// Production [`DrainReactorIo`]: a subscriber on the supervisor's
/// `ExitDrainingChannel` plus the shared pump-error escalation handle.
pub(crate) struct RealDrainReactorIo {
    pub drain_sub: tokio::sync::watch::Receiver<Option<ExitDrainNotice>>,
    pub pump_error_tx: PumpErrorTx,
    /// The exit the tunnel is on, which a gap-free migration changes.
    pub exit_in_use: ExitInUse,
    /// Daemon callback recording the drained exit in the avoid-set.
    pub on_exit_draining: Option<std::sync::Arc<dyn Fn([u8; 16]) + Send + Sync>>,
    /// Daemon hook attempting the gap-free cross-exit migration off the
    /// draining exit (`WarrenTunnelParameters::warren_drain_migrate`).
    /// `None` = not wired: every drain falls back to the rebuild.
    pub drain_migrate: Option<crate::WarrenDrainMigrate>,
}

impl DrainReactorIo for RealDrainReactorIo {
    async fn next_notice(&mut self) -> Option<ExitDrainNotice> {
        loop {
            if self.drain_sub.changed().await.is_err() {
                return None;
            }
            if let Some(notice) = *self.drain_sub.borrow_and_update() {
                return Some(notice);
            }
        }
    }

    fn exit_in_use(&mut self) -> Option<[u8; 16]> {
        (self.exit_in_use)()
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
        warrenguard_transport::drain_policy::stampede_fraction()
    }

    async fn sleep(&mut self, dur: Duration) {
        tokio::time::sleep(dur).await;
    }

    fn escalate(&mut self, msg: String) {
        log::warn!("Warren drain reactor: escalating to the state machine: {msg}");
        crate::reconnect_signal::escalate(&self.pump_error_tx, msg);
    }

    fn report_drained_exit(&mut self, exit: [u8; 16]) {
        if let Some(cb) = self.on_exit_draining.as_ref() {
            cb(exit);
        }
    }

    async fn try_migrate(&mut self, exit: [u8; 16]) -> bool {
        match self.drain_migrate.as_ref() {
            Some(migrate) => migrate(exit).await,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warrenguard_transport::supervised_pump::ExitDrainAdvisory;

    // The pure jitter/cooldown decision tests moved with the policy to
    // `warrenguard_transport::drain_policy`; the reactor tests below still
    // pin the policy end-to-end through `run_drain_reactor`.

    const FAKE_EXIT_ID: [u8; 16] = [0xEE; 16];
    const NEXT_EXIT_ID: [u8; 16] = [0xE1; 16];

    /// `exit` announcing `advisory`.
    fn notice(exit: [u8; 16], advisory: ExitDrainAdvisory) -> ExitDrainNotice {
        ExitDrainNotice {
            exit_id: warrenguard_multihop::ExitId::from_bytes(exit),
            advisory,
        }
    }

    struct FakeIo {
        notices: VecDeque<Option<ExitDrainNotice>>,
        /// Wall clock at each notice's arrival, in order; empty keeps `now`.
        arrivals: VecDeque<u64>,
        now: u64,
        last_escalation: u64,
        fraction: f64,
        slept: Vec<Duration>,
        escalations: Vec<String>,
        reported: Vec<[u8; 16]>,
        /// The exit the tunnel is on.
        exit: Option<[u8; 16]>,
        /// Where each successful migration lands, in order.
        landings: VecDeque<[u8; 16]>,
        /// What `try_migrate` returns (default false = no gap-free migration).
        migrate_succeeds: bool,
        /// The exit each `try_migrate` was asked to leave.
        migrated_off: Vec<[u8; 16]>,
    }

    impl DrainReactorIo for FakeIo {
        async fn next_notice(&mut self) -> Option<ExitDrainNotice> {
            if let Some(at) = self.arrivals.pop_front() {
                self.now = at;
            }
            self.notices.pop_front().flatten()
        }
        fn exit_in_use(&mut self) -> Option<[u8; 16]> {
            self.exit
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
        fn report_drained_exit(&mut self, exit: [u8; 16]) {
            self.reported.push(exit);
        }
        async fn try_migrate(&mut self, exit: [u8; 16]) -> bool {
            self.migrated_off.push(exit);
            if self.migrate_succeeds
                && let Some(landing) = self.landings.pop_front()
            {
                self.exit = Some(landing);
            }
            self.migrate_succeeds
        }
    }

    /// A tunnel on `FAKE_EXIT_ID` that receives `advisory` from it.
    fn fake_with(advisory: Option<ExitDrainAdvisory>, now: u64, last_escalation: u64) -> FakeIo {
        FakeIo {
            notices: VecDeque::from([advisory.map(|a| notice(FAKE_EXIT_ID, a))]),
            arrivals: VecDeque::new(),
            now,
            last_escalation,
            fraction: 0.5,
            slept: Vec::new(),
            escalations: Vec::new(),
            reported: Vec::new(),
            exit: Some(FAKE_EXIT_ID),
            landings: VecDeque::from([NEXT_EXIT_ID]),
            migrate_succeeds: false,
            migrated_off: Vec::new(),
        }
    }

    const FIRST_DRAIN: ExitDrainAdvisory = ExitDrainAdvisory {
        deadline_unix_secs: 1_000,
        reason_code: 0,
    };
    const SECOND_DRAIN: ExitDrainAdvisory = ExitDrainAdvisory {
        deadline_unix_secs: 1_600,
        reason_code: 0,
    };

    #[tokio::test]
    async fn a_drain_on_the_exit_migrated_to_moves_the_session_again() {
        // A rollout drains its nodes one after the other: the exit a gap-free
        // migration landed on drains minutes later, on the same tunnel.
        let mut io = fake_with(Some(FIRST_DRAIN), 940, 0);
        io.notices
            .push_back(Some(notice(NEXT_EXIT_ID, SECOND_DRAIN)));
        io.arrivals = VecDeque::from([940, 1_540]);
        io.migrate_succeeds = true;

        run_drain_reactor(&mut io).await;

        assert_eq!(
            io.migrated_off,
            vec![FAKE_EXIT_ID, NEXT_EXIT_ID],
            "each drain must move the session off the exit it is on"
        );
        assert_eq!(
            io.reported,
            vec![FAKE_EXIT_ID, NEXT_EXIT_ID],
            "each drained exit goes to the avoid-set, never the one already left"
        );
        assert!(io.escalations.is_empty(), "both moves were gap-free");
    }

    #[tokio::test]
    async fn two_exits_announcing_the_same_drain_are_each_left() {
        // A soft drain carries no deadline of its own, so two exits drained
        // that way send identical advisories: only the exit tells them apart.
        let soft = ExitDrainAdvisory {
            deadline_unix_secs: u64::MAX,
            reason_code: 0,
        };
        let mut io = fake_with(Some(soft), 940, 0);
        io.notices.push_back(Some(notice(NEXT_EXIT_ID, soft)));
        io.arrivals = VecDeque::from([940, 1_540]);
        io.migrate_succeeds = true;

        run_drain_reactor(&mut io).await;

        assert_eq!(io.migrated_off, vec![FAKE_EXIT_ID, NEXT_EXIT_ID]);
    }

    #[tokio::test]
    async fn an_advisory_from_an_exit_the_session_already_left_changes_nothing() {
        // The old exit re-sends its advisory until its session closes, and a
        // re-send can be decoded after the migration swapped the session.
        let mut io = fake_with(None, 940, 0);
        io.notices = VecDeque::from([Some(notice(FAKE_EXIT_ID, FIRST_DRAIN))]);
        io.exit = Some(NEXT_EXIT_ID);
        io.migrate_succeeds = true;

        run_drain_reactor(&mut io).await;

        assert!(io.reported.is_empty(), "the exit in use is not draining");
        assert!(io.migrated_off.is_empty());
    }

    #[tokio::test]
    async fn a_drain_deferred_by_the_cooldown_is_followed_by_the_next_one() {
        // The first advisory lands inside the cooldown of an earlier drain
        // reconnect; the tunnel keeps listening, so the next drain still
        // moves it before that exit's deadline.
        let mut io = fake_with(Some(FIRST_DRAIN), 940, 900);
        io.notices
            .push_back(Some(notice(FAKE_EXIT_ID, SECOND_DRAIN)));
        io.arrivals = VecDeque::from([940, 1_540]);
        io.migrate_succeeds = true;

        run_drain_reactor(&mut io).await;

        assert_eq!(
            io.migrated_off,
            vec![FAKE_EXIT_ID],
            "the drain after the cooldown moves"
        );
    }

    #[tokio::test]
    async fn an_advisory_the_exit_sends_again_is_acted_on_once() {
        // The exit re-sends its advisory every few seconds until the session
        // leaves; each re-send wakes the reactor. This one lands while the
        // migration's overlap dial is still in flight, the session still on
        // the draining exit.
        let mut io = fake_with(Some(FIRST_DRAIN), 940, 0);
        io.notices
            .push_back(Some(notice(FAKE_EXIT_ID, FIRST_DRAIN)));
        io.arrivals = VecDeque::from([940, 943]);
        io.migrate_succeeds = true;
        io.landings.clear();

        run_drain_reactor(&mut io).await;

        assert_eq!(io.migrated_off.len(), 1, "one move per advisory");
        assert_eq!(io.reported.len(), 1, "one avoid-set report per advisory");
        assert_eq!(io.slept.len(), 1, "one jitter per advisory");
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
        // Anything queued after the escalation belongs to the rebuilt tunnel.
        io.notices
            .push_back(Some(notice(FAKE_EXIT_ID, SECOND_DRAIN)));

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
        assert_eq!(
            io.reported,
            vec![FAKE_EXIT_ID],
            "the drained exit must be reported to the avoid-set so the reconnect excludes it"
        );
        assert_eq!(
            io.migrated_off,
            vec![FAKE_EXIT_ID],
            "the reactor must attempt a gap-free migration before escalating"
        );
        assert_eq!(
            io.notices.len(),
            1,
            "the escalated rebuild ends this reactor"
        );
    }

    #[tokio::test]
    async fn gap_free_migration_skips_escalation() {
        // When a gap-free cross-exit migration succeeds, the reactor must NOT
        // escalate (no break-before-make tunnel rebuild) - the tunnel stays up.
        let mut io = fake_with(
            Some(ExitDrainAdvisory {
                deadline_unix_secs: 1_000,
                reason_code: 7,
            }),
            940,
            0,
        );
        io.migrate_succeeds = true;

        run_drain_reactor(&mut io).await;

        assert_eq!(
            io.migrated_off,
            vec![FAKE_EXIT_ID],
            "a gap-free migration must be attempted on drain"
        );
        assert!(
            io.escalations.is_empty(),
            "a successful gap-free migration must NOT escalate (no tunnel rebuild): {:?}",
            io.escalations
        );
        // The avoid-set is still recorded (the migration excludes the drained
        // exit) and the anti-stampede jitter still applies.
        assert_eq!(
            io.reported,
            vec![FAKE_EXIT_ID],
            "the drained exit must still be recorded in the avoid-set"
        );
        assert_eq!(
            io.slept,
            vec![Duration::from_secs(10)],
            "the anti-stampede jitter must still apply before migrating"
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
        assert_eq!(
            io.reported,
            vec![FAKE_EXIT_ID],
            "a cooldown-suppressed reactor must STILL record the exit in the avoid-set \
             so the directory excludes it (only the reconnect is suppressed)"
        );
    }

    /// Production IO with `drain_migrate` as the daemon hook, an inert drain
    /// channel and an empty escalation slot (irrelevant to `try_migrate`).
    fn real_io(drain_migrate: Option<crate::WarrenDrainMigrate>) -> RealDrainReactorIo {
        RealDrainReactorIo {
            drain_sub: tokio::sync::watch::channel(None).1,
            pump_error_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
            exit_in_use: std::sync::Arc::new(|| Some(FAKE_EXIT_ID)),
            on_exit_draining: None,
            drain_migrate,
        }
    }

    #[tokio::test]
    async fn real_io_without_daemon_hook_never_migrates() {
        // No daemon hook wired (a daemon predating the gap-free path):
        // try_migrate must report failure so the reactor
        // escalates the break-before-make rebuild, exactly as before.
        assert!(
            !real_io(None).try_migrate(FAKE_EXIT_ID).await,
            "with no daemon hook the real IO must fall back to the rebuild path"
        );
    }

    #[tokio::test]
    async fn real_io_hands_the_draining_exit_to_the_daemon_and_propagates_the_outcome() {
        // The id handed to the daemon hook must be THE draining exit: the
        // daemon records it in the avoid-set before selecting the migration
        // target, which is what guarantees the target is never that exit.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        for outcome in [true, false] {
            let seen_in_hook = seen.clone();
            let hook: crate::WarrenDrainMigrate = std::sync::Arc::new(move |exit_id| {
                let seen = seen_in_hook.clone();
                Box::pin(async move {
                    seen.lock().unwrap().push(exit_id);
                    outcome
                })
            });
            assert_eq!(
                real_io(Some(hook)).try_migrate(NEXT_EXIT_ID).await,
                outcome,
                "the daemon's migration outcome must reach the reactor unchanged"
            );
        }
        assert_eq!(
            *seen.lock().unwrap(),
            vec![NEXT_EXIT_ID; 2],
            "the hook must always receive the draining exit id"
        );
    }

    #[tokio::test]
    async fn a_drain_handled_between_two_sessions_is_charged_to_the_exit_that_sealed_it() {
        // The session that decoded the advisory closed before the reactor
        // handled it, so no session names the exit in use: the notice does.
        let (notices, drain_sub) = tokio::sync::watch::channel(None);
        let (escalation, _escalated) = tokio::sync::oneshot::channel();
        let reported: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        let migrated_off: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        let report = reported.clone();
        let migrate = migrated_off.clone();
        let mut io = RealDrainReactorIo {
            drain_sub,
            pump_error_tx: std::sync::Arc::new(std::sync::Mutex::new(Some(escalation))),
            exit_in_use: std::sync::Arc::new(|| None),
            on_exit_draining: Some(std::sync::Arc::new(move |exit| {
                report.lock().unwrap().push(exit);
            })),
            drain_migrate: Some(std::sync::Arc::new(move |exit| {
                let migrate = migrate.clone();
                Box::pin(async move {
                    migrate.lock().unwrap().push(exit);
                    false
                })
            })),
        };
        // A deadline already inside the safety margin: react without jitter.
        notices.send_replace(Some(notice(
            NEXT_EXIT_ID,
            ExitDrainAdvisory {
                deadline_unix_secs: io.now_unix(),
                reason_code: 0,
            },
        )));
        drop(notices);

        run_drain_reactor(&mut io).await;

        assert_eq!(*reported.lock().unwrap(), vec![NEXT_EXIT_ID]);
        assert_eq!(*migrated_off.lock().unwrap(), vec![NEXT_EXIT_ID]);
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
