//! Moving an Android session off a node that refuses its dials or announces a
//! maintenance drain (ADR 36).
//!
//! The engine names the node a connection terminates at for every refused
//! dial: the entry relay, which on a one-hop circuit is the exit itself. It
//! never names the exit behind a relay, so a hostile relay can make the client
//! avoid that relay and nothing else.
//!
//! Android fixes the TUN address at `establish()`, and the exit is Kotlin's
//! choice (it applies the location pin and the key pins), so a live retarget
//! here can only change the entry of a two-hop circuit: the exit, and the inner
//! address it assigned, stay. A refusing or draining exit ends the session
//! instead, for Kotlin to fail over to another exit at once.

use std::time::Duration;

use tokio::sync::watch;
use warrenguard_transport::drain_policy::{
    DRAINED_EXIT_AVOID_TTL, ExitDrainAdvisory, jitter_delay,
};

use crate::circuit_select::NodeSel;

/// What a refused dial asks of the session.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RefusalReaction {
    /// Dial the same exit through the entry at this directory index.
    RetargetEntry(usize),
    /// The refusing node is the exit: end the session so Kotlin fails over.
    LeaveExit,
    /// Nothing this session can act on: a refusal by a node it already left,
    /// or no other entry to dial. The supervisor keeps its backoff.
    Stay,
}

/// The circuit a session is dialing, and the entries it gave up on.
pub(crate) struct EntryRetarget {
    entry: usize,
    exit: usize,
    entry_country: Option<String>,
    /// Directory indices of refusing entries, with the unix second each was
    /// recorded; an entry is offered again once [`DRAINED_EXIT_AVOID_TTL`]
    /// has passed, since a drain ends in minutes.
    avoided: Vec<(usize, u64)>,
}

impl EntryRetarget {
    /// `entry == exit` is a one-hop circuit.
    pub(crate) fn new(entry: usize, exit: usize, entry_country: Option<&str>) -> Self {
        Self {
            entry,
            exit,
            entry_country: entry_country.map(str::to_owned),
            avoided: Vec::new(),
        }
    }

    /// The directory index of the entry the session dials now.
    pub(crate) fn entry(&self) -> usize {
        self.entry
    }

    /// React to a dial the node carrying `refused_relay_id` refused.
    pub(crate) fn on_refusal(
        &mut self,
        nodes: &[NodeSel<'_>],
        refused_relay_id: &[u8; 16],
        now_unix: u64,
    ) -> RefusalReaction {
        self.avoided
            .retain(|&(_, at)| now_unix.saturating_sub(at) < DRAINED_EXIT_AVOID_TTL.as_secs());
        // A dial already in flight to an entry this session left can still
        // come back refused; it says nothing about the circuit now dialed.
        if nodes.get(self.entry).map(|n| n.relay_id) != Some(refused_relay_id) {
            return RefusalReaction::Stay;
        }
        if self.entry == self.exit {
            return RefusalReaction::LeaveExit;
        }
        self.avoided.push((self.entry, now_unix));
        let exit_relay_id = nodes[self.exit].relay_id;
        let usable = |i: usize| {
            nodes[i].relay_id != exit_relay_id && !self.avoided.iter().any(|&(a, _)| a == i)
        };
        let country = self
            .entry_country
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty());
        let pick = country
            .and_then(|c| {
                (0..nodes.len()).find(|&i| usable(i) && nodes[i].country.eq_ignore_ascii_case(c))
            })
            .or_else(|| (0..nodes.len()).find(|&i| usable(i)));
        match pick {
            Some(entry) => {
                self.entry = entry;
                RefusalReaction::RetargetEntry(entry)
            }
            None => RefusalReaction::Stay,
        }
    }
}

/// Latch `leaving` once the exit announces a drain, after the anti-stampede
/// delay that spreads its clients before its deadline. `fraction` is the
/// client's uniform draw in `[0, 1)` (production passes
/// `drain_policy::stampede_fraction()`).
pub(crate) async fn leave_on_drain(
    mut drain: watch::Receiver<Option<ExitDrainAdvisory>>,
    leaving: watch::Sender<bool>,
    now_unix: impl Fn() -> u64,
    fraction: f64,
) {
    // An advisory published before this task first ran is still the
    // session's drain, so the current value is read before any wait.
    let advisory = loop {
        if let Some(advisory) = *drain.borrow_and_update() {
            break advisory;
        }
        if drain.changed().await.is_err() {
            return;
        }
    };
    let delay: Duration = jitter_delay(advisory.deadline_unix_secs, now_unix(), fraction);
    tokio::time::sleep(delay).await;
    let _ = leaving.send(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OwnedNode {
        exit_ed: [u8; 32],
        relay_id: [u8; 16],
        relay_ed: [u8; 32],
        country: String,
    }

    fn node(tag: u8, country: &str) -> OwnedNode {
        OwnedNode {
            exit_ed: [tag; 32],
            relay_id: [tag; 16],
            relay_ed: [tag.wrapping_add(1); 32],
            country: country.to_owned(),
        }
    }

    fn views(nodes: &[OwnedNode]) -> Vec<NodeSel<'_>> {
        nodes
            .iter()
            .map(|n| NodeSel {
                exit_ed25519: &n.exit_ed,
                relay_id: &n.relay_id,
                relay_ed25519: &n.relay_ed,
                country: &n.country,
            })
            .collect()
    }

    const NOW: u64 = 1_000_000;

    #[test]
    fn a_refusing_entry_is_replaced_for_the_same_exit() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            RefusalReaction::RetargetEntry(2),
            "the exit stays, so does the inner address it assigned"
        );
        assert_eq!(retarget.entry(), 2, "the next attempt dials the new entry");
    }

    #[test]
    fn a_refusing_one_hop_node_hands_the_exit_to_kotlin() {
        // On a one-hop circuit the node the connection terminates at is the
        // exit, and changing the exit is Kotlin's call.
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(1, 1, None);

        assert_eq!(
            retarget.on_refusal(&v, &[2; 16], NOW),
            RefusalReaction::LeaveExit
        );
    }

    #[test]
    fn a_refusal_from_a_node_the_session_left_changes_nothing() {
        // A dial that was already in flight to the old entry can still come
        // back refused after the retarget.
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "NL")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            RefusalReaction::RetargetEntry(2)
        );

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW + 1),
            RefusalReaction::Stay
        );
    }

    #[test]
    fn the_preferred_entry_country_is_kept_when_another_entry_there_can_serve() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, Some("de"));

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            RefusalReaction::RetargetEntry(3)
        );
    }

    #[test]
    fn a_refused_entry_is_offered_again_only_once_its_drain_can_be_over() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            RefusalReaction::RetargetEntry(2)
        );

        // The replacement refuses too, while the first one is still avoided.
        assert_eq!(
            retarget.on_refusal(&v, &[3; 16], NOW + 30),
            RefusalReaction::Stay
        );

        // The replacement's own refusal keeps it out; the first entry is back.
        let later = NOW + DRAINED_EXIT_AVOID_TTL.as_secs();
        let mut retarget = EntryRetarget::new(2, 1, None);
        retarget.avoided.push((0, NOW));
        assert_eq!(
            retarget.on_refusal(&v, &[3; 16], later),
            RefusalReaction::RetargetEntry(0)
        );
    }

    #[test]
    fn with_no_other_entry_the_session_stays_on_its_backoff() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            RefusalReaction::Stay
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_ends_the_session_within_the_exit_deadline() {
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, mut leaving) = watch::channel(false);
        let reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.5));

        drain_tx
            .send(Some(ExitDrainAdvisory {
                deadline_unix_secs: NOW + 45,
                reason_code: 0,
            }))
            .expect("reactor alive");
        let start = tokio::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(60), leaving.changed())
            .await
            .expect("the reactor must latch before the exit's deadline")
            .expect("the reactor holds the sender until it latches");

        assert!(*leaving.borrow());
        // Half of the 20 s spread: the herd is spread, and done before the
        // exit's deadline close.
        assert_eq!(start.elapsed(), Duration::from_secs(10));
        reactor.await.expect("the reactor returns once it latched");
    }

    #[tokio::test(start_paused = true)]
    async fn an_advisory_published_before_the_reactor_ran_still_ends_the_session() {
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, mut leaving) = watch::channel(false);
        drain_tx
            .send(Some(ExitDrainAdvisory {
                deadline_unix_secs: NOW + 2,
                reason_code: 0,
            }))
            .expect("receiver alive");

        let _reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.0));

        tokio::time::timeout(Duration::from_secs(60), leaving.changed())
            .await
            .expect("the reactor must latch on the advisory it found")
            .expect("the reactor holds the sender until it latches");
        assert!(*leaving.borrow());
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_whose_deadline_is_upon_the_session_leaves_at_once() {
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, mut leaving) = watch::channel(false);
        let _reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.9));

        drain_tx
            .send(Some(ExitDrainAdvisory {
                deadline_unix_secs: NOW + 2,
                reason_code: 0,
            }))
            .expect("reactor alive");
        let start = tokio::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(60), leaving.changed())
            .await
            .expect("the reactor must latch")
            .expect("the reactor holds the sender until it latches");

        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
