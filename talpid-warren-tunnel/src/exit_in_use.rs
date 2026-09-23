//! The exit a multi-hop tunnel carries its traffic through right now.
//!
//! A gap-free migration (ADR 36) moves the live session onto another exit
//! without rebuilding the tunnel, so the exit the tunnel was built for only
//! names where it started. The guards that act on "this tunnel's exit" (the
//! drain reactor, the egress probe migrating off a dead exit) read it here,
//! or after the first migration they would charge the exit the session
//! already left and keep the one it is on.

use std::sync::Arc;

use tokio::sync::watch;
use warrenguard_transport::bundle::MultiHopBundle;

/// The exit of the session the supervisor publishes, `None` while it
/// publishes none (between a close and the next dial).
pub(crate) type ExitInUse = Arc<dyn Fn() -> Option<[u8; 16]> + Send + Sync>;

/// A published session, as far as naming its exit goes.
pub(crate) trait SessionExit {
    fn exit_id_bytes(&self) -> [u8; 16];
}

impl SessionExit for MultiHopBundle {
    fn exit_id_bytes(&self) -> [u8; 16] {
        *self.exit_id().as_bytes()
    }
}

/// Read the exit off the sessions published on `sessions`.
pub(crate) fn following<S>(sessions: watch::Receiver<Option<Arc<S>>>) -> ExitInUse
where
    S: SessionExit + Send + Sync + 'static,
{
    Arc::new(move || sessions.borrow().as_ref().map(|s| s.exit_id_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Session([u8; 16]);

    impl SessionExit for Session {
        fn exit_id_bytes(&self) -> [u8; 16] {
            self.0
        }
    }

    #[test]
    fn names_the_exit_of_the_session_published_now() {
        let (publisher, sessions) = watch::channel(Some(Arc::new(Session([1; 16]))));
        let exit_in_use = following(sessions);
        assert_eq!(exit_in_use(), Some([1; 16]));

        publisher.send_replace(Some(Arc::new(Session([2; 16]))));
        assert_eq!(
            exit_in_use(),
            Some([2; 16]),
            "a migration swapped the session"
        );

        publisher.send_replace(None);
        assert_eq!(
            exit_in_use(),
            None,
            "no session is published while the supervisor redials"
        );
    }
}
