//! Why an exit refused a forwarded port as not authorized, and when to ask
//! again (warren-core doc 105: an exit refuses a NAT-PMP Map request that
//! carries no valid entitlement envelope, with RFC 6886 result code 2).
//!
//! Every client that forwards ports reads the refusal the same way: the
//! desktop tunnel controller, and the mobile clients that drive the engine's
//! refresh loop themselves.

/// What a refused rule had presented, which is what the refusal means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortRefusal {
    /// The rule had no entitlement to present: none could be minted yet (the
    /// API unreachable, the first mint still landing), the issuer served this
    /// epoch's batch to another device of the wallet, or the wallet is banned.
    NoEntitlement,
    /// The rule had no entitlement to present because the issuer refused this
    /// device's clock: it is further off the servers' than the correction the
    /// SDK applies can cover. Only the device's own time settings fix it.
    ClockSkew {
        /// The servers' clock minus this device's, in seconds (positive when
        /// the device is behind), when the refusal said.
        offset_secs: Option<i64>,
    },
    /// The exit refused the entitlement the rule presented. Usually transient:
    /// the serial is still held by this client's previous tunnel address
    /// until the exit reaps it, or it belongs to the epoch that just ended.
    EntitlementRefused,
}

impl PortRefusal {
    /// Seconds to wait before asking again after the `attempt`-th refusal in
    /// a row (from 1). A refused entitlement usually heals within a cycle, so
    /// it is asked for again soon; a missing one comes back with the next mint
    /// or the next epoch, so it is asked for on the refresh cadence, starting
    /// short because the first mint of a wallet can still be in flight.
    #[must_use]
    pub fn retry_after_secs(self, attempt: u32) -> u32 {
        const REFUSED: [u32; 6] = [2, 10, 30, 60, 120, 300];
        const MISSING: [u32; 6] = [5, 30, 60, 120, 300, 600];
        let table: &[u32] = match self {
            Self::EntitlementRefused => &REFUSED,
            Self::NoEntitlement | Self::ClockSkew { .. } => &MISSING,
        };
        let index = usize::try_from(attempt.saturating_sub(1)).unwrap_or(usize::MAX);
        table.get(index).or(table.last()).copied().unwrap_or(600)
    }
}

/// Refusals in a row since the last grant, for one rule: what the next
/// refusal means and how long to wait before asking again.
#[derive(Debug, Default)]
pub struct RefusalCount {
    refusals: u32,
}

impl RefusalCount {
    /// One more refusal, meaning `refusal`: how long to wait before asking
    /// again.
    pub fn on_refused(&mut self, refusal: PortRefusal) -> u32 {
        self.refusals = self.refusals.saturating_add(1);
        refusal.retry_after_secs(self.refusals)
    }

    /// The port was granted: the next refusal starts the waits over.
    pub fn on_granted(&mut self) {
        self.refusals = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_without_an_entitlement_is_counted_on_the_mint_cadence() {
        let mut count = RefusalCount::default();

        assert_eq!(count.on_refused(PortRefusal::NoEntitlement), 5);
        assert_eq!(count.on_refused(PortRefusal::NoEntitlement), 30);
    }

    #[test]
    fn a_grant_starts_the_waits_over() {
        let mut count = RefusalCount::default();
        count.on_refused(PortRefusal::EntitlementRefused);
        count.on_refused(PortRefusal::EntitlementRefused);

        count.on_granted();

        assert_eq!(count.on_refused(PortRefusal::EntitlementRefused), 2);
    }

    #[test]
    fn a_refused_entitlement_is_asked_again_soon_then_less_often() {
        let retry = |n| PortRefusal::EntitlementRefused.retry_after_secs(n);
        assert_eq!(retry(1), 2);
        assert!(retry(2) > retry(1));
        assert_eq!(retry(99), 300, "the wait stops growing");
    }

    #[test]
    fn a_missing_entitlement_is_asked_again_soon_then_on_the_mint_cadence() {
        let retry = |n| PortRefusal::NoEntitlement.retry_after_secs(n);
        assert_eq!(retry(1), 5, "the first mint may still be landing");
        assert_eq!(retry(99), 600, "the wait stops growing");
    }

    /// A clock set right comes back with the next mint, on the same cadence
    /// as a missing entitlement: asking sooner only meets the same refusal.
    #[test]
    fn a_clock_refusal_is_asked_again_on_the_mint_cadence() {
        let clock = PortRefusal::ClockSkew {
            offset_secs: Some(-91),
        };
        for attempt in [1, 2, 3, 99] {
            assert_eq!(
                clock.retry_after_secs(attempt),
                PortRefusal::NoEntitlement.retry_after_secs(attempt)
            );
        }
    }
}
