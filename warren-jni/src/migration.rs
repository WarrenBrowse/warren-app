//! Android bindings for the engine's migration watchdog.
//!
//! The decision loop, its constants and the [`MigrationIo`] trait live in the
//! engine (`warrenguard_transport::migration_watchdog`), shared by every client
//! surface; this module supplies the Android IO. A Wi-Fi to cellular handover
//! then rebinds the live QUIC endpoint and revalidates the path in about one
//! RTT instead of tearing the session down for a full re-handshake.
//!
//! Android has neither a bypass to nudge nor a host route to install: the
//! socket the datapath egresses on is escaped by `VpnService.protect`, which
//! [`RebindPolicy::Protect`] re-applies to the fresh socket BEFORE quinn can
//! send on it. The engine refuses the rebind when `protect` refuses, so a
//! migration can never put an unprotected socket on the wire; the session then
//! keeps its current socket and the cycle falls back to a redial.
//!
//! When nothing recovers, [`MigrationIo::escalate`] latches a watch the session
//! driver reads, which ends the session and hands the Kotlin fail-closed policy
//! back its own job (blackhole interface, then reconnect).

#![cfg(any(test, all(target_os = "android", feature = "tunnel")))]

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{mpsc, watch};
use warrenguard_transport::bundle::MultiHopBundle;
use warrenguard_transport::migration_watchdog::{MigrationIo, RxSample};
use warrenguard_transport::multihop::RebindPolicy;
use warrenguard_transport::network_monitor::{PROBE_ANCHOR, preferred_source_ip};
use warrenguard_transport::supervisor::{ClientWatch, SupervisorHandle};

/// Sender half of the route-event channel the watchdog consumes. Installed
/// when a session attempt starts, cleared at teardown.
///
/// Global rather than session-scoped because the Kotlin caller
/// (`WarrenQuinnAdapter`'s `ConnectivityManager.NetworkCallback`) holds no
/// session handle, and `WarrenJni` exposes a single tunnel at a time
/// (`ACTIVE_TUNNEL`), so there is never a second watchdog to disambiguate.
static PATH_CHANGE_TX: Mutex<Option<mpsc::UnboundedSender<()>>> = Mutex::new(None);

/// Install a fresh route-event channel and hand back the receiver the watchdog
/// consumes. Replaces any previous sender, so a watchdog left over from a
/// torn-down attempt stops being fed and sees its source close.
pub(crate) fn install_path_change_channel() -> mpsc::UnboundedReceiver<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    *PATH_CHANGE_TX.lock() = Some(tx);
    rx
}

/// Drop the sender so the watchdog's event source reports closed, which is the
/// engine's documented teardown exit for `run_watchdog`.
pub(crate) fn clear_path_change_channel() {
    *PATH_CHANGE_TX.lock() = None;
}

/// Clears the installed sender when the session attempt that installed it
/// returns, so a Kotlin notification never feeds a watchdog that no longer has
/// a session to migrate.
pub(crate) struct PathChannelGuard;

impl Drop for PathChannelGuard {
    fn drop(&mut self) {
        clear_path_change_channel();
    }
}

/// Wake the watchdog on a Kotlin-reported network handover. Unbounded and
/// lossless: a handover emits several `NetworkCallback` updates and the engine
/// coalesces the burst into one verification cycle.
pub(crate) fn notify_path_change() {
    if let Some(tx) = PATH_CHANGE_TX.lock().as_ref() {
        let _ = tx.send(());
    }
}

/// Android bindings for [`MigrationIo`].
pub(crate) struct AndroidMigrationIo {
    route_events: mpsc::UnboundedReceiver<()>,
    client_rx: ClientWatch,
    supervisor: SupervisorHandle,
    /// Latched by [`MigrationIo::escalate`]; read by the session driver, which
    /// ends the session on it (see `crate::supervised_session`).
    escalated: watch::Sender<bool>,
}

impl AndroidMigrationIo {
    pub(crate) fn new(
        route_events: mpsc::UnboundedReceiver<()>,
        client_rx: ClientWatch,
        supervisor: SupervisorHandle,
        escalated: watch::Sender<bool>,
    ) -> Self {
        Self {
            route_events,
            client_rx,
            supervisor,
            escalated,
        }
    }

    fn current_client(&self) -> Option<Arc<MultiHopBundle>> {
        self.client_rx.borrow().clone()
    }
}

impl MigrationIo for AndroidMigrationIo {
    async fn next_route_event(&mut self) -> bool {
        // Cancel-safe, which the trait requires: the burst coalescer and the
        // park both drop a pending call and issue a fresh one, and
        // `UnboundedReceiver::recv` consumes nothing when dropped.
        self.route_events.recv().await.is_some()
    }

    async fn has_v4_default_route(&mut self) -> bool {
        // Android exposes no routing table to an unprivileged app: the source
        // address the kernel would pick for a v4 destination is the portable
        // stand-in, and it is exactly what the dial would get.
        preferred_source_ip(PROBE_ANCHOR).is_some()
    }

    async fn nudge_bypass(&mut self) {
        // No per-socket bypass on Android: an app cannot set a fwmark or bind
        // an interface, and `VpnService.protect` carries the whole escape.
    }

    fn session_can_migrate(&mut self) -> bool {
        // No published session counts as migratable: the cycle handles the
        // `None` case on every other IO call, and the redial that follows dials
        // a fresh protected socket anyway.
        self.current_client()
            .is_none_or(|client| !client.is_over_carrier())
    }

    async fn ensure_route_escape(&mut self) -> bool {
        // Nothing to install: `VpnService.protect` is destination-independent
        // and the rebind re-applies it to the fresh socket, so there is no
        // window in which the carrier holds neither escape.
        true
    }

    async fn rebind_endpoint(&mut self) {
        let Some(client) = self.current_client() else {
            return;
        };
        // `Protect` is the whole reason the policy exists: the engine builds
        // the fresh wildcard socket, runs `VpnService.protect` on its fd, and
        // refuses the rebind if that fails, so quinn never receives a socket
        // that would egress into the tunnel it carries. Going through
        // `rebind_wildcard` (hence `Endpoint::rebind`) is also what rotates the
        // connection ID, so the two paths are not correlatable by an observer.
        match client.rebind_wildcard(RebindPolicy::Protect) {
            Ok(()) => {
                log::info!("migration watchdog: rebound the QUIC endpoint onto a fresh socket")
            }
            // No-log: `RebindError` carries an errno category, never an address.
            Err(e) => log::debug!("migration watchdog: endpoint rebind refused: {e}"),
        }
    }

    async fn send_probe(&mut self) {
        if let Some(client) = self.current_client()
            && let Err(e) = client.send_daita_padding().await
        {
            log::debug!("migration watchdog: liveness probe send failed: {e}");
        }
    }

    fn rx_sample(&mut self) -> Option<RxSample> {
        self.current_client().map(|client| {
            // Mix the local port into the identity: the Arc address can be
            // reused by the very next session (ABA), the wildcard bind's
            // ephemeral port cannot within any realistic window.
            // Rotate rather than shift: Android ships 32-bit ABIs, where a
            // shift wide enough to clear a 64-bit pointer's low bits is an
            // overflow the compiler rejects outright.
            let port = client.local_addr().map(|a| a.port()).unwrap_or(0);
            RxSample {
                id: (Arc::as_ptr(&client) as usize) ^ usize::from(port).rotate_left(17),
                rx_datagrams: client.quinn_stats().udp_rx.datagrams,
            }
        })
    }

    fn force_reconnect(&mut self) -> bool {
        self.supervisor.force_reconnect()
    }

    fn escalate(&mut self, msg: String) {
        // No-log: the engine's message is a failure category, never identity.
        log::warn!("migration watchdog: escalating to the fail-closed policy: {msg}");
        // Ends the session, which publishes `Disconnected` to Kotlin. The
        // adapter then runs its handover fallback: blackhole interface up
        // FIRST, teardown after.
        let _ = self.escalated.send(true);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ed25519_dalek::SigningKey;
    use warrenguard_multihop::ExitId;
    use warrenguard_transport::IpAssignChannel;
    use warrenguard_transport::supervisor::MultiHopSupervisor;

    use super::*;
    use crate::loopback_exit::{LoopbackExit, config_for};

    /// A watchdog IO over a supervisor that is built but never run, so the
    /// session watch stays empty: the "no published session" contract the
    /// engine loop relies on.
    ///
    /// `route_events` is supplied by the caller rather than taken from
    /// [`install_path_change_channel`]: that channel is process-global (one
    /// tunnel at a time on Android), so a second test installing over it would
    /// silently steal the first one's notifications.
    fn idle_io(
        route_events: mpsc::UnboundedReceiver<()>,
        exit_id: u8,
    ) -> (AndroidMigrationIo, watch::Receiver<bool>, LoopbackExit) {
        let operational = SigningKey::from_bytes(&[0x42; 32]);
        let exit = LoopbackExit::spawn(&operational, ExitId::from_bytes([exit_id; 16]));
        let assigns = IpAssignChannel::new();
        let (supervisor, sessions) =
            MultiHopSupervisor::new(config_for(&exit, &operational, &assigns, None));
        let (escalated_tx, escalated_rx) = watch::channel(false);
        let io = AndroidMigrationIo::new(route_events, sessions, supervisor.handle(), escalated_tx);
        (io, escalated_rx, exit)
    }

    /// `WarrenJni.notifyNetworkChanged()` is the only thing that turns an
    /// Android handover into a QUIC migration, so it must wake the very source
    /// the engine's `next_route_event` consumes, and the teardown guard must
    /// report the source closed (how `run_watchdog` stops instead of probing a
    /// torn down session forever).
    #[tokio::test]
    async fn a_network_change_notification_wakes_the_route_event_source() {
        let (mut io, _escalated, _exit) = idle_io(install_path_change_channel(), 0x71);

        notify_path_change();

        let woke = tokio::time::timeout(Duration::from_secs(5), io.next_route_event())
            .await
            .expect("the notify must wake the watchdog's event source");
        assert!(woke, "a network change is a wake, never a source close");

        drop(PathChannelGuard);
        let alive = tokio::time::timeout(Duration::from_secs(5), io.next_route_event())
            .await
            .expect("a closed source must resolve at once, not hang the watchdog");
        assert!(
            !alive,
            "the teardown guard must report the source closed so the loop exits"
        );
    }

    /// The escalation is the watchdog's hand-back: it must latch the watch the
    /// session driver reads, or an unrecoverable path would sit "Connected"
    /// with the Kotlin fail-closed policy never engaging.
    #[tokio::test]
    async fn an_escalation_latches_the_session_ending_watch() {
        let (mut io, escalated, _exit) = idle_io(mpsc::unbounded_channel().1, 0x73);
        assert!(!*escalated.borrow(), "nothing escalated yet");

        io.escalate("tunnel path not recovered after network change".to_owned());

        assert!(
            *escalated.borrow(),
            "escalating must end the session so Kotlin's fail-closed policy runs"
        );
    }

    /// With no published session every IO call must answer the way the engine
    /// loop expects, or a handover that lands between sessions would rebind a
    /// session that does not exist or read a stale liveness baseline.
    #[tokio::test]
    async fn an_empty_session_watch_answers_the_no_session_contract() {
        let (mut io, _escalated, _exit) = idle_io(mpsc::unbounded_channel().1, 0x74);

        assert!(
            io.session_can_migrate(),
            "no session must not be mistaken for a carrier session (which would \
             skip the rebind forever)"
        );
        assert!(io.ensure_route_escape().await, "protect carries the escape");
        assert!(io.rx_sample().is_none(), "no session, no liveness baseline");
        assert!(
            !io.force_reconnect(),
            "there is no session to close, so the cycle must learn nothing happened"
        );
        // Must not panic nor block: the cycle calls it on every wake.
        io.rebind_endpoint().await;
    }

    /// The rebind is fail-closed by construction: the fresh socket only reaches
    /// quinn once the escape policy applied to it. Off Android there is no
    /// `VpnService.protect` hook, so `RebindPolicy::Protect` is refused, and
    /// the live session must keep the socket it already had rather than egress
    /// unescaped. On device the same refusal fires when `protect` returns
    /// false; only the reason differs.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_refused_protect_policy_leaves_the_live_socket_untouched() {
        let operational = SigningKey::from_bytes(&[0x42; 32]);
        let exit = LoopbackExit::spawn(&operational, ExitId::from_bytes([0x72; 16]));
        let assigns = IpAssignChannel::new();
        let (supervisor, mut sessions) =
            MultiHopSupervisor::new(config_for(&exit, &operational, &assigns, None));
        let (escalated_tx, _escalated_rx) = watch::channel(false);
        let mut io = AndroidMigrationIo::new(
            mpsc::unbounded_channel().1,
            sessions.clone(),
            supervisor.handle(),
            escalated_tx,
        );
        let supervisor_task = tokio::spawn(supervisor.run());

        let client = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(client) = sessions.borrow_and_update().clone() {
                    return client;
                }
                sessions.changed().await.expect("supervisor alive");
            }
        })
        .await
        .expect("the loopback exit must publish a session");
        let before = client
            .local_addr()
            .expect("a live endpoint has a local addr");

        io.rebind_endpoint().await;

        assert_eq!(
            client
                .local_addr()
                .expect("the endpoint must still hold a socket"),
            before,
            "a refused escape policy must leave the live socket in place, never \
             hand quinn an unprotected one"
        );
        supervisor_task.abort();
    }
}
