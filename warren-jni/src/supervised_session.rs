//! Supervised multi-hop session driver: the engine supervisor's state,
//! projected onto the polled `i32` the Kotlin layer reads.
//!
//! Android drives the same datapath as the desktop daemon and iOS: a
//! [`MultiHopSupervisor`] owns the session across disconnects and publishes it
//! on a watch channel, and the two [`supervised_pump`] halves pump the TUN
//! against whatever the supervisor currently publishes. This module holds the
//! Android-specific half of that: waiting for the exit-allocated address the
//! NAT remap is built on, running the pumps, and translating the supervisor's
//! session watch into [`crate::redial::SessionStatus`] for
//! `WarrenJni.getTunnelStatus()`.
//!
//! The `i32` contract is unchanged: `Connected` while a session is published,
//! `Reconnecting` while the watch publishes `None`, `Disconnected` once the
//! supervisor stops (or a redial stops being a blip, see [`SESSION_GRACE`]),
//! `Unauthorized` on a policy rejection. Nothing on the Kotlin side moves.
//!
//! Everything here is portable, so the real control loop (real supervisor,
//! real pumps, real QUIC) is exercised on the host against a loopback exit;
//! only the `AndroidTun` it wraps is device-bound.
//!
//! [`MultiHopSupervisor`]: warrenguard_transport::supervisor::MultiHopSupervisor
//! [`supervised_pump`]: warrenguard_transport::supervised_pump

#![cfg(any(test, all(target_os = "android", feature = "tunnel")))]

use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use tokio::sync::watch;
use warrenguard_multihop::RejectionReason;
use warrenguard_transport::IpAssignSpec;
use warrenguard_transport::supervised_pump::{
    DaitaShared, run_downlink_with_daita, run_uplink_with_daita,
};
use warrenguard_transport::supervisor::ClientWatch;
#[cfg(test)]
use warrenguard_transport::supervisor::ReconnectObserver;
use warrenguard_transport_core::PacketDevice;

use crate::redial::SessionStatus;
use crate::status_watch::StatusCell;

/// How long the datapath may stay without a published session before the
/// Kotlin fail-closed policy is handed back control.
///
/// The supervisor retries forever by design; on Android that would keep
/// `getTunnelStatus()` at `Reconnecting` through a real outage, and the
/// adapter's blocking-mode + connectivity-gated retry would never engage. The
/// bound keeps the pre-supervisor meaning of the contract: a redial is a blip,
/// and past this window "the network is gone" is the honest answer. Sized to
/// cover a handshake plus several redials at the 2 s backoff ceiling.
pub(crate) const SESSION_GRACE: Duration = Duration::from_secs(20);

/// One edge of the supervisor's session watch.
///
/// Mirror of the iOS `watch_transition` rule (`warren-ios`): a reconnection
/// MUST re-emit `Connected`, otherwise the UI stays at `Reconnecting` forever
/// after a handover.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SessionEdge {
    /// A session became live.
    Connected,
    /// A previously live session dropped; a redial is in flight.
    Reconnecting,
    /// No edge to report (steady state, live or not).
    Idle,
}

/// Pure edge detector for the session watch, extracted so the status ordering
/// is unit-testable without a live supervisor.
pub(crate) fn session_edge(live: bool, was_connected: bool) -> SessionEdge {
    match (live, was_connected) {
        (true, false) => SessionEdge::Connected,
        (false, true) => SessionEdge::Reconnecting,
        _ => SessionEdge::Idle,
    }
}

/// Why the supervised session stopped, from the Kotlin contract's viewpoint.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SessionEnd {
    /// The supervisor stopped publishing, a reconnect outlived
    /// [`SESSION_GRACE`], or the datapath was declared dead. Kotlin's
    /// fail-closed policy takes over, exactly as it does for any other session
    /// death.
    Down,
    /// The exit refused the setup with a policy rejection.
    Rejected(RejectionReason),
    /// The exit moved the session to a different inner address. The NAT remap
    /// is fixed at session start (the Android TUN address is immutable after
    /// `establish()`), so the session ends rather than pump packets the exit's
    /// anti-spoof gate would drop.
    AddressChanged,
}

/// Bind the supervisor's per-redial observer to a counter standing in for the
/// one Kotlin reads through `getAutoRecoveryCount()`. The engine fires it once
/// per successful reconnect and never on the initial connect, which is exactly
/// the counter's documented meaning (automatic recoveries, never user actions).
/// The production observer lives in `android_jni`, where it also wakes the
/// Kotlin waiter.
#[cfg(test)]
pub(crate) fn reconnect_observer(counter: &'static AtomicI32) -> ReconnectObserver {
    Arc::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    })
}

/// The supervisor-side channels this driver consumes.
pub(crate) struct SupervisedInputs {
    /// Live session watch (`Some` = connected, `None` = redial in flight).
    pub sessions: ClientWatch,
    /// Terminal policy rejection published by the supervisor.
    pub fatal: watch::Receiver<Option<RejectionReason>>,
    /// Latched `true` once consecutive redials each carried zero application
    /// downlink: the tunnel keeps establishing while the user's traffic goes
    /// nowhere. Without it a redial loop would keep republishing `Connected`
    /// over a dead datapath, which is exactly the masquerade the pre-supervisor
    /// Android path could not produce (a dead pump surfaced `Disconnected`).
    pub datapath_dead: watch::Receiver<bool>,
    /// Exit-allocated inner addresses, published on every (re)connect.
    pub assigns: watch::Receiver<Option<IpAssignSpec>>,
    /// Latched `true` once the migration watchdog gave up on a network change
    /// (`crate::migration`): the path could be neither migrated nor redialed
    /// within the engine's escalation window. The session ends on it, which is
    /// the only way an unrecoverable path stops reporting `Connected` while the
    /// supervisor still holds a session it cannot use.
    pub escalated: watch::Receiver<bool>,
    /// Latched `true` once the in-tunnel egress probe (`crate::egress_probe`)
    /// declared the exit dead: the QUIC session is alive, the client to exit
    /// datapath answers, and the exit forwards nothing to the internet. Neither
    /// `datapath_dead` nor the goodput prober can see that class, so without it
    /// the driver would keep publishing `Connected` over a tunnel with no
    /// egress. Ending the session is how Android escalates a reconnect: the
    /// Kotlin fail-closed policy blocks traffic and redials onto a fresh
    /// circuit, the same recovery the desktop daemon reaches through its shared
    /// reconnect channel.
    pub egress_dead: watch::Receiver<bool>,
}

/// Abort a spawned task when its owner returns, so a session that ends never
/// leaves a pump holding a watch receiver (which would keep the supervisor
/// dialing for a tunnel nobody reads).
pub(crate) struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl AbortOnDrop {
    pub(crate) fn spawn<F>(task: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self(tokio::spawn(task))
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Run one supervised session to its end: wait for the exit-allocated address,
/// wrap the TUN with it, pump both directions against the supervisor's live
/// session, and publish the Kotlin-facing status (waking its waiter on every
/// edge) until something ends it.
///
/// `wrap_tun` receives the first [`IpAssignSpec`] and returns the packet device
/// the pumps drive (on Android, the 1:1 NAT remap onto the exit-assigned
/// source). It is called at most once: no assignment means no datapath, which
/// fails closed rather than pumping packets the exit would drop.
pub(crate) async fn run_supervised<T, F>(
    mut inputs: SupervisedInputs,
    wrap_tun: F,
    daita: DaitaShared,
    status: &StatusCell,
    grace: Duration,
) -> SessionEnd
where
    T: PacketDevice + Clone,
    F: FnOnce(IpAssignSpec) -> T,
{
    let pinned = match await_first_assign(&mut inputs, grace).await {
        Ok(spec) => spec,
        Err(end) => return end,
    };
    let tun = wrap_tun(pinned);

    // One shared DAITA state across both halves, and the cross-pump wake-up
    // the downlink uses when it schedules a closer action timer.
    let state_changed = Arc::new(tokio::sync::Notify::new());
    let _uplink = AbortOnDrop::spawn({
        let rx = inputs.sessions.clone();
        let tun = tun.clone();
        let daita = daita.clone();
        let notify = state_changed.clone();
        async move {
            if let Err(e) = run_uplink_with_daita(rx, tun, daita, notify).await {
                log::warn!("multi-hop uplink terminated: {e}");
            }
        }
    });
    let _downlink = AbortOnDrop::spawn({
        let rx = inputs.sessions.clone();
        let notify = state_changed.clone();
        async move {
            if let Err(e) = run_downlink_with_daita(rx, tun, daita, notify, None, None).await {
                log::warn!("multi-hop downlink terminated: {e}");
            }
        }
    });

    drive_status(inputs, pinned, status, grace).await
}

/// Wait for the exit's first `IpAssign`. Bounded: without it the data plane
/// has no source address the exit accepts, so a silent wait would be a
/// blackhole, not a connection.
async fn await_first_assign(
    inputs: &mut SupervisedInputs,
    grace: Duration,
) -> Result<IpAssignSpec, SessionEnd> {
    loop {
        if let Some(reason) = *inputs.fatal.borrow_and_update() {
            return Err(SessionEnd::Rejected(reason));
        }
        if let Some(spec) = *inputs.assigns.borrow_and_update() {
            return Ok(spec);
        }
        let changed = async {
            tokio::select! {
                r = inputs.assigns.changed() => r.is_ok(),
                r = inputs.fatal.changed() => r.is_ok(),
            }
        };
        match tokio::time::timeout(grace, changed).await {
            Ok(true) => {}
            Ok(false) => return Err(terminal_end(inputs)),
            Err(_) => return Err(SessionEnd::Down),
        }
    }
}

/// Publish `SessionStatus` from the supervisor's session watch until the
/// session ends.
async fn drive_status(
    mut inputs: SupervisedInputs,
    pinned: IpAssignSpec,
    status: &StatusCell,
    grace: Duration,
) -> SessionEnd {
    let mut was_connected = false;
    loop {
        // A definitive rejection wins over every other signal: a redial hits
        // the same refusal, so it must not read as a reconnect.
        if let Some(reason) = *inputs.fatal.borrow_and_update() {
            return SessionEnd::Rejected(reason);
        }
        // A tunnel that establishes but carries nothing must never keep
        // reporting Connected.
        if *inputs.datapath_dead.borrow_and_update() {
            log::warn!("multi-hop datapath dead across consecutive redials; ending the session");
            return SessionEnd::Down;
        }
        // A network change the watchdog could neither migrate nor redial: the
        // supervisor may still hold a session, so nothing else here would ever
        // stop reporting Connected over it.
        if *inputs.escalated.borrow_and_update() {
            log::warn!("multi-hop migration watchdog escalated; ending the session");
            return SessionEnd::Down;
        }
        // The exit answers the client but forwards nothing: every other signal
        // here reads that circuit as healthy, so this is the only thing that
        // can stop it from being reported as Connected.
        if *inputs.egress_dead.borrow_and_update() {
            log::warn!(
                "multi-hop in-tunnel egress probe declared the exit dead; ending the session"
            );
            return SessionEnd::Down;
        }
        if let Some(spec) = *inputs.assigns.borrow_and_update()
            && remap_moved(spec, pinned)
        {
            return SessionEnd::AddressChanged;
        }
        let live = inputs.sessions.borrow_and_update().is_some();
        match session_edge(live, was_connected) {
            SessionEdge::Connected => {
                was_connected = true;
                status.store(SessionStatus::Connected as i32);
            }
            SessionEdge::Reconnecting => {
                was_connected = false;
                status.store(SessionStatus::Reconnecting as i32);
            }
            SessionEdge::Idle => {}
        }

        let changed = async {
            tokio::select! {
                r = inputs.sessions.changed() => r.is_ok(),
                r = inputs.fatal.changed() => r.is_ok(),
                r = inputs.datapath_dead.changed() => r.is_ok(),
                r = inputs.assigns.changed() => r.is_ok(),
                r = inputs.escalated.changed() => r.is_ok(),
                // Pattern-guarded, unlike the others: those channels belong to
                // the supervisor, so losing one means the supervisor is gone
                // and the session with it. The probe is optional (a disable
                // knob, and it returns once it has escalated), so its channel
                // closing says nothing about the exit; the branch is dropped
                // and the wait continues on the signals that do mean something.
                Ok(()) = inputs.egress_dead.changed() => true,
            }
        };
        // Without a live session the wait is bounded: past the grace a redial
        // is no longer a blip (see [`SESSION_GRACE`]).
        let alive = if live {
            changed.await
        } else {
            match tokio::time::timeout(grace, changed).await {
                Ok(alive) => alive,
                Err(_) => return SessionEnd::Down,
            }
        };
        if !alive {
            return terminal_end(&mut inputs);
        }
    }
}

/// Whether a fresh assignment invalidates the NAT remap. Only the two
/// addresses the remap rewrites matter; a change in prefix or gateway is
/// nothing the Android datapath carries (the interface's own addressing is
/// fixed by `VpnService.Builder`).
fn remap_moved(fresh: IpAssignSpec, pinned: IpAssignSpec) -> bool {
    fresh.assigned != pinned.assigned || fresh.assigned_v6 != pinned.assigned_v6
}

/// The end to report once a watch sender is gone: a rejection published just
/// before the supervisor returned still outranks a plain teardown.
fn terminal_end(inputs: &mut SupervisedInputs) -> SessionEnd {
    match *inputs.fatal.borrow_and_update() {
        Some(reason) => SessionEnd::Rejected(reason),
        None => SessionEnd::Down,
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use ed25519_dalek::SigningKey;
    use warrenguard_daita::DaitaState;
    use warrenguard_multihop::ExitId;
    use warrenguard_transport::IpAssignChannel;
    use warrenguard_transport::supervisor::MultiHopSupervisor;
    use warrenguard_transport_core::FakeTun;

    use super::*;
    use crate::loopback_exit::{ASSIGNED_V4, LoopbackExit, config_for};

    /// Recovery counter the supervisor's redial observer feeds, standing in
    /// for the JNI layer's `AUTO_RECOVERY_COUNT` static.
    static RECOVERIES: AtomicI32 = AtomicI32::new(0);

    /// Await `status` reaching `want` (or fail the test) the way Kotlin does:
    /// woken by the cell on every edge, never by a timer.
    async fn await_status(status: &StatusCell, want: SessionStatus, what: &str) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut seen = 0;
        while status.load() != want as i32 {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, status.changed_since(seen)).await {
                Ok(generation) => seen = generation,
                Err(_) => panic!(
                    "{what}: status stayed {} instead of reaching {want:?}",
                    status.load()
                ),
            }
        }
    }

    /// The Kotlin-facing status must walk Connected -> Reconnecting ->
    /// Connected across an exit that dies and comes back, driven by the real
    /// supervisor and the real supervised pumps: that IS the contract Kotlin
    /// polls, and the redial must never surface as `Disconnected` (which would
    /// engage the fail-closed teardown for a blip).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn supervised_session_publishes_reconnecting_then_connected() {
        let operational = SigningKey::from_bytes(&[0x42; 32]);
        let exit = LoopbackExit::spawn(&operational, ExitId::from_bytes([0x51; 16]));
        let assigns = IpAssignChannel::new();
        let (supervisor, sessions) = MultiHopSupervisor::new(config_for(
            &exit,
            &operational,
            &assigns,
            Some(reconnect_observer(&RECOVERIES)),
        ));
        let fatal = supervisor.fatal_rx();
        let (_escalated_tx, escalated) = watch::channel(false);
        let (_egress_tx, egress_dead) = watch::channel(false);
        let inputs = SupervisedInputs {
            sessions,
            fatal,
            datapath_dead: supervisor.datapath_dead_rx(),
            assigns: assigns.subscribe(),
            escalated,
            egress_dead,
        };
        drop(assigns);
        let supervisor_task = tokio::spawn(supervisor.run());

        RECOVERIES.store(0, Ordering::Relaxed);
        static STATUS: StatusCell = StatusCell::new(SessionStatus::Connecting as i32);
        STATUS.store(SessionStatus::Connecting as i32);
        let daita: DaitaShared = Arc::new(parking_lot::Mutex::new(DaitaState::disabled()));
        let driver = tokio::spawn(async move {
            run_supervised(
                inputs,
                |spec| {
                    assert_eq!(
                        spec.assigned,
                        Ipv4Addr::from(ASSIGNED_V4),
                        "the NAT remap must be built on the exit-allocated address"
                    );
                    FakeTun::new()
                },
                daita,
                &STATUS,
                SESSION_GRACE,
            )
            .await
        });

        await_status(&STATUS, SessionStatus::Connected, "initial connect").await;
        assert_eq!(
            RECOVERIES.load(Ordering::Relaxed),
            0,
            "the initial connect is not an automatic recovery"
        );

        exit.kill();
        await_status(&STATUS, SessionStatus::Reconnecting, "exit killed").await;

        exit.restart();
        await_status(&STATUS, SessionStatus::Connected, "exit restarted").await;
        assert!(
            RECOVERIES.load(Ordering::Relaxed) >= 1,
            "the redial that landed must bump the auto-recovery counter"
        );
        assert!(
            exit.accepted.load(Ordering::Relaxed) >= 2,
            "the supervisor must have dialed the exit again, not reused the dead session"
        );

        driver.abort();
        supervisor_task.abort();
    }

    #[test]
    fn a_reconnection_re_emits_connected() {
        assert_eq!(session_edge(true, false), SessionEdge::Connected);
        assert_eq!(session_edge(false, true), SessionEdge::Reconnecting);
        assert_eq!(
            session_edge(true, true),
            SessionEdge::Idle,
            "a steady live session publishes no edge"
        );
        assert_eq!(
            session_edge(false, false),
            SessionEdge::Idle,
            "before the first session there is nothing to report: the JNI entry \
             already published Connecting"
        );
    }

    fn spec(v4: [u8; 4]) -> IpAssignSpec {
        IpAssignSpec {
            assigned: Ipv4Addr::from(v4),
            prefix_len: 24,
            gateway: Ipv4Addr::new(10, 77, 0, 1),
            assigned_v6: None,
            prefix_len_v6: 0,
            gateway_v6: None,
        }
    }

    /// Channels shaped like the supervisor's, so the driver's end conditions
    /// are exercised without standing up a session. The senders are kept alive
    /// by the caller: dropping one is itself an end condition.
    struct Scripted {
        inputs: SupervisedInputs,
        _sessions: watch::Sender<Option<Arc<warrenguard_transport::bundle::MultiHopBundle>>>,
        fatal: watch::Sender<Option<RejectionReason>>,
        datapath_dead: watch::Sender<bool>,
        assigns: watch::Sender<Option<IpAssignSpec>>,
        escalated: watch::Sender<bool>,
        egress_dead: watch::Sender<bool>,
    }

    fn scripted() -> Scripted {
        let (sessions_tx, sessions) = watch::channel(None);
        let (fatal_tx, fatal) = watch::channel(None);
        let (dead_tx, datapath_dead) = watch::channel(false);
        let (assigns_tx, assigns) = watch::channel(Some(spec([10, 77, 0, 2])));
        let (escalated_tx, escalated) = watch::channel(false);
        let (egress_tx, egress_dead) = watch::channel(false);
        Scripted {
            inputs: SupervisedInputs {
                sessions,
                fatal,
                datapath_dead,
                assigns,
                escalated,
                egress_dead,
            },
            _sessions: sessions_tx,
            fatal: fatal_tx,
            datapath_dead: dead_tx,
            assigns: assigns_tx,
            escalated: escalated_tx,
            egress_dead: egress_tx,
        }
    }

    /// A probe that is not running (the disable knob, or one that already
    /// escalated and returned) closes its channel. That says nothing about the
    /// exit, so it must never end a session: treating it like a gone supervisor
    /// would tear the tunnel down the moment the probe was switched off.
    #[tokio::test]
    async fn a_closed_egress_channel_never_ends_a_session() {
        let scripted = scripted();
        drop(scripted.egress_dead);
        let status = StatusCell::new(SessionStatus::Connected as i32);
        // The grace is out of reach, so returning at all within the timeout can
        // only be the closed channel being mistaken for a terminal signal.
        let outcome = tokio::time::timeout(
            Duration::from_millis(500),
            drive_status(
                scripted.inputs,
                spec([10, 77, 0, 2]),
                &status,
                Duration::from_secs(600),
            ),
        )
        .await;
        assert!(
            outcome.is_err(),
            "a closed probe channel must not end the session, got {outcome:?}"
        );
    }

    /// An exit that keeps the QUIC session alive while forwarding nothing to
    /// the internet must stop reading as `Connected`: the in-tunnel egress
    /// probe is the only signal that sees that class, so the driver has to end
    /// the session on it exactly as it does for a dead datapath.
    #[tokio::test]
    async fn an_egress_dead_verdict_hands_back_to_the_fail_closed_policy() {
        let scripted = scripted();
        scripted.egress_dead.send(true).expect("receiver alive");
        let status = StatusCell::new(SessionStatus::Connected as i32);
        // The grace is deliberately out of reach: only the verdict itself can
        // end this session, so a driver that ignored it would hang here instead
        // of quietly passing on the timeout.
        let end = tokio::time::timeout(
            Duration::from_secs(2),
            drive_status(
                scripted.inputs,
                spec([10, 77, 0, 2]),
                &status,
                Duration::from_secs(600),
            ),
        )
        .await
        .expect("the egress-dead verdict must end the session on its own");
        assert_eq!(end, SessionEnd::Down);
    }

    /// A tunnel that keeps establishing while carrying zero downlink must not
    /// masquerade as Connected: the supervisor's escalation ends the session so
    /// the Kotlin fail-closed policy runs, as a dead pump did before.
    #[tokio::test]
    async fn a_dead_datapath_hands_back_to_the_fail_closed_policy() {
        let scripted = scripted();
        scripted.datapath_dead.send(true).expect("receiver alive");
        let status = StatusCell::new(SessionStatus::Connecting as i32);
        // The grace is deliberately out of reach: only the escalation itself
        // can end this session, so a driver that ignored the signal would hang
        // here instead of quietly passing on the timeout.
        let end = tokio::time::timeout(
            Duration::from_secs(2),
            drive_status(
                scripted.inputs,
                spec([10, 77, 0, 2]),
                &status,
                Duration::from_secs(600),
            ),
        )
        .await
        .expect("the dead-datapath escalation must end the session on its own");
        assert_eq!(end, SessionEnd::Down);
    }

    /// A path the migration watchdog could neither migrate nor redial must end
    /// the session: that is what publishes `Disconnected` to Kotlin, and the
    /// adapter's handover fallback (blackhole interface FIRST, teardown after)
    /// runs off it. Without this the supervisor still holds a session it cannot
    /// use and `getTunnelStatus()` would keep answering `Connected`.
    #[tokio::test]
    async fn a_watchdog_escalation_hands_back_to_the_fail_closed_policy() {
        let scripted = scripted();
        scripted.escalated.send(true).expect("receiver alive");
        let status = StatusCell::new(SessionStatus::Connected as i32);
        // The grace is deliberately out of reach: only the escalation itself
        // can end this session, so a driver that ignored the signal would hang
        // here instead of quietly passing on the timeout.
        let end = tokio::time::timeout(
            Duration::from_secs(2),
            drive_status(
                scripted.inputs,
                spec([10, 77, 0, 2]),
                &status,
                Duration::from_secs(600),
            ),
        )
        .await
        .expect("the watchdog escalation must end the session on its own");
        assert_eq!(end, SessionEnd::Down);
    }

    /// A redial that never lands must stop reading as a blip: past the grace
    /// the driver hands back to the Kotlin fail-closed policy instead of
    /// leaving `getTunnelStatus()` at `Reconnecting` for a dead network.
    #[tokio::test]
    async fn a_session_that_never_returns_hands_back_to_the_fail_closed_policy() {
        let scripted = scripted();
        let status = StatusCell::new(SessionStatus::Connecting as i32);
        let end = drive_status(
            scripted.inputs,
            spec([10, 77, 0, 2]),
            &status,
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(end, SessionEnd::Down);
        assert_eq!(
            status.load(),
            SessionStatus::Connecting as i32,
            "a session that never came up must not be reported as a reconnect"
        );
    }

    /// A policy rejection is terminal: it must surface as `Rejected` (which the
    /// caller maps to `Unauthorized`), never as a plain teardown that Kotlin
    /// would retry into the same refusal.
    #[tokio::test]
    async fn a_policy_rejection_outranks_a_teardown() {
        let scripted = scripted();
        scripted
            .fatal
            .send(Some(RejectionReason::NotAllowlisted))
            .expect("receiver alive");
        let status = StatusCell::new(SessionStatus::Connecting as i32);
        let end = drive_status(
            scripted.inputs,
            spec([10, 77, 0, 2]),
            &status,
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(end, SessionEnd::Rejected(RejectionReason::NotAllowlisted));
    }

    /// The NAT remap is fixed for the session's life, so an exit that moves the
    /// inner address must end the session rather than let the pumps emit a
    /// source the exit's anti-spoof gate drops.
    #[tokio::test]
    async fn a_moved_inner_address_ends_the_session() {
        let scripted = scripted();
        scripted
            .assigns
            .send(Some(spec([10, 77, 0, 9])))
            .expect("receiver alive");
        let status = StatusCell::new(SessionStatus::Connecting as i32);
        let end = drive_status(
            scripted.inputs,
            spec([10, 77, 0, 2]),
            &status,
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(end, SessionEnd::AddressChanged);
    }

    #[test]
    fn only_the_remapped_addresses_decide_a_move() {
        let pinned = spec([10, 77, 0, 2]);
        let mut same_address_other_gateway = pinned;
        same_address_other_gateway.gateway = Ipv4Addr::new(10, 77, 0, 254);
        assert!(
            !remap_moved(same_address_other_gateway, pinned),
            "the Android interface addressing is fixed by VpnService.Builder: \
             only the rewritten addresses can invalidate the remap"
        );
        assert!(remap_moved(spec([10, 77, 0, 3]), pinned));
    }
}
