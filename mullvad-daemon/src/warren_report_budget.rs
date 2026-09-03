//! Local budget for the fire-and-forget exit-down reports.
//!
//! A client caught in a failover loop otherwise reports every lap and
//! fills the operator's incident feed with copies of the same local
//! outage (21 of the 25 reports on the 2026-08-27 feed were one client
//! flapping for two hours). The budget lets the first reports of an
//! incident through and throttles the rest. A real exit outage stays
//! visible: every affected client reports under its own budget, so the
//! per-exit count still rises with the number of clients hit.

use std::time::{Duration, Instant};

/// Reports allowed back-to-back before throttling kicks in.
const BURST: u32 = 3;

/// One token grows back per interval: 3 reports/hour sustained.
const REFILL_INTERVAL: Duration = Duration::from_secs(20 * 60);

/// Token bucket gating `POST /v1/incidents/exit-down`. One per daemon
/// run; state is deliberately not persisted (a restart is rare enough
/// that the fresh burst is noise-free).
#[derive(Debug)]
pub(crate) struct ExitDownReportBudget {
    tokens: u32,
    last_refill: Instant,
}

impl ExitDownReportBudget {
    pub(crate) fn new() -> Self {
        Self::starting_at(Instant::now())
    }

    /// Epoch-injected constructor so tests can align the refill
    /// boundaries with the instants they replay.
    fn starting_at(now: Instant) -> Self {
        Self {
            tokens: BURST,
            last_refill: now,
        }
    }

    /// Spends one token if available. `now` is a parameter so tests can
    /// drive time; call sites pass `Instant::now()`.
    pub(crate) fn try_acquire(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last_refill);
        let grown = elapsed.as_secs() / REFILL_INTERVAL.as_secs();
        if grown >= u64::from(BURST) {
            // Saturated: the sub-interval remainder is dropped on
            // purpose, the bucket cannot hold it anyway.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared client rule both clients are held to. Android carries its
    /// own copy of this bucket (`warren-jni/src/incidents.rs`), so the
    /// constants live in one file and each copy replays it.
    fn client_rule() -> serde_json::Value {
        let path = format!(
            "{}/../fixtures/client-rules/incident_reports.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
        serde_json::from_str(&raw).expect("incident_reports.json parses")
    }

    /// The one file both clients read. Android carries its own copy of this
    /// bucket (`warren-jni/src/incidents.rs`), and the storm outcomes below
    /// are a pure function of these two numbers, so holding them to the
    /// shared rule is what keeps the two copies from drifting apart.
    #[test]
    fn the_bucket_is_the_one_the_shared_client_rule_states() {
        let rule = client_rule();
        let budget_rule = &rule["exit_down"]["budget"];
        assert_eq!(budget_rule["burst"].as_u64(), Some(u64::from(BURST)));
        assert_eq!(
            budget_rule["refill_interval_secs"].as_u64(),
            Some(REFILL_INTERVAL.as_secs())
        );
    }

    #[test]
    fn burst_of_three_passes_then_refuses() {
        let t0 = Instant::now();
        let mut budget = ExitDownReportBudget::starting_at(t0);
        for lap in 0..3 {
            assert!(
                budget.try_acquire(t0),
                "report {lap} of the initial burst must pass"
            );
        }
        assert!(
            !budget.try_acquire(t0),
            "the fourth back-to-back report must be throttled"
        );
    }

    #[test]
    fn a_token_grows_back_after_the_refill_interval() {
        let t0 = Instant::now();
        let mut budget = ExitDownReportBudget::starting_at(t0);
        for _ in 0..3 {
            assert!(budget.try_acquire(t0));
        }
        assert!(!budget.try_acquire(t0 + Duration::from_secs(19 * 60)));
        assert!(
            budget.try_acquire(t0 + Duration::from_secs(20 * 60 + 1)),
            "one token must grow back after the refill interval"
        );
        assert!(
            !budget.try_acquire(t0 + Duration::from_secs(20 * 60 + 2)),
            "and only one"
        );
    }

    #[test]
    fn a_two_hour_flap_storm_is_capped() {
        // The 2026-08-27 signature: a failover lap every 150 s for two
        // hours produced 21 reports. Under the budget the same storm
        // sends the 3-report burst plus one report per refill interval.
        let t0 = Instant::now();
        let mut budget = ExitDownReportBudget::starting_at(t0);
        let mut sent = 0;
        for lap in 0..=48 {
            if budget.try_acquire(t0 + Duration::from_secs(150 * lap)) {
                sent += 1;
            }
        }
        assert_eq!(
            sent, 9,
            "a 2 h storm at one lap per 150 s must send burst (3) + refills (6), not every lap"
        );
    }

    #[test]
    fn long_idle_does_not_stockpile_beyond_the_burst() {
        let t0 = Instant::now();
        let mut budget = ExitDownReportBudget::starting_at(t0);
        let after_a_day = t0 + Duration::from_secs(24 * 60 * 60);
        for lap in 0..3 {
            assert!(
                budget.try_acquire(after_a_day),
                "report {lap} after a long idle must pass"
            );
        }
        assert!(
            !budget.try_acquire(after_a_day),
            "idle time must not stockpile tokens beyond the burst"
        );
    }
}
