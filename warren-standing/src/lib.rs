//! Port-forward abuse standing of a Warren wallet, as the clients see it
//! (warren-core doc 105 §5.3, §5.4).
//!
//! Two sources tell a client about its standing. `GET /v1/account/standing`
//! answers the live strikes and the ban in force, and the token and
//! entitlement issuers refuse a banned wallet with a typed error. The second
//! one matters because it arrives on its own, from the background refresh,
//! before any exit is dialed: a v7 client never presents its wallet to an
//! exit, so the exit's ban rejection no longer reaches it.
//!
//! [`StandingTracker`] merges both into one [`Standing`] and says which
//! strikes this device has not warned about yet. [`Ban::auth_failed_reason`]
//! names the tunnel error the ban blocks with, in the `[TOKEN]` form the apps
//! already localise.
//!
//! A strike carries the port that was closed and the case reference to quote
//! when contesting it. Both belong on the account's own screen and nowhere
//! else: the types here render neither through `Debug`, and the ledger that
//! remembers which strikes were announced stores only a digest of each case
//! reference.

mod ledger;
mod port_refusal;
mod tracker;

pub use ledger::{LedgerError, StrikeLedger};
pub use port_refusal::PortRefusal;
pub use tracker::{NewStrike, StandingTracker, StandingUpdate};
pub use warren_api::{AbuseCategory, AccountStrike, BanReasonCode};

use serde::{Deserialize, Serialize};
use warren_api::{AccountBan, AccountStandingResponse, ClientError, TokenClientError};

/// Auth-failed token of a ban for port-forwarding abuse. The apps key their
/// port-forwarding suspension message on it.
pub const AUTH_FAILED_BANNED_PORT_FORWARDING: &str = "[BANNED_PORT_FORWARDING]";

/// Auth-failed token of any other ban.
pub const AUTH_FAILED_BANNED: &str = "[BANNED]";

/// What the client knows of its wallet's standing.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    /// Strikes still inside the sliding window, oldest first.
    pub strikes: Vec<AccountStrike>,
    /// Live strikes that trigger a ban. `0` while only an issuance refusal
    /// is known, since that answer does not carry it.
    pub threshold: u32,
    /// Length of the sliding window in days, `0` when unknown for the same
    /// reason as `threshold`.
    pub window_days: u32,
    /// The ban in force, if any.
    pub ban: Option<Ban>,
}

impl std::fmt::Debug for Standing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Standing")
            .field("strikes", &self.strikes.len())
            .field("threshold", &self.threshold)
            .field("window_days", &self.window_days)
            .field("ban", &self.ban)
            .finish()
    }
}

impl Standing {
    /// A standing that only knows the ban, as an issuance refusal reports it.
    #[must_use]
    pub fn ban_only(ban: Ban) -> Self {
        Self {
            strikes: Vec::new(),
            threshold: 0,
            window_days: 0,
            ban: Some(ban),
        }
    }
}

impl From<AccountStandingResponse> for Standing {
    fn from(response: AccountStandingResponse) -> Self {
        Self {
            strikes: response.strikes,
            threshold: response.threshold,
            window_days: response.window_days,
            ban: response.ban.map(Ban::from),
        }
    }
}

/// A ban on the wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ban {
    /// Why the wallet is banned.
    pub reason: BanReasonCode,
    /// When the ban took effect, Unix seconds. `None` when the ban is known
    /// from an issuance refusal only.
    pub banned_at_unix_secs: Option<u64>,
    /// When it lapses on its own, Unix seconds. `None` for a ban that does
    /// not lapse.
    pub lapses_at_unix_secs: Option<u64>,
}

impl From<AccountBan> for Ban {
    fn from(ban: AccountBan) -> Self {
        Self {
            reason: ban.reason_code,
            banned_at_unix_secs: Some(ban.banned_at_unix_secs),
            lapses_at_unix_secs: ban.lapses_at_unix_secs,
        }
    }
}

impl Ban {
    /// The ban an issuer's refusal carries, `None` for any other failure of
    /// a token or entitlement refresh.
    #[must_use]
    pub fn from_refresh_error(error: &TokenClientError) -> Option<Self> {
        match error {
            TokenClientError::Api(ClientError::Banned {
                reason_code,
                lapses_at_unix_secs,
            }) => Some(Self {
                reason: *reason_code,
                banned_at_unix_secs: None,
                lapses_at_unix_secs: *lapses_at_unix_secs,
            }),
            _ => None,
        }
    }

    /// Whether the ban still holds at `now_unix_secs`. A ban lapses at its
    /// lapse instant, not a second later: the server sweeps the entry then.
    #[must_use]
    pub fn in_force(&self, now_unix_secs: u64) -> bool {
        self.lapses_at_unix_secs
            .is_none_or(|lapses_at| now_unix_secs < lapses_at)
    }

    /// The auth-failed reason the tunnel blocks with while this ban holds.
    #[must_use]
    pub fn auth_failed_reason(&self) -> String {
        match self.reason {
            BanReasonCode::PortForwardingAbuse => format!(
                "{AUTH_FAILED_BANNED_PORT_FORWARDING} the API refused to issue credentials; \
                 access suspended for port-forwarding abuse"
            ),
            _ => format!(
                "{AUTH_FAILED_BANNED} the API refused to issue credentials; access suspended"
            ),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use warren_api::{AbuseCategory, AccountStrike};

    /// A strike as the standing endpoint answers it.
    pub(crate) fn strike(case_reference: &str, port: u16, day: u64) -> AccountStrike {
        serde_json::from_value(serde_json::json!({
            "day_unix_secs": day,
            "category": AbuseCategory::Copyright,
            "exit_country": "FI",
            "port": port,
            "case_reference": case_reference,
        }))
        .expect("a well-formed strike")
    }
}

#[cfg(test)]
mod tests {
    use warren_api::{AccountBan, AccountStandingResponse, ClientError, TokenClientError};

    use super::test_support::strike;
    use super::*;

    fn response(ban: Option<AccountBan>) -> AccountStandingResponse {
        AccountStandingResponse {
            strikes: vec![strike("PF-2026-0001", 51413, 1_790_000_000)],
            threshold: 3,
            window_days: 90,
            ban,
        }
    }

    fn ban(reason: BanReasonCode, lapses_at_unix_secs: Option<u64>) -> Ban {
        Ban {
            reason,
            banned_at_unix_secs: None,
            lapses_at_unix_secs,
        }
    }

    #[test]
    fn a_standing_answer_keeps_its_strikes_threshold_window_and_ban() {
        let standing = Standing::from(response(Some(AccountBan {
            banned_at_unix_secs: 1_790_000_000,
            lapses_at_unix_secs: Some(1_821_536_000),
            reason_code: BanReasonCode::PortForwardingAbuse,
        })));

        assert_eq!(standing.strikes.len(), 1);
        assert_eq!(standing.strikes[0].port, 51413);
        assert_eq!((standing.threshold, standing.window_days), (3, 90));
        assert_eq!(
            standing.ban,
            Some(Ban {
                reason: BanReasonCode::PortForwardingAbuse,
                banned_at_unix_secs: Some(1_790_000_000),
                lapses_at_unix_secs: Some(1_821_536_000),
            })
        );
    }

    #[test]
    fn the_issuers_ban_refusal_becomes_a_ban() {
        let refusal = TokenClientError::Api(ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs: Some(1_821_536_000),
        });

        assert_eq!(
            Ban::from_refresh_error(&refusal),
            Some(ban(BanReasonCode::PortForwardingAbuse, Some(1_821_536_000)))
        );
    }

    #[test]
    fn any_other_refresh_failure_is_not_a_ban() {
        let forbidden = TokenClientError::Api(ClientError::ServerStatus {
            status: 403,
            body: String::new(),
        });

        assert_eq!(Ban::from_refresh_error(&forbidden), None);
        assert_eq!(
            Ban::from_refresh_error(&TokenClientError::BadAttributionKey),
            None
        );
    }

    #[test]
    fn a_ban_holds_until_its_lapse_instant() {
        let lapsing = ban(BanReasonCode::Other, Some(1_000));

        assert!(lapsing.in_force(999));
        assert!(!lapsing.in_force(1_000));
    }

    #[test]
    fn a_ban_without_a_lapse_never_lapses() {
        assert!(ban(BanReasonCode::Other, None).in_force(u64::MAX));
    }

    #[test]
    fn a_port_forwarding_ban_blocks_with_its_own_token() {
        let reason = ban(BanReasonCode::PortForwardingAbuse, None).auth_failed_reason();

        assert!(reason.starts_with("[BANNED_PORT_FORWARDING] "), "{reason}");
    }

    #[test]
    fn any_other_ban_blocks_with_the_generic_token() {
        let reason = ban(BanReasonCode::Other, None).auth_failed_reason();

        assert!(reason.starts_with("[BANNED] "), "{reason}");
    }

    #[test]
    fn debug_renders_neither_the_case_reference_nor_the_port() {
        let rendered = format!("{:?}", Standing::from(response(None)));

        assert!(!rendered.contains("PF-2026-0001"), "{rendered}");
        assert!(!rendered.contains("51413"), "{rendered}");
    }
}
