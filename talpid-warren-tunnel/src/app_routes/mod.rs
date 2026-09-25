//! Per-app exits inside the Warren tunnel (`docs/app-routing.md`, sections 2.2
//! to 2.6).
//!
//! The daemon resolves each app's country to a circuit and hands the tunnel an
//! [`AppRoutesPlan`] on a watch channel. The tunnel runs one route session per
//! planned circuit next to the main session ([`controller`]), and puts the
//! router of `talpid-app-routing` between the TUN device and the sessions
//! ([`datapath`]). Route sessions are admitted on anonymous tokens only
//! ([`session`]), and a route that is not connected drops its apps' packets:
//! they never go through the main session, and never outside the tunnel.

use std::sync::Arc;

use talpid_app_routing::router::SessionAddresses;

use crate::MultiHopConfig;

pub mod controller;
pub mod datapath;
pub mod session;

#[cfg(test)]
mod test_support;

pub use controller::{RelaySink, RouteController, RouteSessions, SessionEvents};
pub use datapath::{RouteTun, RoutedTun, RoutingTable};
pub use session::{RouteSessionConfig, SupervisorRouteSessions};

/// Which process owns a socket, from the host's own tables.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub type HostResolver = talpid_app_routing::owner::SystemResolver;

/// No owner lookup on this platform: its router never routes an app, so
/// this is never asked.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[derive(Default)]
pub struct HostResolver;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl HostResolver {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl talpid_app_routing::owner::OwnerResolver for HostResolver {
    fn socket_owner(&mut self, _flow: &talpid_app_routing::flow::FlowKey) -> Option<u32> {
        None
    }

    fn refresh(&mut self) -> Result<(), talpid_app_routing::owner::OwnerError> {
        Ok(())
    }

    fn process_key(&mut self, _pid: u32) -> Option<talpid_app_routing::app::ProcessKey> {
        None
    }

    fn executable(&mut self, _pid: u32) -> Option<std::path::PathBuf> {
        None
    }
}

/// Route sessions that may run next to the main one: the three session
/// tokens of an epoch, minus the main session's. Enforced here whatever the
/// plan asks, since a settings file edited by hand may ask for more.
pub const MAX_ROUTE_SESSIONS: usize = 2;

/// One route session the daemon asks for: the circuit it dials, and the apps
/// (their app ids, as the router matches them) that leave through it.
#[derive(Clone)]
pub struct PlannedRoute {
    pub circuit: MultiHopConfig,
    pub apps: Vec<String>,
}

/// Every route session the daemon asks for, and the apps it blocks: apps whose
/// exit cannot be served (no relay, over the limit) never fall back to the
/// main session.
#[derive(Clone, Default)]
pub struct AppRoutesPlan {
    pub routes: Vec<PlannedRoute>,
    pub blocked_apps: Vec<String>,
}

impl AppRoutesPlan {
    /// Whether two plans ask for the same sessions and the same apps, so a
    /// republished plan does not touch the live sessions.
    pub fn same_as(&self, other: &Self) -> bool {
        self.blocked_apps == other.blocked_apps
            && self.routes.len() == other.routes.len()
            && self.routes.iter().zip(&other.routes).all(|(a, b)| {
                a.apps == b.apps && circuit_identity(&a.circuit) == circuit_identity(&b.circuit)
            })
    }
}

// App ids are paths and circuits name relays: render counts only.
impl std::fmt::Debug for AppRoutesPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppRoutesPlan")
            .field("routes", &self.routes.len())
            .field("blocked_apps", &self.blocked_apps.len())
            .finish()
    }
}

/// The entry relay and the exit of a circuit, which together are a route
/// session's identity: a change of either is another session.
pub(crate) fn circuit_identity(circuit: &MultiHopConfig) -> ([u8; 16], [u8; 16]) {
    (circuit.relay.relay_id, *circuit.exit.exit_id.as_bytes())
}

/// Why a route session cannot run. Its apps are blocked meanwhile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteUnavailable {
    /// No anonymous token is left for this epoch.
    NoToken,
    /// The exit refused the session: every token was refused (the usual cause
    /// is that each serial already holds a live session), or the account is
    /// refused.
    Refused,
    /// The session failed for a reason a retry will not fix by itself.
    Failed,
}

/// Where a route session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSessionState {
    Connecting,
    Connected,
    Unavailable(RouteUnavailable),
}

/// The state of the route session of one exit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RouteReport {
    pub exit_id: [u8; 16],
    pub state: RouteSessionState,
}

// The exit a route leaves through is exit identity.
impl std::fmt::Debug for RouteReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteReport")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// Receives the state of every route session whenever one of them changes.
pub type AppRouteObserver = Arc<dyn Fn(Vec<RouteReport>) + Send + Sync>;

/// What a route session says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Connecting,
    /// Up, sending from these inner addresses.
    Connected(SessionAddresses),
    Unavailable(RouteUnavailable),
}
