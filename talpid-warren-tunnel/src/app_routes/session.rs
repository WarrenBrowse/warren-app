//! Route sessions on the production datapath: the engine's supervisor and
//! pumps, the same as the main session's, admitted against the main
//! session's anchor where the server offers it (warren-core doc 107), and on
//! anonymous tokens otherwise.
//!
//! A route is dialed by anchor when the main session has an anchor that is
//! not known to be unusable, and the token directory lists its exit as
//! admitting routes. Any refusal of a route by anchor (the exit or the
//! control plane saying no, an exit predating route admission, an anchor that
//! never binds or goes away) is answered at once by the same route on a
//! token, exactly as a route ran before anchors existed; an exit that does
//! not offer route admission, or predates it, is not asked by anchor again
//! for the tunnel's life.
//!
//! What a route session deliberately does not share with the main one: the
//! wallet (it is never handed the signing key, and its supervisor refuses to
//! build a wallet-signed request), and every hook that feeds process-wide
//! state (the dial-refusal cooldown, the session placement, the reconnect
//! counter, the entry RTT store, the drain reactor's escalation cooldown).
//! Those describe the main session; a route session writing them would move
//! the main session's decisions.
//!
//! One piece of process-wide state is shared on purpose: the engine's memory
//! of whether this network lets QUIC through (`udp_hostility`), which every
//! supervisor feeds and reads. It describes the network both sessions cross,
//! so a route that keeps dying there can make the main session try its TCP
//! carrier first.

use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use ed25519_dalek::SigningKey;
use futures::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use talpid_app_routing::{owner::OwnerResolver, router::SessionAddresses};
use warrenguard_backoff::Backoff;
use warrenguard_multihop::RouteRejectCode;
use warrenguard_transport::{
    IpAssignChannel, IpAssignSpec,
    multihop::{MultiHopError, NoSessionTokenCause},
    route_anchor::{AnchorState, RouteAnchorHandle, RouteRefusal, RouteSessionAdmission},
    supervised_pump::{
        ExitDrainingChannel, run_downlink, run_downlink_with_daita, run_idle_cover, run_uplink,
        run_uplink_with_daita,
    },
    supervisor::{ClientWatch, MultiHopSupervisor, SessionAdmission, SupervisorConfig},
};
use warrenguard_transport_core::PacketDevice;

use super::{
    RouteAdmissionSource, RouteUnavailable, SessionEvent, TOKEN_ROUTE_SESSIONS,
    controller::RouteSessions, datapath::RouteTun,
};
use crate::{MultiHopConfig, SessionTokenSource, multi_hop_bind_addr, multi_hop_daita_shared};

/// How long a route session waits after it could not run before it tries
/// again. Tokens are minted on a coarse timer and a serial is released when
/// the session holding it ends, so an immediate retry would only spend a
/// handshake to hear the same answer.
const RETRY_UNAVAILABLE_AFTER: Duration = Duration::from_secs(60);

/// Runs `address` through the escape the carrier needs when its socket cannot
/// be bound to the physical interface (a macOS network whose bind was proven
/// to black-hole): the relay's `/32` route outside the tunnel.
pub type RelayEscape = Arc<dyn Fn(SocketAddr) -> BoxFuture<'static, ()> + Send + Sync>;

/// What every route session of a tunnel shares with the main session: its
/// constraints (IP version, DAITA, idle cover), its carrier escape, its
/// anchor and its token source.
#[derive(Clone)]
pub struct RouteSessionConfig {
    /// The daemon's token source, which opens each route supervisor on
    /// tokens a provider of its own. `None` leaves every route that is not
    /// admitted by anchor without a token, hence unavailable: a route
    /// session never falls back to the wallet.
    pub token_source: Option<SessionTokenSource>,
    /// The main session's anchor, when the main session anchors.
    pub anchor: Option<RouteAnchorHandle>,
    /// Which exits admit routes by anchor.
    pub route_admission: Option<Arc<dyn RouteAdmissionSource>>,
    /// Exits whose refusal of a route by anchor holds for the anchor's life.
    refused_by_anchor: Arc<Mutex<HashSet<[u8; 16]>>>,
    /// One permit per route that may run on tokens at once: the wallet's
    /// batch less the main session's serial. Past them a route waits rather
    /// than walk the serials the other sessions hold, which would show its
    /// exit the main session's serial.
    token_routes: Arc<tokio::sync::Semaphore>,
    pub wants_ipv6: bool,
    pub enable_daita: bool,
    pub idle_cover: bool,
    pub socket_bypass: Option<warrenguard_tun_core::SocketBypass>,
    /// Installed before a dial when `socket_bypass` is `None`.
    pub relay_escape: Option<RelayEscape>,
    /// Told the exit of a route session that announced a maintenance drain,
    /// so the daemon plans that route onto another exit.
    pub on_exit_draining: Option<Arc<dyn Fn([u8; 16]) + Send + Sync>>,
    pub retry_unavailable_after: Duration,
}

impl RouteSessionConfig {
    pub fn new(token_source: Option<SessionTokenSource>) -> Self {
        Self {
            token_source,
            anchor: None,
            route_admission: None,
            refused_by_anchor: Arc::default(),
            token_routes: Arc::new(tokio::sync::Semaphore::new(TOKEN_ROUTE_SESSIONS)),
            wants_ipv6: false,
            enable_daita: false,
            idle_cover: false,
            socket_bypass: None,
            relay_escape: None,
            on_exit_draining: None,
            retry_unavailable_after: RETRY_UNAVAILABLE_AFTER,
        }
    }

    /// How the next dial of the route to `exit` is admitted.
    pub(crate) fn admission_for(&self, exit: &[u8; 16]) -> SessionAdmission {
        let Some(anchor) = &self.anchor else {
            return SessionAdmission::TokensOnly;
        };
        let offered = self
            .route_admission
            .as_ref()
            .is_some_and(|admission| admission.offers_routes(exit));
        let refused = self
            .refused_by_anchor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(exit);
        if dials_by_anchor(anchor.current_state(), offered, refused) {
            SessionAdmission::Route(RouteSessionAdmission {
                anchor: anchor.clone(),
                exit_offers_routes: true,
            })
        } else {
            SessionAdmission::TokensOnly
        }
    }

    fn remember_refusal(&self, exit: [u8; 16]) {
        self.refused_by_anchor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(exit);
    }
}

/// Whether a route is dialed by anchor: the exit admits routes that way and
/// has not refused one for good, and the anchor is bound or still waiting
/// for its verdict (the engine waits for it before dialing). An anchor known
/// to be unusable sends the route straight to a token.
fn dials_by_anchor(anchor: AnchorState, exit_offers_routes: bool, refused_for_good: bool) -> bool {
    exit_offers_routes && !refused_for_good && anchor != AnchorState::Unavailable
}

/// Whether a refusal of a route by anchor holds for the rest of the anchor's
/// life: the exit does not offer route admission, or predates it. Any other
/// refusal (the route limit, an anchor lost or unknown, a control plane that
/// cannot be asked) may heal, so the next attempt asks by anchor again.
fn refusal_lasts(refusal: RouteRefusal) -> bool {
    matches!(
        refusal,
        RouteRefusal::Legacy | RouteRefusal::Rejected(RouteRejectCode::NotOffered)
    )
}

/// Starts each route session as a task of `runtime`.
pub struct SupervisorRouteSessions {
    runtime: tokio::runtime::Handle,
    config: RouteSessionConfig,
}

impl SupervisorRouteSessions {
    pub fn new(runtime: tokio::runtime::Handle, config: RouteSessionConfig) -> Self {
        Self { runtime, config }
    }
}

/// A route session's task. Everything the session holds (the supervisor,
/// the pumps, its view of the TUN device) lives in that one task, so the
/// session is gone once the task is.
pub struct SessionTask(Option<tokio::task::JoinHandle<()>>);

impl SessionTask {
    /// Ends the session and waits until it released what it held, the TUN
    /// device included.
    async fn stop(mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for SessionTask {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}

impl<D, R> RouteSessions<RouteTun<D, R>> for SupervisorRouteSessions
where
    D: PacketDevice + Clone,
    R: OwnerResolver + Send + 'static,
{
    type Handle = SessionTask;

    fn start(
        &mut self,
        circuit: &MultiHopConfig,
        device: RouteTun<D, R>,
        events: super::SessionEvents,
    ) -> Self::Handle {
        let circuit = circuit.clone();
        let config = self.config.clone();
        SessionTask(Some(self.runtime.spawn(async move {
            run_route_session(&circuit, &config, &device, &events).await;
        })))
    }

    fn stop(handle: Self::Handle) -> BoxFuture<'static, ()> {
        Box::pin(handle.stop())
    }
}

/// The supervisor configuration of a route session: the circuit's own
/// target, the main session's constraints, a key of its own, and none of the
/// hooks that feed process-wide state. A route admitted by anchor is handed
/// no token provider at all.
pub(crate) fn route_supervisor_config(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    admission: &SessionAdmission,
    ip_assign: IpAssignChannel,
) -> SupervisorConfig {
    SupervisorConfig {
        relay: Arc::new(circuit.relay.clone()),
        exit_id: circuit.exit.exit_id,
        exit_x25519_multihop_pubkey: circuit.exit.exit_x25519_multihop_pubkey,
        exit_mlkem768_pubkey: circuit.exit.exit_mlkem768_pubkey.clone(),
        operational_pubkey: circuit.operational_pubkey,
        // The supervisor wants a key, and under tokens-only or route admission
        // it never proves possession of it: a random one keeps the wallet out
        // of the route session altogether.
        client_signing: SigningKey::from_bytes(&rand::random()),
        bind_addr: multi_hop_bind_addr(circuit.relay.endpoint),
        enable_gso: circuit.enable_gso,
        use_warren_obfuscation: circuit.use_warren_obfuscation,
        socket_bypass: config.socket_bypass,
        enable_daita: config.enable_daita,
        idle_cover: config.idle_cover,
        backoff: Backoff {
            base: Duration::from_millis(300),
            max: Duration::from_secs(2),
        },
        on_reconnect: None,
        ip_assign_channel: Some(ip_assign),
        wants_ipv6: config.wants_ipv6,
        // One connection: a route carries the traffic of a few apps, and
        // every bonded leg is one more relay connection holding the token.
        n_connections: 1,
        pre_swap_check: None,
        on_overlap_swapped: None,
        on_dial_refused: None,
        on_path_rtt: None,
        session_token_provider: match admission {
            SessionAdmission::Route(_) => None,
            _ => config.token_source.as_ref().map(|open| open()),
        },
    }
}

/// A route supervisor admitted under `admission`: by anchor, or on tokens
/// only. Never the default admission, which would present the wallet.
pub(crate) fn route_supervisor(
    config: SupervisorConfig,
    admission: SessionAdmission,
) -> (MultiHopSupervisor, ClientWatch) {
    let admission = match admission {
        SessionAdmission::Route(route) => SessionAdmission::Route(route),
        _ => SessionAdmission::TokensOnly,
    };
    let (supervisor, client_rx) = MultiHopSupervisor::new(config);
    (supervisor.with_session_admission(admission), client_rx)
}

/// Why a route session that ended cannot run, for the user.
pub(crate) fn unavailable_reason(error: &MultiHopError) -> RouteUnavailable {
    match error {
        MultiHopError::NoSessionToken(NoSessionTokenCause::Empty) => RouteUnavailable::NoToken,
        MultiHopError::NoSessionToken(_) | MultiHopError::Rejected(_) => RouteUnavailable::Refused,
        _ => RouteUnavailable::Failed,
    }
}

/// How one supervisor's life ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionEnd {
    /// It cannot run, or stopped running, for this reason.
    Unavailable(RouteUnavailable),
    /// A route by anchor was refused: the same route may run on a token.
    Refused(RouteRefusal),
}

pub(crate) fn session_end(error: &MultiHopError) -> SessionEnd {
    match error {
        MultiHopError::RouteRefused(refusal) => SessionEnd::Refused(*refusal),
        other => SessionEnd::Unavailable(unavailable_reason(other)),
    }
}

/// The inner addresses a route session sends from.
pub(crate) fn session_addresses(spec: &IpAssignSpec, wants_ipv6: bool) -> SessionAddresses {
    SessionAddresses {
        v4: Some(spec.assigned),
        v6: spec.assigned_v6.filter(|_| wants_ipv6),
    }
}

/// Runs one route session until its task is aborted: dial, carry, and after
/// an end that a retry may fix, wait and dial again.
async fn run_route_session<T: PacketDevice + Clone>(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    device: &T,
    events: &super::SessionEvents,
) {
    let exit = *circuit.exit.exit_id.as_bytes();
    route_loop(
        exit,
        config,
        |event| events.send(event),
        |admission| run_supervised(circuit, config, device, events, admission),
    )
    .await;
}

/// The life of the route to `exit`: each attempt dials by anchor when it can
/// and on a token otherwise, a refusal by anchor is followed at once by a
/// dial on a token, a dial on a token waits for one of the token routes to be
/// free, and an attempt that ends otherwise waits
/// [`RouteSessionConfig::retry_unavailable_after`] before the next one.
async fn route_loop<F, Fut>(
    exit: [u8; 16],
    config: &RouteSessionConfig,
    report: impl Fn(SessionEvent),
    mut dial: F,
) where
    F: FnMut(SessionAdmission) -> Fut,
    Fut: std::future::Future<Output = SessionEnd>,
{
    loop {
        report(SessionEvent::Connecting);
        let reason = 'attempt: {
            if let admission @ SessionAdmission::Route(_) = config.admission_for(&exit) {
                match dial(admission).await {
                    SessionEnd::Refused(refusal) => {
                        if refusal_lasts(refusal) {
                            config.remember_refusal(exit);
                        }
                        log::info!(
                            "App routing: a route by anchor was not admitted ({refusal}); \
                             it runs on a token instead"
                        );
                    }
                    SessionEnd::Unavailable(reason) => break 'attempt reason,
                }
            }
            let Some(_permit) = token_route_permit(config, &report).await else {
                break 'attempt RouteUnavailable::Failed;
            };
            report(SessionEvent::Connecting);
            match dial(SessionAdmission::TokensOnly).await {
                SessionEnd::Unavailable(reason) => reason,
                // A route on tokens is never refused as a route by anchor.
                SessionEnd::Refused(_) => RouteUnavailable::Failed,
            }
        };
        log::info!("App routing: a route session ended ({reason:?}); retrying later");
        report(SessionEvent::Unavailable(reason));
        tokio::time::sleep(config.retry_unavailable_after).await;
    }
}

/// One of the routes the tokens admit at once, waited for (and reported
/// waiting) while every one is taken. `None` only if the permits were closed,
/// which nothing does.
async fn token_route_permit(
    config: &RouteSessionConfig,
    report: &impl Fn(SessionEvent),
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    if let Ok(permit) = Arc::clone(&config.token_routes).try_acquire_owned() {
        return Some(permit);
    }
    report(SessionEvent::Waiting);
    Arc::clone(&config.token_routes).acquire_owned().await.ok()
}

/// One supervisor's life: reports each session it publishes, and says why it
/// ended. The supervisor and the pumps run inside this future, not as tasks
/// of their own, so dropping it ends them all.
async fn run_supervised<T: PacketDevice + Clone>(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    device: &T,
    events: &super::SessionEvents,
    admission: SessionAdmission,
) -> SessionEnd {
    if config.socket_bypass.is_none()
        && let Some(escape) = &config.relay_escape
    {
        escape(circuit.relay.endpoint).await;
    }
    let ip_assign = IpAssignChannel::new();
    let (supervisor, mut client_rx) = route_supervisor(
        route_supervisor_config(circuit, config, &admission, ip_assign.clone()),
        admission,
    );
    let run = supervisor.run();
    tokio::pin!(run);
    let drain = ExitDrainingChannel::new();
    let mut pumps: FuturesUnordered<BoxFuture<'static, ()>> = FuturesUnordered::new();
    let mut started = false;
    let mut watching = true;
    loop {
        tokio::select! {
            ended = &mut run => {
                return match ended {
                    Err(error) => session_end(&error),
                    // `run` returns `Ok` only once every session receiver
                    // is gone, and this future holds one.
                    Ok(()) => SessionEnd::Unavailable(RouteUnavailable::Failed),
                };
            }
            // A pump that ended leaves the route carrying nothing in one
            // direction while the supervisor says it is up: start over.
            Some(()) = pumps.next(), if !pumps.is_empty() => {
                return SessionEnd::Unavailable(RouteUnavailable::Failed);
            }
            changed = client_rx.changed(), if watching => {
                if changed.is_err() {
                    // The supervisor is ending; its result says why.
                    watching = false;
                    continue;
                }
                let bundle = client_rx.borrow_and_update().clone();
                let Some(bundle) = bundle else {
                    events.send(SessionEvent::Connecting);
                    continue;
                };
                if !started {
                    started = true;
                    let daita = match multi_hop_daita_shared(
                        config.enable_daita,
                        bundle.primary().daita_spec(),
                    ) {
                        Ok(daita) => daita,
                        Err(_) => return SessionEnd::Unavailable(RouteUnavailable::Failed),
                    };
                    pumps.extend(pump_futures(&client_rx, device, daita, config.idle_cover, &drain));
                    if let Some(on_draining) = config.on_exit_draining.clone() {
                        pumps.push(watch_drain(&drain, on_draining));
                    }
                }
                drop(bundle);
                let spec = *ip_assign.subscribe().borrow();
                events.send(match spec {
                    Some(spec) => {
                        SessionEvent::Connected(session_addresses(&spec, config.wants_ipv6))
                    }
                    // Without the address its exit assigned, nothing it
                    // carries could be translated back.
                    None => SessionEvent::Connecting,
                });
            }
        }
    }
}

fn pump_futures<T: PacketDevice + Clone>(
    client_rx: &ClientWatch,
    device: &T,
    daita: Option<warrenguard_transport::supervised_pump::DaitaShared>,
    idle_cover: bool,
    drain: &ExitDrainingChannel,
) -> Vec<BoxFuture<'static, ()>> {
    let mut pumps: Vec<BoxFuture<'static, ()>> = Vec::with_capacity(3);
    let (uplink_rx, uplink_device) = (client_rx.clone(), device.clone());
    let (downlink_rx, downlink_device) = (client_rx.clone(), device.clone());
    let drain = drain.clone();
    match daita {
        Some(daita) => {
            let notify = Arc::new(tokio::sync::Notify::new());
            let (uplink_daita, uplink_notify) = (Arc::clone(&daita), Arc::clone(&notify));
            pumps.push(Box::pin(async move {
                let _ =
                    run_uplink_with_daita(uplink_rx, uplink_device, uplink_daita, uplink_notify)
                        .await;
            }));
            pumps.push(Box::pin(async move {
                let _ = run_downlink_with_daita(
                    downlink_rx,
                    downlink_device,
                    daita,
                    notify,
                    None,
                    Some(drain),
                )
                .await;
            }));
        }
        None => {
            pumps.push(Box::pin(async move {
                let _ = run_uplink(uplink_rx, uplink_device).await;
            }));
            pumps.push(Box::pin(async move {
                let _ = run_downlink(downlink_rx, downlink_device, Some(drain)).await;
            }));
        }
    }
    if idle_cover {
        let cover_rx = client_rx.clone();
        pumps.push(Box::pin(async move {
            let _ = run_idle_cover(cover_rx).await;
        }));
    }
    pumps
}

/// Tells the daemon about a drain the route's exit announced; the daemon
/// plans the route onto another exit, which replaces this session.
fn watch_drain(
    drain: &ExitDrainingChannel,
    on_draining: Arc<dyn Fn([u8; 16]) + Send + Sync>,
) -> BoxFuture<'static, ()> {
    let mut notices = drain.subscribe();
    let _ = notices.borrow_and_update();
    Box::pin(async move {
        while notices.changed().await.is_ok() {
            let exit = notices
                .borrow_and_update()
                .as_ref()
                .map(|notice| *notice.exit_id.as_bytes());
            if let Some(exit) = exit {
                on_draining(exit);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use warrenguard_multihop::{RejectionReason, RouteEndReason, RouteKemSecretKey};
    use warrenguard_transport::{
        route_anchor::RouteAnchorConfig, supervisor::SessionTokenProvider,
    };

    use super::*;

    /// The exit of [`circuit`].
    const EXIT: [u8; 16] = [9; 16];

    fn circuit() -> MultiHopConfig {
        crate::app_routes::test_support::circuit(7, 9)
    }

    fn tokens_only() -> SessionAdmission {
        SessionAdmission::TokensOnly
    }

    /// A token directory offering route admission at the listed exits.
    struct Offering(Vec<[u8; 16]>);

    impl RouteAdmissionSource for Offering {
        fn kem(&self) -> Option<warrenguard_multihop::RouteKemPublicKey> {
            Some(kem())
        }

        fn offers_routes(&self, exit_id: &[u8; 16]) -> bool {
            self.0.contains(exit_id)
        }
    }

    fn kem() -> warrenguard_multihop::RouteKemPublicKey {
        RouteKemSecretKey::derive(&[0x71; 32], 1)
            .unwrap()
            .public_key()
            .clone()
    }

    /// A tunnel whose main session anchors (no verdict yet), at a directory
    /// offering route admission at `offered`.
    fn anchored(offered: &[[u8; 16]]) -> RouteSessionConfig {
        let mut config = RouteSessionConfig::new(None);
        config.anchor = Some(RouteAnchorHandle::new(RouteAnchorConfig { kem: kem() }));
        config.route_admission = Some(Arc::new(Offering(offered.to_vec())));
        config.retry_unavailable_after = Duration::from_secs(60);
        config
    }

    fn by_anchor(admission: &SessionAdmission) -> bool {
        matches!(admission, SessionAdmission::Route(_))
    }

    /// A token source that counts the providers it opens and the stacks they
    /// hand out.
    fn counting_source() -> (SessionTokenSource, Arc<AtomicU32>, Arc<AtomicU32>) {
        let opened = Arc::new(AtomicU32::new(0));
        let drawn = Arc::new(AtomicU32::new(0));
        let source: SessionTokenSource = {
            let opened = Arc::clone(&opened);
            let drawn = Arc::clone(&drawn);
            Arc::new(move || {
                opened.fetch_add(1, Ordering::Relaxed);
                let drawn = Arc::clone(&drawn);
                Arc::new(move || {
                    drawn.fetch_add(1, Ordering::Relaxed);
                    Vec::new()
                }) as SessionTokenProvider
            })
        };
        (source, opened, drawn)
    }

    #[test]
    fn a_route_supervisor_dials_the_planned_circuit_with_the_tunnels_constraints() {
        let mut config = RouteSessionConfig::new(None);
        config.wants_ipv6 = true;
        config.enable_daita = true;

        let built =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());

        assert_eq!(built.relay.relay_id, [7; 16]);
        assert_eq!(built.exit_id.as_bytes(), &[9; 16]);
        assert!(built.wants_ipv6);
        assert!(built.enable_daita);
        assert_eq!(built.n_connections, 1);
    }

    #[test]
    fn a_route_supervisor_presents_the_tokens_of_the_daemons_source() {
        let (source, _opened, drawn) = counting_source();
        let config = RouteSessionConfig::new(Some(source));

        let built =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());
        let _ = (built.session_token_provider.expect("a token provider"))();

        assert_eq!(drawn.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn each_route_supervisor_opens_a_token_provider_of_its_own() {
        let (source, opened, _drawn) = counting_source();
        let config = RouteSessionConfig::new(Some(source));

        let _first =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());
        let _second =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());

        assert_eq!(opened.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn each_route_supervisor_holds_a_key_of_its_own() {
        let config = RouteSessionConfig::new(None);

        let first =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());
        let second =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());

        assert_ne!(
            first.client_signing.to_bytes(),
            second.client_signing.to_bytes()
        );
    }

    #[test]
    fn a_route_supervisor_feeds_none_of_the_process_wide_hooks() {
        let config = RouteSessionConfig::new(None);

        let built =
            route_supervisor_config(&circuit(), &config, &tokens_only(), IpAssignChannel::new());

        assert!(built.on_dial_refused.is_none(), "dial-refusal cooldown");
        assert!(built.on_reconnect.is_none(), "reconnect counter");
        assert!(built.on_path_rtt.is_none(), "entry RTT store");
        assert!(built.pre_swap_check.is_none(), "port-forward reservation");
        assert!(built.on_overlap_swapped.is_none(), "migration observer");
    }

    #[test]
    fn a_route_without_a_token_is_shown_without_a_token() {
        let reason = unavailable_reason(&MultiHopError::NoSessionToken(NoSessionTokenCause::Empty));

        assert_eq!(reason, RouteUnavailable::NoToken);
    }

    #[test]
    fn a_route_whose_every_token_is_refused_is_shown_refused() {
        let reason = unavailable_reason(&MultiHopError::NoSessionToken(
            NoSessionTokenCause::AllRefused,
        ));

        assert_eq!(reason, RouteUnavailable::Refused);
    }

    #[test]
    fn a_route_the_exit_rejects_is_shown_refused() {
        let reason = unavailable_reason(&MultiHopError::Rejected(RejectionReason::Banned(0)));

        assert_eq!(reason, RouteUnavailable::Refused);
    }

    #[test]
    fn a_route_session_sends_from_the_addresses_its_exit_assigned() {
        let spec = IpAssignSpec {
            assigned: "10.66.3.4".parse().unwrap(),
            prefix_len: 16,
            gateway: "10.66.0.1".parse().unwrap(),
            assigned_v6: Some("fdcc:f:1::9".parse().unwrap()),
            prefix_len_v6: 64,
            gateway_v6: Some("fdcc:f:1::1".parse().unwrap()),
        };

        assert_eq!(
            session_addresses(&spec, true),
            SessionAddresses {
                v4: Some("10.66.3.4".parse().unwrap()),
                v6: Some("fdcc:f:1::9".parse().unwrap()),
            }
        );
        assert_eq!(session_addresses(&spec, false).v6, None);
    }

    #[test]
    fn a_route_to_an_exit_offering_route_admission_is_dialed_by_anchor() {
        let config = anchored(&[EXIT]);

        assert!(by_anchor(&config.admission_for(&EXIT)));
    }

    #[test]
    fn a_route_to_an_exit_the_directory_does_not_list_runs_on_tokens() {
        let config = anchored(&[[3; 16]]);

        assert!(!by_anchor(&config.admission_for(&EXIT)));
    }

    #[test]
    fn a_tunnel_whose_main_session_does_not_anchor_runs_every_route_on_tokens() {
        let mut config = anchored(&[EXIT]);
        config.anchor = None;

        assert!(!by_anchor(&config.admission_for(&EXIT)));
    }

    #[test]
    fn an_anchor_known_unusable_sends_routes_straight_to_tokens() {
        assert!(!dials_by_anchor(AnchorState::Unavailable, true, false));
        assert!(dials_by_anchor(AnchorState::Unanchored, true, false));
        assert!(dials_by_anchor(
            AnchorState::Anchored { max_routes: 32 },
            true,
            false
        ));
    }

    #[test]
    fn a_route_dialed_by_anchor_opens_no_token_provider() {
        let (source, opened, _drawn) = counting_source();
        let mut config = anchored(&[EXIT]);
        config.token_source = Some(source);

        let admission = config.admission_for(&EXIT);
        let built =
            route_supervisor_config(&circuit(), &config, &admission, IpAssignChannel::new());

        assert!(built.session_token_provider.is_none());
        assert_eq!(opened.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_route_refused_by_anchor_ends_its_run_as_a_refusal() {
        let refusal = RouteRefusal::Rejected(RouteRejectCode::RouteLimit);

        assert_eq!(
            session_end(&MultiHopError::RouteRefused(refusal)),
            SessionEnd::Refused(refusal)
        );
    }

    /// What each dial of a [`route_loop`] was admitted on, and when.
    #[derive(Clone, Default)]
    struct Dials(Arc<Mutex<Vec<(bool, tokio::time::Instant)>>>);

    impl Dials {
        fn record(&self, admission: &SessionAdmission) {
            self.0
                .lock()
                .unwrap()
                .push((by_anchor(admission), tokio::time::Instant::now()));
        }

        fn by_anchor(&self) -> Vec<bool> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .map(|(anchor, _)| *anchor)
                .collect()
        }

        fn gaps(&self) -> Vec<Duration> {
            let dials = self.0.lock().unwrap();
            dials.windows(2).map(|w| w[1].1 - w[0].1).collect()
        }
    }

    /// Runs a route loop whose dials by anchor end with `by_anchor` and whose
    /// dials on tokens end with `on_tokens`, until it made `dials` of them.
    async fn run_loop(
        config: &RouteSessionConfig,
        by_anchor: SessionEnd,
        on_tokens: SessionEnd,
        dials: usize,
    ) -> Dials {
        let record = Dials::default();
        let seen = record.clone();
        let looping = route_loop(
            EXIT,
            config,
            |_event| {},
            |admission| {
                seen.record(&admission);
                let end = if matches!(admission, SessionAdmission::Route(_)) {
                    by_anchor
                } else {
                    on_tokens
                };
                async move { end }
            },
        );
        let enough = async {
            while record.0.lock().unwrap().len() < dials {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        };
        tokio::select! {
            () = looping => unreachable!("a route loop never ends"),
            () = enough => {}
        }
        record
    }

    const NO_TOKEN: SessionEnd = SessionEnd::Unavailable(RouteUnavailable::NoToken);

    #[tokio::test(start_paused = true)]
    async fn every_refusal_by_anchor_is_followed_at_once_by_a_token_route() {
        for refusal in [
            RouteRefusal::Rejected(RouteRejectCode::RouteLimit),
            RouteRefusal::Rejected(RouteRejectCode::Unavailable),
            RouteRefusal::Rejected(RouteRejectCode::AnchorUnknown),
            RouteRefusal::Rejected(RouteRejectCode::NotOffered),
            RouteRefusal::Rejected(RouteRejectCode::Unspecified),
            RouteRefusal::Legacy,
            RouteRefusal::AnchorUnavailable,
            RouteRefusal::Ended(RouteEndReason::AnchorGone),
        ] {
            let config = anchored(&[EXIT]);

            let dials = run_loop(&config, SessionEnd::Refused(refusal), NO_TOKEN, 2).await;

            assert_eq!(dials.by_anchor()[..2], [true, false], "{refusal:?}");
            assert_eq!(dials.gaps()[0], Duration::ZERO, "{refusal:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_exit_that_does_not_offer_routes_is_not_asked_by_anchor_again() {
        for refusal in [
            RouteRefusal::Rejected(RouteRejectCode::NotOffered),
            RouteRefusal::Legacy,
        ] {
            let config = anchored(&[EXIT]);

            let dials = run_loop(&config, SessionEnd::Refused(refusal), NO_TOKEN, 3).await;

            assert_eq!(dials.by_anchor()[..3], [true, false, false], "{refusal:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_refusal_that_may_heal_is_asked_by_anchor_again_at_the_next_attempt() {
        let config = anchored(&[EXIT]);
        let limit = SessionEnd::Refused(RouteRefusal::Rejected(RouteRejectCode::RouteLimit));

        let dials = run_loop(&config, limit, NO_TOKEN, 3).await;

        assert_eq!(dials.by_anchor()[..3], [true, false, true]);
        assert_eq!(dials.gaps()[1], config.retry_unavailable_after);
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_by_anchor_that_fails_otherwise_waits_and_dials_by_anchor_again() {
        let config = anchored(&[EXIT]);
        let failed = SessionEnd::Unavailable(RouteUnavailable::Failed);

        let dials = run_loop(&config, failed, NO_TOKEN, 2).await;

        assert_eq!(dials.by_anchor()[..2], [true, true]);
        assert_eq!(dials.gaps()[0], config.retry_unavailable_after);
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_without_route_admission_runs_on_tokens_as_before() {
        let config = anchored(&[]);

        let dials = run_loop(&config, NO_TOKEN, NO_TOKEN, 2).await;

        assert_eq!(dials.by_anchor()[..2], [false, false]);
        assert_eq!(dials.gaps()[0], config.retry_unavailable_after);
    }

    #[tokio::test(start_paused = true)]
    async fn at_most_the_token_routes_run_on_tokens_at_once_and_the_others_wait() {
        let config = anchored(&[]);
        let dialed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let waiting = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let spawn_route = |exit: u8| {
            let (config, dialed, waiting) =
                (config.clone(), Arc::clone(&dialed), Arc::clone(&waiting));
            tokio::spawn(async move {
                route_loop(
                    [exit; 16],
                    &config,
                    |event| {
                        if event == SessionEvent::Waiting {
                            waiting.fetch_add(1, Ordering::SeqCst);
                        }
                    },
                    |_admission| {
                        dialed.fetch_add(1, Ordering::SeqCst);
                        // Connected on a token, for as long as it lives.
                        std::future::pending::<SessionEnd>()
                    },
                )
                .await;
            })
        };
        let routes: Vec<_> = (1..=TOKEN_ROUTE_SESSIONS as u8 + 1)
            .map(spawn_route)
            .collect();
        tokio::time::sleep(Duration::from_secs(1)).await;
        let (dialed_first, waiting_first) = (
            dialed.load(Ordering::SeqCst),
            waiting.load(Ordering::SeqCst),
        );

        routes[0].abort();
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert_eq!((dialed_first, waiting_first), (TOKEN_ROUTE_SESSIONS, 1));
        assert_eq!(
            dialed.load(Ordering::SeqCst),
            TOKEN_ROUTE_SESSIONS + 1,
            "the waiting route dials once a token route is free"
        );
        for route in routes {
            route.abort();
        }
    }
}
