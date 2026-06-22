//! Daemon-side lifecycle wrapper around the `warrenguard-natpmp-client`
//! refresh loop.
//!
//! [`NatPmpManager`] owns a [`RefreshLoopHandle`] plus a forwarding
//! task that drains the loop's event channel and invokes a
//! caller-supplied observer for each event. The forwarding indirection
//! lets the daemon (which holds the `WarrenStatusCache`) stay decoupled
//! from the warrenguard-natpmp-client crate: only `talpid-warren-tunnel`
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
use warrenguard_natpmp_client::{RefreshLoopHandle, spawn_refresh_loop_from_addr};

use crate::NatPmpConfig;

// Re-exported so consumers do not have to depend on
// `warrenguard-natpmp-client` directly. The forwarding observer signature is
// `Fn(NatPmpEvent)`; the daemon wraps a `WarrenStatusCache` and pushes
// each event into the cache for the Electron UI status stream.
pub use warrenguard_natpmp_client::{NatPmpEvent, NatPmpFailureReason};

/// Observer invoked from the forwarding task for every event emitted by
/// the inner refresh loop. The daemon-side wiring sets this to a
/// closure that records the event into the live status cache; tests
/// provide an `Arc<Mutex<Vec<...>>>`-backed collector.
pub type NatPmpEventObserver = Arc<dyn Fn(NatPmpEvent) + Send + Sync>;

/// Owns the spawned tasks driving an active NAT-PMP mapping: the
/// refresh loop itself (from warrenguard-natpmp-client) and a forwarder
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
    /// Connection parameters retained for `reconfigure` so the
    /// manager can spawn a new refresh loop against the same exit
    /// without the caller having to thread them through again.
    /// Captured once at [`start_from_addr`] time.
    server: SocketAddr,
    bind_addr: Option<IpAddr>,
    runtime: tokio::runtime::Handle,
    /// Cloned and re-used on every reconfigure so the daemon-side
    /// `WarrenStatusCache` keeps receiving events from the new loop
    /// without the caller having to re-wire the observer plumbing.
    observer: NatPmpEventObserver,
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
    /// Wi-Fi interface - otherwise the exit's NAT-PMP server never
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
        let (refresh_handle, forward_handle) =
            Self::spawn_refresh_pair(runtime, server, config, &observer, bind_addr);
        Self {
            refresh_handle: Some(refresh_handle),
            forward_handle: Some(forward_handle),
            server,
            bind_addr,
            runtime: runtime.clone(),
            observer,
        }
    }

    /// Internal helper that mirrors the spawn pair used by both
    /// [`start_from_addr`] (initial construction) and [`reconfigure`]
    /// (live-swap). Centralises the channel + observer wiring so the
    /// two call sites stay byte-for-byte equivalent.
    fn spawn_refresh_pair(
        runtime: &tokio::runtime::Handle,
        server: SocketAddr,
        config: &NatPmpConfig,
        observer: &NatPmpEventObserver,
        bind_addr: Option<IpAddr>,
    ) -> (RefreshLoopHandle, JoinHandle<()>) {
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
        let observer = observer.clone();
        let forward_handle = runtime.spawn(async move {
            // recv() returns None once the inner refresh loop exits
            // (Cancelled or Failed). The forwarder ends naturally.
            while let Some(event) = rx.recv().await {
                observer(event);
            }
        });
        (refresh_handle, forward_handle)
    }

    /// Atomically swap the mapping over to `new_config` without
    /// requiring the tunnel to reconnect.
    ///
    /// Sequence:
    /// 1. Release the current mapping via
    ///    [`RefreshLoopHandle::release`] (sends a `lifetime = 0` Map
    ///    so the exit's per-client quota slot is freed before the
    ///    new allocation arrives - otherwise the new request hits
    ///    `QuotaExceeded` and fails until the old lease GCs at
    ///    expiry, ~1 h).
    /// 2. Abort the forwarder task. The release step already
    ///    drained the loop's event channel (it emitted `Cancelled`
    ///    after the cancel signal), so the abort is just defensive
    ///    cleanup.
    /// 3. Spawn a fresh refresh loop + forwarder pair against
    ///    `new_config`, re-using the same observer so daemon-side
    ///    plumbing (status cache, IPC stream to the renderer) is
    ///    unaware of the swap.
    ///
    /// The observer will receive a fresh `Mapped` event for the new
    /// allocation, which the UI surfaces as the new public port. No
    /// `Cancelled` is propagated to the observer for the *old* loop -
    /// the daemon's UI state model treats the live-swap as "the
    /// mapping moved to new params" rather than "the mapping
    /// disappeared and reappeared".
    pub async fn reconfigure(&mut self, new_config: &NatPmpConfig) {
        // Order matters: ABORT the forwarder FIRST so it can't
        // dispatch the soon-to-be-emitted `Cancelled` event from
        // the old loop to the observer. If we released first, the
        // refresh loop would emit `Cancelled` on the event channel,
        // the forwarder would still be alive, and the observer
        // would briefly see `disabled` between reconfigures -
        // visible in the UI as a status flicker (and as a
        // brief 'disabled' write to the daemon-side
        // WarrenStatusCache).
        if let Some(h) = self.forward_handle.take() {
            h.abort();
        }
        // Step 2: release the live mapping cleanly. `release()`
        // calls `cancel()` internally (the loop's Cancelled `send`
        // now fails silently because the forwarder is gone, which
        // is fine) and then sends a `lifetime = 0` Map so the
        // exit's per-client quota slot frees up immediately -
        // without this, the next allocation hits `QuotaExceeded`
        // and is refused until the old lease GCs (~1 h).
        if let Some(mut handle) = self.refresh_handle.take() {
            handle.release().await;
        }
        // Step 3: spawn the new pair with the same observer. The
        // observer next sees a fresh `Mapped` event with the new
        // params' granted port.
        let (refresh_handle, forward_handle) = Self::spawn_refresh_pair(
            &self.runtime,
            self.server,
            new_config,
            &self.observer,
            self.bind_addr,
        );
        self.refresh_handle = Some(refresh_handle);
        self.forward_handle = Some(forward_handle);
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

    /// Graceful shutdown variant: sends a `lifetime = 0` Map to the
    /// exit so the per-client quota slot frees immediately, then
    /// cancels the refresh loop and aborts the forwarder. Use this
    /// (instead of [`cancel`] or `drop`) when the caller wants the
    /// exit to free the slot before the natural lease expiry -
    /// typically the live-reconfig controller path when the user
    /// disables port forwarding.
    ///
    /// Idempotent: a second call (or a call after `cancel`) is a
    /// no-op-ish - the lifetime=0 Map still fires but the exit
    /// silently ignores requests for unknown mappings, and the
    /// internal Option<…> fields are taken to None on first call so
    /// later cancels do nothing.
    pub async fn release(&mut self) {
        // Order mirrors `reconfigure`: forwarder first so the
        // refresh-loop's `Cancelled` event from `release()` does not
        // propagate to the observer (which would briefly flip the
        // UI to "disabled" - fine here because the next thing the
        // controller does on `Disable` is set the cache to
        // Disabled anyway, but we still keep the order consistent
        // for predictability).
        if let Some(h) = self.forward_handle.take() {
            h.abort();
        }
        if let Some(mut h) = self.refresh_handle.take() {
            h.release().await;
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
    use warrenguard_natpmp_protocol::{MapProto, ResultCode, serialize_response};

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
                let resp = serialize_response(&warrenguard_natpmp_protocol::Response::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::Success,
                    epoch_secs: 0,
                    internal_port: 22,
                    external_port,
                    lifetime_secs,
                    rate_limit: None,
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
            rules: Vec::new(),
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
                let resp = serialize_response(&warrenguard_natpmp_protocol::Response::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::OutOfResources,
                    epoch_secs: 0,
                    internal_port: 22,
                    external_port: 0,
                    lifetime_secs: 0,
                    rate_limit: None,
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

    /// Stub that replies with a configurable external port determined
    /// by the request's lifetime - lets the test infer which Map
    /// request the response is for by reading the granted port back.
    /// Used by the reconfigure tests to assert that the new mapping
    /// kicks in (different port emitted by the stub after reconfigure).
    async fn spawn_port_per_lifetime_stub() -> SocketAddr {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            loop {
                let (n, peer) = match sock.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let lifetime = match warrenguard_natpmp_protocol::parse_request(&buf[..n]) {
                    Ok(warrenguard_natpmp_protocol::Request::Map { lifetime_secs, .. }) => {
                        lifetime_secs
                    }
                    _ => continue,
                };
                // Encode the requested lifetime into the granted
                // port so the test can correlate request → response
                // without leaking the request struct out of the
                // stub. Lifetime 0 = release => skip the reply
                // (mimics a server that doesn't echo deletes).
                if lifetime == 0 {
                    // Still echo a Success(0) so the client's
                    // release() future completes promptly instead
                    // of timing out.
                    let resp = serialize_response(&warrenguard_natpmp_protocol::Response::Map {
                        proto: MapProto::Udp,
                        result_code: ResultCode::Success,
                        epoch_secs: 0,
                        internal_port: 22,
                        external_port: 0,
                        lifetime_secs: 0,
                        rate_limit: None,
                    });
                    let _ = sock.send_to(&resp, peer).await;
                    continue;
                }
                let resp = serialize_response(&warrenguard_natpmp_protocol::Response::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::Success,
                    epoch_secs: 0,
                    internal_port: 22,
                    // 49000 + lifetime → 49060 for lifetime=60, 49120 for lifetime=120 ...
                    external_port: 49000_u16.saturating_add(lifetime as u16),
                    lifetime_secs: lifetime,
                    rate_limit: None,
                });
                let _ = sock.send_to(&resp, peer).await;
            }
        });
        addr
    }

    #[tokio::test]
    async fn reconfigure_releases_old_and_spawns_new_loop() {
        // Live-reconfig contract: after `reconfigure(new_cfg)` the
        // observer must (1) STOP seeing events from the old loop and
        // (2) start seeing events from the new loop with the new
        // params. The granted external port (encoded as `49000 +
        // lifetime`) is the easiest observable to verify the swap.
        let server = spawn_port_per_lifetime_stub().await;
        let (observer, log) = collector();

        let mut manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(22),
            observer,
        );

        // Wait for the initial Mapped with lifetime=60 → port 49060.
        for _ in 0..50 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e,
                    NatPmpEvent::Mapped {
                        external_port: 49060,
                        ..
                    }
                )
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            log.lock().unwrap().iter().any(|e| matches!(
                e,
                NatPmpEvent::Mapped {
                    external_port: 49060,
                    ..
                }
            )),
            "old mapping not observed before reconfigure"
        );

        // Reconfigure with lifetime=120 → expect a new Mapped with
        // port 49120.
        let new_cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 120,
            rules: Vec::new(),
            protocol: MapProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
        };
        manager.reconfigure(&new_cfg).await;

        // The new forwarder should emit a Mapped event with the new
        // port within a short window.
        for _ in 0..100 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e,
                    NatPmpEvent::Mapped {
                        external_port: 49120,
                        ..
                    }
                )
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let snapshot = log.lock().unwrap().clone();
        assert!(
            snapshot.iter().any(|e| matches!(
                e,
                NatPmpEvent::Mapped {
                    external_port: 49120,
                    ..
                }
            )),
            "expected a Mapped event for the new lifetime=120 mapping; events: {snapshot:?}"
        );

        drop(manager);
    }

    #[tokio::test]
    async fn reconfigure_suppresses_old_cancelled_event_in_observer() {
        // UX-critical: the old loop's `Cancelled` event must NOT
        // reach the observer, otherwise the daemon-side status
        // cache briefly flips to 'disabled' between reconfigures,
        // causing the UI to flicker. The forwarder abort in step 2
        // of `reconfigure` drops in-flight events from the old
        // channel before the observer can see them.
        let server = spawn_port_per_lifetime_stub().await;
        let (observer, log) = collector();

        let mut manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(22),
            observer,
        );

        // Wait for the initial Mapped.
        for _ in 0..50 {
            if !log.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let new_cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 120,
            rules: Vec::new(),
            protocol: MapProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
        };
        manager.reconfigure(&new_cfg).await;

        // Give the new loop time to emit the first Mapped.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let events = log.lock().unwrap().clone();
        assert!(
            !events.iter().any(|e| matches!(e, NatPmpEvent::Cancelled)),
            "reconfigure must NOT propagate Cancelled to the observer; events: {events:?}"
        );

        drop(manager);
    }

    #[tokio::test]
    async fn reconfigure_is_callable_multiple_times() {
        // A user that hammers the protocol radio button shouldn't
        // corrupt the manager state. Each reconfigure cleanly
        // releases the previous mapping and spawns a fresh one.
        let server = spawn_port_per_lifetime_stub().await;
        let (observer, log) = collector();

        let mut manager = NatPmpManager::start(
            &tokio::runtime::Handle::current(),
            server,
            &cfg(22),
            observer,
        );
        tokio::time::sleep(Duration::from_millis(100)).await;

        for lifetime in [120u32, 60, 90] {
            let cfg = NatPmpConfig {
                enabled: true,
                lifetime_secs: lifetime,
                rules: Vec::new(),
                protocol: MapProto::Udp,
                suggested_external_port: 0,
                internal_port: 22,
            };
            manager.reconfigure(&cfg).await;
        }

        // Wait for the LAST mapping (lifetime=90 → port 49090) to
        // surface.
        for _ in 0..100 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e,
                    NatPmpEvent::Mapped {
                        external_port: 49090,
                        ..
                    }
                )
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            log.lock().unwrap().iter().any(|e| matches!(
                e,
                NatPmpEvent::Mapped {
                    external_port: 49090,
                    ..
                }
            )),
            "final reconfigure must land its own Mapped event"
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
