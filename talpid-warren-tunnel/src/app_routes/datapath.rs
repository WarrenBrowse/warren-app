//! Where the packets of routed apps leave the main path.
//!
//! The pumps move packets through the [`PacketDevice`] they are handed, so
//! per-app routing sits in two devices over the TUN rather than in the pumps:
//!
//! - [`RoutedTun`] is what the main session's pumps read and write. An uplink
//!   packet of a routed app is taken out of the main path there, translated,
//!   and queued for its route session; everything else is returned untouched.
//! - [`RouteTun`] is what a route session's pumps read and write: they read
//!   the queue of their route, and what they write is translated back and
//!   written to the TUN only when it belongs to a flow of that route.
//!
//! While no app has a route, the main path pays one relaxed atomic load per
//! packet and nothing else: no lock, no clock read, no parse.

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use parking_lot::Mutex;
use talpid_app_routing::{
    owner::OwnerResolver,
    router::{Counters, Delivery, Policy, RouteId, RouteState, Router, Verdict},
};
use tokio::sync::mpsc;
use warrenguard_transport_core::PacketDevice;

/// Uplink packets waiting for one route session. A route session that falls
/// behind loses its newest packets, the way a full TUN queue would.
const ROUTE_QUEUE_PACKETS: usize = 1024;

/// The router between the TUN device and the sessions, shared by the main
/// uplink pump, the route sessions' downlink pumps and the controller.
pub struct RoutingTable<R> {
    /// Whether any app has a route. Read on every main packet, so it is kept
    /// outside the lock.
    active: AtomicBool,
    inner: Mutex<Inner<R>>,
}

struct Inner<R> {
    router: Router<R>,
    /// The queue of each route session, by route.
    queues: Vec<Option<mpsc::Sender<Vec<u8>>>>,
}

/// Where an uplink packet went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Uplink {
    /// Still in the caller's hands, for the main session.
    Main,
    /// Queued for a route session, or dropped.
    Taken,
}

impl<R: OwnerResolver + Send + 'static> RoutingTable<R> {
    /// An inactive table over `resolver`: every packet stays on the main
    /// path until [`Self::set_policy`] routes an app.
    pub fn new(resolver: R) -> Arc<Self> {
        Arc::new(Self {
            active: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                router: Router::new(resolver),
                queues: Vec::new(),
            }),
        })
    }

    /// Replaces the policy. `active` says whether it routes any app, which
    /// the router does not expose.
    pub fn set_policy(&self, policy: Policy, active: bool) {
        let mut inner = self.inner.lock();
        inner.router.set_policy(policy);
        self.active.store(active, Ordering::Release);
    }

    pub fn set_route_state(&self, route: RouteId, state: RouteState) {
        self.inner.lock().router.set_route_state(route, state);
    }

    /// Opens a fresh queue for `route`, replacing any earlier one, whose
    /// reader then sees the queue closed.
    pub fn open_route(&self, route: RouteId) -> mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::channel(ROUTE_QUEUE_PACKETS);
        let mut inner = self.inner.lock();
        let slot = usize::from(route.0);
        if inner.queues.len() <= slot {
            inner.queues.resize(slot + 1, None);
        }
        inner.queues[slot] = Some(tx);
        rx
    }

    /// Closes the queue of `route`. The route's packets are dropped from now
    /// on, since its state is no longer connected.
    pub fn close_route(&self, route: RouteId) {
        let mut inner = self.inner.lock();
        inner.router.set_route_state(route, RouteState::Unavailable);
        if let Some(slot) = inner.queues.get_mut(usize::from(route.0)) {
            *slot = None;
        }
    }

    pub fn counters(&self) -> Counters {
        self.inner.lock().router.counters()
    }

    fn uplink(&self, packet: &mut Vec<u8>) -> Uplink {
        if !self.active.load(Ordering::Relaxed) {
            return Uplink::Main;
        }
        let arrived = Instant::now();
        let mut inner = self.inner.lock();
        match inner.router.uplink(packet, arrived) {
            Verdict::Main => Uplink::Main,
            Verdict::Drop => Uplink::Taken,
            Verdict::Route(route) => {
                if let Some(Some(queue)) = inner.queues.get(usize::from(route.0)) {
                    // A full queue drops the packet: the route session is
                    // behind, and main must never carry it instead.
                    let _ = queue.try_send(std::mem::take(packet));
                }
                Uplink::Taken
            }
        }
    }

    fn downlink(&self, route: RouteId, packet: &mut [u8]) -> Delivery {
        self.inner
            .lock()
            .router
            .downlink(route, packet, Instant::now())
    }
}

/// The main session's view of the TUN device.
pub struct RoutedTun<D, R> {
    device: D,
    table: Arc<RoutingTable<R>>,
}

impl<D: Clone, R> Clone for RoutedTun<D, R> {
    fn clone(&self) -> Self {
        Self {
            device: self.device.clone(),
            table: Arc::clone(&self.table),
        }
    }
}

impl<D, R> RoutedTun<D, R> {
    pub fn new(device: D, table: Arc<RoutingTable<R>>) -> Self {
        Self { device, table }
    }
}

impl<D, R> PacketDevice for RoutedTun<D, R>
where
    D: PacketDevice,
    R: OwnerResolver + Send + 'static,
{
    async fn recv(&self) -> io::Result<Vec<u8>> {
        loop {
            let mut packet = self.device.recv().await?;
            if self.table.uplink(&mut packet) == Uplink::Main {
                return Ok(packet);
            }
        }
    }

    async fn send(&self, packet: &[u8]) -> io::Result<()> {
        self.device.send(packet).await
    }

    fn try_recv(&self) -> io::Result<Option<Vec<u8>>> {
        while let Some(mut packet) = self.device.try_recv()? {
            if self.table.uplink(&mut packet) == Uplink::Main {
                return Ok(Some(packet));
            }
        }
        Ok(None)
    }

    async fn send_batch(&self, packets: &mut Vec<Vec<u8>>) -> io::Result<usize> {
        self.device.send_batch(packets).await
    }

    async fn send_batch_bytes(&self, packets: &mut Vec<bytes::Bytes>) -> io::Result<usize> {
        self.device.send_batch_bytes(packets).await
    }
}

/// A route session's view of the TUN device.
pub struct RouteTun<D, R> {
    device: D,
    table: Arc<RoutingTable<R>>,
    route: RouteId,
    queue: Arc<tokio::sync::Mutex<mpsc::Receiver<Vec<u8>>>>,
}

impl<D: Clone, R> Clone for RouteTun<D, R> {
    fn clone(&self) -> Self {
        Self {
            device: self.device.clone(),
            table: Arc::clone(&self.table),
            route: self.route,
            queue: Arc::clone(&self.queue),
        }
    }
}

impl<D, R> RouteTun<D, R> {
    /// The device of `route`, reading `queue` (from
    /// [`RoutingTable::open_route`]) and writing to `device`.
    pub fn new(
        device: D,
        table: Arc<RoutingTable<R>>,
        route: RouteId,
        queue: mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            device,
            table,
            route,
            queue: Arc::new(tokio::sync::Mutex::new(queue)),
        }
    }
}

impl<D, R> PacketDevice for RouteTun<D, R>
where
    D: PacketDevice,
    R: OwnerResolver + Send + 'static,
{
    async fn recv(&self) -> io::Result<Vec<u8>> {
        self.queue
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "the route's queue is closed"))
    }

    async fn send(&self, packet: &[u8]) -> io::Result<()> {
        let mut packet = packet.to_vec();
        match self.table.downlink(self.route, &mut packet) {
            Delivery::Deliver => self.device.send(&packet).await,
            Delivery::Drop => Ok(()),
        }
    }

    fn try_recv(&self) -> io::Result<Option<Vec<u8>>> {
        let Ok(mut queue) = self.queue.try_lock() else {
            return Ok(None);
        };
        match queue.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the route's queue is closed",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::Ordering, time::Duration};

    use talpid_app_routing::router::SessionAddresses;
    use warrenguard_transport_core::FakeTun;

    use super::*;
    use crate::app_routes::test_support::*;

    const ROUTE_0: RouteId = RouteId(0);

    fn addresses(v4: std::net::Ipv4Addr) -> SessionAddresses {
        SessionAddresses {
            v4: Some(v4),
            v6: None,
        }
    }

    fn browser_on_route(state: RouteState) -> Policy {
        Policy::new(addresses(MAIN), [(BROWSER, ROUTE_0)], vec![state]).unwrap()
    }

    fn table() -> (Arc<RoutingTable<TwoApps>>, TwoApps) {
        let os = TwoApps::default();
        (RoutingTable::new(os.clone()), os)
    }

    async fn within_a_second<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(1), future)
            .await
            .expect("completed within a second")
    }

    async fn nothing_within_a_moment<D: PacketDevice>(device: &D) -> bool {
        tokio::time::timeout(Duration::from_millis(50), device.recv())
            .await
            .is_err()
    }

    #[tokio::test]
    async fn an_inactive_table_hands_every_packet_to_main_without_asking_the_os() {
        let (table, os) = table();
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), table);
        let packet = syn(BROWSER_PORT);

        tun.inject_inbound(packet.clone());

        assert_eq!(within_a_second(routed.recv()).await.unwrap(), packet);
        assert_eq!(os.calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn a_routed_apps_packet_leaves_main_for_its_route_queue_translated() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let queue = table.open_route(ROUTE_0);
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), Arc::clone(&table));
        let route = RouteTun::new(tun.clone(), table, ROUTE_0, queue);

        tun.inject_inbound(syn(BROWSER_PORT));

        assert!(nothing_within_a_moment(&routed).await);
        let queued = tokio::time::timeout(Duration::from_secs(2), route.recv())
            .await
            .expect("the route queue received the packet")
            .unwrap();
        assert_eq!(source(&queued), ROUTE);
    }

    #[tokio::test]
    async fn an_unrouted_apps_packet_stays_on_main_untouched() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), table);
        let packet = syn(EDITOR_PORT);

        tun.inject_inbound(packet.clone());

        assert_eq!(within_a_second(routed.recv()).await.unwrap(), packet);
    }

    #[tokio::test]
    async fn a_routed_apps_packet_is_dropped_while_its_route_is_connecting() {
        let (table, _os) = table();
        table.set_policy(browser_on_route(RouteState::Connecting), true);
        let queue = table.open_route(ROUTE_0);
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), Arc::clone(&table));
        let route = RouteTun::new(tun.clone(), Arc::clone(&table), ROUTE_0, queue);

        tun.inject_inbound(syn(BROWSER_PORT));

        assert!(nothing_within_a_moment(&routed).await);
        assert!(route.try_recv().unwrap().is_none());
        assert_eq!(table.counters().dropped_packets, 1);
    }

    #[tokio::test]
    async fn a_closed_route_drops_its_apps_packets_instead_of_handing_them_to_main() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let _queue = table.open_route(ROUTE_0);
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), Arc::clone(&table));

        table.close_route(ROUTE_0);
        tun.inject_inbound(syn(BROWSER_PORT));

        assert!(nothing_within_a_moment(&routed).await);
        assert_eq!(table.counters().dropped_packets, 1);
    }

    #[tokio::test]
    async fn a_route_answer_is_translated_back_and_written_to_the_tun() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let queue = table.open_route(ROUTE_0);
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), Arc::clone(&table));
        let route = RouteTun::new(tun.clone(), table, ROUTE_0, queue);
        tun.inject_inbound(syn(BROWSER_PORT));
        assert!(nothing_within_a_moment(&routed).await);

        route.send(&syn_ack_to(ROUTE, BROWSER_PORT)).await.unwrap();

        let written = tun.take_outbound();
        assert_eq!(written.len(), 1);
        assert_eq!(destination(&written[0]), MAIN);
    }

    #[tokio::test]
    async fn a_route_cannot_write_to_the_tun_what_is_not_one_of_its_flows() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let queue = table.open_route(ROUTE_0);
        let tun = FakeTun::new();
        let route = RouteTun::new(tun.clone(), table, ROUTE_0, queue);

        route.send(&syn_ack_to(ROUTE, BROWSER_PORT)).await.unwrap();

        assert!(tun.take_outbound().is_empty());
    }

    #[tokio::test]
    async fn the_main_downlink_is_written_to_the_tun_untouched() {
        let (table, _os) = table();
        table.set_policy(
            browser_on_route(RouteState::Connected(addresses(ROUTE))),
            true,
        );
        let tun = FakeTun::new();
        let routed = RoutedTun::new(tun.clone(), table);
        let answer = syn_ack_to(MAIN, EDITOR_PORT);

        routed.send(&answer).await.unwrap();

        assert_eq!(tun.take_outbound(), vec![answer]);
    }
}
