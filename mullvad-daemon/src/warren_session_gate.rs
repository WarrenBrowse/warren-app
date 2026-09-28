//! The login state gates the tunnel, and a logout runs in one order.
//!
//! A logged-out daemon has no tunnel: the session, its routes and forwarded
//! ports, and the kill switch all belong to the account that left. So a
//! connect is refused while no account is logged in, whoever asks for it (a
//! client, the boot auto-connect, a scheduled reconnect), and a logout takes
//! the tunnel down BEFORE it touches the login state or the identity. The
//! identity signs the tunnel, so the only safe moment to drop it is once the
//! tunnel reports disconnected. The wait is bounded: a teardown that does not
//! land in time leaves the login and the identity in place and reports it, and
//! the disconnect it queued stays queued, so a retry finishes the job.

use std::time::Duration;

use crate::device::PrivateDeviceEvent;
use crate::warren_env_arbitration::EnvYieldError;

/// How long a logout waits for the tunnel to report disconnected. The
/// teardown itself takes tens of milliseconds, but it runs after whatever the
/// tunnel state machine is doing when the disconnect arrives, and a dial in
/// flight can hold it for seconds.
pub const LOGOUT_TEARDOWN_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a connect was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectRefusal {
    /// Another product environment holds this machine.
    #[error(transparent)]
    EnvYield(#[from] EnvYieldError),
    /// No account is logged in.
    #[error("no account is logged in on this device: log in before connecting")]
    LoggedOut,
    /// A logout is waiting for the tunnel to come down.
    #[error("this device is logging out")]
    LoggingOut,
}

/// One logout request, and who is waiting for its answer.
#[derive(Debug)]
pub struct LogoutRequest<W> {
    pub waiter: W,
    /// The GUI's backup-confirmed sign-out, which also erases the identity.
    pub wipe_identity: bool,
}

/// What the daemon does with a logout request it has just handed over.
#[derive(Debug)]
pub enum LogoutStart<W> {
    /// The tunnel is down: carry the logout out now.
    CompleteNow(LogoutRequest<W>),
    /// The tunnel is coming down. Arm the deadline tagged with `generation`
    /// and carry the logout out once the tunnel reports disconnected.
    AwaitTeardown { generation: u64 },
    /// A logout is already waiting on the teardown; this one is answered with
    /// it.
    Joined,
}

struct PendingLogout<W> {
    requests: Vec<LogoutRequest<W>>,
    generation: u64,
}

/// The login state as far as the tunnel is concerned, and the logout waiting
/// on a teardown, if any.
pub struct SessionGate<W> {
    logged_out: bool,
    pending: Option<PendingLogout<W>>,
    generations: u64,
    /// A connect was sent to the tunnel and no transition has shown it
    /// arrived. Until one does, a disconnected transition may be one the
    /// tunnel emitted before it read the connect, with the dial still to come.
    connect_in_flight: bool,
}

impl<W> SessionGate<W> {
    pub fn new(logged_out: bool) -> Self {
        Self {
            logged_out,
            pending: None,
            generations: 0,
            connect_in_flight: false,
        }
    }

    /// Follows the account manager's login state. A revocation is not a
    /// logout: it says nothing about whether an account is logged in, and
    /// reading it as a login would reopen the gate a logout just closed.
    pub fn observe_device_event(&mut self, event: &PrivateDeviceEvent) {
        match event {
            PrivateDeviceEvent::Login(_) => self.logged_out = false,
            PrivateDeviceEvent::Logout => self.logged_out = true,
            PrivateDeviceEvent::Revoked => {}
        }
    }

    /// Records a completed logout at once, a turn before the account
    /// manager's own event, so a connect queued in between finds it.
    pub fn logged_out_now(&mut self) {
        self.logged_out = true;
    }

    /// Whether a connect may go ahead. A logout waiting on its teardown
    /// refuses too: the connect would bring back the tunnel it is waiting on.
    pub fn admit_connect(&self) -> Result<(), ConnectRefusal> {
        if self.pending.is_some() {
            Err(ConnectRefusal::LoggingOut)
        } else if self.logged_out {
            Err(ConnectRefusal::LoggedOut)
        } else {
            Ok(())
        }
    }

    /// Whether a logout is waiting on the teardown. A login or an identity
    /// import in that window would be logged out, or erased, by it.
    pub fn logout_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// A connect was sent to the tunnel.
    pub fn connect_sent(&mut self) {
        self.connect_in_flight = true;
    }

    /// Takes a logout request. `disconnect_initiated` is whether the logout's
    /// own disconnect changed the target state: when it did, a disconnected
    /// state is only the one a connect issued a moment ago has not left yet.
    pub fn request_logout(
        &mut self,
        waiter: W,
        wipe_identity: bool,
        tunnel_disconnected: bool,
        disconnect_initiated: bool,
    ) -> LogoutStart<W> {
        let request = LogoutRequest {
            waiter,
            wipe_identity,
        };
        if let Some(pending) = &mut self.pending {
            pending.requests.push(request);
            return LogoutStart::Joined;
        }
        if tunnel_disconnected && !disconnect_initiated && !self.connect_in_flight {
            return LogoutStart::CompleteNow(request);
        }
        self.generations += 1;
        let generation = self.generations;
        self.pending = Some(PendingLogout {
            requests: vec![request],
            generation,
        });
        LogoutStart::AwaitTeardown { generation }
    }

    /// The tunnel moved to a new state. Returns the logout requests to carry
    /// out now, in the order they arrived: those waiting, once the tunnel is
    /// disconnected with no connect still on its way to it.
    pub fn tunnel_transition(&mut self, disconnected: bool) -> Vec<LogoutRequest<W>> {
        if !disconnected {
            // Every connect takes the tunnel out of the disconnected state, so
            // any other state shows the connects sent so far have arrived.
            self.connect_in_flight = false;
            return Vec::new();
        }
        if self.pending.is_none() {
            // Only a waiting logout needs the doubt settled, and keeping it
            // past this point would hold up a logout that comes much later.
            self.connect_in_flight = false;
            return Vec::new();
        }
        if self.connect_in_flight {
            return Vec::new();
        }
        self.pending
            .take()
            .map(|pending| pending.requests)
            .unwrap_or_default()
    }

    /// The teardown deadline tagged `generation` fired. Returns the waiters to
    /// answer with a failure; the login state is left as it was. A deadline
    /// left over from a logout that already settled finds nothing.
    pub fn teardown_deadline(&mut self, generation: u64) -> Vec<W> {
        if !matches!(&self.pending, Some(pending) if pending.generation == generation) {
            return Vec::new();
        }
        // A connect overtaken by a disconnect in the tunnel's own queue leaves
        // it with no state to report, so the doubt it raised is dropped here,
        // and the retry finds the tunnel down.
        self.connect_in_flight = false;
        self.abandon().into_iter().map(|r| r.waiter).collect()
    }

    /// Hands back the logout still waiting, for a daemon that shuts down once
    /// its tunnel is down.
    pub fn abandon(&mut self) -> Vec<LogoutRequest<W>> {
        self.pending
            .take()
            .map(|pending| pending.requests)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectRefusal, LogoutRequest, LogoutStart, SessionGate};
    use crate::device::PrivateDeviceEvent;
    use mullvad_types::warren_pubkey::WarrenPubKey;

    const TUNNEL_UP: bool = false;
    const TUNNEL_DOWN: bool = true;
    const DISCONNECTED: bool = true;
    const LEFT_DISCONNECTED: bool = false;

    fn logged_in() -> SessionGate<&'static str> {
        SessionGate::new(false)
    }

    fn generation_of(start: LogoutStart<&'static str>) -> u64 {
        match start {
            LogoutStart::AwaitTeardown { generation } => generation,
            other => panic!("expected the logout to wait on the teardown, got {other:?}"),
        }
    }

    fn waiters(requests: Vec<LogoutRequest<&'static str>>) -> Vec<(&'static str, bool)> {
        requests
            .into_iter()
            .map(|r| (r.waiter, r.wipe_identity))
            .collect()
    }

    #[test]
    fn a_connect_is_admitted_while_logged_in() {
        assert_eq!(logged_in().admit_connect(), Ok(()));
    }

    #[test]
    fn a_connect_is_refused_while_logged_out() {
        let gate = SessionGate::<()>::new(true);

        assert_eq!(gate.admit_connect(), Err(ConnectRefusal::LoggedOut));
    }

    #[test]
    fn a_login_admits_connects_again() {
        let mut gate = SessionGate::<()>::new(true);

        gate.observe_device_event(&PrivateDeviceEvent::Login(WarrenPubKey::from_bytes(
            &[7; 32],
        )));

        assert_eq!(gate.admit_connect(), Ok(()));
    }

    #[test]
    fn a_logout_event_closes_the_gate() {
        let mut gate = logged_in();

        gate.observe_device_event(&PrivateDeviceEvent::Logout);

        assert_eq!(gate.admit_connect(), Err(ConnectRefusal::LoggedOut));
    }

    #[test]
    fn a_revocation_queued_behind_a_logout_does_not_reopen_the_gate() {
        let mut gate = logged_in();
        gate.logged_out_now();

        gate.observe_device_event(&PrivateDeviceEvent::Revoked);

        assert_eq!(gate.admit_connect(), Err(ConnectRefusal::LoggedOut));
    }

    #[test]
    fn a_connect_is_refused_while_a_logout_waits_for_the_teardown() {
        let mut gate = logged_in();

        generation_of(gate.request_logout("cli", false, TUNNEL_UP, true));

        assert_eq!(gate.admit_connect(), Err(ConnectRefusal::LoggingOut));
        assert!(gate.logout_pending());
    }

    #[test]
    fn a_logout_with_the_tunnel_already_down_completes_at_once() {
        let mut gate = logged_in();

        match gate.request_logout("gui", true, TUNNEL_DOWN, false) {
            LogoutStart::CompleteNow(request) => {
                assert_eq!((request.waiter, request.wipe_identity), ("gui", true));
            }
            other => panic!("expected an immediate logout, got {other:?}"),
        }
        assert!(!gate.logout_pending());
    }

    #[test]
    fn a_logout_with_the_tunnel_up_completes_only_once_the_tunnel_is_down() {
        let mut gate = logged_in();

        generation_of(gate.request_logout("gui", true, TUNNEL_UP, true));
        assert!(gate.tunnel_transition(LEFT_DISCONNECTED).is_empty());

        assert_eq!(
            waiters(gate.tunnel_transition(DISCONNECTED)),
            vec![("gui", true)]
        );
        assert!(
            gate.tunnel_transition(DISCONNECTED).is_empty(),
            "a logout is carried out once"
        );
    }

    #[test]
    fn a_disconnected_state_is_not_down_when_the_logout_itself_cancelled_a_connect() {
        let mut gate = logged_in();

        let start = gate.request_logout("cli", false, TUNNEL_DOWN, true);

        generation_of(start);
    }

    #[test]
    fn a_disconnected_transition_the_tunnel_sent_before_reading_a_connect_is_not_the_teardown() {
        let mut gate = logged_in();
        gate.connect_sent();
        generation_of(gate.request_logout("gui", true, TUNNEL_UP, true));

        assert!(
            gate.tunnel_transition(DISCONNECTED).is_empty(),
            "the dial is still to come"
        );
        assert!(gate.tunnel_transition(LEFT_DISCONNECTED).is_empty());
        assert_eq!(
            waiters(gate.tunnel_transition(DISCONNECTED)),
            vec![("gui", true)]
        );
    }

    #[test]
    fn a_connect_already_on_its_way_keeps_a_disconnected_tunnel_from_counting_as_down() {
        let mut gate = logged_in();
        gate.connect_sent();

        generation_of(gate.request_logout("cli", false, TUNNEL_DOWN, false));
    }

    #[test]
    fn a_disconnected_transition_with_no_logout_waiting_settles_the_connect() {
        let mut gate = logged_in();
        gate.connect_sent();
        gate.tunnel_transition(DISCONNECTED);

        assert!(matches!(
            gate.request_logout("cli", false, TUNNEL_DOWN, false),
            LogoutStart::CompleteNow(_)
        ));
    }

    #[test]
    fn a_teardown_that_misses_the_deadline_fails_the_logout_and_keeps_the_login() {
        let mut gate = logged_in();
        let generation = generation_of(gate.request_logout("gui", true, TUNNEL_UP, true));

        assert_eq!(gate.teardown_deadline(generation), vec!["gui"]);

        assert!(
            gate.tunnel_transition(DISCONNECTED).is_empty(),
            "a failed logout must not be carried out when the tunnel lands later"
        );
        assert_eq!(
            gate.admit_connect(),
            Ok(()),
            "the account is still logged in"
        );
    }

    #[test]
    fn a_retry_after_a_missed_deadline_finds_the_tunnel_down() {
        // A connect overtaken by the logout's disconnect inside the tunnel's
        // queue produces no transition, so nothing else would clear it.
        let mut gate = logged_in();
        gate.connect_sent();
        let generation = generation_of(gate.request_logout("cli", false, TUNNEL_UP, true));
        gate.tunnel_transition(DISCONNECTED);
        gate.teardown_deadline(generation);

        assert!(matches!(
            gate.request_logout("cli", false, TUNNEL_DOWN, false),
            LogoutStart::CompleteNow(_)
        ));
    }

    #[test]
    fn a_stale_deadline_does_not_fail_a_later_logout() {
        let mut gate = logged_in();
        let first = generation_of(gate.request_logout("first", false, TUNNEL_UP, true));
        gate.tunnel_transition(DISCONNECTED);
        let second = generation_of(gate.request_logout("second", false, TUNNEL_UP, true));

        assert!(gate.teardown_deadline(first).is_empty());
        assert_eq!(gate.teardown_deadline(second), vec!["second"]);
    }

    #[test]
    fn a_logout_repeated_while_one_waits_is_answered_with_it() {
        let mut gate = logged_in();
        generation_of(gate.request_logout("cli", false, TUNNEL_UP, true));

        assert!(matches!(
            gate.request_logout("gui", true, TUNNEL_UP, false),
            LogoutStart::Joined
        ));

        assert_eq!(
            waiters(gate.tunnel_transition(DISCONNECTED)),
            vec![("cli", false), ("gui", true)]
        );
    }

    #[test]
    fn a_logout_repeated_after_the_first_completed_completes_again() {
        let mut gate = logged_in();
        generation_of(gate.request_logout("cli", false, TUNNEL_UP, true));
        gate.tunnel_transition(DISCONNECTED);
        gate.logged_out_now();

        assert!(matches!(
            gate.request_logout("cli", false, TUNNEL_DOWN, false),
            LogoutStart::CompleteNow(_)
        ));
        assert_eq!(gate.admit_connect(), Err(ConnectRefusal::LoggedOut));
    }

    #[test]
    fn a_shutdown_hands_back_the_waiting_logout() {
        let mut gate = logged_in();
        generation_of(gate.request_logout("gui", true, TUNNEL_UP, true));

        assert_eq!(waiters(gate.abandon()), vec![("gui", true)]);
        assert!(!gate.logout_pending());
    }
}
