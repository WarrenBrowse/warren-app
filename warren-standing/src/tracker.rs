//! One wallet's standing, merged from the standing endpoint and the issuers'
//! ban refusals.

use serde::Serialize;
use warren_api::{AccountStandingResponse, AccountStrike};

use crate::{Ban, Standing, StrikeLedger};

/// A strike this device has not warned about yet, with what the warning says
/// about it ("warning 2 of 3").
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct NewStrike {
    /// The strike.
    pub strike: AccountStrike,
    /// Its rank among the live strikes, oldest first, from 1.
    pub ordinal: u32,
    /// Live strikes that trigger a ban.
    pub threshold: u32,
}

impl std::fmt::Debug for NewStrike {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewStrike")
            .field("ordinal", &self.ordinal)
            .field("threshold", &self.threshold)
            .finish_non_exhaustive()
    }
}

/// What changed, for the client to publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandingUpdate {
    /// The standing now known, `None` when nothing is (no wallet).
    pub standing: Option<Standing>,
    /// Strikes to warn about, each exactly once per device.
    pub new_strikes: Vec<NewStrike>,
}

/// Holds the current wallet's standing and the device's strike ledger.
///
/// The wallet key is whatever stable handle the client has for its wallet
/// (the SS58 address); it is compared, never logged. A different key starts
/// from nothing, so one wallet's ban never blocks the next one.
#[derive(Debug, Default)]
pub struct StandingTracker {
    ledger: StrikeLedger,
    wallet: Option<String>,
    standing: Option<Standing>,
}

impl StandingTracker {
    /// A tracker that knows nothing yet and remembers `ledger`'s strikes as
    /// announced.
    #[must_use]
    pub fn new(ledger: StrikeLedger) -> Self {
        Self {
            ledger,
            wallet: None,
            standing: None,
        }
    }

    /// The standing endpoint answered for `wallet`. The answer is the whole
    /// truth, ban included, so it replaces whatever was known: this is how a
    /// lifted or lapsed ban clears.
    pub fn on_standing(
        &mut self,
        wallet: &str,
        response: AccountStandingResponse,
    ) -> Option<StandingUpdate> {
        self.switch_to(wallet);
        let standing = Standing::from(response);
        let new_strikes = self
            .ledger
            .take_new(&standing.strikes)
            .into_iter()
            .map(|strike| NewStrike {
                ordinal: ordinal_of(&standing.strikes, &strike),
                threshold: standing.threshold,
                strike,
            })
            .collect::<Vec<_>>();
        self.publish(Some(standing), new_strikes)
    }

    /// An issuer refused `wallet` as banned. Known before any standing
    /// answer, and it does not override one that already carries a ban: that
    /// one also knows when the ban took effect.
    pub fn on_issuance_ban(&mut self, wallet: &str, ban: Ban) -> Option<StandingUpdate> {
        self.switch_to(wallet);
        let standing = match self.standing.clone() {
            Some(standing) if standing.ban.is_some() => standing,
            Some(standing) => Standing {
                ban: Some(ban),
                ..standing
            },
            None => Standing::ban_only(ban),
        };
        self.publish(Some(standing), Vec::new())
    }

    /// The client holds no wallet any more (logged out).
    pub fn on_no_wallet(&mut self) -> Option<StandingUpdate> {
        self.wallet = None;
        self.publish(None, Vec::new())
    }

    /// The standing currently known.
    #[must_use]
    pub fn standing(&self) -> Option<&Standing> {
        self.standing.as_ref()
    }

    /// The ban that holds at `now_unix_secs`, if any.
    #[must_use]
    pub fn ban_in_force(&self, now_unix_secs: u64) -> Option<Ban> {
        self.standing
            .as_ref()
            .and_then(|standing| standing.ban)
            .filter(|ban| ban.in_force(now_unix_secs))
    }

    /// The strike ledger, for the client to persist after an update.
    #[must_use]
    pub fn ledger(&self) -> &StrikeLedger {
        &self.ledger
    }

    fn switch_to(&mut self, wallet: &str) {
        if self.wallet.as_deref() != Some(wallet) {
            self.wallet = Some(wallet.to_owned());
            self.standing = None;
        }
    }

    fn publish(
        &mut self,
        standing: Option<Standing>,
        new_strikes: Vec<NewStrike>,
    ) -> Option<StandingUpdate> {
        if standing == self.standing && new_strikes.is_empty() {
            return None;
        }
        self.standing.clone_from(&standing);
        Some(StandingUpdate {
            standing,
            new_strikes,
        })
    }
}

fn ordinal_of(live: &[AccountStrike], strike: &AccountStrike) -> u32 {
    let rank = live
        .iter()
        .position(|s| s.case_reference == strike.case_reference)
        .map_or(live.len(), |index| index + 1);
    u32::try_from(rank).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use warren_api::{AccountBan, AccountStandingResponse, BanReasonCode};

    use super::*;
    use crate::test_support::strike;

    const WALLET: &str = "wallet-a";

    fn answer(strikes: &[(&str, u16)], ban: Option<AccountBan>) -> AccountStandingResponse {
        AccountStandingResponse {
            strikes: strikes
                .iter()
                .enumerate()
                .map(|(day, (reference, port))| strike(reference, *port, 86_400 * (day as u64 + 1)))
                .collect(),
            threshold: 3,
            window_days: 90,
            ban,
        }
    }

    fn account_ban() -> AccountBan {
        AccountBan {
            banned_at_unix_secs: 5_000,
            lapses_at_unix_secs: Some(9_000),
            reason_code: BanReasonCode::PortForwardingAbuse,
        }
    }

    fn issuance_ban() -> Ban {
        Ban {
            reason: BanReasonCode::PortForwardingAbuse,
            banned_at_unix_secs: None,
            lapses_at_unix_secs: Some(9_000),
        }
    }

    fn warned(update: &StandingUpdate) -> Vec<(&str, u32, u32)> {
        update
            .new_strikes
            .iter()
            .map(|n| (n.strike.case_reference.as_str(), n.ordinal, n.threshold))
            .collect()
    }

    #[test]
    fn a_first_answer_warns_about_every_live_strike_with_its_rank() {
        let mut tracker = StandingTracker::default();

        let update = tracker
            .on_standing(WALLET, answer(&[("PF-1", 50000), ("PF-2", 50001)], None))
            .expect("a first answer is news");

        assert_eq!(warned(&update), [("PF-1", 1, 3), ("PF-2", 2, 3)]);
        assert_eq!(update.standing.map(|s| s.strikes.len()), Some(2));
    }

    #[test]
    fn the_same_answer_again_is_not_news() {
        let mut tracker = StandingTracker::default();
        tracker.on_standing(WALLET, answer(&[("PF-1", 50000)], None));

        assert_eq!(
            tracker.on_standing(WALLET, answer(&[("PF-1", 50000)], None)),
            None
        );
    }

    #[test]
    fn a_new_strike_is_warned_about_once_with_its_rank() {
        let mut tracker = StandingTracker::default();
        tracker.on_standing(WALLET, answer(&[("PF-1", 50000)], None));

        let update = tracker
            .on_standing(WALLET, answer(&[("PF-1", 50000), ("PF-2", 50001)], None))
            .expect("a new strike is news");

        assert_eq!(warned(&update), [("PF-2", 2, 3)]);
    }

    #[test]
    fn a_restarted_client_does_not_warn_twice() {
        let mut first = StandingTracker::default();
        first.on_standing(WALLET, answer(&[("PF-1", 50000)], None));
        let persisted = first.ledger().to_json();

        let mut restarted = StandingTracker::new(StrikeLedger::from_json(&persisted).unwrap());
        let update = restarted
            .on_standing(WALLET, answer(&[("PF-1", 50000)], None))
            .expect("the standing itself is news to a fresh process");

        assert!(update.new_strikes.is_empty());
    }

    #[test]
    fn an_issuance_ban_is_known_before_any_standing_answer() {
        let mut tracker = StandingTracker::default();

        let update = tracker
            .on_issuance_ban(WALLET, issuance_ban())
            .expect("a ban is news");

        assert_eq!(update.standing, Some(Standing::ban_only(issuance_ban())));
        assert_eq!(tracker.ban_in_force(8_999), Some(issuance_ban()));
    }

    #[test]
    fn an_issuance_ban_keeps_the_richer_ban_the_standing_answered() {
        let mut tracker = StandingTracker::default();
        tracker.on_standing(WALLET, answer(&[], Some(account_ban())));

        assert_eq!(tracker.on_issuance_ban(WALLET, issuance_ban()), None);
        assert_eq!(
            tracker.ban_in_force(0).and_then(|b| b.banned_at_unix_secs),
            Some(5_000)
        );
    }

    #[test]
    fn an_issuance_ban_joins_the_strikes_already_known() {
        let mut tracker = StandingTracker::default();
        tracker.on_standing(WALLET, answer(&[("PF-1", 50000)], None));

        tracker.on_issuance_ban(WALLET, issuance_ban());

        let standing = tracker.standing().expect("known");
        assert_eq!(standing.strikes.len(), 1);
        assert_eq!(standing.ban, Some(issuance_ban()));
    }

    #[test]
    fn a_lifted_ban_clears_on_the_next_answer() {
        let mut tracker = StandingTracker::default();
        tracker.on_issuance_ban(WALLET, issuance_ban());

        let update = tracker
            .on_standing(WALLET, answer(&[], None))
            .expect("the lift is news");

        assert_eq!(update.standing.and_then(|s| s.ban), None);
        assert_eq!(tracker.ban_in_force(0), None);
    }

    #[test]
    fn a_lapsed_ban_is_not_in_force() {
        let mut tracker = StandingTracker::default();
        tracker.on_issuance_ban(WALLET, issuance_ban());

        assert_eq!(tracker.ban_in_force(9_000), None);
    }

    #[test]
    fn another_wallet_starts_from_nothing() {
        let mut tracker = StandingTracker::default();
        tracker.on_standing(WALLET, answer(&[("PF-1", 50000)], Some(account_ban())));

        let update = tracker
            .on_issuance_ban("wallet-b", issuance_ban())
            .expect("another wallet is news");

        assert_eq!(update.standing, Some(Standing::ban_only(issuance_ban())));
    }

    #[test]
    fn logging_out_forgets_the_standing() {
        let mut tracker = StandingTracker::default();
        tracker.on_issuance_ban(WALLET, issuance_ban());

        let update = tracker.on_no_wallet().expect("forgetting is news");

        assert_eq!(update.standing, None);
        assert_eq!(tracker.ban_in_force(0), None);
        assert_eq!(tracker.on_no_wallet(), None);
    }
}
