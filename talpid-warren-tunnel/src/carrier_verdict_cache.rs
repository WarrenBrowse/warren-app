//! Persisted per-network verdicts for the macOS carrier egress guard.
//!
//! # Why (avoid re-proving the blackhole on every connect)
//!
//! On a host where the `IP_BOUND_IF` bind black-holes (the multi-interface
//! shape), the bootstrap egress guard re-proves the same
//! blackhole on EVERY connect, spending its full adaptive dead window
//! (1.4-2.2 s) in the connect critical path before reverting to the `/32`
//! escape, so a connect that should take ~200 ms takes seconds every time. Whether
//! the bind egresses is a property of the NETWORK the host sits on, not of
//! the individual connect attempt, so the guard's verdict is cached per
//! network fingerprint (interface + gateway) and replayed:
//!
//! - [`CachedVerdict::RouteOnly`] hit: skip the bind, pre-install the `/32`
//!   escape; the connect is instant and the escape is verified after `Up`
//!   like any other configuration. It used to be trusted without measurement,
//!   which disarmed the guard on precisely the hosts it had already fired on:
//!   when the escape was dead too, the tunnel reported Connected over a
//!   carrier that reached nothing and nothing in the log said so.
//! - anything else (fresh [`CachedVerdict::BindOk`], miss, expired): keep
//!   the bind and run the guard AFTER `Up`, in the background. The connect
//!   never waits on the guard; on a black-holing network the background
//!   guard self-heals to the `/32` escape within its window (~1.5-2 s of
//!   dead egress right after `Up`, once per network per [`VERDICT_TTL`])
//!   and records `RouteOnly` so the next connect skips the bind outright.
//!   Claiming `Up` before egress is proven is acceptable ONLY because the
//!   self-heal plus the dead-datapath escalation backstop it; the
//!   pathology was the absence of both, not the optimism itself.
//!
//! Entries expire after [`VERDICT_TTL`] so a `RouteOnly` network re-earns the
//! leak-free bind periodically instead of paying the `/32` ServerIP exception
//! forever on a topology that may have been fixed. That expiry only happens
//! because the post-`Up` escape verification of a REPLAYED verdict leaves the
//! entry's timestamp alone: it measures the escape, not the bind, so renewing
//! the entry from it would keep any weekly-used network pinned to the escape
//! for good.
//!
//! The fingerprint is stored as a stable FNV-1a hash rather than the raw
//! interface/address strings. This keeps casual reads of the cache file from
//! showing network identifiers, but an unkeyed 64-bit hash over low-entropy
//! inputs is dictionary-invertible: treat the file as mildly obfuscated, not
//! anonymized.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::carrier_egress_guard::GuardOutcome;

/// Lifetime of a cached verdict. Long enough that a stable home/office
/// network pays the blocking probe once a week at most; short enough that a
/// `RouteOnly` network gets periodic chances to graduate back to the bind.
pub(crate) const VERDICT_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// Cap on stored networks; the oldest entries are dropped past it.
const MAX_ENTRIES: usize = 16;

/// v2: the fingerprint dropped the local IP (see [`network_fingerprint`]),
/// so v1 entries hash a different input and must not be read as v2.
const FILE_NAME: &str = "carrier-egress-verdicts.v2";

/// A remembered guard outcome for one network fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachedVerdict {
    /// The bound carrier egressed on this network: keep the bind, verify in
    /// the background.
    BindOk,
    /// The bind black-holed on this network: skip it and pre-install the
    /// `/32` escape.
    RouteOnly,
}

impl CachedVerdict {
    fn as_str(self) -> &'static str {
        match self {
            CachedVerdict::BindOk => "bind-ok",
            CachedVerdict::RouteOnly => "route-only",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "bind-ok" => Some(CachedVerdict::BindOk),
            "route-only" => Some(CachedVerdict::RouteOnly),
            _ => None,
        }
    }
}

/// How the connect path should treat the carrier bind on this network.
pub(crate) enum CarrierProbePlan {
    /// Fresh `BindOk` or no fresh verdict: bind, verify AFTER `Up` in the
    /// background; a `RevertedToRoute` re-verdict self-heals the session
    /// and the cache.
    Background(VerdictRecorder),
    /// Fresh `RouteOnly`: the bind is skipped and the `/32` escape is
    /// pre-installed. The escape is still VERIFIED, because a verdict that
    /// stops being measured stops being true: the fingerprint is interface
    /// plus gateway, so it follows the host onto networks where the escape may
    /// carry nothing.
    SkipRouteOnly(VerdictRecorder),
    /// No default route to bind to: `/32` fallback, nothing to verify.
    NoBind,
}

/// Decide the probe plan for the network identified by `fingerprint`.
pub(crate) fn plan_carrier_probe(
    fingerprint: String,
    verdict_dir: Option<&Path>,
) -> CarrierProbePlan {
    let cache = VerdictCache::load(verdict_dir);
    match cache.lookup(&fingerprint, now_unix()) {
        Some(CachedVerdict::RouteOnly) => CarrierProbePlan::SkipRouteOnly(VerdictRecorder {
            cache,
            fingerprint,
            replayed_route_only: true,
        }),
        Some(CachedVerdict::BindOk) | None => CarrierProbePlan::Background(VerdictRecorder {
            cache,
            fingerprint,
            replayed_route_only: false,
        }),
    }
}

/// What the cache remembers for `fingerprint` right now: the verdict and how
/// many seconds ago it was recorded. `None` when nothing is stored for that
/// network or the entry has aged past [`VERDICT_TTL`].
///
/// Strictly read-only, for the diagnostics surface. It must never renew, prune
/// or rewrite an entry: an observation that changes what it observes would make
/// the expiry unreachable all over again.
pub(crate) fn peek_verdict(
    fingerprint: &str,
    verdict_dir: Option<&Path>,
) -> Option<(CachedVerdict, u64)> {
    let now = now_unix();
    VerdictCache::load(verdict_dir)
        .entries
        .iter()
        .find(|e| e.fingerprint == fingerprint)
        .map(|e| (e.verdict, now.saturating_sub(e.recorded_unix)))
        .filter(|(_, age)| *age < VERDICT_TTL.as_secs())
}

/// One-shot writer that maps a guard outcome back into the cache.
pub(crate) struct VerdictRecorder {
    cache: VerdictCache,
    fingerprint: String,
    /// The plan REPLAYED a cached `RouteOnly` verdict, so the guard that
    /// follows measures the escape and never touches the bind.
    replayed_route_only: bool,
}

impl VerdictRecorder {
    pub(crate) fn record(mut self, outcome: GuardOutcome) {
        let verdict = match outcome {
            GuardOutcome::BypassConfirmed => CachedVerdict::BindOk,
            // A replayed plan re-proves the ESCAPE, which is no evidence about
            // the bind, and [`VerdictCache::record`] re-stamps `recorded_unix`.
            // Writing it back would push the expiry forward on every connect,
            // so a network used at least once a week would never age out and
            // the host would keep the wider `/32` escape forever. Leave the
            // original entry alone and let it expire on schedule.
            GuardOutcome::RevertedToRoute if self.replayed_route_only => return,
            GuardOutcome::RevertedToRoute => CachedVerdict::RouteOnly,
            // Neither configuration egresses. `RouteOnly` would be a lie that
            // outlives the connect and skips the guard for a whole TTL, so the
            // network is forgotten instead and the next connect measures from
            // scratch, bind included.
            GuardOutcome::EscapeAlsoDead => {
                self.cache.forget(&self.fingerprint);
                return;
            }
            // No positive evidence either way: leave the cache alone so the
            // next connect probes again.
            GuardOutcome::Inconclusive => return,
        };
        self.cache.record(&self.fingerprint, verdict, now_unix());
    }
}

/// Stable fingerprint of the network the default route points at. FNV-1a
/// 64-bit rather than `DefaultHasher`: the value must survive process
/// restarts and Rust upgrades to be worth persisting.
///
/// Deliberately excludes the local IP: a DHCP renewal on the SAME network
/// would re-fingerprint it and re-pay the probe. A same-(interface, gateway)
/// collision between two different networks misclassifies benignly: every
/// plan either self-heals in the background or falls back to the proven
/// `/32` escape.
pub(crate) fn network_fingerprint(interface: &str, gateway: IpAddr) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{interface}|{gateway}").bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    fingerprint: String,
    verdict: CachedVerdict,
    recorded_unix: u64,
}

/// The on-disk verdict store. All I/O is best-effort: a missing or corrupt
/// file degrades to an empty cache (one extra blocking probe), never an error.
#[derive(Debug)]
pub(crate) struct VerdictCache {
    path: Option<PathBuf>,
    entries: Vec<Entry>,
}

impl VerdictCache {
    pub(crate) fn load(dir: Option<&Path>) -> Self {
        let path = dir.map(|d| d.join(FILE_NAME));
        let mut entries = Vec::new();
        if let Some(p) = &path
            && let Ok(contents) = std::fs::read_to_string(p)
        {
            for line in contents.lines() {
                let mut parts = line.split_whitespace();
                if let (Some(fp), Some(v), Some(ts)) = (parts.next(), parts.next(), parts.next())
                    && let Some(verdict) = CachedVerdict::parse(v)
                    && let Ok(recorded_unix) = ts.parse::<u64>()
                {
                    entries.push(Entry {
                        fingerprint: fp.to_string(),
                        verdict,
                        recorded_unix,
                    });
                }
            }
        }
        Self { path, entries }
    }

    pub(crate) fn lookup(&self, fingerprint: &str, now_unix: u64) -> Option<CachedVerdict> {
        self.entries
            .iter()
            .find(|e| e.fingerprint == fingerprint)
            .filter(|e| now_unix.saturating_sub(e.recorded_unix) < VERDICT_TTL.as_secs())
            .map(|e| e.verdict)
    }

    pub(crate) fn record(&mut self, fingerprint: &str, verdict: CachedVerdict, now_unix: u64) {
        self.entries.retain(|e| e.fingerprint != fingerprint);
        self.entries.push(Entry {
            fingerprint: fingerprint.to_string(),
            verdict,
            recorded_unix: now_unix,
        });
        if self.entries.len() > MAX_ENTRIES {
            self.entries
                .sort_by_key(|e| std::cmp::Reverse(e.recorded_unix));
            self.entries.truncate(MAX_ENTRIES);
        }
        self.persist();
    }

    /// Drop whatever is remembered for `fingerprint`, so the next lookup is a
    /// miss and the caller measures instead of replaying.
    pub(crate) fn forget(&mut self, fingerprint: &str) {
        let before = self.entries.len();
        self.entries.retain(|e| e.fingerprint != fingerprint);
        if self.entries.len() != before {
            self.persist();
        }
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!(
                "{} {} {}\n",
                e.fingerprint,
                e.verdict.as_str(),
                e.recorded_unix
            ));
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(path, out) {
            log::debug!("carrier verdict cache: persist failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("warren-verdict-cache-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lookup_returns_the_recorded_verdict_before_ttl() {
        let mut cache = VerdictCache::load(None);
        cache.record("fp1", CachedVerdict::RouteOnly, 1_000);
        assert_eq!(
            cache.lookup("fp1", 1_000 + VERDICT_TTL.as_secs() - 1),
            Some(CachedVerdict::RouteOnly)
        );
    }

    #[test]
    fn lookup_expires_after_ttl() {
        let mut cache = VerdictCache::load(None);
        cache.record("fp1", CachedVerdict::RouteOnly, 1_000);
        assert_eq!(cache.lookup("fp1", 1_000 + VERDICT_TTL.as_secs()), None);
    }

    #[test]
    fn record_overwrites_the_previous_verdict_for_the_same_network() {
        let mut cache = VerdictCache::load(None);
        cache.record("fp1", CachedVerdict::RouteOnly, 1_000);
        cache.record("fp1", CachedVerdict::BindOk, 2_000);
        assert_eq!(cache.lookup("fp1", 2_001), Some(CachedVerdict::BindOk));
    }

    #[test]
    fn cache_roundtrips_through_disk() {
        let dir = tmp_dir("roundtrip");
        let mut cache = VerdictCache::load(Some(&dir));
        cache.record("fp1", CachedVerdict::RouteOnly, 1_000);
        cache.record("fp2", CachedVerdict::BindOk, 2_000);
        let reloaded = VerdictCache::load(Some(&dir));
        assert_eq!(
            reloaded.lookup("fp1", 1_001),
            Some(CachedVerdict::RouteOnly)
        );
        assert_eq!(reloaded.lookup("fp2", 2_001), Some(CachedVerdict::BindOk));
    }

    #[test]
    fn corrupted_lines_are_ignored_on_load() {
        let dir = tmp_dir("corrupt");
        std::fs::write(
            dir.join(FILE_NAME),
            "garbage\nfp1 route-only not-a-number\nfp2 bind-ok 2000\nfp3 nonsense 3000\n",
        )
        .unwrap();
        let cache = VerdictCache::load(Some(&dir));
        assert_eq!(cache.lookup("fp1", 1_001), None);
        assert_eq!(cache.lookup("fp2", 2_001), Some(CachedVerdict::BindOk));
        assert_eq!(cache.lookup("fp3", 3_001), None);
    }

    #[test]
    fn prune_keeps_only_the_most_recent_networks() {
        let mut cache = VerdictCache::load(None);
        for i in 0..(MAX_ENTRIES as u64 + 4) {
            cache.record(&format!("fp{i}"), CachedVerdict::BindOk, 1_000 + i);
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert_eq!(cache.lookup("fp0", 1_100), None, "oldest entry pruned");
        assert!(
            cache
                .lookup(&format!("fp{}", MAX_ENTRIES + 3), 1_100)
                .is_some()
        );
    }

    #[test]
    fn fingerprint_changes_with_every_network_component() {
        let gw1: IpAddr = "192.168.1.254".parse().unwrap();
        let gw2: IpAddr = "192.168.1.1".parse().unwrap();
        let base = network_fingerprint("en8", gw1);
        assert_ne!(base, network_fingerprint("en0", gw1));
        assert_ne!(base, network_fingerprint("en8", gw2));
        assert_eq!(base, network_fingerprint("en8", gw1));
    }

    #[test]
    fn plan_defers_verify_to_background_on_a_cache_miss_and_replays_fresh_verdicts() {
        let dir = tmp_dir("plan");
        let fp = network_fingerprint("en8", "192.168.1.254".parse().unwrap());

        let plan = plan_carrier_probe(fp.clone(), Some(&dir));
        let CarrierProbePlan::Background(recorder) = plan else {
            panic!("cache miss must keep the bind and verify in the background");
        };
        recorder.record(GuardOutcome::RevertedToRoute);

        assert!(matches!(
            plan_carrier_probe(fp.clone(), Some(&dir)),
            CarrierProbePlan::SkipRouteOnly(_)
        ));

        let mut cache = VerdictCache::load(Some(&dir));
        cache.record(&fp, CachedVerdict::BindOk, now_unix());
        assert!(matches!(
            plan_carrier_probe(fp, Some(&dir)),
            CarrierProbePlan::Background(_)
        ));
    }

    #[test]
    fn inconclusive_outcomes_never_touch_the_cache() {
        let dir = tmp_dir("inconclusive");
        let CarrierProbePlan::Background(recorder) = plan_carrier_probe("fp1".into(), Some(&dir))
        else {
            panic!("cache miss must keep the bind and verify in the background");
        };
        recorder.record(GuardOutcome::Inconclusive);
        assert_eq!(
            VerdictCache::load(Some(&dir)).lookup("fp1", now_unix()),
            None
        );
    }

    #[test]
    fn an_escape_that_is_dead_too_is_forgotten_rather_than_recorded() {
        // Caching `RouteOnly` for a host that egresses through neither
        // configuration pins the dead one for a whole TTL and skips the guard
        // on every later connect. Forgetting the network instead is what lets
        // the next connect re-arm the bind and measure again.
        let dir = tmp_dir("escape-dead");
        let fp = "fp1".to_string();
        let CarrierProbePlan::Background(recorder) = plan_carrier_probe(fp.clone(), Some(&dir))
        else {
            panic!("cache miss must keep the bind and verify in the background");
        };
        recorder.record(GuardOutcome::RevertedToRoute);
        assert!(matches!(
            plan_carrier_probe(fp.clone(), Some(&dir)),
            CarrierProbePlan::SkipRouteOnly(_)
        ));

        let CarrierProbePlan::SkipRouteOnly(recorder) = plan_carrier_probe(fp.clone(), Some(&dir))
        else {
            panic!("a fresh RouteOnly verdict must skip the bind");
        };
        recorder.record(GuardOutcome::EscapeAlsoDead);

        assert_eq!(VerdictCache::load(Some(&dir)).lookup(&fp, now_unix()), None);
        assert!(
            matches!(
                plan_carrier_probe(fp, Some(&dir)),
                CarrierProbePlan::Background(_)
            ),
            "the next connect must re-arm the bind instead of replaying a dead verdict"
        );
    }

    /// A `SkipRouteOnly` connect measures the ESCAPE, not the bind, so its
    /// `RevertedToRoute` carries no news about the bind. Re-stamping the entry
    /// from it pushes the expiry forward on every connect, and a network used
    /// at least once a week then never ages out: the host keeps the wider `/32`
    /// escape forever and the bind is never retried.
    #[test]
    fn a_replayed_route_only_verdict_keeps_its_original_age() {
        let dir = tmp_dir("no-restamp");
        let recorded = now_unix() - VERDICT_TTL.as_secs() + 30;
        let mut cache = VerdictCache::load(Some(&dir));
        cache.record("fp1", CachedVerdict::RouteOnly, recorded);

        let CarrierProbePlan::SkipRouteOnly(recorder) =
            plan_carrier_probe("fp1".into(), Some(&dir))
        else {
            panic!("a fresh RouteOnly verdict must skip the bind");
        };
        recorder.record(GuardOutcome::RevertedToRoute);

        assert_eq!(
            VerdictCache::load(Some(&dir)).lookup("fp1", now_unix() + 60),
            None,
            "a re-proved escape must not renew the entry, or it never expires"
        );
    }

    /// The other half of the same rule: a `Background` plan DID measure the
    /// bind, so its verdict is fresh evidence and must reset the clock.
    #[test]
    fn a_measured_bind_re_stamps_the_entry() {
        let dir = tmp_dir("restamp");
        let recorded = now_unix() - VERDICT_TTL.as_secs() + 30;
        let mut cache = VerdictCache::load(Some(&dir));
        cache.record("fp1", CachedVerdict::BindOk, recorded);

        let CarrierProbePlan::Background(recorder) = plan_carrier_probe("fp1".into(), Some(&dir))
        else {
            panic!("a BindOk verdict must keep the bind and verify in the background");
        };
        recorder.record(GuardOutcome::RevertedToRoute);

        assert_eq!(
            VerdictCache::load(Some(&dir)).lookup("fp1", now_unix() + 60),
            Some(CachedVerdict::RouteOnly),
            "a fresh bind measurement must reset the entry's clock"
        );
    }

    #[test]
    fn a_cached_route_only_network_still_gets_a_recorder_to_verify_with() {
        // The verdict has to stay a measurement: a `SkipRouteOnly` plan that
        // carried no recorder is exactly what disarmed the guard for good.
        let dir = tmp_dir("route-only-recorder");
        let mut cache = VerdictCache::load(Some(&dir));
        cache.record("fp1", CachedVerdict::RouteOnly, now_unix());
        let CarrierProbePlan::SkipRouteOnly(recorder) =
            plan_carrier_probe("fp1".into(), Some(&dir))
        else {
            panic!("a fresh RouteOnly verdict must skip the bind");
        };
        recorder.record(GuardOutcome::RevertedToRoute);
        assert_eq!(
            VerdictCache::load(Some(&dir)).lookup("fp1", now_unix()),
            Some(CachedVerdict::RouteOnly),
            "a re-proved escape keeps its verdict"
        );
    }

    #[test]
    fn peek_reports_the_verdict_with_its_age_and_never_touches_the_entry() {
        let dir = tmp_dir("peek");
        let recorded = now_unix() - 3_600;
        let mut cache = VerdictCache::load(Some(&dir));
        cache.record("fp1", CachedVerdict::RouteOnly, recorded);

        let (verdict, age) = peek_verdict("fp1", Some(&dir)).expect("a fresh entry must be seen");
        assert_eq!(verdict, CachedVerdict::RouteOnly);
        assert!((3_600..3_610).contains(&age), "age was {age}");

        peek_verdict("fp1", Some(&dir));
        let (_, age_after) = peek_verdict("fp1", Some(&dir)).expect("still there");
        assert!(
            age_after >= 3_600,
            "reading the cache must not renew the entry"
        );
    }

    #[test]
    fn peek_reports_nothing_for_an_unknown_or_expired_network() {
        let dir = tmp_dir("peek-miss");
        assert_eq!(peek_verdict("fp1", Some(&dir)), None);

        let mut cache = VerdictCache::load(Some(&dir));
        cache.record(
            "fp1",
            CachedVerdict::BindOk,
            now_unix() - VERDICT_TTL.as_secs() - 1,
        );
        assert_eq!(peek_verdict("fp1", Some(&dir)), None);
    }

    #[test]
    fn legacy_v1_entries_are_not_read() {
        let dir = tmp_dir("legacy");
        std::fs::write(dir.join("carrier-egress-verdicts.v1"), "fp1 bind-ok 2000\n").unwrap();
        assert_eq!(VerdictCache::load(Some(&dir)).lookup("fp1", 2_001), None);
    }
}
