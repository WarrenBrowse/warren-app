//! Android bindings for the engine's in-tunnel egress liveness probe.
//!
//! Every guard the Android datapath already runs looks at the CLIENT half of
//! the circuit: the supervisor's RX-silence watch sees QUIC keep-alives, and
//! the goodput prober echoes paired probes off the tunnel gateway. An exit that
//! answers both while forwarding NOTHING to the internet satisfies all of them,
//! so the app kept rendering "You are protected" over a tunnel with no egress.
//! Android was the last Warren platform with nothing that could see that class.
//!
//! The scheduler, the cadence, the debounce and the DNS probe are the engine's
//! [`warrenguard_transport::egress_probe`]; this module supplies only the
//! Android [`EgressProbeIo`] around them, mirroring the desktop daemon's
//! `RealEgressProbeIo`. The probe queries the exit resolver
//! ([`warrenguard_config::TUNNEL_GATEWAY_IP`]`:53`, the same server the
//! `VpnService` hands to the system) through the TUN, so an answer proves the
//! exit decapsulates, forwards and reaches its upstream, and the target is
//! routable only inside the tunnel, so the probe can never leak.
//!
//! Everything except the datapath probe itself is portable, so the bindings are
//! host-tested against the real engine scheduler with only the network (the one
//! system boundary here) scripted.

#![cfg(any(test, all(target_os = "android", feature = "tunnel")))]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;
use warrenguard_transport::egress_probe::{
    EgressProbeIo, ExitEvidence, ProbeOutcome, TransportEvidence, exit_evidence_from, jittered,
    probe_gateway_dns, transport_evidence_from,
};
use warrenguard_transport::supervisor::ClientWatch;

/// Verdict value published when the in-tunnel probe declares the exit dead.
///
/// Deliberately DISTINCT from the goodput prober's "both sizes dead" rather
/// than reusing it: the two verdicts answer different questions (nothing
/// crosses the client to exit datapath, versus the exit forwards nothing to the
/// internet) and are produced by independent probes, so a support log that
/// cannot tell them apart cannot tell a client-side wedge from a broken exit.
/// The UI maps both onto the same "connection interrupted" state, so the
/// distinction costs no new user-facing copy.
pub(crate) const PATH_HEALTH_EGRESS_DEAD: i32 = 3;

/// Verdict sink. Production binds it to the JNI cell Kotlin polls through
/// `getPathHealth`; a host test observes it directly.
pub(crate) type VerdictSink = Arc<dyn Fn(bool) + Send + Sync>;

/// Datapath probe override. `None` runs the engine's in-tunnel DNS probe; a
/// host test scripts it, because the network is the only system boundary this
/// module owns.
pub(crate) type ProbeFn =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ProbeOutcome> + Send>> + Send + Sync>;

/// Reads the live session's QUIC ACK counter. `None` runs the production read
/// off the session watch; a host test scripts it, because a real
/// `MultiHopBundle` cannot be built without a network.
pub(crate) type AckFn = Arc<dyn Fn() -> Option<u64> + Send + Sync>;

/// Compose the single `i32` Kotlin polls out of the two independent verdict
/// publishers (the supervisor's goodput prober and this probe).
///
/// They own separate cells on purpose: a single shared one would let the
/// goodput publisher's next transition erase a live egress-dead verdict, and
/// the two run on unrelated cadences. Egress-dead wins because it is the more
/// specific finding, and both map to the same UI state anyway.
pub(crate) fn compose_path_health(path_health: i32, egress_dead: bool) -> i32 {
    if egress_dead {
        PATH_HEALTH_EGRESS_DEAD
    } else {
        path_health
    }
}

/// Per-tick jitter source, keeping a fleet of clients from probing in lockstep.
///
/// The desktop binding draws it from `rand`, which this crate pulls in only on
/// the Android target, so a host-tested module cannot reach it. The sub-second
/// wall clock is decorrelated enough for a spread whose only job is to smear a
/// +/-15% window.
fn jitter_fraction() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    f64::from(nanos) / 1e9
}

/// Android bindings for [`EgressProbeIo`].
pub(crate) struct AndroidEgressProbeIo {
    /// Steady-state cadence, used once a probe has proven the circuit forwards.
    pub interval: Duration,
    /// Faster cadence used while the circuit is unproven OR a failure is
    /// pending, so neither a circuit dead from connect nor a suspicion raised
    /// mid-session waits a full steady interval to be settled.
    pub startup_interval: Duration,
    /// Supervisor session watch. `None` (tests) reads as "session present".
    pub sessions: Option<ClientWatch>,
    /// Where the verdict goes. `None` disables publication.
    pub verdict: Option<VerdictSink>,
    /// Escalation channel: the session driver ends the session on `true`
    /// (`SupervisedInputs::egress_dead`), which is how Android leaves
    /// Connected and hands back to the Kotlin fail-closed policy.
    pub escalate: Option<watch::Sender<bool>>,
    /// Datapath probe; see [`ProbeFn`].
    pub probe: Option<ProbeFn>,
    /// ACK-counter read; see [`AckFn`].
    pub acks: Option<AckFn>,
    /// The counter's value when the current failure streak began, so the
    /// evidence answers about THIS streak and not about the whole session.
    pub acks_at_streak_start: Option<u64>,
    /// REAL downlink packet count, the counter only the exit can move. `None`
    /// reads the live bundle off `sessions`; a test scripts it.
    pub real_rx: Option<AckFn>,
    /// Its value when the current failure streak began.
    pub real_rx_at_streak_start: Option<u64>,
}

impl AndroidEgressProbeIo {
    /// ACK frames the peer has sent us on the live session, or `None` when
    /// there is no session to read.
    /// Decoded IP packets the exit forwarded to us, summed across every bonded
    /// leg. Deliberately NOT a quinn frame counter: an armed exit pads its
    /// downlink with dummies, so a frame counter advances over a tunnel
    /// carrying no user traffic at all.
    fn read_real_rx(&mut self) -> Option<u64> {
        if let Some(scripted) = self.real_rx.as_ref() {
            return scripted();
        }
        self.sessions
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.real_traffic_totals().1)
    }

    /// The path round trip the next probe will run on, so its schedule follows
    /// a link whose queueing delay changes under load.
    fn read_path_rtt(&mut self) -> Option<std::time::Duration> {
        self.sessions
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.quinn_stats().path.rtt)
    }

    fn read_acks(&mut self) -> Option<u64> {
        if let Some(scripted) = self.acks.as_ref() {
            return scripted();
        }
        self.sessions
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.quinn_stats().frame_rx.acks)
    }
}

impl EgressProbeIo for AndroidEgressProbeIo {
    async fn next_tick(&mut self, settled: bool) -> bool {
        let interval = if settled {
            self.interval
        } else {
            self.startup_interval
        };
        tokio::time::sleep(jittered(interval, jitter_fraction())).await;
        true
    }

    fn session_present(&mut self) -> bool {
        match self.sessions.as_mut() {
            Some(rx) => rx.borrow_and_update().is_some(),
            None => true,
        }
    }

    async fn probe(&mut self) -> ProbeOutcome {
        match self.probe.as_ref() {
            Some(scripted) => scripted().await,
            None => probe_gateway_dns(self.read_path_rtt()).await,
        }
    }

    fn publish(&mut self, egress_dead: bool) {
        if egress_dead {
            log::warn!(
                "egress probe: exit not forwarding (in-tunnel DNS probe dead while the \
                 QUIC session is alive); the tunnel is no longer carrying traffic"
            );
        } else {
            log::info!("egress probe: egress recovered");
        }
        if let Some(sink) = self.verdict.as_ref() {
            sink(egress_dead);
        }
    }

    /// Android subscribes no [`ExitDrainChannel`]: the supervised pumps here are
    /// built without one, so no advisory ever arrives and the gap-free
    /// migration branch must stay unreachable. Claiming a drain would swallow
    /// the escalation and leave the user sitting on a dead exit.
    ///
    /// [`ExitDrainChannel`]: warrenguard_transport::supervised_pump::ExitDrainChannel
    fn drain_active(&mut self) -> bool {
        false
    }

    async fn try_migrate(&mut self) -> bool {
        false
    }

    fn mark_streak_start(&mut self) {
        self.acks_at_streak_start = self.read_acks();
        self.real_rx_at_streak_start = self.read_real_rx();
    }

    /// Only the peer can acknowledge what it received from us, so a counter
    /// that has not moved across the failure streak means the path carried
    /// nothing and the exit is not the suspect. `Unknown` when there is no
    /// session to read: absent evidence must not suppress a conviction.
    fn transport_evidence(&mut self) -> TransportEvidence {
        transport_evidence_from(self.acks_at_streak_start, self.read_acks())
    }

    /// An exit still delivering decoded IP packets is forwarding, whatever our
    /// own query did. On a link whose uplink is saturated the query queues away
    /// while the exit keeps delivering, and convicting there costs a fresh QUIC
    /// epoch and every request in flight with it.
    fn exit_evidence(&mut self) -> ExitEvidence {
        exit_evidence_from(self.real_rx_at_streak_start, self.read_real_rx())
    }

    fn escalate_reconnect(&mut self, msg: String) {
        match self.escalate.as_ref() {
            // A closed receiver means the session is already tearing down, so
            // the escalation has nothing left to do.
            Some(tx) => {
                log::warn!("egress probe: escalating reconnect: {msg}");
                let _ = tx.send(true);
            }
            None => log::warn!(
                "egress probe: exit not forwarding but no escalation channel wired; \
                 verdict published only: {msg}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use warrenguard_transport::egress_probe::run_egress_probe;

    use super::*;

    /// Cadences small enough that a paused-clock test walks many ticks fast,
    /// while staying inside the engine's accepted ranges.
    const TEST_INTERVAL: Duration = Duration::from_secs(25);
    const TEST_STARTUP: Duration = Duration::from_secs(3);

    /// Records what the bindings published, standing in for the JNI cell.
    #[derive(Default)]
    struct Sink(Mutex<Vec<bool>>);

    impl Sink {
        fn seen(&self) -> Vec<bool> {
            self.0.lock().expect("sink never poisoned").clone()
        }
    }

    /// Build the real bindings with the network scripted by `results`, cycling
    /// on its last entry once exhausted so the loop can run indefinitely.
    fn io_with(
        results: Vec<bool>,
        sink: &Arc<Sink>,
        escalate: watch::Sender<bool>,
    ) -> AndroidEgressProbeIo {
        let calls = Arc::new(AtomicUsize::new(0));
        let sink_cb = Arc::clone(sink);
        AndroidEgressProbeIo {
            interval: TEST_INTERVAL,
            startup_interval: TEST_STARTUP,
            sessions: None,
            verdict: Some(Arc::new(move |dead| {
                sink_cb.0.lock().expect("sink never poisoned").push(dead);
            })),
            escalate: Some(escalate),
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
            probe: Some(Arc::new(move || {
                let n = calls.fetch_add(1, Ordering::Relaxed);
                let ok = *results
                    .get(n)
                    .unwrap_or_else(|| results.last().expect("non-empty script"));
                let outcome = if ok {
                    ProbeOutcome::Alive
                } else {
                    ProbeOutcome::Dead
                };
                Box::pin(async move { outcome })
            })),
        }
    }

    /// A path that carries nothing must never be blamed on the exit. This probe
    /// convicts an exit that ACKs keep-alives and forwards nothing, so while the
    /// peer ACKs nothing at all the premise does not hold: the redial the
    /// conviction triggers rides the same dead path, and the QUIC idle timeout
    /// owns that death. Convicting anyway costs the user every request in
    /// flight, which is the 2026-08-08 incident.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_path_never_convicts_the_exit() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let mut io = io_with(vec![false], &sink, escalate);
        // The peer acknowledges nothing for the whole run: a path stall.
        io.acks = Some(Arc::new(|| Some(42)));
        let _ = tokio::time::timeout(Duration::from_secs(600), run_egress_probe(&mut io, 3)).await;
        assert!(
            !*escalated.borrow(),
            "a stalled path must never end the session"
        );
        assert!(
            sink.seen().is_empty(),
            "and must not even banner: {:?}",
            sink.seen()
        );
    }

    /// The other side of the same coin: when the path IS carrying traffic and
    /// the exit still answers nothing, that is evidence against the exit and it
    /// must still be convicted.
    #[tokio::test(start_paused = true)]
    async fn an_exit_that_forwards_nothing_behind_a_live_path_is_still_convicted() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let mut io = io_with(vec![false], &sink, escalate);
        let acks = Arc::new(std::sync::atomic::AtomicU64::new(0));
        io.acks = Some(Arc::new(move || {
            Some(acks.fetch_add(10, Ordering::Relaxed))
        }));
        let _ = tokio::time::timeout(Duration::from_secs(600), run_egress_probe(&mut io, 3)).await;
        assert!(
            *escalated.borrow(),
            "an exit forwarding nothing over a live path must still be convicted"
        );
        assert_eq!(sink.seen(), vec![true]);
    }

    /// A forwarding exit must never raise the banner, however long the probe
    /// runs: a false verdict tears down a healthy tunnel.
    #[tokio::test(start_paused = true)]
    async fn a_forwarding_exit_never_publishes_a_verdict() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let mut io = io_with(vec![true], &sink, escalate);
        // Bounded because a healthy probe loop never returns on its own.
        let _ = tokio::time::timeout(Duration::from_secs(3600), run_egress_probe(&mut io, 3)).await;
        assert!(
            sink.seen().is_empty(),
            "a forwarding exit must publish nothing: {:?}",
            sink.seen()
        );
        assert!(!*escalated.borrow(), "and must never end the session");
    }

    /// The debounce is the whole point: a rollout hot-swap blip must not flap
    /// the UI, so failures that are broken by a success never accumulate.
    #[tokio::test(start_paused = true)]
    async fn failures_broken_by_a_success_never_reach_the_threshold() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let mut io = io_with(
            vec![false, false, true, false, false, true],
            &sink,
            escalate,
        );
        let _ = tokio::time::timeout(Duration::from_secs(3600), run_egress_probe(&mut io, 3)).await;
        assert!(
            sink.seen().is_empty(),
            "non-consecutive failures must not publish: {:?}",
            sink.seen()
        );
        assert!(!*escalated.borrow(), "and must never end the session");
    }

    /// The gap this closes: an exit whose QUIC session is alive while it
    /// forwards nothing must publish exactly one dead verdict and end the
    /// session, so the card stops claiming protection and Kotlin redials.
    #[tokio::test(start_paused = true)]
    async fn consecutive_failures_publish_the_verdict_and_end_the_session() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let mut io = io_with(vec![false], &sink, escalate);
        tokio::time::timeout(Duration::from_secs(3600), run_egress_probe(&mut io, 3))
            .await
            .expect("an escalating probe returns on its own");
        assert_eq!(
            sink.seen(),
            vec![true],
            "the threshold must publish exactly one dead verdict"
        );
        assert!(
            *escalated.borrow(),
            "a dead exit with no gap-free migration must end the session"
        );
    }

    /// The same failure the desktop tunnel carries, and the reason this guard
    /// exists on both: a saturated uplink queues our own query away while the
    /// exit keeps forwarding, so judging on the query alone tears down a
    /// working tunnel and kills every request in flight.
    #[tokio::test(start_paused = true)]
    async fn an_exit_still_delivering_is_never_convicted() {
        let sink = Arc::new(Sink::default());
        let (escalate, escalated) = watch::channel(false);
        let delivered = Arc::new(AtomicUsize::new(1_000));
        let read = Arc::clone(&delivered);
        // Every probe fails, and the exit delivers more payload on each one.
        let mut io = io_with(vec![false], &sink, escalate);
        io.real_rx = Some(Arc::new(move || {
            Some(read.fetch_add(1_500, Ordering::Relaxed) as u64)
        }));

        tokio::time::timeout(Duration::from_secs(600), run_egress_probe(&mut io, 3))
            .await
            .expect_err("a probe that never convicts must keep running");

        assert!(
            sink.seen().is_empty(),
            "an exit delivering payload throughout must never be published dead"
        );
        assert!(
            !*escalated.borrow(),
            "and the session must never be ended over it"
        );
    }

    /// A redial window must not conflate two exits: the failover may land
    /// elsewhere, so the count restarts and the threshold is never reached.
    #[tokio::test(start_paused = true)]
    async fn a_redial_window_is_never_probed() {
        let sink = Arc::new(Sink::default());
        let (escalate, _escalated) = watch::channel(false);
        let (_sessions_tx, sessions) = watch::channel(None);
        let mut io = io_with(vec![false], &sink, escalate);
        io.sessions = Some(sessions);
        let _ = tokio::time::timeout(Duration::from_secs(3600), run_egress_probe(&mut io, 1)).await;
        assert!(
            sink.seen().is_empty(),
            "no published session means no probe and no verdict: {:?}",
            sink.seen()
        );
    }

    /// Recovery clears the banner: the sink mirrors the verdict in both
    /// directions, so a cleared verdict reaches Kotlin as a cleared one.
    #[test]
    fn a_cleared_verdict_reaches_the_sink() {
        let sink = Arc::new(Sink::default());
        let sink_cb = Arc::clone(&sink);
        let (escalate, _escalated) = watch::channel(false);
        let mut io = AndroidEgressProbeIo {
            interval: TEST_INTERVAL,
            startup_interval: TEST_STARTUP,
            sessions: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
            verdict: Some(Arc::new(move |dead| {
                sink_cb.0.lock().expect("sink never poisoned").push(dead);
            })),
            escalate: Some(escalate),
            probe: None,
        };
        io.publish(true);
        io.publish(false);
        assert_eq!(sink.seen(), vec![true, false]);
    }

    /// Android subscribes no exit-drain advisory (the supervised pumps are
    /// built without a drain channel), so the gap-free migration branch must
    /// never be taken: claiming a drain would swallow the escalation and leave
    /// the user on a dead exit.
    #[tokio::test]
    async fn android_never_claims_a_gap_free_migration() {
        let (escalate, _escalated) = watch::channel(false);
        let mut io = AndroidEgressProbeIo {
            interval: TEST_INTERVAL,
            startup_interval: TEST_STARTUP,
            sessions: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
            verdict: None,
            escalate: Some(escalate),
            probe: None,
        };
        assert!(!io.drain_active());
        assert!(!io.try_migrate().await);
    }

    /// The escalation must be inert without a channel (and must not panic):
    /// the verdict still bannered, nothing torn down.
    #[test]
    fn an_unwired_escalation_is_inert() {
        let mut io = AndroidEgressProbeIo {
            interval: TEST_INTERVAL,
            startup_interval: TEST_STARTUP,
            sessions: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
            verdict: None,
            escalate: None,
            probe: None,
        };
        io.publish(true);
        io.escalate_reconnect("no channel".to_owned());
    }

    /// The two verdict publishers own separate cells, so composing them must
    /// surface the egress verdict whatever the goodput prober currently reads,
    /// and must not invent one when egress is fine.
    #[test]
    fn the_egress_verdict_outranks_the_goodput_reading() {
        assert_eq!(compose_path_health(0, false), 0);
        assert_eq!(compose_path_health(1, false), 1);
        assert_eq!(compose_path_health(0, true), PATH_HEALTH_EGRESS_DEAD);
        assert_eq!(compose_path_health(2, true), PATH_HEALTH_EGRESS_DEAD);
    }

    /// The jitter must stay inside the engine's window: outside it the cadence
    /// silently changes.
    #[test]
    fn the_jitter_fraction_stays_in_range() {
        for _ in 0..64 {
            let f = jitter_fraction();
            assert!((0.0..1.0).contains(&f), "jitter fraction out of range: {f}");
        }
    }
}
