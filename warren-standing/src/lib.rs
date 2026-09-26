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
//!
//! [`entitlements`] is the other half of a forwarded port: the per-wallet
//! entitlement mint and the slot each rule draws its envelope from, the piece
//! the desktop daemon, Android and iOS present on every NAT-PMP request.

pub mod entitlements;
mod ledger;
mod natpmp_slot;
mod port_refusal;
mod store;
mod tracker;

pub use ledger::{LedgerError, StrikeLedger};
pub use natpmp_slot::NatPmpSlot;
pub use port_refusal::{PortRefusal, RefusalCount};
pub use store::{LEDGER_FILE, StandingStore, ban_verdict_json};
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
            TokenClientError::Api(error) => Self::from_client_error(error),
            _ => None,
        }
    }

    /// The ban an API refusal carries, `None` for any other failure. Issuance
    /// and every call that credits time (a voucher redemption, the store
    /// payment calls) refuse a wallet on the CRL this way, before consuming
    /// anything (warren-core doc 105 §5.3).
    #[must_use]
    pub fn from_client_error(error: &ClientError) -> Option<Self> {
        match error {
            ClientError::Banned {
                reason_code,
                lapses_at_unix_secs,
            } => Some(Self {
                reason: *reason_code,
                banned_at_unix_secs: None,
                lapses_at_unix_secs: *lapses_at_unix_secs,
            }),
            _ => None,
        }
    }

    /// The ban an exit's `RejectedBanned` answer carries, from the opaque
    /// ban-reason code the exit sealed on it. The exit says nothing about
    /// when the ban lapses. Code `1` is port-forwarding abuse
    /// (`warren-exit-policy` `ban_reason_code::PORT_FORWARDING_ABUSE`); any
    /// other code, `0` and codes newer than this build included, is the
    /// generic suspension.
    #[must_use]
    pub fn from_exit_rejection(code: u8) -> Self {
        const PORT_FORWARDING_ABUSE: u8 = 1;
        Self {
            reason: if code == PORT_FORWARDING_ABUSE {
                BanReasonCode::PortForwardingAbuse
            } else {
                BanReasonCode::Other
            },
            banned_at_unix_secs: None,
            lapses_at_unix_secs: None,
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

/// The mobile FFI envelope of a call the API refused because the wallet is
/// banned: `{"ok":false,"error":"banned","ban":{"reason":..,"lapses_at_unix_secs":N|null}}`.
/// Android and iOS answer a voucher redemption and a store payment call with
/// it, so the two apps read one shape. The refused voucher or store
/// transaction is still the user's to present once the ban ends.
#[must_use]
pub fn ban_refusal_envelope(ban: &Ban) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "banned",
        "ban": {
            "reason": ban.reason,
            "lapses_at_unix_secs": ban.lapses_at_unix_secs,
        },
    })
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
    fn a_credit_refusal_without_a_lapse_is_a_ban_with_no_known_end() {
        let refusal = ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs: None,
        };

        assert_eq!(
            Ban::from_client_error(&refusal),
            Some(ban(BanReasonCode::PortForwardingAbuse, None))
        );
    }

    #[test]
    fn a_403_with_another_body_is_not_a_ban() {
        let forbidden = ClientError::ServerStatus {
            status: 403,
            body: "identity mismatch".to_owned(),
        };

        assert_eq!(Ban::from_client_error(&forbidden), None);
    }

    #[test]
    fn the_refusal_envelope_names_the_reason_and_the_lapse() {
        let envelope = ban_refusal_envelope(&ban(
            BanReasonCode::PortForwardingAbuse,
            Some(1_821_536_000),
        ));

        assert_eq!(
            envelope,
            serde_json::json!({
                "ok": false,
                "error": "banned",
                "ban": {"reason": "port_forwarding_abuse", "lapses_at_unix_secs": 1_821_536_000u64},
            })
        );
    }

    #[test]
    fn the_refusal_envelope_of_a_ban_with_no_known_end_carries_a_null_lapse() {
        let envelope = ban_refusal_envelope(&ban(BanReasonCode::Other, None));

        assert_eq!(envelope["ban"]["reason"], "other");
        assert_eq!(
            envelope["ban"]["lapses_at_unix_secs"],
            serde_json::Value::Null
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
    fn an_exit_ban_rejection_with_the_port_forwarding_code_is_a_port_forwarding_ban() {
        assert_eq!(
            Ban::from_exit_rejection(1),
            ban(BanReasonCode::PortForwardingAbuse, None)
        );
    }

    #[test]
    fn an_exit_ban_rejection_with_any_other_code_is_a_generic_ban() {
        for code in [0, 2, 200] {
            assert_eq!(
                Ban::from_exit_rejection(code),
                ban(BanReasonCode::Other, None)
            );
        }
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
