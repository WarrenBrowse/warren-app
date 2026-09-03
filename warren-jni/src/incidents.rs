//! Incident telemetry: the two fire-and-forget reports that make an outage a
//! client suffers visible in the operator's feed.
//!
//! Both are the desktop daemon's, ported rather than reinvented:
//! `mullvad-daemon/src/tunnel.rs` posts `/v1/incidents/exit-down` from the
//! failover assembly, and `lib.rs` posts `/v1/incidents/pubkey-mismatch` when
//! the user answers the pin-mismatch modal. Neither can affect the connection:
//! a failed POST costs one telemetry point and nothing else.
//!
//! The exit-down report rides a token bucket, because a client caught in a
//! failover loop otherwise reports every lap and fills the feed with copies of
//! one local outage (21 of the 25 reports on the 2026-08-27 feed were one
//! client flapping for two hours). A real exit outage stays visible: every
//! affected client reports under its own budget, so the per-exit count still
//! rises with the number of clients hit. Its constants are the daemon's, and
//! `fixtures/client-rules/incident_reports.json` holds both copies to the same
//! numbers.

use std::time::{Duration, Instant};

use warren_api::{IncidentExitDownRequest, IncidentPubkeyMismatchRequest, IncidentReason};

/// Reports allowed back-to-back before throttling kicks in.
const BURST: u32 = 3;

/// One token grows back per interval: 3 reports/hour sustained.
const REFILL_INTERVAL: Duration = Duration::from_secs(20 * 60);

/// Token bucket gating `POST /v1/incidents/exit-down`. One per process; state
/// is deliberately not persisted (a process restart is rare enough that the
/// fresh burst is noise-free).
#[derive(Debug)]
pub struct ExitDownReportBudget {
    tokens: u32,
    last_refill: Instant,
}

impl ExitDownReportBudget {
    #[must_use]
    pub fn new() -> Self {
        Self::starting_at(Instant::now())
    }

    /// Epoch-injected constructor so tests can align the refill boundaries
    /// with the instants they replay.
    fn starting_at(now: Instant) -> Self {
        Self {
            tokens: BURST,
            last_refill: now,
        }
    }

    /// Spends one token if available. `now` is a parameter so tests can drive
    /// time; call sites pass `Instant::now()`.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last_refill);
        let grown = elapsed.as_secs() / REFILL_INTERVAL.as_secs();
        if grown >= u64::from(BURST) {
            // Saturated: the sub-interval remainder is dropped on purpose, the
            // bucket cannot hold it anyway.
            self.tokens = BURST;
            self.last_refill = now;
        } else if grown > 0 {
            // `grown < BURST` here, so the cast and the Duration
            // multiplication cannot overflow.
            let grown = u32::try_from(grown).expect("grown < BURST fits u32");
            self.tokens = self.tokens.saturating_add(grown).min(BURST);
            self.last_refill += REFILL_INTERVAL * grown;
        }
        if self.tokens == 0 {
            return false;
        }
        self.tokens -= 1;
        true
    }
}

impl Default for ExitDownReportBudget {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a report did not leave. Coarse classes only: a gap in the operator feed
/// is diagnosed from the class, never from a value the report would have
/// carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotSent {
    /// The local token bucket refused this lap.
    Budget,
    /// The failure names no usable exit key, so the server would answer 400.
    Malformed,
    /// No wallet identity to sign the request with.
    Identity,
    /// The native runtime is not up yet (`initLogger` has not run).
    Runtime,
    /// The POST never reached the API.
    Transport,
    /// The API refused the body.
    Rejected,
}

impl NotSent {
    const fn class(self) -> &'static str {
        match self {
            Self::Budget => "budget",
            Self::Malformed => "malformed",
            Self::Identity => "identity",
            Self::Runtime => "runtime",
            Self::Transport => "transport",
            Self::Rejected => "rejected",
        }
    }
}

/// The `POST /v1/incidents/exit-down` body for an exit this client gave up on.
///
/// The reason is fixed at [`IncidentReason::HandshakeFail`], the daemon's
/// choice at the same call site: the retry path knows the previous attempt
/// failed and nothing finer, and exit-down surfaces as a pre-tunnel exchange
/// failure.
///
/// # Errors
///
/// [`NotSent::Malformed`] when `exit_pubkey_hex` is not 64 hex characters.
pub fn exit_down_request(
    exit_pubkey_hex: &str,
    ts_unix: u64,
) -> Result<IncidentExitDownRequest, NotSent> {
    let exit_pubkey_hex =
        warren_api::PubkeyHex::try_from(exit_pubkey_hex).map_err(|_| NotSent::Malformed)?;
    Ok(IncidentExitDownRequest {
        exit_pubkey_hex,
        reason_code: IncidentReason::HandshakeFail,
        ts_unix,
    })
}

/// The `POST /v1/incidents/pubkey-mismatch` body for a pin the TOFU check
/// refused. Every field is already public through the signed relay list, and
/// the server records no signer, so the report says what changed without
/// saying who saw it. The observed value is passed through unvalidated on
/// purpose: a report whose observed key is garbage served by a MITM is exactly
/// the report worth receiving.
#[must_use]
pub fn pubkey_mismatch_request(
    exit_id_hex: &str,
    old_pubkey_hex: &str,
    new_pubkey_hex: &str,
    country_code: &str,
    city: &str,
    ts_unix: u64,
) -> IncidentPubkeyMismatchRequest {
    IncidentPubkeyMismatchRequest {
        exit_id_hex: exit_id_hex.to_owned(),
        old_pubkey_hex: old_pubkey_hex.to_owned(),
        new_pubkey_hex: new_pubkey_hex.to_owned(),
        country_code: country_code.to_owned(),
        city: city.to_owned(),
        ts_unix,
    }
}

/// The envelope the JNI export answers: whether the report left, and if not
/// the class that says so.
#[must_use]
pub fn envelope(outcome: Result<(), NotSent>) -> String {
    match outcome {
        Ok(()) => r#"{"ok":true}"#.to_owned(),
        Err(reason) => format!(r#"{{"ok":false,"reason":"{}"}}"#, reason.class()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{ExitDownReportBudget, NotSent, envelope, exit_down_request};

    fn fixture() -> serde_json::Value {
        let path = format!(
            "{}/../fixtures/client-rules/incident_reports.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
        serde_json::from_str(&raw).expect("incident_reports.json parses")
    }

    /// The whole budget rule, replayed from the file the desktop daemon's own
    /// copy is held to. Each storm is a burst pattern with the number of
    /// reports that may leave it, so the constants are pinned through the
    /// behaviour they produce rather than asserted twice.
    #[test]
    fn the_storms_of_the_shared_client_rule_replay() {
        let rule = fixture();
        let budget_rule = &rule["exit_down"]["budget"];
        for storm in budget_rule["storms"]
            .as_array()
            .expect("storms is an array")
        {
            let name = storm["name"].as_str().expect("a storm has a name");
            let t0 = Instant::now();
            let mut budget = ExitDownReportBudget::starting_at(t0);
            let mut sent = 0_u64;
            for offset in storm_offsets(storm) {
                if budget.try_acquire(t0 + Duration::from_secs(offset)) {
                    sent += 1;
                }
            }
            assert_eq!(
                sent,
                storm["sent"].as_u64().expect("a storm names its count"),
                "storm {name}"
            );
        }
    }

    fn storm_offsets(storm: &serde_json::Value) -> Vec<u64> {
        if let Some(list) = storm["attempts_secs"].as_array() {
            return list
                .iter()
                .map(|v| v.as_u64().expect("an attempt offset is a number"))
                .collect();
        }
        let start = storm["start_secs"].as_u64().expect("start_secs");
        let period = storm["period_secs"].as_u64().expect("period_secs");
        let attempts = storm["attempts"].as_u64().expect("attempts");
        (0..attempts).map(|lap| start + period * lap).collect()
    }

    /// The body a client builds for the exit it just gave up on: that exit's
    /// key, the reason the shared rule fixes, and the moment it happened.
    #[test]
    fn the_exit_down_body_names_the_dead_exit_and_the_ruled_reason() {
        let dead = "ab".repeat(32);
        let req = exit_down_request(&dead, 1_756_000_000).expect("a 64-hex key builds a report");

        assert_eq!(req.exit_pubkey_hex.as_str(), dead);
        assert_eq!(req.ts_unix, 1_756_000_000);
        let ruled = fixture()["exit_down"]["reason_code"]
            .as_str()
            .expect("the rule names a reason code")
            .to_owned();
        assert_eq!(
            serde_json::to_value(req.reason_code).expect("the reason serialises"),
            serde_json::Value::String(ruled),
        );
    }

    /// A key that is not 64 hex characters is a client bug, and the server
    /// answers 400: the report is dropped here rather than posted.
    #[test]
    fn a_malformed_exit_key_builds_no_report() {
        assert_eq!(
            exit_down_request("not-a-key", 1).err(),
            Some(NotSent::Malformed)
        );
        assert_eq!(exit_down_request("", 1).err(), Some(NotSent::Malformed));
    }

    /// The mismatch body is pure forensics: both keys under one exit id, plus
    /// the location the relay list already publishes.
    #[test]
    fn the_mismatch_body_carries_both_keys_and_the_published_location() {
        let req = super::pubkey_mismatch_request(
            &"1a".repeat(16),
            &"ab".repeat(32),
            &"cd".repeat(32),
            "nl",
            "Amsterdam",
            1_756_000_001,
        );

        assert_eq!(req.exit_id_hex, "1a".repeat(16));
        assert_eq!(req.old_pubkey_hex, "ab".repeat(32));
        assert_eq!(req.new_pubkey_hex, "cd".repeat(32));
        assert_eq!(req.country_code, "nl");
        assert_eq!(req.city, "Amsterdam");
        assert_eq!(req.ts_unix, 1_756_000_001);
    }

    /// The envelope Kotlin reads: whether the report left, and if not the
    /// class that says so. No value the report carried is ever in it.
    #[test]
    fn the_envelope_states_the_outcome_as_a_class() {
        assert_eq!(envelope(Ok(())), r#"{"ok":true}"#);
        for (reason, class) in [
            (NotSent::Budget, "budget"),
            (NotSent::Malformed, "malformed"),
            (NotSent::Identity, "identity"),
            (NotSent::Runtime, "runtime"),
            (NotSent::Transport, "transport"),
            (NotSent::Rejected, "rejected"),
        ] {
            assert_eq!(
                envelope(Err(reason)),
                format!(r#"{{"ok":false,"reason":"{class}"}}"#)
            );
        }
    }
}
