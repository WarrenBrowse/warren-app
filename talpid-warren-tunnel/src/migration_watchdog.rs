//! Migration watchdog: keeps the multi-hop QUIC path alive across
//! network changes, with a layered fallback.
//!
//! Primary path (zero work here): the QUIC socket is wildcard-bound
//! and the relay accepts migration, so after a default-route change
//! the very next packet leaves through the new interface and the
//! relay revalidates the path in ~1 RTT. This module only VERIFIES
//! that this happened, nudges the relay bypass route, and falls back
//! when it did not:
//!
//! 1. On a default-route change, re-point the relay/32 bypass at the
//!    new gateway (platform-specific nudge), then probe the tunnel
//!    with DAITA padding datagrams for [`MIGRATION_TIMEOUT`].
//! 2. No response (relay without migration support, broken NAT):
//!    [`SupervisorHandle::force_reconnect`], which redials from a
//!    fresh socket under the supervisor's 300 ms / 2 s backoff. TUN,
//!    routes, firewall and the state machine are untouched.
//! 3. Still no live session after [`ESCALATE_TIMEOUT`] (captive
//!    portal, IPv6-only network with a v4-only relay): surface a pump
//!    error so `wait()` returns `BackendTransient` and the state
//!    machine runs its normal fail-closed reconnect cycle.
//!
//! The decision loop is pure over [`WatchdogIo`] so every branch is
//! unit-testable with paused time; `RealWatchdogIo` supplies the
//! platform bindings (talpid-routing listeners, supervisor handle,
//! DAITA probe).

use std::sync::Arc;
use std::time::Duration;

use talpid_routing::RouteManagerHandle;
use warren_client::multi_hop::MultiHopClient;
use warren_client::supervisor::SupervisorHandle;

/// Coalescing window for bursts of route events (an interface flap
/// emits several within milliseconds). This is event coalescing, not
/// a state debounce: the probe starts at most 250 ms after the FIRST
/// event of a burst.
pub(crate) const ROUTE_SETTLE: Duration = Duration::from_millis(250);
/// Interval between liveness probes while waiting for the migrated
/// path to answer.
pub(crate) const PROBE_INTERVAL: Duration = Duration::from_millis(250);
/// How long passive migration gets before the watchdog forces a
/// supervisor re-handshake. Several probe RTTs plus link-settle time,
/// far above the relay's ~1 RTT path validation.
pub(crate) const MIGRATION_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a forced reconnect gets to republish a live session
/// before the watchdog escalates to the state machine.
pub(crate) const ESCALATE_TIMEOUT: Duration = Duration::from_secs(30);

/// Identity + rx-counter sample of the currently published session.
/// The `id` combines the `Arc` pointer of the published client with
/// its local UDP port: rx counters are only comparable between samples
/// of the SAME session (a fresh session restarts its counters near
/// zero). The pointer alone is NOT a safe identity: the allocator can
/// reuse a freed `Arc` address for the very next session (ABA), which
/// once made the watchdog kill a freshly re-established session
/// because its low rx counter read as "no progress" on the old
/// baseline. The wildcard bind gives every session a fresh ephemeral
/// port, which disambiguates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RxSample {
    pub id: usize,
    pub rx_datagrams: u64,
}

/// Platform/IO surface consumed by [`run_watchdog`]. Implemented for
/// production by [`RealWatchdogIo`] and by mocks in the unit tests.
pub(crate) trait WatchdogIo {
    /// Resolves on the next default-route change event. Returns
    /// `false` when the event source is gone (teardown): the watchdog
    /// loop exits.
    async fn next_route_event(&mut self) -> bool;

    /// `true` when a usable IPv4 default route is currently present
    /// (the relay is dialed over v4).
    async fn has_v4_default_route(&mut self) -> bool;

    /// Re-point the relay bypass host route at the current default
    /// gateway. Best-effort.
    async fn nudge_bypass(&mut self);

    /// Actively rebind the live session's QUIC endpoint onto a fresh
    /// wildcard UDP socket. On macOS a wildcard socket does NOT
    /// reliably follow an interface change on its own (the kernel
    /// keeps the old flow's source state), so passive migration
    /// stalls; a fresh socket forces the next packet out the current
    /// default route, giving the relay a new 4-tuple to validate.
    /// Best-effort and a no-op when no session is published.
    fn rebind_endpoint(&mut self);

    /// Send one in-tunnel liveness probe (DAITA padding datagram) on
    /// the current session, if any. Any answer bumps the rx counters
    /// observed by [`Self::rx_sample`].
    async fn send_probe(&mut self);

    /// Sample the live session's identity + rx counter. `None` when
    /// no session is published (supervisor redialing).
    fn rx_sample(&mut self) -> Option<RxSample>;

    /// Close the current session so the supervisor redials from a
    /// fresh socket. Returns `false` when no session was published.
    fn force_reconnect(&mut self) -> bool;

    /// Report an unrecoverable path to the tunnel monitor (surfaces
    /// as `BackendTransient` through the pump-error channel).
    fn escalate(&mut self, msg: String);
}

/// Outcome of one verification cycle, for logging and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleOutcome {
    /// The path answered probes (or a fresh session was published)
    /// without a forced reconnect.
    Migrated,
    /// The forced re-handshake brought a live session back.
    Reconnected,
    /// Nothing recovered within [`ESCALATE_TIMEOUT`]; escalated.
    Escalated,
    /// No usable v4 default route: parked until the next event, with
    /// the escalation backstop armed for online-but-broken networks.
    Parked,
}

/// `true` when `cur` proves the tunnel is receiving again relative to
/// `base`. A session swap (different id) counts as alive: a fresh
/// session only exists because a full handshake just succeeded.
fn rx_advanced(base: Option<RxSample>, cur: Option<RxSample>) -> bool {
    match (base, cur) {
        // Same id but the counter went BACKWARDS: quinn counters are
        // monotonic within a session, so this is an ABA id collision
        // with a fresh session (= a handshake succeeded) and counts as
        // alive.
        (Some(b), Some(c)) if b.id == c.id => c.rx_datagrams != b.rx_datagrams,
        (Some(b), Some(c)) => b.id != c.id,
        // No baseline session but one is published now: it was just
        // dialed on the new network, which is proof of liveness.
        (None, Some(_)) => true,
        (_, None) => false,
    }
}

/// Probe the current session until it answers or `window` elapses.
async fn probe_until<I: WatchdogIo>(io: &mut I, window: Duration) -> bool {
    let base = io.rx_sample();
    let deadline = tokio::time::Instant::now() + window;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        io.send_probe().await;
        tokio::time::sleep(PROBE_INTERVAL).await;
        if rx_advanced(base, io.rx_sample()) {
            return true;
        }
    }
}

/// One full verification cycle following a (coalesced) route event.
pub(crate) async fn run_cycle<I: WatchdogIo>(io: &mut I) -> CycleOutcome {
    if !io.has_v4_default_route().await {
        // Truly offline windows are owned by the state machine
        // (offline grace then Block(IsOffline)). The backstop below
        // only matters on networks that count as online without a v4
        // route (IPv6-only): no offline edge ever fires there, so
        // without it the UI would sit "Connected" on a dead tunnel
        // forever.
        let woke_by_event = {
            let parked = tokio::time::sleep(ESCALATE_TIMEOUT);
            tokio::pin!(parked);
            tokio::select! {
                () = &mut parked => false,
                _alive = io.next_route_event() => true,
            }
        };
        if woke_by_event {
            // New event (or source closed): restart from the top; a
            // closed source makes the caller's next recv exit the loop.
            return CycleOutcome::Parked;
        }
        if io.has_v4_default_route().await {
            return CycleOutcome::Parked;
        }
        if probe_until(io, MIGRATION_TIMEOUT).await {
            return CycleOutcome::Migrated;
        }
        io.escalate(
            "no IPv4 default route and tunnel unresponsive after network change".to_string(),
        );
        return CycleOutcome::Escalated;
    }

    io.nudge_bypass().await;
    // Active migration: rebind onto a fresh socket BEFORE probing.
    // Passive migration on the existing socket is unreliable on macOS
    // (kernel keeps the old interface's flow state), so the probes
    // below would otherwise keep leaving the dead interface and the
    // path would never revalidate. A fresh socket + the repointed
    // relay bypass route is what actually moves traffic to the new
    // interface; the relay (migration enabled) then validates it.
    io.rebind_endpoint();

    let started = tokio::time::Instant::now();
    log::info!(
        "Warren migration watchdog: default-route change; rebound socket, probing the path \
         (baseline {:?})",
        io.rx_sample()
    );
    if probe_until(io, MIGRATION_TIMEOUT).await {
        log::info!(
            "Warren migration watchdog: QUIC path revalidated in {} ms after default-route change",
            started.elapsed().as_millis()
        );
        return CycleOutcome::Migrated;
    }

    let last_sample = io.rx_sample();
    let had_session = io.force_reconnect();
    log::warn!(
        "Warren migration watchdog: path not revalidated within {MIGRATION_TIMEOUT:?} \
         (last sample {last_sample:?}); forced supervisor reconnect (had_session={had_session})"
    );

    let escalate_deadline = tokio::time::Instant::now() + ESCALATE_TIMEOUT;
    loop {
        if tokio::time::Instant::now() >= escalate_deadline {
            io.escalate(format!(
                "tunnel path not recovered within {ESCALATE_TIMEOUT:?} after network change"
            ));
            return CycleOutcome::Escalated;
        }
        if probe_until(io, MIGRATION_TIMEOUT).await {
            log::info!(
                "Warren migration watchdog: session recovered via forced re-handshake \
                 {} ms after the default-route change",
                started.elapsed().as_millis()
            );
            return CycleOutcome::Reconnected;
        }
    }
}

/// Watchdog main loop: coalesce route-event bursts, then run one
/// verification cycle per burst. Exits when the event source closes
/// (route manager torn down) or after an escalation (the monitor is
/// about to tear the tunnel down anyway).
pub(crate) async fn run_watchdog<I: WatchdogIo>(io: &mut I) {
    loop {
        if !io.next_route_event().await {
            return;
        }
        // Coalesce the burst: flaps emit several events back-to-back.
        let mut source_closed = false;
        {
            let settle = tokio::time::sleep(ROUTE_SETTLE);
            tokio::pin!(settle);
            loop {
                tokio::select! {
                    () = &mut settle => break,
                    alive = io.next_route_event() => {
                        if !alive {
                            source_closed = true;
                            break;
                        }
                    }
                }
            }
        }
        if source_closed {
            return;
        }
        if run_cycle(io).await == CycleOutcome::Escalated {
            return;
        }
    }
}

// ── Production IO ────────────────────────────────────────────────────

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<Option<Arc<MultiHopClient>>>;
/// Shared single-shot pump error sender (same instance the pumps use).
type PumpErrorTx = Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

/// Production bindings for [`WatchdogIo`].
pub(crate) struct RealWatchdogIo {
    pub route_events: tokio::sync::mpsc::UnboundedReceiver<()>,
    /// Kept alive for the platform subscriptions that are RAII-bound
    /// (Windows `CallbackHandle`); `None` elsewhere.
    #[allow(dead_code)]
    pub _subscription_guard: Option<Box<dyn std::any::Any + Send>>,
    // Only macOS reads this (get_default_routes / refresh_routes); the
    // Linux and Windows nudge + has-route paths use free functions, so
    // the field is dead there but kept for a uniform struct shape.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub route_manager: RouteManagerHandle,
    pub client_rx: ClientWatch,
    pub supervisor: SupervisorHandle,
    pub pump_error_tx: PumpErrorTx,
    /// Relay endpoint IPv4, for the bypass-route nudge.
    pub relay_ipv4: Option<std::net::Ipv4Addr>,
}

impl RealWatchdogIo {
    fn current_client(&self) -> Option<Arc<MultiHopClient>> {
        self.client_rx.borrow().clone()
    }
}

impl WatchdogIo for RealWatchdogIo {
    async fn next_route_event(&mut self) -> bool {
        self.route_events.recv().await.is_some()
    }

    async fn has_v4_default_route(&mut self) -> bool {
        #[cfg(target_os = "macos")]
        {
            match self.route_manager.get_default_routes().await {
                Ok((v4, _v6)) => v4.is_some(),
                Err(e) => {
                    log::debug!("watchdog: get_default_routes failed: {e}");
                    false
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            crate::detect_default_iface().is_ok()
        }
        #[cfg(target_os = "windows")]
        {
            talpid_routing::get_best_default_route(talpid_windows::net::AddressFamily::Ipv4)
                .map(|r| r.is_some())
                .unwrap_or(false)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            true
        }
    }

    async fn nudge_bypass(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // Two layers, both load-bearing. The QUIC path validation
            // after a migration times out in ~2-3 s, so the relay/32
            // bypass MUST point at the new gateway well before then or
            // the migrated path can never be validated and the relay
            // tears the connection down (observed as "closed by peer").
            //
            // 1. Directly delete+add relay/32 at the freshly-read
            //    gateway. Deterministic and immediate, independent of
            //    talpid's internal best-route freshness timing.
            if let Some(relay) = self.relay_ipv4 {
                match self.route_manager.get_default_routes().await {
                    Ok((Some(v4), _)) => {
                        let relay_host = format!("{relay}");
                        let gw = v4.router_ip.to_string();
                        // delete is best-effort: the stale route may have
                        // already died with the old interface.
                        let _ = tokio::process::Command::new("route")
                            .args(["-n", "delete", "-host", &relay_host])
                            .output()
                            .await;
                        let add = tokio::process::Command::new("route")
                            .args(["-n", "add", "-host", &relay_host, &gw])
                            .output()
                            .await;
                        match add {
                            Ok(o) if o.status.success() => log::info!(
                                "watchdog: repointed relay bypass {relay_host} -> gw {gw}"
                            ),
                            Ok(o) => log::debug!(
                                "watchdog: relay bypass add rc={:?} stderr={}",
                                o.status.code(),
                                String::from_utf8_lossy(&o.stderr).trim()
                            ),
                            Err(e) => log::debug!("watchdog: relay bypass add failed: {e}"),
                        }
                    }
                    Ok((None, _)) => log::debug!("watchdog: no v4 default route to repoint bypass"),
                    Err(e) => log::debug!("watchdog: get_default_routes failed: {e}"),
                }
            }
            // 2. Also kick talpid's own refresh so its DefaultNode record
            //    and the tunnel default converge (and it owns the
            //    eventual clean teardown of our manual route).
            if let Err(e) = self.route_manager.refresh_routes() {
                log::debug!("watchdog: refresh_routes nudge failed: {e}");
            }
        }
        #[cfg(target_os = "linux")]
        {
            // The static `relay/32 via <old gw>` in the main table goes
            // stale when the old interface stays up-but-dead; replace it
            // against the current default. The kernel removes it by
            // itself when the old interface goes down, so failures here
            // are non-fatal noise.
            if let Some(relay) = self.relay_ipv4 {
                match (
                    crate::detect_default_iface(),
                    crate::detect_default_gateway_v4(),
                ) {
                    (Ok(iface), Ok(gw)) => {
                        let status = tokio::process::Command::new("ip")
                            .args([
                                "route",
                                "replace",
                                &format!("{relay}/32"),
                                "via",
                                &gw.to_string(),
                                "dev",
                                &iface,
                            ])
                            .status()
                            .await;
                        if let Err(e) = status {
                            log::debug!("watchdog: ip route replace nudge failed: {e}");
                        }
                    }
                    (iface, gw) => {
                        log::debug!(
                            "watchdog: bypass nudge skipped (iface={:?} gw={:?})",
                            iface.err(),
                            gw.err()
                        );
                    }
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(relay) = self.relay_ipv4
                && let Err(e) =
                    warren_client::default_route_split_windows::refresh_host_exception(relay).await
            {
                log::debug!("watchdog: refresh_host_exception nudge failed: {e}");
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            let _ = &self.relay_ipv4;
        }
    }

    fn rebind_endpoint(&mut self) {
        let Some(client) = self.current_client() else {
            return;
        };
        // Family-match the fresh socket to the live local address so a
        // v6-dialed relay keeps a v6 source. Bind wildcard so the OS
        // picks the source per packet from the current routing table.
        let bind: std::net::SocketAddr = match client.local_addr() {
            Ok(std::net::SocketAddr::V6(_)) => {
                (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
            }
            _ => (std::net::Ipv4Addr::UNSPECIFIED, 0).into(),
        };
        match std::net::UdpSocket::bind(bind) {
            Ok(sock) => {
                if let Err(e) = client.rebind(sock) {
                    log::debug!("watchdog: endpoint rebind failed: {e}");
                } else {
                    log::info!("Warren migration watchdog: rebound QUIC endpoint to a fresh socket");
                }
            }
            Err(e) => log::debug!("watchdog: fresh migration socket bind failed: {e}"),
        }
    }

    async fn send_probe(&mut self) {
        if let Some(client) = self.current_client()
            && let Err(e) = client.send_daita_padding().await
        {
            log::debug!("watchdog: liveness probe send failed: {e}");
        }
    }

    fn rx_sample(&mut self) -> Option<RxSample> {
        self.current_client().map(|client| {
            // Mix the local port into the identity: the Arc address can
            // be reused by the very next session (ABA), the wildcard
            // bind's ephemeral port cannot, within any realistic window.
            let port = client.local_addr().map(|a| a.port()).unwrap_or(0);
            RxSample {
                id: (Arc::as_ptr(&client) as usize) ^ (usize::from(port) << 47),
                rx_datagrams: client.quinn_stats().udp_rx.datagrams,
            }
        })
    }

    fn force_reconnect(&mut self) -> bool {
        self.supervisor.force_reconnect()
    }

    fn escalate(&mut self, msg: String) {
        log::warn!("Warren migration watchdog: escalating to the state machine: {msg}");
        if let Some(tx) = self.pump_error_tx.lock().ok().and_then(|mut g| g.take()) {
            let _ = tx.send(msg);
        }
    }
}

/// Subscribe to default-route changes, normalized to a unit-tick
/// channel. Returns the receiver plus an optional RAII guard that
/// must stay alive for the subscription to keep firing.
pub(crate) async fn subscribe_route_events(
    route_manager: &RouteManagerHandle,
) -> Result<
    (
        tokio::sync::mpsc::UnboundedReceiver<()>,
        Option<Box<dyn std::any::Any + Send>>,
    ),
    String,
> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(target_os = "macos")]
    {
        use futures::StreamExt;
        let mut events = route_manager
            .default_route_listener()
            .await
            .map_err(|e| format!("default_route_listener: {e}"))?;
        tokio::spawn(async move {
            while let Some(_event) = events.next().await {
                if tx.send(()).is_err() {
                    return;
                }
            }
        });
        Ok((rx, None))
    }
    #[cfg(target_os = "linux")]
    {
        use futures::StreamExt;
        use talpid_routing::CallbackMessage;
        let mut events = route_manager
            .change_listener()
            .await
            .map_err(|e| format!("change_listener: {e}"))?;
        tokio::spawn(async move {
            while let Some(msg) = events.next().await {
                let is_default_route = match &msg {
                    CallbackMessage::NewRoute(route) | CallbackMessage::DelRoute(route) => {
                        route.prefix().prefix() == 0
                    }
                };
                if is_default_route && tx.send(()).is_err() {
                    return;
                }
            }
        });
        Ok((rx, None))
    }
    #[cfg(target_os = "windows")]
    {
        let handle = route_manager
            .add_default_route_change_callback(Box::new(move |_event, _family| {
                let _ = tx.send(());
            }))
            .await
            .map_err(|e| format!("add_default_route_change_callback: {e}"))?;
        Ok((rx, Some(Box::new(handle) as Box<dyn std::any::Any + Send>)))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = route_manager;
        drop(tx);
        Ok((rx, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Scripted mock: every IO answer is pre-loaded; calls are recorded.
    struct MockIo {
        route_events: tokio::sync::mpsc::UnboundedReceiver<()>,
        has_route: bool,
        /// Successive samples returned by `rx_sample`, then the last
        /// one repeats forever.
        samples: VecDeque<Option<RxSample>>,
        last_sample: Option<RxSample>,
        probes_sent: u32,
        nudges: u32,
        rebinds: u32,
        force_reconnects: u32,
        escalations: Vec<String>,
    }

    impl MockIo {
        fn new(
            has_route: bool,
            samples: Vec<Option<RxSample>>,
        ) -> (Self, tokio::sync::mpsc::UnboundedSender<()>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let mut samples: VecDeque<_> = samples.into();
            let last = samples.pop_back().flatten();
            (
                Self {
                    route_events: rx,
                    has_route,
                    samples,
                    last_sample: last,
                    probes_sent: 0,
                    nudges: 0,
                    rebinds: 0,
                    force_reconnects: 0,
                    escalations: Vec::new(),
                },
                tx,
            )
        }
    }

    impl WatchdogIo for MockIo {
        async fn next_route_event(&mut self) -> bool {
            self.route_events.recv().await.is_some()
        }
        async fn has_v4_default_route(&mut self) -> bool {
            self.has_route
        }
        async fn nudge_bypass(&mut self) {
            self.nudges += 1;
        }
        fn rebind_endpoint(&mut self) {
            self.rebinds += 1;
        }
        async fn send_probe(&mut self) {
            self.probes_sent += 1;
        }
        fn rx_sample(&mut self) -> Option<RxSample> {
            match self.samples.pop_front() {
                Some(s) => s,
                None => self.last_sample,
            }
        }
        fn force_reconnect(&mut self) -> bool {
            self.force_reconnects += 1;
            true
        }
        fn escalate(&mut self, msg: String) {
            self.escalations.push(msg);
        }
    }

    fn sample(id: usize, rx: u64) -> Option<RxSample> {
        Some(RxSample {
            id,
            rx_datagrams: rx,
        })
    }

    #[test]
    fn rx_advanced_same_session_requires_counter_movement() {
        assert!(rx_advanced(sample(1, 10), sample(1, 11)));
        assert!(!rx_advanced(sample(1, 10), sample(1, 10)));
        // Counter going BACKWARDS on the same id = ABA address reuse by
        // a fresh session (monotonic within a session) = alive.
        assert!(rx_advanced(sample(1, 10), sample(1, 9)));
    }

    #[test]
    fn rx_advanced_session_swap_counts_as_alive() {
        // A fresh session only exists because a handshake succeeded on
        // the new network; its counters are NOT comparable.
        assert!(rx_advanced(sample(1, 500), sample(2, 3)));
        assert!(rx_advanced(None, sample(2, 0)));
        assert!(!rx_advanced(sample(1, 500), None));
        assert!(!rx_advanced(None, None));
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_confirms_migration_when_rx_advances() {
        // Baseline 10, first post-probe sample 11: confirmed on the
        // first probe, no force_reconnect, exactly one nudge.
        let (mut io, _tx) = MockIo::new(true, vec![sample(7, 10), sample(7, 11)]);
        let outcome = run_cycle(&mut io).await;
        assert_eq!(outcome, CycleOutcome::Migrated);
        assert_eq!(io.nudges, 1);
        assert_eq!(io.rebinds, 1, "active rebind must run before probing");
        assert_eq!(io.force_reconnects, 0);
        assert!(io.escalations.is_empty());
        assert!(io.probes_sent >= 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_forces_reconnect_after_migration_timeout_then_recovers() {
        // Counter never advances on session 7 (stuck path); after the
        // forced reconnect a NEW session id appears => Reconnected.
        let stuck: Vec<Option<RxSample>> = std::iter::repeat_n(sample(7, 10), 16).collect();
        let mut script = stuck;
        // Post-reconnect: a fresh session publishes (different id).
        script.push(sample(8, 1));
        script.push(sample(8, 2));
        let (mut io, _tx) = MockIo::new(true, script);
        let outcome = run_cycle(&mut io).await;
        assert_eq!(outcome, CycleOutcome::Reconnected);
        assert_eq!(io.force_reconnects, 1, "exactly one forced reconnect");
        assert!(io.escalations.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_escalates_when_nothing_recovers() {
        // Session never answers and never swaps: Migrated window fails,
        // forced reconnect fails, escalation fires with a message.
        let (mut io, _tx) = MockIo::new(true, vec![sample(7, 10)]);
        let outcome = run_cycle(&mut io).await;
        assert_eq!(outcome, CycleOutcome::Escalated);
        assert_eq!(io.force_reconnects, 1);
        assert_eq!(io.escalations.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_parks_without_v4_route_until_next_event() {
        // No v4 route: no probes, no forced reconnect. A new route
        // event within the backstop window re-parks (fresh cycle).
        let (mut io, tx) = MockIo::new(false, vec![sample(7, 10)]);
        tx.send(()).expect("inject route event");
        let outcome = run_cycle(&mut io).await;
        assert_eq!(outcome, CycleOutcome::Parked);
        assert_eq!(io.force_reconnects, 0);
        assert_eq!(io.probes_sent, 0);
        assert!(io.escalations.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_escalates_from_park_when_network_stays_v4_less_and_tunnel_dead() {
        // IPv6-only backstop: no v4 route for the whole ESCALATE
        // window, no recovery on the final probe => escalate (the
        // offline grace never fires on an online-v6 network, so the
        // watchdog is the only honest exit).
        let (mut io, _tx) = MockIo::new(false, vec![sample(7, 10)]);
        let outcome = run_cycle(&mut io).await;
        assert_eq!(outcome, CycleOutcome::Escalated);
        assert_eq!(io.escalations.len(), 1);
        assert_eq!(io.force_reconnects, 0, "park path never force-reconnects");
    }

    #[tokio::test(start_paused = true)]
    async fn watchdog_coalesces_event_bursts_into_one_cycle() {
        // 5 events in a burst => exactly one nudge (one cycle), since
        // the settle window swallows the follow-ups. Dropping the
        // sender afterwards makes the main loop exit cleanly.
        let (mut io, tx) = MockIo::new(true, vec![sample(7, 10), sample(7, 11)]);
        for _ in 0..5 {
            tx.send(()).expect("inject");
        }
        // Close the source long after the cycle completes (paused time
        // advances instantly); dropping it earlier would end the settle
        // window before the cycle ever runs.
        let dropper = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            drop(tx);
        });
        run_watchdog(&mut io).await;
        dropper.await.expect("dropper task");
        assert_eq!(io.nudges, 1, "burst must collapse into one cycle");
        assert_eq!(io.force_reconnects, 0);
    }
}
