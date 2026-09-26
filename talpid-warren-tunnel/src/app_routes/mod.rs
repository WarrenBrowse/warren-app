//! Per-app exits inside the Warren tunnel (`docs/app-routing.md`, sections 2.2
//! to 2.6).
//!
//! The daemon resolves each app's country to a circuit and hands the tunnel an
//! [`AppRoutesPlan`] on a watch channel. The tunnel runs one route session per
//! planned circuit next to the main session ([`controller`]), and puts the
//! router of `talpid-app-routing` between the TUN device and the sessions
//! ([`datapath`]). Route sessions are admitted against the main session's
//! anchor where the server offers it (warren-core doc 107), and on anonymous
//! tokens otherwise ([`session`]); never on the wallet. A route that is not
//! connected drops its apps' packets: they never go through the main session,
//! and never outside the tunnel.

use std::sync::Arc;

use talpid_app_routing::router::SessionAddresses;
use warrenguard_multihop::RouteKemPublicKey;
use warrenguard_transport::route_anchor::AnchorState;

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

/// Route sessions that may run next to the main one while routes are admitted
/// on tokens: the three session tokens of an epoch, minus the main session's.
pub const TOKEN_ROUTE_SESSIONS: usize = 2;

/// The most route sessions a tunnel runs, whatever the server admits: the
/// router names a route with one byte, and one more id holds the apps that
/// cannot be served.
pub const MAX_ROUTE_SESSIONS: usize = u8::MAX as usize;

/// How many route sessions may run next to the main one: what the main
/// session's anchor admits once the server bound it, and what the tokens
/// admit otherwise (no anchor, no verdict yet, or none to be had).
pub fn route_capacity(anchor: Option<AnchorState>) -> usize {
    match anchor {
        Some(AnchorState::Anchored { max_routes }) => {
            usize::from(max_routes).clamp(TOKEN_ROUTE_SESSIONS, MAX_ROUTE_SESSIONS)
        }
        Some(AnchorState::Unanchored | AnchorState::Unavailable) | None => TOKEN_ROUTE_SESSIONS,
    }
}

/// Route admission by anchor as the control plane announces it in its token
/// directory: the key a main session anchors with, and which exits admit
/// routes that way. Read when a tunnel starts (the key) and before each route
/// dial (the exits), so a directory refresh reaches the next dial.
pub trait RouteAdmissionSource: Send + Sync {
    /// The route KEM key, while the directory offers route admission.
    fn kem(&self) -> Option<RouteKemPublicKey>;

    /// Whether the exit with this multihop id admits routes by anchor.
    fn offers_routes(&self, exit_id: &[u8; 16]) -> bool;
}

/// One route session the daemon asks for: the circuit it dials, and the apps
/// (their app ids, as the router matches them) that leave through it.
#[derive(Clone)]
pub struct PlannedRoute {
    pub circuit: MultiHopConfig,
    pub apps: Vec<String>,
}

/// Apps whose exit the main connection already matches: the main session
/// carries them while it is on `exit_id`, and they are blocked while it is
/// not (it may have moved since the plan was made).
#[derive(Clone, PartialEq, Eq)]
pub struct MainRoute {
    pub exit_id: [u8; 16],
    pub apps: Vec<String>,
}

/// Every route session the daemon asks for, and the apps it blocks: apps whose
/// exit cannot be served (no relay, over the limit) never fall back to the
/// main session.
#[derive(Clone, Default)]
pub struct AppRoutesPlan {
    pub routes: Vec<PlannedRoute>,
    pub blocked_apps: Vec<String>,
    pub main_apps: Vec<MainRoute>,
}

impl AppRoutesPlan {
    /// Whether two plans ask for the same sessions and the same apps, so a
    /// republished plan does not touch the live sessions.
    pub fn same_as(&self, other: &Self) -> bool {
        self.blocked_apps == other.blocked_apps
            && self.main_apps == other.main_apps
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
    /// Every route the server admits right now is taken: this one has no
    /// session yet, and starts as soon as one is free.
    Waiting,
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
    /// Waiting for one of the routes the tokens admit at once to be free.
    Waiting,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_anchor_admits_what_the_server_said() {
        assert_eq!(
            route_capacity(Some(AnchorState::Anchored { max_routes: 32 })),
            32
        );
    }

    #[test]
    fn routes_run_on_tokens_until_an_anchor_is_bound() {
        for anchor in [
            None,
            Some(AnchorState::Unanchored),
            Some(AnchorState::Unavailable),
        ] {
            assert_eq!(route_capacity(anchor), TOKEN_ROUTE_SESSIONS, "{anchor:?}");
        }
    }

    #[test]
    fn an_anchor_never_admits_fewer_routes_than_the_tokens() {
        assert_eq!(
            route_capacity(Some(AnchorState::Anchored { max_routes: 1 })),
            TOKEN_ROUTE_SESSIONS
        );
    }

    #[test]
    fn the_router_ids_bound_what_an_anchor_admits() {
        assert_eq!(
            route_capacity(Some(AnchorState::Anchored { max_routes: 4096 })),
            MAX_ROUTE_SESSIONS
        );
    }
}
