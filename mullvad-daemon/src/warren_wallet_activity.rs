//! Whether the API work tied to the wallet may run.
//!
//! A logout that keeps the recovery phrase leaves the wallet loaded, so every
//! background task that signs with it (the v7 token refresh and the route
//! admission it reads, the port-entitlement mint, the standing poll, the
//! campaign voucher lookup) would carry on for an account nobody is logged in
//! to. They all wait on this switch instead: the daemon turns it off when an
//! account logs out or is revoked and on when one logs in, and a task that
//! finds it off parks until it comes back on. A round already in flight is
//! left to finish, because an issuance answer dropped halfway would cost the
//! epoch it paid for.

use std::sync::Arc;

use tokio::sync::watch;

/// The switch, shared by the daemon (which sets it) and the tasks that wait
/// on it.
#[derive(Clone)]
pub(crate) struct WalletActivity {
    tx: Arc<watch::Sender<bool>>,
}

impl WalletActivity {
    pub(crate) fn new(active: bool) -> Self {
        Self {
            tx: Arc::new(watch::Sender::new(active)),
        }
    }

    /// Turns the wallet's API work on or off. Waiters hear only a change.
    pub(crate) fn set(&self, active: bool) {
        self.tx.send_if_modified(|current| {
            let changed = *current != active;
            *current = active;
            changed
        });
    }

    pub(crate) fn is_active(&self) -> bool {
        *self.tx.borrow()
    }

    /// A receiver for a task that lives in another crate.
    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }

    /// Returns once the wallet's API work may run.
    pub(crate) async fn wait_active(&self) {
        let mut rx = self.tx.subscribe();
        // The sender lives in `self`, so the wait can only end on a change.
        let _ = rx.wait_for(|active| *active).await;
    }

    /// [`Self::wait_active`], saying in the log that `task` waits and when
    /// it resumes, so a quiet logged-out daemon reads as intended.
    pub(crate) async fn park_while_inactive(&self, task: &str) {
        if self.is_active() {
            return;
        }
        log::info!("{task} paused: no account is logged in");
        self.wait_active().await;
        log::info!("{task} resumed: an account is logged in");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::WalletActivity;

    #[tokio::test]
    async fn a_waiter_returns_at_once_while_active() {
        let activity = WalletActivity::new(true);

        tokio::time::timeout(Duration::from_millis(100), activity.wait_active())
            .await
            .expect("an active wallet does not park its tasks");
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_parks_while_inactive_and_resumes_on_login() {
        let activity = WalletActivity::new(false);
        let waiter = tokio::spawn({
            let activity = activity.clone();
            async move { activity.wait_active().await }
        });

        tokio::time::sleep(Duration::from_secs(3600)).await;
        assert!(!waiter.is_finished(), "a logged-out wallet parks its tasks");

        activity.set(true);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("a login resumes the parked tasks")
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_task_parked_by_a_logout_runs_its_next_round_only_after_a_login() {
        let activity = WalletActivity::new(true);
        let rounds = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let task = tokio::spawn({
            let activity = activity.clone();
            let rounds = rounds.clone();
            async move {
                loop {
                    activity.park_while_inactive("test task").await;
                    rounds.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rounds.load(std::sync::atomic::Ordering::SeqCst), 1);

        activity.set(false);
        tokio::time::sleep(Duration::from_secs(600)).await;
        assert_eq!(
            rounds.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "no round runs while logged out"
        );

        activity.set(true);
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rounds.load(std::sync::atomic::Ordering::SeqCst), 2);
        task.abort();
    }

    #[test]
    fn the_switch_reads_back_what_was_set() {
        let activity = WalletActivity::new(true);

        activity.set(false);

        assert!(!activity.is_active());
        assert!(!*activity.subscribe().borrow());
    }
}
