//! Route sessions on the production datapath: the engine's supervisor and
//! pumps, the same as the main session's, admitted on anonymous tokens only.
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

use std::{net::SocketAddr, sync::Arc, time::Duration};

use ed25519_dalek::SigningKey;
use futures::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use talpid_app_routing::{owner::OwnerResolver, router::SessionAddresses};
use warrenguard_backoff::Backoff;
use warrenguard_transport::{
    IpAssignChannel, IpAssignSpec,
    multihop::{MultiHopError, NoSessionTokenCause},
    supervised_pump::{
        ExitDrainingChannel, run_downlink, run_downlink_with_daita, run_idle_cover, run_uplink,
        run_uplink_with_daita,
    },
    supervisor::{
        ClientWatch, MultiHopSupervisor, SessionAdmission, SessionTokenProvider, SupervisorConfig,
    },
};
use warrenguard_transport_core::PacketDevice;

use super::{RouteUnavailable, SessionEvent, controller::RouteSessions, datapath::RouteTun};
use crate::{MultiHopConfig, multi_hop_bind_addr, multi_hop_daita_shared};

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
/// constraints (IP version, DAITA, idle cover), its carrier escape and its
/// token source.
#[derive(Clone)]
pub struct RouteSessionConfig {
    /// The daemon's token provider. `None` leaves every route without a
    /// token, hence unavailable: a route session never falls back to the
    /// wallet.
    pub token_provider: Option<SessionTokenProvider>,
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
    pub fn new(token_provider: Option<SessionTokenProvider>) -> Self {
        Self {
            token_provider,
            wants_ipv6: false,
            enable_daita: false,
            idle_cover: false,
            socket_bypass: None,
            relay_escape: None,
            on_exit_draining: None,
            retry_unavailable_after: RETRY_UNAVAILABLE_AFTER,
        }
    }
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
/// hooks that feed process-wide state.
pub(crate) fn route_supervisor_config(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    ip_assign: IpAssignChannel,
) -> SupervisorConfig {
    SupervisorConfig {
        relay: Arc::new(circuit.relay.clone()),
        exit_id: circuit.exit.exit_id,
        exit_x25519_multihop_pubkey: circuit.exit.exit_x25519_multihop_pubkey,
        exit_mlkem768_pubkey: circuit.exit.exit_mlkem768_pubkey.clone(),
        operational_pubkey: circuit.operational_pubkey,
        // The supervisor wants a key, and under tokens-only admission it never
        // proves possession of it: a random one keeps the wallet out of the
        // route session altogether.
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
        session_token_provider: config.token_provider.clone(),
    }
}

/// A supervisor that only ever admits its sessions on a token.
pub(crate) fn route_supervisor(config: SupervisorConfig) -> (MultiHopSupervisor, ClientWatch) {
    let (supervisor, client_rx) = MultiHopSupervisor::new(config);
    (
        supervisor.with_session_admission(SessionAdmission::TokensOnly),
        client_rx,
    )
}

/// Why a route session that ended cannot run, for the user.
pub(crate) fn unavailable_reason(error: &MultiHopError) -> RouteUnavailable {
    match error {
        MultiHopError::NoSessionToken(NoSessionTokenCause::Empty) => RouteUnavailable::NoToken,
        MultiHopError::NoSessionToken(_) | MultiHopError::Rejected(_) => RouteUnavailable::Refused,
        _ => RouteUnavailable::Failed,
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
    loop {
        events.send(SessionEvent::Connecting);
        let reason = run_supervised(circuit, config, device, events).await;
        log::info!("App routing: a route session ended ({reason:?}); retrying later");
        events.send(SessionEvent::Unavailable(reason));
        tokio::time::sleep(config.retry_unavailable_after).await;
    }
}

/// One supervisor's life: reports each session it publishes, and says why it
/// ended. The supervisor and the pumps run inside this future, not as tasks
/// of their own, so dropping it ends them all.
async fn run_supervised<T: PacketDevice + Clone>(
    circuit: &MultiHopConfig,
    config: &RouteSessionConfig,
    device: &T,
    events: &super::SessionEvents,
) -> RouteUnavailable {
    if config.socket_bypass.is_none()
        && let Some(escape) = &config.relay_escape
    {
        escape(circuit.relay.endpoint).await;
    }
    let ip_assign = IpAssignChannel::new();
    let (supervisor, mut client_rx) =
        route_supervisor(route_supervisor_config(circuit, config, ip_assign.clone()));
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
                    Err(error) => unavailable_reason(&error),
                    // `run` returns `Ok` only once every session receiver
                    // is gone, and this future holds one.
                    Ok(()) => RouteUnavailable::Failed,
                };
            }
            // A pump that ended leaves the route carrying nothing in one
            // direction while the supervisor says it is up: start over.
            Some(()) = pumps.next(), if !pumps.is_empty() => return RouteUnavailable::Failed,
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
                        Err(_) => return RouteUnavailable::Failed,
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

    use warrenguard_multihop::RejectionReason;

    use super::*;

    fn circuit() -> MultiHopConfig {
        crate::app_routes::test_support::circuit(7, 9)
    }

    fn counting_provider() -> (SessionTokenProvider, Arc<AtomicU32>) {
        let calls = Arc::new(AtomicU32::new(0));
        let counted = Arc::clone(&calls);
        let provider: SessionTokenProvider = Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
            Vec::new()
        });
        (provider, calls)
    }

    #[test]
    fn a_route_supervisor_dials_the_planned_circuit_with_the_tunnels_constraints() {
        let mut config = RouteSessionConfig::new(None);
        config.wants_ipv6 = true;
        config.enable_daita = true;

        let built = route_supervisor_config(&circuit(), &config, IpAssignChannel::new());

        assert_eq!(built.relay.relay_id, [7; 16]);
        assert_eq!(built.exit_id.as_bytes(), &[9; 16]);
        assert!(built.wants_ipv6);
        assert!(built.enable_daita);
        assert_eq!(built.n_connections, 1);
    }

    #[test]
    fn a_route_supervisor_presents_the_tokens_of_the_daemons_provider() {
        let (provider, calls) = counting_provider();
        let config = RouteSessionConfig::new(Some(provider));

        let built = route_supervisor_config(&circuit(), &config, IpAssignChannel::new());
        let _ = (built.session_token_provider.expect("a token provider"))();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn each_route_supervisor_holds_a_key_of_its_own() {
        let config = RouteSessionConfig::new(None);

        let first = route_supervisor_config(&circuit(), &config, IpAssignChannel::new());
        let second = route_supervisor_config(&circuit(), &config, IpAssignChannel::new());

        assert_ne!(
            first.client_signing.to_bytes(),
            second.client_signing.to_bytes()
        );
    }

    #[test]
    fn a_route_supervisor_feeds_none_of_the_process_wide_hooks() {
        let config = RouteSessionConfig::new(None);

        let built = route_supervisor_config(&circuit(), &config, IpAssignChannel::new());

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
}
