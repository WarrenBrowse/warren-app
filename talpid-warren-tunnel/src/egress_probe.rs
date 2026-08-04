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
//! When the verdict fires while a drain advisory is active, the probe
//! prefers the drain reactor's gap-free migration hook
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
    EgressProbeConfig, EgressProbeIo, ProbeOutcome, jittered, probe_gateway_dns, run_egress_probe,
};

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<
    Option<std::sync::Arc<warrenguard_transport::bundle::MultiHopBundle>>,
>;
type DrainWatch =
    tokio::sync::watch::Receiver<Option<warrenguard_transport::supervised_pump::ExitDrainAdvisory>>;

/// Shared single-shot pump-error sender (the same instance the pumps and
/// `session_liveness` hold); `take()` fires the reconnect exactly once.
type PumpErrorTx = std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

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
    /// Gap-free migration hook + the exit it would migrate off.
    pub drain_migrate: Option<crate::WarrenDrainMigrate>,
    /// Shared reconnect channel. `None` only disables the escalation
    /// (verdict banner still fires), used by tests and any caller that
    /// opts out.
    pub pump_error_tx: Option<PumpErrorTx>,
    pub current_exit_id: [u8; 16],
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
        probe_gateway_dns().await
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

    fn drain_active(&mut self) -> bool {
        self.drain_rx
            .as_mut()
            .is_some_and(|rx| rx.borrow_and_update().is_some())
    }

    async fn try_migrate(&mut self) -> bool {
        match self.drain_migrate.as_ref() {
            Some(migrate) => migrate(self.current_exit_id).await,
            None => false,
        }
    }

    fn escalate_reconnect(&mut self, msg: String) {
        match self.pump_error_tx.as_ref() {
            Some(tx) => {
                if let Some(sender) = tx.lock().unwrap_or_else(|p| p.into_inner()).take() {
                    log::warn!("Warren egress probe: escalating reconnect: {msg}");
                    let _ = sender.send(msg);
                }
                // If already taken, another guard beat us to the reconnect:
                // benign, the tunnel is already leaving Connected.
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
            current_exit_id: [0; 16],
        };
        assert!(io.session_present(), "no watch defaults to session present");
        assert!(!io.drain_active(), "no drain channel => never draining");
        assert!(!io.try_migrate().await, "no hook => no migration");
        io.publish(true); // must not panic without a callback
        io.escalate_reconnect("no channel".to_owned()); // must not panic
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
            current_exit_id: [0; 16],
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
            current_exit_id: [0; 16],
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
