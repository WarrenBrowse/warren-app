//! In-tunnel egress liveness probe (doc 62 item 5).
//!
//! RX-silence detection (`session_liveness`, the supervisor's dead-path
//! watch) only sees the QUIC transport: an exit that is drained or
//! half-swapped during a fleet rollout keeps ACKing keep-alives, so the
//! session never looks dead while the exit forwards NOTHING and the UI
//! shows "Connected" with zero actual internet (field incident,
//! 2026-07-09). This probe closes that gap by exercising the datapath
//! end to end: a periodic DNS query THROUGH the tunnel to the
//! exit-provided resolver (the tunnel gateway, the same server the
//! system DNS uses while connected). Any answer proves the exit
//! decapsulates, forwards and can reach its upstream; the firewall's
//! connected policy explicitly allows port 53 to the configured
//! in-tunnel resolver on every platform, and the gateway address is
//! only routable via the TUN, so the probe can never leak outside the
//! tunnel.
//!
//! Escalation is debounced: [`EgressProbeConfig::failure_threshold`]
//! consecutive failures publish an "egress dead" verdict through the
//! daemon callback (surfaced as `exit_egress_dead` in the status
//! cache); one success clears it. A rollout hot-swap blip (~1-2 s)
//! never reaches the threshold. While the supervisor has no published
//! session the probe is skipped entirely (the RX-silence machinery owns
//! that case) and the failure count resets.
//!
//! When the verdict fires while a drain advisory is active, the probe
//! prefers the drain reactor's gap-free migration hook
//! (`warren_drain_migrate`) over just bannering; otherwise it only
//! publishes and lets `session_liveness` / the dead-path escalation do
//! their job (no duplicated reconnect logic).

use std::net::SocketAddr;
use std::time::Duration;

/// Disable knob: `WARREN_EGRESS_PROBE=0` turns the probe off.
pub const EGRESS_PROBE_ENV: &str = "WARREN_EGRESS_PROBE";
/// Probe cadence in seconds (jittered +/-15% per tick).
pub const EGRESS_PROBE_INTERVAL_ENV: &str = "WARREN_EGRESS_PROBE_INTERVAL_SECS";
/// Consecutive failures before the egress-dead verdict.
pub const EGRESS_PROBE_FAILURES_ENV: &str = "WARREN_EGRESS_PROBE_FAILURES";

const DEFAULT_INTERVAL: Duration = Duration::from_secs(25);
const INTERVAL_RANGE_SECS: std::ops::RangeInclusive<u64> = 5..=600;
const DEFAULT_FAILURE_THRESHOLD: u32 = 3;
const FAILURE_RANGE: std::ops::RangeInclusive<u32> = 1..=10;

/// Exit-provided in-tunnel resolver: the fleet-invariant tunnel gateway
/// (`warrenguard_config::TUNNEL_GATEWAY_IP`), same address the system
/// DNS points at while connected, port 53.
const GATEWAY_DNS: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 66, 0, 1)),
    53,
);

/// Name resolved by the probe. Warren infrastructure, queried against
/// Warren's own exit resolver: no third party learns anything.
const PROBE_QNAME: &str = "warrenbrowse.com";

/// Overall wait for an answer within one probe (two sends inside).
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
/// Retransmit offset of the second datagram inside one probe, so a
/// single lost UDP packet does not count as an egress failure.
const PROBE_RETRANSMIT: Duration = Duration::from_secs(2);

/// Resolved probe settings (env knobs applied once at tunnel start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EgressProbeConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub failure_threshold: u32,
}

impl EgressProbeConfig {
    pub fn from_env() -> Self {
        Self::resolve(
            std::env::var(EGRESS_PROBE_ENV).ok().as_deref(),
            std::env::var(EGRESS_PROBE_INTERVAL_ENV).ok().as_deref(),
            std::env::var(EGRESS_PROBE_FAILURES_ENV).ok().as_deref(),
        )
    }

    /// Pure resolution so the knob semantics are unit-testable:
    /// invalid or out-of-range values warn and keep the default
    /// (mirrors `WARREN_N_CONNECTIONS`, never clamps silently).
    fn resolve(enable: Option<&str>, interval: Option<&str>, failures: Option<&str>) -> Self {
        let enabled = enable.map(str::trim) != Some("0");
        let interval = match interval.map(|raw| raw.trim().parse::<u64>()) {
            None => DEFAULT_INTERVAL,
            Some(Ok(secs)) if INTERVAL_RANGE_SECS.contains(&secs) => Duration::from_secs(secs),
            Some(_) => {
                log::warn!(
                    "ignoring invalid {EGRESS_PROBE_INTERVAL_ENV} \
                     (expected integer in {INTERVAL_RANGE_SECS:?})"
                );
                DEFAULT_INTERVAL
            }
        };
        let failure_threshold = match failures.map(|raw| raw.trim().parse::<u32>()) {
            None => DEFAULT_FAILURE_THRESHOLD,
            Some(Ok(n)) if FAILURE_RANGE.contains(&n) => n,
            Some(_) => {
                log::warn!(
                    "ignoring invalid {EGRESS_PROBE_FAILURES_ENV} \
                     (expected integer in {FAILURE_RANGE:?})"
                );
                DEFAULT_FAILURE_THRESHOLD
            }
        };
        Self {
            enabled,
            interval,
            failure_threshold,
        }
    }
}

/// Jittered tick delay: `interval * (0.85 + 0.3 * fraction)` with
/// `fraction` uniform in `[0, 1)`, so a fleet of clients never probes
/// in lockstep.
pub(crate) fn jittered(interval: Duration, fraction: f64) -> Duration {
    interval.mul_f64(0.85 + 0.3 * fraction.clamp(0.0, 1.0))
}

/// Builds a minimal RFC 1035 query: header (RD set) + one A/IN
/// question for [`PROBE_QNAME`].
pub(crate) fn build_dns_query(txid: u16, qname: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(17 + qname.len() + 1);
    pkt.extend_from_slice(&txid.to_be_bytes());
    pkt.extend_from_slice(&[
        0x01, 0x00, // flags: RD
        0x00, 0x01, // QDCOUNT = 1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // AN/NS/AR = 0
    ]);
    for label in qname.split('.').filter(|l| !l.is_empty()) {
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0); // root label
    pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]); // QTYPE=A, QCLASS=IN
    pkt
}

/// `true` when `buf` is a DNS response to our `txid`. Any response
/// (even SERVFAIL) proves the round trip through the exit, which is
/// all the liveness probe needs; the RCODE is irrelevant.
pub(crate) fn is_matching_response(buf: &[u8], txid: u16) -> bool {
    buf.len() >= 12 && buf[0..2] == txid.to_be_bytes() && buf[2] & 0x80 != 0
}

/// IO surface consumed by [`run_egress_probe`]; mocked in tests.
pub(crate) trait EgressProbeIo {
    /// Waits for the next probe tick. `false` = teardown, the loop exits.
    async fn next_tick(&mut self) -> bool;
    /// `true` while the supervisor has a live published session (always
    /// `true` on single-hop, which has no supervisor).
    fn session_present(&mut self) -> bool;
    /// One end-to-end probe through the tunnel. `true` = egress alive.
    async fn probe(&mut self) -> bool;
    /// Publishes the verdict to the daemon (edge-triggered only).
    fn publish(&mut self, egress_dead: bool);
    /// `true` while a drain advisory is active on this tunnel.
    fn drain_active(&mut self) -> bool;
    /// Attempts the gap-free drain migration off the current exit.
    /// `true` = migration dispatched.
    async fn try_migrate(&mut self) -> bool;
}

/// Probe scheduler: counts consecutive failures while a session is
/// published, publishes the egress-dead verdict at the threshold,
/// clears it on the first success. Never probes without a session
/// (the RX-silence machinery owns that case).
pub(crate) async fn run_egress_probe<I: EgressProbeIo>(io: &mut I, failure_threshold: u32) {
    let mut consecutive_failures: u32 = 0;
    let mut dead = false;
    loop {
        if !io.next_tick().await {
            return;
        }
        if !io.session_present() {
            // A redial in flight: failures across it would conflate two
            // different exits (a failover may land elsewhere).
            consecutive_failures = 0;
            continue;
        }
        if io.probe().await {
            consecutive_failures = 0;
            if dead {
                dead = false;
                io.publish(false);
            }
        } else {
            consecutive_failures = consecutive_failures.saturating_add(1);
            if !dead && consecutive_failures >= failure_threshold {
                dead = true;
                io.publish(true);
                // Under an active drain the exit is deliberately going
                // away: prefer the gap-free migration path over waiting
                // for the operator's hard-close.
                if io.drain_active() {
                    if io.try_migrate().await {
                        log::info!(
                            "Warren egress probe: exit not forwarding while draining; \
                             gap-free migration dispatched"
                        );
                    } else {
                        log::warn!(
                            "Warren egress probe: exit not forwarding while draining and \
                             the migration hook declined; leaving escalation to the \
                             session-liveness/dead-path machinery"
                        );
                    }
                }
            }
        }
    }
}

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<
    Option<std::sync::Arc<warrenguard_transport::bundle::MultiHopBundle>>,
>;
type DrainWatch =
    tokio::sync::watch::Receiver<Option<warrenguard_transport::supervised_pump::ExitDrainAdvisory>>;

/// Production bindings for [`EgressProbeIo`].
pub(crate) struct RealEgressProbeIo {
    pub interval: Duration,
    /// `None` on single-hop (no supervisor: a session always exists
    /// while the pump runs).
    pub client_rx: Option<ClientWatch>,
    /// Daemon verdict callback (`WarrenTunnelParameters::on_egress_verdict`).
    pub verdict: Option<std::sync::Arc<dyn Fn(bool) + Send + Sync>>,
    /// `None` on single-hop (no drain channel).
    pub drain_rx: Option<DrainWatch>,
    /// Gap-free migration hook + the exit it would migrate off.
    pub drain_migrate: Option<crate::WarrenDrainMigrate>,
    pub current_exit_id: [u8; 16],
}

impl EgressProbeIo for RealEgressProbeIo {
    async fn next_tick(&mut self) -> bool {
        tokio::time::sleep(jittered(self.interval, rand::random::<f64>())).await;
        true
    }

    fn session_present(&mut self) -> bool {
        match self.client_rx.as_mut() {
            Some(rx) => rx.borrow_and_update().is_some(),
            None => true,
        }
    }

    async fn probe(&mut self) -> bool {
        probe_gateway_dns().await
    }

    fn publish(&mut self, egress_dead: bool) {
        if egress_dead {
            log::warn!(
                "Warren egress probe: exit not forwarding (in-tunnel DNS probe dead \
                 while the QUIC session is alive); surfacing exit_egress_dead"
            );
        } else {
            log::info!("Warren egress probe: egress recovered; clearing exit_egress_dead");
        }
        if let Some(cb) = self.verdict.as_ref() {
            cb(egress_dead);
        }
    }

    fn drain_active(&mut self) -> bool {
        self.drain_rx
            .as_mut()
            .is_some_and(|rx| rx.borrow_and_update().is_some())
    }

    async fn try_migrate(&mut self) -> bool {
        match self.drain_migrate.as_ref() {
            Some(migrate) => migrate(self.current_exit_id).await,
            None => false,
        }
    }
}

/// One in-tunnel DNS round trip to the exit resolver. Two datagrams
/// spaced [`PROBE_RETRANSMIT`], overall deadline [`PROBE_TIMEOUT`].
/// Local socket errors (bind/send) are inconclusive, not egress-dead:
/// they report success so a host-side hiccup never raises the banner.
async fn probe_gateway_dns() -> bool {
    let sock = match tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Warren egress probe: local socket bind failed (inconclusive): {e}");
            return true;
        }
    };
    if let Err(e) = sock.connect(GATEWAY_DNS).await {
        log::warn!("Warren egress probe: connect failed (inconclusive): {e}");
        return true;
    }
    let txid = rand::random::<u16>();
    let query = build_dns_query(txid, PROBE_QNAME);
    if sock.send(&query).await.is_err() {
        // Send failures are routing/firewall races during teardown, not
        // an exit verdict.
        return true;
    }
    let mut buf = [0u8; 512];
    let deadline = tokio::time::Instant::now() + PROBE_TIMEOUT;
    let retransmit = tokio::time::sleep(PROBE_RETRANSMIT);
    tokio::pin!(retransmit);
    let mut retransmitted = false;
    loop {
        tokio::select! {
            () = &mut retransmit, if !retransmitted => {
                retransmitted = true;
                let _ = sock.send(&query).await;
            }
            recv = sock.recv(&mut buf) => {
                match recv {
                    Ok(n) if is_matching_response(&buf[..n], txid) => return true,
                    Ok(_) => {} // unrelated datagram, keep reading
                    Err(_) => return false,
                }
            }
            () = tokio::time::sleep_until(deadline) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    // --- config knobs -----------------------------------------------

    #[test]
    fn config_defaults_when_env_unset() {
        let cfg = EgressProbeConfig::resolve(None, None, None);
        assert!(cfg.enabled, "probe is on by default");
        assert_eq!(cfg.interval, DEFAULT_INTERVAL);
        assert_eq!(cfg.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
    }

    #[test]
    fn config_disable_knob_and_overrides() {
        assert!(!EgressProbeConfig::resolve(Some("0"), None, None).enabled);
        assert!(EgressProbeConfig::resolve(Some("1"), None, None).enabled);
        let cfg = EgressProbeConfig::resolve(None, Some("30"), Some("2"));
        assert_eq!(cfg.interval, Duration::from_secs(30));
        assert_eq!(cfg.failure_threshold, 2);
    }

    #[test]
    fn config_rejects_out_of_range_values() {
        // Invalid values must warn + keep the default, never clamp: a
        // typo cannot silently change the probe cadence.
        let cfg = EgressProbeConfig::resolve(None, Some("1"), Some("0"));
        assert_eq!(cfg.interval, DEFAULT_INTERVAL);
        assert_eq!(cfg.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
        let cfg = EgressProbeConfig::resolve(None, Some("abc"), Some("99"));
        assert_eq!(cfg.interval, DEFAULT_INTERVAL);
        assert_eq!(cfg.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
    }

    #[test]
    fn jitter_spreads_within_15_percent() {
        let base = Duration::from_secs(20);
        assert_eq!(jittered(base, 0.0), Duration::from_secs(17));
        assert_eq!(jittered(base, 0.5), Duration::from_secs(20));
        assert_eq!(jittered(base, 1.0), Duration::from_secs(23));
    }

    // --- DNS packet building / matching ------------------------------

    #[test]
    fn dns_query_encodes_header_and_question() {
        let pkt = build_dns_query(0xABCD, "warrenbrowse.com");
        assert_eq!(&pkt[0..2], &[0xAB, 0xCD], "txid big-endian");
        assert_eq!(&pkt[2..4], &[0x01, 0x00], "RD flag only");
        assert_eq!(&pkt[4..6], &[0x00, 0x01], "one question");
        // Question: 12"warrenbrowse" 3"com" 0, A, IN.
        let mut expected_q = vec![12u8];
        expected_q.extend_from_slice(b"warrenbrowse");
        expected_q.push(3);
        expected_q.extend_from_slice(b"com");
        expected_q.extend_from_slice(&[0, 0x00, 0x01, 0x00, 0x01]);
        assert_eq!(&pkt[12..], expected_q.as_slice());
    }

    #[test]
    fn response_matching_requires_txid_and_qr_bit() {
        let mut resp = build_dns_query(0x1234, "warrenbrowse.com");
        assert!(
            !is_matching_response(&resp, 0x1234),
            "a query echo (QR=0) is not a response"
        );
        resp[2] |= 0x80;
        assert!(is_matching_response(&resp, 0x1234));
        assert!(
            !is_matching_response(&resp, 0x9999),
            "txid mismatch must not match (stray datagram)"
        );
        assert!(!is_matching_response(&[0x12, 0x34, 0x80], 0x1234), "runt");
    }

    // --- scheduler ----------------------------------------------------

    /// Scripted mock: one entry per tick.
    struct MockIo {
        /// Per tick: `None` = no session (skip), `Some(ok)` = probe result.
        script: VecDeque<Option<bool>>,
        published: Vec<bool>,
        drain_active: bool,
        migrate_succeeds: bool,
        migrate_attempts: u32,
    }

    impl MockIo {
        fn scripted(script: impl IntoIterator<Item = Option<bool>>) -> Self {
            Self {
                script: script.into_iter().collect(),
                published: Vec::new(),
                drain_active: false,
                migrate_succeeds: false,
                migrate_attempts: 0,
            }
        }
    }

    impl EgressProbeIo for MockIo {
        async fn next_tick(&mut self) -> bool {
            !self.script.is_empty()
        }
        fn session_present(&mut self) -> bool {
            // A skipped tick (no session) consumes its script entry here,
            // since the scheduler `continue`s without calling probe().
            if self
                .script
                .front()
                .expect("tick gated by next_tick")
                .is_some()
            {
                true
            } else {
                self.script.pop_front();
                false
            }
        }
        async fn probe(&mut self) -> bool {
            self.script
                .pop_front()
                .flatten()
                .expect("probe only runs with a session")
        }
        fn publish(&mut self, egress_dead: bool) {
            self.published.push(egress_dead);
        }
        fn drain_active(&mut self) -> bool {
            self.drain_active
        }
        async fn try_migrate(&mut self) -> bool {
            self.migrate_attempts += 1;
            self.migrate_succeeds
        }
    }

    #[tokio::test]
    async fn verdict_fires_after_threshold_consecutive_failures_only() {
        // Two failures under threshold 3: nothing published (a rollout
        // hot-swap blip must not flap the UI).
        let mut io = MockIo::scripted([Some(false), Some(false), Some(true)]);
        run_egress_probe(&mut io, 3).await;
        assert!(
            io.published.is_empty(),
            "sub-threshold failures must never publish: {:?}",
            io.published
        );

        // Three consecutive failures: exactly one dead verdict.
        let mut io = MockIo::scripted([Some(false), Some(false), Some(false), Some(false)]);
        run_egress_probe(&mut io, 3).await;
        assert_eq!(
            io.published,
            vec![true],
            "threshold reached publishes ONE dead verdict, further failures stay silent"
        );
    }

    #[tokio::test]
    async fn single_success_clears_the_dead_verdict() {
        let mut io = MockIo::scripted([Some(false), Some(false), Some(true)]);
        run_egress_probe(&mut io, 2).await;
        assert_eq!(
            io.published,
            vec![true, false],
            "one success after the verdict must clear it immediately"
        );
    }

    #[tokio::test]
    async fn success_resets_the_consecutive_failure_count() {
        // fail, fail, ok, fail, fail: never 3 consecutive => no verdict.
        let mut io = MockIo::scripted([
            Some(false),
            Some(false),
            Some(true),
            Some(false),
            Some(false),
        ]);
        run_egress_probe(&mut io, 3).await;
        assert!(
            io.published.is_empty(),
            "non-consecutive failures must not accumulate: {:?}",
            io.published
        );
    }

    #[tokio::test]
    async fn no_session_ticks_never_probe_and_reset_the_count() {
        // Two failures, then a redial window (no session), then two more
        // failures: the count must restart, so threshold 3 never fires.
        let mut io = MockIo::scripted([Some(false), Some(false), None, Some(false), Some(false)]);
        run_egress_probe(&mut io, 3).await;
        assert!(
            io.published.is_empty(),
            "a redial gap must reset the failure count (the new session may \
             be a different exit): {:?}",
            io.published
        );
    }

    #[tokio::test]
    async fn teardown_exits_without_publishing() {
        let mut io = MockIo::scripted([]);
        run_egress_probe(&mut io, 1).await;
        assert!(io.published.is_empty());
    }

    #[tokio::test]
    async fn drain_active_verdict_prefers_the_migration_hook() {
        let mut io = MockIo::scripted([Some(false), Some(false)]);
        io.drain_active = true;
        io.migrate_succeeds = true;
        run_egress_probe(&mut io, 2).await;
        assert_eq!(io.published, vec![true], "verdict still published");
        assert_eq!(
            io.migrate_attempts, 1,
            "an egress-dead verdict under an active drain must trigger the \
             gap-free migration path"
        );
    }

    #[tokio::test]
    async fn no_drain_means_no_migration_attempt() {
        let mut io = MockIo::scripted([Some(false)]);
        run_egress_probe(&mut io, 1).await;
        assert_eq!(io.published, vec![true]);
        assert_eq!(
            io.migrate_attempts, 0,
            "without a drain advisory the probe only banners; reconnect logic \
             stays with session_liveness/dead-path"
        );
    }

    #[tokio::test]
    async fn real_io_without_callback_or_channels_is_inert_but_alive() {
        // Single-hop shape: no supervisor watch, no drain channel, no
        // migrate hook. session_present must default to true and
        // try_migrate to false.
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            client_rx: None,
            verdict: None,
            drain_rx: None,
            drain_migrate: None,
            current_exit_id: [0; 16],
        };
        assert!(io.session_present(), "single-hop always has a session");
        assert!(!io.drain_active(), "no drain channel => never draining");
        assert!(!io.try_migrate().await, "no hook => no migration");
        io.publish(true); // must not panic without a callback
    }

    #[tokio::test]
    async fn real_io_forwards_the_verdict_to_the_daemon_callback() {
        let seen: std::sync::Arc<std::sync::Mutex<Vec<bool>>> = Default::default();
        let seen_cb = seen.clone();
        let mut io = RealEgressProbeIo {
            interval: Duration::from_secs(25),
            client_rx: None,
            verdict: Some(std::sync::Arc::new(move |dead| {
                seen_cb.lock().unwrap().push(dead);
            })),
            drain_rx: None,
            drain_migrate: None,
            current_exit_id: [0; 16],
        };
        io.publish(true);
        io.publish(false);
        assert_eq!(*seen.lock().unwrap(), vec![true, false]);
    }
}
