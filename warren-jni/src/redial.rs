//! Redial supervision for the Android tunnel session.
//!
//! Mirrors the desktop split between the transparent supervisor redial and
//! the `session_liveness` backstop, collapsed into one loop because Android
//! runs a single session with no daemon state machine:
//!
//! - On session loss that is neither a user cancel nor a policy rejection,
//!   the engine publishes [`SessionStatus::Reconnecting`] and redials with
//!   the engine backoff ([`warrenguard_backoff::Backoff::HANDSHAKE`]).
//! - A redial that lands re-publishes [`SessionStatus::Connected`] and is
//!   counted as an automatic recovery ([`SessionIo::note_auto_recovery`]).
//! - When no redial lands within [`SESSION_LOSS_ESCALATE`] of the loss, the
//!   engine gives up and publishes [`SessionStatus::Disconnected`].
//!
//! Contract with the Kotlin layer (the honest-status contract):
//!
//! - `Reconnecting` (3) means "quick blip": a transparent redial is in
//!   flight and expected to land within seconds. The VpnService TUN is
//!   still established and captures all traffic (the dead pump drops it),
//!   so no kill-switch action is needed during this window.
//! - `Disconnected` (0) after a `Reconnecting` window means "network gone":
//!   the 15 s deadline expired without a successful redial. The Kotlin
//!   fail-closed policy takes over (blackhole interface + connectivity
//!   gated retry), exactly as it does for any other session death.
//! - `Unauthorized` (4) is terminal on any dial: retrying cannot recover a
//!   lapsed subscription, so the engine never redials past it.

use std::time::Duration;

use warrenguard_backoff::Backoff;

/// How long a lost session may stay lost before the engine stops
/// redialing and reports `Disconnected`. Matches the desktop
/// `session_liveness::SESSION_LOSS_ESCALATE`: redials after a transient
/// blip land within a couple of backoff laps, so 15 s of continuous
/// no-session means the network is not coming back on its own and the
/// Kotlin fail-closed policy must take over.
pub const SESSION_LOSS_ESCALATE: Duration = Duration::from_secs(15);

/// Upper bound on redial attempts per loss window. The deadline is the
/// real limit (HANDSHAKE backoff reaches its ceiling well before 32
/// attempts); this only keeps the backoff iterator finite.
const REDIAL_ATTEMPT_CAP: usize = 32;

/// Tunnel session status reported back to Kotlin via
/// `WarrenJni.getTunnelStatus()`. Encoded as an `i32` rather than an enum
/// to match the existing JNI int contract.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    /// Transparent redial in flight after a session loss. See the module
    /// doc for the Kotlin-side contract.
    Reconnecting = 3,
    /// The exit refused the setup with a policy rejection: the account is
    /// not authorized, i.e. the subscription has lapsed or was revoked.
    /// Distinct from `Disconnected` so the Kotlin layer can surface
    /// "subscription expired" and STOP the reconnect loop (retrying cannot
    /// recover an unauthorized account until it is renewed).
    Unauthorized = 4,
}

/// Outcome of one dial attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialOutcome {
    /// A session is staged and ready to pump.
    Established,
    /// The exit policy-rejected the identity; terminal, never retried.
    Unauthorized,
    /// Transport-level failure; retried under the backoff until the
    /// loss deadline.
    Failed,
}

/// Session side effects consumed by [`run_supervised`]; implemented by the
/// real tunnel in `tunnel.rs` and mocked in tests.
pub trait SessionIo {
    /// Dial the exit and stage a live session for [`SessionIo::pump`].
    fn dial(&mut self) -> impl Future<Output = DialOutcome> + Send;

    /// Drive the staged session until it dies (connection closed, pump
    /// error). Cancellation is handled by the engine, not here.
    fn pump(&mut self) -> impl Future<Output = ()> + Send;

    /// Publish a status transition toward the Kotlin poller.
    fn publish(&mut self, status: SessionStatus);

    /// Record one automatic recovery (a redial that landed after a loss).
    /// Never invoked for the initial connect or any user action.
    fn note_auto_recovery(&mut self);
}

/// Sticky cancellation signal: `wait` resolves when the user tears the
/// tunnel down, and keeps resolving immediately afterwards so the engine
/// can re-arm it in successive `select!`s.
pub trait CancelSignal {
    fn wait(&mut self) -> impl Future<Output = ()> + Send;
}

/// [`CancelSignal`] over the JNI cancel oneshot. Both an explicit send and
/// the sender being dropped (how `disconnectTunnel` signals) mean cancel.
pub struct OneshotCancel {
    rx: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl OneshotCancel {
    pub fn new(rx: tokio::sync::oneshot::Receiver<()>) -> Self {
        Self { rx: Some(rx) }
    }
}

impl CancelSignal for OneshotCancel {
    async fn wait(&mut self) {
        if let Some(rx) = self.rx.take() {
            let _ = rx.await;
        }
    }
}

/// Verdict of a cancellable, optionally deadline-bounded dial.
enum DialVerdict {
    Established,
    Unauthorized,
    Failed,
    Cancelled,
}

async fn dial_bounded<I: SessionIo, C: CancelSignal>(
    io: &mut I,
    cancel: &mut C,
    deadline: Option<tokio::time::Instant>,
) -> DialVerdict {
    tokio::select! {
        () = cancel.wait() => DialVerdict::Cancelled,
        outcome = async {
            match deadline {
                // A dial still in flight at the loss deadline is cut: the
                // deadline is the honesty contract, not a best effort.
                Some(d) => tokio::time::timeout_at(d, io.dial()).await.ok(),
                None => Some(io.dial().await),
            }
        } => match outcome {
            Some(DialOutcome::Established) => DialVerdict::Established,
            Some(DialOutcome::Unauthorized) => DialVerdict::Unauthorized,
            Some(DialOutcome::Failed) | None => DialVerdict::Failed,
        },
    }
}

/// Drive a session from initial dial through losses and redials until a
/// terminal status (`Disconnected` or `Unauthorized`) is published.
///
/// The initial dial gets a single attempt: its failure policy (blackhole,
/// retry scheduling, flap detection) belongs to the Kotlin layer, which
/// initiated the connect and owns the fail-closed machinery. Only
/// mid-session losses are redialed here, because they are invisible to
/// Kotlin until this engine reports them.
pub async fn run_supervised<I: SessionIo, C: CancelSignal>(io: &mut I, cancel: &mut C) {
    io.publish(SessionStatus::Connecting);
    match dial_bounded(io, cancel, None).await {
        DialVerdict::Established => io.publish(SessionStatus::Connected),
        DialVerdict::Unauthorized => {
            io.publish(SessionStatus::Unauthorized);
            return;
        }
        DialVerdict::Failed | DialVerdict::Cancelled => {
            io.publish(SessionStatus::Disconnected);
            return;
        }
    }

    loop {
        tokio::select! {
            () = cancel.wait() => {
                io.publish(SessionStatus::Disconnected);
                return;
            }
            () = io.pump() => {}
        }

        io.publish(SessionStatus::Reconnecting);
        let deadline = tokio::time::Instant::now() + SESSION_LOSS_ESCALATE;
        let mut recovered = false;
        for delay in Backoff::HANDSHAKE.take(REDIAL_ATTEMPT_CAP) {
            let gave_up = tokio::select! {
                () = cancel.wait() => true,
                () = tokio::time::sleep_until(deadline) => true,
                () = tokio::time::sleep(delay) => false,
            };
            if gave_up {
                break;
            }
            match dial_bounded(io, cancel, Some(deadline)).await {
                DialVerdict::Established => {
                    recovered = true;
                    break;
                }
                DialVerdict::Unauthorized => {
                    io.publish(SessionStatus::Unauthorized);
                    return;
                }
                DialVerdict::Failed => {}
                DialVerdict::Cancelled => break,
            }
        }
        if !recovered {
            io.publish(SessionStatus::Disconnected);
            return;
        }
        io.publish(SessionStatus::Connected);
        io.note_auto_recovery();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    /// Scripted [`SessionIo`]: dial outcomes pop from a queue (an empty
    /// queue keeps failing), pump behaviors likewise (an empty queue
    /// pends forever, i.e. a healthy session).
    struct MockIo {
        dials: VecDeque<DialOutcome>,
        pumps: VecDeque<PumpScript>,
        statuses: Vec<SessionStatus>,
        recoveries: u32,
    }

    enum PumpScript {
        LostAfter(Duration),
        Never,
    }

    impl MockIo {
        fn new(dials: Vec<DialOutcome>, pumps: Vec<PumpScript>) -> Self {
            Self {
                dials: dials.into(),
                pumps: pumps.into(),
                statuses: Vec::new(),
                recoveries: 0,
            }
        }
    }

    impl SessionIo for MockIo {
        async fn dial(&mut self) -> DialOutcome {
            self.dials.pop_front().unwrap_or(DialOutcome::Failed)
        }

        async fn pump(&mut self) {
            match self.pumps.pop_front() {
                Some(PumpScript::LostAfter(d)) => tokio::time::sleep(d).await,
                Some(PumpScript::Never) | None => std::future::pending::<()>().await,
            }
        }

        fn publish(&mut self, status: SessionStatus) {
            self.statuses.push(status);
        }

        fn note_auto_recovery(&mut self) {
            self.recoveries += 1;
        }
    }

    struct WatchCancel(tokio::sync::watch::Receiver<bool>);

    impl CancelSignal for WatchCancel {
        async fn wait(&mut self) {
            if *self.0.borrow_and_update() {
                return;
            }
            while self.0.changed().await.is_ok() {
                if *self.0.borrow_and_update() {
                    return;
                }
            }
            // Sender gone without firing: never cancelled.
            std::future::pending::<()>().await;
        }
    }

    fn cancel_pair() -> (tokio::sync::watch::Sender<bool>, WatchCancel) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (tx, WatchCancel(rx))
    }

    #[tokio::test(start_paused = true)]
    async fn redial_success_reconnects_and_counts_one_recovery() {
        use DialOutcome::*;
        let mut io = MockIo::new(
            vec![Established, Failed, Established],
            vec![
                PumpScript::LostAfter(Duration::from_secs(5)),
                PumpScript::Never,
            ],
        );
        let (cancel_tx, mut cancel) = cancel_pair();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let _ = cancel_tx.send(true);
        });
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(
            io.statuses,
            vec![
                SessionStatus::Connecting,
                SessionStatus::Connected,
                SessionStatus::Reconnecting,
                SessionStatus::Connected,
                SessionStatus::Disconnected,
            ],
            "loss must surface Reconnecting, then Connected on redial success"
        );
        assert_eq!(io.recoveries, 1, "exactly one automatic recovery");
    }

    #[tokio::test(start_paused = true)]
    async fn continuous_loss_escalates_to_disconnected_at_the_deadline() {
        let mut io = MockIo::new(
            vec![DialOutcome::Established],
            vec![PumpScript::LostAfter(Duration::from_secs(1))],
        );
        let (_cancel_tx, mut cancel) = cancel_pair();
        let start = tokio::time::Instant::now();
        run_supervised(&mut io, &mut cancel).await;
        let elapsed = start.elapsed();
        assert_eq!(
            io.statuses.last(),
            Some(&SessionStatus::Disconnected),
            "an unrecoverable loss must end Disconnected (Kotlin takes over)"
        );
        assert!(
            io.statuses.contains(&SessionStatus::Reconnecting),
            "the loss window must be visible as Reconnecting"
        );
        // 1 s of pump + the 15 s loss deadline.
        assert!(
            elapsed >= Duration::from_secs(16) && elapsed < Duration::from_secs(17),
            "give-up must land on the loss deadline, got {elapsed:?}"
        );
        assert_eq!(io.recoveries, 0, "a failed recovery must never count");
    }

    #[tokio::test(start_paused = true)]
    async fn unauthorized_on_redial_is_terminal() {
        let mut io = MockIo::new(
            vec![DialOutcome::Established, DialOutcome::Unauthorized],
            vec![PumpScript::LostAfter(Duration::from_secs(1))],
        );
        let (_cancel_tx, mut cancel) = cancel_pair();
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(
            io.statuses.last(),
            Some(&SessionStatus::Unauthorized),
            "a policy rejection must stop the redial loop with Unauthorized"
        );
        assert_eq!(io.recoveries, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_during_redial_publishes_disconnected_promptly() {
        let mut io = MockIo::new(
            vec![DialOutcome::Established],
            vec![PumpScript::LostAfter(Duration::from_secs(1))],
        );
        let (cancel_tx, mut cancel) = cancel_pair();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = cancel_tx.send(true);
        });
        let start = tokio::time::Instant::now();
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(io.statuses.last(), Some(&SessionStatus::Disconnected));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "cancel must cut the redial loop before the loss deadline"
        );
        assert_eq!(io.recoveries, 0, "a user cancel never counts as recovery");
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_during_pump_publishes_disconnected() {
        let mut io = MockIo::new(vec![DialOutcome::Established], vec![PumpScript::Never]);
        let (cancel_tx, mut cancel) = cancel_pair();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = cancel_tx.send(true);
        });
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(
            io.statuses,
            vec![
                SessionStatus::Connecting,
                SessionStatus::Connected,
                SessionStatus::Disconnected,
            ]
        );
        assert_eq!(io.recoveries, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn initial_dial_failure_is_disconnected_without_redial() {
        let mut io = MockIo::new(vec![DialOutcome::Failed], vec![]);
        let (_cancel_tx, mut cancel) = cancel_pair();
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(
            io.statuses,
            vec![SessionStatus::Connecting, SessionStatus::Disconnected],
            "initial connect failures belong to the Kotlin retry policy"
        );
        assert_eq!(io.recoveries, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn initial_unauthorized_is_terminal() {
        let mut io = MockIo::new(vec![DialOutcome::Unauthorized], vec![]);
        let (_cancel_tx, mut cancel) = cancel_pair();
        run_supervised(&mut io, &mut cancel).await;
        assert_eq!(
            io.statuses,
            vec![SessionStatus::Connecting, SessionStatus::Unauthorized]
        );
    }

    #[tokio::test]
    async fn oneshot_cancel_fires_on_sender_drop_and_stays_fired() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let mut cancel = OneshotCancel::new(rx);
        drop(tx);
        cancel.wait().await;
        // Sticky: a second wait resolves immediately instead of hanging.
        cancel.wait().await;
    }
}
