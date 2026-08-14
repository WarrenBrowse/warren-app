//! Per-leg downlink-stall classification for the bonded QUIC carrier.
//!
//! # Why (half a tunnel can die without a single layer noticing)
//!
//! The bundle spreads the datapath over N QUIC connections. When one of them
//! keeps sending but stops receiving, the tunnel stays Connected, the aggregate
//! throughput merely drops, and nothing names the cause: the incident that
//! produced this module was read off `dg_tx=108 dg_rx=1` in a DEBUG log, days
//! later. Comparing each leg's `udp_tx`/`udp_rx` datagram counters across one
//! sampling interval turns that into a number the UI and the CLI can show.
//!
//! This is an INDICATOR and never a guard. It takes no action, redials nothing,
//! and drops no leg. Two properties bound what it can claim:
//!
//! - Exit-side idle cover (DAITA dummies) arrives as ordinary received
//!   datagrams, so a leg under cover keeps a non-zero rx delta even when it
//!   carries no user traffic. The check therefore UNDER-detects whenever cover
//!   is armed, and a zero here is never proof of health.
//! - The counters belong to a quinn `Connection`, so an overlap swap replaces
//!   them with a fresh set. That is handled by refusing to compare across a
//!   width change and by saturating every delta, which reads a restarted
//!   counter as "no traffic" rather than as a stall.

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

/// How many legs stopped receiving while still sending, between the two
/// samples.
///
/// Returns `0` when the bundle width changed between samples: the legs can no
/// longer be paired by index, and a fresh baseline has nothing to say about the
/// interval that just elapsed.
#[must_use]
pub(crate) fn count_downlink_stalled(previous: &[LegDatagrams], current: &[LegDatagrams]) -> u8 {
    if previous.len() != current.len() {
        return 0;
    }
    let stalled = previous
        .iter()
        .zip(current)
        .filter(|(before, now)| {
            let tx = now.tx.saturating_sub(before.tx);
            let rx = now.rx.saturating_sub(before.rx);
            rx == 0 && tx > MIN_TX_FOR_STALL
        })
        .count();
    u8::try_from(stalled).unwrap_or(u8::MAX)
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
/// was at it. So a value has to be observed twice in a row before it is
/// published, at the cost of one interval of latency.
#[derive(Debug, Default)]
pub(crate) struct DebouncedLegCounts {
    published: (u8, u8),
    candidate: Option<(u8, u8)>,
}

impl DebouncedLegCounts {
    /// Counts currently published: `(legs bonded, legs downlink-stalled)`.
    #[must_use]
    pub(crate) fn published(&self) -> (u8, u8) {
        self.published
    }

    /// Feed one interval's observation. Returns the new counts when they are
    /// confirmed and differ from what is published, and `None` when there is
    /// nothing new to say.
    pub(crate) fn observe(&mut self, counts: (u8, u8)) -> Option<(u8, u8)> {
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

    #[test]
    fn a_leg_that_sends_and_receives_nothing_back_is_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(208, 100)];
        assert_eq!(count_downlink_stalled(&before, &after), 1);
    }

    #[test]
    fn a_leg_that_still_receives_is_not_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(208, 101)];
        assert_eq!(
            count_downlink_stalled(&before, &after),
            0,
            "a single datagram back proves the path carries something"
        );
    }

    #[test]
    fn an_idle_leg_is_not_stalled() {
        let before = [leg(100, 100)];
        let after = [leg(110, 100)];
        assert_eq!(
            count_downlink_stalled(&before, &after),
            0,
            "a handful of sends with no reply is not evidence of a one-way path"
        );
    }

    #[test]
    fn each_stalled_leg_is_counted_and_healthy_ones_are_not() {
        let before = [leg(0, 0), leg(0, 0), leg(0, 0), leg(0, 0)];
        let after = [leg(500, 0), leg(500, 480), leg(500, 0), leg(3, 0)];
        assert_eq!(count_downlink_stalled(&before, &after), 2);
    }

    #[test]
    fn a_bundle_that_changed_width_reports_nothing_for_that_interval() {
        let before = [leg(0, 0), leg(0, 0)];
        let after = [leg(500, 0)];
        assert_eq!(
            count_downlink_stalled(&before, &after),
            0,
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
        assert_eq!(count_downlink_stalled(&before, &after), 0);
    }

    #[test]
    fn a_count_is_published_once_a_second_interval_confirms_it() {
        let mut counts = DebouncedLegCounts::default();
        assert_eq!(counts.observe((8, 1)), None, "one interval is not evidence");
        assert_eq!(counts.observe((8, 1)), Some((8, 1)));
        assert_eq!(counts.published(), (8, 1));
        assert_eq!(
            counts.observe((8, 1)),
            None,
            "an unchanged count must not be republished"
        );
    }

    #[test]
    fn an_alternating_count_is_never_published() {
        let mut counts = DebouncedLegCounts::default();
        for _ in 0..10 {
            assert_eq!(counts.observe((8, 1)), None);
            assert_eq!(counts.observe((8, 0)), None);
        }
        assert_eq!(
            counts.published(),
            (0, 0),
            "a value that never repeats must never reach the UI"
        );
    }

    #[test]
    fn returning_to_the_published_value_drops_the_pending_candidate() {
        let mut counts = DebouncedLegCounts::default();
        counts.observe((8, 0));
        counts.observe((8, 0));
        assert_eq!(counts.published(), (8, 0));

        assert_eq!(counts.observe((8, 2)), None);
        assert_eq!(counts.observe((8, 0)), None, "back to what is published");
        assert_eq!(
            counts.observe((8, 2)),
            None,
            "the earlier sighting must not count towards confirmation"
        );
        assert_eq!(counts.observe((8, 2)), Some((8, 2)));
    }
}
