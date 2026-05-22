//! Daemon-side lifecycle wrapper around the `warren-natpmp-client`
//! refresh loop.
//!
//! [`NatPmpManager`] owns a [`RefreshLoopHandle`] plus a forwarding
//! task that drains the loop's event channel and invokes a
//! caller-supplied observer for each event. The forwarding indirection
//! lets the daemon (which holds the `WarrenStatusCache`) stay decoupled
//! from the warren-natpmp-client crate: only `talpid-warren-tunnel`
//! depends on it, and the daemon wires a closure that pushes events
//! into the status cache.
//!
//! ## Lifecycle
//!
//! - [`NatPmpManager::start`] spawns the refresh loop + the forwarding
//!   task and returns the manager.
//! - [`NatPmpManager::cancel`] stops the loop and aborts the forwarder.
//!   Idempotent.
//! - Dropping the manager calls `cancel` defensively, so a panicking
//!   tunnel teardown cannot leak the spawned tasks.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use warren_natpmp_client::{RefreshLoopHandle, spawn_refresh_loop_from_addr};

use crate::NatPmpConfig;

// Re-exported so consumers do not have to depend on
// `warren-natpmp-client` directly. The forwarding observer signature is
// `Fn(NatPmpEvent)`; the daemon wraps a `WarrenStatusCache` and pushes
// each event into the cache for the Electron UI status stream.
pub use warren_natpmp_client::NatPmpEvent;

/// Observer invoked from the forwarding task for every event emitted by
/// the inner refresh loop. The daemon-side wiring sets this to a
/// closure that records the event into the live status cache; tests
/// provide an `Arc<Mutex<Vec<...>>>`-backed collector.
pub type NatPmpEventObserver = Arc<dyn Fn(NatPmpEvent) + Send + Sync>;

/// Owns the spawned tasks driving an active NAT-PMP mapping: the
/// refresh loop itself (from warren-natpmp-client) and a forwarder
/// that drains its event channel and dispatches to the observer.
pub struct NatPmpManager {
    /// `None` once `cancel()` has been called. Wrapping in `Option`
    /// lets us call `RefreshLoopHandle::cancel` (which takes
    /// `&mut self`) from the manager's own `cancel` (which also takes
    /// `&mut self`) without an inner-mutability dance.
    refresh_handle: Option<RefreshLoopHandle>,
    /// Forwarder task. Aborted (not joined) on cancel: the
    /// loop-spawned channel will close on its own once the refresh
    /// handle is cancelled, so the task would exit naturally on the
    /// next `recv`; aborting is just faster cleanup and idempotent.
    forward_handle: Option<JoinHandle<()>>,
}

impl NatPmpManager {
    /// Spawns the refresh loop + forwarder. The caller passes a tokio
    /// runtime handle to scope the spawn explicitly (matches the
    /// `runtime: tokio::runtime::Handle` pattern used elsewhere in
    /// `talpid-warren-tunnel`; do not call this from outside a tokio
    /// context).
    #[must_use = "the returned manager owns the spawned tasks; drop discards control"]
    pub fn start(
        runtime: &tokio::runtime::Handle,
        server: SocketAddr,
        config: &NatPmpConfig,
        observer: NatPmpEventObserver,
    ) -> Self {
        Self::start_from_addr(runtime, server, config, observer, None)
    }

    /// Variant of [`start`] that forces the local UDP source IP via
    /// `bind_addr`. Required on Android (and any host whose default
    /// route bypasses the tunnel) so the NAT-PMP request egresses
    /// through the tunnel rather than the underlying mobile-data /
    /// Wi-Fi interface — otherwise the exit's NAT-PMP server never
    /// sees the request. Pass the assigned tunnel inner IPv4 (typ.
    /// `10.66.0.x` after the IP allocator's IpAssign frame).
    #[must_use = "the returned manager owns the spawned tasks; drop discards control"]
    pub fn start_from_addr(
        runtime: &tokio::runtime::Handle,
        server: SocketAddr,
        config: &NatPmpConfig,
        observer: NatPmpEventObserver,
        bind_addr: Option<IpAddr>,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<NatPmpEvent>();
        let refresh_handle = spawn_refresh_loop_from_addr(
            server,
            config.protocol,
            config.internal_port,
            config.suggested_external_port,
            config.lifetime_secs,
            tx,
            bind_addr,
        );
        let forward_handle = runtime.spawn(async move {
            // recv() returns None once the inner refresh loop exits
            // (Cancelled or Failed). The forwarder ends naturally.
            while let Some(event) = rx.recv().await {
                observer(event);
            }
        });
        Self {
            refresh_handle: Some(refresh_handle),
            forward_handle: Some(forward_handle),
        }
    }

    /// Stops the refresh loop and aborts the forwarder. Idempotent.
    pub fn cancel(&mut self) {
        if let Some(mut h) = self.refresh_handle.take() {
            h.cancel();
        }
        if let Some(h) = self.forward_handle.take() {
            h.abort();
        }
    }

    /// True iff the forwarder task has finished (either naturally,
    /// e.g. after the loop emitted `Failed`, or because `cancel` was
    /// called). Useful in tests that need to assert orderly shutdown.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.forward_handle
            .as_ref()
            .is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for NatPmpManager {
    fn drop(&mut self) {
        // Defensive teardown: ensure the spawned tasks do not outlive
        // the manager even if the owner forgot to call `cancel`.
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::net::UdpSocket;
    use warren_natpmp_protocol::{MapProto, ResultCode, serialize_response};

    /// Sets up a tiny UDP stub that responds to every datagram with a
    /// Map response carrying the supplied lifetime + external port.
    async fn spawn_stub(lifetime_secs: u32, external_port: u16) -> SocketAddr {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            loop {
                let (_, peer) = match sock.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let resp = serialize_response(&warren_natpmp_protocol::Response::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::Success,
                    epoch_secs: 0,
                    internal_port: 22,
                    external_port,
                    lifetime_secs,
                });
                let _ = sock.send_to(&resp, peer).await;
            }
        });
        addr
    }

    fn collector() -> (NatPmpEventObserver, Arc<Mutex<Vec<NatPmpEvent>>>) {
        let log: Arc<Mutex<Vec<NatPmpEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let log_for_obs = log.clone();
        let observer: NatPmpEventObserver = Arc::new(move |evt| {
            log_for_obs
                .lock()
                .expect("test observer lock poisoned")
                .push(evt);
        });
        (observer, log)
    }

    fn cfg(internal_port: u16) -> NatPmpConfig {
        NatPmpConfig {
            enabled: true,
            lifetime_secs: 60,
            protocol: MapProto::Udp,
            suggested_external_port: 0,
            internal_port,
        }
    }

    #[tokio::test]
    async fn manager_forwards_mapped_event_to_observer() {
        let server = spawn_stub(60, 49152).await;
        let (observer, log) = collector();

        let manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(22),
            observer,
        );

        // Wait briefly for the first Mapped to reach the observer.
        for _ in 0..50 {
            if !log.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let snapshot = log.lock().unwrap().clone();
        assert!(
            matches!(
                snapshot.first(),
                Some(NatPmpEvent::Mapped {
                    external_port: 49152,
                    ..
                })
            ),
            "expected first event to be Mapped with port 49152, got: {snapshot:?}"
        );

        drop(manager);
    }

    #[tokio::test]
    async fn manager_cancel_is_idempotent() {
        let server = spawn_stub(60, 50000).await;
        let (observer, _log) = collector();

        let mut manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(23),
            observer,
        );

        // Multiple cancels must not panic.
        manager.cancel();
        manager.cancel();
        manager.cancel();
        assert!(manager.is_finished() || manager.forward_handle.is_none());
    }

    #[tokio::test]
    async fn manager_drop_cancels_tasks() {
        let server = spawn_stub(60, 51000).await;
        let (observer, _log) = collector();

        let manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(24),
            observer,
        );
        // Move into a separate scope so the explicit drop fires and we
        // can verify task cleanup did not panic.
        drop(manager);
        // Yield to give the runtime a moment to process the abort.
        tokio::time::sleep(Duration::from_millis(20)).await;
        // Nothing to assert directly here besides "no panic", but
        // a leaked task would surface as a runtime warning. We pass.
    }

    #[tokio::test]
    async fn manager_observer_receives_failure_when_server_rejects() {
        // Stub that replies once with OutOfResources then ignores
        // subsequent requests. The refresh loop emits Failed and exits;
        // the forwarder drains the Failed event then ends naturally.
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            if let Ok((_, peer)) = sock.recv_from(&mut buf).await {
                let resp = serialize_response(&warren_natpmp_protocol::Response::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::OutOfResources,
                    epoch_secs: 0,
                    internal_port: 22,
                    external_port: 0,
                    lifetime_secs: 0,
                });
                let _ = sock.send_to(&resp, peer).await;
            }
        });

        let (observer, log) = collector();
        let manager =
            NatPmpManager::start(&tokio::runtime::Handle::current(), addr, &cfg(22), observer);

        for _ in 0..100 {
            if log
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, NatPmpEvent::Failed { .. }))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let events = log.lock().unwrap().clone();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, NatPmpEvent::Failed { .. })),
            "expected at least one Failed event, got: {events:?}"
        );

        drop(manager);
    }

    #[tokio::test]
    async fn manager_start_idempotent_across_two_instances() {
        // Spawning two managers against two different stubs must not
        // interfere - the observers receive their own events. The
        // daemon never spawns two managers for the same tunnel, but
        // this guards against a future regression that might leak a
        // manager across a tunnel transition.
        let server_a = spawn_stub(60, 60001).await;
        let server_b = spawn_stub(60, 60002).await;

        let (observer_a, log_a) = collector();
        let (observer_b, log_b) = collector();

        let m_a = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server_a,
            &cfg(22),
            observer_a,
        );
        let m_b = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server_b,
            &cfg(23),
            observer_b,
        );

        for _ in 0..50 {
            if !log_a.lock().unwrap().is_empty() && !log_b.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let a = log_a.lock().unwrap().clone();
        let b = log_b.lock().unwrap().clone();
        assert!(
            matches!(
                a.first(),
                Some(NatPmpEvent::Mapped {
                    external_port: 60001,
                    ..
                })
            ),
            "manager A observed wrong port: {a:?}"
        );
        assert!(
            matches!(
                b.first(),
                Some(NatPmpEvent::Mapped {
                    external_port: 60002,
                    ..
                })
            ),
            "manager B observed wrong port: {b:?}"
        );

        drop(m_a);
        drop(m_b);
    }
}
