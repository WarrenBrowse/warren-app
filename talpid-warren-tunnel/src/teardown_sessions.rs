//! Closes the transport sessions a torn-down tunnel still holds open.
//!
//! Aborting the tunnel's tasks drops their references to the session bundle,
//! and the bundle closes its QUIC sessions when the last reference goes. One
//! reference outlives the teardown: the engine's path-health prober holds the
//! bundle through each probe round, a sleep of its whole cadence (15 s by
//! default), and gives it up early only when the bundle's sessions close,
//! which nothing did. So the sessions stayed open for up to a cadence after
//! the tunnel reported Disconnected: the exit kept a live session for a client
//! that had left, and the path probe, which holds clones of the connections
//! and ends only once all of them are closed, logged them after the
//! disconnect. Teardown therefore closes the last published bundle itself,
//! before it returns, whoever still holds it; the prober then lets go at once.
//! The close is idempotent, so a bundle already closed costs nothing.

use std::sync::Arc;

use warrenguard_transport::bundle::MultiHopBundle;

/// QUIC application close code of a teardown: a normal close, as the bundle's
/// own drop uses.
const TEARDOWN_CLOSE_CODE: u32 = 0;

/// The sessions of one published bundle, as teardown sees them.
pub(crate) trait TeardownSessions {
    /// How many of its sessions are still open.
    fn open_sessions(&self) -> usize;
    /// Closes every one of them.
    fn close_sessions(&self);
}

impl TeardownSessions for MultiHopBundle {
    fn open_sessions(&self) -> usize {
        self.clone_connections()
            .iter()
            .filter(|conn| conn.close_reason().is_none())
            .count()
    }

    fn close_sessions(&self) {
        self.close(TEARDOWN_CLOSE_CODE, b"tunnel down");
    }
}

/// Closes the sessions of `last`, the bundle the supervisor last published,
/// and answers how many were still open.
pub(crate) fn close_leftover_sessions<B: TeardownSessions>(last: Option<&Arc<B>>) -> usize {
    let Some(bundle) = last else {
        return 0;
    };
    let open = bundle.open_sessions();
    bundle.close_sessions();
    open
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{TeardownSessions, close_leftover_sessions};

    /// A bundle of `n` sessions that stays open until closed, whoever else
    /// holds it.
    struct FakeBundle {
        open: AtomicUsize,
    }

    impl FakeBundle {
        fn with_open(n: usize) -> Arc<Self> {
            Arc::new(Self {
                open: AtomicUsize::new(n),
            })
        }
    }

    impl TeardownSessions for FakeBundle {
        fn open_sessions(&self) -> usize {
            self.open.load(Ordering::SeqCst)
        }

        fn close_sessions(&self) {
            self.open.store(0, Ordering::SeqCst);
        }
    }

    #[test]
    fn teardown_closes_the_sessions_a_leftover_reference_keeps_open() {
        let bundle = FakeBundle::with_open(8);
        let leftover = Arc::clone(&bundle);

        let closed = close_leftover_sessions(Some(&bundle));

        assert_eq!(closed, 8);
        assert_eq!(
            leftover.open_sessions(),
            0,
            "the reference that outlives the teardown sees them closed"
        );
    }

    #[test]
    fn a_bundle_already_closed_reports_nothing_left() {
        let bundle = FakeBundle::with_open(0);

        assert_eq!(close_leftover_sessions(Some(&bundle)), 0);
    }

    #[test]
    fn no_published_bundle_is_nothing_to_close() {
        assert_eq!(close_leftover_sessions::<FakeBundle>(None), 0);
    }
}
