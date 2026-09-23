//! In-tunnel egress liveness probe (doc 62 item 5).
//!
//! RX-silence detection (`session_liveness`, the supervisor's dead-path
//! watch) only sees the QUIC transport: an exit that is drained or
//! half-swapped during a fleet rollout keeps ACKing keep-alives, so the
//! session never looks dead while the exit forwards NOTHING and the UI
//! shows "Connected" with zero actual internet. This probe closes that
//! gap by exercising the datapath
//! end to end: a periodic DNS query THROUGH the tunnel to the
//! exit-provided resolver (the tunnel gateway, the same server the
//! system DNS uses while connected). Any answer proves the exit
//! decapsulates, forwards and can reach its upstream; the firewall's
//! connected policy explicitly allows port 53 to the configured
//! in-tunnel resolver on every platform, and the gateway address is
//! only routable via the TUN, so the probe can never leak outside the
//! tunnel.
//!
//! Escalation is debounced: [`EgressProbeConfig::failure_threshold`]
//! consecutive failures publish an "egress dead" verdict through the
//! daemon callback (surfaced as `exit_egress_dead` in the status
//! cache); one success clears it. A rollout hot-swap blip (~1-2 s)
//! never reaches the threshold. While the supervisor has no published
//! session the probe is skipped entirely (the RX-silence machinery owns
//! that case) and the failure count resets.
//!
//! When the verdict fires while the exit the session is on has announced a
//! drain, the probe prefers the drain reactor's gap-free migration hook
//! (`warren_drain_migrate`). Otherwise it escalates a full reconnect
//! through the shared pump-error channel (the same one `session_liveness`
//! uses): a live QUIC session over an exit that forwards nothing is a dead
//! path the RX-silence guards cannot see, so banner-and-wait would leave
//! the user offline until the exit self-healed. The faster cadence runs
//! while the circuit is unproven or a failure is pending, so a circuit dead
//! from connect is convicted in ~11s, and a suspicion raised mid-session is
//! confirmed or cleared without waiting two steady intervals.
//!
//! The probe machinery (knobs, cadence, DNS probe, verdict scheduler) is the
//! engine home `warrenguard_transport::egress_probe`; this module keeps only
//! the daemon bindings around it.

use std::time::Duration;

pub(crate) use warrenguard_transport::egress_probe::{
    EgressProbeConfig, EgressProbeIo, ExitEvidence, ProbeOutcome, TransportEvidence,
    exit_evidence_from, jittered, probe_gateway_dns, run_egress_probe, transport_evidence_from,
};

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<
    Option<std::sync::Arc<warrenguard_transport::bundle::MultiHopBundle>>,
>;
type DrainWatch =
    tokio::sync::watch::Receiver<Option<warrenguard_transport::supervised_pump::ExitDrainNotice>>;

use crate::exit_in_use::ExitInUse;
use crate::reconnect_signal::PumpErrorTx;

/// Production bindings for [`EgressProbeIo`].
pub(crate) struct RealEgressProbeIo {
    pub interval: Duration,
    /// Faster cadence used while the circuit is unproven or a failure is
    /// pending.
    pub startup_interval: Duration,
    /// Supervisor session watch read by `session_present`. `Some` in
    /// production; `None` only in tests, where `session_present` defaults
    /// to true.
    pub client_rx: Option<ClientWatch>,
    /// Daemon verdict callback (`WarrenTunnelParameters::on_egress_verdict`).
    pub verdict: Option<std::sync::Arc<dyn Fn(bool) + Send + Sync>>,
    /// Exit-drain advisory watch. `Some` in production; `None` only in
    /// tests, where `drain_active` is false.
    pub drain_rx: Option<DrainWatch>,
    /// Gap-free migration hook.
    pub drain_migrate: Option<crate::WarrenDrainMigrate>,
    /// Shared reconnect channel. `None` only disables the escalation
    /// (verdict banner still fires), used by tests and any caller that
    /// opts out.
    pub pump_error_tx: Option<PumpErrorTx>,
    /// The exit the tunnel is on, which an earlier gap-free migration may have
    /// changed.
    pub exit_in_use: ExitInUse,
    /// The exit whose drain [`EgressProbeIo::drain_active`] last matched, the
    /// one a migration moves off. Taken from the sealed notice, so a session
    /// swapped between that check and the move never gets a healthy exit
    /// charged with a drain it did not announce.
    pub draining_exit: Option<[u8; 16]>,
    /// ACK-counter read. `None` reads the live session off `client_rx`; a test
    /// scripts it, because a real `MultiHopBundle` needs a network.
    pub acks: Option<std::sync::Arc<dyn Fn() -> Option<u64> + Send + Sync>>,
    /// The counter's value when the current failure streak began, so the
    /// evidence answers about THIS streak and not about the whole session.
    pub acks_at_streak_start: Option<u64>,
    /// REAL downlink packet count, the counter only the exit can move. `None`
    /// reads the live bundle off `client_rx`; a test scripts it.
    pub real_rx: Option<std::sync::Arc<dyn Fn() -> Option<u64> + Send + Sync>>,
    /// Its value when the current failure streak began.
    pub real_rx_at_streak_start: Option<u64>,
}

impl RealEgressProbeIo {
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
        self.client_rx
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.real_traffic_totals().1)
    }

    /// The path round trip the next probe will run on, so its schedule follows
    /// a link whose queueing delay changes under load.
    fn read_path_rtt(&mut self) -> Option<Duration> {
        self.client_rx
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.quinn_stats().path.rtt)
    }

    fn read_acks(&mut self) -> Option<u64> {
        if let Some(scripted) = self.acks.as_ref() {
            return scripted();
        }
        self.client_rx
            .as_mut()?
            .borrow_and_update()
            .as_ref()
            .map(|bundle| bundle.quinn_stats().frame_rx.acks)
    }
}

impl EgressProbeIo for RealEgressProbeIo {
    async fn next_tick(&mut self, settled: bool) -> bool {
        let interval = if settled {
            self.interval
        } else {
            self.startup_interval
        };
        tokio::time::sleep(jittered(interval, rand::random::<f64>())).await;
        true
    }

    fn session_present(&mut self) -> bool {
        match self.client_rx.as_mut() {
            Some(rx) => rx.borrow_and_update().is_some(),
            None => true,
        }
    }

    async fn probe(&mut self) -> ProbeOutcome {
        probe_gateway_dns(self.read_path_rtt()).await
    }

    fn publish(&mut self, egress_dead: bool) {
        if egress_dead {
            log::warn!(
                "Warren egress probe: exit not forwarding (in-tunnel DNS probe dead \
                 while the QUIC session is alive); surfacing exit_egress_dead"
            );
        } else {
            log::info!("Warren egress probe: egress recovered; clearing exit_egress_dead");
        }
        if let Some(cb) = self.verdict.as_ref() {
            cb(egress_dead);
        }
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

    /// The exit the session is on announced a drain. An advisory from an exit
    /// the session already left says nothing about the one it is on.
    fn drain_active(&mut self) -> bool {
        let latest = self
            .drain_rx
            .as_mut()
            .and_then(|rx| *rx.borrow_and_update());
        let in_use = (self.exit_in_use)();
        self.draining_exit = latest
            .map(|notice| *notice.exit_id.as_bytes())
            .filter(|sealed_by| in_use == Some(*sealed_by));
        self.draining_exit.is_some()
    }

    async fn try_migrate(&mut self) -> bool {
        match (self.drain_migrate.as_ref(), self.draining_exit) {
            (Some(migrate), Some(exit)) => migrate(exit).await == crate::WarrenDrainPass::Migrating,
            _ => false,
        }
    }

    fn escalate_reconnect(&mut self, msg: String) {
        match self.pump_error_tx.as_ref() {
            Some(tx) => {
                log::warn!("Warren egress probe: escalating reconnect: {msg}");
                // `false` means another guard beat us to the reconnect:
                // benign, the tunnel is already leaving Connected.
                crate::reconnect_signal::escalate(tx, msg);
            }
            None => log::warn!(
                "Warren egress probe: exit not forwarding but no reconnect channel wired; \
                 verdict bannered only: {msg}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exit in use of a tunnel whose supervisor has published nothing.
    fn no_session_yet() -> ExitInUse {
        std::sync::Arc::new(|| None)
    }

    /// A path that carries nothing must never be blamed on the exit. This probe
    /// convicts an exit that ACKs keep-alives and forwards nothing, so while the
    /// peer ACKs nothing at all the premise does not hold and the redial the
    /// conviction triggers rides the same dead path. Convicting anyway costs the
    /// user every request in flight (2026-08-08 incident).
    #[tokio::test(start_paused = true)]
    async fn a_stalled_path_never_convicts_the_exit() {
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = std::sync::Arc::clone(&fired);
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: Some(std::sync::Arc::new(move |dead| {
                if dead {
                    seen.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            })),
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: Some(std::sync::Arc::new(std::sync::Mutex::new(Some(tx)))),
            exit_in_use: no_session_yet(),
            draining_exit: None,
            // The peer acknowledges nothing for the whole run: a path stall.
            acks: Some(std::sync::Arc::new(|| Some(42))),
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        };
        io.mark_streak_start();
        assert_eq!(
            io.transport_evidence(),
            TransportEvidence::Silent,
            "no ACK arrived since the streak began: the path carried nothing"
        );
        assert!(!fired.load(std::sync::atomic::Ordering::Relaxed));
    }

    /// The other side: a path that IS carrying traffic leaves the exit as the
    /// only suspect, so the conviction must still be reachable.
    #[tokio::test(start_paused = true)]
    async fn a_live_path_reports_progress_so_the_exit_stays_convictable() {
        let acks = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let reader = std::sync::Arc::clone(&acks);
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: None,
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: Some(std::sync::Arc::new(move || {
                Some(reader.load(std::sync::atomic::Ordering::Relaxed))
            })),
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        };
        io.mark_streak_start();
        acks.fetch_add(9, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(io.transport_evidence(), TransportEvidence::Progressing);
    }

    /// A saturated uplink queues our own query away while the exit keeps
    /// forwarding. Judged on the query alone the exit is convicted, the tunnel
    /// is torn down and every request in flight dies with it, over a datapath
    /// that was working. Measured on a member's line 2026-08-11: 16 such
    /// convictions in 11 hours, each with megabytes delivered over the streak.
    #[tokio::test]
    async fn an_exit_still_delivering_is_never_convicted_however_the_query_fared() {
        let delivered = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1_000));
        let read = delivered.clone();
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: None,
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: Some(std::sync::Arc::new(move || {
                Some(read.load(std::sync::atomic::Ordering::Relaxed))
            })),
            real_rx_at_streak_start: None,
        };

        io.mark_streak_start();
        assert_eq!(
            io.exit_evidence(),
            ExitEvidence::Quiet,
            "nothing delivered yet over this streak"
        );

        delivered.fetch_add(1_500, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            io.exit_evidence(),
            ExitEvidence::Delivering,
            "decoded IP packets only the exit can produce arrived over the \
             streak, so the exit is forwarding whatever our query did"
        );
    }

    /// Absent evidence must never read as observed silence, or a conviction
    /// would be suppressed on a reading nobody took.
    #[tokio::test]
    async fn no_session_reports_unknown_exit_evidence() {
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: None,
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: Some(std::sync::Arc::new(|| None)),
            real_rx_at_streak_start: None,
        };
        io.mark_streak_start();
        assert_eq!(io.exit_evidence(), ExitEvidence::Unknown);
    }

    #[tokio::test]
    async fn real_io_without_callback_or_channels_is_inert_but_alive() {
        // Inert config: no supervisor watch, no drain channel, no
        // migrate hook, no reconnect channel. session_present must
        // default to true, try_migrate to false, and escalate_reconnect
        // must not panic (it banners only).
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: None,
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        };
        assert!(io.session_present(), "no watch defaults to session present");
        assert!(!io.drain_active(), "no drain channel => never draining");
        assert!(!io.try_migrate().await, "no hook => no migration");
        io.publish(true); // must not panic without a callback
        io.escalate_reconnect("no channel".to_owned()); // must not panic
    }

    /// Probe IO on a tunnel whose session is on `exit_in_use`, whose drain
    /// channel last carried `latest`, and whose migration hook records the
    /// exit it is asked to leave into `left`.
    fn draining_io(
        exit_in_use: [u8; 16],
        latest: warrenguard_transport::supervised_pump::ExitDrainNotice,
        left: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>>,
    ) -> RealEgressProbeIo {
        RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: Some(tokio::sync::watch::channel(Some(latest)).1),
            drain_migrate: Some(std::sync::Arc::new(move |exit| {
                let left = left.clone();
                Box::pin(async move {
                    left.lock().unwrap().push(exit);
                    crate::WarrenDrainPass::Migrating
                })
            })),
            pump_error_tx: None,
            exit_in_use: std::sync::Arc::new(move || Some(exit_in_use)),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        }
    }

    fn soft_drain_of(exit: [u8; 16]) -> warrenguard_transport::supervised_pump::ExitDrainNotice {
        warrenguard_transport::supervised_pump::ExitDrainNotice {
            exit_id: warrenguard_multihop::ExitId::from_bytes(exit),
            advisory: warrenguard_transport::supervised_pump::ExitDrainAdvisory {
                deadline_unix_secs: u64::MAX,
                reason_code: 0,
            },
        }
    }

    const BUILT_FOR: [u8; 16] = [0xA0; 16];
    const MIGRATED_TO: [u8; 16] = [0xB0; 16];

    #[tokio::test]
    async fn a_drain_after_a_migration_moves_off_the_exit_the_session_is_on() {
        // A gap-free migration swapped the session onto another exit after the
        // tunnel was built, and that exit drains in turn. The exit the probe
        // hands the daemon is recorded in the avoid-set and left: it must be
        // the one now forwarding nothing.
        let left: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        let mut io = draining_io(MIGRATED_TO, soft_drain_of(MIGRATED_TO), left.clone());

        assert!(io.drain_active(), "the exit in use is draining");
        assert!(io.try_migrate().await);
        assert_eq!(*left.lock().unwrap(), vec![MIGRATED_TO]);
    }

    #[tokio::test]
    async fn the_move_charges_the_exit_that_announced_the_drain_even_after_a_swap() {
        // The supervisor may publish a swapped session between the drain check
        // and the move: the exit that sealed no advisory must not be charged.
        let left: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        let mut io = draining_io(BUILT_FOR, soft_drain_of(BUILT_FOR), left.clone());
        assert!(io.drain_active());

        io.exit_in_use = std::sync::Arc::new(|| Some(MIGRATED_TO));
        assert!(io.try_migrate().await);

        assert_eq!(*left.lock().unwrap(), vec![BUILT_FOR]);
    }

    #[tokio::test]
    async fn the_drain_of_an_exit_already_left_does_not_make_the_one_in_use_draining() {
        // The drain that moved the session off its first exit stays the last
        // notice on the channel; a dead probe on the exit it moved to is then
        // no drain, and must reconnect rather than add that exit to the
        // drained avoid-set.
        let left: std::sync::Arc<std::sync::Mutex<Vec<[u8; 16]>>> = Default::default();
        let mut io = draining_io(MIGRATED_TO, soft_drain_of(BUILT_FOR), left);

        assert!(!io.drain_active());
    }

    #[tokio::test]
    async fn real_io_forwards_the_verdict_to_the_daemon_callback() {
        let seen: std::sync::Arc<std::sync::Mutex<Vec<bool>>> = Default::default();
        let seen_cb = seen.clone();
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: Some(std::sync::Arc::new(move |dead| {
                seen_cb.lock().unwrap().push(dead);
            })),
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: None,
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        };
        io.publish(true);
        io.publish(false);
        assert_eq!(*seen.lock().unwrap(), vec![true, false]);
    }

    #[tokio::test]
    async fn real_io_escalate_reconnect_fires_the_pump_error_channel_once() {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let shared: PumpErrorTx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            startup_interval: Duration::from_secs(3),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            pump_error_tx: Some(shared.clone()),
            exit_in_use: no_session_yet(),
            draining_exit: None,
            acks: None,
            acks_at_streak_start: None,
            real_rx: None,
            real_rx_at_streak_start: None,
        };
        io.escalate_reconnect("exit not forwarding".to_owned());
        assert_eq!(
            rx.await.expect("reconnect message delivered"),
            "exit not forwarding",
            "the reconnect reason reaches the state machine via pump_error_rx"
        );
        // A second escalation (another guard racing) must be a benign
        // no-op: the sender was already taken.
        io.escalate_reconnect("second".to_owned());
        assert!(
            shared.lock().unwrap().is_none(),
            "the oneshot sender is consumed exactly once"
        );
    }
}
