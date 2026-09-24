//! Moving an Android session off a node that refuses its dials or announces a
//! maintenance drain (ADR 36).
//!
//! A refused dial is charged to the node the connection terminates at: the
//! entry relay, which on a one-hop circuit is the exit itself. A refusal is
//! not authenticated (a close in the handshake can be forged on path, and a
//! peer holding only the cover certificate can refuse before the relay proves
//! its identity), so it may only ever move the session off an entry, for the
//! same exit: acted on for an exit, it would let whoever forges it choose the
//! user's exit by elimination. A one-hop circuit therefore stays on its
//! backoff when refused.
//!
//! The drain advisory is sealed by the exit's session, so it can move the
//! session off the exit. Android fixes the TUN address at `establish()` and
//! the exit is Kotlin's choice (location pin, key pins), so a drain ends the
//! session for Kotlin to fail over to another exit at once.

use std::time::Duration;

use tokio::sync::watch;
use warrenguard_transport::drain_policy::{DRAINED_EXIT_AVOID_TTL, ExitDrainNotice, jitter_delay};

use crate::circuit_select::NodeSel;

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

    /// The directory index of another entry to dial the same exit through
    /// after the node carrying `refused_relay_id` refused a dial, or `None`
    /// to stay on the supervisor's backoff.
    pub(crate) fn on_refusal(
        &mut self,
        nodes: &[NodeSel<'_>],
        refused_relay_id: &[u8; 16],
        now_unix: u64,
    ) -> Option<usize> {
        // Only a refusal naming the entry dialed now counts: a report naming
        // any other node says nothing about the circuit in use.
        if self.entry == self.exit || nodes.get(self.entry)?.relay_id != refused_relay_id {
            return None;
        }
        let refused = self.entry;
        self.avoided.retain(|&(i, at)| {
            i != refused && now_unix.saturating_sub(at) < DRAINED_EXIT_AVOID_TTL.as_secs()
        });
        self.avoided.push((refused, now_unix));
        let exit_relay_id = nodes[self.exit].relay_id;
        let usable = |i: usize| {
            nodes[i].relay_id != exit_relay_id && !self.avoided.iter().any(|&(a, _)| a == i)
        };
        let country = self
            .entry_country
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty());
        let entry = country
            .and_then(|c| {
                (0..nodes.len()).find(|&i| usable(i) && nodes[i].country.eq_ignore_ascii_case(c))
            })
            .or_else(|| (0..nodes.len()).find(|&i| usable(i)))?;
        self.entry = entry;
        Some(entry)
    }
}

/// Latch `leaving` once the exit announces a drain, after the anti-stampede
/// delay that spreads its clients before its deadline, when `another_exit`
/// says Kotlin has one to fail over to. `fraction` is the client's uniform
/// draw in `[0, 1)` (production passes `drain_policy::stampede_fraction()`).
pub(crate) async fn leave_on_drain(
    mut drain: watch::Receiver<Option<ExitDrainNotice>>,
    leaving: watch::Sender<bool>,
    now_unix: impl Fn() -> u64,
    fraction: f64,
    another_exit: bool,
) {
    // An advisory published before this task first ran is still the
    // session's drain, so the current value is read before any wait.
    let advisory = loop {
        if let Some(notice) = *drain.borrow_and_update() {
            break notice.advisory;
        }
        if drain.changed().await.is_err() {
            return;
        }
    };
    if !another_exit {
        // Ending the session would only redial the exit that refuses it. A
        // closed `leaving` channel never ends a session.
        log::info!("multi-hop exit draining and no other exit can serve; staying until its close");
        return;
    }
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

    /// The session's exit announcing a maintenance drain until `deadline`.
    fn drain_notice(deadline_unix_secs: u64) -> ExitDrainNotice {
        ExitDrainNotice {
            exit_id: warrenguard_multihop::ExitId::from_bytes([2; 16]),
            advisory: warrenguard_transport::drain_policy::ExitDrainAdvisory {
                deadline_unix_secs,
                reason_code: 0,
            },
        }
    }

    #[test]
    fn a_refusing_entry_is_replaced_for_the_same_exit() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW),
            Some(2),
            "the exit stays, so does the inner address it assigned"
        );
        assert_eq!(retarget.entry(), 2, "the next attempt dials the new entry");
    }

    #[test]
    fn a_refused_one_hop_circuit_stays_on_its_exit() {
        // On a one-hop circuit the refusing node is the exit. The refusal is
        // not authenticated, so acting on it would let whoever forges it
        // choose the exit by elimination.
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(1, 1, None);

        assert_eq!(retarget.on_refusal(&v, &[2; 16], NOW), None);
        assert_eq!(retarget.entry(), 1);
    }

    #[test]
    fn a_refusal_from_a_node_the_session_left_changes_nothing() {
        // A dial that was already in flight to the old entry can still come
        // back refused after the retarget.
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "NL")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW), Some(2));

        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW + 1), None);
    }

    #[test]
    fn the_preferred_entry_country_is_kept_when_another_entry_there_can_serve() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, Some("de"));

        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW), Some(3));
    }

    #[test]
    fn a_refused_entry_is_offered_again_only_once_its_drain_can_be_over() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW), Some(2));

        // The replacement refuses too, while the first one is still avoided.
        assert_eq!(retarget.on_refusal(&v, &[3; 16], NOW + 30), None);

        // Once the first refusal's window is over, that entry is back.
        let later = NOW + DRAINED_EXIT_AVOID_TTL.as_secs();
        assert_eq!(retarget.on_refusal(&v, &[3; 16], later), Some(0));
    }

    #[test]
    fn an_entry_that_keeps_refusing_is_recorded_once() {
        // With no other entry the session stays, and the same entry refuses
        // every redial of the backoff: the avoid list must not grow with them.
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        for second in 0..5 {
            assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW + second), None);
        }

        assert_eq!(retarget.avoided, vec![(0, NOW + 4)]);
    }

    #[test]
    fn with_no_other_entry_the_session_stays_on_its_backoff() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW), None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_ends_the_session_within_the_exit_deadline() {
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, mut leaving) = watch::channel(false);
        let reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.5, true));

        drain_tx
            .send(Some(drain_notice(NOW + 45)))
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
            .send(Some(drain_notice(NOW + 2)))
            .expect("receiver alive");

        let _reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.0, true));

        tokio::time::timeout(Duration::from_secs(60), leaving.changed())
            .await
            .expect("the reactor must latch on the advisory it found")
            .expect("the reactor holds the sender until it latches");
        assert!(*leaving.borrow());
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_with_no_exit_to_fail_over_to_keeps_the_session() {
        // A location pinned to a country with one node: ending the session
        // would only redial the draining node, which refuses until it
        // restarts. The session stays until the exit closes it.
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, leaving) = watch::channel(false);
        let reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.0, false));

        drain_tx
            .send(Some(drain_notice(NOW + 2)))
            .expect("reactor alive");
        tokio::time::sleep(Duration::from_secs(60)).await;

        assert!(!*leaving.borrow(), "the session must not be ended");
        drop(drain_tx);
        reactor.await.expect("the reactor ends with the session");
    }

    #[tokio::test(start_paused = true)]
    async fn a_drain_whose_deadline_is_upon_the_session_leaves_at_once() {
        let (drain_tx, drain) = watch::channel(None);
        let (leaving_tx, mut leaving) = watch::channel(false);
        let _reactor = tokio::spawn(leave_on_drain(drain, leaving_tx, || NOW, 0.9, true));

        drain_tx
            .send(Some(drain_notice(NOW + 2)))
            .expect("reactor alive");
        let start = tokio::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(60), leaving.changed())
            .await
            .expect("the reactor must latch")
            .expect("the reactor holds the sender until it latches");

        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
