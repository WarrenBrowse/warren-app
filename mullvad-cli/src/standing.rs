//! How the CLI prints the wallet's port-forward abuse standing (warren-core
//! doc 105): the human warning lines and the JSON view shared by
//! `account standing`, `port-forward status` and `status --listen`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use warren_standing::{AbuseCategory, AccountStrike, Ban, BanReasonCode, NewStrike, Standing};

/// Where a warning is contested, and the page that explains the procedure.
pub const CONTEST_LINE: &str = "To contest a warning, write to abuse@warrenbrowse.com quoting its \
     case reference (https://warren.ro/signalements).";

/// The human label of a report category.
fn category_label(category: AbuseCategory) -> &'static str {
    match category {
        AbuseCategory::Copyright => "copyright",
        AbuseCategory::MalwareC2 => "malware or command and control",
        AbuseCategory::Spam => "spam",
        AbuseCategory::Scanning => "scanning or intrusion attempts",
        AbuseCategory::Phishing => "phishing",
        AbuseCategory::Csam => "child sexual abuse material",
        _ => "other",
    }
}

/// The stable JSON label of a report category, the contract's own wire name.
fn category_json_label(category: AbuseCategory) -> &'static str {
    match category {
        AbuseCategory::Copyright => "copyright",
        AbuseCategory::MalwareC2 => "malware_c2",
        AbuseCategory::Spam => "spam",
        AbuseCategory::Scanning => "scanning",
        AbuseCategory::Phishing => "phishing",
        AbuseCategory::Csam => "csam",
        _ => "other",
    }
}

fn ban_reason_json_label(reason: BanReasonCode) -> &'static str {
    match reason {
        BanReasonCode::PortForwardingAbuse => "port_forwarding_abuse",
        _ => "other",
    }
}

/// `YYYY-MM-DD` in UTC, the precision a strike is recorded at.
fn utc_day(unix_secs: u64) -> String {
    i64::try_from(unix_secs)
        .ok()
        .and_then(|secs| DateTime::<Utc>::from_timestamp(secs, 0))
        .map_or_else(|| "?".to_owned(), |day| day.format("%Y-%m-%d").to_string())
}

/// A case reference as the terminal may print it. The reference comes from
/// the API over TLS alone, so a control character in it is escaped rather
/// than handed to the terminal.
fn printable(reference: &str) -> String {
    reference
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

/// One warning, as the user reads it: "Warning 1 of 3: public port N was
/// closed on DAY after an abuse report (category). Case reference: REF".
pub fn strike_line(strike: &AccountStrike, ordinal: u32, threshold: u32) -> String {
    let rank = if threshold == 0 {
        format!("Warning {ordinal}")
    } else {
        format!("Warning {ordinal} of {threshold}")
    };
    format!(
        "{rank}: public port {} was closed on {} after an abuse report ({}). Case reference: {}",
        strike.port,
        utc_day(strike.day_unix_secs),
        category_label(strike.category),
        printable(&strike.case_reference),
    )
}

/// The warning line of a strike notice from the event stream.
pub fn notice_line(notice: &NewStrike) -> String {
    strike_line(&notice.strike, notice.ordinal, notice.threshold)
}

/// The suspension line, with its lapse date when there is one.
pub fn ban_line(ban: &Ban) -> String {
    let why = match ban.reason {
        BanReasonCode::PortForwardingAbuse => "Account suspended for port-forwarding abuse",
        _ => "Account suspended",
    };
    match ban.lapses_at_unix_secs {
        Some(lapses_at) => format!("{why} until {}.", utc_day(lapses_at)),
        None => format!("{why}."),
    }
}

/// The lines `port-forward status` adds under the mappings. Nothing in good
/// standing, so the lines scripts parse stay exactly what they were.
pub fn status_lines(standing: Option<&Standing>) -> Vec<String> {
    let Some(standing) = standing else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(ban) = &standing.ban {
        lines.push(ban_line(ban));
    }
    lines.extend(strike_lines(standing));
    if !standing.strikes.is_empty() {
        lines.push(CONTEST_LINE.to_owned());
    }
    lines
}

fn strike_lines(standing: &Standing) -> impl Iterator<Item = String> + '_ {
    standing
        .strikes
        .iter()
        .zip(1u32..)
        .map(|(strike, ordinal)| strike_line(strike, ordinal, standing.threshold))
}

/// The whole report `account standing` prints.
pub fn report_lines(standing: &Standing) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(ban) = &standing.ban {
        lines.push(ban_line(ban));
    }
    if standing.strikes.is_empty() {
        if standing.ban.is_none() {
            lines.push(if standing.window_days == 0 {
                "No port forwarding warnings.".to_owned()
            } else {
                format!(
                    "No port forwarding warnings in the last {} days.",
                    standing.window_days
                )
            });
        }
        return lines;
    }
    lines.extend(strike_lines(standing));
    lines.push(CONTEST_LINE.to_owned());
    lines
}

/// JSON view of the standing. Every optional field is present as `null`, the
/// rule the `port-forward` JSON contract already follows.
#[derive(Serialize)]
pub struct StandingJson<'a> {
    threshold: u32,
    window_days: u32,
    strikes: Vec<StrikeJson<'a>>,
    ban: Option<BanJson>,
}

#[derive(Serialize)]
struct StrikeJson<'a> {
    day: String,
    day_unix_secs: u64,
    category: &'static str,
    exit_country: Option<&'a str>,
    port: u16,
    case_reference: &'a str,
}

#[derive(Serialize)]
struct BanJson {
    reason: &'static str,
    banned_at_unix_secs: Option<u64>,
    lapses_at_unix_secs: Option<u64>,
}

impl<'a> From<&'a Standing> for StandingJson<'a> {
    fn from(standing: &'a Standing) -> Self {
        StandingJson {
            threshold: standing.threshold,
            window_days: standing.window_days,
            strikes: standing
                .strikes
                .iter()
                .map(|strike| StrikeJson {
                    day: utc_day(strike.day_unix_secs),
                    day_unix_secs: strike.day_unix_secs,
                    category: category_json_label(strike.category),
                    exit_country: strike.exit_country.as_ref().map(|c| c.as_str()),
                    port: strike.port,
                    case_reference: &strike.case_reference,
                })
                .collect(),
            ban: standing.ban.as_ref().map(|ban| BanJson {
                reason: ban_reason_json_label(ban.reason),
                banned_at_unix_secs: ban.banned_at_unix_secs,
                lapses_at_unix_secs: ban.lapses_at_unix_secs,
            }),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 2026-09-24T00:00:00Z.
    pub(crate) const DAY: u64 = 1_790_208_000;

    pub(crate) fn strike(port: u16, case_reference: &str) -> AccountStrike {
        AccountStrike {
            day_unix_secs: DAY,
            category: AbuseCategory::Copyright,
            exit_country: Some("FI".parse().unwrap()),
            port,
            case_reference: case_reference.to_owned(),
        }
    }

    pub(crate) fn standing(strikes: Vec<AccountStrike>, ban: Option<Ban>) -> Standing {
        Standing {
            strikes,
            threshold: 3,
            window_days: 90,
            ban,
        }
    }

    fn pf_ban(lapses_at_unix_secs: Option<u64>) -> Ban {
        Ban {
            reason: BanReasonCode::PortForwardingAbuse,
            banned_at_unix_secs: Some(DAY),
            lapses_at_unix_secs,
        }
    }

    #[test]
    fn a_strike_reads_as_its_rank_port_day_category_and_case() {
        assert_eq!(
            strike_line(&strike(51413, "PF-2026-0042"), 1, 3),
            "Warning 1 of 3: public port 51413 was closed on 2026-09-24 after an abuse \
             report (copyright). Case reference: PF-2026-0042"
        );
    }

    #[test]
    fn a_case_reference_cannot_drive_the_terminal() {
        let line = strike_line(&strike(51413, "PF-1\u{1b}[2J"), 1, 3);

        assert!(!line.contains('\u{1b}'), "{line:?}");
        assert!(line.ends_with("Case reference: PF-1\\u{1b}[2J"), "{line:?}");
    }

    #[test]
    fn a_strike_without_a_known_threshold_gives_its_rank_alone() {
        assert!(strike_line(&strike(51413, "PF-1"), 2, 0).starts_with("Warning 2: "));
    }

    #[test]
    fn a_ban_names_its_lapse_day() {
        assert_eq!(
            ban_line(&pf_ban(Some(DAY + 365 * 86_400))),
            "Account suspended for port-forwarding abuse until 2027-09-24."
        );
    }

    #[test]
    fn a_ban_without_a_lapse_names_none() {
        assert_eq!(
            ban_line(&Ban {
                reason: BanReasonCode::Other,
                banned_at_unix_secs: None,
                lapses_at_unix_secs: None,
            }),
            "Account suspended."
        );
    }

    #[test]
    fn good_standing_adds_no_status_line() {
        assert!(status_lines(None).is_empty());
        assert!(status_lines(Some(&standing(Vec::new(), None))).is_empty());
    }

    #[test]
    fn strikes_add_their_lines_ranked_and_the_way_to_contest() {
        let lines = status_lines(Some(&standing(
            vec![strike(50000, "PF-1"), strike(50001, "PF-2")],
            None,
        )));

        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("Warning 1 of 3: public port 50000 "));
        assert!(lines[1].starts_with("Warning 2 of 3: public port 50001 "));
        assert_eq!(lines[2], CONTEST_LINE);
    }

    #[test]
    fn a_ban_leads_the_status_lines() {
        let lines = status_lines(Some(&standing(Vec::new(), Some(pf_ban(None)))));

        assert_eq!(lines, ["Account suspended for port-forwarding abuse."]);
    }

    #[test]
    fn the_report_of_good_standing_says_so() {
        assert_eq!(
            report_lines(&standing(Vec::new(), None)),
            ["No port forwarding warnings in the last 90 days."]
        );
    }

    #[test]
    fn the_json_view_pins_every_field() {
        let standing = standing(vec![strike(51413, "PF-2026-0042")], Some(pf_ban(None)));

        let json = serde_json::to_string(&StandingJson::from(&standing)).unwrap();

        assert_eq!(
            json,
            r#"{"threshold":3,"window_days":90,"strikes":[{"day":"2026-09-24","day_unix_secs":1790208000,"category":"copyright","exit_country":"FI","port":51413,"case_reference":"PF-2026-0042"}],"ban":{"reason":"port_forwarding_abuse","banned_at_unix_secs":1790208000,"lapses_at_unix_secs":null}}"#
        );
    }
}
