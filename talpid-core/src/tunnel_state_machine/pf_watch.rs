//! Has the state machine check, at a steady pace, that pf still enforces the policy it applied.
//!
//! pf announces nothing when another process reloads the main ruleset or disables pf, and either
//! leaves this build's anchor defined and enforcing nothing. Each check costs a few ioctls, and
//! puts the policy back only when it is gone.

use std::sync::Weak;
use std::time::Duration;

use futures::channel::mpsc;

use super::TunnelCommand;

/// The longest pf can go without the policy before it is put back.
pub(super) const INTERVAL: Duration = Duration::from_secs(2);

/// Sends [`TunnelCommand::ReassertFirewall`] every `interval`, until the state machine is gone.
pub(super) async fn run_pf_watch(
    command_tx: Weak<mpsc::UnboundedSender<TunnelCommand>>,
    interval: Duration,
) {
    loop {
        tokio::time::sleep(interval).await;
        let Some(tx) = command_tx.upgrade() else {
            return;
        };
        if tx.unbounded_send(TunnelCommand::ReassertFirewall).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn checks(commands: &mut mpsc::UnboundedReceiver<TunnelCommand>) -> usize {
        let mut count = 0;
        while let Ok(command) = commands.try_recv() {
            assert!(matches!(command, TunnelCommand::ReassertFirewall));
            count += 1;
        }
        count
    }

    async fn advance(by: Duration) {
        tokio::task::yield_now().await;
        tokio::time::advance(by).await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn the_state_machine_is_asked_to_check_once_per_interval() {
        let (command_tx, mut commands) = mpsc::unbounded();
        let command_tx = Arc::new(command_tx);
        tokio::spawn(run_pf_watch(Arc::downgrade(&command_tx), INTERVAL));

        advance(INTERVAL / 2).await;
        assert_eq!(checks(&mut commands), 0);
        advance(INTERVAL / 2).await;
        assert_eq!(checks(&mut commands), 1);
        for _ in 0..3 {
            advance(INTERVAL).await;
            assert_eq!(checks(&mut commands), 1);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_watch_ends_with_the_state_machine() {
        let (command_tx, _commands) = mpsc::unbounded();
        let command_tx = Arc::new(command_tx);
        let watch = tokio::spawn(run_pf_watch(Arc::downgrade(&command_tx), INTERVAL));

        drop(command_tx);
        advance(INTERVAL).await;

        assert!(watch.is_finished());
    }
}
