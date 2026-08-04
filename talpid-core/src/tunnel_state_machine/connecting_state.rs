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
    SharedTunnelStateValues, SubscriptionStatus, TunnelCommand, TunnelCommandReceiver, TunnelState,
    TunnelStateTransition, exit_refusal,
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

/// Number of leading connect attempts that retry at [`MIN_TUNNEL_ALIVE_TIME`]
/// before the interval starts growing.
const FAST_RETRY_ATTEMPTS: u32 = 4;

/// Ceiling on the retry interval, and with it the worst-case wait between an
/// exit admitting a wallet and the client noticing.
///
/// Measured on the beta fleet: at 15 s the schedule leaves a gap wide enough to
/// straddle an exit's allowlist poll, and a wallet registered a second before
/// connecting took 35.3 s to reach the internet in 2 runs out of 3. At 10 s the
/// same three runs measured 1.11 / 4.13 / 11.19 s, for the same number of dials
/// per minute.
const MAX_RECONNECT_FLOOR: Duration = Duration::from_secs(10);

/// Lower bound on how long a failed connect lap takes, which is what paces the
/// reconnect loop.
///
/// The first [`FAST_RETRY_ATTEMPTS`] attempts keep the base floor: the common
/// refusal (an exit whose allowlist poll has not yet picked up a fresh
/// subscription) clears within a second or two, and a prompt retry is what
/// keeps that case invisible. Past that the interval doubles up to
/// [`MAX_RECONNECT_FLOOR`], because a failure that survived four laps will not
/// clear any sooner for being asked ten more times and each dial costs the
/// exit an admission check.
///
/// The base is [`MIN_TUNNEL_ALIVE_TIME`], upstream's guard against a tunnel
/// that dies instantly spinning the loop, unless [`exit_refusal`] hands over
/// its permission to dial sooner (a refusal the API confirmed will clear, and
/// confirmed recently enough to still be reusing that answer, the one case
/// where a faster dial cannot be spinning on a broken tunnel).
fn reconnect_floor(retry_attempt: u32, pacing: exit_refusal::LapPacing) -> Duration {
    let doublings = retry_attempt.saturating_sub(FAST_RETRY_ATTEMPTS - 1);
    let factor = 1u32.checked_shl(doublings).unwrap_or(u32::MAX);
    pacing
        .floor(MIN_TUNNEL_ALIVE_TIME)
        .saturating_mul(factor)
        .min(MAX_RECONNECT_FLOOR)
}

/// Report how a failed connect lap ended to [`exit_refusal`], which is the one
/// place that decides whether a refusal is worth waiting for. The lap itself
/// never parks the state machine: it only records, and the next
/// [`ConnectingState::enter_warren`] acts on it.
fn note_failed_attempt(error: &tunnel_monitor::Error) {
    exit_refusal::note_attempt(error.warren_session_rejection(), Instant::now());
}

/// Ask the account API whether the refusals seen so far can still clear, and
/// return the state to park on when they cannot.
///
/// Blocking on the state-machine thread is deliberate: the answer decides
/// whether this attempt happens at all, and it is bounded by
/// [`exit_refusal::STATUS_TIMEOUT`]. It costs nothing on a connect that is not
/// being refused, because [`exit_refusal`] only asks while a refusal run is
/// open.
fn refusal_block_reason(shared_values: &SharedTunnelStateValues) -> Option<ErrorStateCause> {
    let runtime = shared_values.runtime.clone();
    let provider = Arc::clone(&shared_values.subscription_status);
    exit_refusal::block_reason_before_dialing(Instant::now(), move || {
        let status = runtime.block_on(async {
            tokio::time::timeout(exit_refusal::STATUS_TIMEOUT, provider.subscription_status())
                .await
                .unwrap_or(SubscriptionStatus::Unknown)
        });
        log::info!("The exit refused the session; the account API reports {status:?}");
        status
    })
}

/// Sliding-window flap detector for the reconnect loop.
///
/// `retry_attempt` cannot bound the loop: the Warren backend emits
/// `TunnelEvent::Up` once routes install, before the data plane is
/// confirmed, so a path that drops immediately still resets
/// `retry_attempt`. A bad handover thus churns Connecting→(fake)
/// Connected→Down forever. This wall-clock detector is independent of
/// `retry_attempt`: more than [`FLAP_MAX`] laps within [`FLAP_WINDOW`]
/// drop into a stable, cancelable `ErrorState` instead (fail-closed: the
/// kill-switch still blocks, but the churn stops). Self-cleaning - a
/// stable connection emits no close events, so the window ages out.
static RECENT_RECONNECTS: LazyLock<Mutex<Vec<Instant>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Max reconnect laps tolerated within [`FLAP_WINDOW`] before the loop is
/// broken to a stable blocked state. Six laps in 25 s (~one every 4 s) is
/// pathological flapping, well clear of any legitimate reconnect cadence.
pub(super) const FLAP_MAX: usize = 6;
pub(super) const FLAP_WINDOW: Duration = Duration::from_secs(25);

/// Record a reconnect lap and report whether the tunnel is flapping.
/// Prunes timestamps older than [`FLAP_WINDOW`] first so the window
/// slides and a stable connection naturally empties it. Shared by the
/// Connecting and Connected reconnect paths (see [`RECENT_RECONNECTS`]).
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
    /// - No `ip_availability` check on the state machine side - the
    ///   backend handles its own multi-path resolution (v4/v6) at
    ///   handshake time.
    /// - No Android `prepare_tun_config` - the Warren backend opens its
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
        // Attempt zero is a fresh connect (the user pressed Connect, or a
        // tunnel that was up came down), so whatever the previous run was
        // refused for, the exit gets a full budget and a fresh question again.
        if retry_attempt == 0 {
            exit_refusal::forget_run();
        } else if let Some(cause) = refusal_block_reason(shared_values) {
            return ErrorState::enter(shared_values, cause);
        }

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
                    note_failed_attempt(&error);
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

            // Pace the loop from here rather than from the state machine: this
            // is the one place a failed lap is guaranteed to pass through, and
            // padding the lap keeps the pacing invisible to every state that
            // waits on the close event.
            if block_reason.is_none()
                && let Some(remaining_time) =
                    reconnect_floor(retry_attempt, exit_refusal::lap_pacing(Instant::now()))
                        .checked_sub(start.elapsed())
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
                note_failed_attempt(&error);
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

        // Warren split routes are out-of-band from the `RouteManager`
        // above, so `clear_routes()` cannot remove them. See
        // `force_route_cleanup`.
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
                shared_values.set_lockdown_mode(lockdown_mode);
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

        // Same offline precedence as ConnectedState: a close while the
        // host is offline must read as `IsOffline` (auto-recovers on the
        // online edge), never count toward the flap detector.
        if shared_values.connectivity.is_offline() {
            reset_flap_detector();
            Self::reset_routes(shared_values);
            return NewState(ErrorState::enter(shared_values, ErrorStateCause::IsOffline));
        }

        // A refusal loop is not flapping. The detector exists for a data plane
        // that churns Connecting -> (fake) Connected -> Down; a refused session
        // never reaches Connected, its cadence is deliberate, and
        // [`exit_refusal`] already decides when it stops. Counting it here
        // would break the loop at the sixth lap on the exit's behalf, which is
        // exactly the paying user this change exists to protect.
        if exit_refusal::run_is_open() {
            log::debug!(
                "Tunnel closed on an exit refusal. Reconnecting, attempt {}.",
                self.retry_attempt + 1
            );
            Self::reset_routes(shared_values);
            return NewState(ConnectingState::enter(
                shared_values,
                self.retry_attempt + 1,
            ));
        }

        // Bound the loop: sustained flapping drops to a stable, cancelable
        // blocked state instead of churning. See [`RECENT_RECONNECTS`].
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
                ErrorStateCause::WarrenTunnelFlapping,
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
            EventResult::OfflineGraceExpired => {
                unreachable!("offline grace only exists in the connected state")
            }
        }
    }
}

#[cfg(test)]
mod retry_policy_tests {
    use std::time::Duration;

    use super::exit_refusal::LapPacing;
    use super::{
        FAST_RETRY_ATTEMPTS, MAX_RECONNECT_FLOOR, MIN_TUNNEL_ALIVE_TIME, reconnect_floor,
        tunnel_monitor,
    };

    fn refusal() -> tunnel_monitor::Error {
        tunnel_monitor::Error::WarrenTunnelMonitoring(talpid_warren_tunnel::Error::SessionRejected(
            "[EXPIRED_ACCOUNT] exit rejected the session".to_owned(),
        ))
    }

    fn network_glitch() -> tunnel_monitor::Error {
        tunnel_monitor::Error::WarrenTunnelMonitoring(
            talpid_warren_tunnel::Error::BackendTransient("tun read timeout".to_owned()),
        )
    }

    #[test]
    fn the_first_laps_retry_at_the_floor_so_a_syncing_exit_stays_invisible() {
        // The measured common case connects after 1 to 3 refusals: the exit
        // was mid-poll and admits the wallet a second later. Slowing those
        // laps down would turn an invisible hiccup into a visible outage.
        for attempt in 0..FAST_RETRY_ATTEMPTS {
            assert_eq!(
                reconnect_floor(attempt, LapPacing::ANTI_SPIN),
                MIN_TUNNEL_ALIVE_TIME,
                "attempt {attempt} must still retry promptly"
            );
        }
    }

    #[test]
    fn only_a_confirmed_transient_refusal_dials_at_the_shorter_floor() {
        // The narrowness is the whole safety argument: every other failure
        // keeps upstream's anti-spin second, so a tunnel that dies instantly
        // can never be made to spin twice as fast by this.
        for attempt in 0..FAST_RETRY_ATTEMPTS {
            assert_eq!(
                reconnect_floor(attempt, LapPacing::CONFIRMED_TRANSIENT),
                Duration::from_millis(500),
                "a refusal the API confirmed must halve the wait at attempt {attempt}"
            );
            assert_eq!(
                reconnect_floor(attempt, LapPacing::ANTI_SPIN),
                MIN_TUNNEL_ALIVE_TIME,
                "everything else keeps the anti-spin floor at attempt {attempt}"
            );
        }
    }

    #[test]
    fn the_shorter_floor_still_grows_and_is_still_capped() {
        // Halving the floor must not remove the bound a stuck refusal needs.
        assert_eq!(
            reconnect_floor(4, LapPacing::CONFIRMED_TRANSIENT),
            Duration::from_secs(1)
        );
        assert_eq!(
            reconnect_floor(7, LapPacing::CONFIRMED_TRANSIENT),
            Duration::from_secs(8)
        );
        assert_eq!(
            reconnect_floor(u32::MAX, LapPacing::CONFIRMED_TRANSIENT),
            MAX_RECONNECT_FLOOR
        );
    }

    #[test]
    fn one_refusal_cycle_still_costs_the_exit_ten_dials_not_twenty() {
        // The trade poka approved, computed rather than asserted: the shorter
        // floor buys back up to a second of lateness for one extra dial per
        // 30 s. The first lap has no verdict yet, so it always pays the full
        // anti-spin floor.
        let mut elapsed = Duration::ZERO;
        let mut dials = 1;
        for attempt in 0.. {
            let pacing = if attempt == 0 {
                LapPacing::ANTI_SPIN
            } else {
                LapPacing::CONFIRMED_TRANSIENT
            };
            elapsed += reconnect_floor(attempt, pacing);
            if elapsed >= Duration::from_secs(30) {
                break;
            }
            dials += 1;
        }
        assert_eq!(
            dials, 10,
            "a refusal that never clears must stay near ten dials per 30 s"
        );
    }

    #[test]
    fn the_interval_grows_and_is_capped_so_a_stuck_refusal_stops_hammering() {
        assert_eq!(
            reconnect_floor(4, LapPacing::ANTI_SPIN),
            Duration::from_secs(2)
        );
        assert_eq!(
            reconnect_floor(5, LapPacing::ANTI_SPIN),
            Duration::from_secs(4)
        );
        assert_eq!(
            reconnect_floor(6, LapPacing::ANTI_SPIN),
            Duration::from_secs(8)
        );
        assert_eq!(
            reconnect_floor(7, LapPacing::ANTI_SPIN),
            MAX_RECONNECT_FLOOR,
            "the interval must be capped, not doubled forever"
        );
        assert_eq!(
            reconnect_floor(u32::MAX, LapPacing::ANTI_SPIN),
            MAX_RECONNECT_FLOOR,
            "a very long run must not overflow into a tiny (or infinite) interval"
        );
    }

    #[test]
    fn only_an_exit_refusal_feeds_the_refusal_run() {
        // What the connecting state contributes to the decision: which failed
        // laps count as refusals. Anything else must close the run, or a
        // network glitch would be judged as an account verdict.
        assert_eq!(
            refusal().warren_session_rejection(),
            Some("[EXPIRED_ACCOUNT] exit rejected the session")
        );
        assert_eq!(network_glitch().warren_session_rejection(), None);
    }
}
