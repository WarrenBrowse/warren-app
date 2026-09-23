//! The firewall opening a gap-free migration needs before it dials.
//!
//! The desktop firewall lets the tunnel reach, outside the tunnel, only the
//! relay it names: the one the tunnel was started with. The overlap dial of a
//! migration (ADR 36) to any other relay is dropped by it, and the session then
//! leaves a draining exit only when that exit closes it, with an outage. So a
//! migration first names its target relay next to the one in use, waits until
//! the firewall took it, and only then dials; once the session has landed, the
//! firewall names the relay it landed on alone.
//!
//! The opening is the one the relay in use already has: the tunnel's own
//! sockets (root, or the daemon executable on Windows) to a relay of the
//! verified directory. No traffic of the user's leaves outside the tunnel
//! through it.

use std::sync::{Arc, Mutex, Weak};

use futures::future::BoxFuture;
use talpid_types::net::{Endpoint, TransportProtocol};
use warrenguard_multihop::RelayDescriptorSigned;
use warrenguard_transport::supervisor::{CircuitTarget, OverlapSwapObserver};

/// Hands the firewall the complete set of relay endpoints the tunnel may reach
/// outside the tunnel, and resolves once it applied them.
pub(crate) type PeerSink = Arc<dyn Fn(Vec<Endpoint>) -> BoxFuture<'static, ()> + Send + Sync>;

/// The relays the firewall names for this tunnel.
pub(crate) struct PeerFence {
    peers: Mutex<Peers>,
    sink: PeerSink,
}

struct Peers {
    /// The relay the session is on.
    landed: Vec<Endpoint>,
    /// The relays of migrations dialled since it landed there.
    pending: Vec<Endpoint>,
}

impl PeerFence {
    pub(crate) fn new(in_use: &RelayDescriptorSigned, sink: PeerSink) -> Self {
        Self {
            peers: Mutex::new(Peers {
                landed: relay_endpoints(in_use),
                pending: Vec::new(),
            }),
            sink,
        }
    }

    /// Name `target` too, and resolve once the firewall took it. A migration
    /// requested before an earlier one landed keeps that one's relay named:
    /// the supervisor may still be dialling it.
    pub(crate) async fn open_for(&self, target: &RelayDescriptorSigned) {
        let named = {
            let mut peers = self.lock();
            for endpoint in relay_endpoints(target) {
                if !peers.landed.contains(&endpoint) && !peers.pending.contains(&endpoint) {
                    peers.pending.push(endpoint);
                }
            }
            peers.landed.iter().chain(&peers.pending).copied().collect()
        };
        (self.sink)(named).await;
    }

    /// The session landed on `relay`: name it alone.
    pub(crate) async fn landed_on(&self, relay: &RelayDescriptorSigned) {
        let named = {
            let mut peers = self.lock();
            peers.landed = relay_endpoints(relay);
            peers.pending.clear();
            peers.landed.clone()
        };
        (self.sink)(named).await;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Peers> {
        self.peers.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Every endpoint the tunnel dials `relay` on: both address families it
/// publishes, over the UDP carrier and the TCP fallback.
pub(crate) fn relay_endpoints(relay: &RelayDescriptorSigned) -> Vec<Endpoint> {
    std::iter::once(relay.endpoint)
        .chain(relay.endpoint_v6)
        .flat_map(|addr| {
            [TransportProtocol::Udp, TransportProtocol::Tcp]
                .into_iter()
                .map(move |proto| Endpoint::from_socket_address(addr, proto))
        })
        .collect()
}

/// The supervisor's swap observer: narrow the firewall to the relay the
/// session landed on, then hand the swap to `then` (the daemon's observer).
pub(crate) fn swap_observer(
    fence: Arc<PeerFence>,
    runtime: tokio::runtime::Handle,
    then: Option<OverlapSwapObserver>,
) -> OverlapSwapObserver {
    Arc::new(move |landed: &CircuitTarget| {
        let fence = Arc::clone(&fence);
        let relay = Arc::clone(&landed.relay);
        runtime.spawn(async move { fence.landed_on(&relay).await });
        if let Some(then) = then.as_ref() {
            then(landed);
        }
    })
}

/// The supervisor's migrate handle behind the [`PeerFence`]: every migration,
/// whether off a draining exit or off a refusing entry, opens the firewall for
/// its target before the overlap dial starts.
///
/// The daemon keeps it across tunnels, so it holds the fence weakly: the fence
/// carries the tunnel's event sender, and the state machine reads the end of
/// that channel as the tunnel going down.
#[derive(Clone)]
pub struct WarrenMigrateHandle {
    fence: Weak<PeerFence>,
    migrate: Arc<dyn Fn(CircuitTarget) + Send + Sync>,
    runtime: tokio::runtime::Handle,
}

impl WarrenMigrateHandle {
    pub(crate) fn new(
        fence: &Arc<PeerFence>,
        migrate: Arc<dyn Fn(CircuitTarget) + Send + Sync>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self {
            fence: Arc::downgrade(fence),
            migrate,
            runtime,
        }
    }

    /// Gap-free migration onto `target` (`MigrateHandle::migrate_to`), once
    /// the firewall names its relay. Returns at once; the dial starts on the
    /// tunnel's runtime, and a tunnel already torn down never starts it.
    pub fn migrate_to(&self, target: CircuitTarget) {
        let Some(fence) = self.fence.upgrade() else {
            return;
        };
        let migrate = Arc::clone(&self.migrate);
        self.runtime.spawn(async move {
            fence.open_for(&target.relay).await;
            migrate(target);
        });
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    fn relay(v4: &str, v6: Option<&str>) -> RelayDescriptorSigned {
        RelayDescriptorSigned {
            relay_id: [0x11; 16],
            relay_ed25519_pubkey: [0x22; 32],
            endpoint: v4.parse().unwrap(),
            endpoint_v6: v6.map(|v6| v6.parse().unwrap()),
            signature: [0x33; 64],
            cover_domain: None,
            tcp_fallback: false,
        }
    }

    fn target(relay: RelayDescriptorSigned) -> CircuitTarget {
        CircuitTarget {
            relay: Arc::new(relay),
            exit_id: warrenguard_multihop::ExitId::from_bytes([0x44; 16]),
            exit_x25519_multihop_pubkey: [0x55; 32],
            exit_mlkem768_pubkey: None,
        }
    }

    fn udp_and_tcp(addr: &str) -> Vec<Endpoint> {
        let addr: SocketAddr = addr.parse().unwrap();
        vec![
            Endpoint::from_socket_address(addr, TransportProtocol::Udp),
            Endpoint::from_socket_address(addr, TransportProtocol::Tcp),
        ]
    }

    /// What reached the firewall and the supervisor, in order.
    #[derive(Debug, PartialEq, Eq)]
    enum Step {
        Named(Vec<Endpoint>),
        Applied,
        Dialled,
    }

    type Log = Arc<Mutex<Vec<Step>>>;

    /// A firewall that takes a scheduling round to apply what it is handed.
    fn firewall(log: &Log) -> PeerSink {
        let log = Arc::clone(log);
        Arc::new(move |peers| {
            let log = Arc::clone(&log);
            Box::pin(async move {
                log.lock().unwrap().push(Step::Named(peers));
                tokio::task::yield_now().await;
                log.lock().unwrap().push(Step::Applied);
            })
        })
    }

    #[tokio::test]
    async fn a_migration_dials_only_once_the_firewall_names_its_target() {
        let log: Log = Arc::default();
        let fence = Arc::new(PeerFence::new(
            &relay("198.51.100.1:443", None),
            firewall(&log),
        ));
        let (dialled, dial) = tokio::sync::oneshot::channel();
        let dialled = Mutex::new(Some(dialled));
        let dial_log = Arc::clone(&log);
        let handle = WarrenMigrateHandle::new(
            &fence,
            Arc::new(move |_target| {
                dial_log.lock().unwrap().push(Step::Dialled);
                if let Some(done) = dialled.lock().unwrap().take() {
                    let _ = done.send(());
                }
            }),
            tokio::runtime::Handle::current(),
        );

        handle.migrate_to(target(relay("198.51.100.2:443", None)));
        dial.await.expect("the migration dials");

        let mut both = udp_and_tcp("198.51.100.1:443");
        both.extend(udp_and_tcp("198.51.100.2:443"));
        assert_eq!(
            *log.lock().unwrap(),
            vec![Step::Named(both), Step::Applied, Step::Dialled]
        );
    }

    #[tokio::test]
    async fn once_landed_the_firewall_names_only_the_relay_the_session_is_on() {
        let log: Log = Arc::default();
        let fence = PeerFence::new(&relay("198.51.100.1:443", None), firewall(&log));
        let second = relay("198.51.100.2:443", None);
        fence.open_for(&second).await;
        log.lock().unwrap().clear();

        fence.landed_on(&second).await;

        assert_eq!(
            *log.lock().unwrap(),
            vec![Step::Named(udp_and_tcp("198.51.100.2:443")), Step::Applied]
        );
    }

    #[tokio::test]
    async fn a_migration_requested_before_the_last_one_landed_keeps_it_reachable() {
        // The supervisor dials the last target it was given, and may already
        // be dialling the previous one.
        let log: Log = Arc::default();
        let fence = PeerFence::new(&relay("198.51.100.1:443", None), firewall(&log));
        fence.open_for(&relay("198.51.100.2:443", None)).await;
        log.lock().unwrap().clear();

        fence.open_for(&relay("198.51.100.3:443", None)).await;

        let mut all = udp_and_tcp("198.51.100.1:443");
        all.extend(udp_and_tcp("198.51.100.2:443"));
        all.extend(udp_and_tcp("198.51.100.3:443"));
        assert_eq!(*log.lock().unwrap(), vec![Step::Named(all), Step::Applied]);
    }

    #[tokio::test]
    async fn a_committed_swap_narrows_the_firewall_and_still_reaches_the_daemon() {
        let log: Log = Arc::default();
        let fence = Arc::new(PeerFence::new(
            &relay("198.51.100.1:443", None),
            firewall(&log),
        ));
        let (seen, observed) = tokio::sync::oneshot::channel();
        let seen = Mutex::new(Some(seen));
        let daemon: OverlapSwapObserver = Arc::new(move |landed: &CircuitTarget| {
            if let Some(seen) = seen.lock().unwrap().take() {
                let _ = seen.send(landed.relay.endpoint);
            }
        });
        let observer = swap_observer(
            Arc::clone(&fence),
            tokio::runtime::Handle::current(),
            Some(daemon),
        );

        observer(&target(relay("198.51.100.2:443", None)));

        let landed = tokio::time::timeout(std::time::Duration::from_secs(5), observed)
            .await
            .expect("the daemon's observer must run")
            .unwrap();
        assert_eq!(landed, "198.51.100.2:443".parse::<SocketAddr>().unwrap());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while log.lock().unwrap().last() != Some(&Step::Applied) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the firewall must be narrowed");
        assert_eq!(
            *log.lock().unwrap(),
            vec![Step::Named(udp_and_tcp("198.51.100.2:443")), Step::Applied]
        );
    }

    #[tokio::test]
    async fn a_handle_the_daemon_keeps_holds_nothing_of_a_torn_down_tunnel() {
        // The fence holds the tunnel's event sender: kept alive by the handle,
        // the state machine would never see the tunnel's events end.
        let events_sender = Arc::new(());
        let held = Arc::clone(&events_sender);
        let fence = Arc::new(PeerFence::new(
            &relay("198.51.100.1:443", None),
            Arc::new(move |_peers| {
                let _held = &held;
                Box::pin(async {})
            }),
        ));
        let dialled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dial = Arc::clone(&dialled);
        let handle = WarrenMigrateHandle::new(
            &fence,
            Arc::new(move |_target| dial.store(true, std::sync::atomic::Ordering::SeqCst)),
            tokio::runtime::Handle::current(),
        );

        drop(fence);
        assert_eq!(Arc::strong_count(&events_sender), 1);
        handle.migrate_to(target(relay("198.51.100.2:443", None)));
        tokio::task::yield_now().await;

        assert!(!dialled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_relay_is_reached_on_both_families_and_both_carriers() {
        let mut expected = udp_and_tcp("198.51.100.1:443");
        expected.extend(udp_and_tcp("[2001:db8::1]:443"));

        assert_eq!(
            relay_endpoints(&relay("198.51.100.1:443", Some("[2001:db8::1]:443"))),
            expected
        );
    }
}
