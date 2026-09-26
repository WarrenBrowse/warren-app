//! The per-packet decision: main session, a route session, or dropped.
//!
//! Uplink, each new flow is attributed to the process owning its socket, and
//! from there to the route of its app; the flow table remembers the answer so
//! a known flow costs a hash lookup and, when routed, an in-place source
//! rewrite. Downlink, a packet from a route session is delivered only when it
//! belongs to a flow that route carries, with its destination translated back.
//!
//! Failure rules, from `docs/app-routing.md`:
//! - a new flow is attributed only from a socket snapshot taken after its
//!   packet arrived, never from an older one that a reused port could fool;
//! - a flow whose owner cannot be found even then goes through the main
//!   session, except a TCP segment of a connection already under way, which
//!   is dropped: that connection may have been routed;
//! - a flow of a routed app is dropped while its route is not connected, and
//!   never falls back to the main session;
//! - a later fragment whose first fragment was not seen is dropped, since its
//!   datagram may be a routed app's.

use std::{
    collections::HashMap,
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};

use crate::{
    app::{AppMatcher, DecisionCache, PathFlavor},
    flow::{self, Classified, Direction, FlowKey, FlowTable, FragmentKey, Transport},
    ip, nat,
    owner::{OwnerResolver, SocketOwner},
};

/// Flows tracked at most, once routing is active.
pub const DEFAULT_FLOW_CAPACITY: usize = 32_768;
const PROCESS_CAPACITY: usize = 4096;
const FRAGMENT_CAPACITY: usize = 256;
const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(10);
const SWEEP_INTERVAL: Duration = Duration::from_secs(5);

/// A route session, by its index in the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteId(pub u8);

/// The inner addresses a session sends from.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionAddresses {
    pub v4: Option<Ipv4Addr>,
    pub v6: Option<Ipv6Addr>,
}

impl SessionAddresses {
    fn for_family_of(&self, ip: IpAddr) -> Option<IpAddr> {
        match ip {
            IpAddr::V4(_) => self.v4.map(IpAddr::V4),
            IpAddr::V6(_) => self.v6.map(IpAddr::V6),
        }
    }
}

/// Where a route session stands. Only a connected route carries packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteState {
    Connecting,
    Connected(SessionAddresses),
    Unavailable,
}

// A session's inner addresses tie traffic to an exit: render which families
// are set, never the addresses.
impl std::fmt::Debug for SessionAddresses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAddresses")
            .field("v4", &self.v4.is_some())
            .field("v6", &self.v6.is_some())
            .finish()
    }
}

/// Why a policy is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PolicyError {
    #[error("an app is assigned a route the policy does not have")]
    UnknownRoute,
}

/// Which app goes through which route, and the state of each route.
pub struct Policy {
    main: SessionAddresses,
    apps: AppMatcher<RouteId>,
    routes: Vec<RouteState>,
}

impl Policy {
    /// `apps` maps app ids to routes; `main` are the main session's inner
    /// addresses, the only sources a routed packet may carry.
    ///
    /// # Errors
    ///
    /// [`PolicyError::UnknownRoute`] when an app names a route past `routes`.
    pub fn new<P: AsRef<OsStr>>(
        main: SessionAddresses,
        apps: impl IntoIterator<Item = (P, RouteId)>,
        routes: Vec<RouteState>,
    ) -> Result<Self, PolicyError> {
        let apps: Vec<(P, RouteId)> = apps.into_iter().collect();
        if apps
            .iter()
            .any(|(_, route)| usize::from(route.0) >= routes.len())
        {
            return Err(PolicyError::UnknownRoute);
        }
        Ok(Self {
            main,
            apps: AppMatcher::new(PathFlavor::HOST, apps),
            routes,
        })
    }

    /// No app routed: every packet goes through the main session.
    pub fn inactive() -> Self {
        Self {
            main: SessionAddresses::default(),
            apps: AppMatcher::new(PathFlavor::HOST, Vec::<(&str, RouteId)>::new()),
            routes: Vec::new(),
        }
    }

    fn is_active(&self) -> bool {
        !self.apps.is_empty()
    }
}

/// What to do with an uplink packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Through the main session, unchanged.
    Main,
    /// Through this route session; its source has been rewritten.
    Route(RouteId),
    Drop,
}

/// What to do with a packet a route session delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// To the TUN device; its destination has been rewritten.
    Deliver,
    Drop,
}

/// What the router has done, in numbers only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counters {
    pub new_flows: u64,
    pub unresolved_flows: u64,
    pub refreshes: u64,
    pub refresh_errors: u64,
    pub routed_packets: u64,
    pub dropped_packets: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Binding {
    Main,
    Route(RouteId),
    /// Neither main nor a route may carry it.
    Blocked,
}

const FLAG_SYN: u8 = 0x02;
const FLAG_ACK: u8 = 0x10;

/// The first fragments seen in one direction, so the later ones follow them.
struct FragmentMap {
    entries: HashMap<FragmentKey, (Binding, Instant)>,
}

impl FragmentMap {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Records the binding of a datagram. A full map drops its expired
    /// entries, then its oldest one, so a burst of fragments costs the
    /// oldest datagram rather than every datagram in flight.
    fn remember(&mut self, fragment: FragmentKey, binding: Binding, now: Instant) {
        if self.entries.len() >= FRAGMENT_CAPACITY && !self.entries.contains_key(&fragment) {
            self.expire(now);
            if self.entries.len() >= FRAGMENT_CAPACITY
                && let Some(oldest) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, (_, seen))| *seen)
                    .map(|(key, _)| *key)
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(fragment, (binding, now));
    }

    fn get(&self, fragment: &FragmentKey, now: Instant) -> Option<Binding> {
        self.entries
            .get(fragment)
            .filter(|(_, seen)| now.saturating_duration_since(*seen) <= FRAGMENT_TIMEOUT)
            .map(|(binding, _)| *binding)
    }

    fn expire(&mut self, now: Instant) {
        self.entries
            .retain(|_, (_, seen)| now.saturating_duration_since(*seen) <= FRAGMENT_TIMEOUT);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Routes packets for one policy at a time. Not thread-safe by design: it
/// lives on the packet path's own task.
pub struct Router<R> {
    resolver: R,
    policy: Policy,
    flows: Option<FlowTable<Binding>>,
    flow_capacity: usize,
    uplink_fragments: FragmentMap,
    downlink_fragments: FragmentMap,
    decisions: DecisionCache<RouteId>,
    /// When the resolver's view was last read from the OS.
    snapshot_taken: Option<Instant>,
    last_sweep: Option<Instant>,
    counters: Counters,
}

impl<R: OwnerResolver> Router<R> {
    /// An inactive router over `resolver`.
    pub fn new(resolver: R) -> Self {
        Self::with_capacity(resolver, DEFAULT_FLOW_CAPACITY)
    }

    /// As [`Self::new`], tracking at most `flow_capacity` flows.
    pub fn with_capacity(resolver: R, flow_capacity: usize) -> Self {
        Self {
            resolver,
            policy: Policy::inactive(),
            flows: None,
            flow_capacity,
            uplink_fragments: FragmentMap::new(),
            downlink_fragments: FragmentMap::new(),
            decisions: DecisionCache::new(PROCESS_CAPACITY),
            snapshot_taken: None,
            last_sweep: None,
            counters: Counters::default(),
        }
    }

    /// Replaces the policy. Every flow is attributed again, since an app may
    /// have changed route.
    pub fn set_policy(&mut self, policy: Policy) {
        if policy.is_active() && self.flows.is_none() {
            // Allocated on first use: a router that never routes costs no
            // table.
            self.flows = Some(FlowTable::new(self.flow_capacity));
            self.uplink_fragments.entries.reserve(FRAGMENT_CAPACITY);
            self.downlink_fragments.entries.reserve(FRAGMENT_CAPACITY);
        }
        if let Some(flows) = &mut self.flows {
            flows.clear();
        }
        self.uplink_fragments.clear();
        self.downlink_fragments.clear();
        self.decisions.clear();
        self.resolver.watch_programs(&policy.apps);
        // The resolver's view was taken for the old programs.
        self.snapshot_taken = None;
        self.policy = policy;
    }

    /// Updates one route's state, keeping the flows it carries.
    pub fn set_route_state(&mut self, route: RouteId, state: RouteState) {
        if let Some(slot) = self.policy.routes.get_mut(usize::from(route.0)) {
            *slot = state;
        }
    }

    /// Decides for an uplink packet that arrived at `arrived`, rewriting its
    /// source when routed. `arrived` is when the packet was read from the
    /// TUN device: a new flow is only attributed from a view of the OS taken
    /// after that.
    pub fn uplink(&mut self, packet: &mut [u8], arrived: Instant) -> Verdict {
        if !self.policy.is_active() {
            return Verdict::Main;
        }
        self.sweep(arrived);
        let Ok(classified) = flow::classify(packet, Direction::Uplink) else {
            return Verdict::Main;
        };
        let binding = match classified {
            Classified::Flow {
                key,
                tcp_flags,
                fragment,
            } => {
                let binding = self.flow_binding(key, tcp_flags, arrived);
                if let Some(fragment) = fragment {
                    self.uplink_fragments.remember(fragment, binding, arrived);
                }
                binding
            }
            Classified::LaterFragment(fragment) => self
                .uplink_fragments
                .get(&fragment, arrived)
                .unwrap_or(Binding::Blocked),
            Classified::IcmpError { key } => {
                // The host's own error about a routed flow would need its
                // quoted packet translated too; losing it costs the remote
                // end a timeout, sending it through main would reveal it.
                return match self
                    .flows
                    .as_ref()
                    .and_then(|flows| flows.peek(&key, arrived))
                {
                    Some(Binding::Route(_) | Binding::Blocked) => self.dropped(),
                    _ => Verdict::Main,
                };
            }
            Classified::Other => Binding::Main,
        };
        self.apply_uplink(binding, packet)
    }

    /// Decides for a packet that route session `route` delivered, rewriting
    /// its destination back when it belongs to a flow of that route.
    pub fn downlink(&mut self, route: RouteId, packet: &mut [u8], now: Instant) -> Delivery {
        if !self.policy.is_active() {
            return Delivery::Drop;
        }
        let Some(RouteState::Connected(addresses)) =
            self.policy.routes.get(usize::from(route.0)).copied()
        else {
            return self.not_delivered();
        };
        let Ok(classified) = flow::classify(packet, Direction::Downlink) else {
            return self.not_delivered();
        };
        let ours = Some(Binding::Route(route));
        let main = match classified {
            Classified::Flow {
                key,
                tcp_flags,
                fragment,
            } => {
                let Some((original, main)) = self.original_flow(key, addresses) else {
                    return self.not_delivered();
                };
                // Only a flow this route carries may have its state moved by
                // what the route delivers.
                if self.peek(&original, now) != ours {
                    return self.not_delivered();
                }
                self.lookup(&original, Direction::Downlink, tcp_flags, now);
                if let Some(fragment) = fragment {
                    self.downlink_fragments
                        .remember(fragment, Binding::Route(route), now);
                }
                main
            }
            Classified::IcmpError { key } => {
                let Some((original, main)) = self.original_flow(key, addresses) else {
                    return self.not_delivered();
                };
                if self.peek(&original, now) != ours {
                    return self.not_delivered();
                }
                main
            }
            Classified::LaterFragment(fragment) => {
                let Some(main) = self.policy.main.for_family_of(fragment.dst) else {
                    return self.not_delivered();
                };
                if self.downlink_fragments.get(&fragment, now) != ours {
                    return self.not_delivered();
                }
                main
            }
            Classified::Other => return self.not_delivered(),
        };
        if nat::rewrite_destination(packet, main).is_err() {
            return self.not_delivered();
        }
        self.counters.routed_packets += 1;
        Delivery::Deliver
    }

    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// The flow as the host knows it, for a packet a route delivered to its
    /// own address, and the main address to restore.
    fn original_flow(&self, key: FlowKey, route: SessionAddresses) -> Option<(FlowKey, IpAddr)> {
        let local = key.local.ip();
        if route.for_family_of(local) != Some(local) {
            return None;
        }
        let main = self.policy.main.for_family_of(local)?;
        let original = FlowKey {
            local: std::net::SocketAddr::new(main, key.local.port()),
            ..key
        };
        Some((original, main))
    }

    fn flow_binding(&mut self, key: FlowKey, tcp_flags: u8, arrived: Instant) -> Binding {
        // A connection opening on a 5-tuple that is still known is a new
        // connection, possibly from another program: attribute it again.
        let opening = is_opening(key, tcp_flags);
        if !opening && let Some(binding) = self.lookup(&key, Direction::Uplink, tcp_flags, arrived)
        {
            return binding;
        }
        let binding = self.attribute(&key, opening, arrived);
        if let Some(flows) = &mut self.flows {
            flows.insert(key, binding, Direction::Uplink, tcp_flags, arrived);
        }
        binding
    }

    fn lookup(
        &mut self,
        key: &FlowKey,
        direction: Direction,
        tcp_flags: u8,
        now: Instant,
    ) -> Option<Binding> {
        self.flows.as_mut()?.lookup(key, direction, tcp_flags, now)
    }

    fn peek(&self, key: &FlowKey, now: Instant) -> Option<Binding> {
        self.flows.as_ref()?.peek(key, now)
    }

    /// Finds the app behind a new flow: its socket's owner in a view of the
    /// OS taken after the packet arrived, then that process's decision,
    /// looked up once per process.
    fn attribute(&mut self, key: &FlowKey, opening: bool, arrived: Instant) -> Binding {
        self.counters.new_flows += 1;
        // No OS table lists ICMP sockets: an echo is documented to go through
        // main, and asking would only cost a snapshot.
        if key.transport == Transport::IcmpEcho {
            return Binding::Main;
        }
        self.ensure_fresh_view(arrived);
        // A connection under way may already be routed, and seeing its
        // remaining segments through main would tie its two exits together:
        // only a holder found among every process lets it through main. A
        // new flow may be sent there on the narrower search, as an owner
        // never found would be.
        let under_way = key.transport == Transport::Tcp && !opening;
        let owner = if under_way {
            self.resolver
                .socket_owner(key)
                .map_or(SocketOwner::Unknown, SocketOwner::Process)
        } else {
            self.resolver.owner(key)
        };
        let pid = match owner {
            SocketOwner::Process(pid) => Some(pid),
            // A live socket that no process of a routed program holds.
            SocketOwner::Unwatched => return Binding::Main,
            SocketOwner::Unknown => None,
        };
        let Some((pid, process)) = pid.and_then(|pid| Some((pid, self.resolver.process_key(pid)?)))
        else {
            self.counters.unresolved_flows += 1;
            // A TCP segment that does not open a connection belongs to one
            // under way, and an ownerless socket is one closing: its
            // connection may have been routed, so its last segments are
            // better lost than seen on main.
            return if key.transport == Transport::Tcp && !opening {
                Binding::Blocked
            } else {
                Binding::Main
            };
        };
        let route = match self.decisions.get(process) {
            Some(route) => route,
            None => match self.resolver.executable(pid) {
                Some(path) => {
                    let route = self.policy.apps.lookup(path.as_os_str());
                    self.decisions.insert(process, route);
                    route
                }
                // The process is gone, or going: not a program known to take
                // no route, so nothing is remembered about it.
                None if under_way => return Binding::Blocked,
                None => None,
            },
        };
        route.map_or(Binding::Main, Binding::Route)
    }

    /// Reads the OS again unless the current view was taken after `arrived`.
    fn ensure_fresh_view(&mut self, arrived: Instant) {
        if self.snapshot_taken.is_some_and(|taken| taken >= arrived) {
            return;
        }
        // The view covers what happened before the read began, not what
        // happened while it ran.
        let started = Instant::now();
        self.counters.refreshes += 1;
        if self.resolver.refresh().is_err() {
            self.counters.refresh_errors += 1;
        }
        self.snapshot_taken = Some(started);
    }

    fn apply_uplink(&mut self, binding: Binding, packet: &mut [u8]) -> Verdict {
        let route = match binding {
            Binding::Main => return Verdict::Main,
            Binding::Blocked => return self.dropped(),
            Binding::Route(route) => route,
        };
        let Some(RouteState::Connected(addresses)) =
            self.policy.routes.get(usize::from(route.0)).copied()
        else {
            return self.dropped();
        };
        let Ok(layout) = ip::locate(packet) else {
            return self.dropped();
        };
        let src = layout.src(packet);
        // Only the main address can be restored on the way back.
        let from_main = self.policy.main.for_family_of(src) == Some(src);
        let translated = from_main
            && addresses
                .for_family_of(src)
                .is_some_and(|new| nat::rewrite_source(packet, new).is_ok());
        if !translated {
            return self.dropped();
        }
        self.counters.routed_packets += 1;
        Verdict::Route(route)
    }

    fn dropped(&mut self) -> Verdict {
        self.counters.dropped_packets += 1;
        Verdict::Drop
    }

    fn not_delivered(&mut self) -> Delivery {
        self.counters.dropped_packets += 1;
        Delivery::Drop
    }

    fn sweep(&mut self, now: Instant) {
        if self
            .last_sweep
            .is_some_and(|last| now.saturating_duration_since(last) < SWEEP_INTERVAL)
        {
            return;
        }
        self.last_sweep = Some(now);
        if let Some(flows) = &mut self.flows {
            flows.expire(now);
        }
        self.uplink_fragments.expire(now);
        self.downlink_fragments.expire(now);
    }
}

fn is_opening(key: FlowKey, tcp_flags: u8) -> bool {
    key.transport == Transport::Tcp && tcp_flags & FLAG_SYN != 0 && tcp_flags & FLAG_ACK == 0
}

#[cfg(test)]
pub(crate) mod fake {
    //! A resolver over a pretend OS, the system boundary of the router.

    use std::{collections::HashMap, path::PathBuf};

    use crate::{
        app::{AppMatcher, ProcessKey},
        flow::FlowKey,
        owner::{OwnerError, OwnerResolver, SocketOwner},
    };

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Calls {
        pub socket_owner: u32,
        pub refresh: u32,
        pub process_key: u32,
        pub executable: u32,
    }

    #[derive(Default)]
    pub struct FakeOs {
        /// What the OS has now.
        pub sockets: HashMap<FlowKey, u32>,
        pub processes: HashMap<u32, (u64, PathBuf)>,
        /// What the last refresh saw.
        snapshot: HashMap<FlowKey, u32>,
        pub calls: Calls,
        pub fail_refresh: bool,
        /// How long reading the OS takes.
        pub refresh_takes: std::time::Duration,
        /// Whether this OS searches only the watched programs' processes, as
        /// Linux does, and says so for a socket another process holds.
        pub narrows: bool,
        /// The programs the router last named.
        pub watched: Option<AppMatcher<()>>,
        /// Processes of a watched program the narrowed search misses, as
        /// one whose start event was lost would be.
        pub missed: std::collections::HashSet<u32>,
        /// Processes whose program cannot be read, as one that exits between
        /// two reads.
        pub unreadable: std::collections::HashSet<u32>,
    }

    impl FakeOs {
        pub fn process(&mut self, pid: u32, start_time: u64, path: &str) {
            self.processes
                .insert(pid, (start_time, PathBuf::from(path)));
        }

        pub fn socket(&mut self, flow: FlowKey, pid: u32) {
            self.sockets.insert(flow, pid);
        }
    }

    impl FakeOs {
        fn is_watched(&self, pid: u32) -> bool {
            let program = self.processes.get(&pid).map(|(_, path)| path);
            match (&self.watched, program) {
                (Some(watched), Some(program)) => watched.lookup(program.as_os_str()).is_some(),
                _ => false,
            }
        }
    }

    impl OwnerResolver for FakeOs {
        fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
            self.calls.socket_owner += 1;
            self.snapshot.get(flow).copied()
        }

        fn owner(&mut self, flow: &FlowKey) -> SocketOwner {
            self.calls.socket_owner += 1;
            match self.snapshot.get(flow).copied() {
                Some(pid)
                    if self.narrows && (!self.is_watched(pid) || self.missed.contains(&pid)) =>
                {
                    SocketOwner::Unwatched
                }
                Some(pid) => SocketOwner::Process(pid),
                None => SocketOwner::Unknown,
            }
        }

        fn watch_programs<V: Copy>(&mut self, programs: &AppMatcher<V>) {
            self.watched = Some(programs.apps_only());
        }

        fn refresh(&mut self) -> Result<(), OwnerError> {
            self.calls.refresh += 1;
            std::thread::sleep(self.refresh_takes);
            if self.fail_refresh {
                self.snapshot.clear();
                return Err(OwnerError::UnknownLayout);
            }
            self.snapshot = self.sockets.clone();
            Ok(())
        }

        fn process_key(&mut self, pid: u32) -> Option<ProcessKey> {
            self.calls.process_key += 1;
            self.processes.get(&pid).map(|(start_time, _)| ProcessKey {
                pid,
                start_time: *start_time,
                image: 0,
            })
        }

        fn executable(&mut self, pid: u32) -> Option<PathBuf> {
            self.calls.executable += 1;
            if self.unreadable.contains(&pid) {
                return None;
            }
            self.processes.get(&pid).map(|(_, path)| path.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fake::FakeOs, *};
    use crate::{flow::Transport, testutil::*};
    use std::net::SocketAddr;

    const MAIN: [u8; 4] = [10, 64, 0, 2];
    const ROUTE0: [u8; 4] = [10, 99, 0, 7];
    const ROUTE1: [u8; 4] = [10, 98, 0, 9];
    const REMOTE: [u8; 4] = [198, 51, 100, 9];
    const MAIN6: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
    const ROUTE6: [u8; 16] = [0xfd, 0x99, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7];
    const REMOTE6: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];

    const BROWSER: &str = "/opt/apps/browser";
    const MAILER: &str = "/opt/apps/mailer";
    const OTHER: &str = "/opt/apps/other";

    fn v4(octets: [u8; 4]) -> Option<Ipv4Addr> {
        Some(Ipv4Addr::from(octets))
    }

    fn main_addresses() -> SessionAddresses {
        SessionAddresses {
            v4: v4(MAIN),
            v6: Some(Ipv6Addr::from(MAIN6)),
        }
    }

    fn connected(octets: [u8; 4]) -> RouteState {
        RouteState::Connected(SessionAddresses {
            v4: v4(octets),
            v6: None,
        })
    }

    /// The browser through route 0, the mailer through route 1.
    fn policy(route0: RouteState, route1: RouteState) -> Policy {
        Policy::new(
            main_addresses(),
            [(BROWSER, RouteId(0)), (MAILER, RouteId(1))],
            vec![route0, route1],
        )
        .unwrap()
    }

    fn tcp_flow(local_port: u16) -> FlowKey {
        FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new(IpAddr::from(MAIN), local_port),
            remote: SocketAddr::new(IpAddr::from(REMOTE), 443),
        }
    }

    fn udp_flow(local_port: u16) -> FlowKey {
        FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new(IpAddr::from(MAIN), local_port),
            remote: SocketAddr::new(IpAddr::from(REMOTE), 443),
        }
    }

    /// A pretend OS where the browser (pid 10), the mailer (pid 20) and
    /// another app (pid 30) run.
    fn os() -> FakeOs {
        let mut os = FakeOs::default();
        os.process(10, 1000, BROWSER);
        os.process(20, 2000, MAILER);
        os.process(30, 3000, OTHER);
        os
    }

    fn router(os: FakeOs) -> Router<FakeOs> {
        let mut router = Router::with_capacity(os, 64);
        router.set_policy(policy(connected(ROUTE0), connected(ROUTE1)));
        router
    }

    fn syn(local_port: u16) -> Vec<u8> {
        tcp_v4(MAIN, REMOTE, local_port, 443, TCP_SYN, b"")
    }

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn an_inactive_router_sends_everything_to_main_without_asking_the_os() {
        let mut router = Router::new(os());
        let mut packet = syn(50000);
        let original = packet.clone();

        let verdict = router.uplink(&mut packet, now());

        assert_eq!(verdict, Verdict::Main);
        assert_eq!(packet, original);
        assert_eq!(router.resolver.calls, fake::Calls::default());
    }

    #[test]
    fn routes_a_chosen_apps_flow_and_rewrites_its_source() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let mut packet = syn(50000);

        let verdict = router.uplink(&mut packet, now());

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
        assert_eq!(packet, tcp_v4(ROUTE0, REMOTE, 50000, 443, TCP_SYN, b""));
    }

    #[test]
    fn each_app_takes_its_own_route() {
        let mut os = os();
        os.socket(udp_flow(50001), 20);
        let mut router = router(os);
        let mut packet = udp_v4(MAIN, REMOTE, 50001, 443, b"quic");

        let verdict = router.uplink(&mut packet, now());

        assert_eq!(verdict, Verdict::Route(RouteId(1)));
        assert_eq!(packet, udp_v4(ROUTE1, REMOTE, 50001, 443, b"quic"));
    }

    #[test]
    fn an_app_without_a_route_stays_on_main_untouched() {
        let mut os = os();
        os.socket(tcp_flow(50000), 30);
        let mut router = router(os);
        let mut packet = syn(50000);
        let original = packet.clone();

        let verdict = router.uplink(&mut packet, now());

        assert_eq!(verdict, Verdict::Main);
        assert_eq!(packet, original);
    }

    #[test]
    fn a_known_flow_never_asks_the_os_again() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        let after_first = router.resolver.calls;

        for _ in 0..100 {
            let mut packet = tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"data");
            assert_eq!(
                router.uplink(&mut packet, start),
                Verdict::Route(RouteId(0))
            );
        }

        assert_eq!(router.resolver.calls, after_first);
    }

    #[test]
    fn the_router_names_the_programs_it_routes_to_the_resolver() {
        let router = router(os());

        let watched = router.resolver.watched.as_ref().unwrap();

        assert!(watched.lookup(OsStr::new(BROWSER)).is_some());
        assert!(watched.lookup(OsStr::new(MAILER)).is_some());
        assert!(watched.lookup(OsStr::new(OTHER)).is_none());
    }

    #[test]
    fn a_flow_an_unwatched_process_holds_goes_to_main_without_asking_for_its_program() {
        let mut os = os();
        os.narrows = true;
        os.socket(tcp_flow(50000), 30);
        let mut router = router(os);

        let verdict = router.uplink(&mut syn(50000), now());

        assert_eq!(verdict, Verdict::Main);
        assert_eq!(router.counters().unresolved_flows, 0);
        assert_eq!(
            (
                router.resolver.calls.process_key,
                router.resolver.calls.executable
            ),
            (0, 0)
        );
    }

    #[test]
    fn a_connection_under_way_that_an_unwatched_process_holds_stays_on_main() {
        let mut os = os();
        os.narrows = true;
        os.socket(tcp_flow(50000), 30);
        let mut router = router(os);

        let verdict = router.uplink(
            &mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"data"),
            now(),
        );

        assert_eq!(verdict, Verdict::Main);
    }

    #[test]
    fn a_connection_under_way_is_searched_for_among_every_process() {
        let mut os = os();
        os.narrows = true;
        os.missed.insert(10);
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);

        let verdict = router.uplink(
            &mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"data"),
            now(),
        );

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
    }

    #[test]
    fn a_connection_under_way_whose_program_cannot_be_read_is_dropped() {
        let mut os = os();
        os.unreadable.insert(10);
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);

        let verdict = router.uplink(
            &mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"data"),
            now(),
        );

        assert_eq!(verdict, Verdict::Drop);
    }

    #[test]
    fn a_program_that_could_not_be_read_is_read_again_for_the_next_flow() {
        let mut os = os();
        os.unreadable.insert(10);
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 10);
        let mut router = router(os);
        let first = router.uplink(&mut syn(50000), now());

        router.resolver.unreadable.clear();
        let second = router.uplink(&mut syn(50001), now());

        assert_eq!(first, Verdict::Main);
        assert_eq!(second, Verdict::Route(RouteId(0)));
    }

    #[test]
    fn a_flow_a_watched_process_holds_takes_its_route_when_the_os_narrows() {
        let mut os = os();
        os.narrows = true;
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);

        let verdict = router.uplink(&mut syn(50000), now());

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
    }

    #[test]
    fn a_new_policy_is_never_answered_from_a_view_taken_before_it() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 20);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);

        router.set_policy(policy(connected(ROUTE0), connected(ROUTE1)));
        router.uplink(&mut syn(50001), start);

        assert_eq!(router.resolver.calls.refresh, 2);
    }

    #[test]
    fn a_flow_that_arrived_while_the_os_was_being_read_gets_a_view_of_its_own() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 20);
        os.refresh_takes = Duration::from_millis(50);
        let mut router = router(os);
        let before = now();
        router.uplink(&mut syn(50000), before);

        // Read from the TUN device while the first view was being taken.
        router.uplink(&mut syn(50001), before + Duration::from_millis(25));

        assert_eq!(router.resolver.calls.refresh, 2);
    }

    #[test]
    fn one_fresh_snapshot_serves_every_new_flow_of_a_burst() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 30);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.uplink(&mut syn(50001), start);

        assert_eq!(router.resolver.calls.refresh, 1);
    }

    #[test]
    fn an_unresolvable_flow_goes_to_main_after_one_fresh_snapshot() {
        let mut router = router(os());
        let start = now();
        let first = router.uplink(&mut syn(50000), start);
        let second_flow = router.uplink(&mut syn(50001), start);
        let again = router.uplink(&mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b""), start);

        assert_eq!(
            (first, second_flow, again),
            (Verdict::Main, Verdict::Main, Verdict::Main)
        );
        assert_eq!(router.resolver.calls.refresh, 1);
        assert_eq!(router.counters().unresolved_flows, 2);
    }

    #[test]
    fn a_packet_that_arrived_after_the_snapshot_gets_a_fresh_one() {
        let mut router = router(os());
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.resolver.socket(tcp_flow(50001), 10);

        let verdict = router.uplink(&mut syn(50001), start + Duration::from_secs(1));

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
        assert_eq!(router.resolver.calls.refresh, 2);
    }

    #[test]
    fn a_failed_refresh_leaves_the_flow_on_main_and_is_counted() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.fail_refresh = true;
        let mut router = router(os);

        let verdict = router.uplink(&mut syn(50000), now());

        assert_eq!(verdict, Verdict::Main);
        assert_eq!(router.counters().refresh_errors, 1);
    }

    #[test]
    fn looks_an_executable_up_once_per_process() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 10);
        let mut router = router(os);
        let start = now();

        router.uplink(&mut syn(50000), start);
        router.uplink(&mut syn(50001), start);

        assert_eq!(router.resolver.calls.executable, 1);
    }

    #[test]
    fn a_recycled_pid_is_attributed_to_its_new_program() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.resolver.process(10, 9999, OTHER);
        router.resolver.socket(tcp_flow(50001), 10);
        let verdict = router.uplink(&mut syn(50001), start);

        assert_eq!(verdict, Verdict::Main);
    }

    #[test]
    fn a_routed_flow_is_dropped_while_its_route_is_not_connected() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();

        router.set_route_state(RouteId(0), RouteState::Connecting);
        let connecting = router.uplink(&mut syn(50000), start);
        router.set_route_state(RouteId(0), RouteState::Unavailable);
        let unavailable = router.uplink(&mut syn(50000), start);

        assert_eq!((connecting, unavailable), (Verdict::Drop, Verdict::Drop));
        assert_eq!(router.counters().dropped_packets, 2);
    }

    #[test]
    fn a_flow_resumes_on_its_route_once_it_reconnects() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.set_route_state(RouteId(0), RouteState::Connecting);
        router.uplink(&mut syn(50000), start);
        let calls = router.resolver.calls;

        router.set_route_state(RouteId(0), connected(ROUTE0));
        let verdict = router.uplink(
            &mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"data"),
            start,
        );

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
        assert_eq!(router.resolver.calls, calls);
    }

    #[test]
    fn a_routed_ipv6_flow_is_dropped_when_its_route_has_no_ipv6() {
        let flow = FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new(IpAddr::from(MAIN6), 50000),
            remote: SocketAddr::new(IpAddr::from(REMOTE6), 443),
        };
        let mut os = os();
        os.socket(flow, 10);
        let mut router = router(os);

        let verdict = router.uplink(&mut tcp_v6(MAIN6, REMOTE6, 50000, 443, TCP_SYN, b""), now());

        assert_eq!(verdict, Verdict::Drop);
    }

    #[test]
    fn a_routed_ipv6_flow_is_translated_when_its_route_has_ipv6() {
        let flow = FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new(IpAddr::from(MAIN6), 50000),
            remote: SocketAddr::new(IpAddr::from(REMOTE6), 443),
        };
        let mut os = os();
        os.socket(flow, 10);
        let mut router = router(os);
        router.set_route_state(
            RouteId(0),
            RouteState::Connected(SessionAddresses {
                v4: v4(ROUTE0),
                v6: Some(Ipv6Addr::from(ROUTE6)),
            }),
        );
        let mut packet = tcp_v6(MAIN6, REMOTE6, 50000, 443, TCP_SYN, b"");

        let verdict = router.uplink(&mut packet, now());

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
        assert_eq!(packet, tcp_v6(ROUTE6, REMOTE6, 50000, 443, TCP_SYN, b""));
    }

    #[test]
    fn a_routed_packet_from_another_source_than_the_main_address_is_dropped() {
        let stray = [10, 64, 0, 3];
        let flow = FlowKey {
            local: SocketAddr::new(IpAddr::from(stray), 50000),
            ..tcp_flow(50000)
        };
        let mut os = os();
        os.socket(flow, 10);
        let mut router = router(os);

        let verdict = router.uplink(&mut tcp_v4(stray, REMOTE, 50000, 443, TCP_SYN, b""), now());

        assert_eq!(verdict, Verdict::Drop);
    }

    #[test]
    fn translates_an_answer_back_to_the_main_address() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        let mut answer = tcp_v4(REMOTE, ROUTE0, 443, 50000, TCP_SYN | TCP_ACK, b"");

        let delivery = router.downlink(RouteId(0), &mut answer, start);

        assert_eq!(delivery, Delivery::Deliver);
        assert_eq!(
            answer,
            tcp_v4(REMOTE, MAIN, 443, 50000, TCP_SYN | TCP_ACK, b"")
        );
    }

    #[test]
    fn drops_an_answer_for_a_flow_the_route_does_not_carry() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        os.socket(tcp_flow(50001), 30);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.uplink(&mut syn(50001), start);

        let unknown = router.downlink(
            RouteId(0),
            &mut tcp_v4(REMOTE, ROUTE0, 443, 50002, TCP_ACK, b""),
            start,
        );
        let main_flow = router.downlink(
            RouteId(0),
            &mut tcp_v4(REMOTE, ROUTE0, 443, 50001, TCP_ACK, b""),
            start,
        );
        let other_route = router.downlink(
            RouteId(1),
            &mut tcp_v4(REMOTE, ROUTE1, 443, 50000, TCP_ACK, b""),
            start,
        );

        assert_eq!(
            (unknown, main_flow, other_route),
            (Delivery::Drop, Delivery::Drop, Delivery::Drop)
        );
    }

    #[test]
    fn drops_an_answer_addressed_to_another_address_than_the_route() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);

        let delivery = router.downlink(
            RouteId(0),
            &mut tcp_v4(REMOTE, MAIN, 443, 50000, TCP_ACK, b""),
            start,
        );

        assert_eq!(delivery, Delivery::Drop);
    }

    #[test]
    fn drops_what_a_disconnected_route_delivers() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.set_route_state(RouteId(0), RouteState::Connecting);

        let delivery = router.downlink(
            RouteId(0),
            &mut tcp_v4(REMOTE, ROUTE0, 443, 50000, TCP_ACK, b""),
            start,
        );

        assert_eq!(delivery, Delivery::Drop);
    }

    #[test]
    fn translates_an_icmp_error_about_a_routed_flow() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        let mut sent = udp_v4(MAIN, REMOTE, 50000, 443, &[1; 32]);
        router.uplink(&mut sent, start);
        let mut error = icmp_v4_error([203, 0, 113, 1], ROUTE0, &sent[..28]);

        let delivery = router.downlink(RouteId(0), &mut error, start);

        assert_eq!(delivery, Delivery::Deliver);
        assert_eq!(&error[16..20], &MAIN);
        assert_eq!(&error[28 + 12..28 + 16], &MAIN);
    }

    #[test]
    fn an_icmp_error_the_host_sends_about_a_routed_flow_is_dropped() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        os.socket(udp_flow(50001), 30);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50000, 443, b"x"), start);
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50001, 443, b"x"), start);
        let received_routed = udp_v4(REMOTE, MAIN, 443, 50000, b"late");
        let received_main = udp_v4(REMOTE, MAIN, 443, 50001, b"late");

        let routed = router.uplink(
            &mut icmp_v4_error(MAIN, REMOTE, &received_routed[..28]),
            start,
        );
        let main = router.uplink(
            &mut icmp_v4_error(MAIN, REMOTE, &received_main[..28]),
            start,
        );

        assert_eq!((routed, main), (Verdict::Drop, Verdict::Main));
    }

    #[test]
    fn the_later_fragments_of_a_routed_datagram_follow_the_first() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        let datagram = udp_v4(MAIN, REMOTE, 50000, 443, &[5; 64]);
        let mut first = as_v4_fragment(datagram.clone(), 77, 0, true);
        let mut later = as_v4_fragment(datagram, 77, 4, false);

        let first_verdict = router.uplink(&mut first, start);
        let later_verdict = router.uplink(&mut later, start);

        assert_eq!(first_verdict, Verdict::Route(RouteId(0)));
        assert_eq!(later_verdict, Verdict::Route(RouteId(0)));
        assert_eq!(&later[12..16], &ROUTE0);
        assert!(ipv4_header_ok(&later));
    }

    #[test]
    fn a_later_fragment_whose_first_was_not_seen_is_dropped() {
        let mut router = router(os());
        let mut unknown = as_v4_fragment(udp_v4(MAIN, REMOTE, 50009, 443, &[5; 64]), 78, 4, false);

        let verdict = router.uplink(&mut unknown, now());

        assert_eq!(verdict, Verdict::Drop);
    }

    #[test]
    fn a_full_fragment_map_forgets_its_oldest_datagram_first() {
        let mut os = os();
        os.socket(udp_flow(50000), 30);
        let mut router = router(os);
        let start = now();
        let datagram = udp_v4(MAIN, REMOTE, 50000, 443, &[5; 64]);
        for (index, id) in (0..=FRAGMENT_CAPACITY as u16).enumerate() {
            let mut first = as_v4_fragment(datagram.clone(), id, 0, true);
            router.uplink(&mut first, start + Duration::from_millis(index as u64));
        }
        let at = start + Duration::from_secs(1);

        let oldest = router.uplink(&mut as_v4_fragment(datagram.clone(), 0, 4, false), at);
        let next = router.uplink(&mut as_v4_fragment(datagram, 1, 4, false), at);

        assert_eq!((oldest, next), (Verdict::Drop, Verdict::Main));
    }

    #[test]
    fn a_route_cannot_flush_the_fragments_of_another_route() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50000, 443, b"q"), start);
        let answer = udp_v4(REMOTE, ROUTE0, 443, 50000, &[6; 64]);
        router.downlink(
            RouteId(0),
            &mut as_v4_fragment(answer.clone(), 1, 0, true),
            start,
        );
        // Route 1 claims route 0's flow with fragments of its own.
        let claim = udp_v4(REMOTE, ROUTE1, 443, 50000, &[6; 64]);
        for id in 1000..1000 + 2 * FRAGMENT_CAPACITY as u16 {
            router.downlink(
                RouteId(1),
                &mut as_v4_fragment(claim.clone(), id, 0, true),
                start,
            );
        }

        let later = router.downlink(RouteId(0), &mut as_v4_fragment(answer, 1, 4, false), start);

        assert_eq!(later, Delivery::Deliver);
    }

    #[test]
    fn a_route_cannot_close_a_flow_it_does_not_carry() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        let reset = tcp_v4(REMOTE, ROUTE1, 443, 50000, TCP_RST, b"");

        let delivery = router.downlink(RouteId(1), &mut reset.clone(), start);
        let later = start + crate::flow::CLOSING_LINGER * 3;
        router.uplink(&mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b""), later);

        assert_eq!(delivery, Delivery::Drop);
        assert_eq!(router.counters().new_flows, 1);
    }

    #[test]
    fn a_route_cannot_deliver_an_icmp_error_about_a_flow_it_does_not_carry() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50000, 443, b"q"), start);
        let claimed = udp_v4(ROUTE1, REMOTE, 50000, 443, &[1; 32]);

        let delivery = router.downlink(
            RouteId(1),
            &mut icmp_v4_error([203, 0, 113, 1], ROUTE1, &claimed[..28]),
            start,
        );

        assert_eq!(delivery, Delivery::Drop);
    }

    #[test]
    fn a_route_sharing_another_routes_address_cannot_deliver_its_fragments() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        router.set_route_state(RouteId(1), connected(ROUTE0));
        let start = now();
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50000, 443, b"q"), start);
        let answer = udp_v4(REMOTE, ROUTE0, 443, 50000, &[6; 64]);
        router.downlink(
            RouteId(0),
            &mut as_v4_fragment(answer.clone(), 1, 0, true),
            start,
        );

        let later = router.downlink(RouteId(1), &mut as_v4_fragment(answer, 1, 4, false), start);

        assert_eq!(later, Delivery::Drop);
    }

    #[test]
    fn a_stale_snapshot_is_not_trusted_for_a_packet_that_arrived_after_it() {
        // Another app held the flow's port when the snapshot was taken, and
        // the browser holds it by the time its packet arrives.
        let mut os = os();
        os.socket(tcp_flow(50000), 30);
        os.socket(udp_flow(50001), 30);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.resolver.socket(udp_flow(50001), 10);

        let verdict = router.uplink(
            &mut udp_v4(MAIN, REMOTE, 50001, 443, b"quic"),
            start + Duration::from_secs(1),
        );

        assert_eq!(verdict, Verdict::Route(RouteId(0)));
    }

    #[test]
    fn a_tcp_segment_of_a_connection_without_a_live_owner_is_dropped() {
        let mut router = router(os());

        let verdict = router.uplink(
            &mut tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK | TCP_FIN, b""),
            now(),
        );

        assert_eq!(verdict, Verdict::Drop);
    }

    #[test]
    fn a_connection_opening_on_a_known_tuple_is_attributed_again() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);
        router.resolver.socket(tcp_flow(50000), 30);

        let verdict = router.uplink(&mut syn(50000), start + Duration::from_secs(1));

        assert_eq!(verdict, Verdict::Main);
    }

    #[test]
    fn an_echo_goes_to_main_without_asking_the_os() {
        let mut router = router(os());

        let verdict = router.uplink(&mut icmp_echo_v4(MAIN, REMOTE, 7, true), now());

        assert_eq!(verdict, Verdict::Main);
        assert_eq!(router.resolver.calls, fake::Calls::default());
    }

    #[test]
    fn a_route_state_renders_without_its_addresses() {
        let rendered = format!("{:?}", connected(ROUTE0));

        assert!(!rendered.contains("10.99.0.7"), "{rendered}");
    }

    #[test]
    fn the_later_fragments_of_an_answer_follow_the_first() {
        let mut os = os();
        os.socket(udp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut udp_v4(MAIN, REMOTE, 50000, 443, b"q"), start);
        let answer = udp_v4(REMOTE, ROUTE0, 443, 50000, &[6; 64]);
        let mut first = as_v4_fragment(answer.clone(), 90, 0, true);
        let mut later = as_v4_fragment(answer, 90, 4, false);

        let first_delivery = router.downlink(RouteId(0), &mut first, start);
        let later_delivery = router.downlink(RouteId(0), &mut later, start);

        assert_eq!(
            (first_delivery, later_delivery),
            (Delivery::Deliver, Delivery::Deliver)
        );
        assert_eq!(&later[16..20], &MAIN);
    }

    #[test]
    fn a_new_policy_attributes_every_flow_again() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();
        router.uplink(&mut syn(50000), start);

        router.set_policy(
            Policy::new(
                main_addresses(),
                [(MAILER, RouteId(0))],
                vec![connected(ROUTE0)],
            )
            .unwrap(),
        );
        let verdict = router.uplink(&mut syn(50000), start);

        assert_eq!(verdict, Verdict::Main);
    }

    #[test]
    fn an_unparseable_packet_goes_to_main_uplink_and_is_dropped_downlink() {
        let mut router = router(os());
        let mut garbage = vec![0x45, 0, 0];

        let up = router.uplink(&mut garbage.clone(), now());
        let down = router.downlink(RouteId(0), &mut garbage, now());

        assert_eq!((up, down), (Verdict::Main, Delivery::Drop));
    }

    #[test]
    fn counts_routed_packets() {
        let mut os = os();
        os.socket(tcp_flow(50000), 10);
        let mut router = router(os);
        let start = now();

        router.uplink(&mut syn(50000), start);
        router.downlink(
            RouteId(0),
            &mut tcp_v4(REMOTE, ROUTE0, 443, 50000, TCP_ACK, b""),
            start,
        );

        assert_eq!(router.counters().routed_packets, 2);
        assert_eq!(router.counters().new_flows, 1);
    }

    #[test]
    fn refuses_a_policy_naming_a_route_it_does_not_have() {
        let result = Policy::new(
            main_addresses(),
            [(BROWSER, RouteId(1))],
            vec![connected(ROUTE0)],
        );

        assert_eq!(result.err(), Some(PolicyError::UnknownRoute));
    }
}
