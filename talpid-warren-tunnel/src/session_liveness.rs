//! Session-loss backstop: keeps the UI honest when the supervisor's
//! transparent redial does not succeed.
//!
//! The multi-hop supervisor absorbs QUIC disconnects by design: it
//! publishes `None`, redials with backoff, and republishes, all without
//! touching the tunnel state machine. That is the right behavior for a
//! blip (a redial lands in seconds and the user never notices), but it
//! has no exit: on a network that is dead yet still routed (a hotspot
//! that walked away keeps its interface associated, IP and default
//! route intact), no route event fires, the offline monitor still sees
//! routes, the migration watchdog never wakes, and the UI shows
//! "Connected" on a dead tunnel forever.
//!
//! This guard watches the supervisor's published-session channel
//! directly: when no session has been published for
//! [`SESSION_LOSS_ESCALATE`], it surfaces a pump error so `wait()`
//! returns `BackendTransient` and the state machine leaves Connected
//! for its normal reconnect cycle (which blocks with `IsOffline` if the
//! offline monitor agrees, or keeps retrying under the flap detector
//! otherwise). Recovery stays automatic either way; only the displayed
//! state stops lying.

use std::sync::Arc;

use std::time::Duration;

use warrenguard_transport::bundle::MultiHopBundle;

/// How long the supervisor may stay without a published session before
/// the guard escalates. Redials after a transient blip land within a
/// couple of backoff laps (300 ms / 2 s), so 15 s of continuous
/// no-session means the network is not coming back on its own; this
/// also matches the Connected state's offline grace, so both honest
/// exits from Connected trigger on the same horizon.
pub(crate) const SESSION_LOSS_ESCALATE: Duration = Duration::from_secs(15);

/// IO surface consumed by [`run_session_liveness`]; mocked in tests.
pub(crate) trait LivenessIo {
    /// Resolves on the next published-session change. Returns `false`
    /// when the channel is gone (teardown): the guard exits.
    async fn session_changed(&mut self) -> bool;

    /// `true` when a live session is currently published.
    fn has_session(&mut self) -> bool;

    /// Report the dead session to the tunnel monitor (surfaces as
    /// `BackendTransient` through the pump-error channel).
    fn escalate(&mut self, msg: String);
}

/// Guard main loop: arm a deadline whenever the published session
/// disappears, disarm it when one comes back, escalate when it expires.
pub(crate) async fn run_session_liveness<I: LivenessIo>(io: &mut I) {
    loop {
        while io.has_session() {
            if !io.session_changed().await {
                return;
            }
        }
        let deadline = tokio::time::sleep(SESSION_LOSS_ESCALATE);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                () = &mut deadline => {
                    // Re-check: a republish and the timer can race.
                    if io.has_session() {
                        break;
                    }
                    io.escalate(format!(
                        "tunnel session lost and not re-established within \
                         {SESSION_LOSS_ESCALATE:?}; leaving Connected"
                    ));
                    return;
                }
                alive = io.session_changed() => {
                    if !alive {
                        return;
                    }
                    if io.has_session() {
                        break;
                    }
                }
            }
        }
    }
}

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<Option<Arc<MultiHopBundle>>>;
/// Shared single-shot pump error sender (same instance the pumps use).
type PumpErrorTx = Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

/// Production bindings for [`LivenessIo`].
pub(crate) struct RealLivenessIo {
    pub client_rx: ClientWatch,
    pub pump_error_tx: PumpErrorTx,
}

impl LivenessIo for RealLivenessIo {
    async fn session_changed(&mut self) -> bool {
        self.client_rx.changed().await.is_ok()
    }

    fn has_session(&mut self) -> bool {
        self.client_rx.borrow_and_update().is_some()
    }

    fn escalate(&mut self, msg: String) {
        log::warn!("Warren session liveness: escalating to the state machine: {msg}");
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

    /// Scripted mock driven by a watch channel of bools (session
    /// present / absent); escalations are recorded.
    struct MockIo {
        session_rx: tokio::sync::watch::Receiver<bool>,
        escalations: Vec<String>,
    }

    fn mock() -> (MockIo, tokio::sync::watch::Sender<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(true);
        (
            MockIo {
                session_rx: rx,
                escalations: Vec::new(),
            },
            tx,
        )
    }

    impl LivenessIo for MockIo {
        async fn session_changed(&mut self) -> bool {
            self.session_rx.changed().await.is_ok()
        }
        fn has_session(&mut self) -> bool {
            *self.session_rx.borrow_and_update()
        }
        fn escalate(&mut self, msg: String) {
            self.escalations.push(msg);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn escalates_when_session_stays_lost_past_the_deadline() {
        let (mut io, tx) = mock();
        let driver = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tx.send(false).expect("guard alive");
            // Keep the sender alive well past the escalation deadline.
            tokio::time::sleep(SESSION_LOSS_ESCALATE * 3).await;
            drop(tx);
        });
        run_session_liveness(&mut io).await;
        driver.await.expect("driver task");
        assert_eq!(io.escalations.len(), 1, "one escalation exactly");
        assert!(io.escalations[0].contains("not re-established"));
    }

    #[tokio::test(start_paused = true)]
    async fn republish_before_the_deadline_disarms_the_guard() {
        let (mut io, tx) = mock();
        let driver = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tx.send(false).expect("guard alive");
            // Redial lands well inside the escalation window.
            tokio::time::sleep(SESSION_LOSS_ESCALATE / 3).await;
            tx.send(true).expect("guard alive");
            // Stay connected long past the old deadline, then tear down.
            tokio::time::sleep(SESSION_LOSS_ESCALATE * 3).await;
            drop(tx);
        });
        run_session_liveness(&mut io).await;
        driver.await.expect("driver task");
        assert!(
            io.escalations.is_empty(),
            "a recovered session must never escalate"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_short_losses_never_escalate() {
        let (mut io, tx) = mock();
        let driver = tokio::spawn(async move {
            for _ in 0..5 {
                tokio::time::sleep(Duration::from_secs(2)).await;
                tx.send(false).expect("guard alive");
                tokio::time::sleep(Duration::from_secs(2)).await;
                tx.send(true).expect("guard alive");
            }
            drop(tx);
        });
        run_session_liveness(&mut io).await;
        driver.await.expect("driver task");
        assert!(io.escalations.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn teardown_while_lost_exits_without_escalating() {
        let (mut io, tx) = mock();
        let driver = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tx.send(false).expect("guard alive");
            // Channel closes (monitor teardown) before the deadline.
            tokio::time::sleep(Duration::from_secs(2)).await;
            drop(tx);
        });
        run_session_liveness(&mut io).await;
        driver.await.expect("driver task");
        assert!(io.escalations.is_empty());
    }
}
