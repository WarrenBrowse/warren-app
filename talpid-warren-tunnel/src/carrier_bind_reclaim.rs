//! Post-migration reclaim of the macOS leak-free carrier bind.
//!
//! # Why
//!
//! A migration rebinds the carrier onto a fresh socket, which necessarily
//! drops any `IP_BOUND_IF` bind, so
//! [`MigrationIo::ensure_route_escape`](warrenguard_transport::migration_watchdog::MigrationIo::ensure_route_escape)
//! degrades the escape to the `<carrier_ip>/32 DefaultNode` route first. That
//! route is a destination-keyed exception another host application can take to
//! reach the relay IP off-tunnel, which the bind does not have. The session
//! keeps the route (nothing here removes it, see below), so what this reclaim
//! buys is the NEXT connect on this network: a measured `BindOk` makes it take
//! the leak-free bind and never install the exception at all.
//!
//! # A verdict is only ever a measurement
//!
//! The degradation above is forced by the rebind and says nothing about
//! whether the bind works on the network the host just moved onto. Writing
//! `RouteOnly` for it poisons that network for the whole
//! [`VERDICT_TTL`](crate::carrier_verdict_cache::VERDICT_TTL), so every later
//! connect there skips the bind and pre-installs the exception: the escape
//! would spread instead of shrinking. [`verdict_for`] is the single place a
//! verdict is derived, and it only accepts a
//! [`GuardOutcome`](crate::carrier_egress_guard::GuardOutcome), which exists
//! only after the bootstrap guard actually probed.
//!
//! # Why the `/32` stays for the live session
//!
//! Dropping it needs more than a route removal: on a confirmed-bind host
//! `non_tunnel_routes` is empty and `apply_tunnel_default_routes` returns early
//! on `relay_route_is_valid`, so installing the `/32` is also what brings in
//! talpid's `0/0 dev tun` default and the ifscoped physical default. Undoing
//! all three live is a routing change of its own, in the exact area of the
//! 2026-07-13 carrier blackhole. Keeping the route is what makes this sequence
//! safe: at every instant the carrier holds the route, and for part of it a
//! bind on top, so it is never left with neither.
//!
//! The decision logic is pure over [`ReclaimIo`] so every branch is
//! unit-testable with paused time and no real socket or route table.

use crate::carrier_egress_guard::GuardOutcome;
use crate::carrier_verdict_cache::CachedVerdict;

/// What the reclaim did, for logging and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReclaimOutcome {
    /// This network is already MEASURED as black-holing the bind: nothing was
    /// tried, which is what keeps a known-bad network from paying the guard's
    /// dead window on every migration.
    Skipped,
    /// No bind could be put on the wire (physical interface unresolved, or the
    /// engine refused the rebind): the session kept its socket.
    NotAttempted,
    /// The bind egressed. Recorded, so the next connect here takes it and
    /// never installs the `/32` exception.
    BindConfirmed,
    /// The bind black-holed and the guard already reverted the session to the
    /// route escape. Recorded so the next migration skips the attempt.
    BindBlackholed,
    /// Neither proven nor disproven: the session is back on an unbound socket
    /// and nothing was written, so the next connect measures again.
    Unproven,
}

/// The verdict a watchdog cycle may persist for the network it moved onto.
///
/// `None` covers the three cases that are not evidence FOR a configuration: no
/// measurement was taken at all (the forced degradation), a measurement that
/// proved nothing, and one where the route escape black-holed as well. All
/// three must leave the cache untouched so the next connect measures rather
/// than replays a verdict nobody observed, and `RouteOnly` in particular is
/// never written for a host that egresses through neither configuration.
#[must_use]
pub(crate) fn verdict_for(measurement: Option<GuardOutcome>) -> Option<CachedVerdict> {
    match measurement? {
        GuardOutcome::BypassConfirmed => Some(CachedVerdict::BindOk),
        GuardOutcome::RevertedToRoute | GuardOutcome::BindBlackholed => {
            Some(CachedVerdict::RouteOnly)
        }
        GuardOutcome::EscapeAlsoDead | GuardOutcome::Inconclusive => None,
    }
}

/// I/O seam consumed by [`run_bind_reclaim`]. Implemented for production by
/// `RealWatchdogIo` (macOS only) and by a scripted mock in the unit tests.
pub(crate) trait ReclaimIo {
    /// The verdict already MEASURED for the network the host egresses on now.
    async fn measured_verdict(&mut self) -> Option<CachedVerdict>;

    /// Rebind the live session onto a socket bound to the current physical
    /// interface. `false` when the interface could not be resolved or the bind
    /// could not be applied; the session then keeps the socket it had.
    async fn rebind_bound(&mut self) -> bool;

    /// Probe the freshly bound carrier. On a black-holing bind the guard
    /// reverts the session itself (route reinstalled, socket unbound).
    async fn verify_bind(&mut self) -> GuardOutcome;

    /// Put the session back on an unbound socket, the state the route escape
    /// alone was proven safe with.
    async fn rebind_unbound(&mut self);

    /// Persist a measured verdict against the network the measurement was
    /// taken on.
    async fn record_verdict(&mut self, verdict: CachedVerdict);
}

/// Try to take the leak-free carrier bind back on the network the session just
/// migrated onto, and record what was observed.
pub(crate) async fn run_bind_reclaim<I: ReclaimIo>(io: &mut I) -> ReclaimOutcome {
    if io.measured_verdict().await == Some(CachedVerdict::RouteOnly) {
        return ReclaimOutcome::Skipped;
    }
    if !io.rebind_bound().await {
        return ReclaimOutcome::NotAttempted;
    }
    let measurement = io.verify_bind().await;
    match verdict_for(Some(measurement)) {
        Some(verdict) => {
            io.record_verdict(verdict).await;
            match verdict {
                CachedVerdict::BindOk => ReclaimOutcome::BindConfirmed,
                CachedVerdict::RouteOnly => ReclaimOutcome::BindBlackholed,
            }
        }
        // An unproven bind is exactly the blackhole risk, so it does not stay.
        None => {
            io.rebind_unbound().await;
            ReclaimOutcome::Unproven
        }
    }
}

// Production I/O.

pub(crate) use real_io::RealReclaimIo;

mod real_io {
    use std::path::PathBuf;
    use std::sync::Arc;

    use talpid_routing::RouteManagerHandle;
    use warrenguard_transport::bundle::MultiHopBundle;
    use warrenguard_transport::multihop::RebindPolicy;

    use super::ReclaimIo;
    use crate::carrier_egress_guard::{GuardOutcome, RealEgressGuardIo, run_bootstrap_guard};
    use crate::carrier_verdict_cache::{
        CachedVerdict, VerdictCache, network_fingerprint, now_unix,
    };

    /// Watch receiver over the supervisor's published multi-hop session.
    type ClientWatch = tokio::sync::watch::Receiver<Option<Arc<MultiHopBundle>>>;

    /// Production bindings for [`ReclaimIo`].
    pub(crate) struct RealReclaimIo {
        client_rx: ClientWatch,
        route_manager: RouteManagerHandle,
        verdict_dir: Option<PathBuf>,
        /// Resolved on the cache lookup and reused for the write, so a
        /// measurement can never be attributed to a network other than the one
        /// it was taken on (the default route may move again mid-reclaim).
        fingerprint: Option<String>,
    }

    impl RealReclaimIo {
        pub(crate) fn new(
            client_rx: ClientWatch,
            route_manager: RouteManagerHandle,
            verdict_dir: Option<PathBuf>,
        ) -> Self {
            Self {
                client_rx,
                route_manager,
                verdict_dir,
                fingerprint: None,
            }
        }

        fn current_client(&self) -> Option<Arc<MultiHopBundle>> {
            self.client_rx.borrow().clone()
        }
    }

    impl ReclaimIo for RealReclaimIo {
        async fn measured_verdict(&mut self) -> Option<CachedVerdict> {
            let (v4, _v6) = self.route_manager.get_default_routes().await.ok()?;
            let v4 = v4?;
            let fingerprint = network_fingerprint(&v4.interface, v4.router_ip);
            let verdict =
                VerdictCache::load(self.verdict_dir.as_deref()).lookup(&fingerprint, now_unix());
            self.fingerprint = Some(fingerprint);
            verdict
        }

        async fn rebind_bound(&mut self) -> bool {
            let Some(client) = self.current_client() else {
                return false;
            };
            // Re-resolve rather than reuse the connect-time index: the point of
            // the reclaim is the interface the host egresses on NOW, and this
            // discovery sees past the tunnel's own default.
            let ifindex =
                match warrenguard_route_split::default_route_split_macos::discover_physical_ifindex(
                )
                .await
                {
                    Ok(ifindex) => ifindex,
                    Err(e) => {
                        log::debug!("carrier bind reclaim: physical interface unresolved: {e}");
                        return false;
                    }
                };
            match client.rebind_wildcard(RebindPolicy::Bypass(crate::warren_carrier_socket_bypass(
                ifindex,
            ))) {
                Ok(()) => true,
                Err(e) => {
                    log::debug!("carrier bind reclaim: bound rebind refused: {e}");
                    false
                }
            }
        }

        async fn verify_bind(&mut self) -> GuardOutcome {
            let mut guard_io = RealEgressGuardIo {
                client_rx: self.client_rx.clone(),
            };
            run_bootstrap_guard(&mut guard_io).await
        }

        async fn rebind_unbound(&mut self) {
            let Some(client) = self.current_client() else {
                return;
            };
            if let Err(e) = client.rebind_wildcard(RebindPolicy::Plain) {
                log::warn!(
                    "carrier bind reclaim: could not put the carrier back on an unbound socket: \
                     {e}. The <carrier_ip>/32 escape is still installed and the dead-path \
                     escalation remains the backstop."
                );
            }
        }

        async fn record_verdict(&mut self, verdict: CachedVerdict) {
            // No fingerprint means the default route could not be read, so
            // there is no network to attribute the measurement to.
            let Some(fingerprint) = self.fingerprint.clone() else {
                return;
            };
            VerdictCache::load(self.verdict_dir.as_deref()).record(
                &fingerprint,
                verdict,
                now_unix(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEASURED_VERDICT: &str = "measured_verdict";
    const REBIND_BOUND: &str = "rebind_bound";
    const VERIFY_BIND: &str = "verify_bind";
    const REBIND_UNBOUND: &str = "rebind_unbound";
    const RECORD_VERDICT: &str = "record_verdict";

    /// Records every seam call in order, so a test can assert on the sequence
    /// and not only on the outcome.
    struct MockIo {
        cached: Option<CachedVerdict>,
        bind_applies: bool,
        measurement: GuardOutcome,
        calls: Vec<&'static str>,
        recorded: Option<CachedVerdict>,
    }

    impl MockIo {
        fn new(cached: Option<CachedVerdict>, measurement: GuardOutcome) -> Self {
            Self {
                cached,
                bind_applies: true,
                measurement,
                calls: Vec::new(),
                recorded: None,
            }
        }
    }

    impl ReclaimIo for MockIo {
        async fn measured_verdict(&mut self) -> Option<CachedVerdict> {
            self.calls.push(MEASURED_VERDICT);
            self.cached
        }
        async fn rebind_bound(&mut self) -> bool {
            self.calls.push(REBIND_BOUND);
            self.bind_applies
        }
        async fn verify_bind(&mut self) -> GuardOutcome {
            self.calls.push(VERIFY_BIND);
            self.measurement
        }
        async fn rebind_unbound(&mut self) {
            self.calls.push(REBIND_UNBOUND);
        }
        async fn record_verdict(&mut self, verdict: CachedVerdict) {
            self.calls.push(RECORD_VERDICT);
            self.recorded = Some(verdict);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn reclaim_is_skipped_on_a_network_measured_to_blackhole_the_bind() {
        // The guard already spent its dead window proving this network kills a
        // bound carrier. Retrying costs that window again on every migration
        // and buys nothing.
        let mut io = MockIo::new(
            Some(CachedVerdict::RouteOnly),
            GuardOutcome::BypassConfirmed,
        );

        assert_eq!(run_bind_reclaim(&mut io).await, ReclaimOutcome::Skipped);

        assert_eq!(
            io.calls,
            vec![MEASURED_VERDICT],
            "a measured blackhole must stop the sequence before any rebind"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmed_bind_is_recorded_so_the_next_connect_skips_the_route_exception() {
        // The whole point: the live session keeps its /32, and the recorded
        // BindOk is what makes the NEXT connect here take the leak-free bind
        // and never install the exception.
        let mut io = MockIo::new(None, GuardOutcome::BypassConfirmed);

        assert_eq!(
            run_bind_reclaim(&mut io).await,
            ReclaimOutcome::BindConfirmed
        );

        assert_eq!(io.recorded, Some(CachedVerdict::BindOk));
        assert_eq!(
            io.calls,
            vec![MEASURED_VERDICT, REBIND_BOUND, VERIFY_BIND, RECORD_VERDICT],
            "the verdict must follow the measurement, never precede it"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_unconfirmed_bind_is_undone_and_records_no_verdict() {
        // No evidence either way. The bind does not stay (that is the
        // blackhole risk) and the cache is left alone so the next connect
        // measures instead of replaying a guess.
        let mut io = MockIo::new(None, GuardOutcome::Inconclusive);

        assert_eq!(run_bind_reclaim(&mut io).await, ReclaimOutcome::Unproven);

        assert_eq!(io.recorded, None);
        assert_eq!(
            io.calls,
            vec![MEASURED_VERDICT, REBIND_BOUND, VERIFY_BIND, REBIND_UNBOUND]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_blackholing_bind_is_recorded_from_the_guards_own_revert() {
        // The guard reverted the session itself (route reinstalled, socket
        // unbound), so the only thing left is to remember it.
        let mut io = MockIo::new(None, GuardOutcome::RevertedToRoute);

        assert_eq!(
            run_bind_reclaim(&mut io).await,
            ReclaimOutcome::BindBlackholed
        );

        assert_eq!(io.recorded, Some(CachedVerdict::RouteOnly));
        assert!(
            !io.calls.contains(&REBIND_UNBOUND),
            "the guard already unbound; undoing it twice would rebind a working session"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_refused_bind_leaves_the_session_alone() {
        // The engine fails the rebind closed when the bypass cannot be
        // applied, so there is nothing bound to measure or to undo.
        let mut io = MockIo::new(None, GuardOutcome::BypassConfirmed);
        io.bind_applies = false;

        assert_eq!(
            run_bind_reclaim(&mut io).await,
            ReclaimOutcome::NotAttempted
        );

        assert_eq!(io.calls, vec![MEASURED_VERDICT, REBIND_BOUND]);
        assert_eq!(io.recorded, None);
    }

    #[test]
    fn a_forced_degradation_records_no_verdict() {
        // The escape degraded before a migration rebind is imposed by the
        // rebind, never measured against the network the session moved onto.
        // Writing RouteOnly for it made every later connect there pre-install
        // the /32 exception for the whole verdict TTL.
        assert_eq!(verdict_for(None), None);
    }

    #[test]
    fn an_escape_that_black_holed_too_records_no_verdict() {
        // The guard measured that the route escape carries no more than the
        // bind did. That is evidence AGAINST both configurations, so it must
        // not be written as a `RouteOnly` preference for one of them.
        assert_eq!(verdict_for(Some(GuardOutcome::EscapeAlsoDead)), None);
    }
}
