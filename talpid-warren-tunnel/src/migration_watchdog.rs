//! Production IO for the engine's migration watchdog.
//!
//! The decision loop, its constants and the [`MigrationIo`] trait live in the
//! engine (`warrenguard_transport::migration_watchdog`), shared by every
//! client surface; this module supplies the desktop bindings:
//! [`RealWatchdogIo`] (talpid-routing route events, supervisor handle, DAITA
//! probe, pump-error escalation) and [`subscribe_route_events`].
//!
//! Per-platform carrier escape across a rebind, decided by [`rebind_policy`]
//! and applied by the engine's `rebind_wildcard`: Linux marks the carrier with
//! the Warren fwmark, so it follows the main table's updated default with
//! nothing to do; Windows reapplies `IP_UNICAST_IF` to the fresh socket, or
//! refuses the rebind outright when it has no bypass to apply; macOS holds a
//! per-socket `IP_BOUND_IF` bind that the rebind necessarily drops, so
//! [`MigrationIo::ensure_route_escape`] degrades it to the
//! `<carrier_ip>/32 DefaultNode` route first and the fresh socket stays
//! unbound. No escape, no rebind: the cycle redials rather than hand quinn a
//! socket that self-nests.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use talpid_routing::RouteManagerHandle;
use warrenguard_transport::bundle::MultiHopBundle;
pub(crate) use warrenguard_transport::migration_watchdog::run_watchdog;
use warrenguard_transport::migration_watchdog::{MigrationIo, RxSample};
use warrenguard_transport::multihop::RebindPolicy;
use warrenguard_transport::supervisor::SupervisorHandle;
use warrenguard_tun_core::SocketBypass;

/// Watch receiver over the supervisor's published session.
type ClientWatch = tokio::sync::watch::Receiver<Option<Arc<MultiHopBundle>>>;
/// Shared single-shot pump error sender (same instance the pumps use).
type PumpErrorTx = Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>;

/// Production bindings for [`MigrationIo`].
pub(crate) struct RealWatchdogIo {
    pub route_events: tokio::sync::mpsc::UnboundedReceiver<()>,
    /// Kept alive for the platform subscriptions that are RAII-bound
    /// (Windows `CallbackHandle`); `None` elsewhere. Held only for its Drop.
    /// The leading underscore already exempts it from `dead_code` on every
    /// platform, so no lint attribute is needed (an `expect(dead_code)` here
    /// would be unfulfilled).
    pub _subscription_guard: Option<Box<dyn std::any::Any + Send>>,
    // Only macOS reads this (get_default_routes, in has_v4_default_route);
    // the Linux and Windows nudge + has-route paths use free functions, so
    // the field is dead there but kept for a uniform struct shape.
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub route_manager: RouteManagerHandle,
    pub client_rx: ClientWatch,
    pub supervisor: SupervisorHandle,
    pub pump_error_tx: PumpErrorTx,
    /// The carrier bypass resolved at connect time
    /// (`warren_carrier_socket_bypass` in `lib.rs`), `None` on Linux/other
    /// targets where a fwmark/route carries the escape instead of a per-socket
    /// bind. Windows re-resolves it fresh on each rebind and falls back to this
    /// connect-time value if re-resolution fails (see
    /// [`RealWatchdogIo::current_socket_bypass`]).
    #[cfg_attr(not(target_os = "windows"), expect(dead_code))]
    pub socket_bypass: Option<SocketBypass>,
    // The three fields below feed the macOS route escape of
    // [`MigrationIo::ensure_route_escape`] and mirror `RealEgressGuardIo`; they
    // are kept on every platform for a uniform struct shape, like
    // `route_manager` above.
    /// Carrier IPs to except with the `/32` escape (the relay endpoint on
    /// multi-hop, the only UDP peer the client dials).
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub carrier_ips: Vec<IpAddr>,
    /// TUN interface name, for the DefaultNode route set.
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub tun_iface: String,
    /// Where the per-network carrier verdicts live, so a degraded network is
    /// remembered for the next connect.
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub verdict_dir: Option<PathBuf>,
}

/// The escape the fresh migration socket must carry, or `None` when none can be
/// established and the rebind must therefore be skipped.
///
/// Windows is the only surface that re-escapes the socket itself, and it fails
/// closed: there is no host route to the exit to fall back on, so an unbypassed
/// carrier is captured by the split-default `/1` halves and self-nests into the
/// tunnel it carries. macOS deliberately stays unbound: its escape is the
/// destination-keyed `<carrier_ip>/32` route that
/// [`MigrationIo::ensure_route_escape`] has just confirmed live, and reapplying
/// `IP_BOUND_IF` here blackholes all egress on a multi-interface host. Linux
/// keeps the fwmark, which follows the main table's updated default on its own.
fn rebind_policy(bypass: Option<SocketBypass>) -> Option<RebindPolicy> {
    #[cfg(target_os = "windows")]
    {
        bypass.map(RebindPolicy::Bypass)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = bypass;
        Some(RebindPolicy::Plain)
    }
}

impl RealWatchdogIo {
    fn current_client(&self) -> Option<Arc<MultiHopBundle>> {
        self.client_rx.borrow().clone()
    }

    /// The bypass the fresh migration socket must carry, re-resolved against
    /// the interface the host egresses on NOW so a genuine Wi-Fi to Ethernet
    /// hand-off follows the new path; falls back to the connect-time bypass
    /// when re-resolution fails, which still recovers a same-interface flap.
    #[cfg(target_os = "windows")]
    async fn current_socket_bypass(&mut self) -> Option<SocketBypass> {
        match crate::discover_warren_phys_ifindex().await {
            Ok(ifindex) => {
                let fresh = crate::warren_carrier_socket_bypass(ifindex);
                self.socket_bypass = Some(fresh);
                Some(fresh)
            }
            Err(e) => {
                log::debug!(
                    "watchdog: physical interface re-resolve failed, using the connect-time bypass: {e}"
                );
                self.socket_bypass
            }
        }
    }
}

impl MigrationIo for RealWatchdogIo {
    async fn next_route_event(&mut self) -> bool {
        self.route_events.recv().await.is_some()
    }

    async fn has_v4_default_route(&mut self) -> bool {
        #[cfg(target_os = "macos")]
        {
            match self.route_manager.get_default_routes().await {
                Ok((v4, _v6)) => v4.is_some(),
                Err(e) => {
                    log::debug!("watchdog: get_default_routes failed: {e}");
                    false
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            crate::detect_default_iface().is_ok()
        }
        #[cfg(target_os = "windows")]
        {
            talpid_routing::get_best_default_route(talpid_windows::net::AddressFamily::Ipv4)
                .map(|r| r.is_some())
                .unwrap_or(false)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            true
        }
    }

    async fn nudge_bypass(&mut self) {
        // Nothing to re-point at connect-time granularity: the carrier escapes
        // by socket on macOS/Windows and by fwmark on Linux, which follows the
        // main table's updated default on its own. The macOS bind that the
        // rebind drops is handled by `ensure_route_escape`, not here.
    }

    fn session_can_migrate(&mut self) -> bool {
        // No published session counts as migratable: the cycle already handles
        // the `None` case on every other IO call, and the redial that follows
        // dials a fresh native socket anyway.
        self.current_client()
            .is_none_or(|client| !client.is_over_carrier())
    }

    async fn ensure_route_escape(&mut self) -> bool {
        #[cfg(target_os = "macos")]
        {
            // A connect whose `IP_BOUND_IF` bind was CONFIRMED never installed
            // the `<carrier_ip>/32` route, and the rebind below hands quinn a
            // deliberately unbound socket: without this the fresh socket holds
            // neither escape and the carrier self-nests into the tunnel it
            // carries until the escalation. Degrade the bind to the
            // destination-keyed escape, and WAIT for the install to be
            // confirmed, because an install racing the rebind loses against
            // QUIC's path-validation window.
            if !crate::carrier_egress_guard::install_carrier_route_escape(
                &self.route_manager,
                &self.tun_iface,
                &self.carrier_ips,
            )
            .await
            {
                return false;
            }
            // Nothing is recorded against the network here: this degradation is
            // imposed by the rebind and never measured, so calling it
            // `RouteOnly` would make every later connect on this network skip
            // the leak-free bind and pre-install the exception for the whole
            // verdict TTL. Only `reclaim_escape` writes, and only from a guard
            // measurement.
            log::info!(
                "Warren migration watchdog: carrier escape degraded to the <carrier_ip>/32 \
                 DefaultNode route ahead of the rebind"
            );
            true
        }
        // Linux carries the escape by fwmark and Windows reapplies
        // `IP_UNICAST_IF` to the fresh socket inside `rebind_endpoint`, so on
        // both the escape survives the rebind with nothing to install here.
        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    async fn rebind_endpoint(&mut self) {
        let Some(client) = self.current_client() else {
            return;
        };
        #[cfg(target_os = "windows")]
        let bypass = self.current_socket_bypass().await;
        // Every other target escapes off-socket, so there is nothing to resolve
        // and nothing to reapply.
        #[cfg(not(target_os = "windows"))]
        let bypass = None;
        let Some(policy) = rebind_policy(bypass) else {
            log::debug!(
                "watchdog: no carrier bypass available, skipping rebind to stay fail-closed"
            );
            return;
        };
        // The engine builds the fresh wildcard socket the same way the dial
        // does and applies `policy` BEFORE quinn can send on it, so a migration
        // cannot put an unescaped carrier on the wire; on any failure the
        // session keeps the socket it already had.
        match client.rebind_wildcard(policy) {
            Ok(()) => {
                log::info!("Warren migration watchdog: rebound QUIC endpoint to a fresh socket")
            }
            Err(e) => log::debug!("watchdog: endpoint rebind failed: {e}"),
        }
    }

    // Only macOS trades a per-socket escape for a route across a rebind, so
    // only macOS has one to take back; the trait's empty default covers the
    // rest.
    #[cfg(target_os = "macos")]
    async fn reclaim_escape(&mut self) {
        let mut io = crate::carrier_bind_reclaim::RealReclaimIo::new(
            self.client_rx.clone(),
            self.route_manager.clone(),
            self.carrier_ips.clone(),
            self.tun_iface.clone(),
            self.verdict_dir.clone(),
        );
        let outcome = crate::carrier_bind_reclaim::run_bind_reclaim(&mut io).await;
        log::info!("Warren migration watchdog: carrier bind reclaim: {outcome:?}");
    }

    async fn send_probe(&mut self) {
        if let Some(client) = self.current_client()
            && let Err(e) = client.send_daita_padding().await
        {
            log::debug!("watchdog: liveness probe send failed: {e}");
        }
    }

    fn rx_sample(&mut self) -> Option<RxSample> {
        self.current_client().map(|client| {
            // Mix the local port into the identity: the Arc address can
            // be reused by the very next session (ABA), the wildcard
            // bind's ephemeral port cannot, within any realistic window.
            // Rotate rather than shift: a shift wide enough to clear a
            // 64-bit pointer's low bits overflows the 32-bit `usize` of
            // Android's ABIs, and `warren-jni` computes this same identity,
            // so the repo carries one shape rather than two.
            let port = client.local_addr().map(|a| a.port()).unwrap_or(0);
            RxSample {
                id: (Arc::as_ptr(&client) as usize) ^ usize::from(port).rotate_left(17),
                rx_datagrams: client.quinn_stats().udp_rx.datagrams,
            }
        })
    }

    fn force_reconnect(&mut self) -> bool {
        self.supervisor.force_reconnect()
    }

    fn escalate(&mut self, msg: String) {
        log::warn!("Warren migration watchdog: escalating to the state machine: {msg}");
        if let Some(tx) = self
            .pump_error_tx
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = tx.send(msg);
        }
    }
}

/// Subscribe to default-route changes, normalized to a unit-tick
/// channel. Returns the receiver plus an optional RAII guard that
/// must stay alive for the subscription to keep firing.
pub(crate) async fn subscribe_route_events(
    route_manager: &RouteManagerHandle,
) -> Result<
    (
        tokio::sync::mpsc::UnboundedReceiver<()>,
        Option<Box<dyn std::any::Any + Send>>,
    ),
    String,
> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(target_os = "macos")]
    {
        use futures::StreamExt;
        let mut events = route_manager
            .default_route_listener()
            .await
            .map_err(|e| format!("default_route_listener: {e}"))?;
        tokio::spawn(async move {
            while let Some(_event) = events.next().await {
                if tx.send(()).is_err() {
                    return;
                }
            }
        });
        Ok((rx, None))
    }
    #[cfg(target_os = "linux")]
    {
        use futures::StreamExt;
        use talpid_routing::CallbackMessage;
        let mut events = route_manager
            .change_listener()
            .await
            .map_err(|e| format!("change_listener: {e}"))?;
        tokio::spawn(async move {
            while let Some(msg) = events.next().await {
                let is_default_route = match &msg {
                    CallbackMessage::NewRoute(route) | CallbackMessage::DelRoute(route) => {
                        route.prefix().prefix() == 0
                    }
                };
                if is_default_route && tx.send(()).is_err() {
                    return;
                }
            }
        });
        Ok((rx, None))
    }
    #[cfg(target_os = "windows")]
    {
        let handle = route_manager
            .add_default_route_change_callback(Box::new(move |_event, _family| {
                let _ = tx.send(());
            }))
            .await
            .map_err(|e| format!("add_default_route_change_callback: {e}"))?;
        Ok((rx, Some(Box::new(handle) as Box<dyn std::any::Any + Send>)))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = route_manager;
        drop(tx);
        Ok((rx, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warrenguard_tun_core::SocketBypass;

    /// Reapplying `IP_BOUND_IF` to the migration socket blackholes every egress
    /// on a multi-interface host once the physical default becomes ifscoped, so
    /// macOS must stay unbound even when a bypass is at hand: its escape is the
    /// `<carrier_ip>/32` route `ensure_route_escape` just confirmed.
    #[test]
    #[cfg(target_os = "macos")]
    fn macos_leaves_the_fresh_socket_unbound_even_with_a_bypass_at_hand() {
        let policy = rebind_policy(Some(SocketBypass::BoundIf(7)));

        assert!(
            matches!(policy, Some(RebindPolicy::Plain)),
            "binding the migration socket is the carrier-blackhole failure mode"
        );
    }

    /// The Warren fwmark rides on the socket the kernel already marks, and it
    /// follows the main table's updated default on its own, so a Linux rebind
    /// has nothing to reapply.
    #[test]
    #[cfg(target_os = "linux")]
    fn linux_leaves_the_fresh_socket_plain_because_the_fwmark_carries_the_escape() {
        let policy = rebind_policy(Some(SocketBypass::Fwmark(
            warrenguard_tun_core::WARREN_TUNNEL_FWMARK,
        )));

        assert!(matches!(policy, Some(RebindPolicy::Plain)));
    }

    /// Windows has no destination route to fall back on: the fresh socket must
    /// carry the bypass resolved for the interface the host now egresses on.
    #[test]
    #[cfg(target_os = "windows")]
    fn windows_carries_the_resolved_bypass_onto_the_fresh_socket() {
        let policy = rebind_policy(Some(SocketBypass::UnicastIf(9)));

        assert!(matches!(
            policy,
            Some(RebindPolicy::Bypass(SocketBypass::UnicastIf(9)))
        ));
    }

    /// An unbypassed Windows carrier is captured by the split-default `/1`
    /// halves and self-nests into the tunnel it carries, so no bypass means no
    /// rebind at all.
    #[test]
    #[cfg(target_os = "windows")]
    fn windows_refuses_to_rebind_without_a_bypass() {
        assert!(rebind_policy(None).is_none());
    }
}
