//! The one channel every in-tunnel guard uses to make the state machine leave
//! `Connected`.
//!
//! The pumps, the session liveness watch, the migration watchdog, the drain
//! reactor, the egress probe and the carrier egress guard all end a session
//! the same way: by sending one reason down the monitor's single-shot pump
//! error channel, which `wait()` surfaces as a recoverable backend error so
//! the state machine tears the tunnel down and reconnects. Single-shot is the
//! point: the first guard to speak ends the session, and a second reason
//! for the same death has nowhere to go and nothing to add.

use std::sync::{Arc, Mutex};

/// Shared handle on the monitor's single-shot pump error sender. `None` once
/// a guard has taken it.
pub(crate) type PumpErrorTx = Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

/// Hand `reason` to the tunnel monitor, ending the live session. Returns
/// `true` when this call is the one that ended it, `false` when another guard
/// already had (benign: the tunnel is already leaving `Connected`).
pub(crate) fn escalate(tx: &PumpErrorTx, reason: String) -> bool {
    match tx.lock().unwrap_or_else(|p| p.into_inner()).take() {
        Some(sender) => sender.send(reason).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel() -> (PumpErrorTx, tokio::sync::oneshot::Receiver<String>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (Arc::new(Mutex::new(Some(tx))), rx)
    }

    #[tokio::test]
    async fn the_first_reason_reaches_the_monitor() {
        let (tx, rx) = channel();
        assert!(escalate(&tx, "carrier dead".to_owned()));
        assert_eq!(rx.await.expect("the monitor receives it"), "carrier dead");
    }

    #[tokio::test]
    async fn a_second_reason_has_nowhere_to_go() {
        let (tx, rx) = channel();
        assert!(escalate(&tx, "first".to_owned()));
        assert!(
            !escalate(&tx, "second".to_owned()),
            "the channel is single-shot: the session is already ending"
        );
        assert_eq!(rx.await.expect("the first reason stands"), "first");
    }

    #[tokio::test]
    async fn a_monitor_that_already_left_is_not_an_escalation() {
        let (tx, rx) = channel();
        drop(rx);
        assert!(!escalate(&tx, "late".to_owned()));
    }
}
