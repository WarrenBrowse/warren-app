use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use futures::channel::{mpsc, oneshot};
use futures::future::Fuse;
use futures::{FutureExt, StreamExt};
use talpid_routing::RouteManagerHandle;
use talpid_tunnel::tun_provider::TunProvider;
use talpid_tunnel::{EventHook, TunnelArgs, TunnelEvent, TunnelMetadata};
use talpid_types::ErrorExt;
use talpid_types::net::{AllowedClients, AllowedEndpoint, AllowedTunnelTraffic};
use talpid_types::tunnel::{ErrorStateCause, FirewallPolicyError};
use talpid_warren_tunnel::WarrenTunnelParameters;

use super::backend_params::BackendParams;

use super::connected_state::TunnelEventsReceiver;
use super::{
    AfterDisconnect, ConnectedState, DisconnectingState, ErrorState, EventConsequence, EventResult,
    SharedTunnelStateValues, TunnelCommand, TunnelCommandReceiver, TunnelState,
    TunnelStateTransition,
};

use crate::firewall::FirewallPolicy;
#[cfg(target_os = "macos")]
use crate::resolver::LOCAL_DNS_RESOLVER;
use crate::tunnel_state_machine::tunnel_monitor::{self, TunnelMonitor};
#[cfg(target_os = "macos")]
use talpid_dns::DnsConfig;

pub(crate) type TunnelCloseEvent = Fuse<oneshot::Receiver<Option<ErrorStateCause>>>;

const MIN_TUNNEL_ALIVE_TIME: Duration = Duration::from_millis(1000);
#[cfg(target_os = "windows")]
const MAX_ATTEMPT_CREATE_TUN: u32 = 4;

/// Sliding-window flap detector for the reconnect loop.
///
/// `retry_attempt` alone cannot bound the loop: the Warren backend emits
/// `TunnelEvent::Up` as soon as routes install, *before* the data plane
/// is confirmed, so a tunnel that immediately drops still looks
/// "connected" for an instant and resets `retry_attempt`. A network
/// handover that leaves the path unusable therefore churns
/// Connecting→(fake)Connected→Down forever, holding the kill-switch and
/// re-installing split routes on every lap — with no escape for the user
/// (the exact ethernet→WiFi lockup this guards against).
///
/// This wall-clock detector is independent of `retry_attempt`: if the
/// loop laps more than [`FLAP_MAX`] times within [`FLAP_WINDOW`], the
/// state machine stops churning and drops into a STABLE, cancelable
/// `ErrorState` (fail-closed: the kill-switch still blocks leaks, but the
/// route churn stops and `Disconnect` clears it instantly). The window is
/// self-cleaning: a genuinely stable connection emits no close events, so
/// the timestamps age out and the next disconnect starts fresh.
static RECENT_RECONNECTS: LazyLock<Mutex<Vec<Instant>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Max reconnect laps tolerated within [`FLAP_WINDOW`] before the loop is
/// broken to a stable blocked state. Six laps in 25 s (~one every 4 s) is
/// pathological flapping, well clear of any legitimate reconnect cadence.
pub(super) const FLAP_MAX: usize = 6;
pub(super) const FLAP_WINDOW: Duration = Duration::from_secs(25);

/// Record a reconnect lap and report whether the tunnel is flapping.
/// Prunes timestamps older than [`FLAP_WINDOW`] first so the window
/// slides and a stable connection naturally empties it.
///
/// Shared with [`super::ConnectedState`]: the Warren backend emits a
/// premature `Up`, so most laps run Connecting→Connected→Down and the
/// drop is handled by `ConnectedState`, not `ConnectingState`. Both
/// reconnect paths must feed the same window or the detector misses the
/// dominant churn.
pub(super) fn note_reconnect_and_is_flapping(now: Instant) -> bool {
    let mut laps = RECENT_RECONNECTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    laps.retain(|t| now.duration_since(*t) < FLAP_WINDOW);
    laps.push(now);
    laps.len() > FLAP_MAX
}

/// Clear the flap window. Called once the loop is broken to a stable
/// state so a later user-initiated reconnect starts from a clean slate.
pub(super) fn reset_flap_detector() {
    RECENT_RECONNECTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

const INITIAL_ALLOWED_TUNNEL_TRAFFIC: AllowedTunnelTraffic = AllowedTunnelTraffic::None;

/// The tunnel has been started, but it is not established/functional.
pub struct ConnectingState {
    tunnel_events: TunnelEventsReceiver,
    tunnel_parameters: BackendParams,
    tunnel_metadata: Option<TunnelMetadata>,
    allowed_tunnel_traffic: AllowedTunnelTraffic,
    tunnel_close_event: TunnelCloseEvent,
    tunnel_close_tx: oneshot::Sender<()>,
    retry_attempt: u32,
}

impl ConnectingState {
    pub(super) fn enter(
        shared_values: &mut SharedTunnelStateValues,
        retry_attempt: u32,
    ) -> (Box<dyn TunnelState>, TunnelStateTransition) {
        #[cfg(target_os = "macos")]
        if *LOCAL_DNS_RESOLVER {
            // Set system DNS to our local DNS resolver
            let system_dns = DnsConfig::default().resolve(
                &[shared_values.filtering_resolver.listening_addr().ip()],
                shared_values.filtering_resolver.listening_addr().port(),
            );
            let _ = shared_values
                .dns_monitor
                .set("lo", system_dns)
                .inspect_err(|err| {
                    log::error!(
                        "{}",
                        err.display_chain_with_msg(
                            "Failed to configure system to use filtering resolver"
                        )
                    );
                });
        }

        Self::enter_warren(shared_values, retry_attempt)
    }

    /// Warren (QUIC) connecting path.
    ///
    /// - No `ip_availability` check on the state machine side — the
    ///   backend handles its own multi-path resolution (v4/v6) at
    ///   handshake time.
    /// - No Android `prepare_tun_config` — the Warren backend opens its
    ///   own TUN via `args.tun_provider` on the `WarrenTunnelMonitor::start` side.
    ///
    /// The pre-handshake firewall is applied via
    /// [`Self::set_firewall_policy`]:
    /// [`BackendParams::get_next_hop_endpoints`] exposes the candidate
    /// exit IPs (from `endpoint_addr.ip_addrs()`), which authorizes
    /// outgoing traffic to the exit without regressing no-leak.
    fn enter_warren(
        shared_values: &mut SharedTunnelStateValues,
        retry_attempt: u32,
    ) -> (Box<dyn TunnelState>, TunnelStateTransition) {
        let warren_params = match shared_values.runtime.block_on(
            shared_values
                .tunnel_parameters_generator
                .generate_warren_tunnel_params(retry_attempt),
        ) {
            Ok(params) => params,
            Err(err) => {
                return ErrorState::enter(
                    shared_values,
                    ErrorStateCause::TunnelParameterError(err),
                );
            }
        };

        // Pre-handshake firewall for the Warren tunnel: the
        // `BackendParams::Warren` variant exposes the candidate exit IPs via
        // `get_next_hop_endpoints()`, and the firewall authorizes them just like
        // it does for WG peers (no no-leak regression).
        let backend_info = super::backend_params::WarrenBackendInfo::from_params(&warren_params);
        let backend_params = BackendParams::Warren(backend_info);
        if let Err(error) = Self::set_firewall_policy(
            shared_values,
            &backend_params,
            &None,
            AllowedTunnelTraffic::None,
        ) {
            return ErrorState::enter(
                shared_values,
                ErrorStateCause::SetFirewallPolicyError(error),
            );
        }

        let connecting_state = Self::start_tunnel_warren(
            shared_values.runtime.clone(),
            warren_params,
            &shared_values.log_dir,
            &shared_values.resource_dir,
            shared_values.tun_provider.clone(),
            &shared_values.route_manager,
            retry_attempt,
        );

        let transition_endpoint = connecting_state.tunnel_parameters.get_tunnel_endpoint();
        (
            Box::new(connecting_state),
            TunnelStateTransition::Connecting(transition_endpoint),
        )
    }

    /// Mirror of [`Self::start_tunnel`] for the Warren side: identical
    /// structure (event channel, oneshot close, retry/sleep policy)
    /// but invokes [`TunnelMonitor::start_warren_tunnel`] instead of
    /// [`TunnelMonitor::start`].
    fn start_tunnel_warren(
        runtime: tokio::runtime::Handle,
        parameters: WarrenTunnelParameters,
        log_dir: &Option<PathBuf>,
        resource_dir: &Path,
        tun_provider: Arc<Mutex<TunProvider>>,
        route_manager: &RouteManagerHandle,
        retry_attempt: u32,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded();
        let event_hook = EventHook::new(event_tx);

        let route_manager = route_manager.clone();
        let log_dir = log_dir.clone();
        let resource_dir = resource_dir.to_path_buf();

        let (tunnel_close_tx, tunnel_close_rx) = oneshot::channel();
        let (tunnel_close_event_tx, tunnel_close_event_rx) = oneshot::channel();

        // Extract the lightweight display/firewall snapshot before
        // moving the full parameters (including signing_key) into the
        // blocking task. No secret material is cloned.
        let stored_params = BackendParams::Warren(
            super::backend_params::WarrenBackendInfo::from_params(&parameters),
        );

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();

            let args = TunnelArgs {
                runtime,
                resource_dir: &resource_dir,
                event_hook,
                tunnel_close_rx,
                tun_provider,
                retry_attempt,
                route_manager,
            };

            let block_reason = match TunnelMonitor::start_warren_tunnel(&parameters, &log_dir, args)
            {
                Ok(monitor) => {
                    let reason = Self::wait_for_tunnel_monitor(monitor, retry_attempt);
                    log::debug!(
                        "Warren tunnel monitor exited with block reason: {:?}",
                        reason
                    );
                    reason
                }
                Err(error) if should_retry(&error, retry_attempt) => {
                    log::warn!(
                        "{}",
                        error.display_chain_with_msg(
                            "Retrying to connect after failing to start Warren tunnel"
                        )
                    );
                    None
                }
                Err(error) => {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to start Warren tunnel")
                    );
                    Some(error.into())
                }
            };

            if block_reason.is_none()
                && let Some(remaining_time) = MIN_TUNNEL_ALIVE_TIME.checked_sub(start.elapsed())
            {
                thread::sleep(remaining_time);
            }

            if tunnel_close_event_tx.send(block_reason).is_err() {
                log::warn!("Tunnel state machine stopped before receiving tunnel closed event");
            }

            log::trace!("Warren tunnel monitor thread exit");
        });

        ConnectingState {
            tunnel_events: event_rx.fuse(),
            tunnel_parameters: stored_params,
            tunnel_metadata: None,
            allowed_tunnel_traffic: INITIAL_ALLOWED_TUNNEL_TRAFFIC,
            tunnel_close_event: tunnel_close_event_rx.fuse(),
            tunnel_close_tx,
            retry_attempt,
        }
    }

    fn set_firewall_policy(
        shared_values: &mut SharedTunnelStateValues,
        params: &BackendParams,
        tunnel_metadata: &Option<TunnelMetadata>,
        allowed_tunnel_traffic: AllowedTunnelTraffic,
    ) -> Result<(), FirewallPolicyError> {
        #[cfg(target_os = "linux")]
        shared_values.disable_connectivity_check();

        let endpoints = params.get_next_hop_endpoints();

        #[cfg(target_os = "windows")]
        let clients = AllowedClients::from(vec![std::env::current_exe().unwrap()]);

        #[cfg(not(target_os = "windows"))]
        let clients = AllowedClients::Root;

        #[cfg(target_os = "windows")]
        let exit_endpoint_ip = params.get_exit_hop_endpoint().map(|ep| ep.address.ip());

        let peer_endpoints = endpoints
            .into_iter()
            .map(|endpoint| AllowedEndpoint {
                endpoint,
                clients: clients.clone(),
            })
            .collect();

        #[cfg(target_os = "macos")]
        let redirect_interface = shared_values
            .runtime
            .block_on(shared_values.split_tunnel.interface());

        let policy = FirewallPolicy::Connecting {
            peer_endpoints,
            #[cfg(target_os = "windows")]
            exit_endpoint_ip,
            tunnel: tunnel_metadata.clone(),
            allow_lan: shared_values.allow_lan,
            allowed_endpoint: shared_values.allowed_endpoint.clone(),
            allowed_tunnel_traffic,
            #[cfg(target_os = "macos")]
            redirect_interface,
        };
        shared_values
            .firewall
            .apply_policy(policy)
            .map_err(|error| {
                log::error!(
                    "{}",
                    error.display_chain_with_msg(
                        "Failed to apply firewall policy for connecting state"
                    )
                );
                match error {
                    #[cfg(windows)]
                    crate::firewall::Error::ApplyingConnectingPolicy(policy_error) => policy_error,
                    _ => FirewallPolicyError::Generic,
                }
            })
    }

    fn wait_for_tunnel_monitor(
        tunnel_monitor: TunnelMonitor,
        retry_attempt: u32,
    ) -> Option<ErrorStateCause> {
        match tunnel_monitor.wait() {
            Ok(_) => None,
            Err(error) if !should_retry(&error, retry_attempt) => {
                log::error!(
                    "{}",
                    error.display_chain_with_msg("Tunnel has stopped unexpectedly")
                );
                Some(ErrorStateCause::StartTunnelError)
            }
            Err(error) => {
                log::warn!(
                    "{}",
                    error.display_chain_with_msg("Tunnel has stopped unexpectedly")
                );
                None
            }
        }
    }

    fn reset_routes(
        #[cfg(target_os = "windows")] shared_values: &SharedTunnelStateValues,
        #[cfg(not(target_os = "windows"))] shared_values: &mut SharedTunnelStateValues,
    ) {
        if let Err(error) = shared_values.route_manager.clear_routes() {
            log::error!("{}", error.display_chain_with_msg("Failed to clear routes"));
        }

        // The Warren split-default routes are installed via the
        // `route`/`ip` CLI out-of-band, so the talpid `RouteManager` above
        // has no record of them and `clear_routes()` cannot remove them.
        // Force them out here so a route reset (disconnect, reconnect,
        // error transition) can never leave a `0.0.0.0/1` → TUN split
        // blackholing every egress packet. Idempotent no-op when nothing
        // is installed.
        talpid_warren_tunnel::force_route_cleanup();
        #[cfg(target_os = "linux")]
        if let Err(error) = shared_values
            .runtime
            .block_on(shared_values.route_manager.clear_routing_rules())
        {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to clear routing rules")
            );
        }
    }

    fn disconnect(
        self,
        shared_values: &mut SharedTunnelStateValues,
        after_disconnect: AfterDisconnect,
    ) -> EventConsequence {
        Self::reset_routes(shared_values);

        EventConsequence::NewState(DisconnectingState::enter(
            self.tunnel_close_tx,
            self.tunnel_close_event,
            after_disconnect,
        ))
    }

    #[cfg(not(target_os = "android"))]
    fn reset_firewall(
        self: Box<Self>,
        shared_values: &mut SharedTunnelStateValues,
    ) -> EventConsequence {
        match Self::set_firewall_policy(
            shared_values,
            &self.tunnel_parameters,
            &self.tunnel_metadata,
            self.allowed_tunnel_traffic.clone(),
        ) {
            Ok(()) => EventConsequence::SameState(self),
            Err(error) => self.disconnect(
                shared_values,
                AfterDisconnect::Block(ErrorStateCause::SetFirewallPolicyError(error)),
            ),
        }
    }

    fn handle_commands(
        self: Box<Self>,
        command: Option<TunnelCommand>,
        shared_values: &mut SharedTunnelStateValues,
    ) -> EventConsequence {
        use self::EventConsequence::*;

        match command {
            Some(TunnelCommand::AllowLan(allow_lan, complete_tx)) => {
                let consequence = if shared_values.set_allow_lan(allow_lan) {
                    #[cfg(target_os = "android")]
                    {
                        self.disconnect(shared_values, AfterDisconnect::Reconnect(0))
                    }
                    #[cfg(not(target_os = "android"))]
                    self.reset_firewall(shared_values)
                } else {
                    SameState(self)
                };
                let _ = complete_tx.send(());
                consequence
            }
            #[cfg(not(target_os = "android"))]
            Some(TunnelCommand::AllowEndpoint(endpoint, tx)) => {
                if shared_values.allowed_endpoint != endpoint {
                    shared_values.allowed_endpoint = endpoint;
                    if let Err(error) = Self::set_firewall_policy(
                        shared_values,
                        &self.tunnel_parameters,
                        &self.tunnel_metadata,
                        self.allowed_tunnel_traffic.clone(),
                    ) {
                        let _ = tx.send(());
                        return self.disconnect(
                            shared_values,
                            AfterDisconnect::Block(ErrorStateCause::SetFirewallPolicyError(error)),
                        );
                    }
                }
                let _ = tx.send(());
                SameState(self)
            }
            Some(TunnelCommand::Dns(servers, complete_tx)) => {
                let consequence = if shared_values.set_dns_config(servers) {
                    #[cfg(target_os = "android")]
                    {
                        self.disconnect(shared_values, AfterDisconnect::Reconnect(0))
                    }
                    #[cfg(not(target_os = "android"))]
                    SameState(self)
                } else {
                    SameState(self)
                };

                let _ = complete_tx.send(());
                consequence
            }
            #[cfg(not(target_os = "android"))]
            Some(TunnelCommand::LockdownMode(lockdown_mode, complete_tx)) => {
                shared_values.lockdown_mode = lockdown_mode;
                let _ = complete_tx.send(());
                SameState(self)
            }
            Some(TunnelCommand::Connectivity(connectivity)) => {
                shared_values.connectivity = connectivity;
                if connectivity.is_offline() {
                    self.disconnect(
                        shared_values,
                        AfterDisconnect::Block(ErrorStateCause::IsOffline),
                    )
                } else {
                    SameState(self)
                }
            }
            Some(TunnelCommand::Connect) => {
                self.disconnect(shared_values, AfterDisconnect::Reconnect(0))
            }
            Some(TunnelCommand::Disconnect) | None => {
                self.disconnect(shared_values, AfterDisconnect::Nothing)
            }
            Some(TunnelCommand::Block(reason)) => {
                self.disconnect(shared_values, AfterDisconnect::Block(reason))
            }
            #[cfg(target_os = "android")]
            Some(TunnelCommand::BypassSocket(fd, done_tx)) => {
                shared_values.bypass_socket(fd, done_tx);
                SameState(self)
            }
            #[cfg(windows)]
            Some(TunnelCommand::SetExcludedApps(result_tx, paths)) => {
                shared_values.exclude_paths(paths, result_tx);
                SameState(self)
            }
            #[cfg(target_os = "android")]
            Some(TunnelCommand::SetExcludedApps(result_tx, paths)) => {
                if shared_values.set_excluded_paths(paths) {
                    let _ = result_tx.send(Ok(()));
                    self.disconnect(shared_values, AfterDisconnect::Reconnect(0))
                } else {
                    let _ = result_tx.send(Ok(()));
                    SameState(self)
                }
            }
            #[cfg(target_os = "macos")]
            Some(TunnelCommand::SetExcludedApps(result_tx, paths)) => {
                match shared_values.set_exclude_paths(paths) {
                    Ok(added_device) => {
                        let _ = result_tx.send(Ok(()));

                        if added_device
                            && let Err(error) = Self::set_firewall_policy(
                                shared_values,
                                &self.tunnel_parameters,
                                &self.tunnel_metadata,
                                self.allowed_tunnel_traffic.clone(),
                            )
                        {
                            return self.disconnect(
                                shared_values,
                                AfterDisconnect::Block(ErrorStateCause::SetFirewallPolicyError(
                                    error,
                                )),
                            );
                        }
                    }
                    Err(error) => {
                        let cause = ErrorStateCause::from(&error);
                        let _ = result_tx.send(Err(error));
                        return self.disconnect(shared_values, AfterDisconnect::Block(cause));
                    }
                }
                SameState(self)
            }
        }
    }

    fn handle_tunnel_events(
        mut self: Box<Self>,
        event: Option<(TunnelEvent, oneshot::Sender<()>)>,
        shared_values: &mut SharedTunnelStateValues,
    ) -> EventConsequence {
        use self::EventConsequence::*;

        match event {
            Some((TunnelEvent::AuthFailed(reason), _)) => self.disconnect(
                shared_values,
                AfterDisconnect::Block(ErrorStateCause::AuthFailed(reason)),
            ),
            Some((TunnelEvent::InterfaceUp(metadata, allowed_tunnel_traffic), _done_tx)) => {
                #[cfg(windows)]
                if let Err(error) = shared_values
                    .split_tunnel
                    .set_tunnel_addresses(Some(&metadata))
                {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg(
                            "Failed to register addresses with split tunnel driver"
                        )
                    );
                    return self.disconnect(
                        shared_values,
                        AfterDisconnect::Block(ErrorStateCause::SplitTunnelError),
                    );
                }

                #[cfg(target_os = "macos")]
                if let Err(error) = shared_values.enable_split_tunnel(&metadata) {
                    return self.disconnect(shared_values, AfterDisconnect::Block(error));
                }

                self.allowed_tunnel_traffic = allowed_tunnel_traffic;
                self.tunnel_metadata = Some(metadata);

                match Self::set_firewall_policy(
                    shared_values,
                    &self.tunnel_parameters,
                    &self.tunnel_metadata,
                    self.allowed_tunnel_traffic.clone(),
                ) {
                    Ok(()) => SameState(self),
                    Err(error) => self.disconnect(
                        shared_values,
                        AfterDisconnect::Block(ErrorStateCause::SetFirewallPolicyError(error)),
                    ),
                }
            }
            Some((TunnelEvent::Up(metadata), _)) => NewState(ConnectedState::enter(
                shared_values,
                metadata,
                self.tunnel_events,
                self.tunnel_parameters,
                self.tunnel_close_event,
                self.tunnel_close_tx,
            )),
            Some((TunnelEvent::Down, _)) => {
                // It is important to reset this before the tunnel device is down,
                // or else commands that reapply the firewall rules will fail since
                // they refer to a non-existent device.
                self.allowed_tunnel_traffic = INITIAL_ALLOWED_TUNNEL_TRAFFIC;
                self.tunnel_metadata = None;

                SameState(self)
            }
            None => {
                // The channel was closed
                log::debug!("The tunnel disconnected unexpectedly");
                let retry_attempt = self.retry_attempt + 1;
                self.disconnect(shared_values, AfterDisconnect::Reconnect(retry_attempt))
            }
        }
    }

    fn handle_tunnel_close_event(
        self,
        block_reason: Option<ErrorStateCause>,
        shared_values: &mut SharedTunnelStateValues,
    ) -> EventConsequence {
        use self::EventConsequence::*;

        if let Some(block_reason) = block_reason {
            reset_flap_detector();
            Self::reset_routes(shared_values);
            return NewState(ErrorState::enter(shared_values, block_reason));
        }

        // Bound the reconnect loop. If the tunnel is flapping faster than
        // any legitimate reconnect cadence, stop churning and drop into a
        // STABLE, cancelable blocked state instead of holding the
        // kill-switch and re-installing split routes forever. This is the
        // structural guarantee that a bad network handover can never wedge
        // Warren with no escape (fail-closed but always cancelable).
        if note_reconnect_and_is_flapping(Instant::now()) {
            log::error!(
                "Warren tunnel is flapping (>{FLAP_MAX} reconnects within {FLAP_WINDOW:?}); \
                 entering a stable blocked state instead of churning. \
                 Disconnect to clear, then reconnect once the network is stable."
            );
            reset_flap_detector();
            Self::reset_routes(shared_values);
            return NewState(ErrorState::enter(
                shared_values,
                ErrorStateCause::StartTunnelError,
            ));
        }

        log::info!(
            "Tunnel closed. Reconnecting, attempt {}.",
            self.retry_attempt + 1
        );
        Self::reset_routes(shared_values);
        EventConsequence::NewState(ConnectingState::enter(
            shared_values,
            self.retry_attempt + 1,
        ))
    }
}

#[cfg_attr(not(target_os = "windows"), expect(unused_variables))]
fn should_retry(error: &tunnel_monitor::Error, retry_attempt: u32) -> bool {
    #[cfg(target_os = "windows")]
    if error.get_tunnel_device_error().is_some() {
        return retry_attempt < MAX_ATTEMPT_CREATE_TUN;
    }
    error.is_recoverable()
}

impl TunnelState for ConnectingState {
    fn handle_event(
        mut self: Box<Self>,
        runtime: &tokio::runtime::Handle,
        commands: &mut TunnelCommandReceiver,
        shared_values: &mut SharedTunnelStateValues,
    ) -> EventConsequence {
        let result = runtime.block_on(async {
            futures::select! {
                command = commands.next() => EventResult::Command(command),
                event = self.tunnel_events.next() => EventResult::Event(event),
                result = &mut self.tunnel_close_event => EventResult::Close(result),
            }
        });

        match result {
            EventResult::Command(command) => self.handle_commands(command, shared_values),
            EventResult::Event(event) => self.handle_tunnel_events(event, shared_values),
            EventResult::Close(result) => {
                if result.is_err() {
                    log::warn!("Tunnel monitor thread has stopped unexpectedly");
                }
                let block_reason = result.unwrap_or(None);
                self.handle_tunnel_close_event(block_reason, shared_values)
            }
        }
    }
}
