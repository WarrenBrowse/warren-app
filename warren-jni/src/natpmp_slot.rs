// The NAT-PMP refresh loop's slot: where the starter task leaves the handle
// that cancels the loop, and where the session teardown picks it up. Pure
// state machine, host-tested; the wiring that fills it is Android-gated in
// `tunnel`.

/// What the session knows about its NAT-PMP refresh loop.
///
/// An `Option` cannot express the state that decides the race here. The
/// starter gives the entitlement mint a head start, then spawns the loop and
/// stores its handle, and those last two steps have no await between them.
/// `tokio::task::abort()` only takes effect at a yield point, so it cannot
/// preempt a starter already past its last one: a teardown that empties an
/// `Option` slot in that window leaves the freshly spawned loop with nobody
/// holding its handle, renewing a mapping against a TUN that is gone.
///
/// [`Cancelled`](Self::Cancelled) is that missing state: the teardown writes
/// it, and a starter that finds it cancels the loop itself instead of storing
/// a handle into a guard that no longer exists.
#[derive(Debug)]
pub(crate) enum NatPmpSlot<H> {
    /// No loop yet: the starter is still waiting on the entitlement mint.
    Pending,
    /// The loop is running, and this handle cancels it.
    Running(H),
    /// The session is over. Nothing may be stored here any more.
    Cancelled,
}

impl<H> NatPmpSlot<H> {
    /// End the session: marks the slot cancelled and hands back the handle to
    /// cancel, if the loop had already been stored.
    pub(crate) fn cancel(&mut self) -> Option<H> {
        match std::mem::replace(self, Self::Cancelled) {
            Self::Running(handle) => Some(handle),
            Self::Pending | Self::Cancelled => None,
        }
    }

    /// Store the handle of a loop that has just been spawned.
    ///
    /// Hands the handle straight back when the session is already over, which
    /// is the caller's cue to cancel the loop it just started rather than
    /// leave it running for the life of the process.
    pub(crate) fn store(&mut self, handle: H) -> Option<H> {
        if matches!(self, Self::Cancelled) {
            Some(handle)
        } else {
            *self = Self::Running(handle);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NatPmpSlot;

    #[test]
    fn a_stored_loop_is_the_one_the_teardown_cancels() {
        let mut slot = NatPmpSlot::Pending;

        assert!(slot.store(7).is_none(), "an open slot keeps the handle");

        assert_eq!(slot.cancel(), Some(7));
    }

    /// The window the state exists for: the teardown runs while the starter is
    /// between spawning the loop and storing its handle, which no abort can
    /// interrupt. The starter is then the one holding a live loop, so it is
    /// the one that has to stop it.
    #[test]
    fn a_loop_spawned_after_the_teardown_is_handed_back_to_be_cancelled() {
        let mut slot = NatPmpSlot::Pending;
        assert_eq!(slot.cancel(), None, "nothing was running yet");

        assert_eq!(
            slot.store(7),
            Some(7),
            "the session is over: the starter cancels what it just spawned"
        );
        assert!(matches!(slot, NatPmpSlot::Cancelled));
    }

    #[test]
    fn a_second_teardown_has_nothing_left_to_cancel() {
        let mut slot = NatPmpSlot::Pending;
        slot.store(7);

        assert_eq!(slot.cancel(), Some(7));
        assert_eq!(slot.cancel(), None);
    }
}
