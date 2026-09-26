//! Keeps the route sessions in line with the daemon's plan.
//!
//! The controller owns the route sessions of one tunnel. On every plan it
//! stops the sessions whose circuit left the plan, names the relays of the
//! remaining and new ones to the firewall and waits until it took them, then
//! starts the new sessions and hands the router the apps of each route. Each
//! session's state feeds the router, so a route that is not connected drops
//! its apps' packets, and the daemon, which shows it.
//!
//! Sessions are kept in fixed slots, one [`RouteId`] each, and one more route
//! past the slots carries the blocked apps: a route that is never connected.
//! How many slots may hold a session follows the main session's anchor
//! ([`super::route_capacity`]): the routes of the plan past that number wait,
//! their apps blocked, until a larger answer or a shorter plan frees a slot.

use std::sync::Arc;

use futures::future::BoxFuture;
use talpid_app_routing::{
    owner::OwnerResolver,
    router::{Policy, RouteId, RouteState, SessionAddresses},
};
use tokio::sync::{mpsc, watch};
use warrenguard_multihop::RelayDescriptorSigned;
use warrenguard_transport::route_anchor::AnchorState;
use warrenguard_transport_core::PacketDevice;

use super::{
    AppRouteObserver, AppRoutesPlan, MAX_ROUTE_SESSIONS, MainRoute, PlannedRoute, RouteReport,
    RouteSessionState, SessionEvent, TOKEN_ROUTE_SESSIONS, circuit_identity,
    datapath::{RouteTun, RoutingTable},
    route_capacity,
};
use crate::MultiHopConfig;

/// The route past the session slots, never connected, that blocks the apps
/// whose exit cannot be served.
const BLOCKED: RouteId = RouteId(MAX_ROUTE_SESSIONS as u8);

/// Names to the firewall every relay the route sessions may reach outside the
/// tunnel, and resolves once it applied them.
pub type RelaySink =
    Arc<dyn Fn(Vec<RelayDescriptorSigned>) -> BoxFuture<'static, ()> + Send + Sync>;

/// Starts route sessions. The handle ends its session when dropped.
pub trait RouteSessions<T>: Send + 'static {
    type Handle: Send + 'static;

    /// Starts a session dialing `circuit` and carrying the packets of
    /// `device`. It reports through `events`, starting from connecting.
    fn start(&mut self, circuit: &MultiHopConfig, device: T, events: SessionEvents)
    -> Self::Handle;

    /// Ends a session and resolves once it released what it held.
    fn stop(handle: Self::Handle) -> BoxFuture<'static, ()> {
        Box::pin(async move { drop(handle) })
    }
}

/// A session's line to the controller, tagged so the reports of a session
/// that was replaced in its slot are ignored.
pub struct SessionEvents {
    tx: mpsc::UnboundedSender<Tagged>,
    slot: usize,
    generation: u64,
}

impl SessionEvents {
    pub fn send(&self, event: SessionEvent) {
        let _ = self.tx.send(Tagged {
            slot: self.slot,
            generation: self.generation,
            event,
        });
    }
}

struct Tagged {
    slot: usize,
    generation: u64,
    event: SessionEvent,
}

struct Slot<H> {
    circuit: MultiHopConfig,
    apps: Vec<String>,
    state: RouteSessionState,
    addresses: Option<SessionAddresses>,
    generation: u64,
    /// `None` while the slot is reserved and its session not started yet:
    /// its apps are already the route's, and dropped until it connects.
    session: Option<H>,
}

impl<H> Slot<H> {
    fn identity(&self) -> ([u8; 16], [u8; 16]) {
        circuit_identity(&self.circuit)
    }

    fn route_state(&self) -> RouteState {
        match (self.state, self.addresses) {
            (RouteSessionState::Connected, Some(addresses)) => RouteState::Connected(addresses),
            (RouteSessionState::Unavailable(_), _) => RouteState::Unavailable,
            _ => RouteState::Connecting,
        }
    }
}

/// The next state of the main session's anchor, or `None` once its sender is
/// gone. Never resolves without an anchor.
async fn anchor_changed(anchor: &mut Option<watch::Receiver<AnchorState>>) -> Option<AnchorState> {
    let Some(anchor) = anchor.as_mut() else {
        return std::future::pending().await;
    };
    anchor.changed().await.ok()?;
    Some(*anchor.borrow_and_update())
}

/// The route sessions of one tunnel.
pub struct RouteController<D, R, S>
where
    D: PacketDevice + Clone,
    R: OwnerResolver + Send + 'static,
    S: RouteSessions<RouteTun<D, R>>,
{
    table: Arc<RoutingTable<R>>,
    device: D,
    sessions: S,
    /// The main session's inner addresses, the only sources the router
    /// translates.
    main: SessionAddresses,
    /// The exit the main session is on right now.
    main_exit: Option<[u8; 16]>,
    name_relays: RelaySink,
    observer: Option<AppRouteObserver>,
    slots: Vec<Option<Slot<S::Handle>>>,
    /// How many slots may hold a session now.
    capacity: usize,
    /// The plan last applied, applied again when the capacity changes.
    plan: AppRoutesPlan,
    /// The exits of the planned routes past the capacity.
    waiting: Vec<[u8; 16]>,
    blocked_apps: Vec<String>,
    main_apps: Vec<MainRoute>,
    events_tx: mpsc::UnboundedSender<Tagged>,
    events_rx: mpsc::UnboundedReceiver<Tagged>,
    next_generation: u64,
    reported: Option<Vec<RouteReport>>,
    /// The relays last named to the firewall.
    named: Vec<RelayDescriptorSigned>,
    /// The app-to-route mapping of the policy the table holds.
    mapping: Option<Vec<(String, RouteId)>>,
}

impl<D, R, S> RouteController<D, R, S>
where
    D: PacketDevice + Clone,
    R: OwnerResolver + Send + 'static,
    S: RouteSessions<RouteTun<D, R>>,
{
    pub fn new(
        table: Arc<RoutingTable<R>>,
        device: D,
        sessions: S,
        main: SessionAddresses,
        name_relays: RelaySink,
        observer: Option<AppRouteObserver>,
    ) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            table,
            device,
            sessions,
            main,
            main_exit: None,
            name_relays,
            observer,
            slots: (0..MAX_ROUTE_SESSIONS).map(|_| None).collect(),
            capacity: TOKEN_ROUTE_SESSIONS,
            plan: AppRoutesPlan::default(),
            waiting: Vec::new(),
            blocked_apps: Vec::new(),
            main_apps: Vec::new(),
            events_tx,
            events_rx,
            next_generation: 0,
            reported: None,
            named: Vec::new(),
            mapping: None,
        }
    }

    /// Installs the policy of `plan` at once, before any packet flows: every
    /// planned app is on a route that is not connected yet, so none of its
    /// packets can go through the main session while the sessions start.
    /// `main_exit` is the exit the main session is on.
    pub fn seed(&mut self, plan: &AppRoutesPlan, main_exit: Option<[u8; 16]>) {
        self.main_exit = main_exit;
        self.reserve(plan);
        self.set_policy();
    }

    /// How many route sessions may run from now on. Routes of the plan past
    /// it are stopped and wait; waiting ones start when it grows.
    pub async fn set_capacity(&mut self, capacity: usize) {
        let capacity = capacity.min(MAX_ROUTE_SESSIONS);
        if capacity == self.capacity {
            return;
        }
        self.capacity = capacity;
        let plan = self.plan.clone();
        self.apply(&plan).await;
    }

    /// Follows `plan`, the exit the main session is on, and the state of the
    /// main session's anchor when it has one, until the plan's sender is gone
    /// or `shutdown` resolves; then stops every session and waits until they
    /// released what they held.
    pub async fn run(
        mut self,
        mut plan: watch::Receiver<AppRoutesPlan>,
        mut main_exit: watch::Receiver<Option<[u8; 16]>>,
        mut anchor: Option<watch::Receiver<AnchorState>>,
        shutdown: impl std::future::Future<Output = ()>,
    ) {
        tokio::pin!(shutdown);
        self.main_exit = *main_exit.borrow_and_update();
        if let Some(anchor) = anchor.as_mut() {
            self.capacity = route_capacity(Some(*anchor.borrow_and_update()));
        }
        let first = plan.borrow_and_update().clone();
        self.apply(&first).await;
        let mut following_main = true;
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                state = anchor_changed(&mut anchor) => match state {
                    // Only a bound anchor sets the capacity. One that becomes
                    // unavailable after it was bound leaves its routes up:
                    // they stay admitted at their exits until the control
                    // plane ends them (warren-core doc 107 section 12), and a
                    // route it ends falls back to a token.
                    Some(state @ AnchorState::Anchored { .. }) => {
                        self.set_capacity(route_capacity(Some(state))).await;
                    }
                    Some(AnchorState::Unanchored | AnchorState::Unavailable) => {}
                    // The main supervisor is gone with the tunnel; what it
                    // last said stands until the plan's sender goes too.
                    None => anchor = None,
                },
                changed = plan.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let next = plan.borrow_and_update().clone();
                    self.apply(&next).await;
                }
                changed = main_exit.changed(), if following_main => {
                    if changed.is_err() {
                        following_main = false;
                        continue;
                    }
                    self.main_exit = *main_exit.borrow_and_update();
                    self.set_policy();
                }
                Some(tagged) = self.events_rx.recv() => self.handle_event(tagged),
            }
        }
        self.stop_all().await;
    }

    async fn stop_all(&mut self) {
        let mut stopping = Vec::new();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if let Some(slot) = slot.take() {
                self.table.close_route(route_id(index));
                stopping.extend(slot.session.map(S::stop));
            }
        }
        futures::future::join_all(stopping).await;
    }

    /// Brings the sessions and the router in line with `plan`.
    pub async fn apply(&mut self, plan: &AppRoutesPlan) {
        self.plan = plan.clone();
        let served: Vec<([u8; 16], [u8; 16])> = self
            .partition(plan)
            .0
            .iter()
            .map(|route| circuit_identity(&route.circuit))
            .collect();
        let mut stopping = Vec::new();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let kept = slot
                .as_ref()
                .is_some_and(|slot| served.contains(&slot.identity()));
            if !kept && let Some(slot) = slot.take() {
                self.table.close_route(route_id(index));
                stopping.extend(slot.session.map(S::stop));
            }
        }
        // A stopped session's relay is no longer named below, so it must be
        // gone before the firewall hears it.
        futures::future::join_all(stopping).await;

        // The apps of a new route are the route's from now on, and dropped
        // while its session is not up: the policy goes in before any wait.
        self.reserve(plan);
        self.set_policy();

        // The firewall lets a route's relay through before its session
        // dials, so the first handshake is not dropped.
        let relays: Vec<RelayDescriptorSigned> = self
            .slots
            .iter()
            .flatten()
            .map(|slot| slot.circuit.relay.clone())
            .collect();
        if relays != self.named {
            (self.name_relays)(relays.clone()).await;
            self.named = relays;
        }

        for index in 0..self.slots.len() {
            if self.slots[index]
                .as_ref()
                .is_some_and(|slot| slot.session.is_none())
            {
                self.start(index);
            }
        }
        self.report();
    }

    /// Takes a slot for each new route of `plan` and gives every route its
    /// apps, without starting anything.
    /// The routes of `plan` that may hold a session now, and those that wait:
    /// first the routes that already hold one, in plan order, so a route
    /// added to the plan never takes the place of a live one, then the
    /// others, up to the capacity.
    fn partition<'p>(
        &self,
        plan: &'p AppRoutesPlan,
    ) -> (Vec<&'p PlannedRoute>, Vec<&'p PlannedRoute>) {
        let (live, new): (Vec<&PlannedRoute>, Vec<&PlannedRoute>) =
            plan.routes.iter().partition(|route| {
                let identity = circuit_identity(&route.circuit);
                self.slots
                    .iter()
                    .flatten()
                    .any(|slot| slot.identity() == identity)
            });
        let mut served: Vec<&PlannedRoute> = live.into_iter().chain(new).collect();
        let waiting = served.split_off(served.len().min(self.capacity));
        (served, waiting)
    }

    fn reserve(&mut self, plan: &AppRoutesPlan) {
        let (served, over_limit) = self.partition(plan);
        if !over_limit.is_empty() {
            log::debug!(
                "App routing: {} route sessions asked for, {} may run now; the others wait",
                plan.routes.len(),
                self.capacity
            );
        }
        self.waiting = over_limit
            .iter()
            .map(|route| circuit_identity(&route.circuit).1)
            .collect();
        for route in served {
            let identity = circuit_identity(&route.circuit);
            if let Some(slot) = self
                .slots
                .iter_mut()
                .flatten()
                .find(|slot| slot.identity() == identity)
            {
                slot.apps.clone_from(&route.apps);
                continue;
            }
            let Some(free) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
                break;
            };
            let generation = self.next_generation;
            self.next_generation += 1;
            *free = Some(Slot {
                circuit: route.circuit.clone(),
                apps: route.apps.clone(),
                state: RouteSessionState::Connecting,
                addresses: None,
                generation,
                session: None,
            });
        }
        self.blocked_apps = plan
            .blocked_apps
            .iter()
            .chain(over_limit.iter().flat_map(|route| &route.apps))
            .cloned()
            .collect();
        self.main_apps.clone_from(&plan.main_apps);
    }

    fn start(&mut self, index: usize) {
        let id = route_id(index);
        let queue = self.table.open_route(id);
        let device = RouteTun::new(self.device.clone(), Arc::clone(&self.table), id, queue);
        let Some(slot) = self.slots[index].as_mut() else {
            return;
        };
        let events = SessionEvents {
            tx: self.events_tx.clone(),
            slot: index,
            generation: slot.generation,
        };
        slot.session = Some(self.sessions.start(&slot.circuit, device, events));
        let state = slot.route_state();
        self.table.set_route_state(id, state);
    }

    fn handle_event(&mut self, tagged: Tagged) {
        let Some(Some(slot)) = self.slots.get_mut(tagged.slot) else {
            return;
        };
        if slot.generation != tagged.generation {
            return;
        }
        (slot.state, slot.addresses) = match tagged.event {
            SessionEvent::Connecting => (RouteSessionState::Connecting, None),
            SessionEvent::Connected(addresses) => (RouteSessionState::Connected, Some(addresses)),
            SessionEvent::Unavailable(reason) => (RouteSessionState::Unavailable(reason), None),
            SessionEvent::Waiting => (RouteSessionState::Waiting, None),
        };
        let state = slot.route_state();
        self.table.set_route_state(route_id(tagged.slot), state);
        self.report();
    }

    /// Hands the table the current app-to-route mapping. An app the plan
    /// leaves on the main session is blocked unless the main session is on
    /// the exit the plan assumed: the main session may have moved since.
    fn set_policy(&mut self) {
        let mut routes: Vec<RouteState> = self
            .slots
            .iter()
            .map(|slot| {
                slot.as_ref()
                    .map_or(RouteState::Unavailable, Slot::route_state)
            })
            .collect();
        routes.push(RouteState::Unavailable);
        let main_exit = self.main_exit;
        let mapping: Vec<(String, RouteId)> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| Some((index, slot.as_ref()?)))
            .flat_map(|(index, slot)| {
                slot.apps
                    .iter()
                    .map(move |app| (app.clone(), route_id(index)))
            })
            .chain(self.blocked_apps.iter().map(|app| (app.clone(), BLOCKED)))
            .chain(
                self.main_apps
                    .iter()
                    .filter(|main| Some(main.exit_id) != main_exit)
                    .flat_map(|main| main.apps.iter().map(|app| (app.clone(), BLOCKED))),
            )
            .collect();
        if self.mapping.as_ref() == Some(&mapping) {
            // Same apps on the same routes: keep the flows the router knows,
            // so the answers of a route's live flows still get through.
            for (index, state) in routes.into_iter().enumerate() {
                self.table.set_route_state(route_id(index), state);
            }
            return;
        }
        let active = !mapping.is_empty();
        match Policy::new(
            self.main,
            mapping.iter().map(|(app, route)| (app, *route)),
            routes,
        ) {
            Ok(policy) => {
                self.table.set_policy(policy, active);
                self.mapping = Some(mapping);
            }
            // Unreachable: every route id above indexes `routes`. The table
            // keeps the policy it has rather than let an app fall to main.
            Err(error) => log::error!("App routing policy refused: {error}"),
        }
    }

    fn report(&mut self) {
        let reports: Vec<RouteReport> = self
            .slots
            .iter()
            .flatten()
            .map(|slot| RouteReport {
                exit_id: slot.identity().1,
                state: slot.state,
            })
            .chain(self.waiting.iter().map(|exit_id| RouteReport {
                exit_id: *exit_id,
                state: RouteSessionState::Waiting,
            }))
            .collect();
        if self.reported.as_ref() == Some(&reports) {
            return;
        }
        self.reported = Some(reports.clone());
        if let Some(observer) = &self.observer {
            observer(reports);
        }
    }
}

impl<D, R, S> Drop for RouteController<D, R, S>
where
    D: PacketDevice + Clone,
    R: OwnerResolver + Send + 'static,
    S: RouteSessions<RouteTun<D, R>>,
{
    fn drop(&mut self) {
        // The sessions end with their handles; their apps stay blocked by
        // the policy until the table itself goes.
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.take().is_some() {
                self.table.close_route(route_id(index));
            }
        }
    }
}

fn route_id(index: usize) -> RouteId {
    RouteId(u8::try_from(index).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use std::{
        net::Ipv4Addr,
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use warrenguard_transport_core::FakeTun;

    use super::*;
    use crate::app_routes::{PlannedRoute, RouteUnavailable, datapath::RoutedTun, test_support::*};

    type Device = RouteTun<FakeTun, TwoApps>;

    struct Started {
        events: SessionEvents,
        device: Device,
        alive: Arc<AtomicBool>,
    }

    /// Records every session started and every relay set named, in order.
    #[derive(Clone, Default)]
    struct World {
        started: Arc<Mutex<Vec<Started>>>,
        log: Arc<Mutex<Vec<String>>>,
        /// Set: the firewall never finishes applying what it is handed.
        firewall_stuck: Arc<AtomicBool>,
        reports: Arc<Mutex<Vec<Vec<RouteReport>>>>,
    }

    struct FakeHandle(Arc<AtomicBool>);

    impl Drop for FakeHandle {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }

    impl RouteSessions<Device> for World {
        type Handle = FakeHandle;

        fn start(
            &mut self,
            circuit: &MultiHopConfig,
            device: Device,
            events: SessionEvents,
        ) -> FakeHandle {
            let alive = Arc::new(AtomicBool::new(true));
            self.log
                .lock()
                .unwrap()
                .push(format!("start {}", circuit.relay.endpoint));
            self.started.lock().unwrap().push(Started {
                events,
                device,
                alive: Arc::clone(&alive),
            });
            FakeHandle(alive)
        }
    }

    impl World {
        fn sink(&self) -> RelaySink {
            let log = Arc::clone(&self.log);
            let stuck = Arc::clone(&self.firewall_stuck);
            Arc::new(move |relays: Vec<RelayDescriptorSigned>| {
                let named: Vec<String> = relays.iter().map(|r| r.endpoint.to_string()).collect();
                log.lock()
                    .unwrap()
                    .push(format!("name [{}]", named.join(",")));
                if stuck.load(Ordering::SeqCst) {
                    return Box::pin(std::future::pending());
                }
                Box::pin(async {})
            })
        }

        fn observer(&self) -> AppRouteObserver {
            let reports = Arc::clone(&self.reports);
            Arc::new(move |report| reports.lock().unwrap().push(report))
        }

        fn starts(&self) -> usize {
            self.started.lock().unwrap().len()
        }

        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }

        /// The session started `nth`, as the controller knows it.
        fn send(&self, nth: usize, event: SessionEvent) {
            self.started.lock().unwrap()[nth].events.send(event);
        }

        fn device(&self, nth: usize) -> Device {
            self.started.lock().unwrap()[nth].device.clone()
        }

        fn alive(&self, nth: usize) -> bool {
            self.started.lock().unwrap()[nth]
                .alive
                .load(Ordering::SeqCst)
        }

        fn last_report(&self) -> Vec<RouteReport> {
            self.reports
                .lock()
                .unwrap()
                .last()
                .cloned()
                .unwrap_or_default()
        }
    }

    fn route(relay: u8, apps: &[&str]) -> PlannedRoute {
        PlannedRoute {
            circuit: circuit(relay, relay),
            apps: apps.iter().map(|app| (*app).to_owned()).collect(),
        }
    }

    fn plan(routes: Vec<PlannedRoute>) -> AppRoutesPlan {
        AppRoutesPlan {
            routes,
            ..Default::default()
        }
    }

    fn connected(v4: Ipv4Addr) -> SessionEvent {
        SessionEvent::Connected(SessionAddresses {
            v4: Some(v4),
            v6: None,
        })
    }

    struct Rig {
        world: World,
        tun: FakeTun,
        table: Arc<RoutingTable<TwoApps>>,
        os: TwoApps,
        controller: RouteController<FakeTun, TwoApps, World>,
    }

    impl Rig {
        fn new() -> Self {
            let world = World::default();
            let tun = FakeTun::new();
            let os = TwoApps::default();
            let table = RoutingTable::new(os.clone());
            let controller = RouteController::new(
                Arc::clone(&table),
                tun.clone(),
                world.clone(),
                SessionAddresses {
                    v4: Some(MAIN),
                    v6: None,
                },
                world.sink(),
                Some(world.observer()),
            );
            Self {
                world,
                tun,
                table,
                os,
                controller,
            }
        }

        /// Hands the controller what the sessions reported so far.
        fn deliver_events(&mut self) {
            while let Ok(tagged) = self.controller.events_rx.try_recv() {
                self.controller.handle_event(tagged);
            }
        }

        /// Where an opening from `port` goes: main, or route session `nth`.
        async fn opening_goes(&self, port: u16) -> Where {
            let routed = RoutedTun::new(self.tun.clone(), Arc::clone(&self.table));
            self.tun.inject_inbound(syn(port));
            if let Ok(Ok(packet)) =
                tokio::time::timeout(Duration::from_millis(50), routed.recv()).await
            {
                assert_eq!(source(&packet), MAIN, "main gets the packet untouched");
                return Where::Main;
            }
            for nth in 0..self.world.starts() {
                if let Ok(Some(packet)) = self.world.device(nth).try_recv() {
                    return Where::Route(nth, source(&packet));
                }
            }
            Where::Dropped
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Where {
        Main,
        Route(usize, Ipv4Addr),
        Dropped,
    }

    #[tokio::test]
    async fn starts_a_session_for_a_planned_route_and_routes_its_app_once_connected() {
        let mut rig = Rig::new();

        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;
        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();

        assert_eq!(rig.world.starts(), 1);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Route(0, ROUTE));
        assert_eq!(rig.opening_goes(EDITOR_PORT).await, Where::Main);
    }

    #[tokio::test]
    async fn names_a_routes_relay_to_the_firewall_before_its_session_dials() {
        let mut rig = Rig::new();

        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;

        assert_eq!(
            rig.world.log(),
            vec!["name [192.0.2.1:443]", "start 192.0.2.1:443"]
        );
    }

    #[tokio::test]
    async fn a_route_that_is_still_connecting_drops_its_apps_packets() {
        let mut rig = Rig::new();

        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
    }

    #[tokio::test]
    async fn a_session_gone_unavailable_blocks_its_apps_and_is_reported() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;
        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();

        rig.world
            .send(0, SessionEvent::Unavailable(RouteUnavailable::NoToken));
        rig.deliver_events();

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(
            rig.world.last_report(),
            vec![RouteReport {
                exit_id: [1; 16],
                state: RouteSessionState::Unavailable(RouteUnavailable::NoToken),
            }]
        );
    }

    #[tokio::test]
    async fn apps_of_one_route_share_its_session() {
        let mut rig = Rig::new();

        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER, EDITOR])]))
            .await;
        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();

        assert_eq!(rig.world.starts(), 1);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Route(0, ROUTE));
        assert_eq!(rig.opening_goes(EDITOR_PORT).await, Where::Route(0, ROUTE));
    }

    #[tokio::test]
    async fn a_new_plan_keeps_the_sessions_it_still_asks_for() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;
        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();

        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER]), route(2, &[EDITOR])]))
            .await;

        assert_eq!(rig.world.starts(), 2);
        assert!(rig.world.alive(0));
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Route(0, ROUTE));
    }

    #[tokio::test]
    async fn a_route_left_out_of_the_plan_is_stopped_and_its_relay_no_longer_named() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER]), route(2, &[EDITOR])]))
            .await;

        rig.controller.apply(&plan(vec![route(2, &[EDITOR])])).await;

        assert!(!rig.world.alive(0));
        assert!(rig.world.alive(1));
        assert_eq!(rig.world.log().last().unwrap(), "name [192.0.2.2:443]");
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Main);
    }

    fn three_routes() -> AppRoutesPlan {
        plan(vec![route(1, &[]), route(2, &[]), route(3, &[BROWSER])])
    }

    #[tokio::test]
    async fn the_routes_past_the_capacity_wait_with_their_apps_blocked() {
        let mut rig = Rig::new();

        rig.controller.apply(&three_routes()).await;

        assert_eq!(rig.world.starts(), TOKEN_ROUTE_SESSIONS);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(
            rig.world.last_report().last(),
            Some(&RouteReport {
                exit_id: [3; 16],
                state: RouteSessionState::Waiting,
            })
        );
    }

    #[tokio::test]
    async fn a_route_added_past_the_capacity_waits_instead_of_replacing_a_live_one() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(2, &[EDITOR]), route(3, &[BROWSER])]))
            .await;

        rig.controller
            .apply(&plan(vec![
                route(1, &[]),
                route(2, &[EDITOR]),
                route(3, &[BROWSER]),
            ]))
            .await;

        assert_eq!(rig.world.starts(), 2);
        assert!(rig.world.alive(0));
        assert!(rig.world.alive(1));
        assert!(rig.world.last_report().contains(&RouteReport {
            exit_id: [1; 16],
            state: RouteSessionState::Waiting,
        }));
    }

    #[tokio::test]
    async fn a_larger_capacity_starts_the_waiting_routes() {
        let mut rig = Rig::new();
        rig.controller.apply(&three_routes()).await;

        rig.controller.set_capacity(32).await;
        rig.world.send(2, connected(ROUTE));
        rig.deliver_events();

        assert_eq!(rig.world.starts(), 3);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Route(2, ROUTE));
        assert!(
            rig.world
                .last_report()
                .iter()
                .all(|report| report.state != RouteSessionState::Waiting)
        );
    }

    #[tokio::test]
    async fn a_smaller_capacity_stops_the_routes_past_it() {
        let mut rig = Rig::new();
        rig.controller.set_capacity(32).await;
        rig.controller.apply(&three_routes()).await;

        rig.controller.set_capacity(TOKEN_ROUTE_SESSIONS).await;

        assert!(rig.world.alive(0));
        assert!(rig.world.alive(1));
        assert!(!rig.world.alive(2));
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(
            rig.world.log().last().unwrap(),
            "name [192.0.2.1:443,192.0.2.2:443]"
        );
    }

    #[tokio::test]
    async fn a_session_waiting_for_a_token_route_is_reported_waiting_with_its_apps_held() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;

        rig.world.send(0, SessionEvent::Waiting);
        rig.deliver_events();

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(
            rig.world.last_report(),
            vec![RouteReport {
                exit_id: [1; 16],
                state: RouteSessionState::Waiting,
            }]
        );
    }

    #[tokio::test]
    async fn run_keeps_the_routes_of_an_anchor_that_goes_unavailable_once_bound() {
        let rig = Rig::new();
        let world = rig.world.clone();
        let (_plan_tx, plan_rx) = watch::channel(three_routes());
        let (anchor_tx, anchor_rx) = watch::channel(AnchorState::Anchored { max_routes: 32 });
        let running = tokio::spawn(rig.controller.run(
            plan_rx,
            watch::channel(None).1,
            Some(anchor_rx),
            std::future::pending(),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            while world.starts() < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the anchor admits three routes");

        anchor_tx.send_replace(AnchorState::Unavailable);
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }

        assert!((0..3).all(|nth| world.alive(nth)));
        running.abort();
    }

    #[tokio::test]
    async fn run_follows_what_the_main_sessions_anchor_admits() {
        let rig = Rig::new();
        let world = rig.world.clone();
        let (_plan_tx, plan_rx) = watch::channel(three_routes());
        let (anchor_tx, anchor_rx) = watch::channel(AnchorState::Unanchored);
        let running = tokio::spawn(rig.controller.run(
            plan_rx,
            watch::channel(None).1,
            Some(anchor_rx),
            std::future::pending(),
        ));
        let starts = |wanted: usize| {
            let world = world.clone();
            async move {
                tokio::time::timeout(Duration::from_secs(1), async {
                    while world.starts() < wanted {
                        tokio::task::yield_now().await;
                    }
                })
                .await
            }
        };
        starts(TOKEN_ROUTE_SESSIONS)
            .await
            .expect("the token routes started");

        anchor_tx.send_replace(AnchorState::Anchored { max_routes: 32 });

        starts(3).await.expect("the anchor admits the third route");
        running.abort();
    }

    #[tokio::test]
    async fn a_blocked_app_never_reaches_main() {
        let mut rig = Rig::new();

        rig.controller
            .apply(&AppRoutesPlan {
                blocked_apps: vec![BROWSER.to_owned()],
                ..Default::default()
            })
            .await;

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(rig.opening_goes(EDITOR_PORT).await, Where::Main);
    }

    #[tokio::test]
    async fn a_replaced_sessions_late_report_is_ignored() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;
        rig.controller.apply(&plan(Vec::new())).await;
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;

        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
    }

    #[tokio::test]
    async fn the_observer_hears_a_state_once_per_change() {
        let mut rig = Rig::new();
        rig.controller
            .apply(&plan(vec![route(1, &[BROWSER])]))
            .await;

        rig.world.send(0, SessionEvent::Connecting);
        rig.deliver_events();

        assert_eq!(
            *rig.world.reports.lock().unwrap(),
            vec![vec![RouteReport {
                exit_id: [1; 16],
                state: RouteSessionState::Connecting,
            }]]
        );
    }

    #[tokio::test]
    async fn a_plan_without_routes_leaves_the_main_path_without_os_questions() {
        let mut rig = Rig::new();

        rig.controller.apply(&plan(Vec::new())).await;

        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Main);
        assert_eq!(rig.os.calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn run_follows_the_plan_and_ends_the_sessions_with_its_sender() {
        let rig = Rig::new();
        let world = rig.world.clone();
        let (plan_tx, plan_rx) = watch::channel(plan(Vec::new()));
        let running = tokio::spawn(rig.controller.run(
            plan_rx,
            watch::channel(None).1,
            None,
            std::future::pending(),
        ));

        plan_tx.send_replace(plan(vec![route(1, &[BROWSER])]));
        tokio::time::timeout(Duration::from_secs(1), async {
            while world.starts() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the session started");
        drop(plan_tx);
        tokio::time::timeout(Duration::from_secs(1), running)
            .await
            .expect("run ended")
            .unwrap();

        assert!(!world.alive(0));
    }

    #[tokio::test]
    async fn run_stops_the_sessions_on_shutdown() {
        let rig = Rig::new();
        let world = rig.world.clone();
        let (_plan_tx, plan_rx) = watch::channel(plan(vec![route(1, &[BROWSER])]));
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let running =
            tokio::spawn(
                rig.controller
                    .run(plan_rx, watch::channel(None).1, None, async move {
                        let _ = stop_rx.await;
                    }),
            );
        tokio::time::timeout(Duration::from_secs(1), async {
            while world.starts() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the session started");

        stop_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), running)
            .await
            .expect("run ended")
            .unwrap();

        assert!(!world.alive(0));
    }

    #[tokio::test]
    async fn a_seeded_plan_holds_its_apps_before_any_session_starts() {
        let mut rig = Rig::new();

        rig.controller.seed(&plan(vec![route(1, &[BROWSER])]), None);

        assert_eq!(rig.world.starts(), 0);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
        assert_eq!(rig.opening_goes(EDITOR_PORT).await, Where::Main);
    }

    #[tokio::test]
    async fn a_new_routes_apps_are_held_while_the_firewall_names_its_relay() {
        let mut rig = Rig::new();
        rig.world.firewall_stuck.store(true, Ordering::SeqCst);

        let applying = tokio::time::timeout(
            Duration::from_millis(50),
            rig.controller.apply(&plan(vec![route(1, &[BROWSER])])),
        )
        .await;

        assert!(applying.is_err(), "the firewall is still applying");
        assert_eq!(rig.world.starts(), 0);
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Dropped);
    }

    #[tokio::test]
    async fn an_app_left_on_main_rides_it_only_while_main_is_on_the_planned_exit() {
        let mut rig = Rig::new();
        let on_main = AppRoutesPlan {
            main_apps: vec![MainRoute {
                exit_id: [9; 16],
                apps: vec![BROWSER.to_owned()],
            }],
            ..Default::default()
        };

        rig.controller.seed(&on_main, Some([9; 16]));
        let while_on_it = rig.opening_goes(BROWSER_PORT).await;
        rig.controller.seed(&on_main, Some([8; 16]));
        let once_moved = rig.opening_goes(BROWSER_PORT).await;

        assert_eq!(while_on_it, Where::Main);
        assert_eq!(once_moved, Where::Dropped);
    }

    #[tokio::test]
    async fn an_unchanged_mapping_keeps_the_flows_a_route_carries() {
        let mut rig = Rig::new();
        let routed = plan(vec![route(1, &[BROWSER])]);
        rig.controller.apply(&routed).await;
        rig.world.send(0, connected(ROUTE));
        rig.deliver_events();
        assert_eq!(rig.opening_goes(BROWSER_PORT).await, Where::Route(0, ROUTE));

        rig.controller.apply(&routed).await;
        rig.world
            .device(0)
            .send(&syn_ack_to(ROUTE, BROWSER_PORT))
            .await
            .unwrap();

        assert_eq!(rig.tun.take_outbound().len(), 1, "the answer got through");
    }

    #[tokio::test]
    async fn a_plan_without_routes_names_nothing_to_the_firewall() {
        let mut rig = Rig::new();

        rig.controller.apply(&plan(Vec::new())).await;

        assert!(rig.world.log().is_empty());
    }
}
