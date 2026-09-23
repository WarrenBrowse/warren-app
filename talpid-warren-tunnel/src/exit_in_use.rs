//! The exit a multi-hop tunnel carries its traffic through right now.
//!
//! A gap-free migration (ADR 36) moves the live session onto another exit
//! without rebuilding the tunnel, so the exit the tunnel was built for only
//! names where it started. The guards that act on "this tunnel's exit" (the
//! drain reactor, the egress probe migrating off a dead exit) read it here,
//! or after the first migration they would charge the exit the session
//! already left and keep the one it is on.

use std::sync::Arc;

use warrenguard_transport::supervisor::ClientWatch;

/// The exit of the session the supervisor publishes, `None` while it
/// publishes none (between a close and the next dial).
pub(crate) type ExitInUse = Arc<dyn Fn() -> Option<[u8; 16]> + Send + Sync>;

/// Read the exit off the sessions published on `sessions`.
pub(crate) fn following(sessions: ClientWatch) -> ExitInUse {
    Arc::new(move || {
        sessions
            .borrow()
            .as_ref()
            .map(|bundle| *bundle.exit_id().as_bytes())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_no_exit_while_the_supervisor_publishes_no_session() {
        let (_publisher, sessions) = tokio::sync::watch::channel(None);

        assert_eq!(following(sessions)(), None);
    }
}
