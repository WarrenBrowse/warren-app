//! Per-leg delivery classification for the bonded QUIC carrier.
//!
//! # Why (half a tunnel can die without a single layer noticing)
//!
//! The bundle spreads the datapath over N QUIC connections. One of them can
//! stop carrying tunnel frames while the tunnel stays Connected and the
//! aggregate throughput merely drops, and nothing names the cause. Two ways it
//! happens, and two pieces of evidence:
//!
//! - The exit acknowledges every datagram at the QUIC layer and then drops the
//!   frames of a session it no longer holds, so a leg whose uplink delivers
//!   nothing keeps a climbing receive counter. Only the engine's path-health
//!   sweep sees it: it sends a probe on every leg, and a leg that returned
//!   neither probe is not getting its frames through
//!   ([`warrenguard_transport::path_health::LegHealth`]).
//! - A leg that keeps sending while literally nothing comes back, not even
//!   the exit's acknowledgements, has a one-way path. The probe cannot see it,
//!   because the exit answers a probe on whichever leg it picks, so each leg's
//!   `udp_tx`/`udp_rx` datagram counters are compared across one sampling
//!   interval.
//!
//! A leg is counted as not delivering when either says so. A received datagram
//! is never evidence that a leg delivers.
//!
//! This is an INDICATOR and never a guard. It takes no action, redials nothing,
//! and drops no leg (the engine already keeps flows off a leg that answers no
//! probe). Two properties bound what it can claim:
//!
//! - Exit-side idle cover (DAITA dummies) arrives as ordinary received
//!   datagrams, so the counter comparison UNDER-detects whenever cover is armed.
//! - The counters belong to a quinn `Connection`, so an overlap swap replaces
//!   them with a fresh set. That is handled by refusing to compare across a
//!   width change and by saturating every delta, which reads a restarted
//!   counter as "no traffic" rather than as a stall. The probe reading restarts
//!   with every bundle and says nothing until its first sweep of the new one.

use warrenguard_transport::path_health::LegHealth;

/// Datagram counters for one bonded leg, sampled from `quinn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegDatagrams {
    /// `udp_tx.datagrams`: datagrams the transport ISSUED on this leg. Climbs
    /// even when nothing reaches the wire, which is exactly what makes silence
    /// on `rx` meaningful.
    pub(crate) tx: u64,
    /// `udp_rx.datagrams`: datagrams that came back on this leg.
    pub(crate) rx: u64,
}

/// Sends a leg must have issued over one interval before a silent downlink
/// counts as evidence.
///
/// A QUIC peer answers ack-eliciting traffic within its `max_ack_delay`, so
/// over a 5 s interval eleven or more sends with literally zero datagrams back
/// is a one-way path, not a quiet one. Below that the leg is merely idle: the
/// router hands packets to the legs it prefers, and a leg that sent a keepalive
/// or two says nothing either way. The threshold is deliberately on the send
/// side, because it is the only counter that keeps climbing on a black-holed
/// path.
const MIN_TX_FOR_STALL: u64 = 10;

/// Indices of the legs that kept sending while receiving nothing back between
/// the two samples.
///
/// Empty when the bundle width changed between samples: the legs can no longer
/// be paired by index, and a fresh baseline has nothing to say about the
/// interval that just elapsed.
fn downlink_stalled_legs(previous: &[LegDatagrams], current: &[LegDatagrams]) -> Vec<usize> {
    if previous.len() != current.len() {
        return Vec::new();
    }
    previous
        .iter()
        .zip(current)
        .enumerate()
        .filter(|(_, (before, now))| {
            let tx = now.tx.saturating_sub(before.tx);
            let rx = now.rx.saturating_sub(before.rx);
            rx == 0 && tx > MIN_TX_FOR_STALL
        })
        .map(|(leg, _)| leg)
        .collect()
}

/// `(legs bonded, legs not delivering)` for the live bond: a leg does not
/// deliver when it returned neither probe of the latest sweep, or when it kept
/// sending while nothing came back over the interval.
///
/// `None` while the probe reading does not cover the bond being sampled (no
/// sweep of this bundle yet, or a leg came or went since the last one): the
/// counters alone cannot say that a leg delivers, so there is nothing to
/// publish until a sweep measured every leg.
#[must_use]
pub(crate) fn leg_counts(
    health: &LegHealth,
    previous: &[LegDatagrams],
    current: &[LegDatagrams],
) -> Option<(u8, u8)> {
    if health.legs == 0 || health.legs != current.len() {
        return None;
    }
    let stalled = downlink_stalled_legs(previous, current);
    let not_delivering = (0..current.len())
        .filter(|leg| health.unresponsive.contains(leg) || stalled.contains(leg))
        .count();
    Some((
        u8::try_from(current.len()).unwrap_or(u8::MAX),
        u8::try_from(not_delivering).unwrap_or(u8::MAX),
    ))
}

/// Two-interval confirmation for the published leg counts.
///
/// # Why
///
/// Publishing a count means republishing the tunnel metadata, which re-enters
/// the Connected transition; the daemon treats each of those as a new tunnel
/// state, re-fetches the exit location and re-notifies every front end. A count
/// that alternates between two values on consecutive ticks would do all of that
/// every five seconds, and would flicker the indicator chip on and off while it
/// was at it. So a value has to be observed on two ticks in a row before it is
/// published, at the cost of one interval of latency. The probe half of the
/// count moves at most once per path-health sweep (15 s by default), slower
/// than the tick, so the confirmation mostly filters the counter half.
#[derive(Debug, Default)]
pub(crate) struct DebouncedLegCounts {
    published: (u8, u8),
    candidate: Option<(u8, u8)>,
}

impl DebouncedLegCounts {
    /// Counts currently published: `(legs bonded, legs not delivering)`.
    #[must_use]
    pub(crate) fn published(&self) -> (u8, u8) {
        self.published
    }

    /// Feed one interval's observation, `None` for a tick with no reading
    /// ([`leg_counts`]). Returns the new counts when they are confirmed and
    /// differ from what is published, and `None` when there is nothing new to
    /// say. A tick with no reading keeps what is published and restarts the
    /// confirmation: the sightings on either side of it may describe two
    /// different bonds.
    pub(crate) fn observe(&mut self, counts: Option<(u8, u8)>) -> Option<(u8, u8)> {
        let Some(counts) = counts else {
            self.candidate = None;
            return None;
        };
        if counts == self.published {
            self.candidate = None;
            return None;
        }
        if self.candidate == Some(counts) {
            self.published = counts;
            self.candidate = None;
            return Some(counts);
        }
        self.candidate = Some(counts);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(tx: u64, rx: u64) -> LegDatagrams {
        LegDatagrams { tx, rx }
    }

    fn probed(legs: usize, unresponsive: &[usize]) -> LegHealth {
        let mut health = LegHealth::default();
        health.legs = legs;
        health.unresponsive = unresponsive.to_vec();
        health
    }

    #[test]
    fn a_leg_that_answers_no_probe_is_not_delivering_while_it_still_receives() {
        // The exit acknowledges every datagram at the QUIC layer and then drops
        // the frames of a session it no longer holds, so the dead leg's receive
        // counter climbs like everyone else's.
        let before = [leg(100, 100); 8];
        let after = [leg(208, 160); 8];
        assert_eq!(leg_counts(&probed(8, &[3]), &before, &after), Some((8, 1)));
    }

    #[test]
    fn a_leg_that_answered_its_probe_but_receives_nothing_back_is_not_delivering() {
        let before = [leg(100, 100); 4];
        let mut after = [leg(208, 160); 4];
        after[2] = leg(208, 100);
        assert_eq!(leg_counts(&probed(4, &[]), &before, &after), Some((4, 1)));
    }

    #[test]
    fn a_leg_that_fails_both_ways_is_counted_once() {
        let before = [leg(100, 100); 4];
        let mut after = [leg(208, 160); 4];
        after[1] = leg(208, 100);
        assert_eq!(
            leg_counts(&probed(4, &[1, 3]), &before, &after),
            Some((4, 2))
        );
    }

    #[test]
    fn a_bond_every_leg_of_which_answered_is_published_as_all_delivering() {
        let before = [leg(100, 100); 8];
        let after = [leg(208, 160); 8];
        assert_eq!(leg_counts(&probed(8, &[]), &before, &after), Some((8, 0)));
    }

    #[test]
    fn a_bond_no_sweep_has_measured_yet_publishes_nothing() {
        let before = [leg(100, 100); 8];
        let after = [leg(208, 160); 8];
        assert_eq!(
            leg_counts(&LegHealth::default(), &before, &after),
            None,
            "received datagrams alone must never be published as delivery"
        );
    }

    #[test]
    fn a_sweep_of_another_width_publishes_nothing() {
        // Indices are only meaningful for the bond the sweep measured: after a
        // leg came or went they name other legs.
        let before = [leg(100, 100); 7];
        let after = [leg(208, 160); 7];
        assert_eq!(leg_counts(&probed(8, &[6]), &before, &after), None);
    }

    #[test]
    fn a_leg_that_sends_and_receives_nothing_back_is_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(208, 100)];
        assert_eq!(downlink_stalled_legs(&before, &after), [0]);
    }

    #[test]
    fn a_leg_that_still_receives_is_not_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(208, 101)];
        assert!(
            downlink_stalled_legs(&before, &after).is_empty(),
            "a single datagram back proves the path carries something"
        );
    }

    #[test]
    fn an_idle_leg_is_not_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(110, 100)];
        assert!(
            downlink_stalled_legs(&before, &after).is_empty(),
            "a handful of sends with no reply is not evidence of a one-way path"
        );
    }

    #[test]
    fn each_stalled_leg_is_named_and_healthy_ones_are_not() {
        let before = [leg(0, 0), leg(0, 0), leg(0, 0), leg(0, 0)];
        let after = [leg(500, 0), leg(500, 480), leg(500, 0), leg(3, 0)];
        assert_eq!(downlink_stalled_legs(&before, &after), [0, 2]);
    }

    #[test]
    fn a_bundle_that_changed_width_reports_nothing_for_that_interval() {
        let before = [leg(0, 0), leg(0, 0)];
        let after = [leg(500, 0)];
        assert!(
            downlink_stalled_legs(&before, &after).is_empty(),
            "legs cannot be paired by index across a width change"
        );
    }

    #[test]
    fn a_leg_whose_counters_restarted_reads_as_idle_not_stalled() {
        // An overlap swap hands the same index a fresh quinn connection, whose
        // counters start from zero. Reading that as a huge negative delta and
        // then as a stall would flag every migration.
        let before = [leg(9_000, 8_000)];
        let after = [leg(12, 0)];
        assert!(downlink_stalled_legs(&before, &after).is_empty());
    }

    #[test]
    fn a_count_is_published_once_a_second_interval_confirms_it() {
        let mut counts = DebouncedLegCounts::default();
        assert_eq!(
            counts.observe(Some((8, 1))),
            None,
            "one interval is not evidence"
        );
        assert_eq!(counts.observe(Some((8, 1))), Some((8, 1)));
        assert_eq!(counts.published(), (8, 1));
        assert_eq!(
            counts.observe(Some((8, 1))),
            None,
            "an unchanged count must not be republished"
        );
    }

    #[test]
    fn an_alternating_count_is_never_published() {
        let mut counts = DebouncedLegCounts::default();
        for _ in 0..10 {
            assert_eq!(counts.observe(Some((8, 1))), None);
            assert_eq!(counts.observe(Some((8, 0))), None);
        }
        assert_eq!(
            counts.published(),
            (0, 0),
            "a value that never repeats must never reach the UI"
        );
    }

    #[test]
    fn a_tick_without_a_reading_breaks_the_confirmation() {
        // After an overlap swap the probe reading starts over, and a sighting
        // from the bond before the gap says nothing about the one after it.
        let mut counts = DebouncedLegCounts::default();
        assert_eq!(counts.observe(Some((8, 3))), None);
        assert_eq!(counts.observe(None), None);
        assert_eq!(
            counts.observe(Some((8, 3))),
            None,
            "a sighting before the gap must not count towards confirmation"
        );
        assert_eq!(counts.observe(Some((8, 3))), Some((8, 3)));
    }

    #[test]
    fn returning_to_the_published_value_drops_the_pending_candidate() {
        let mut counts = DebouncedLegCounts::default();
        counts.observe(Some((8, 0)));
        counts.observe(Some((8, 0)));
        assert_eq!(counts.published(), (8, 0));

        assert_eq!(counts.observe(Some((8, 2))), None);
        assert_eq!(
            counts.observe(Some((8, 0))),
            None,
            "back to what is published"
        );
        assert_eq!(
            counts.observe(Some((8, 2))),
            None,
            "the earlier sighting must not count towards confirmation"
        );
        assert_eq!(counts.observe(Some((8, 2))), Some((8, 2)));
    }
}
