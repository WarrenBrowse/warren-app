//! The session status cell Kotlin waits on instead of polling.
//!
//! Every publisher of a Kotlin-visible fact (the session status, the datapath
//! verdicts, the NAT-PMP mapping, the recovery counter) bumps one generation
//! counter and wakes the waiters. The JNI `awaitStatusChange` blocks on it, so
//! a transition reaches the UI the moment it is published instead of on the
//! next tick of a timer, and an idle session wakes nothing.

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use tokio::sync::Notify;

/// An `i32` status plus the generation counter every change of it, or of a
/// sibling fact riding the same wake, advances.
pub(crate) struct StatusCell {
    value: AtomicI32,
    generation: AtomicU64,
    changed: Notify,
}

impl StatusCell {
    pub(crate) const fn new(initial: i32) -> Self {
        Self {
            value: AtomicI32::new(initial),
            generation: AtomicU64::new(0),
            changed: Notify::const_new(),
        }
    }

    pub(crate) fn load(&self) -> i32 {
        self.value.load(Ordering::SeqCst)
    }

    /// Publish a new status and wake the waiters.
    pub(crate) fn store(&self, value: i32) {
        self.value.store(value, Ordering::SeqCst);
        self.bump();
    }

    /// Wake the waiters for a change of a sibling fact (a verdict, the
    /// NAT-PMP mapping, the recovery counter) the status itself did not
    /// carry: the waiter re-reads every fact on each wake.
    pub(crate) fn bump(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.changed.notify_waiters();
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Resolve with the current generation as soon as it differs from
    /// `last_seen`, at once when a change already happened.
    ///
    /// The `Notified` future is created BEFORE the generation is read: tokio
    /// guarantees it receives a `notify_waiters` issued after its creation
    /// even when it has not been polled yet, so a bump landing between the
    /// read and the await can never be lost.
    pub(crate) async fn changed_since(&self, last_seen: u64) -> u64 {
        loop {
            let notified = self.changed.notified();
            let now = self.generation();
            if now != last_seen {
                return now;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn a_store_wakes_a_waiter_registered_before_it() {
        let cell = Arc::new(StatusCell::new(1));
        let waiter = tokio::spawn({
            let cell = Arc::clone(&cell);
            async move { cell.changed_since(0).await }
        });
        // Let the waiter park on the notify before the change lands.
        tokio::time::sleep(Duration::from_millis(50)).await;

        cell.store(2);

        let generation = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("the waiter must be woken by the store")
            .expect("the waiter task must not panic");
        assert_eq!(generation, 1, "one change advances the generation once");
        assert_eq!(
            cell.load(),
            2,
            "the waiter reads the stored value on its wake"
        );
    }

    #[tokio::test]
    async fn a_change_already_published_resolves_without_waiting() {
        let cell = StatusCell::new(1);
        cell.store(2);
        cell.store(0);

        let generation = tokio::time::timeout(Duration::from_millis(100), cell.changed_since(0))
            .await
            .expect("a generation the caller has not seen must resolve at once");

        assert_eq!(
            generation, 2,
            "the newest generation, never an intermediate one"
        );
    }

    #[tokio::test]
    async fn a_side_channel_bump_wakes_without_touching_the_value() {
        let cell = Arc::new(StatusCell::new(2));
        let waiter = tokio::spawn({
            let cell = Arc::clone(&cell);
            async move { cell.changed_since(0).await }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        cell.bump();

        let generation = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("a bump must wake the waiter like a store does")
            .expect("the waiter task must not panic");
        assert_eq!(generation, 1);
        assert_eq!(cell.load(), 2, "a bump carries no value of its own");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unchanged_cell_keeps_the_waiter_pending_until_the_timeout() {
        let cell = StatusCell::new(2);
        let seen = cell.generation();

        let outcome = tokio::time::timeout(Duration::from_secs(1), cell.changed_since(seen)).await;

        assert!(
            outcome.is_err(),
            "nothing changed, so the wait must run to the caller's timeout"
        );
        assert_eq!(
            cell.generation(),
            seen,
            "waiting must not advance the generation"
        );
    }
}
