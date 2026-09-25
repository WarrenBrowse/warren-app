//! The wallet's port-forward abuse standing (warren-core doc 105) between the
//! shared `warren-standing` model and the wire.

use warren_standing::{AbuseCategory, AccountStrike, Ban, BanReasonCode, NewStrike, Standing};

use crate::types::{FromProtobufTypeError, proto};

impl From<&Standing> for proto::WarrenAccountStanding {
    fn from(standing: &Standing) -> Self {
        proto::WarrenAccountStanding {
            strikes: standing
                .strikes
                .iter()
                .map(proto::WarrenAccountStrike::from)
                .collect(),
            threshold: standing.threshold,
            window_days: standing.window_days,
            ban: standing.ban.as_ref().map(proto::WarrenAccountBan::from),
        }
    }
}

impl TryFrom<proto::WarrenAccountStanding> for Standing {
    type Error = FromProtobufTypeError;

    fn try_from(standing: proto::WarrenAccountStanding) -> Result<Self, Self::Error> {
        Ok(Standing {
            strikes: standing
                .strikes
                .into_iter()
                .map(AccountStrike::try_from)
                .collect::<Result<_, _>>()?,
            threshold: standing.threshold,
            window_days: standing.window_days,
            ban: standing.ban.map(Ban::from),
        })
    }
}

impl From<&AccountStrike> for proto::WarrenAccountStrike {
    fn from(strike: &AccountStrike) -> Self {
        proto::WarrenAccountStrike {
            day_unix_secs: strike.day_unix_secs,
            category: i32::from(category_to_proto(strike.category)),
            exit_country: strike
                .exit_country
                .as_ref()
                .map(|country| country.as_str().to_owned()),
            port: u32::from(strike.port),
            case_reference: strike.case_reference.clone(),
        }
    }
}

impl TryFrom<proto::WarrenAccountStrike> for AccountStrike {
    type Error = FromProtobufTypeError;

    fn try_from(strike: proto::WarrenAccountStrike) -> Result<Self, Self::Error> {
        Ok(AccountStrike {
            day_unix_secs: strike.day_unix_secs,
            category: category_from_proto(strike.category),
            exit_country: strike
                .exit_country
                .map(|country| country.parse())
                .transpose()
                .map_err(|_| FromProtobufTypeError::invalid_argument("invalid exit country"))?,
            port: u16::try_from(strike.port)
                .map_err(|_| FromProtobufTypeError::invalid_argument("strike port out of range"))?,
            case_reference: strike.case_reference,
        })
    }
}

impl From<&Ban> for proto::WarrenAccountBan {
    fn from(ban: &Ban) -> Self {
        proto::WarrenAccountBan {
            reason: i32::from(match ban.reason {
                BanReasonCode::PortForwardingAbuse => {
                    proto::WarrenBanReason::WarrenBanPortForwardingAbuse
                }
                _ => proto::WarrenBanReason::WarrenBanOther,
            }),
            banned_at_unix_secs: ban.banned_at_unix_secs,
            lapses_at_unix_secs: ban.lapses_at_unix_secs,
        }
    }
}

impl From<proto::WarrenAccountBan> for Ban {
    fn from(ban: proto::WarrenAccountBan) -> Self {
        Ban {
            // A reason a newer daemon sends and this client does not know is
            // still a ban: it reads as the generic one.
            reason: match proto::WarrenBanReason::try_from(ban.reason) {
                Ok(proto::WarrenBanReason::WarrenBanPortForwardingAbuse) => {
                    BanReasonCode::PortForwardingAbuse
                }
                Ok(proto::WarrenBanReason::WarrenBanOther) | Err(_) => BanReasonCode::Other,
            },
            banned_at_unix_secs: ban.banned_at_unix_secs,
            lapses_at_unix_secs: ban.lapses_at_unix_secs,
        }
    }
}

impl From<&NewStrike> for proto::WarrenAccountStrikeNotice {
    fn from(notice: &NewStrike) -> Self {
        proto::WarrenAccountStrikeNotice {
            strike: Some(proto::WarrenAccountStrike::from(&notice.strike)),
            ordinal: notice.ordinal,
            threshold: notice.threshold,
        }
    }
}

impl TryFrom<proto::WarrenAccountStrikeNotice> for NewStrike {
    type Error = FromProtobufTypeError;

    fn try_from(notice: proto::WarrenAccountStrikeNotice) -> Result<Self, Self::Error> {
        Ok(NewStrike {
            strike: notice
                .strike
                .ok_or_else(|| FromProtobufTypeError::invalid_argument("notice without a strike"))
                .and_then(AccountStrike::try_from)?,
            ordinal: notice.ordinal,
            threshold: notice.threshold,
        })
    }
}

fn category_to_proto(category: AbuseCategory) -> proto::WarrenAbuseCategory {
    use proto::WarrenAbuseCategory as P;
    match category {
        AbuseCategory::Copyright => P::WarrenAbuseCopyright,
        AbuseCategory::MalwareC2 => P::WarrenAbuseMalwareC2,
        AbuseCategory::Spam => P::WarrenAbuseSpam,
        AbuseCategory::Scanning => P::WarrenAbuseScanning,
        AbuseCategory::Phishing => P::WarrenAbusePhishing,
        AbuseCategory::Csam => P::WarrenAbuseCsam,
        _ => P::WarrenAbuseOther,
    }
}

fn category_from_proto(category: i32) -> AbuseCategory {
    use proto::WarrenAbuseCategory as P;
    match P::try_from(category) {
        Ok(P::WarrenAbuseCopyright) => AbuseCategory::Copyright,
        Ok(P::WarrenAbuseMalwareC2) => AbuseCategory::MalwareC2,
        Ok(P::WarrenAbuseSpam) => AbuseCategory::Spam,
        Ok(P::WarrenAbuseScanning) => AbuseCategory::Scanning,
        Ok(P::WarrenAbusePhishing) => AbuseCategory::Phishing,
        Ok(P::WarrenAbuseCsam) => AbuseCategory::Csam,
        Ok(P::WarrenAbuseOther) | Err(_) => AbuseCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strike(category: AbuseCategory, exit_country: Option<&str>) -> AccountStrike {
        AccountStrike {
            day_unix_secs: 1_790_035_200,
            category,
            exit_country: exit_country.map(|c| c.parse().unwrap()),
            port: 51413,
            case_reference: "PF-2026-0042".to_owned(),
        }
    }

    fn standing(ban: Option<Ban>) -> Standing {
        Standing {
            strikes: vec![
                strike(AbuseCategory::Copyright, Some("FI")),
                strike(AbuseCategory::Scanning, None),
            ],
            threshold: 3,
            window_days: 90,
            ban,
        }
    }

    #[test]
    fn a_standing_survives_the_wire() {
        let sent = standing(Some(Ban {
            reason: BanReasonCode::PortForwardingAbuse,
            banned_at_unix_secs: Some(1_790_035_200),
            lapses_at_unix_secs: Some(1_821_571_200),
        }));

        let received = Standing::try_from(proto::WarrenAccountStanding::from(&sent));

        assert_eq!(received.unwrap(), sent);
    }

    #[test]
    fn every_known_category_survives_the_wire() {
        for category in [
            AbuseCategory::Copyright,
            AbuseCategory::MalwareC2,
            AbuseCategory::Spam,
            AbuseCategory::Scanning,
            AbuseCategory::Phishing,
            AbuseCategory::Csam,
            AbuseCategory::Other,
        ] {
            assert_eq!(
                category_from_proto(category_to_proto(category).into()),
                category
            );
        }
    }

    #[test]
    fn a_category_this_build_does_not_know_reads_as_other() {
        assert_eq!(category_from_proto(99), AbuseCategory::Other);
    }

    #[test]
    fn a_ban_reason_this_build_does_not_know_is_still_a_ban() {
        let ban = Ban::from(proto::WarrenAccountBan {
            reason: 99,
            banned_at_unix_secs: None,
            lapses_at_unix_secs: None,
        });

        assert_eq!(ban.reason, BanReasonCode::Other);
    }

    #[test]
    fn a_strike_port_beyond_u16_is_refused() {
        let mut wire = proto::WarrenAccountStrike::from(&strike(AbuseCategory::Spam, None));
        wire.port = 70_000;

        assert!(AccountStrike::try_from(wire).is_err());
    }

    #[test]
    fn a_notice_keeps_its_rank_and_threshold() {
        let sent = NewStrike {
            strike: strike(AbuseCategory::Copyright, Some("FI")),
            ordinal: 2,
            threshold: 3,
        };

        let received = NewStrike::try_from(proto::WarrenAccountStrikeNotice::from(&sent));

        assert_eq!(received.unwrap(), sent);
    }

    #[test]
    fn a_notice_without_a_strike_is_refused() {
        let wire = proto::WarrenAccountStrikeNotice {
            strike: None,
            ordinal: 1,
            threshold: 3,
        };

        assert!(NewStrike::try_from(wire).is_err());
    }
}
