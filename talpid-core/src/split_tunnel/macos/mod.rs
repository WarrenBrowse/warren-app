use core::fmt;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Weak},
};
use talpid_routing::RouteManagerHandle;
use talpid_types::{
    ErrorExt,
    split_tunnel::{SplitApps, SplitTunnelMode},
    tunnel::ErrorStateCause,
};
use tokio::sync::{mpsc, oneshot};

use self::process::ExclusionStatus;

#[expect(non_camel_case_types, clippy::allow_attributes)]
mod bindings;
mod bpf;
mod default;
mod process;
mod tun;

use crate::tunnel_state_machine::TunnelCommand;
pub use tun::VpnInterface;

/// Check whether the current process has full-disk access enabled.
/// This is required by the process monitor.
pub use process::has_full_disk_access;
/// Whether the split tunnel can run here, and why not.
pub use process::{Capability, build_supports_split_tunnel, capability};

/// Errors caused by split tunneling
#[derive(Debug, Clone)]
pub struct Error {
    inner: Arc<InnerError>,
}

impl Error {
    fn unavailable() -> Self {
        Self {
            inner: Arc::new(InnerError::Unavailable),
        }
    }
}

impl From<&Error> for ErrorStateCause {
    fn from(value: &Error) -> Self {
        match &*value.inner {
            InnerError::Process(error) => ErrorStateCause::from(error),
            _v if _v.is_offline() => ErrorStateCause::IsOffline,
            _ => ErrorStateCause::SplitTunnelError,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.inner, f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl<T: Into<InnerError>> From<T> for Error {
    fn from(inner: T) -> Self {
        Self {
            inner: Arc::new(inner.into()),
        }
    }
}

/// Errors caused by split tunneling
#[derive(thiserror::Error, Debug)]
enum InnerError {
    /// Process monitor error
    #[error("Process monitor error")]
    Process(#[from] process::Error),
    /// Failed to initialize split tunnel
    #[error("Failed to initialize split tunnel")]
    InitializeTunnel(#[from] tun::Error),
    /// Default interface unavailable
    #[error("Default interface unavailable")]
    Default(#[from] default::Error),
    /// Split tunnel is unavailable
    #[error("Split tunnel is unavailable")]
    Unavailable,
}

impl InnerError {
    /// Return whether the error is due to a missing default route
    fn is_offline(&self) -> bool {
        matches!(self, InnerError::Default(_))
    }
}

/// Split tunneling actor
pub struct SplitTunnel {
    state: State,
    tunnel_tx: Weak<futures::channel::mpsc::UnboundedSender<TunnelCommand>>,
    rx: mpsc::UnboundedReceiver<Message>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

enum Message {
    /// Return the name of the split tunnel interface
    GetInterface {
        result_tx: oneshot::Sender<Option<String>>,
    },
    /// Shut down split tunnel service
    Shutdown { result_tx: oneshot::Sender<()> },
    /// Set the split mode and the paths it diverts
    SetSplitApps {
        result_tx: oneshot::Sender<Result<(), Error>>,
        mode: SplitTunnelMode,
        paths: HashSet<PathBuf>,
    },
    /// Update VPN tunnel interface
    SetTunnel {
        result_tx: oneshot::Sender<Result<(), Error>>,
        vpn_interface: VpnInterface,
    },
    /// Remove VPN tunnel interface. It is sufficient to call this when entering the disconnected
    /// state, to avoid pointless cleanup during reconnects.
    ResetTunnel {
        result_tx: oneshot::Sender<Result<(), Error>>,
    },
}

/// Handle for interacting with the split tunnel module
#[derive(Clone)]
pub struct Handle {
    tx: mpsc::UnboundedSender<Message>,
}

impl Handle {
    /// Shut down split tunnel
    pub async fn shutdown(&self) {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.tx.send(Message::Shutdown { result_tx });
        if let Err(error) = result_rx.await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Split tunnel is already down")
            );
        }
    }

    /// Return name of split tunnel interface
    pub async fn interface(&self) -> Option<String> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.tx.send(Message::GetInterface { result_tx });
        result_rx.await.ok()?
    }

    /// Set the split mode and the apps it diverts
    pub async fn set_split_apps(&self, apps: SplitApps) -> Result<(), Error> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.tx.send(Message::SetSplitApps {
            result_tx,
            mode: apps.mode,
            paths: apps.apps.into_iter().map(PathBuf::from).collect(),
        });
        result_rx.await.map_err(|_| Error::unavailable())?
    }

    /// Set VPN tunnel interface
    pub async fn set_tunnel(&self, vpn_interface: VpnInterface) -> Result<(), Error> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.tx.send(Message::SetTunnel {
            result_tx,
            vpn_interface,
        });
        result_rx.await.map_err(|_| Error::unavailable())?
    }

    /// Forget the VPN tunnel interface. This destroys the split tunneling interface when it is
    /// active.
    pub async fn reset_tunnel(&self) -> Result<(), Error> {
        let (result_tx, result_rx) = oneshot::channel();
        let _ = self.tx.send(Message::ResetTunnel { result_tx });
        result_rx.await.map_err(|_| Error::unavailable())?
    }
}

impl SplitTunnel {
    /// Initialize split tunneling
    pub fn spawn(
        tunnel_tx: Weak<futures::channel::mpsc::UnboundedSender<TunnelCommand>>,
        route_manager: RouteManagerHandle,
    ) -> Handle {
        let (tx, rx) = mpsc::unbounded_channel();
        let split_tunnel = Self {
            state: State::NoExclusions { route_manager },
            tunnel_tx,
            rx,
            shutdown_tx: None,
        };

        tokio::spawn(Self::run(split_tunnel));

        Handle { tx }
    }

    async fn run(mut self) {
        loop {
            let process_monitor_stopped = async {
                match self.state.process_monitor() {
                    Some(process) => process.wait().await,
                    None => futures::future::pending().await,
                }
            };

            tokio::select! {
                // Handle process monitor being stopped
                result = process_monitor_stopped => {
                    self.handle_process_monitor_shutdown(result);
                }

                // Handle messages
                message = self.rx.recv() => {
                    let Some(message) = message else {
                        break
                    };
                    if !self.handle_message(message).await {
                        break;
                    }
                }
            }
        }

        self.shutdown().await;

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Handle process monitor unexpectedly stopping
    fn handle_process_monitor_shutdown(&mut self, result: Result<(), process::Error>) {
        let cause = match result {
            Ok(_) => ErrorStateCause::SplitTunnelError,
            Err(ref error) => ErrorStateCause::from(error),
        };
        match result {
            Ok(()) => log::error!("Process monitor stopped unexpectedly with no error"),
            Err(ref error) => {
                log::error!(
                    "{}",
                    error.display_chain_with_msg("Process monitor stopped unexpectedly")
                );
            }
        }

        // Enter the error state if split tunneling is active. Otherwise, we might make incorrect
        // decisions for new processes
        if self.state.active()
            && let Some(tunnel_tx) = self.tunnel_tx.upgrade()
        {
            let _ = tunnel_tx.unbounded_send(TunnelCommand::Block(cause));
        }

        self.state.fail(result.err().map(Error::from));
    }

    /// Handle an incoming message
    /// Return whether the actor should continue running
    async fn handle_message(&mut self, message: Message) -> bool {
        match message {
            Message::GetInterface { result_tx } => {
                let _ = result_tx.send(self.interface().map(str::to_owned));
            }
            Message::Shutdown { result_tx } => {
                self.shutdown_tx = Some(result_tx);
                return false;
            }
            Message::SetSplitApps {
                result_tx,
                mode,
                paths,
            } => {
                let _ = result_tx.send(self.state.set_split_apps(mode, paths).await);
            }
            Message::SetTunnel {
                result_tx,
                vpn_interface,
            } => {
                let _ = result_tx.send(self.state.set_tunnel(vpn_interface).await);
            }
            Message::ResetTunnel { result_tx } => {
                let _ = result_tx.send(self.state.reset_tunnel().await);
            }
        }
        true
    }

    /// Shut down split tunnel
    async fn shutdown(&mut self) {
        match self.state.fail(None) {
            State::ProcessMonitorOnly { mut process, .. } => {
                process.shutdown().await;
            }
            State::Active {
                mut process,
                tun_handle,
                ..
            } => {
                if let Err(error) = tun_handle.shutdown().await {
                    log::error!("Failed to stop split tunnel: {error}");
                }
                process.shutdown().await;
            }
            State::Failed { .. } | State::NoExclusions { .. } | State::StandBy { .. } => (),
        }
    }

    /// Return name of split tunnel interface
    fn interface(&self) -> Option<&str> {
        match &self.state {
            State::Active { tun_handle, .. } => Some(tun_handle.name()),
            _ => None,
        }
    }
}

enum State {
    /// The initial state: no paths have been provided
    NoExclusions { route_manager: RouteManagerHandle },
    /// There is an active VPN connection but no split tunnel utun
    StandBy {
        route_manager: RouteManagerHandle,
        vpn_interface: VpnInterface,
    },
    /// There is a process monitor (and paths) but no split tunnel utun yet
    ProcessMonitorOnly {
        route_manager: RouteManagerHandle,
        process: process::ProcessMonitorHandle,
    },
    /// There is a split tunnel utun as well as paths to exclude
    Active {
        route_manager: RouteManagerHandle,
        process: process::ProcessMonitorHandle,
        tun_handle: tun::SplitTunnelHandle,
        vpn_interface: VpnInterface,
    },
    /// State entered when anything at all fails. Users can force a transition out of this state
    /// by disabling/clearing the paths to use.
    Failed {
        route_manager: RouteManagerHandle,
        vpn_interface: Option<VpnInterface>,
        cause: Option<Error>,
    },
}

impl State {
    fn process_monitor(&mut self) -> Option<&mut process::ProcessMonitorHandle> {
        match self {
            State::ProcessMonitorOnly { process, .. } | State::Active { process, .. } => {
                Some(process)
            }
            State::NoExclusions { .. } | State::StandBy { .. } | State::Failed { .. } => None,
        }
    }

    const fn route_manager(&self) -> &RouteManagerHandle {
        match self {
            State::NoExclusions { route_manager, .. }
            | State::StandBy { route_manager, .. }
            | State::ProcessMonitorOnly { route_manager, .. }
            | State::Active { route_manager, .. }
            | State::Failed { route_manager, .. } => route_manager,
        }
    }

    const fn vpn_interface(&self) -> Option<&VpnInterface> {
        match self {
            State::Failed { vpn_interface, .. } => vpn_interface.as_ref(),
            State::Active { vpn_interface, .. } | State::StandBy { vpn_interface, .. } => {
                Some(vpn_interface)
            }
            State::NoExclusions { .. } | State::ProcessMonitorOnly { .. } => None,
        }
    }

    const fn fail_cause(&self) -> Option<&Error> {
        match self {
            State::Failed { cause, .. } => cause.as_ref(),
            _ => None,
        }
    }

    /// Take `self`, leaving a failed state in its place. The original value is returned
    /// `cause` optionally specifies a failure cause. Unless specified, the last known error will be
    /// used instead.
    fn fail(&mut self, cause: Option<Error>) -> Self {
        std::mem::replace(
            self,
            State::Failed {
                route_manager: self.route_manager().clone(),
                vpn_interface: self.vpn_interface().cloned(),
                cause: cause.or_else(|| self.fail_cause().cloned()),
            },
        )
    }

    /// Return whether split tunneling is currently engaged. That is, there's both a process monitor
    /// and a VPN tunnel present
    const fn active(&self) -> bool {
        matches!(self, State::Active { .. })
    }

    /// Set the split mode and its paths. When they engage the split tunnel (include-only, or
    /// paths to exclude), this initializes split tunneling if a tunnel device is also set.
    async fn set_split_apps(
        &mut self,
        mode: SplitTunnelMode,
        paths: HashSet<PathBuf>,
    ) -> Result<(), Error> {
        self.transition(move |self_| self_.set_split_apps_inner(mode, paths))
            .await
    }

    async fn set_split_apps_inner(
        mut self,
        mode: SplitTunnelMode,
        paths: HashSet<PathBuf>,
    ) -> Result<Self, ErrorWithTransition> {
        let engaged = engages_split_tunnel(mode, &paths);
        match self {
            // If there are currently no paths and no process monitor, initialize it
            State::NoExclusions { route_manager } if engaged => {
                log::debug!("Initializing process monitor");

                let process = process::ProcessMonitor::spawn().await?;
                process.states().set_split_apps(mode, paths);

                Ok(State::ProcessMonitorOnly {
                    route_manager,
                    process,
                })
            }
            // If there are currently no paths and no process monitor, initialize it
            State::StandBy {
                route_manager,
                vpn_interface,
            } if engaged => {
                log::debug!("Initializing process monitor");

                let process = process::ProcessMonitor::spawn().await?;
                process.states().set_split_apps(mode, paths);

                State::ProcessMonitorOnly {
                    route_manager,
                    process,
                }
                .set_tunnel_inner(vpn_interface)
                .await
            }
            // If nothing engages the split tunnel, do nothing
            State::NoExclusions { .. } | State::StandBy { .. } => Ok(self),
            // If nothing engages the split tunnel any more but it was enabled for an active VPN
            // connection, disable split tunneling while caching the VPN interface.
            //
            // Note that the point is to drop the split tunnel handle to clean up the split tunnel
            // interface from the user's system.
            State::Active {
                route_manager,
                mut process,
                tun_handle,
                vpn_interface,
            } if !engaged => {
                if let Err(error) = tun_handle.shutdown().await {
                    log::error!("Failed to stop split tunnel: {error}");
                }
                process.shutdown().await;
                Ok(State::StandBy {
                    route_manager,
                    vpn_interface,
                })
            }
            // If split tunneling is already initialized, or only the process monitor is, update the
            // paths only
            State::Active {
                ref mut process, ..
            }
            | State::ProcessMonitorOnly {
                ref mut process, ..
            } => {
                process.states().set_split_apps(mode, paths);
                Ok(self)
            }
            // If nothing engages the split tunnel, transition out of the failed state
            State::Failed {
                route_manager,
                vpn_interface,
                cause: _,
            } if !engaged => {
                log::debug!("Transitioning out of split tunnel error state");

                match vpn_interface {
                    Some(vpn_interface) => Ok(State::StandBy {
                        route_manager,
                        vpn_interface,
                    }),
                    None => Ok(State::NoExclusions { route_manager }),
                }
            }
            // Otherwise, remain in the failed state
            State::Failed { cause, .. } => Err(cause.unwrap_or(Error::unavailable()).into()),
        }
    }

    /// Forget the VPN tunnel interface. This destroys the split tunneling interface when it is
    /// active.
    pub async fn reset_tunnel(&mut self) -> Result<(), Error> {
        self.transition(|state| state.reset_tunnel_inner()).await
    }

    async fn reset_tunnel_inner(self) -> Result<Self, ErrorWithTransition> {
        match self {
            // If split tunneling is currently active, that means that there are paths to exclude,
            // so shut down the ST utun but keep the process monitor.
            State::Active {
                route_manager,
                process,
                tun_handle,
                vpn_interface: _,
            } => {
                if let Err(error) = tun_handle.shutdown().await {
                    log::error!("Failed to stop split tunnel: {error}");
                }
                Ok(State::ProcessMonitorOnly {
                    route_manager,
                    process,
                })
            }
            // If we're in standby mode, simply forget the VPN interface.
            State::StandBy {
                route_manager,
                vpn_interface: _,
            } => Ok(State::NoExclusions { route_manager }),
            // If we're in `Failed`, just forget the VPN interface.
            State::Failed {
                route_manager,
                vpn_interface: _,
                cause,
            } => Ok(State::Failed {
                route_manager,
                vpn_interface: None,
                cause,
            }),
            // For any other state, do nothing.
            _ => Ok(self),
        }
    }

    /// Update VPN tunnel interface that non-excluded packets are sent on
    async fn set_tunnel(&mut self, vpn_interface: VpnInterface) -> Result<(), Error> {
        self.transition(move |self_| self_.set_tunnel_inner(vpn_interface))
            .await
    }

    async fn set_tunnel_inner(
        mut self,
        vpn_interface: VpnInterface,
    ) -> Result<Self, ErrorWithTransition> {
        match self {
            // If split tunneling is already initialized, just update the interfaces
            State::Active {
                route_manager,
                mut process,
                tun_handle,
                vpn_interface: old_vpn_interface,
            } => {
                // Try to update the default interface first
                // If this fails, remain in the current state and just fail
                let default_interface = match default::get_default_interface(&route_manager).await {
                    Ok(default_interface) => default_interface,
                    Err(error) => {
                        return Err(ErrorWithTransition {
                            error: error.into(),
                            next_state: Some(State::Active {
                                route_manager,
                                process,
                                tun_handle,
                                vpn_interface: old_vpn_interface,
                            }),
                        });
                    }
                };

                log::debug!("Updating split tunnel device");

                match tun_handle
                    .set_interfaces(default_interface, Some(vpn_interface.clone()))
                    .await
                {
                    Ok(tun_handle) => Ok(State::Active {
                        route_manager,
                        process,
                        tun_handle,
                        vpn_interface,
                    }),
                    Err(error) => {
                        process.shutdown().await;
                        Err(error.into())
                    }
                }
            }
            // If there is a process monitor, initialize split tunneling
            State::ProcessMonitorOnly {
                route_manager,
                mut process,
            } => {
                // Try to update the default interface first
                // If this fails, remain in the current state and just fail
                let default_interface = match default::get_default_interface(&route_manager).await {
                    Ok(default_interface) => default_interface,
                    Err(error) => {
                        return Err(ErrorWithTransition {
                            error: error.into(),
                            next_state: Some(State::ProcessMonitorOnly {
                                route_manager,
                                process,
                            }),
                        });
                    }
                };

                log::debug!("Initializing split tunnel device");

                let states = process.states().clone();
                let result = tun::create_split_tunnel(
                    default_interface,
                    Some(vpn_interface.clone()),
                    route_manager.clone(),
                    Box::new(move |packet| {
                        let pid = attributed_pid(packet.header.pth_pid, packet.header.pth_epid);
                        let (mode, status) = states.lookup(pid);
                        routing_decision(mode, status)
                    }),
                )
                .await;

                match result {
                    Ok(tun_handle) => Ok(State::Active {
                        route_manager,
                        process,
                        tun_handle,
                        vpn_interface,
                    }),
                    Err(error) => {
                        process.shutdown().await;
                        Err(error.into())
                    }
                }
            }
            // Even if there are no paths to exclude, remember the new tunnel interface
            State::NoExclusions { route_manager } => Ok(State::StandBy {
                route_manager,
                vpn_interface,
            }),
            // If there are no paths to exclude, remain in the current state
            State::StandBy {
                vpn_interface: ref mut old_vpn_interface,
                ..
            } => {
                *old_vpn_interface = vpn_interface;
                Ok(self)
            }
            // Remain in the failed state and return error if VPN is up
            State::Failed {
                vpn_interface: ref mut old_vpn_interface,
                cause,
                ..
            } => {
                *old_vpn_interface = Some(vpn_interface);
                Err(cause.unwrap_or(Error::unavailable()).into())
            }
        }
    }

    /// Helper function that tries to perform a state transition using `transition`.
    /// On error, transition to `next_state` specified alongside the error. If not specified,
    /// transition to or remain in `State::Failed`.
    async fn transition<F: std::future::Future<Output = Result<Self, ErrorWithTransition>>>(
        &mut self,
        transition: impl FnOnce(Self) -> F,
    ) -> Result<(), Error> {
        let state = self.fail(None);
        match (transition)(state).await {
            Ok(new_state) => {
                *self = new_state;
                Ok(())
            }
            Err(ErrorWithTransition {
                error,
                next_state: Some(next_state),
            }) => {
                *self = next_state;
                Err(error)
            }
            Err(ErrorWithTransition {
                error,
                next_state: None,
            }) => {
                self.fail(Some(error.clone()));
                Err(error)
            }
        }
    }
}

/// Whether `mode` and `paths` need the split tunnel. Include-only needs it
/// even with no path: everything else must still leave outside the tunnel.
fn engages_split_tunnel(mode: SplitTunnelMode, paths: &HashSet<PathBuf>) -> bool {
    mode == SplitTunnelMode::IncludeOnly || !paths.is_empty()
}

/// The process a packet is attributed to. A delegated socket (WebKit's
/// network process working for Safari, for one) carries the process it acts
/// for as its effective pid, and that is the app the user chose.
fn attributed_pid(pid: libc::pid_t, effective_pid: libc::pid_t) -> libc::pid_t {
    if effective_pid != 0 {
        effective_pid
    } else {
        pid
    }
}

/// Where a packet of a process with `status` goes under `mode`. `Listed`
/// means the process belongs to one of the split tunnel's paths. A process
/// the monitor does not know yet is dropped in both modes: it may belong to
/// an included app, and it must not be tunneled if it is excluded.
fn routing_decision(mode: SplitTunnelMode, status: ExclusionStatus) -> tun::RoutingDecision {
    match (mode, status) {
        (_, ExclusionStatus::Unknown) => tun::RoutingDecision::Drop,
        (SplitTunnelMode::Exclude, ExclusionStatus::Listed)
        | (SplitTunnelMode::IncludeOnly, ExclusionStatus::Unlisted) => {
            tun::RoutingDecision::DefaultInterface
        }
        (SplitTunnelMode::Exclude, ExclusionStatus::Unlisted)
        | (SplitTunnelMode::IncludeOnly, ExclusionStatus::Listed) => {
            tun::RoutingDecision::VpnTunnel
        }
    }
}

struct ErrorWithTransition {
    error: Error,
    next_state: Option<State>,
}

impl<T: Into<Error>> From<T> for ErrorWithTransition {
    fn from(error: T) -> Self {
        Self {
            error: error.into(),
            next_state: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusion_sends_listed_processes_outside_and_the_rest_through_the_tunnel() {
        let listed = routing_decision(SplitTunnelMode::Exclude, ExclusionStatus::Listed);
        let unlisted = routing_decision(SplitTunnelMode::Exclude, ExclusionStatus::Unlisted);

        assert_eq!(listed, tun::RoutingDecision::DefaultInterface);
        assert_eq!(unlisted, tun::RoutingDecision::VpnTunnel);
    }

    #[test]
    fn include_only_tunnels_listed_processes_and_sends_the_rest_outside() {
        let listed = routing_decision(SplitTunnelMode::IncludeOnly, ExclusionStatus::Listed);
        let unlisted = routing_decision(SplitTunnelMode::IncludeOnly, ExclusionStatus::Unlisted);

        assert_eq!(listed, tun::RoutingDecision::VpnTunnel);
        assert_eq!(unlisted, tun::RoutingDecision::DefaultInterface);
    }

    #[test]
    fn an_unknown_process_is_dropped_in_every_mode() {
        for mode in [SplitTunnelMode::Exclude, SplitTunnelMode::IncludeOnly] {
            assert_eq!(
                routing_decision(mode, ExclusionStatus::Unknown),
                tun::RoutingDecision::Drop
            );
        }
    }

    #[test]
    fn a_delegated_packet_belongs_to_the_process_it_acts_for() {
        const WEBKIT_NETWORKING: libc::pid_t = 812;
        const SAFARI: libc::pid_t = 640;

        assert_eq!(attributed_pid(WEBKIT_NETWORKING, SAFARI), SAFARI);
        assert_eq!(attributed_pid(WEBKIT_NETWORKING, 0), WEBKIT_NETWORKING);
    }

    #[test]
    fn include_only_engages_the_split_tunnel_without_paths() {
        let no_paths = HashSet::new();
        let one_path = HashSet::from([PathBuf::from("/Applications/Firefox.app")]);

        assert!(engages_split_tunnel(
            SplitTunnelMode::IncludeOnly,
            &no_paths
        ));
        assert!(!engages_split_tunnel(SplitTunnelMode::Exclude, &no_paths));
        assert!(engages_split_tunnel(SplitTunnelMode::Exclude, &one_path));
    }
}
