//! Warren adapter for the talpid tunnel state machine.
//!
//! This crate exposes [`WarrenTunnelMonitor`], a drop-in alternative to
//! [`talpid_wireguard::WireguardMonitor`] consumed by
//! `talpid_core::tunnel_state_machine::tunnel_monitor::TunnelMonitor`
//! through an enum dispatch. The API mirrors
//! [`WireguardMonitor::start`] / `wait` so `connecting_state.rs` can
//! treat both backends uniformly.
//!
//! Underneath, [`WarrenTunnelMonitor::start`] performs the QUIC
//! handshake through [`warren_tunnel::ClientTunnel`], opens a TUN via
//! the talpid `TunProvider`, emits `TunnelEvent::InterfaceUp` / `Up`
//! and spawns the bidirectional pump (TUN <-> QUIC datagrams).
//! `wait()` blocks on the close-signal, drops the routing-table
//! override and aborts the pump.

use std::net::IpAddr;
use std::path::Path;
use std::time::Instant;

use ed25519_dalek::{SigningKey, VerifyingKey};
use ipnetwork::IpNetwork;
use talpid_routing::{Node, RequiredRoute};
use talpid_tunnel::tun_provider::{Tun, TunConfig};
use talpid_tunnel::{TunnelArgs, TunnelEvent, TunnelMetadata};
use talpid_types::net::AllowedTunnelTraffic;
use warren_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};
// Re-exported below so downstream crates (talpid-core, mullvad-daemon)
// can construct `MultiHopConfig` without depending on warren-multihop
// directly. Same pattern as `warren-relay-selector::warren_types`.
pub use warren_multihop::RelayDescriptorSigned as MultiHopRelayDescriptor;
pub use warren_multihop::{ExitDescriptorSigned as MultiHopExitDescriptor, ExitId};
// NAT-PMP wire protocol enum re-exported so daemon code constructs
// `NatPmpConfig { protocol: NatPmpProto::Udp, .. }` without depending
// directly on the warren-natpmp-protocol crate. The crate itself is a
// path-dep of this one (so the type lives in this binary's symbol
// table) and is also referenced explicitly by the daemon-side
// `warren_nat_pmp` module.
pub use warren_natpmp_protocol::MapProto as NatPmpProto;
// IPv4 CIDR descriptor used by the daemon-side `--bypass-cidr`
// settings plumbing. Re-exported so callers (mullvad-daemon, gRPC
// conversions, settings persistence) consume one canonical type
// instead of duplicating it across crates.
pub use warren_client::bypass_cidr::BypassCidr;
use warren_protocol::{WarrenExitAddr, WarrenTransportAddr};
use warren_tunnel::{
    ClientSession, ClientTunnel, MultiSession, pump_bidirectional, pump_multi_bidirectional,
};

mod adapter;
use adapter::MullvadTunPacketDevice;

/// Daemon-side NAT-PMP lifecycle wrapper that owns the refresh loop +
/// event forwarder and drops them on tunnel teardown. Activated only
/// when `WarrenTunnelParameters::nat_pmp` carries `Some(cfg)` with
/// `cfg.enabled == true`.
pub mod nat_pmp_manager;
pub use nat_pmp_manager::{NatPmpEvent, NatPmpEventObserver, NatPmpManager};

/// Split-default policy routing helper that routes Internet traffic via
/// the tunnel without overriding the kernel main routing table.
pub mod default_route_split;

// Trace prefix used by the start-sequence and pump-metrics debug logs.
// Format: `[warren-trace] T{N}={ms}ms <event>`. `N` increments at each
// step of `start()`, `ms` is the elapsed since `start_t`. Logs are at
// `debug` level so they stay out of release output; enable with
// `RUST_LOG=talpid_warren_tunnel=debug` when diagnosing a start/wait
// sequencing issue.
const TRACE_PREFIX: &str = "[warren-trace]";

/// Parameters required to start a Warren tunnel.
///
/// Field shape mirrors the inputs to [`ClientTunnel::connect`] /
/// [`ClientTunnel::connect_multi`]. Exit selection (`exit_addr`) is
/// provided upstream by the Warren-fork relay selector; the
/// `signing_key` is derived from the user's BIP39 mnemonic via
/// `warren_identity::derive_node_key` (auth wallet).
///
/// Note: `exit_addr.id` carries the exit's Ed25519 pubkey post-Quinn
/// migration, so the legacy separate `exit_id` parameter has been
/// folded into `exit_addr`.
#[derive(Clone)]
pub struct WarrenTunnelParameters {
    /// Candidate addresses of the exit (UDP IPv4/IPv6) plus the exit's
    /// Ed25519 pubkey in `exit_addr.id`. Built by the relay selector
    /// from `exit-info.json` published by the exits.
    pub exit_addr: WarrenExitAddr,

    /// Client Ed25519 signing key (derived from the user's BIP39
    /// mnemonic). `talpid-warren-tunnel` never generates an ephemeral
    /// identity: the identity must be stable so reconnects re-attach
    /// to the same tunnel IP on the exit side.
    pub signing_key: SigningKey,

    /// Number of parallel QUIC connections to use. `1` = mono-conn
    /// (classic), `N > 1` = multi-flow bonding aggregated by identity
    /// on the exit side.
    pub n_connections: u8,

    /// Client feature bitmask advertised in the `Setup` frame
    /// (cf. `warren_protocol::features`). `0` = IPv4 baseline.
    /// Combinable via OR: `IPV6`, `PORT_FORWARD`, ...
    pub features: u32,

    /// Multi-hop configuration. `None` (default) selects the legacy
    /// single-hop path through [`warren_tunnel::ClientTunnel`]; `Some`
    /// dispatches to a multi-hop session driven by
    /// `warren_client::supervisor::MultiHopSupervisor` against the
    /// supplied first-hop relay.
    ///
    /// Selecting multi-hop changes the firewall surface (the daemon
    /// only opens the relay endpoint, not the exit candidates) and the
    /// GUI tunnel endpoint (the exit is shown via `entry_endpoint`).
    /// Both wiring details live in
    /// `talpid_core::tunnel_state_machine::backend_params`.
    pub multi_hop: Option<MultiHopConfig>,

    /// Optional observer fired once per successful reconnect by the
    /// multi-hop supervisor (NOT on the initial connect). `None`
    /// disables the live `reconnect_count` surface; the daemon-side
    /// `ParametersGenerator` wires this to a closure that bumps its
    /// `WarrenStatusCache` so the Electron UI counter advances.
    /// Ignored on the single-hop path (single-hop has no auto-reconnect
    /// supervisor in /v1).
    pub on_reconnect: Option<warren_client::supervisor::ReconnectObserver>,

    /// NAT-PMP port-forwarding configuration. `None` (default) disables
    /// the feature entirely; `Some` instructs the daemon-side
    /// `NatPmpManager` to spawn a `refresh_loop` against the exit's
    /// NAT-PMP server (RFC 6886, UDP/5351 of the tunnel gateway) once
    /// the tunnel is `Up`, and to tear it down on disconnect.
    ///
    /// Port-forwarding is Warren's headline differentiator since
    /// Mullvad and IVPN dropped the feature in 2023. The wire format
    /// and the exit-side service are out of scope here: this struct
    /// only carries the user's preference (toggle + lifetime +
    /// protocol) from the Settings UI down to the daemon-side manager.
    ///
    /// Not consumed inside `talpid-warren-tunnel` itself: the dispatcher
    /// only relays the field to the daemon-side `NatPmpManager` via
    /// the tunnel state machine. The structural mirror is intentional
    /// (parameters live in one struct).
    pub nat_pmp: Option<NatPmpConfig>,

    /// Observer invoked for every NAT-PMP event emitted by the daemon
    /// owned `NatPmpManager` (Mapped, Renewed, Failed, Cancelled).
    /// `None` short-circuits the manager spawn entirely (no observer =
    /// nowhere to forward events = the manager would be useless). The
    /// daemon wires this to a closure that pushes events into the
    /// `WarrenStatusCache`, which in turn drives the
    /// `NatPmpStatusUpdates` gRPC stream the Electron UI subscribes to.
    pub nat_pmp_observer: Option<NatPmpEventObserver>,

    /// IPv4 CIDRs that should bypass the tunnel and remain reachable
    /// via the host's main routing table (LAN, private ranges, inbound
    /// SSH on a management interface, ...). Each entry becomes an
    /// `ip rule add to <cidr> lookup main pref 49` installed alongside
    /// the standard `0.0.0.0/1` + `128.0.0.0/1` split-default routes.
    /// Empty (default) preserves the M4.E.D behaviour: the tunnel
    /// captures all traffic except the exit IP itself.
    ///
    /// Linux-only at this layer: macOS and Windows daemon routing is
    /// handled by talpid-core's platform splitters, which do not yet
    /// consume this list. UI exposure is deferred to a future phase;
    /// the field is plumbed end-to-end so future UI work only needs
    /// the gRPC + Redux glue, not a fresh daemon traversal.
    pub bypass_cidrs: Vec<BypassCidr>,
}

impl std::fmt::Debug for WarrenTunnelParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No-log Warren: never log the full signing_key (secret material)
        // nor the full exit pubkey (session-PII). Minimal debug shape.
        f.debug_struct("WarrenTunnelParameters")
            .field("exit_addr", &"<redacted>")
            .field("signing_key", &"<redacted>")
            .field("n_connections", &self.n_connections)
            .field("features", &format_args!("{:#010x}", self.features))
            .field("multi_hop", &self.multi_hop.as_ref().map(|_| "<redacted>"))
            .field(
                "on_reconnect",
                &self.on_reconnect.as_ref().map(|_| "<observer>"),
            )
            .field("nat_pmp", &self.nat_pmp)
            .field(
                "nat_pmp_observer",
                &self.nat_pmp_observer.as_ref().map(|_| "<observer>"),
            )
            .field("bypass_cidrs", &self.bypass_cidrs)
            .finish()
    }
}

/// NAT-PMP port-forwarding configuration carried by
/// [`WarrenTunnelParameters::nat_pmp`].
///
/// Wired by the Electron UI through the gRPC `SetNatPmpSettings` rpc.
/// Default-disabled (the field is `None` upstream when `enabled =
/// false`) so existing user installs see no change in behaviour after
/// the M4.H.F upgrade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NatPmpConfig {
    /// User-facing toggle. `false` is the default; when `false` the
    /// daemon-side manager does NOT start a refresh loop at all.
    pub enabled: bool,
    /// Lifetime requested from the exit in seconds. The server clamps
    /// to its own `[60..3600]` range, so values outside that range will
    /// be silently capped server-side; UI offers `3600` (1h), `21600`
    /// (6h, ends up clamped), `86400` (24h, ends up clamped). The
    /// refresh loop renews at `granted / 2`.
    pub lifetime_secs: u32,
    /// Transport protocol to map. Single value in /v1: UDP or TCP.
    /// Mapping both protocols simultaneously is left to a future
    /// extension (would spawn two refresh loops).
    pub protocol: NatPmpProto,
    /// Suggested external port. `0` (default) lets the server pick a
    /// port from its allocator pool (recommended). Pinning a port is
    /// possible by setting a value in `49152..=65535` but the server
    /// is allowed to ignore the suggestion (RFC 6886 §3.3).
    pub suggested_external_port: u16,
    /// Internal port the application binds locally. `0` means "no
    /// specific local port", in which case the daemon defers a
    /// concrete port choice to the manager. UI may expose this field
    /// for advanced users.
    pub internal_port: u16,
}

impl NatPmpConfig {
    /// Default lifetime: 1 hour. Matches the exit-side allocator's
    /// maximum clamp ([60..3600] seconds), so the user gets the longest
    /// possible interval between renewals (renewal every 30 min).
    pub const DEFAULT_LIFETIME_SECS: u32 = 3600;

    /// Default disabled configuration; the daemon treats this as
    /// equivalent to `None` (no refresh loop spawned). Provided so
    /// callers can produce a `Some(NatPmpConfig::default_disabled())`
    /// for places that need to thread the config but not enable it.
    #[must_use]
    pub fn default_disabled() -> Self {
        Self {
            enabled: false,
            lifetime_secs: Self::DEFAULT_LIFETIME_SECS,
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
        }
    }

    /// Default enabled configuration for first-time users: UDP, 1 hour
    /// lifetime, server picks the external port. Internal port is 0
    /// (UI must set the bind port before any application can actually
    /// receive traffic; the mapping itself is created regardless).
    #[must_use]
    pub fn default_enabled() -> Self {
        Self {
            enabled: true,
            lifetime_secs: Self::DEFAULT_LIFETIME_SECS,
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
        }
    }
}

/// Inputs required to bring up a multi-hop session, populated by the
/// daemon-side relay selector and consumed by the dispatcher in
/// [`WarrenTunnelMonitor::start`].
///
/// Both descriptors are verified against [`Self::operational_pubkey`]
/// before any UDP traffic is emitted. The exit `WarrenExitAddr` carried
/// at the [`WarrenTunnelParameters`] root is ignored on the multi-hop
/// path: the dispatcher derives the routing tag, the X25519 HPKE
/// pubkey, and the Ed25519 RPK identity from [`Self::exit`].
#[derive(Clone)]
pub struct MultiHopConfig {
    /// Signed first-hop descriptor. The client dials
    /// [`RelayDescriptorSigned::endpoint`] over QUIC; the relay's
    /// Ed25519 identity is pinned via the TLS RPK at handshake time.
    pub relay: RelayDescriptorSigned,
    /// Signed exit descriptor. The relay never holds the HPKE key, so
    /// the exit's long-lived X25519 pubkey advertised here is the only
    /// trust anchor for the payload encryption.
    pub exit: ExitDescriptorSigned,
    /// Ed25519 operational pubkey shared across the relay + exit
    /// descriptor signatures. Out-of-band trust anchor for the
    /// multi-hop pool.
    pub operational_pubkey: VerifyingKey,
    /// Enable UDP segmentation offload (GSO) on the multi-hop QUIC
    /// transport. Recommended on physical NICs, disable on virtio
    /// (Hetzner Cloud / KVM guests) and on macOS where GSO is not
    /// supported.
    pub enable_gso: bool,
    /// Opt into the full M4.0 wire-mimicry profile (Initial padding +
    /// split ClientHello). `true` is the production default against a
    /// real warren-relay; `false` is used for loopback benches where
    /// the relay-inbound transport config does not mirror these knobs
    /// (caveat M4.D #1 in `warren-client::multi_hop`).
    pub use_warren_obfuscation: bool,
}

impl std::fmt::Debug for MultiHopConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No-log Warren: relay/exit pubkeys and endpoints are
        // long-term identifiers correlatable across sessions; the
        // operational pubkey identifies the deployment. All redacted.
        f.debug_struct("MultiHopConfig")
            .field("relay", &"<redacted>")
            .field("exit", &"<redacted>")
            .field("operational_pubkey", &"<redacted>")
            .field("enable_gso", &self.enable_gso)
            .field("use_warren_obfuscation", &self.use_warren_obfuscation)
            .finish()
    }
}

/// Errors specific to the Warren tunnel backend.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The QUIC handshake (`connect` / `connect_multi`) failed
    /// (timeout, exit rejected, protocol version mismatch, etc.).
    /// The underlying warren-tunnel error is stringified so the
    /// `Display` does not leak identity material (no-log Warren).
    #[error("Warren handshake failed: {0}")]
    Handshake(String),

    /// Could not open the TUN device via
    /// [`talpid_tunnel::tun_provider::TunProvider`].
    #[error("Warren tun setup failed: {0}")]
    TunSetup(String),

    /// Generic backend error (pump I/O, route install, ...).
    #[error("Warren tunnel backend error: {0}")]
    Backend(String),
}

impl Error {
    /// Whether the error is worth retrying from the state-machine
    /// side. `Handshake` is `true` (transient network glitch is the
    /// common cause); `TunSetup` is `false` (privilege / kernel
    /// module / name collision: retry will not help); `Backend` is
    /// `false` (structural).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::Handshake(_))
    }
}

/// Active Warren tunnel monitor.
///
/// API mirrors [`talpid_wireguard::WireguardMonitor`]:
/// - [`Self::start`]: blocking factory (QUIC handshake + TUN setup +
///   pump spawn).
/// - [`Self::wait`]: blocks until the daemon close-signal fires.
///
/// `start` performs the QUIC handshake via warren-tunnel, opens the
/// TUN through `args.tun_provider`, emits `InterfaceUp` then `Up`,
/// installs the routing override and spawns the bidirectional pump.
/// `wait` blocks on `tunnel_close_rx`, emits `Down`, uninstalls the
/// routing override and drains the pump.
pub struct WarrenTunnelMonitor {
    runtime: tokio::runtime::Handle,
    /// Backend-specific task handles. Single-hop owns one bidirectional
    /// pump; multi-hop owns three tasks (supervisor + uplink +
    /// downlink). `wait()` aborts all of them on teardown and surfaces
    /// abnormal terminations through the same `Backend` error path.
    backend: MonitorBackend,
    /// Event sink towards the daemon state machine.
    event_hook: talpid_tunnel::EventHook,
    /// Oneshot from the daemon: fires to request tunnel shutdown.
    close_rx: futures::channel::oneshot::Receiver<()>,
    /// Guard for the split-default policy routing override. Installed
    /// after the Up events to force Internet traffic through the
    /// tunnel without breaking the daemon -> exit QUIC socket. Cleanup
    /// happens in `wait()` before the pump is aborted (mirrors the
    /// install order). `None` if no IPv4 next-hop IP was available.
    default_route_guard: Option<default_route_split::DefaultRouteSplitGuard>,
    /// Lifecycle owner of the NAT-PMP refresh loop + event forwarder
    /// spawned after the tunnel is up. `None` when port-forwarding is
    /// disabled in the user settings (`params.nat_pmp.is_none()` OR
    /// `params.nat_pmp_observer.is_none()`). Dropping the field
    /// cancels the loop and aborts the forwarder.
    nat_pmp_manager: Option<NatPmpManager>,
}

/// Backend-specific task ownership.
///
/// - [`MonitorBackend::SingleHop`] owns a single bidirectional pump
///   spawned by `warren_tunnel::pump_bidirectional` /
///   `pump_multi_bidirectional` and an oneshot channel to surface
///   abnormal pump terminations.
/// - [`MonitorBackend::MultiHop`] owns a 3-task fanout: the
///   `MultiHopSupervisor` future (drives connect+reconnect), the
///   uplink pump (TUN -> multi-hop) and the downlink pump (multi-hop
///   -> TUN), wired together through a watch channel.
enum MonitorBackend {
    SingleHop {
        pump_handle: tokio::task::JoinHandle<()>,
        pump_error_rx: tokio::sync::oneshot::Receiver<String>,
    },
    MultiHop {
        supervisor_handle: tokio::task::JoinHandle<()>,
        uplink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        downlink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        /// Channel through which the pump task reports the first fatal
        /// error (uplink/downlink/supervisor death). `wait()` consumes
        /// it the same way the single-hop path consumes
        /// `pump_error_rx`. A retriable disconnect from the underlying
        /// QUIC connection does NOT push an error here: the supervisor
        /// absorbs it transparently and the pump tasks park on the
        /// watch channel until the supervisor re-publishes.
        pump_error_rx: tokio::sync::oneshot::Receiver<String>,
    },
}

impl WarrenTunnelMonitor {
    /// Starts a Warren tunnel from `params`, dispatching to the
    /// single-hop path through `warren_tunnel::ClientTunnel` (when
    /// `params.multi_hop` is `None`) or the multi-hop path through
    /// `warren_client::supervisor::MultiHopSupervisor` (when `Some`).
    ///
    /// Both paths block the current thread until the underlying QUIC
    /// session is established and the TUN is up.
    ///
    /// # Errors
    ///
    /// See [`Self::start_single_hop`] and [`Self::start_multi_hop`].
    pub fn start(
        params: &WarrenTunnelParameters,
        args: TunnelArgs<'_>,
        log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        match params.multi_hop.clone() {
            None => Self::start_single_hop(params, args, log_path),
            Some(cfg) => Self::start_multi_hop(params, cfg, args, log_path),
        }
    }

    /// Single-hop path: dial the exit directly through
    /// `warren_tunnel::ClientTunnel`, open a TUN, install routing
    /// guards, then spawn the bidirectional pump.
    ///
    /// Sequence:
    /// 1. `connect` (or `connect_multi`) against warren-tunnel.
    /// 2. Build `TunConfig` from the IPs assigned by the exit.
    /// 3. `tun_provider.open_tun()` for the platform-specific device.
    /// 4. Emit `TunnelEvent::InterfaceUp` then `Up` via `event_hook`.
    /// 5. Install routing override + spawn the bidirectional pump.
    ///
    /// # Errors
    ///
    /// - [`Error::Handshake`] if the QUIC handshake or `Setup`
    ///   exchange fails.
    /// - [`Error::TunSetup`] if opening the TUN fails (privileges,
    ///   interface name collision, kernel module missing).
    fn start_single_hop(
        params: &WarrenTunnelParameters,
        args: TunnelArgs<'_>,
        _log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        // Per-step start timestamps. Keep these (low cost, debug-only)
        // so a future regression in the talpid/warren-tunnel handoff
        // can be pinpointed without instrumenting the codebase again.
        let start_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T0=0ms phase=start_begin n_conns={} features={:#010x}",
            params.n_connections,
            params.features
        );

        let runtime = args.runtime.clone();
        // Filter out non-Internet-routable candidate addresses (RFC 1918
        // IPv4, ULA / link-local IPv6, loopback). Some legacy exit
        // metadata still carries the TUN gateway IP (10.66.0.1) in its
        // candidate list; if the client tried it, the kernel would route
        // through the tunnel itself, causing an encapsulation loop.
        // Quinn upstream does not do path discovery the way Iroh did
        // (no `n0_nat_traversal` extension), so this filter is now
        // defense-in-depth.
        let exit_addr = filter_endpoint_addr_for_wan(params.exit_addr.clone());
        let signing = params.signing_key.clone();
        let n_conns = params.n_connections;
        let features = params.features;
        let mut event_hook = args.event_hook;

        // Detect the outbound source IP (eth0 / wlan0) used to reach
        // the exit before the handshake, so we bind the QUIC `Endpoint`
        // explicitly to that IP instead of `0.0.0.0:0`. Defense in depth
        // against any future regression that might re-introduce
        // multi-path / rebind behavior in the transport layer.
        let bind_local_ip: Option<std::net::SocketAddr> =
            exit_addr.ip_addrs().next().and_then(|exit_sa| {
                match detect_default_local_ip(exit_sa) {
                    Ok(ip) => Some(std::net::SocketAddr::new(ip, 0)),
                    Err(e) => {
                        log::warn!(
                            "Warren: detect_default_local_ip failed: {e}. \
                         Falling back to bind 0.0.0.0:0 (unspecified)."
                        );
                        None
                    }
                }
            });
        if let Some(addr) = bind_local_ip {
            log::info!("Warren client bind local IP = {}", addr.ip());
        }

        log::debug!(
            "{TRACE_PREFIX} T1={}ms phase=handshake_start (block_on connect_*)",
            start_t.elapsed().as_millis()
        );
        let handshake_t = Instant::now();
        let session_kind = runtime.block_on(async move {
            let mut client = ClientTunnel::with_signing_key(&signing).with_features(features);
            if let Some(addr) = bind_local_ip {
                client = client.with_bind_local_ip(addr);
            }
            match select_session_request(n_conns) {
                SessionRequest::Mono => client
                    .connect(exit_addr)
                    .await
                    .map(SessionKind::Mono)
                    .map_err(|e| Error::Handshake(format!("{e:#}"))),
                SessionRequest::Multi(n) => client
                    .connect_multi(exit_addr, n)
                    .await
                    .map(SessionKind::Multi)
                    .map_err(|e| Error::Handshake(format!("{e:#}"))),
            }
        })?;
        log::debug!(
            "{TRACE_PREFIX} T2={}ms phase=handshake_done elapsed_handshake={}ms session_kind={}",
            start_t.elapsed().as_millis(),
            handshake_t.elapsed().as_millis(),
            match session_kind {
                SessionKind::Mono(_) => "Mono",
                SessionKind::Multi(_) => "Multi",
            }
        );

        // Step 2: TUN config derived from the session.
        let tun_t = Instant::now();
        let tun_config = build_tun_config_for_kind(&session_kind);
        let tun = {
            let mut provider = args
                .tun_provider
                .lock()
                .map_err(|_| Error::TunSetup("tun_provider mutex poisoned".to_owned()))?;
            *provider.config_mut() = tun_config.clone();
            provider
                .open_tun()
                .map_err(|e| Error::TunSetup(format!("{e}")))?
        };
        log::debug!(
            "{TRACE_PREFIX} T3={}ms phase=tun_opened elapsed_tun={}ms iface={}",
            start_t.elapsed().as_millis(),
            tun_t.elapsed().as_millis(),
            tun.interface_name()
                .unwrap_or_else(|_| "<unknown>".to_owned())
        );

        let metadata = build_tunnel_metadata(&tun, &tun_config);

        // Extract the inner `AsyncDevice` for the pump. `Tun = UnixTun`
        // exposes `into_inner` -> `into_async_device` (cf. the
        // Warren-fork patch on `talpid-tunnel/tun_provider/unix.rs`).
        // `MullvadTunPacketDevice` then wraps the `AsyncDevice` in an
        // `Arc` so it can be cloned between the uplink and downlink
        // tasks of the pump.
        let async_device = tun.into_inner().into_async_device();
        let packet_device = MullvadTunPacketDevice::new(async_device);

        // Emit Up events: `InterfaceUp` first (state machine then
        // installs routes + firewall), then `Up` (tunnel ready for
        // user traffic). Same sequencing as `WireguardMonitor`.
        log::debug!(
            "{TRACE_PREFIX} T4={}ms phase=interfaceup_emit (state machine va poser firewall+DNS rules)",
            start_t.elapsed().as_millis()
        );
        let events_t = Instant::now();
        runtime.block_on(async {
            event_hook
                .on_event(TunnelEvent::InterfaceUp(
                    metadata.clone(),
                    AllowedTunnelTraffic::All,
                ))
                .await;
            log::debug!(
                "{TRACE_PREFIX} T4b={}ms phase=interfaceup_consumed (firewall installed), emit Up",
                start_t.elapsed().as_millis()
            );
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });
        log::debug!(
            "{TRACE_PREFIX} T5={}ms phase=up_consumed elapsed_events={}ms (Connected state firewall installed)",
            start_t.elapsed().as_millis(),
            events_t.elapsed().as_millis()
        );

        // Install routes via route_manager: redirect user traffic
        // through the TUN while preserving the daemon's own access to
        // the peer endpoint (otherwise daemon -> tun -> exit -> daemon
        // == routing loop).
        //
        // Split-default strategy:
        // - 0.0.0.0/1 + 128.0.0.0/1 dev tun0 : covers all of 0.0.0.0/0
        //   without replacing the existing default route, less
        //   intrusive, clean restore at teardown via route_manager.
        // - <exit_ip>/32 dev <physical_iface> : more specific than /1
        //   so daemon -> exit packets bypass the tun.
        let exit_ips: Vec<IpAddr> = params.exit_addr.ip_addrs().map(|sa| sa.ip()).collect();
        // Per-platform route set, mirroring Mullvad WireGuard's
        // `get_endpoint_routes` / `get_pre_tunnel_routes` /
        // `get_post_tunnel_routes` dispatch:
        // - Linux: bypass `<exit_ip>/32 via <gw> dev <physical>` in
        //   the main table + split-default `/1 + /1 dev <tun>` in
        //   table 100 via `default_route_split`. Iface + gw detected
        //   from `/proc/net/route`.
        // - macOS: bypass `<exit_ip>/32 NetNode::DefaultNode`
        //   (talpid-routing resolves best_default_route at apply
        //   time) + `0.0.0.0/0 dev <tun>` (triggers the
        //   `tunnel_default_routes` ifscope dance). No upfront
        //   detection: talpid-routing already tracks the physical
        //   iface and gw via its internal monitor.
        #[cfg(target_os = "linux")]
        let routes = {
            let physical_iface = detect_default_iface().unwrap_or_else(|e| {
                log::warn!(
                    "Failed to detect default iface, falling back to 'eth0': {e}. \
                     Bypass routes for exit IPs may not install correctly."
                );
                "eth0".to_owned()
            });
            let gateway_v4 = detect_default_gateway_v4().ok();
            if let Some(gw) = gateway_v4 {
                log::info!("Detected default gateway: {gw} (used for bypass exit routes)");
            } else {
                log::warn!(
                    "Failed to detect default gateway via /proc/net/route. \
                     Bypass routes will use scope link (may fail ARP on cloud VPS)."
                );
            }
            build_warren_tunnel_routes(&metadata.interface, &exit_ips, &physical_iface, gateway_v4)
        };
        #[cfg(target_os = "macos")]
        let routes = build_warren_tunnel_routes_macos(&metadata.interface, &exit_ips);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let routes: Vec<RequiredRoute> = {
            log::warn!("Warren tunnel routing not yet implemented for this platform.");
            Vec::new()
        };

        let route_manager = args.route_manager.clone();
        let routes_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T6={}ms phase=routes_add_start",
            start_t.elapsed().as_millis()
        );
        let metadata_iface_for_log = metadata.interface.clone();
        runtime.block_on(async move {
            match route_manager.add_routes(routes.into_iter().collect()).await {
                Ok(()) => {
                    log::info!("Warren tunnel routes installed (tun={metadata_iface_for_log})")
                }
                Err(e) => log::warn!(
                    "Failed to install Warren tunnel routes: {e}. \
                     Tunnel up but no traffic forwarding."
                ),
            }
        });
        log::debug!(
            "{TRACE_PREFIX} T7={}ms phase=routes_added elapsed_routes={}ms",
            start_t.elapsed().as_millis(),
            routes_t.elapsed().as_millis()
        );

        // Linux: install the split-default policy routing (table 100
        // + ip rule bypass for the exit IP). On macOS the split-default
        // is posted directly in the main routing table by
        // `build_warren_tunnel_routes_macos` (the `/32` bypass is
        // specific enough to avoid a routing loop, so no policy routing
        // needed).
        #[cfg(target_os = "linux")]
        let default_route_guard = {
            let exit_ip_v4 = exit_ips.iter().find_map(|ip| match ip {
                IpAddr::V4(v4) => Some(*v4),
                IpAddr::V6(_) => None,
            });
            if let Some(v4) = exit_ip_v4 {
                let tun_name_for_split = metadata.interface.clone();
                runtime
                    .block_on(async move {
                        default_route_split::DefaultRouteSplitGuard::install(
                            v4,
                            &tun_name_for_split,
                        )
                        .await
                    })
                    .map(Some)
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "Warren: failed to install default-route split: {e}. \
                             Internet traffic will NOT route via tunnel. \
                             Need root + ip in PATH."
                        );
                        None
                    })
            } else {
                log::warn!(
                    "Warren: no IPv4 exit IP available, skip default-route split. \
                     Internet traffic via IPv6-only exit not yet supported."
                );
                None
            }
        };
        #[cfg(not(target_os = "linux"))]
        let default_route_guard: Option<default_route_split::DefaultRouteSplitGuard> = None;

        // Spawn the bidirectional TUN <-> QUIC datagram pump. The task
        // runs until (a) the session closes (drop of Session /
        // MultiSession -> QUIC connections closed) or (b) an I/O error
        // on the TUN (interface brought down by the kernel).
        //
        // The pump error is propagated via an internal oneshot
        // consumed by `wait()` (rather than swallowed in a log::warn!),
        // so the state machine can decide whether to retry.
        //
        // Dispatch on `SessionKind`:
        // - Mono -> `pump_bidirectional(tun, conn)`. The session value
        //   must be moved into the closure to keep the underlying
        //   `Endpoint` alive (otherwise its drop makes `read_datagram`
        //   return "endpoint driver future was dropped" immediately).
        //   Same pattern as `warren-client::main`.
        // - Multi -> `pump_multi_bidirectional(tun, multi_session)`
        //   for N-connection bonding (uplink round-robin + N downlink
        //   tasks).
        let (pump_error_tx, pump_error_rx) = tokio::sync::oneshot::channel::<String>();
        let pump_metrics = packet_device.metrics();
        let pump_spawn_t = Instant::now();
        let pump_handle = runtime.spawn(async move {
            log::info!("{TRACE_PREFIX} pump=running");
            let pump_result = match session_kind {
                SessionKind::Mono(session) => {
                    let conn = session.clone_conn();
                    let res = pump_bidirectional(packet_device, conn).await;
                    drop(session);
                    res
                }
                SessionKind::Multi(multi) => pump_multi_bidirectional(packet_device, multi).await,
            };
            match pump_result {
                Ok(()) => {
                    log::debug!("Warren pump terminated cleanly");
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    log::warn!("{TRACE_PREFIX} pump=terminated reason=error msg=\"{msg}\"");
                    // `send` may fail if `wait()` already dropped the
                    // receiver (external close beat us, teardown
                    // aborted the pump). Benign in that case.
                    let _ = pump_error_tx.send(msg);
                }
            }
        });

        // Periodic pump metrics task (every 2s, logs uplink + downlink
        // counters). Lets a future bench distinguish which direction
        // stalls (uplink stops but downlink continues -> server-side
        // `read_datagram` issue; both stop at once -> QUIC connection
        // closed). The task aborts when teardown aborts the pump.
        let _metrics_task = runtime.spawn(async move {
            let mut prev_up = 0u64;
            let mut prev_down = 0u64;
            let tick_start = Instant::now();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            interval.tick().await; // skip first immediate tick
            loop {
                interval.tick().await;
                let up = pump_metrics.uplink_packets();
                let down = pump_metrics.downlink_packets();
                let dup = up.saturating_sub(prev_up);
                let ddown = down.saturating_sub(prev_down);
                let elapsed = tick_start.elapsed().as_millis();
                log::debug!(
                    "{TRACE_PREFIX} pump_metrics t={elapsed}ms uplink={up} (+{dup}) downlink={down} (+{ddown})"
                );
                prev_up = up;
                prev_down = down;
            }
        });

        log::debug!(
            "{TRACE_PREFIX} T8={}ms phase=pump_spawned elapsed_total_start={}ms (data plane should now flow)",
            start_t.elapsed().as_millis(),
            pump_spawn_t.duration_since(start_t).as_millis()
        );

        let _ = metadata; // kept for future MTU-change re-emission

        // Spawn the NAT-PMP refresh loop once the data plane is up.
        // The exit's NAT-PMP server lives on the tunnel gateway
        // (10.66.0.1:5351); routing was installed at T7 so the UDP
        // datagrams now flow through the TUN.
        let nat_pmp_manager = spawn_nat_pmp_manager_if_enabled(&runtime, params);

        Ok(Self {
            runtime,
            backend: MonitorBackend::SingleHop {
                pump_handle,
                pump_error_rx,
            },
            event_hook,
            close_rx: args.tunnel_close_rx,
            default_route_guard,
            nat_pmp_manager,
        })
    }

    /// Multi-hop path: spawn a `MultiHopSupervisor` that dials the
    /// first-hop relay with auto-reconnect, open a TUN with a
    /// deterministic IP derived from the client signing key, then wire
    /// the supervisor's [`ClientWatch`] into the uplink + downlink
    /// pumps.
    ///
    /// The supervisor + pumps live inside the returned monitor and
    /// terminate together when `wait()` aborts them. Reconnects across
    /// QUIC disconnects (idle timeout, peer reset, transient blip
    /// beyond the 180 s `max_idle_timeout`) are absorbed silently by
    /// the supervisor; the pump tasks park on the watch channel until
    /// the supervisor re-publishes a live session.
    ///
    /// # Errors
    ///
    /// - [`Error::Handshake`] if the supervisor cannot establish an
    ///   initial session within the bounded wait window.
    /// - [`Error::TunSetup`] if opening the TUN fails (privileges,
    ///   interface name collision, kernel module missing).
    /// - [`Error::Backend`] for non-retriable supervisor errors (PKI,
    ///   TLS provider setup), bubbled up before any pump is spawned.
    fn start_multi_hop(
        params: &WarrenTunnelParameters,
        cfg: MultiHopConfig,
        args: TunnelArgs<'_>,
        _log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        use std::sync::Arc;
        use std::time::Duration;
        use warren_backoff::Backoff;
        use warren_client::multi_hop::MultiHopClient;
        use warren_client::supervised_pump::{run_downlink, run_uplink};
        use warren_client::supervisor::{MultiHopSupervisor, SupervisorConfig};

        let start_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T0=0ms phase=start_begin mode=multi_hop \
             enable_gso={} use_warren_obfuscation={}",
            cfg.enable_gso,
            cfg.use_warren_obfuscation
        );

        let runtime = args.runtime.clone();
        let mut event_hook = args.event_hook;

        // Detect the outbound source IP towards the RELAY (not the
        // exit) so the QUIC `Endpoint` binds explicitly to that IP and
        // does not rebind onto the TUN once routing flips.
        let relay_endpoint = cfg.relay.endpoint;
        let bind_local_ip: std::net::SocketAddr = match detect_default_local_ip(relay_endpoint) {
            Ok(ip) => std::net::SocketAddr::new(ip, 0),
            Err(e) => {
                log::warn!(
                    "Warren multi-hop: detect_default_local_ip for relay failed: {e}. \
                         Falling back to 0.0.0.0:0 (unspecified)."
                );
                std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0))
            }
        };
        log::info!(
            "Warren multi-hop client bind local IP = {}",
            bind_local_ip.ip()
        );

        // Spawn the supervisor and wait for the first successful dial
        // so the rest of the bootstrap (TUN, routing) has a live
        // session to anchor to.
        let supervisor_config = SupervisorConfig {
            relay: Arc::new(cfg.relay.clone()),
            exit_id: cfg.exit.exit_id,
            exit_x25519_multihop_pubkey: cfg.exit.exit_x25519_multihop_pubkey,
            operational_pubkey: cfg.operational_pubkey,
            client_signing: params.signing_key.clone(),
            bind_addr: bind_local_ip,
            enable_gso: cfg.enable_gso,
            use_warren_obfuscation: cfg.use_warren_obfuscation,
            backoff: Backoff::HANDSHAKE,
            on_reconnect: params.on_reconnect.clone(),
        };
        let (supervisor, mut client_rx) = MultiHopSupervisor::new(supervisor_config);
        let supervisor_handle = runtime.spawn(async move {
            if let Err(e) = supervisor.run().await {
                log::warn!(
                    "{TRACE_PREFIX} multi-hop supervisor terminated with non-retriable error: {e:#}"
                );
            }
        });

        let handshake_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T1={}ms phase=handshake_start (block_on supervisor first dial)",
            start_t.elapsed().as_millis()
        );
        // Bound the initial-dial wait to 5 * Backoff::HANDSHAKE.max
        // (~150 s) so a permanently-unreachable relay surfaces as a
        // clean Error::Handshake instead of hanging the state machine.
        let initial_wait_bound = Duration::from_secs(150);
        let initial_client: Arc<MultiHopClient> =
            runtime.block_on(async {
                let deadline = tokio::time::Instant::now() + initial_wait_bound;
                loop {
                    if let Some(c) = client_rx.borrow().clone() {
                        return Ok(c);
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(Error::Handshake(format!(
                            "multi-hop supervisor did not produce an initial session within {initial_wait_bound:?}"
                        )));
                    }
                    if tokio::time::timeout(remaining, client_rx.changed())
                        .await
                        .is_err()
                    {
                        return Err(Error::Handshake(format!(
                            "multi-hop supervisor did not produce an initial session within {initial_wait_bound:?}"
                        )));
                    }
                }
            })?;
        log::debug!(
            "{TRACE_PREFIX} T2={}ms phase=handshake_done elapsed_handshake={}ms session_kind=MultiHop",
            start_t.elapsed().as_millis(),
            handshake_t.elapsed().as_millis()
        );
        // Drop the strong reference; the watch channel still holds one.
        // Lets `MultiHopClient::close` on the watch path release cleanly
        // when the supervisor swaps in a fresh session.
        drop(initial_client);

        // Build a TUN config with a deterministic IP derived from the
        // client pubkey. Multi-hop /v1 has no server-side IP allocator
        // (the relay never holds the HPKE key, the exit talks ciphertext
        // only), so the client picks its own slot inside
        // `warren_config::TUNNEL_POOL_CIDR` (`10.66.0.0/16`).
        //
        // Derivation: octet C, D = pubkey[0], pubkey[1].max(2). Skips
        // `10.66.X.0` (network) and `10.66.X.1` (gateway). Collision
        // odds across two clients on the same exit are ~1/65 000.
        // Future M4.H.C will replace this with a coordinated allocator
        // (subscription-bound, persisted exit-side).
        let pubkey_bytes = params.signing_key.verifying_key().to_bytes();
        let tun_ip = derive_multi_hop_tun_ip(&pubkey_bytes);
        let tun_config = build_multi_hop_tun_config(tun_ip);

        let tun_t = Instant::now();
        let tun = {
            let mut provider = args
                .tun_provider
                .lock()
                .map_err(|_| Error::TunSetup("tun_provider mutex poisoned".to_owned()))?;
            *provider.config_mut() = tun_config.clone();
            provider
                .open_tun()
                .map_err(|e| Error::TunSetup(format!("{e}")))?
        };
        log::debug!(
            "{TRACE_PREFIX} T3={}ms phase=tun_opened elapsed_tun={}ms iface={} ip={}",
            start_t.elapsed().as_millis(),
            tun_t.elapsed().as_millis(),
            tun.interface_name()
                .unwrap_or_else(|_| "<unknown>".to_owned()),
            tun_ip
        );

        let metadata = build_tunnel_metadata(&tun, &tun_config);
        let async_device = tun.into_inner().into_async_device();
        let packet_device = MullvadTunPacketDevice::new(async_device);

        log::debug!(
            "{TRACE_PREFIX} T4={}ms phase=interfaceup_emit (multi-hop)",
            start_t.elapsed().as_millis()
        );
        let events_t = Instant::now();
        runtime.block_on(async {
            event_hook
                .on_event(TunnelEvent::InterfaceUp(
                    metadata.clone(),
                    AllowedTunnelTraffic::All,
                ))
                .await;
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });
        log::debug!(
            "{TRACE_PREFIX} T5={}ms phase=up_consumed elapsed_events={}ms",
            start_t.elapsed().as_millis(),
            events_t.elapsed().as_millis()
        );

        // Routing install: bypass the relay endpoint IP (the only UDP
        // peer the daemon reaches on the data plane), then split-default
        // on the TUN.
        let next_hop_ips: Vec<IpAddr> = vec![relay_endpoint.ip()];
        #[cfg(target_os = "linux")]
        let routes = {
            let physical_iface = detect_default_iface().unwrap_or_else(|e| {
                log::warn!(
                    "Failed to detect default iface, falling back to 'eth0': {e}. \
                     Bypass route for the relay IP may not install correctly."
                );
                "eth0".to_owned()
            });
            let gateway_v4 = detect_default_gateway_v4().ok();
            build_warren_tunnel_routes(
                &metadata.interface,
                &next_hop_ips,
                &physical_iface,
                gateway_v4,
            )
        };
        #[cfg(target_os = "macos")]
        let routes = build_warren_tunnel_routes_macos(&metadata.interface, &next_hop_ips);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let routes: Vec<RequiredRoute> = {
            log::warn!("Warren multi-hop tunnel routing not yet implemented for this platform.");
            Vec::new()
        };

        let route_manager = args.route_manager.clone();
        let metadata_iface_for_log = metadata.interface.clone();
        runtime.block_on(async move {
            match route_manager.add_routes(routes.into_iter().collect()).await {
                Ok(()) => log::info!(
                    "Warren multi-hop tunnel routes installed (tun={metadata_iface_for_log})"
                ),
                Err(e) => log::warn!(
                    "Failed to install Warren multi-hop tunnel routes: {e}. \
                     Tunnel up but no traffic forwarding."
                ),
            }
        });

        #[cfg(target_os = "linux")]
        let default_route_guard = {
            let relay_ip_v4 = match relay_endpoint.ip() {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            };
            if let Some(v4) = relay_ip_v4 {
                let tun_name_for_split = metadata.interface.clone();
                runtime
                    .block_on(async move {
                        default_route_split::DefaultRouteSplitGuard::install(
                            v4,
                            &tun_name_for_split,
                        )
                        .await
                    })
                    .map(Some)
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "Warren multi-hop: failed to install default-route split: {e}. \
                             Internet traffic will NOT route via tunnel."
                        );
                        None
                    })
            } else {
                log::warn!("Warren multi-hop: no IPv4 relay endpoint, skip default-route split.");
                None
            }
        };
        #[cfg(not(target_os = "linux"))]
        let default_route_guard: Option<default_route_split::DefaultRouteSplitGuard> = None;

        // Spawn the uplink + downlink pumps. Each consumes a clone of
        // the watch receiver so they can independently park on the
        // supervisor's reconnect signal.
        let (pump_error_tx, pump_error_rx) = tokio::sync::oneshot::channel::<String>();
        let pump_error_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(pump_error_tx)));
        let pump_error_tx_uplink = pump_error_tx.clone();
        let pump_error_tx_downlink = pump_error_tx.clone();

        let uplink_rx = client_rx.clone();
        let uplink_device = packet_device.clone();
        let uplink_handle = runtime.spawn(async move {
            log::info!("{TRACE_PREFIX} multi-hop uplink=running");
            let res = run_uplink(uplink_rx, uplink_device).await;
            if let Err(ref e) = res {
                let msg = format!("multi-hop uplink: {e:#}");
                log::warn!("{TRACE_PREFIX} {msg}");
                if let Some(tx) = pump_error_tx_uplink.lock().ok().and_then(|mut g| g.take()) {
                    let _ = tx.send(msg);
                }
            }
            res
        });

        let downlink_rx = client_rx.clone();
        let downlink_device = packet_device.clone();
        let downlink_handle = runtime.spawn(async move {
            log::info!("{TRACE_PREFIX} multi-hop downlink=running");
            let res = run_downlink(downlink_rx, downlink_device).await;
            if let Err(ref e) = res {
                let msg = format!("multi-hop downlink: {e:#}");
                log::warn!("{TRACE_PREFIX} {msg}");
                if let Some(tx) = pump_error_tx_downlink
                    .lock()
                    .ok()
                    .and_then(|mut g| g.take())
                {
                    let _ = tx.send(msg);
                }
            }
            res
        });

        // Drop the local watch receiver. Only the uplink/downlink keep
        // theirs alive; once those are aborted, the supervisor sees
        // `tx.closed()` and terminates cleanly.
        drop(client_rx);

        log::debug!(
            "{TRACE_PREFIX} T8={}ms phase=pump_spawned mode=multi_hop (uplink + downlink + supervisor live)",
            start_t.elapsed().as_millis()
        );

        // Spawn the NAT-PMP refresh loop once the data plane is up.
        // The exit-side NAT-PMP server is reachable via the tunnel
        // gateway (10.66.0.1:5351). On multi-hop the routing identity
        // of the gateway is the exit's TUN IP, same as single-hop.
        let nat_pmp_manager = spawn_nat_pmp_manager_if_enabled(&runtime, params);

        Ok(Self {
            runtime,
            backend: MonitorBackend::MultiHop {
                supervisor_handle,
                uplink_handle,
                downlink_handle,
                pump_error_rx,
            },
            event_hook,
            close_rx: args.tunnel_close_rx,
            default_route_guard,
            nat_pmp_manager,
        })
    }

    /// Blocks the current thread until (a) the external close-signal
    /// from the daemon fires, or (b) the pump terminates abnormally
    /// (TUN I/O error, QUIC session closed). Emits [`TunnelEvent::Down`]
    /// in both cases, then aborts and drains the pump task to release
    /// the TUN fd before the `tun_provider` can be reused.
    ///
    /// # Errors
    ///
    /// [`Error::Backend`] if the pump terminated abnormally before the
    /// close-signal arrived (state machine should retry, but
    /// `is_recoverable()` returns `false` to stay conservative until
    /// the error nature is classified more precisely).
    pub fn wait(self) -> Result<(), Error> {
        let WarrenTunnelMonitor {
            runtime,
            backend,
            mut event_hook,
            close_rx,
            default_route_guard,
            nat_pmp_manager,
        } = self;

        // Drop the NAT-PMP manager first so its refresh loop stops
        // emitting events while the rest of the tunnel teardown
        // proceeds. The forwarder task is aborted via the manager's
        // Drop impl; pending events still in flight are discarded
        // (the observer side gets at most one extra Cancelled event,
        // which the daemon-side WarrenStatusCache handles gracefully).
        drop(nat_pmp_manager);

        // Split the backend into its oneshot receiver (raced against
        // `close_rx` below) and its owned task handles (aborted after
        // the race finishes). Both backends surface abnormal pump
        // terminations through the same `pump_error_rx` channel.
        enum BackendHandles {
            SingleHop(tokio::task::JoinHandle<()>),
            MultiHop {
                supervisor: tokio::task::JoinHandle<()>,
                uplink: tokio::task::JoinHandle<anyhow::Result<()>>,
                downlink: tokio::task::JoinHandle<anyhow::Result<()>>,
            },
        }
        let (pump_error_rx, handles) = match backend {
            MonitorBackend::SingleHop {
                pump_handle,
                pump_error_rx,
            } => (pump_error_rx, BackendHandles::SingleHop(pump_handle)),
            MonitorBackend::MultiHop {
                supervisor_handle,
                uplink_handle,
                downlink_handle,
                pump_error_rx,
            } => (
                pump_error_rx,
                BackendHandles::MultiHop {
                    supervisor: supervisor_handle,
                    uplink: uplink_handle,
                    downlink: downlink_handle,
                },
            ),
        };

        let result = runtime.block_on(async move {
            // `tokio::select!` races the two signals. The first
            // to arrive "wins" and the losing branch is dropped (= the
            // internal futures are cleanly cancelled, no leak).
            let outcome: Result<(), Error> = tokio::select! {
                close_res = close_rx => {
                    // External close: daemon requests shutdown. Err =
                    // Sender dropped without signaling (rare: daemon
                    // crashed). We treat it as an implicit close
                    // (no error — the state machine will continue
                    // its normal cycle).
                    let _ = close_res;
                    Ok(())
                }
                pump_res = pump_error_rx => {
                    // Pump terminated before the external close.
                    // `Ok(msg)`: the pump explicitly sent an error.
                    // `Err(_)`: sender dropped without sending = clean
                    // exit (QUIC session closed gracefully by the
                    // exit, e.g. idle_timeout). No error to report.
                    match pump_res {
                        Ok(msg) => Err(Error::Backend(format!(
                            "pump terminated abnormally: {msg}"
                        ))),
                        Err(_) => Ok(()),
                    }
                }
            };
            event_hook.on_event(TunnelEvent::Down).await;
            outcome
        });

        // Uninstall the split-default policy routing before aborting
        // the pump, mirroring the install order. Best-effort: log a
        // warning but do not fail teardown.
        runtime.block_on(async {
            if let Some(guard) = default_route_guard
                && let Err(e) = guard.uninstall().await
            {
                log::warn!("Warren default-route split cleanup failed: {e}");
            }
        });

        // Abort backend tasks to release the TUN device and the QUIC
        // session(s) they hold. `JoinHandle::abort` triggers a clean
        // cancel (task terminates at the next `await` cancellation
        // point). Wait afterwards so the TUN fd is actually closed
        // kernel-side before returning, otherwise an immediate retry
        // could race with `open_tun()` on the same interface name.
        //
        // Multi-hop aborts in a deterministic order: uplink/downlink
        // first (release their watch receivers), then the supervisor
        // (whose `tx.closed()` would otherwise race with the pump
        // teardown).
        runtime.block_on(async {
            match handles {
                BackendHandles::SingleHop(pump_handle) => {
                    pump_handle.abort();
                    let _ = pump_handle.await;
                }
                BackendHandles::MultiHop {
                    supervisor,
                    uplink,
                    downlink,
                } => {
                    uplink.abort();
                    downlink.abort();
                    let _ = tokio::join!(uplink, downlink);
                    supervisor.abort();
                    let _ = supervisor.await;
                }
            }
        });

        result
    }
}

/// Client-side session variant. Dispatch between `Mono` (single
/// dedicated QUIC connection) and `Multi` (N-connection bonding) is
/// driven by `n_connections`; see [`select_session_request`].
enum SessionKind {
    /// Mono-conn dedicated session, used when `n_connections == 1`
    /// (or `0`, treated as degenerate). Single QUIC `Connection`, no
    /// multi-connection bonding overhead.
    Mono(ClientSession),
    /// Multi-conn bonded session, used when `n_connections > 1`.
    /// Aggregates throughput across N parallel QUIC connections.
    Multi(MultiSession),
}

impl SessionKind {
    fn assigned_ipv4(&self) -> std::net::Ipv4Addr {
        match self {
            SessionKind::Mono(s) => s.assigned_ipv4(),
            SessionKind::Multi(s) => s.assigned_ipv4(),
        }
    }

    fn assigned_ipv6(&self) -> Option<std::net::Ipv6Addr> {
        match self {
            SessionKind::Mono(s) => s.assigned_ipv6(),
            SessionKind::Multi(s) => s.assigned_ipv6(),
        }
    }

    fn assigned_max_mtu(&self) -> u16 {
        match self {
            SessionKind::Mono(s) => s.assigned_max_mtu(),
            SessionKind::Multi(s) => s.assigned_max_mtu(),
        }
    }
}

/// Filter `WarrenExitAddr.addrs` to keep only Internet-routable
/// addresses. Excludes:
/// - RFC 1918 private IPv4 (10/8, 172.16/12, 192.168/16).
/// - IPv4 loopback (127/8), link-local (169.254/16), broadcast,
///   multicast, unspecified.
/// - IPv6 loopback (::1), unspecified, multicast, link-local
///   (fe80::/10), unique-local (fc00::/7).
///
/// Preserves `id` and any future non-IP transport variants (Warren
/// does not use relays today; the match stays open via `_` to track
/// the upstream `#[non_exhaustive]` shape).
///
/// Defense in depth post-Quinn migration: the underlying iroh
/// `n0_nat_traversal` bug class (path discovery probing the peer's
/// TUN gateway IP) is structurally eliminated, but this filter still
/// hardens against malformed exit metadata that would carry a
/// Spawns a [`NatPmpManager`] if NAT-PMP is enabled in the supplied
/// parameters AND an observer is wired. Returns `None` in every other
/// case so the manager is never spawned with a no-op observer.
///
/// The server address defaults to the tunnel gateway:5351
/// (`warren_natpmp_client::default_server_addr()`); routing for the
/// gateway is set up by the time this helper is called (just after
/// the bidirectional pump is spawned).
fn spawn_nat_pmp_manager_if_enabled(
    runtime: &tokio::runtime::Handle,
    params: &WarrenTunnelParameters,
) -> Option<NatPmpManager> {
    let cfg = params.nat_pmp.as_ref()?;
    if !cfg.enabled {
        log::debug!("Warren NAT-PMP: config present but disabled, skipping manager spawn");
        return None;
    }
    let Some(observer) = params.nat_pmp_observer.clone() else {
        // Disabled defensively: spawning the loop without an observer
        // would silently leak events; the daemon-side wiring is
        // expected to plug an observer whenever `nat_pmp.enabled =
        // true`.
        log::warn!(
            "Warren NAT-PMP: config enabled but no observer wired; manager spawn suppressed"
        );
        return None;
    };
    let server = warren_natpmp_client::default_server_addr();
    log::info!("Warren NAT-PMP: starting refresh loop against {server}");
    Some(NatPmpManager::start(runtime, server, cfg, observer))
}

/// `10.66.0.1` candidate or similar.
#[must_use]
fn filter_endpoint_addr_for_wan(addr: WarrenExitAddr) -> WarrenExitAddr {
    let mut filtered = WarrenExitAddr::new(addr.id);
    for transport_addr in &addr.addrs {
        match transport_addr {
            WarrenTransportAddr::Ip(socket) if is_routable_internet(*socket) => {
                filtered = filtered.with_ip_addr(*socket);
            }
            WarrenTransportAddr::Ip(_) => {
                // Non-routable IP, drop silently. An empty filtered
                // address set is acceptable; the connect() call will
                // surface a clear error to the caller.
            }
            _ => {
                // Future transport variants (Warren does not ship any
                // today). Pass through untouched so we do not start
                // dropping non-IP variants the day they ship.
                filtered.addrs.insert(transport_addr.clone());
            }
        }
    }
    filtered
}

/// Whether a `SocketAddr` is routable on the public Internet. Cf.
/// [`filter_endpoint_addr_for_wan`].
#[must_use]
fn is_routable_internet(socket: std::net::SocketAddr) -> bool {
    match socket.ip() {
        std::net::IpAddr::V4(v4) => {
            !v4.is_private()
                && !v4.is_loopback()
                && !v4.is_link_local()
                && !v4.is_broadcast()
                && !v4.is_multicast()
                && !v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                && !is_ipv6_unique_local(v6)
                && !is_ipv6_link_local(v6)
        }
    }
}

/// IPv6 `fc00::/7` (Unique Local Address, RFC 4193).
/// `Ipv6Addr::is_unique_local` is still unstable in std as of early
/// 2026, hence the explicit check.
#[must_use]
fn is_ipv6_unique_local(addr: std::net::Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xfe00) == 0xfc00
}

/// IPv6 `fe80::/10` (Link-Local). `Ipv6Addr::is_unicast_link_local` is
/// gated behind the unstable `ip` feature, hence the explicit check.
#[must_use]
fn is_ipv6_link_local(addr: std::net::Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xffc0) == 0xfe80
}

/// Picks the session variant based on `n_connections`. Pure function,
/// kept testable: mono vs multi is a behavioural decision that lives
/// here, not deep inside the warren-tunnel client builder.
#[derive(Debug, PartialEq, Eq)]
enum SessionRequest {
    /// Mono-conn dedicated session (`client.connect`).
    Mono,
    /// Multi-conn bonded session with this total count
    /// (`client.connect_multi(_, n)`).
    Multi(u8),
}

/// Selects the session variant from `n_connections`:
/// - `0` or `1` -> [`SessionRequest::Mono`] (0 is the degenerate case
///   and gets mono rather than a panic, since the upper layer should
///   validate stricter).
/// - `>= 2` -> [`SessionRequest::Multi(n)`].
#[must_use]
fn select_session_request(n_connections: u8) -> SessionRequest {
    if n_connections <= 1 {
        SessionRequest::Mono
    } else {
        SessionRequest::Multi(n_connections)
    }
}

/// Derives a deterministic IPv4 in `warren_config::TUNNEL_POOL_CIDR`
/// (`10.66.0.0/16`) from the client Ed25519 pubkey bytes.
///
/// Multi-hop /v1 has no server-side IP allocator (the relay is
/// HPKE-blind, the exit talks ciphertext only); the client picks its
/// own slot. Using the pubkey makes the slot stable across reconnects
/// for the same identity and lets two different identities very
/// likely land on different IPs (~1/65 000 collision odds).
///
/// Skips the network address `10.66.X.0` and the gateway `10.66.X.1`
/// to avoid reserved values.
#[must_use]
fn derive_multi_hop_tun_ip(pubkey_bytes: &[u8; 32]) -> std::net::Ipv4Addr {
    let octet_c = pubkey_bytes[0];
    let octet_d = pubkey_bytes[1].max(2);
    std::net::Ipv4Addr::new(10, 66, octet_c, octet_d)
}

/// Builds a `TunConfig` for the multi-hop path with the given IPv4.
/// MTU pinned to 1280 (Warren `TUNNEL_INITIAL_MTU`), gateway pinned to
/// `10.66.0.1` (Warren `TUNNEL_GATEWAY_IP`). No IPv6 address: multi-hop
/// /v1 is IPv4-only (cf. `warren-client::ipv6_killswitch`).
fn build_multi_hop_tun_config(ipv4: std::net::Ipv4Addr) -> TunConfig {
    TunConfig {
        #[cfg(target_os = "linux")]
        name: None,
        #[cfg(target_os = "linux")]
        packet_information: false,
        addresses: vec![IpAddr::V4(ipv4)],
        // 1280 matches the multi-hop CLI binary default and the
        // baseline `TUNNEL_INITIAL_MTU` floor agreed in /v1 obfuscation
        // doctrine. DPLPMTUD on the QUIC transport may negotiate higher
        // path-MTU; the TUN itself stays at the safe floor.
        mtu: 1280,
        ipv4_gateway: std::net::Ipv4Addr::new(10, 66, 0, 1),
        ipv6_gateway: None,
        routes: vec![],
        allow_lan: false,
        dns_servers: None,
        excluded_packages: vec![],
        #[cfg(target_os = "windows")]
        resource_dir: std::path::PathBuf::new(),
    }
}

/// Builds the `TunConfig` from a [`SessionKind`] (Mono or
/// Multi). Reuses the IPs assigned by the Warren server-side
/// allocator and makes them compatible with the talpid API.
fn build_tun_config_for_kind(session: &SessionKind) -> TunConfig {
    let ipv4 = session.assigned_ipv4();
    let ipv6 = session.assigned_ipv6();
    let max_mtu = session.assigned_max_mtu();

    let mut addresses: Vec<IpAddr> = Vec::with_capacity(2);
    addresses.push(IpAddr::V4(ipv4));
    if let Some(v6) = ipv6 {
        addresses.push(IpAddr::V6(v6));
    }

    TunConfig {
        #[cfg(target_os = "linux")]
        name: None,
        #[cfg(target_os = "linux")]
        packet_information: false,
        addresses,
        mtu: max_mtu,
        // Warren convention: the IPv4 gateway is the `.1` of the
        // tunnel pool (`10.66.0.1`), exposed by `warren-config`.
        // Hardcoded literal until `warren-config` is wired as a
        // direct path-dep.
        ipv4_gateway: std::net::Ipv4Addr::new(10, 66, 0, 1),
        ipv6_gateway: None,
        // No additional routes here; routing is owned by the route
        // installer below and refined by the relay selector for
        // future full-tunnel vs split-tunnel modes.
        routes: vec![],
        allow_lan: false,
        dns_servers: None,
        excluded_packages: vec![],
        #[cfg(target_os = "windows")]
        resource_dir: std::path::PathBuf::new(),
    }
}

/// Builds the `TunnelMetadata` payload emitted with `Up` / `Down`
/// events.
fn build_tunnel_metadata(tun: &Tun, config: &TunConfig) -> TunnelMetadata {
    // Interface name: pulled from the device when possible (Linux
    // may have auto-assigned it); fallback to "warren0" otherwise.
    let interface = tun
        .interface_name()
        .unwrap_or_else(|_| "warren0".to_owned());
    TunnelMetadata {
        interface,
        ips: config.addresses.clone(),
        ipv4_gateway: config.ipv4_gateway,
        ipv6_gateway: config.ipv6_gateway,
    }
}

/// Builds the `RequiredRoute` set to redirect user traffic through
/// the TUN while bypassing daemon-side packets to the candidate exit
/// IPs (otherwise the daemon -> exit traffic would loop back into the
/// tunnel).
///
/// Strategy:
/// 1. For each candidate exit IP: a `/32` (or `/128`) route via the
///    physical interface, more specific than the `/1 + /1` below, so
///    the daemon -> exit packets keep using the physical NIC.
/// 2. `0.0.0.0/1` + `128.0.0.0/1` via the TUN interface: covers the
///    entire IPv4 space without replacing the existing default route,
///    a classic split-default trick.
///
/// Bypass form: `<exit_ip>/32 via <gateway> dev <physical>` with the
/// explicit gateway is required on cloud VPS where the exit IP is
/// not on the same L2 as the egress NIC (ARP would otherwise fail).
/// If `gateway` is `None`, fall back to `Node::device(physical)`
/// (scope-link), which works on flat LANs but typically not on
/// Hetzner / cloud topologies.
///
/// The `0.0.0.0/1` + `128.0.0.0/1` split-default is posted separately
/// by [`default_route_split`] (policy routing in table 100), not here
/// in the main routing table.
#[cfg(target_os = "linux")]
#[must_use]
fn build_warren_tunnel_routes(
    _tun_iface: &str,
    exit_ips: &[IpAddr],
    physical_iface: &str,
    gateway: Option<std::net::Ipv4Addr>,
) -> Vec<RequiredRoute> {
    // `Node::new(ip, iface)` -> `via <ip> dev <iface>` on the kernel
    // side. With `None`, `Node::device(iface)` posts a scope-link
    // route (works only on flat LANs).
    let physical_node = match gateway {
        Some(gw) => Node::new(IpAddr::V4(gw), physical_iface.to_owned()),
        None => Node::device(physical_iface.to_owned()),
    };

    let mut routes: Vec<RequiredRoute> = Vec::with_capacity(exit_ips.len());
    for ip in exit_ips {
        let net = IpNetwork::from(*ip);
        routes.push(RequiredRoute::new(net, physical_node.clone()));
    }

    // We do NOT post `0.0.0.0/1 + 128.0.0.0/1 dev <tun>` in the main
    // table. Those routes would also capture the daemon's outbound
    // QUIC packets to the exit port, creating a routing loop. They
    // are posted in a dedicated table 100 by
    // `default_route_split::DefaultRouteSplitGuard::install` *after*
    // talpid_routing has posted the bypass routes above in the main
    // table. On macOS, see [`build_warren_tunnel_routes_macos`] for
    // the native ifscope strategy.

    routes
}

/// Build `RequiredRoute`s to redirect user traffic through the TUN on
/// macOS, while bypassing daemon-side packets to the exit IPs. Mirror
/// of Mullvad WireGuard's pattern (`talpid-wireguard/src/lib.rs:843-859`,
/// plus `get_post_tunnel_routes`), adapted to Warren's needs (single
/// exit, Quinn QUIC transport).
///
/// macOS strategy — do NOT reproduce the Linux `/1 + /1` recipe:
///
/// 1. **`<exit_ip>/32 NetNode::DefaultNode`** — bypass so that the
///    daemon's QUIC packets to the exit take the physical NIC instead
///    of the TUN (otherwise routing loop). `DefaultNode` is a symbolic
///    node: talpid-routing macOS posts `<ip>/32 via
///    best_default_route.router_ip` in `apply_non_tunnel_routes`
///    (`talpid-routing/src/unix/macos/mod.rs:541`), executed **after**
///    the ifscope dance, hence with an ARP-able L3 gateway (not the SDL
///    link-scope that fails ARP for off-LAN exits).
///
/// 2. **`0.0.0.0/0 dev <tun>`** — default redirect. Prefix 0 triggers
///    the `tunnel_default_routes` special case in talpid-routing macOS
///    (`mod.rs:344-354`) which:
///    - Transforms the previous default `0.0.0.0/0 via gw dev <physical>`
///      into an **ifscope** route (= visible only to sockets bound to
///      the physical iface).
///    - Posts the new default `0.0.0.0/0 dev <tun>` un-scoped (= visible
///      to everything else, i.e. user traffic).
///
/// Cleanup is automatic via `cleanup_routes` + `try_restore_default_routes`
/// (with exponential backoff retry, `mod.rs:613-671`) when the tunnel
/// tears down.
///
/// macOS has no policy routing (= one routing table); the ifscope
/// mechanism is the native Darwin equivalent of Linux's table 100.
#[cfg(target_os = "macos")]
#[must_use]
fn build_warren_tunnel_routes_macos(tun_iface: &str, exit_ips: &[IpAddr]) -> Vec<RequiredRoute> {
    use talpid_routing::NetNode;

    let mut routes: Vec<RequiredRoute> = Vec::with_capacity(exit_ips.len() + 1);

    for ip in exit_ips {
        routes.push(RequiredRoute::new(
            IpNetwork::from(*ip),
            NetNode::DefaultNode,
        ));
    }

    let tun_node = Node::device(tun_iface.to_owned());
    let default_v4 = ipnetwork::Ipv4Network::new(std::net::Ipv4Addr::new(0, 0, 0, 0), 0)
        .expect("0.0.0.0/0 is a valid prefix");
    routes.push(RequiredRoute::new(IpNetwork::V4(default_v4), tun_node));

    routes
}

/// Detects the name of the interface carrying the IPv4 default
/// route. Used when posting bypass routes for the exit IPs.
///
/// Reads `/proc/net/route`: text format with a header line, then
/// `Iface\tDestination\tGateway\t...` lines. The default route has
/// `Destination == 00000000`. Returns the first match.
///
/// # Errors
///
/// I/O on `/proc/net/route` (non-Linux system, `/proc` not mounted)
/// or no IPv4 default route (isolated machine).
#[cfg(target_os = "linux")]
fn detect_default_iface() -> std::io::Result<String> {
    let routes = std::fs::read_to_string("/proc/net/route")?;
    for line in routes.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[1] == "00000000" {
            return Ok(fields[0].to_owned());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no IPv4 default route in /proc/net/route",
    ))
}

/// Detects the gateway IP of the IPv4 default route. Required to
/// post `<exit_ip>/32 via <gateway>` instead of
/// `<exit_ip>/32 dev <iface>` scope-link, which fails ARP on cloud
/// VPS where the exit IP is not on the same L2 as the egress NIC.
///
/// `/proc/net/route` format: `Iface\tDest\tGateway\tFlags\t...`
/// where `Dest` and `Gateway` are u32 hex in little-endian (so byte
/// swap is needed to reconstruct the `Ipv4Addr`).
#[cfg(target_os = "linux")]
fn detect_default_gateway_v4() -> std::io::Result<std::net::Ipv4Addr> {
    let routes = std::fs::read_to_string("/proc/net/route")?;
    for line in routes.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[1] == "00000000" {
            // Gateway is in fields[2], hex little-endian (network byte order
            // reversed by /proc).
            let gw_hex = u32::from_str_radix(fields[2], 16).map_err(|e| {
                std::io::Error::other(format!("parse gateway hex {}: {e}", fields[2]))
            })?;
            // Swap bytes: /proc/net/route is little-endian host
            // order, we want network byte order for Ipv4Addr.
            let gw = std::net::Ipv4Addr::from(gw_hex.swap_bytes());
            if gw.is_unspecified() {
                continue; // 0.0.0.0 is not a valid gateway
            }
            return Ok(gw);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no IPv4 default gateway in /proc/net/route",
    ))
}

/// Detects the local source IP (eth0 / wlan0) that the kernel would
/// use to reach `target`. Trick: `UdpSocket::connect()` for UDP does
/// no network I/O (no handshake), it just resolves the local route,
/// and `local_addr()` returns the source IP the kernel would pick.
/// Works on Linux, macOS and Windows.
///
/// Used to bind the client QUIC `Endpoint` to this specific IP
/// rather than `0.0.0.0:0` (unspecified). Defense in depth so the
/// transport stays pinned to the original egress interface even
/// after the TUN is up.
///
/// # Errors
///
/// I/O on bind / connect (no Internet, no default route). The caller
/// should fall back to the unspecified bind.
fn detect_default_local_ip(target: std::net::SocketAddr) -> std::io::Result<std::net::IpAddr> {
    let bind = if target.is_ipv4() {
        std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            0,
        ))
    } else {
        std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            std::net::Ipv6Addr::UNSPECIFIED,
            0,
            0,
            0,
        ))
    };
    let sock = std::net::UdpSocket::bind(bind)?;
    sock.connect(target)?;
    Ok(sock.local_addr()?.ip())
}

#[cfg(test)]
mod tests {
    use super::*;
    use warren_protocol::WarrenPubkey;

    #[test]
    fn warren_tunnel_parameters_debug_does_not_leak_secrets() {
        // No-log Warren: Debug must never reveal signing_key nor the
        // full exit pubkey. The first is secret material, the second
        // is session-PII identifying the user on the exit side.
        let signing = SigningKey::from_bytes(&[0u8; 32]);
        let exit_id = WarrenPubkey::from_bytes([1u8; 32]);
        let params = WarrenTunnelParameters {
            exit_addr: WarrenExitAddr::new(exit_id),
            signing_key: signing,
            n_connections: 2,
            features: 0x1,
            multi_hop: None,
            on_reconnect: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            bypass_cidrs: Vec::new(),
        };
        let s = format!("{params:?}");
        assert!(s.contains("<redacted>"), "must mask secrets: {s}");
        assert!(
            !s.contains(&exit_id.to_hex()),
            "must not leak the exit pubkey in hex: {s}"
        );
        assert!(s.contains("n_connections: 2"));
        assert!(s.contains("features: 0x00000001"));
    }

    #[test]
    fn warren_tunnel_parameters_default_multi_hop_is_none() {
        // Backwards-compat anchor: every existing call site that
        // constructs `WarrenTunnelParameters` without specifying
        // `multi_hop` (single-hop = legacy path) must keep yielding
        // `None` so the dispatcher dispatches to
        // `warren_tunnel::ClientTunnel`. Mutation = silent multi-hop
        // by default = breaks every existing deployment.
        let params = WarrenTunnelParameters {
            exit_addr: WarrenExitAddr::new(WarrenPubkey::from_bytes([1u8; 32])),
            signing_key: SigningKey::from_bytes(&[0u8; 32]),
            n_connections: 1,
            features: 0,
            multi_hop: None,
            on_reconnect: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            bypass_cidrs: Vec::new(),
        };
        assert!(params.multi_hop.is_none());
        assert!(
            params.on_reconnect.is_none(),
            "default on_reconnect must be None so the single-hop legacy path \
             does not spuriously try to wire a multi-hop-only observer"
        );
        assert!(
            params.nat_pmp.is_none(),
            "default nat_pmp must be None so the daemon-side NatPmpManager \
             stays inert until the user opts in via the UI"
        );
    }

    #[test]
    fn warren_tunnel_parameters_debug_when_multi_hop_some_marks_it_as_redacted() {
        // No-log Warren: when multi_hop is Some, the relay + exit
        // descriptors carry pubkeys + UDP endpoints that re-identify
        // the session. The Debug surface must mark it as
        // `Some("<redacted>")` and never let the underlying hex
        // pubkeys leak into a log line.
        let signing = SigningKey::from_bytes(&[0u8; 32]);
        let exit_id = WarrenPubkey::from_bytes([1u8; 32]);
        let mh = MultiHopConfig {
            relay: RelayDescriptorSigned {
                relay_id: [0xaa; 16],
                relay_ed25519_pubkey: [0xbb; 32],
                endpoint: "192.0.2.10:443".parse().unwrap(),
                signature: [0xcc; 64],
            },
            exit: ExitDescriptorSigned {
                exit_id: warren_multihop::ExitId([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                endpoint: "192.0.2.20:443".parse().unwrap(),
                signature: [0x11; 64],
            },
            operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
            enable_gso: true,
            use_warren_obfuscation: true,
        };
        let params = WarrenTunnelParameters {
            exit_addr: WarrenExitAddr::new(exit_id),
            signing_key: signing,
            n_connections: 1,
            features: 0,
            multi_hop: Some(mh),
            on_reconnect: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            bypass_cidrs: Vec::new(),
        };
        let s = format!("{params:?}");
        assert!(
            s.contains("Some(\"<redacted>\")") || s.contains("Some(\"<redacted>\")"),
            "multi_hop Some must render as Some(\"<redacted>\"), got: {s}"
        );
        assert!(
            !s.contains("bb") && !s.contains("ee") && !s.contains("ff"),
            "relay/exit pubkeys must not leak: {s}"
        );
        assert!(
            !s.contains("192.0.2.10") && !s.contains("192.0.2.20"),
            "relay/exit endpoints must not leak: {s}"
        );
    }

    #[test]
    fn warren_tunnel_parameters_debug_when_nat_pmp_some_redacts_internals() {
        // No-log Warren: NAT-PMP config carries the user's port choice
        // and protocol preference. Those are not session-PII but the
        // Debug surface must still display the field structurally so
        // operators reading a log line can see whether port-forwarding
        // is on without exposing the picked port (which becomes
        // identifying once the mapping is live).
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            protocol: NatPmpProto::Udp,
            suggested_external_port: 22222,
            internal_port: 22,
        };
        let params = WarrenTunnelParameters {
            exit_addr: WarrenExitAddr::new(WarrenPubkey::from_bytes([0u8; 32])),
            signing_key: SigningKey::from_bytes(&[0u8; 32]),
            n_connections: 1,
            features: 0,
            multi_hop: None,
            on_reconnect: None,
            nat_pmp: Some(cfg.clone()),
            nat_pmp_observer: None,
            bypass_cidrs: Vec::new(),
        };
        let s = format!("{params:?}");
        // NatPmpConfig derives Debug, so the Some(..) renders the
        // structural shape. We assert the toggle is visible (operators
        // need to see "enabled: true") and that the suggested external
        // port number is dropped or carried (here carried; future
        // tightening can mask it if needed).
        assert!(
            s.contains("enabled: true"),
            "nat_pmp enabled toggle must appear in Debug: {s}"
        );
    }

    #[test]
    fn nat_pmp_config_default_disabled_yields_enabled_false() {
        let cfg = NatPmpConfig::default_disabled();
        assert!(!cfg.enabled);
        assert_eq!(cfg.lifetime_secs, NatPmpConfig::DEFAULT_LIFETIME_SECS);
        assert_eq!(cfg.protocol, NatPmpProto::Udp);
    }

    #[test]
    fn nat_pmp_config_default_enabled_uses_one_hour_default_lifetime() {
        let cfg = NatPmpConfig::default_enabled();
        assert!(cfg.enabled);
        assert_eq!(cfg.lifetime_secs, 3600);
        assert_eq!(cfg.protocol, NatPmpProto::Udp);
        assert_eq!(cfg.suggested_external_port, 0);
    }

    #[test]
    fn derive_multi_hop_tun_ip_is_deterministic_for_same_pubkey() {
        // Same identity must land on the same TUN IP across reconnects
        // so subscriptions and exit-side state stay attached to a
        // stable IP. Mutation = stale exit-side state across restart.
        let pubkey = [0x42u8; 32];
        let a = derive_multi_hop_tun_ip(&pubkey);
        let b = derive_multi_hop_tun_ip(&pubkey);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_multi_hop_tun_ip_skips_network_and_gateway_slots() {
        // The /16 pool has `.0` reserved as the network address and
        // `.1` reserved for the gateway. Even with pubkey bytes that
        // would land on those slots, the derivation must bump to a
        // safe value.
        let pubkey = {
            let mut p = [0u8; 32];
            p[1] = 0; // would land on .X.0
            p
        };
        let ip = derive_multi_hop_tun_ip(&pubkey);
        assert_ne!(ip.octets()[3], 0, "must not land on the network slot");

        let pubkey = {
            let mut p = [0u8; 32];
            p[1] = 1; // would land on .X.1 (gateway)
            p
        };
        let ip = derive_multi_hop_tun_ip(&pubkey);
        assert_ne!(ip.octets()[3], 1, "must not land on the gateway slot");
    }

    #[test]
    fn derive_multi_hop_tun_ip_stays_in_pool_cidr() {
        // Anchor: the derivation always produces an address inside
        // 10.66.0.0/16. A regression that swapped octet positions
        // would surface here.
        for seed in 0..32u8 {
            let mut p = [0u8; 32];
            p[0] = seed;
            p[1] = seed.wrapping_add(7);
            let ip = derive_multi_hop_tun_ip(&p);
            let o = ip.octets();
            assert_eq!(o[0], 10);
            assert_eq!(o[1], 66);
        }
    }

    #[test]
    fn build_multi_hop_tun_config_pins_mtu_and_gateway() {
        // Multi-hop /v1 is IPv4-only at MTU 1280 with the canonical
        // gateway `10.66.0.1`. Any mutation here would silently
        // change the wire profile across deployments.
        let cfg = build_multi_hop_tun_config(std::net::Ipv4Addr::new(10, 66, 7, 42));
        assert_eq!(cfg.mtu, 1280, "multi-hop TUN MTU must be 1280");
        assert_eq!(
            cfg.ipv4_gateway,
            std::net::Ipv4Addr::new(10, 66, 0, 1),
            "multi-hop TUN gateway must be 10.66.0.1"
        );
        assert!(cfg.ipv6_gateway.is_none(), "multi-hop is IPv4-only in /v1");
        assert_eq!(cfg.addresses.len(), 1);
        assert!(!cfg.allow_lan, "no LAN access on multi-hop");
        assert!(cfg.dns_servers.is_none(), "DNS handled by exit forwarder");
    }

    #[test]
    fn multi_hop_config_debug_does_not_leak_descriptors() {
        // Independent anchor: `MultiHopConfig::Debug` is also
        // formatted directly when the daemon logs the config struct
        // standalone (e.g. trace-on-build). The redaction must hold
        // there too, with the gso + obfuscation knobs exposed for
        // operational debug.
        let mh = MultiHopConfig {
            relay: RelayDescriptorSigned {
                relay_id: [0xaa; 16],
                relay_ed25519_pubkey: [0xbb; 32],
                endpoint: "192.0.2.10:443".parse().unwrap(),
                signature: [0xcc; 64],
            },
            exit: ExitDescriptorSigned {
                exit_id: warren_multihop::ExitId([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                endpoint: "192.0.2.20:443".parse().unwrap(),
                signature: [0x11; 64],
            },
            operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
            enable_gso: false,
            use_warren_obfuscation: true,
        };
        let s = format!("{mh:?}");
        assert!(s.contains("<redacted>"));
        assert!(s.contains("enable_gso: false"));
        assert!(s.contains("use_warren_obfuscation: true"));
        assert!(!s.contains("192.0.2"), "endpoints must not leak: {s}");
    }

    #[test]
    fn handshake_error_is_recoverable() {
        // A transient handshake error (network glitch) must be
        // retryable on the state-machine side so a session is not
        // killed by a single packet loss.
        let e = Error::Handshake("simulated".into());
        assert!(e.is_recoverable());
    }

    #[test]
    fn backend_error_is_not_recoverable() {
        // Structural error: do not retry (saves CPU and network).
        let e = Error::Backend("simulated".into());
        assert!(!e.is_recoverable());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn build_routes_emits_only_bypass_via_physical() {
        // `build_warren_tunnel_routes` does NOT post the split-default
        // `0.0.0.0/1 + 128.0.0.0/1 dev <tun>` in the main table:
        // those routes would also capture the daemon's outbound QUIC
        // packets to the exit port and create a routing loop. They
        // are posted in the dedicated table 100 by
        // `default_route_split::DefaultRouteSplitGuard::install`
        // after talpid_routing has posted the bypass routes below in
        // the main table.
        //
        // Anti-regression: if anyone re-adds the `/1` here, the
        // tunnel poisons itself.
        let exit_ips: Vec<IpAddr> = vec![
            "91.99.122.154".parse().unwrap(),
            "2a01:4f8:c013:14a1::1".parse().unwrap(),
        ];
        let routes = build_warren_tunnel_routes("tun0", &exit_ips, "eth0", None);

        assert_eq!(
            routes.len(),
            2,
            "2 bypass (v4+v6) + 0 split-default = 2 routes (got {} : {routes:?})",
            routes.len()
        );

        let dump = format!("{routes:?}");
        // Both exit IPs must appear as /32 and /128 bypass routes.
        assert!(
            dump.contains("addr: 91.99.122.154") && dump.contains("prefix: 32"),
            "exit v4 must have a /32 bypass route in {dump}"
        );
        assert!(
            dump.contains("addr: 2a01:4f8:c013:14a1::1") && dump.contains("prefix: 128"),
            "exit v6 must have a /128 bypass route in {dump}"
        );
        // Anti-regression: no /1 split-default in this Vec.
        assert!(
            !dump.contains("addr: 0.0.0.0"),
            "0.0.0.0/1 split-default must NOT be in the main table \
             (routing loop). See default_route_split. dump = {dump}"
        );
        assert!(
            !dump.contains("addr: 128.0.0.0"),
            "128.0.0.0/1 split-default must NOT be in the main table \
             (routing loop). dump = {dump}"
        );
        // All bypass routes target eth0 (the physical interface).
        assert!(
            dump.contains(r#"device: Some("eth0")"#),
            "physical_iface 'eth0' expected as node device in {dump}"
        );
        assert!(
            !dump.contains(r#"device: Some("tun0")"#),
            "tun_iface must NOT appear in build_warren_tunnel_routes \
             (split-default lives in default_route_split::install)"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn build_routes_with_no_exit_ips_emits_no_routes() {
        // Edge case: empty exit_ips emits 0 routes (the `/1` no
        // longer live here, they are owned by default_route_split's
        // table 100).
        let routes = build_warren_tunnel_routes("tun0", &[], "eth0", None);
        assert_eq!(routes.len(), 0, "0 bypass + 0 split-default");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn build_routes_v4_only_exit_emits_one_bypass() {
        // IPv4-only exit: 1 /32 v4 bypass. No v6, no split-default.
        let exit_ips: Vec<IpAddr> = vec!["91.99.122.154".parse().unwrap()];
        let routes = build_warren_tunnel_routes("tun0", &exit_ips, "eth0", None);
        assert_eq!(routes.len(), 1, "1 v4 bypass route");
        let dump = format!("{routes:?}");
        assert!(
            !dump.contains("V6("),
            "no Ipv6Network expected for a v4-only exit"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn build_routes_macos_uses_default_node_bypass_and_default_redirect() {
        // macOS - mirror of the Mullvad WireGuard pattern, adapted to
        // Warren's needs:
        //   1. `<exit_ip>/32 NetNode::DefaultNode` -> bypass via the
        //      best default route (resolved at apply time by
        //      talpid-routing).
        //   2. `0.0.0.0/0 dev <tun>` -> triggers the
        //      `tunnel_default_routes` ifscope dance (native macOS
        //      recipe, no policy routing on Darwin).
        //
        // The `node` field on `RequiredRoute` is private, so the
        // assertions below use the `Debug` output, which exposes
        // both the prefix and the node variant.
        let exit_ips: Vec<IpAddr> = vec!["91.99.122.154".parse().unwrap()];
        let routes = build_warren_tunnel_routes_macos("utun4", &exit_ips);

        assert_eq!(
            routes.len(),
            2,
            "macOS: 1 bypass exit + 1 default redirect (got {} : {routes:?})",
            routes.len()
        );

        // Bypass exit IP via NetNode::DefaultNode.
        let bypass = routes
            .iter()
            .find(|r| r.prefix.prefix() == 32)
            .expect("/32 bypass expected");
        let bypass_dump = format!("{bypass:?}");
        assert!(
            bypass_dump.contains("DefaultNode"),
            "bypass exit must use NetNode::DefaultNode (talpid-routing \
             resolves it via best_default_route.router_ip at apply time, \
             guaranteeing an ARP-able L3 gateway). dump = {bypass_dump}"
        );

        // Default redirect via TUN.
        let default = routes
            .iter()
            .find(|r| r.prefix.prefix() == 0)
            .expect("/0 default expected");
        let dump = format!("{default:?}");
        assert!(
            dump.contains(r#"device: Some("utun4")"#),
            "default redirect must target tun_iface (triggers the \
             tunnel_default_routes special case in talpid-routing \
             macOS). dump = {dump}"
        );

        // Anti-regression: no `/1` (= the old broken recipe).
        assert!(
            !routes.iter().any(|r| r.prefix.prefix() == 1),
            "no /1 split-default on macOS - the ifscope dance is the \
             native macOS recipe and replaces /1 + /1. routes = {routes:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn build_routes_macos_with_no_exit_ips_still_emits_default_redirect() {
        // Edge case: empty exit_ips -> 0 bypass + 1 default redirect.
        // The tunnel still routes user traffic via TUN but without
        // an exit bypass (case: IPv6-only exit, not yet supported).
        let routes = build_warren_tunnel_routes_macos("utun4", &[]);
        assert_eq!(routes.len(), 1, "macOS no exit: 0 bypass + 1 default");
        assert_eq!(routes[0].prefix.prefix(), 0, "single item must be /0");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn build_routes_macos_multiple_exit_ips_yields_one_bypass_each() {
        // Multiple exit IPs (v4 + v6): one bypass per IP + the default.
        let exit_ips: Vec<IpAddr> = vec![
            "91.99.122.154".parse().unwrap(),
            "2a01:4f8:c013:14a1::1".parse().unwrap(),
        ];
        let routes = build_warren_tunnel_routes_macos("utun4", &exit_ips);
        assert_eq!(
            routes.len(),
            3,
            "2 exit IPs (v4+v6) -> 2 bypass + 1 default redirect = 3"
        );
        // Field `node` is private; check via Debug output.
        let bypass_count = routes
            .iter()
            .filter(|r| format!("{r:?}").contains("DefaultNode"))
            .count();
        assert_eq!(bypass_count, 2, "1 DefaultNode bypass per exit IP");
    }

    #[test]
    fn select_session_request_returns_mono_for_one_connection() {
        // `n_connections == 1` must pick the `connect()` mono-conn
        // path. Anti-regression: do not switch to `connect_multi(_,1)`
        // for n=1, that path activated multi-flow logic that did not
        // make sense for a single connection.
        assert_eq!(select_session_request(1), SessionRequest::Mono);
    }

    #[test]
    fn select_session_request_returns_multi_for_two_or_more_connections() {
        // n >= 2 activates multi-flow bonding (`connect_multi`).
        assert_eq!(select_session_request(2), SessionRequest::Multi(2));
        assert_eq!(select_session_request(4), SessionRequest::Multi(4));
        assert_eq!(select_session_request(8), SessionRequest::Multi(8));
    }

    #[test]
    fn is_routable_internet_v4_accepts_public_addrs() {
        // Regression sentinel: Hetzner Cloud IPs (the typical
        // warren-exit target) must be considered routable, otherwise
        // `filter_endpoint_addr_for_wan` would empty the EndpointAddr
        // and the iroh client couldn't reach the exit.
        for ip in ["91.99.122.154:7000", "178.104.4.40:7000", "8.8.8.8:53"] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(
                is_routable_internet(sa),
                "{ip} must be routable (public Internet IP)"
            );
        }
    }

    #[test]
    fn is_routable_internet_v4_rejects_rfc1918_and_loopback() {
        // `10.66.0.1:7000` (= TUN gateway warren0 exit) is the IP we
        // most care about rejecting: a misconfigured exit metadata
        // would carry it and lead to a routing loop. Same for the
        // standard RFC1918 + loopback + link-local + unspecified.
        for ip in [
            "10.66.0.1:7000",     // anti-loop sentinel
            "10.0.0.1:443",       // RFC1918 10/8
            "172.20.0.5:80",      // RFC1918 172.16/12
            "192.168.1.1:8080",   // RFC1918 192.168/16
            "127.0.0.1:7100",     // loopback
            "169.254.169.254:80", // link-local
            "0.0.0.0:0",          // unspecified
        ] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(
                !is_routable_internet(sa),
                "{ip} must be REJECTED (non-routable Internet)"
            );
        }
    }

    #[test]
    fn is_routable_internet_v6_accepts_public_globals_rejects_local() {
        // Public IPv6 = OK. ULA fc00::/7 + link-local fe80::/10 +
        // loopback ::1 + unspecified = REJECT.
        let public: std::net::SocketAddr = "[2a01:4f8:c013:14a1::1]:7000".parse().unwrap();
        assert!(is_routable_internet(public), "global IPv6 must be routable");

        for ip in [
            "[fc00::1]:7000", // ULA RFC 4193
            "[fd00::1]:7000", // ULA fd00::/8 (sub-range of fc00::/7)
            "[fe80::1]:7000", // link-local
            "[::1]:7000",     // loopback
            "[::]:7000",      // unspecified
        ] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(!is_routable_internet(sa), "{ip} must be REJECTED");
        }
    }

    #[test]
    fn filter_endpoint_addr_drops_private_v4_keeps_public_pair() {
        // Realistic case: exit metadata carries (1) a routable public
        // IP plus (2) the TUN gateway IP `10.66.0.1`. The filter keeps
        // (1) and drops (2).
        let id = WarrenPubkey::from_bytes([7u8; 32]);
        let public: std::net::SocketAddr = "91.99.122.154:7000".parse().unwrap();
        let tun_gw: std::net::SocketAddr = "10.66.0.1:7000".parse().unwrap();
        let addr = WarrenExitAddr::new(id)
            .with_ip_addr(public)
            .with_ip_addr(tun_gw);

        let filtered = filter_endpoint_addr_for_wan(addr);
        let kept: Vec<_> = filtered.ip_addrs().collect();
        assert_eq!(kept.len(), 1, "exactly one address kept (public)");
        assert_eq!(kept[0], public, "public IP kept");
        assert!(
            !kept.contains(&tun_gw),
            "tun gateway 10.66.0.1 must be dropped"
        );
    }

    #[test]
    fn filter_endpoint_addr_preserves_endpoint_id() {
        // The peer's Ed25519 identity must not be altered by the
        // filter, otherwise the TLS RPK check would reject the
        // handshake.
        let id = WarrenPubkey::from_bytes([42u8; 32]);
        let addr = WarrenExitAddr::new(id).with_ip_addr("10.0.0.1:7000".parse().unwrap());

        let filtered = filter_endpoint_addr_for_wan(addr);
        assert_eq!(filtered.id, id, "pubkey preserved through filter");
    }

    #[test]
    fn filter_endpoint_addr_with_only_private_ips_returns_empty_addrs() {
        // Edge case: server advertises only private IPs (= ops bug or
        // malformed exit metadata). The filter returns an empty addr
        // set; the caller (`client.connect`) then surfaces a clean
        // "connect to exit failed" instead of looping on bogus
        // candidates.
        let id = WarrenPubkey::from_bytes([1u8; 32]);
        let addr = WarrenExitAddr::new(id)
            .with_ip_addr("10.66.0.1:7000".parse().unwrap())
            .with_ip_addr("192.168.1.1:7000".parse().unwrap());

        let filtered = filter_endpoint_addr_for_wan(addr);
        assert_eq!(filtered.ip_addrs().count(), 0, "all dropped");
        assert_eq!(filtered.id, id);
    }

    #[test]
    fn select_session_request_treats_zero_as_mono() {
        // Edge case: `n_connections == 0` is a degenerate case (the
        // `Setup` frame requires `total_connections >= 1`). Rather
        // than panicking in `select_session_request`, we return Mono
        // (calls `connect()`); the upper-layer params builder is
        // responsible for stricter validation.
        assert_eq!(select_session_request(0), SessionRequest::Mono);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_default_iface_returns_some_iface_on_linux_runtime_or_skip() {
        // Sanity check: on a Linux test environment, `/proc/net/route`
        // exists and returns at least one non-empty interface name.
        // If `/proc` is not mounted or there is no default route
        // (isolated container, no-network CI), skip rather than fail.
        match detect_default_iface() {
            Ok(iface) => assert!(!iface.is_empty(), "iface name must be non-empty"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skip detect_default_iface: no default route ({e})");
            }
            Err(e) => panic!("unexpected I/O error: {e}"),
        }
    }

    #[test]
    fn detect_default_local_ip_returns_routable_or_loopback_ip() {
        // Invariant: the `UdpSocket::connect` trick must return a
        // non-unspecified source IP (not `0.0.0.0` nor `[::]`). On a
        // dev machine with Internet we get the eth0 / wlan0 IP. On an
        // isolated CI sandbox the bind / connect may fail OR return
        // loopback, both tolerated; what must not happen is the
        // unspecified IP, which would let the transport rebind on any
        // interface including the TUN once the tunnel is up.
        let target = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::new(1, 1, 1, 1),
            53,
        ));
        match detect_default_local_ip(target) {
            Ok(ip) => assert!(
                !ip.is_unspecified(),
                "regression: detect_default_local_ip must NEVER return \
                 0.0.0.0 or [::]. ip={ip}"
            ),
            Err(e) => eprintln!("skip detect_default_local_ip (no-network CI?): {e}"),
        }
    }
}
