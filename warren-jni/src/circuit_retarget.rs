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
    ///
    /// `reachable` keeps, in order, the candidates this host's network can
    /// route (production: [`crate::circuit_select::probe_reachable`]), so a
    /// refusal is never answered with an entry the network cannot reach. When
    /// the entry country has entries left and the network routes none of
    /// them, the answer is `None`, never an entry in another country: for an
    /// unroutable entry the supervisor then ends in `NoReachableEntry`, which
    /// the user is shown.
    pub(crate) fn on_refusal(
        &mut self,
        nodes: &[NodeSel<'_>],
        refused_relay_id: &[u8; 16],
        now_unix: u64,
        reachable: impl FnOnce(&[usize]) -> Vec<usize>,
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
        let distinct = |i: &usize| nodes[*i].relay_id != exit_relay_id;
        let country = self
            .entry_country
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty());
        // The country bounds the pool whenever it has a node that can front
        // the exit, avoided or not, as `circuit_select::entry_candidates`
        // decides it: a pinned country whose only entry refused is not left
        // for another country.
        let in_country: Vec<usize> = country
            .map(|c| {
                (0..nodes.len())
                    .filter(|i| distinct(i) && nodes[*i].country.eq_ignore_ascii_case(c))
                    .collect()
            })
            .unwrap_or_default();
        let pool: Vec<usize> = if in_country.is_empty() {
            (0..nodes.len()).filter(distinct).collect()
        } else {
            in_country
        };
        let candidates: Vec<usize> = pool
            .into_iter()
            .filter(|i| !self.avoided.iter().any(|&(a, _)| a == *i))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let entry = *reachable(&candidates).first()?;
        self.entry = entry;
        Some(entry)
    }
}

/// The circuit a dial takes: [`crate::circuit_select::select_circuit_indices`]
/// through the engine's kernel probe from `bind_addr` (the dial socket's
/// own bind and protect escape), with the families of the candidate entries
/// recorded for the problem report.
pub(crate) fn select_dial_circuit(
    dir: &warren_discovery_core::VerifiedMultiHopDirectory,
    want_exit: &[u8; 32],
    two_hop: bool,
    want_entry: Option<&[u8; 32]>,
    want_country: Option<&str>,
    bind_addr: std::net::SocketAddr,
) -> Result<(usize, usize), crate::circuit_select::CircuitSelectError> {
    crate::circuit_select::select_circuit_indices(
        &node_views(dir),
        want_exit,
        two_hop,
        want_entry,
        want_country,
        |candidates| {
            crate::dial_facts::record_entry_families(crate::entry_families::families_of(
                candidates.iter().map(|&i| {
                    let relay = &dir.nodes[i].relay;
                    (&relay.endpoint, relay.endpoint_v6.as_ref())
                }),
            ));
            crate::circuit_select::probe_reachable(|i| &dir.nodes[i].relay, candidates, bind_addr)
        },
    )
}

/// The per-node fields circuit selection reads, borrowed from the verified
/// directory.
pub(crate) fn node_views(
    dir: &warren_discovery_core::VerifiedMultiHopDirectory,
) -> Vec<crate::circuit_select::NodeSel<'_>> {
    dir.nodes
        .iter()
        .map(|n| crate::circuit_select::NodeSel {
            exit_ed25519: &n.exit.exit_ed25519_pubkey,
            relay_id: &n.relay.relay_id,
            relay_ed25519: &n.relay.relay_ed25519_pubkey,
            country: &n.country,
        })
        .collect()
}

/// The supervisor's dial-refusal observer for one attempt.
///
/// The refusal is charged to the node the connection terminates at,
/// `relay_id`, whatever hop the engine reports, and it only ever moves the
/// session to another entry for the same exit (see `crate::circuit_retarget`
/// for why a refusal never moves the exit). The entry it moves to is one this
/// network routes: the engine reports an entry it cannot route through this
/// same observer, and ends in `NoReachableEntry` unless the observer names
/// one it can, from inside the call.
pub(crate) fn refusal_hook(
    dir: std::sync::Arc<warren_discovery_core::VerifiedMultiHopDirectory>,
    retarget: std::sync::Arc<parking_lot::Mutex<crate::circuit_retarget::EntryRetarget>>,
    migrate: std::sync::Arc<std::sync::OnceLock<warrenguard_transport::supervisor::MigrateHandle>>,
    exit_idx: usize,
    exit_mlkem768_pubkey: Option<Vec<u8>>,
    bind_addr: std::net::SocketAddr,
) -> warrenguard_transport::supervisor::DialRefusedObserver {
    std::sync::Arc::new(move |_hop, relay_id: [u8; 16], _exit_id| {
        // Probed once, before the retarget lock: each probe socket takes a
        // `VpnService.protect` upcall into the JVM, which must not run under
        // the lock the supervisor's observer holds.
        let every_node: Vec<usize> = (0..dir.nodes.len()).collect();
        let routed =
            crate::circuit_select::probe_reachable(|i| &dir.nodes[i].relay, &every_node, bind_addr);
        if let Some(refused) = dir.nodes.iter().position(|n| n.relay.relay_id == relay_id) {
            crate::dial_facts::record_dial_error(crate::dial_facts::refusal_class(
                routed.contains(&refused),
            ));
        }
        let reachable = |candidates: &[usize]| {
            candidates
                .iter()
                .copied()
                .filter(|i| routed.contains(i))
                .collect()
        };
        let Some(entry) =
            retarget
                .lock()
                .on_refusal(&node_views(&dir), &relay_id, unix_now(), reachable)
        else {
            return;
        };
        // No-logs: the category of the reaction, never the node.
        log::info!("multi-hop entry refused the dial; dialing the exit through another");
        let exit = &dir.nodes[exit_idx].exit;
        if let Some(handle) = migrate.get() {
            handle.migrate_to(warrenguard_transport::supervisor::CircuitTarget {
                relay: std::sync::Arc::new(dir.nodes[entry].relay.clone()),
                exit_id: exit.exit_id,
                exit_x25519_multihop_pubkey: exit.exit_x25519_multihop_pubkey,
                exit_mlkem768_pubkey: exit_mlkem768_pubkey.clone(),
            });
        }
    })
}

/// Seconds since the Unix epoch, 0 on a clock before it.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

    /// A signed directory whose first two nodes publish IPv6 only and the
    /// others a loopback IPv4 address. Dialed from the pinned IPv4 bind
    /// `127.0.0.1:0`, which reaches no IPv6 address, the first two stand for
    /// v4-only entries seen from an IPv6-only network (the other way round),
    /// on any test host and without touching its routes.
    fn two_unroutable_nodes() -> warren_discovery_core::VerifiedMultiHopDirectory {
        use warren_app_routes::plan::fixture::{directory, test_node};
        let node = |tag: u8, country: &str, endpoint: &str| {
            let mut node = test_node(tag, country, "City", 10);
            node.relay.endpoint = endpoint.parse().expect("static addr");
            node
        };
        directory(vec![
            node(1, "ro", "[2001:db8::1]:443"),
            node(2, "fr", "[2001:db8::2]:443"),
            node(3, "de", "127.0.0.1:9"),
            node(4, "nl", "127.0.0.1:10"),
        ])
    }

    const PINNED_V4: &str = "127.0.0.1:0";

    #[test]
    fn a_dial_enters_through_the_first_candidate_the_kernel_routes() {
        let dir = two_unroutable_nodes();
        let exit = dir.nodes[3].exit.exit_ed25519_pubkey;

        let got = select_dial_circuit(
            &dir,
            &exit,
            true,
            None,
            None,
            PINNED_V4.parse().expect("static addr"),
        );

        assert_eq!(got, Ok((2, 3)), "the IPv6-only nodes are passed over");
    }

    #[test]
    fn a_refusal_from_an_entry_the_kernel_cannot_route_moves_to_one_it_can() {
        // The engine reports an unroutable entry through the refusal hook;
        // the hook must answer with an entry this host routes.
        let dir = std::sync::Arc::new(two_unroutable_nodes());
        let retarget = std::sync::Arc::new(parking_lot::Mutex::new(EntryRetarget::new(0, 3, None)));
        let hook = refusal_hook(
            std::sync::Arc::clone(&dir),
            std::sync::Arc::clone(&retarget),
            std::sync::Arc::new(std::sync::OnceLock::new()),
            3,
            None,
            PINNED_V4.parse().expect("static addr"),
        );

        hook(
            warrenguard_transport::multihop::DialRefusedHop::Entry,
            dir.nodes[0].relay.relay_id,
            *dir.nodes[3].exit.exit_id.as_bytes(),
        );

        assert_eq!(retarget.lock().entry(), 2, "FR is IPv6-only too");
    }

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

    /// A network that routes every entry.
    fn all_reachable(candidates: &[usize]) -> Vec<usize> {
        candidates.to_vec()
    }

    #[test]
    fn a_refusal_is_answered_with_an_entry_this_network_routes() {
        // Forum topic 210: after the v4-only entry proves unroutable, the
        // next entry in directory order is v4-only too; the retarget passes
        // over it rather than feeding the supervisor a second doomed dial.
        let nodes = [node(1, "RO"), node(2, "FI"), node(3, "FR"), node(4, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        let got = retarget.on_refusal(&v, &[1; 16], NOW, |candidates| {
            candidates.iter().copied().filter(|&i| i != 2).collect()
        });

        assert_eq!(got, Some(3));
    }

    #[test]
    fn a_pinned_country_whose_only_entry_refused_is_not_left_for_another() {
        // One node per country is the live fleet's shape: the refused FR
        // entry is avoided, and DE must not take its place.
        let nodes = [node(1, "FR"), node(2, "FI"), node(3, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, Some("fr"));

        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW, all_reachable), None);
        assert_eq!(retarget.entry(), 0);
    }

    #[test]
    fn a_pinned_country_the_network_cannot_route_is_not_left_for_another() {
        let nodes = [node(1, "FR"), node(2, "FI"), node(3, "FR"), node(4, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, Some("fr"));

        let got = retarget.on_refusal(&v, &[1; 16], NOW, |candidates| {
            assert_eq!(candidates, &[2], "only the pinned country is offered");
            Vec::new()
        });

        assert_eq!(got, None);
        assert_eq!(retarget.entry(), 0);
    }

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
            retarget.on_refusal(&v, &[1; 16], NOW, all_reachable),
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

        assert_eq!(retarget.on_refusal(&v, &[2; 16], NOW, all_reachable), None);
        assert_eq!(retarget.entry(), 1);
    }

    #[test]
    fn a_refusal_from_a_node_the_session_left_changes_nothing() {
        // A dial that was already in flight to the old entry can still come
        // back refused after the retarget.
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "NL")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW, all_reachable),
            Some(2)
        );

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW + 1, all_reachable),
            None
        );
    }

    #[test]
    fn the_preferred_entry_country_is_kept_when_another_entry_there_can_serve() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE"), node(4, "DE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, Some("de"));

        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW, all_reachable),
            Some(3)
        );
    }

    #[test]
    fn a_refused_entry_is_offered_again_only_once_its_drain_can_be_over() {
        let nodes = [node(1, "DE"), node(2, "FR"), node(3, "SE")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);
        assert_eq!(
            retarget.on_refusal(&v, &[1; 16], NOW, all_reachable),
            Some(2)
        );

        // The replacement refuses too, while the first one is still avoided.
        assert_eq!(
            retarget.on_refusal(&v, &[3; 16], NOW + 30, all_reachable),
            None
        );

        // Once the first refusal's window is over, that entry is back.
        let later = NOW + DRAINED_EXIT_AVOID_TTL.as_secs();
        assert_eq!(
            retarget.on_refusal(&v, &[3; 16], later, all_reachable),
            Some(0)
        );
    }

    #[test]
    fn an_entry_that_keeps_refusing_is_recorded_once() {
        // With no other entry the session stays, and the same entry refuses
        // every redial of the backoff: the avoid list must not grow with them.
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        for second in 0..5 {
            assert_eq!(
                retarget.on_refusal(&v, &[1; 16], NOW + second, all_reachable),
                None
            );
        }

        assert_eq!(retarget.avoided, vec![(0, NOW + 4)]);
    }

    #[test]
    fn with_no_other_entry_the_session_stays_on_its_backoff() {
        let nodes = [node(1, "DE"), node(2, "FR")];
        let v = views(&nodes);
        let mut retarget = EntryRetarget::new(0, 1, None);

        assert_eq!(retarget.on_refusal(&v, &[1; 16], NOW, all_reachable), None);
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
