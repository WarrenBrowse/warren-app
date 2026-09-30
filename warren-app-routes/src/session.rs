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
//! for the tunnel's life. A route on a token moves back to the anchor once
//! the anchor could admit it, make before break, so it stops holding one of
//! the wallet's serials.
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
use tokio::sync::watch;
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
    Credentials, RouteAdmissionSource, RouteUnavailable, SessionEvent, TOKEN_ROUTE_SESSIONS,
    controller::RouteSessions, datapath::RouteTun,
};
use crate::{MultiHopConfig, SessionTokenSource, daita_shared, multi_hop_bind_addr};

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
    /// The daemon's credentials refreshes: a route that could not run for
    /// want of a token dials again as soon as one brings tokens.
    pub credentials: Option<watch::Receiver<Credentials>>,
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
            credentials: None,
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
        MultiHopError::NoReachableEntry => RouteUnavailable::NoReachableEntry,
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
        config.anchor.as_ref().map(RouteAnchorHandle::state),
        |event| events.send(event),
        RouteDials {
            dial: |admission| async move {
                carry(
                    dial(circuit, config, admission).await,
                    config,
                    device,
                    events,
                )
                .await
            },
            probe: |admission| async move {
                until_connected(dial(circuit, config, admission).await, PROBE_TIMEOUT).await
            },
            carry: |live| carry(live, config, device, events),
        },
    )
    .await;
}

/// How a route loop reaches its exit: `dial` dials under an admission and
/// carries the route until the session ends; `probe` dials by anchor next to
/// a route on tokens and resolves once that session is up, carrying nothing;
/// `carry` carries the route on a session `probe` brought up.
pub(crate) struct RouteDials<D, P, C> {
    pub dial: D,
    pub probe: P,
    pub carry: C,
}

/// The life of the route to `exit`: each attempt dials by anchor when it can
/// and on a token otherwise, a refusal by anchor is followed at once by a
/// dial on a token, a dial on a token waits for one of the token routes to be
/// free, and an attempt that ends otherwise waits
/// [`RouteSessionConfig::retry_unavailable_after`] before the next one.
///
/// A route on a token moves to the anchor once the anchor is bound
/// ([`upgrade_by_anchor`]), make before break: the session by anchor comes up
/// next to it, the route's packets move to it, and only then does the session
/// on the token end, which releases its serial and its place among the token
/// routes.
async fn route_loop<U, D, DF, P, PF, C, CF>(
    exit: [u8; 16],
    config: &RouteSessionConfig,
    anchor_states: Option<watch::Receiver<AnchorState>>,
    report: impl Fn(SessionEvent),
    mut dials: RouteDials<D, P, C>,
) where
    D: FnMut(SessionAdmission) -> DF,
    DF: std::future::Future<Output = SessionEnd>,
    P: FnMut(SessionAdmission) -> PF,
    PF: std::future::Future<Output = Result<U, SessionEnd>>,
    C: FnMut(U) -> CF,
    CF: std::future::Future<Output = SessionEnd>,
{
    let mut moved: Option<U> = None;
    'route: loop {
        let reason = 'attempt: {
            let by_anchor = if let Some(live) = moved.take() {
                Some((dials.carry)(live).await)
            } else if let admission @ SessionAdmission::Route(_) = config.admission_for(&exit) {
                report(SessionEvent::Connecting);
                Some((dials.dial)(admission).await)
            } else {
                None
            };
            // A route by anchor that was just refused is not asked again
            // at once: a move right after it would flap the route.
            let move_after = matches!(by_anchor, Some(SessionEnd::Refused(_)))
                .then_some(config.retry_unavailable_after);
            match by_anchor {
                Some(SessionEnd::Refused(refusal)) => {
                    if refusal_lasts(refusal) {
                        config.remember_refusal(exit);
                    }
                    log::info!(
                        "App routing: a route by anchor was not admitted ({refusal}); \
                         it runs on a token instead"
                    );
                }
                Some(SessionEnd::Unavailable(reason)) => break 'attempt reason,
                None => {}
            }
            report(SessionEvent::Connecting);
            // While every token route is taken, a route that the anchor could
            // admit in the meantime goes by anchor rather than keep waiting.
            let permit = tokio::select! {
                permit = token_route_permit(config, &report) => permit,
                _ = anchor_ready(exit, config, anchor_states.clone(), move_after) => continue 'route,
            };
            let Some(_permit) = permit else {
                break 'attempt RouteUnavailable::Failed;
            };
            report(SessionEvent::Connecting);
            let on_tokens = (dials.dial)(SessionAdmission::TokensOnly);
            let upgrade = upgrade_by_anchor(
                exit,
                config,
                anchor_states.clone(),
                move_after,
                &mut dials.probe,
            );
            tokio::pin!(on_tokens, upgrade);
            tokio::select! {
                end = &mut on_tokens => match end {
                    SessionEnd::Unavailable(reason) => break 'attempt reason,
                    // A route on tokens is never refused as a route by anchor.
                    SessionEnd::Refused(_) => break 'attempt RouteUnavailable::Failed,
                },
                live = &mut upgrade => {
                    log::info!(
                        "App routing: a route on a token moves to the main session's anchor"
                    );
                    moved = Some(live);
                    // Leaving this scope ends the session on the token and
                    // gives back its permit.
                    continue 'route;
                }
            }
        };
        log::info!("App routing: a route session ended ({reason:?}); retrying later");
        report(SessionEvent::Unavailable(reason));
        retry_wait(config, anchor_states.clone()).await;
    }
}

/// The wait before a route that could not run dials again:
/// [`RouteSessionConfig::retry_unavailable_after`], cut short by a refresh
/// that brings tokens or by the main session's anchor being bound, since
/// either may be what the route lacked.
async fn retry_wait(
    config: &RouteSessionConfig,
    anchor_states: Option<watch::Receiver<AnchorState>>,
) {
    let tokens = async {
        let Some(mut credentials) = config.credentials.clone() else {
            return std::future::pending().await;
        };
        credentials.mark_unchanged();
        loop {
            if credentials.changed().await.is_err() {
                return std::future::pending().await;
            }
            if credentials.borrow_and_update().has_tokens {
                return;
            }
        }
    };
    let bound = async {
        let Some(mut states) = anchor_states else {
            return std::future::pending().await;
        };
        states.mark_unchanged();
        loop {
            if states.changed().await.is_err() {
                return std::future::pending().await;
            }
            if matches!(*states.borrow_and_update(), AnchorState::Anchored { .. }) {
                return;
            }
        }
    };
    tokio::select! {
        () = tokio::time::sleep(config.retry_unavailable_after) => {}
        () = tokens => log::info!("App routing: tokens arrived; a waiting route dials again"),
        () = bound => log::info!("App routing: the anchor is bound; a waiting route dials again"),
    }
}

/// How long a route by anchor dialed next to a route on a token may take to
/// come up before the attempt is given up until the next one.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Resolves once the route to `exit`, which runs on a token, is up by anchor:
/// it waits `not_before` when given, then for the main session's anchor to be
/// bound and for the route to be dialed by anchor
/// ([`RouteSessionConfig::admission_for`]), then brings a session up with
/// `probe`. A probe that fails leaves the route on its token and is tried
/// again [`RouteSessionConfig::retry_unavailable_after`] later; one refused
/// for good ([`refusal_lasts`]) is not tried again. Never resolves for a
/// tunnel whose main session does not anchor.
async fn upgrade_by_anchor<U, P, PF>(
    exit: [u8; 16],
    config: &RouteSessionConfig,
    anchor_states: Option<watch::Receiver<AnchorState>>,
    not_before: Option<Duration>,
    probe: &mut P,
) -> U
where
    P: FnMut(SessionAdmission) -> PF,
    PF: std::future::Future<Output = Result<U, SessionEnd>>,
{
    let mut not_before = not_before;
    loop {
        let admission = anchor_ready(exit, config, anchor_states.clone(), not_before.take()).await;
        match probe(admission).await {
            Ok(live) => return live,
            Err(SessionEnd::Refused(refusal)) if refusal_lasts(refusal) => {
                config.remember_refusal(exit);
            }
            Err(_) => {}
        }
        tokio::time::sleep(config.retry_unavailable_after).await;
    }
}

/// Resolves with the admission by anchor of the route to `exit` once the main
/// session's anchor is bound and the route would be dialed by anchor
/// ([`RouteSessionConfig::admission_for`]), not before `not_before` when
/// given. Never resolves for a tunnel whose main session does not anchor.
async fn anchor_ready(
    exit: [u8; 16],
    config: &RouteSessionConfig,
    anchor_states: Option<watch::Receiver<AnchorState>>,
    not_before: Option<Duration>,
) -> SessionAdmission {
    let Some(mut states) = anchor_states else {
        return std::future::pending().await;
    };
    if let Some(wait) = not_before {
        tokio::time::sleep(wait).await;
    }
    loop {
        let bound = matches!(*states.borrow_and_update(), AnchorState::Anchored { .. });
        if bound && let admission @ SessionAdmission::Route(_) = config.admission_for(&exit) {
            return admission;
        }
        // The directory's exit list has no signal of its own: look again now
        // and then.
        tokio::select! {
            changed = states.changed() => {
                if changed.is_err() {
                    return std::future::pending().await;
                }
            }
            () = tokio::time::sleep(config.retry_unavailable_after) => {}
        }
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

/// A route supervisor that has been dialed: the future that runs it, which
/// is the session (dropping it ends the session), and what it publishes.
/// `B` is what it publishes a session as, the engine's bundle outside tests.
pub(crate) struct Dialed<B = Arc<warrenguard_transport::bundle::MultiHopBundle>> {
    run: BoxFuture<'static, Result<(), MultiHopError>>,
    client_rx: watch::Receiver<Option<B>>,
    ip_assign: IpAssignChannel,
}

/// Dials the route's circuit under `admission`.
async fn dial(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    admission: SessionAdmission,
) -> Dialed {
    if config.socket_bypass.is_none()
        && let Some(escape) = &config.relay_escape
    {
        escape(circuit.relay.endpoint).await;
    }
    let ip_assign = IpAssignChannel::new();
    let (supervisor, client_rx) = route_supervisor(
        route_supervisor_config(circuit, config, &admission, ip_assign.clone()),
        admission,
    );
    Dialed {
        run: Box::pin(supervisor.run()),
        client_rx,
        ip_assign,
    }
}

/// Runs `dialed` until it published a session whose exit assigned its
/// addresses, carrying nothing, and hands it back up. Gives up after
/// `timeout`, and says why the supervisor ended if it did.
async fn until_connected<B>(
    mut dialed: Dialed<B>,
    timeout: Duration,
) -> Result<Dialed<B>, SessionEnd> {
    let mut assigned = dialed.ip_assign.subscribe();
    let connected = async {
        let (mut watching, mut assigning) = (true, true);
        loop {
            let up = dialed.client_rx.borrow_and_update().is_some();
            if up && assigned.borrow_and_update().is_some() {
                return Ok(());
            }
            tokio::select! {
                ended = &mut dialed.run => {
                    return Err(match ended {
                        Err(error) => session_end(&error),
                        Ok(()) => SessionEnd::Unavailable(RouteUnavailable::Failed),
                    });
                }
                // Closed: the supervisor is ending, and its result says why.
                changed = dialed.client_rx.changed(), if watching => watching = changed.is_ok(),
                changed = assigned.changed(), if assigning => assigning = changed.is_ok(),
            }
        }
    };
    match tokio::time::timeout(timeout, connected).await {
        Ok(Ok(())) => Ok(dialed),
        Ok(Err(end)) => Err(end),
        Err(_elapsed) => Err(SessionEnd::Unavailable(RouteUnavailable::Failed)),
    }
}

/// Carries the route on `dialed`: reports each session it publishes, and says
/// why it ended. The supervisor and the pumps run inside this future, not as
/// tasks of their own, so dropping it ends them all. A session already
/// published when it starts (one [`until_connected`] brought up) is carried
/// at once.
async fn carry<T: PacketDevice + Clone>(
    dialed: Dialed,
    config: &RouteSessionConfig,
    device: &T,
    events: &super::SessionEvents,
) -> SessionEnd {
    let Dialed {
        mut run,
        mut client_rx,
        ip_assign,
    } = dialed;
    let drain = ExitDrainingChannel::new();
    let mut pumps: FuturesUnordered<BoxFuture<'static, ()>> = FuturesUnordered::new();
    let mut started = false;
    let mut watching = true;
    let mut published = client_rx.borrow().is_some();
    loop {
        if published {
            published = false;
            let bundle = client_rx.borrow_and_update().clone();
            let Some(bundle) = bundle else {
                events.send(SessionEvent::Connecting);
                continue;
            };
            if !started {
                started = true;
                let daita = match daita_shared(bundle.primary().daita_spec()) {
                    Ok(daita) => daita,
                    Err(_) => return SessionEnd::Unavailable(RouteUnavailable::Failed),
                };
                if daita.is_none() && config.enable_daita {
                    log::warn!(
                        "App routing: DAITA was requested but this route's exit did not grant \
                         it; its pumps run undefended"
                    );
                }
                pumps.extend(pump_futures(
                    &client_rx,
                    device,
                    daita,
                    config.idle_cover,
                    &drain,
                ));
                if let Some(on_draining) = config.on_exit_draining.clone() {
                    pumps.push(watch_drain(&drain, on_draining));
                }
            }
            drop(bundle);
            let spec = *ip_assign.subscribe().borrow();
            events.send(match spec {
                Some(spec) => SessionEvent::Connected(session_addresses(&spec, config.wants_ipv6)),
                // Without the address its exit assigned, nothing it
                // carries could be translated back.
                None => SessionEvent::Connecting,
            });
        }
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
                published = true;
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
        crate::test_support::circuit(7, 9)
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
    fn a_route_whose_entries_the_network_cannot_route_names_the_network() {
        // Forum topic 210: a failure the user can only end by changing
        // network, which the generic "failed" hid.
        let reason = unavailable_reason(&MultiHopError::NoReachableEntry);

        assert_eq!(reason, RouteUnavailable::NoReachableEntry);
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
            None,
            |_event| {},
            RouteDials {
                dial: |admission| {
                    seen.record(&admission);
                    let end = if matches!(admission, SessionAdmission::Route(_)) {
                        by_anchor
                    } else {
                        on_tokens
                    };
                    async move { end }
                },
                probe: |_admission| std::future::pending::<Result<(), SessionEnd>>(),
                carry: |()| std::future::pending::<SessionEnd>(),
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
                    None,
                    |event| {
                        if event == SessionEvent::Waiting {
                            waiting.fetch_add(1, Ordering::SeqCst);
                        }
                    },
                    RouteDials {
                        dial: |_admission| {
                            dialed.fetch_add(1, Ordering::SeqCst);
                            // Connected on a token, for as long as it lives.
                            std::future::pending::<SessionEnd>()
                        },
                        probe: |_admission| std::future::pending::<Result<(), SessionEnd>>(),
                        carry: |()| std::future::pending::<SessionEnd>(),
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

    /// A move of a route from its token to the anchor, as a test watches it:
    /// when the session on the token ended, when each probe by anchor was
    /// made, and whether the route was then carried on what the probe
    /// brought up.
    #[derive(Clone, Default)]
    struct Move {
        token_session_ended: Arc<Mutex<Option<tokio::time::Instant>>>,
        token_dials: Arc<AtomicU32>,
        anchor_dials: Arc<AtomicU32>,
        probes: Arc<Mutex<Vec<tokio::time::Instant>>>,
        carried: Arc<Mutex<Option<tokio::time::Instant>>>,
        /// Each step in the order it happened: a paused clock gives steps of
        /// one instant the same time.
        steps: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Move {
        fn step(&self, step: &'static str) {
            self.steps.lock().unwrap().push(step);
        }
    }

    /// Records when, and in which order, the future holding it is dropped.
    struct EndsAt(Move);

    impl Drop for EndsAt {
        fn drop(&mut self) {
            *self.0.token_session_ended.lock().unwrap() = Some(tokio::time::Instant::now());
            self.0.step("token session ended");
        }
    }

    /// Spawns a route loop at a directory offering route admission at
    /// [`EXIT`] whose first dial by anchor is refused (the anchor not bound
    /// yet), so the route runs on a token that stays up. Each probe by anchor
    /// answers the next of `probes` (then succeeds); a session a probe brought
    /// up is carried until it ends with `after_move`.
    fn spawn_moving_route(
        config: &RouteSessionConfig,
        states: watch::Receiver<AnchorState>,
        probes: Vec<Result<(), SessionEnd>>,
        after_move: Option<SessionEnd>,
    ) -> (Move, tokio::task::JoinHandle<()>) {
        let watched = Move::default();
        let seen = watched.clone();
        let config = config.clone();
        let answers = Arc::new(Mutex::new(std::collections::VecDeque::from(probes)));
        let task = tokio::spawn(async move {
            let first_by_anchor = Arc::new(std::sync::atomic::AtomicBool::new(true));
            route_loop(
                EXIT,
                &config,
                Some(states),
                |_event| {},
                RouteDials {
                    dial: |admission| {
                        let first = first_by_anchor.swap(false, Ordering::SeqCst);
                        let by_anchor = by_anchor(&admission);
                        if by_anchor {
                            seen.anchor_dials.fetch_add(1, Ordering::SeqCst);
                        } else {
                            seen.token_dials.fetch_add(1, Ordering::SeqCst);
                        }
                        let ends = (!by_anchor).then(|| EndsAt(seen.clone()));
                        async move {
                            if by_anchor && first {
                                return SessionEnd::Refused(RouteRefusal::AnchorUnavailable);
                            }
                            let _ends = ends;
                            std::future::pending::<SessionEnd>().await
                        }
                    },
                    probe: |_admission| {
                        seen.probes
                            .lock()
                            .unwrap()
                            .push(tokio::time::Instant::now());
                        let answer = answers.lock().unwrap().pop_front().unwrap_or(Ok(()));
                        let seen = seen.clone();
                        async move {
                            if answer.is_ok() {
                                seen.step("by anchor up");
                            }
                            answer
                        }
                    },
                    carry: |()| {
                        *seen.carried.lock().unwrap() = Some(tokio::time::Instant::now());
                        seen.step("carried by anchor");
                        async move {
                            match after_move {
                                Some(end) => end,
                                None => std::future::pending().await,
                            }
                        }
                    },
                },
            )
            .await;
        });
        (watched, task)
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_on_a_token_moves_to_the_anchor_once_bound_and_only_then_ends_its_token_session()
     {
        let config = anchored(&[EXIT]);
        let (anchor, states) = watch::channel(AnchorState::Unanchored);
        let (moving, task) = spawn_moving_route(&config, states, Vec::new(), None);

        tokio::time::sleep(Duration::from_secs(300)).await;
        assert!(
            moving.probes.lock().unwrap().is_empty(),
            "no move while the anchor has no verdict"
        );
        assert_eq!(
            config.token_routes.available_permits(),
            TOKEN_ROUTE_SESSIONS - 1
        );

        let bound_at = tokio::time::Instant::now();
        anchor.send_replace(AnchorState::Anchored { max_routes: 32 });
        tokio::time::sleep(Duration::from_secs(1)).await;

        let probes = moving.probes.lock().unwrap().clone();
        assert_eq!(
            probes,
            vec![bound_at],
            "one move, as soon as the anchor is bound"
        );
        assert_eq!(
            *moving.steps.lock().unwrap(),
            ["by anchor up", "token session ended", "carried by anchor"],
            "make before break"
        );
        assert_eq!(
            config.token_routes.available_permits(),
            TOKEN_ROUTE_SESSIONS,
            "the route gives its place among the token routes back"
        );
        assert_eq!(moving.token_dials.load(Ordering::SeqCst), 1);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_move_keeps_the_route_on_its_token_and_is_tried_again_later() {
        let config = anchored(&[EXIT]);
        let (_anchor, states) = watch::channel(AnchorState::Anchored { max_routes: 32 });
        let limit = SessionEnd::Refused(RouteRefusal::Rejected(RouteRejectCode::RouteLimit));
        let (moving, task) = spawn_moving_route(&config, states, vec![Err(limit)], None);

        tokio::time::sleep(config.retry_unavailable_after + Duration::from_secs(1)).await;
        assert_eq!(moving.probes.lock().unwrap().len(), 1);
        assert!(
            moving.token_session_ended.lock().unwrap().is_none(),
            "a refused move leaves the route on its token"
        );

        tokio::time::sleep(config.retry_unavailable_after).await;

        let probes = moving.probes.lock().unwrap().clone();
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[1] - probes[0], config.retry_unavailable_after);
        assert!(moving.token_session_ended.lock().unwrap().is_some());
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_an_exit_refuses_by_anchor_for_good_stays_on_its_token() {
        let config = anchored(&[EXIT]);
        let (_anchor, states) = watch::channel(AnchorState::Anchored { max_routes: 32 });
        let not_offered = SessionEnd::Refused(RouteRefusal::Rejected(RouteRejectCode::NotOffered));
        let (moving, task) = spawn_moving_route(&config, states, vec![Err(not_offered)], None);

        tokio::time::sleep(config.retry_unavailable_after * 10).await;

        assert_eq!(moving.probes.lock().unwrap().len(), 1);
        assert!(moving.token_session_ended.lock().unwrap().is_none());
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_moved_route_whose_anchor_goes_away_is_back_on_a_token_at_once() {
        let config = anchored(&[EXIT]);
        let (_anchor, states) = watch::channel(AnchorState::Anchored { max_routes: 32 });
        let gone = SessionEnd::Refused(RouteRefusal::Ended(RouteEndReason::AnchorGone));
        let (moving, task) = spawn_moving_route(&config, states, Vec::new(), Some(gone));

        tokio::time::sleep(config.retry_unavailable_after + Duration::from_secs(1)).await;
        assert!(moving.carried.lock().unwrap().is_some(), "moved first");
        let dials_after_the_loss = moving.token_dials.load(Ordering::SeqCst);
        tokio::time::sleep(config.retry_unavailable_after / 2).await;

        assert_eq!(
            dials_after_the_loss, 2,
            "the route runs on a token again without waiting"
        );
        assert_eq!(
            moving.probes.lock().unwrap().len(),
            1,
            "and is not moved again right away, which would flap it"
        );
        task.abort();
    }

    /// A dialed supervisor a test drives: it publishes through the returned
    /// sender, its exit assigns through the returned channel, and it ends
    /// with what the test sends it.
    #[expect(clippy::type_complexity, reason = "a test fixture's handles")]
    fn dialed_by_hand() -> (
        Dialed<()>,
        watch::Sender<Option<()>>,
        IpAssignChannel,
        tokio::sync::oneshot::Sender<Result<(), MultiHopError>>,
    ) {
        let (published, client_rx) = watch::channel(None);
        let (end, ended) = tokio::sync::oneshot::channel();
        let ip_assign = IpAssignChannel::new();
        let dialed = Dialed {
            run: Box::pin(async move { ended.await.unwrap_or(Ok(())) }),
            client_rx,
            ip_assign: ip_assign.clone(),
        };
        (dialed, published, ip_assign, end)
    }

    fn assigned() -> IpAssignSpec {
        IpAssignSpec {
            assigned: "10.66.3.4".parse().unwrap(),
            prefix_len: 16,
            gateway: "10.66.0.1".parse().unwrap(),
            assigned_v6: None,
            prefix_len_v6: 64,
            gateway_v6: None,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_by_anchor_is_up_only_once_its_exit_assigned_its_addresses() {
        let (dialed, published, ip_assign, _end) = dialed_by_hand();
        let probing = tokio::spawn(until_connected(dialed, PROBE_TIMEOUT));

        published.send_replace(Some(()));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let up_before_its_address = probing.is_finished();
        ip_assign.publish(assigned());
        let result = probing.await.unwrap();

        assert!(
            !up_before_its_address,
            "no address, nothing it carries could be translated"
        );
        assert!(result.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_by_anchor_that_does_not_come_up_in_time_is_given_up() {
        let (dialed, _published, _ip_assign, _end) = dialed_by_hand();
        let started = tokio::time::Instant::now();

        let result = until_connected(dialed, PROBE_TIMEOUT).await;

        assert!(matches!(
            result,
            Err(SessionEnd::Unavailable(RouteUnavailable::Failed))
        ));
        assert_eq!(started.elapsed(), PROBE_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_by_anchor_refused_before_it_came_up_says_why() {
        let (dialed, published, _ip_assign, end) = dialed_by_hand();
        let refusal = RouteRefusal::Rejected(RouteRejectCode::RouteLimit);
        // The supervisor drops its sender as it ends.
        drop(published);
        end.send(Err(MultiHopError::RouteRefused(refusal))).unwrap();

        let result = until_connected(dialed, PROBE_TIMEOUT).await;

        assert!(matches!(result, Err(SessionEnd::Refused(r)) if r == refusal));
    }

    fn refreshed(has_tokens: bool) -> Credentials {
        Credentials {
            rounds: 1,
            has_tokens,
            issued_elsewhere: false,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_without_a_token_dials_again_as_soon_as_a_refresh_brings_tokens() {
        let (credentials, followed) = watch::channel(Credentials::default());
        let mut config = anchored(&[]);
        config.credentials = Some(followed);
        let refresh = async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            credentials.send_replace(refreshed(true));
            std::future::pending::<()>().await;
        };

        let dials = tokio::select! {
            dials = run_loop(&config, NO_TOKEN, NO_TOKEN, 2) => dials,
            () = refresh => unreachable!(),
        };

        assert_eq!(dials.gaps()[0], Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_that_brings_no_token_leaves_the_route_to_its_wait() {
        let (credentials, followed) = watch::channel(Credentials::default());
        let mut config = anchored(&[]);
        config.credentials = Some(followed);
        let refresh = async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            credentials.send_replace(refreshed(false));
            std::future::pending::<()>().await;
        };

        let dials = tokio::select! {
            dials = run_loop(&config, NO_TOKEN, NO_TOKEN, 2) => dials,
            () = refresh => unreachable!(),
        };

        assert_eq!(dials.gaps()[0], config.retry_unavailable_after);
    }

    #[tokio::test(start_paused = true)]
    async fn the_wait_before_a_retry_ends_as_soon_as_the_anchor_is_bound() {
        let config = anchored(&[EXIT]);
        let (anchor, states) = watch::channel(AnchorState::Unavailable);
        let bind = async {
            tokio::time::sleep(Duration::from_secs(3)).await;
            anchor.send_replace(AnchorState::Anchored { max_routes: 32 });
            std::future::pending::<()>().await;
        };
        let started = tokio::time::Instant::now();

        tokio::select! {
            () = retry_wait(&config, Some(states)) => {}
            () = bind => unreachable!(),
        }

        assert_eq!(started.elapsed(), Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_route_waiting_for_a_token_route_goes_by_anchor_once_the_anchor_admits_it() {
        let config = anchored(&[EXIT]);
        let _every_token_route_taken = Arc::clone(&config.token_routes)
            .try_acquire_many_owned(TOKEN_ROUTE_SESSIONS as u32)
            .unwrap();
        let (_anchor, states) = watch::channel(AnchorState::Anchored { max_routes: 32 });
        let (moving, task) = spawn_moving_route(&config, states, Vec::new(), None);

        tokio::time::sleep(config.retry_unavailable_after / 2).await;
        let anchor_dials_while_damped = moving.anchor_dials.load(Ordering::SeqCst);
        tokio::time::sleep(config.retry_unavailable_after).await;

        assert_eq!(anchor_dials_while_damped, 1, "not asked again at once");
        assert_eq!(moving.anchor_dials.load(Ordering::SeqCst), 2);
        assert_eq!(moving.token_dials.load(Ordering::SeqCst), 0);
        task.abort();
    }
}
