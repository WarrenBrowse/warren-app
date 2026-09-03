//! The exit an Android dial goes to: the one node a location pin (country,
//! city or a single exit) resolves to over the relay list `listRelays`
//! verified, and the alternative a drop retry moves to. Pure JSON in, JSON
//! out, so the JNI export is a thin shell and the host tests exercise the
//! exact bytes Kotlin sends and reads.
//!
//! Scoping (active rows inside the pin, the failover's same-country
//! preference) is spelled here; the pick among the survivors is the shared
//! `warren_discovery_core::pick_exit` (highest weight, ties broken by the
//! smallest exit id), the rule the desktop daemon dials by, so two equal
//! weight exits in one country land on the same node on every platform.
//! The Kotlin copy this replaced broke ties on the LARGEST id.

use serde::Deserialize;
use warren_discovery_core::{ExitCandidate, pick_exit};

/// What the user pinned in the location picker, as Kotlin's `ExitPin`
/// crosses the boundary: `{"kind":"automatic"}`,
/// `{"kind":"country","country":..}`, `{"kind":"city","country":..,"city":..}`
/// or `{"kind":"exit","exit_id":..}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum ExitPinSpec {
    /// Nothing pinned: the connect path applies its own fallback chain.
    Automatic,
    /// Any active exit in `country` (ISO alpha-2, matched case-insensitively).
    Country { country: String },
    /// Any active exit in `city` of `country`, both matched case-insensitively.
    City { country: String, city: String },
    /// One specific exit, by its 16-byte exit id (lowercase hex).
    Exit { exit_id: String },
}

/// One row of the relay list in the schema `listRelays` projects (the
/// Kotlin `WarrenRelaySummary`); only the fields the choice reads are
/// decoded, the rest are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RelayRow {
    pub exit_id: String,
    #[serde(default)]
    pub exit_pubkey_hex: String,
    pub country: String,
    #[serde(default)]
    pub city: String,
    pub active: bool,
    pub weight: u64,
}

/// Index into `relays` of the exit `pin` dials, or `None` when the pin
/// names nothing usable (an empty scope, an inactive exit, or
/// [`ExitPinSpec::Automatic`], which pins nothing on purpose so the
/// caller's own fallback chain still runs).
#[must_use]
pub(crate) fn resolve_exit_pin(pin: &ExitPinSpec, relays: &[RelayRow]) -> Option<usize> {
    if *pin == ExitPinSpec::Automatic {
        return None;
    }
    let scope: Vec<usize> = relays
        .iter()
        .enumerate()
        .filter(|(_, r)| r.active && admits(pin, None, r))
        .map(|(i, _)| i)
        .collect();
    pick_among(relays, &scope)
}

/// Index into `relays` of the exit an unpinned dial goes to: the shared
/// pick among the active rows of the preferred `exit_country`, and among
/// every active row when that country has none.
///
/// An [`ExitPinSpec::Automatic`] pin names no scope, so the choice belongs
/// to the connect path rather than to [`resolve_exit_pin`]. It lives here
/// so it is the shared `warren_discovery_core::pick_exit` too: the Kotlin
/// chain this replaced took the first active row of the list, which
/// ignored the fleet's weights on the configuration most users run while
/// the daemon and iOS applied the shared rule to the same directory.
///
/// The country is a preference, not a pin: when nothing in it is active the
/// dial takes the heaviest exit anywhere rather than refusing to connect.
#[must_use]
pub(crate) fn resolve_automatic_exit(
    exit_country: Option<&str>,
    relays: &[RelayRow],
) -> Option<usize> {
    let preferred: Vec<usize> = relays
        .iter()
        .enumerate()
        .filter(|(_, r)| r.active && admits(&ExitPinSpec::Automatic, exit_country, r))
        .map(|(i, _)| i)
        .collect();
    pick_among(relays, &preferred).or_else(|| {
        let anywhere: Vec<usize> = relays
            .iter()
            .enumerate()
            .filter(|(_, r)| r.active)
            .map(|(i, _)| i)
            .collect();
        pick_among(relays, &anywhere)
    })
}

/// Index into `relays` of the exit an automatic retry dials once the exit
/// with `failed_exit_pubkey_hex` dropped, or `None` when the pin leaves no
/// alternative (the retry then redials the same exit).
///
/// The desktop selector's failover policy
/// (`WarrenRelaySelector::select_failover_alternative`): an active
/// alternative inside the pinned scope, in the failed exit's own country
/// first, then in any country the scope allows, never the failed exit
/// itself. An automatic pin is scoped by `exit_country` when one is
/// preferred. The pick among the candidates is [`resolve_exit_pin`]'s, so
/// the same drop lands on the same alternative every time.
#[must_use]
pub(crate) fn resolve_failover_exit(
    pin: &ExitPinSpec,
    exit_country: Option<&str>,
    relays: &[RelayRow],
    failed_exit_pubkey_hex: &str,
) -> Option<usize> {
    let in_scope: Vec<usize> = relays
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.active && r.exit_pubkey_hex != failed_exit_pubkey_hex && admits(pin, exit_country, r)
        })
        .map(|(i, _)| i)
        .collect();
    let failed_country = relays
        .iter()
        .find(|r| r.exit_pubkey_hex == failed_exit_pubkey_hex)
        .map(|r| r.country.as_str());
    let same_country: Vec<usize> = failed_country
        .map(|c| {
            in_scope
                .iter()
                .copied()
                .filter(|&i| relays[i].country.eq_ignore_ascii_case(c))
                .collect()
        })
        .unwrap_or_default();
    pick_among(
        relays,
        if same_country.is_empty() {
            &in_scope
        } else {
            &same_country
        },
    )
}

/// Whether `relay` is inside the pin's scope. `exit_country` only matters
/// to an automatic pin, which the failover narrows to the preferred exit
/// country when one is set.
fn admits(pin: &ExitPinSpec, exit_country: Option<&str>, relay: &RelayRow) -> bool {
    match pin {
        ExitPinSpec::Automatic => exit_country
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .is_none_or(|c| relay.country.eq_ignore_ascii_case(c)),
        ExitPinSpec::Exit { exit_id } => relay.exit_id == *exit_id,
        ExitPinSpec::City { country, city } => {
            relay.country.eq_ignore_ascii_case(country) && relay.city.eq_ignore_ascii_case(city)
        }
        ExitPinSpec::Country { country } => relay.country.eq_ignore_ascii_case(country),
    }
}

/// The shared pick over the rows `scope` indexes. A row whose exit id is
/// not 16 bytes cannot be named in a setup frame, so it is never dialed
/// (the list is Rust's own projection, so this is a defence, not a path).
fn pick_among(relays: &[RelayRow], scope: &[usize]) -> Option<usize> {
    let dialable: Vec<(usize, ExitCandidate)> = scope
        .iter()
        .filter_map(|&i| {
            exit_id16(&relays[i].exit_id).map(|exit_id| {
                (
                    i,
                    ExitCandidate {
                        weight: relays[i].weight,
                        exit_id,
                    },
                )
            })
        })
        .collect();
    let candidates: Vec<ExitCandidate> = dialable.iter().map(|(_, c)| *c).collect();
    pick_exit(&candidates).map(|k| dialable[k].0)
}

fn exit_id16(hex_id: &str) -> Option<[u8; 16]> {
    hex::decode(hex_id).ok()?.try_into().ok()
}

/// The JNI answer: `{"index":<n>}` when a row was chosen, `{"index":null}`
/// otherwise (nothing usable, or an input Rust could not read).
fn index_json(index: Option<usize>) -> String {
    serde_json::json!({ "index": index }).to_string()
}

fn decode<T: for<'de> Deserialize<'de>>(what: &str, json: &str) -> Option<T> {
    match serde_json::from_str(json) {
        Ok(v) => Some(v),
        Err(_) => {
            // The class only: the payload names exits and cities the user chose.
            log::warn!("exit pin: could not decode the {what} handed over JNI");
            None
        }
    }
}

/// [`resolve_exit_pin`] over the JSON Kotlin hands the JNI export:
/// `pin_json` an `ExitPin`, `relays_json` the `listRelays` array. Returns
/// [`index_json`].
#[must_use]
pub(crate) fn resolve_exit_pin_json(pin_json: &str, relays_json: &str) -> String {
    let picked = match (
        decode::<ExitPinSpec>("pin", pin_json),
        decode::<Vec<RelayRow>>("relay list", relays_json),
    ) {
        (Some(pin), Some(relays)) => resolve_exit_pin(&pin, &relays),
        _ => None,
    };
    index_json(picked)
}

/// [`resolve_automatic_exit`] over the JSON Kotlin hands the JNI export; an
/// empty `exit_country` is "none preferred". Returns [`index_json`].
#[must_use]
pub(crate) fn resolve_automatic_exit_json(exit_country: &str, relays_json: &str) -> String {
    let picked = decode::<Vec<RelayRow>>("relay list", relays_json).and_then(|relays| {
        resolve_automatic_exit(Some(exit_country).filter(|c| !c.trim().is_empty()), &relays)
    });
    index_json(picked)
}

/// [`resolve_failover_exit`] over the JSON Kotlin hands the JNI export; an
/// empty `exit_country` is "none preferred". Returns [`index_json`].
#[must_use]
pub(crate) fn resolve_failover_exit_json(
    pin_json: &str,
    exit_country: &str,
    relays_json: &str,
    failed_exit_pubkey_hex: &str,
) -> String {
    let picked = match (
        decode::<ExitPinSpec>("pin", pin_json),
        decode::<Vec<RelayRow>>("relay list", relays_json),
    ) {
        (Some(pin), Some(relays)) => resolve_failover_exit(
            &pin,
            Some(exit_country).filter(|c| !c.trim().is_empty()),
            &relays,
            failed_exit_pubkey_hex,
        ),
        _ => None,
    };
    index_json(picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-byte exit id whose last byte is `n`, as lowercase hex.
    fn id(n: u8) -> String {
        format!("{n:032x}")
    }

    fn relay(n: u8, country: &str, city: &str, active: bool, weight: u64) -> RelayRow {
        RelayRow {
            exit_id: id(n),
            exit_pubkey_hex: format!("k-{n}"),
            country: country.to_owned(),
            city: city.to_owned(),
            active,
            weight,
        }
    }

    fn country(c: &str) -> ExitPinSpec {
        ExitPinSpec::Country {
            country: c.to_owned(),
        }
    }

    fn city(c: &str, city: &str) -> ExitPinSpec {
        ExitPinSpec::City {
            country: c.to_owned(),
            city: city.to_owned(),
        }
    }

    fn exit(n: u8) -> ExitPinSpec {
        ExitPinSpec::Exit { exit_id: id(n) }
    }

    // de1 = 1, de2 = 2, de3 = 3, fr1 = 4, fr2 = 5 (inactive), nl1 = 6.
    fn catalogue() -> Vec<RelayRow> {
        vec![
            relay(1, "DE", "Frankfurt", true, 10),
            relay(2, "DE", "Frankfurt", true, 30),
            relay(3, "DE", "Berlin", true, 50),
            relay(4, "FR", "Paris", true, 90),
            relay(5, "FR", "Paris", false, 99),
        ]
    }

    #[test]
    fn automatic_pins_nothing_so_the_caller_keeps_its_own_fallback() {
        assert_eq!(
            resolve_exit_pin(&ExitPinSpec::Automatic, &catalogue()),
            None
        );
    }

    #[test]
    fn an_automatic_dial_picks_the_heaviest_active_exit_not_the_first_row() {
        // The Kotlin chain this replaced took `relays.firstOrNull { active }`,
        // so an unpinned Android dial ignored the fleet's weights while the
        // daemon and iOS applied `pick_exit` to the same directory.
        assert_eq!(resolve_automatic_exit(None, &catalogue()), Some(3));
    }

    #[test]
    fn an_automatic_dial_stays_in_the_preferred_country_when_it_has_an_active_exit() {
        assert_eq!(resolve_automatic_exit(Some("DE"), &catalogue()), Some(2));
        assert_eq!(
            resolve_automatic_exit(Some("de"), &catalogue()),
            Some(2),
            "the preference matches the catalogue case-insensitively"
        );
    }

    #[test]
    fn an_automatic_dial_leaves_a_country_with_nothing_active_rather_than_refusing() {
        // Every French row is down: an unpinned dial is a preference, not a
        // pin, so it takes the heaviest exit anywhere instead of stranding
        // the user with no circuit at all.
        let france_down: Vec<RelayRow> = catalogue()
            .into_iter()
            .map(|r| {
                let down = r.country == "FR";
                RelayRow { active: !down, ..r }
            })
            .collect();
        assert_eq!(resolve_automatic_exit(Some("FR"), &france_down), Some(2));
    }

    #[test]
    fn an_automatic_dial_with_a_blank_preference_is_no_preference() {
        assert_eq!(resolve_automatic_exit(Some("  "), &catalogue()), Some(3));
    }

    #[test]
    fn an_automatic_dial_with_no_active_exit_at_all_resolves_to_nothing() {
        let down = vec![relay(7, "SE", "Stockholm", false, 1)];
        assert_eq!(resolve_automatic_exit(None, &down), None);
        assert_eq!(resolve_automatic_exit(Some("SE"), &down), None);
    }

    #[test]
    fn an_exit_pin_resolves_to_that_exit() {
        assert_eq!(resolve_exit_pin(&exit(1), &catalogue()), Some(0));
    }

    #[test]
    fn an_exit_pin_on_an_inactive_or_unknown_exit_resolves_to_nothing() {
        assert_eq!(resolve_exit_pin(&exit(5), &catalogue()), None);
        assert_eq!(resolve_exit_pin(&exit(99), &catalogue()), None);
    }

    #[test]
    fn a_country_pin_resolves_to_the_heaviest_active_exit_in_that_country() {
        assert_eq!(resolve_exit_pin(&country("DE"), &catalogue()), Some(2));
        // fr2 is heavier and inactive.
        assert_eq!(resolve_exit_pin(&country("FR"), &catalogue()), Some(3));
    }

    #[test]
    fn a_country_pin_matches_the_catalogue_case_insensitively() {
        assert_eq!(resolve_exit_pin(&country("de"), &catalogue()), Some(2));
    }

    #[test]
    fn a_country_pin_with_no_active_exit_resolves_to_nothing() {
        let down = vec![relay(7, "SE", "Stockholm", false, 1)];
        assert_eq!(resolve_exit_pin(&country("SE"), &down), None);
    }

    #[test]
    fn a_city_pin_resolves_inside_that_city_only() {
        assert_eq!(
            resolve_exit_pin(&city("DE", "Frankfurt"), &catalogue()),
            Some(1)
        );
        assert_eq!(
            resolve_exit_pin(&city("de", "frankfurt"), &catalogue()),
            Some(1),
            "city and country match case-insensitively"
        );
        assert_eq!(
            resolve_exit_pin(&city("FR", "Paris"), &catalogue()[4..]),
            None,
            "an inactive exit is not a candidate"
        );
    }

    #[test]
    fn equal_weights_resolve_to_the_smallest_exit_id_on_every_dial() {
        // The daemon's rule, promoted to warren-discovery-core: the Kotlin
        // copy this replaced landed on the LARGEST id, so two equal-weight
        // exits in one country dialed different nodes on desktop and Android.
        let tied = vec![
            relay(2, "NL", "Amsterdam", true, 7),
            relay(1, "NL", "Amsterdam", true, 7),
        ];
        assert_eq!(resolve_exit_pin(&country("NL"), &tied), Some(1));
        let reversed: Vec<RelayRow> = tied.iter().rev().cloned().collect();
        assert_eq!(resolve_exit_pin(&country("NL"), &reversed), Some(0));
    }

    #[test]
    fn a_row_whose_exit_id_is_not_sixteen_bytes_is_never_dialed() {
        let mut rows = catalogue();
        rows[2].exit_id = "not-hex".to_owned();
        assert_eq!(
            resolve_exit_pin(&country("DE"), &rows),
            Some(1),
            "the heaviest German row cannot be named in a setup frame"
        );
        assert_eq!(
            resolve_exit_pin(
                &ExitPinSpec::Exit {
                    exit_id: "not-hex".to_owned()
                },
                &rows
            ),
            None
        );
    }

    // Failover: the exit an automatic retry dials once the one it was on
    // dropped (same country first, then any country in scope, never the
    // failed exit itself, nothing outside the pin).

    fn keyed() -> Vec<RelayRow> {
        vec![
            relay(1, "DE", "Frankfurt", true, 10),
            relay(2, "DE", "Frankfurt", true, 30),
            relay(3, "DE", "Berlin", true, 50),
            relay(4, "FR", "Paris", true, 90),
            relay(6, "NL", "Amsterdam", true, 20),
        ]
    }

    fn failover(pin: &ExitPinSpec, failed: u8, rows: &[RelayRow]) -> Option<usize> {
        resolve_failover_exit(pin, None, rows, &format!("k-{failed}"))
    }

    #[test]
    fn a_failover_prefers_an_alternative_in_the_failed_exits_country() {
        // fr1 is heavier, but the failed exit was German and Germany has spares.
        assert_eq!(failover(&ExitPinSpec::Automatic, 3, &keyed()), Some(1));
    }

    #[test]
    fn a_failover_never_returns_the_exit_that_failed() {
        let only = vec![relay(4, "FR", "Paris", true, 90)];
        assert_eq!(failover(&ExitPinSpec::Automatic, 4, &only), None);
    }

    #[test]
    fn a_failover_falls_back_to_any_country_when_the_failed_country_has_no_spare() {
        assert_eq!(failover(&ExitPinSpec::Automatic, 4, &keyed()), Some(2));
    }

    #[test]
    fn a_failover_stays_inside_a_country_pin() {
        let germany_down: Vec<RelayRow> = keyed()
            .into_iter()
            .map(|r| {
                let down = r.country == "DE" && r.exit_id != id(1);
                RelayRow { active: !down, ..r }
            })
            .collect();
        assert_eq!(failover(&country("DE"), 3, &germany_down), Some(0));
        assert_eq!(failover(&country("DE"), 1, &germany_down), None);
        assert_eq!(
            failover(&country("FR"), 4, &keyed()),
            None,
            "the only French exit failed"
        );
    }

    #[test]
    fn a_failover_stays_inside_a_city_pin() {
        assert_eq!(failover(&city("DE", "Frankfurt"), 2, &keyed()), Some(0));
        assert_eq!(failover(&city("DE", "Berlin"), 3, &keyed()), None);
    }

    #[test]
    fn a_single_exit_pin_leaves_no_failover_alternative() {
        assert_eq!(failover(&exit(3), 3, &keyed()), None);
    }

    #[test]
    fn an_automatic_pin_with_a_preferred_country_fails_over_inside_that_country() {
        assert_eq!(
            resolve_failover_exit(&ExitPinSpec::Automatic, Some("DE"), &keyed(), "k-3"),
            Some(1)
        );
        assert_eq!(
            resolve_failover_exit(&ExitPinSpec::Automatic, Some("FR"), &keyed(), "k-4"),
            None
        );
        assert_eq!(
            resolve_failover_exit(&ExitPinSpec::Automatic, Some("  "), &keyed(), "k-4"),
            Some(2),
            "a blank preferred country is no preference"
        );
    }

    #[test]
    fn a_failover_from_an_exit_the_catalogue_no_longer_lists_still_picks_in_scope() {
        assert_eq!(failover(&ExitPinSpec::Automatic, 99, &keyed()), Some(3));
    }

    #[test]
    fn a_failover_skips_inactive_exits() {
        // fr2 is heavier than fr1 and inactive: France has no spare.
        assert_eq!(failover(&country("FR"), 4, &catalogue()), None);
    }

    // The JNI contract: the exact bytes Kotlin sends and reads. The Kotlin
    // twin (`JniExitPinResolverTest`) pins the same request and answer.

    const PIN_COUNTRY_DE: &str = r#"{"kind":"country","country":"DE"}"#;
    const TWO_GERMAN_ROWS: &str = concat!(
        r#"[{"exit_id":"00000000000000000000000000000001","exit_pubkey_hex":"aa","endpoint":"10.0.0.1:443","country":"DE","city":"Frankfurt","active":true,"weight":10},"#,
        r#"{"exit_id":"00000000000000000000000000000002","exit_pubkey_hex":"bb","endpoint":"10.0.0.2:443","country":"DE","city":"Berlin","active":true,"weight":30}]"#
    );

    #[test]
    fn the_json_contract_answers_the_index_of_the_chosen_row() {
        assert_eq!(
            resolve_exit_pin_json(PIN_COUNTRY_DE, TWO_GERMAN_ROWS),
            r#"{"index":1}"#
        );
        assert_eq!(
            resolve_exit_pin_json(r#"{"kind":"automatic"}"#, TWO_GERMAN_ROWS),
            r#"{"index":null}"#
        );
        assert_eq!(
            resolve_exit_pin_json(
                r#"{"kind":"city","country":"de","city":"frankfurt"}"#,
                TWO_GERMAN_ROWS
            ),
            r#"{"index":0}"#
        );
        assert_eq!(
            resolve_exit_pin_json(
                r#"{"kind":"exit","exit_id":"00000000000000000000000000000002"}"#,
                TWO_GERMAN_ROWS
            ),
            r#"{"index":1}"#
        );
    }

    #[test]
    fn the_failover_json_contract_answers_the_alternative() {
        assert_eq!(
            resolve_failover_exit_json(r#"{"kind":"automatic"}"#, "", TWO_GERMAN_ROWS, "bb"),
            r#"{"index":0}"#
        );
        assert_eq!(
            resolve_failover_exit_json(PIN_COUNTRY_DE, "", TWO_GERMAN_ROWS, "aa"),
            r#"{"index":1}"#
        );
        assert_eq!(
            resolve_failover_exit_json(r#"{"kind":"automatic"}"#, "FR", TWO_GERMAN_ROWS, "aa"),
            r#"{"index":null}"#,
            "a preferred exit country scopes an automatic pin"
        );
    }

    #[test]
    fn the_automatic_json_contract_answers_the_shared_pick() {
        assert_eq!(
            resolve_automatic_exit_json("", TWO_GERMAN_ROWS),
            r#"{"index":1}"#,
            "the heaviest row, not the first one"
        );
        assert_eq!(
            resolve_automatic_exit_json("DE", TWO_GERMAN_ROWS),
            r#"{"index":1}"#
        );
        assert_eq!(
            resolve_automatic_exit_json("FR", TWO_GERMAN_ROWS),
            r#"{"index":1}"#,
            "a preference no active row satisfies still dials the fleet's heaviest"
        );
        assert_eq!(resolve_automatic_exit_json("", "[]"), r#"{"index":null}"#);
        assert_eq!(
            resolve_automatic_exit_json("", "{not json"),
            r#"{"index":null}"#
        );
    }

    #[test]
    fn an_unreadable_input_answers_no_index() {
        assert_eq!(
            resolve_exit_pin_json("{not json", TWO_GERMAN_ROWS),
            r#"{"index":null}"#
        );
        assert_eq!(
            resolve_exit_pin_json(r#"{"kind":"planet"}"#, TWO_GERMAN_ROWS),
            r#"{"index":null}"#
        );
        assert_eq!(
            resolve_exit_pin_json(PIN_COUNTRY_DE, r#"[{"exit_id":1}]"#),
            r#"{"index":null}"#
        );
        assert_eq!(
            resolve_failover_exit_json("{", "", TWO_GERMAN_ROWS, "aa"),
            r#"{"index":null}"#
        );
    }

    /// The shared crate's `exit_pick.json` vector, replayed through the JSON
    /// contract itself: each candidate becomes an active row of one country,
    /// the pin is that country, and the answer must name the vector's index.
    #[test]
    fn exit_vectors_replay_through_the_jni_contract() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../warren-contract/warren-discovery/tests/fixtures/exit_pick.json"
        ))
        .expect("exit_pick.json must parse");
        let cases = fixture["exit"].as_array().expect("exit section");
        assert!(cases.len() >= 8, "the exit section must keep its cases");
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            let rows: Vec<serde_json::Value> = case["candidates"]
                .as_array()
                .expect("candidates")
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    serde_json::json!({
                        "exit_id": c["exit_id"],
                        "exit_pubkey_hex": format!("{i:064x}"),
                        "endpoint": "198.51.100.1:443",
                        "country": "XX",
                        "city": "City",
                        "active": true,
                        "weight": c["weight"],
                    })
                })
                .collect();
            let relays_json = serde_json::Value::Array(rows).to_string();
            assert_eq!(
                resolve_exit_pin_json(r#"{"kind":"country","country":"xx"}"#, &relays_json),
                serde_json::json!({ "index": case["expected"] }).to_string(),
                "exit vector `{name}` diverged through the JNI contract"
            );
            assert_eq!(
                resolve_automatic_exit_json("XX", &relays_json),
                serde_json::json!({ "index": case["expected"] }).to_string(),
                "exit vector `{name}` diverged through the automatic JNI contract"
            );
        }
    }
}
