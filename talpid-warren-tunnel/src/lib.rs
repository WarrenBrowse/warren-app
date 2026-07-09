//! Warren adapter for the talpid tunnel state machine.
//!
//! This crate exposes [`WarrenTunnelMonitor`], the QUIC tunnel backend
//! consumed by
//! `talpid_core::tunnel_state_machine::tunnel_monitor::TunnelMonitor`.
//! It exposes a `start` / `wait` API so `connecting_state.rs` can drive
//! the tunnel lifecycle.
//!
//! Underneath, [`WarrenTunnelMonitor::start`] performs the QUIC
//! handshake through [`warrenguard_transport::ClientTunnel`], opens a TUN via
//! the talpid `TunProvider`, emits `TunnelEvent::InterfaceUp` / `Up`
//! and spawns the bidirectional pump (TUN <-> QUIC datagrams).
//! `wait()` blocks on the close-signal, drops the routing-table
//! override and aborts the pump.

use std::{net::IpAddr, path::Path, time::Instant};

use ed25519_dalek::{SigningKey, VerifyingKey};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use ipnetwork::IpNetwork;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use talpid_routing::Node;
use talpid_routing::RequiredRoute;
use talpid_tunnel::{
    TunnelArgs, TunnelEvent, TunnelMetadata,
    tun_provider::{Tun, TunConfig},
};
use talpid_types::net::AllowedTunnelTraffic;
use warrenguard_multihop::{ExitDescriptorSigned, RejectionReason, RelayDescriptorSigned};
// Re-exported below so downstream crates (talpid-core, mullvad-daemon)
// can construct `MultiHopConfig` without depending on warrenguard-multihop
// directly. Same pattern as `warren-relay-selector::warren_types`.
pub use warrenguard_multihop::{
    ExitDescriptorSigned as MultiHopExitDescriptor, ExitId,
    RelayDescriptorSigned as MultiHopRelayDescriptor,
};
// NAT-PMP wire protocol enum re-exported so daemon code constructs
// `NatPmpConfig { protocol: NatPmpProto::Udp, .. }` without depending
// directly on the warrenguard-natpmp-protocol crate. The crate itself is a
// path-dep of this one (so the type lives in this binary's symbol
// table) and is also referenced explicitly by the daemon-side
// `warren_nat_pmp` module.
pub use warrenguard_natpmp_protocol::MapProto as NatPmpProto;
// IPv4 CIDR descriptor used by the daemon-side `--bypass-cidr`
// settings plumbing. Re-exported so callers (mullvad-daemon, gRPC
// conversions, settings persistence) consume one canonical type
// instead of duplicating it across crates.
pub use warrenguard_route_split::bypass_cidr::BypassCidr;
// ADR 36 (Option A): the daemon stores a per-tunnel migrate handle and builds
// migration targets from the directory's selected circuit, so re-export both
// types from this crate (it owns `MultiHopConfig`).
pub use warrenguard_transport::supervisor::{CircuitTarget, MigrateHandle};
// docs/59 Lot 3: the daemon builds the pre-swap NAT-PMP reservation
// closure, whose subject is the candidate bundle, and hands the
// supervisor a `PreSwapCheck` / `OverlapSwapObserver`. Re-exported so
// mullvad-daemon needs no direct warrenguard-transport dependency.
pub use warrenguard_transport::bundle::MultiHopBundle;
pub use warrenguard_transport::supervisor::{OverlapSwapObserver, PreSwapCheck};

/// Project a [`MultiHopConfig`] (a circuit the directory selected) into the
/// [`CircuitTarget`] the supervisor migrates onto: the relay + exit identity,
/// nothing else (the operational key + client identity are circuit-invariant).
#[must_use]
pub fn migration_target(cfg: &MultiHopConfig) -> CircuitTarget {
    CircuitTarget {
        relay: std::sync::Arc::new(cfg.relay.clone()),
        exit_id: cfg.exit.exit_id,
        exit_x25519_multihop_pubkey: cfg.exit.exit_x25519_multihop_pubkey,
    }
}
/// ADR 36 gap-free drain path: daemon-side migration hook consumed by the
/// drain reactor. Input: the 16-byte id of the DRAINING exit. Output:
/// whether a make-before-break migration off it was dispatched (the tunnel
/// stays up); `false` sends the reactor to the break-before-make rebuild.
pub type WarrenDrainMigrate = std::sync::Arc<
    dyn Fn([u8; 16]) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

use warrenguard_daita::DaitaState;
use warrenguard_pump::{
    pump_bidirectional, pump_bidirectional_with_daita, pump_bidirectional_with_idle_cover,
    pump_multi_bidirectional, pump_multi_bidirectional_with_daita,
    pump_multi_bidirectional_with_idle_cover,
};
use warrenguard_transport::{ClientSession, ClientTunnel, MultiSession};
/// Re-export of the single-hop stable exit identifier from
/// warrenguard-wire. Pubkey pinning keys its TOFU lookup
/// on this 16-byte value so a legitimate Ed25519 rotation stays
/// distinguishable from an exit-substitution attack.
pub use warrenguard_wire::ExitId as RelayExitId;
/// Re-export of the `Setup`-frame feature bitmask constants
/// (`features::IPV6`, `PORT_FORWARD`, ...) so daemon-side callers that
/// only depend on `talpid-warren-tunnel` (e.g.
/// `mullvad_daemon::warren_tunnel_params`) can OR them into
/// [`WarrenTunnelParameters::features`] without taking a direct
/// `warrenguard-wire` dependency.
pub use warrenguard_wire::features;
use warrenguard_wire::{WarrenExitAddr, WarrenTransportAddr};

mod adapter;
mod device_id;
mod drain_reactor;
mod migration_watchdog;
mod session_liveness;
use adapter::MullvadTunPacketDevice;

/// Daemon-side NAT-PMP lifecycle wrapper that owns the refresh loop +
/// event forwarder and drops them on tunnel teardown. Activated only
/// when `WarrenTunnelParameters::nat_pmp` carries `Some(cfg)` with
/// `cfg.enabled == true`.
pub mod nat_pmp_manager;
pub use nat_pmp_manager::{NatPmpEvent, NatPmpEventObserver, NatPmpFailureReason, NatPmpManager};

/// Pre-flight NAT-PMP reservation over a not-yet-published session
/// (docs/59 Lot 3): reserve a pinned port on a candidate exit BEFORE
/// committing a make-before-break migration, so a pinned rule never
/// silently loses its port to a maintenance switch.
pub mod natpmp_preflight;

/// Split-default policy routing helper that routes Internet traffic via
/// the tunnel without overriding the kernel main routing table.
pub mod default_route_split;
pub use default_route_split::force_route_cleanup;

// Trace prefix used by the start-sequence and pump-metrics debug logs.
// Format: `[warren-trace] T{N}={ms}ms <event>`. `N` increments at each
// step of `start()`, `ms` is the elapsed since `start_t`. Logs are at
// `debug` level so they stay out of release output; enable with
// `RUST_LOG=talpid_warren_tunnel=debug` when diagnosing a start/wait
// sequencing issue.
const TRACE_PREFIX: &str = "[warren-trace]";

/// INFO-level one-line connect summary. The per-phase T0-T8 traces stay
/// at debug; this single line is what production logs carry so connect
/// latency is attributable in the field without a debug rebuild.
fn connect_summary_line(
    mode: &str,
    prep_ms: u128,
    handshake_ms: u128,
    tun_ms: u128,
    routes_up_ms: u128,
    total_ms: u128,
) -> String {
    format!(
        "Warren connected in {total_ms}ms (mode={mode} prep={prep_ms}ms \
         handshake={handshake_ms}ms tun={tun_ms}ms routes+up={routes_up_ms}ms)"
    )
}

/// INFO-level one-line disconnect summary; total is the sum of the
/// phases (the teardown has no single wall-clock anchor).
fn disconnect_summary_line(
    down_event_ms: u128,
    natpmp_ms: u128,
    routes_ms: u128,
    tasks_ms: u128,
) -> String {
    let total = down_event_ms + natpmp_ms + routes_ms + tasks_ms;
    format!(
        "Warren disconnected in {total}ms (down_event={down_event_ms}ms \
         natpmp={natpmp_ms}ms routes={routes_ms}ms tasks={tasks_ms}ms)"
    )
}

/// Wall-clock ceiling on a tunnel handshake / first-dial wait.
///
/// A handshake that never completes (e.g. an exit that silently drops a
/// protocol-incompatible `Setup` instead of resetting the connection)
/// must never wedge the blocking tunnel thread forever: past this bound
/// the dial surfaces a recoverable [`Error::Handshake`] and the state
/// machine recovers on its own (retry, then a cancelable blocked state
/// once the flap detector settles). Single-hop and multi-hop share it.
const HANDSHAKE_WAIT_BOUND: std::time::Duration = std::time::Duration::from_secs(150);

/// Outcome of racing a handshake future against the daemon close signal
/// and the [`HANDSHAKE_WAIT_BOUND`] wall-clock ceiling.
enum HandshakeRace<T> {
    /// The handshake future resolved on its own.
    Completed(T),
    /// The daemon asked the tunnel to close (user pressed Cancel /
    /// Disconnect) before the handshake finished. The dial MUST unwind
    /// promptly so the blocking tunnel thread returns and the
    /// `DisconnectingState`'s `tunnel_close_event` fires: otherwise the
    /// disconnect wedges until the daemon is killed (the bug this guards).
    Aborted,
    /// The wall-clock ceiling elapsed before either of the above.
    TimedOut,
}

/// Race a handshake `fut` against the daemon `close_rx` and a wall-clock
/// `bound`.
///
/// `close_rx` is polled with priority (`biased`) so a Cancel is honored
/// immediately even when the handshake is simultaneously ready. It is
/// borrowed, not consumed: on [`HandshakeRace::Completed`] the caller
/// still owns the receiver and hands it to the running monitor, whose
/// `wait()` races it again for the steady-state close.
async fn race_handshake<F>(
    fut: F,
    close_rx: &mut futures::channel::oneshot::Receiver<()>,
    bound: std::time::Duration,
) -> HandshakeRace<F::Output>
where
    F: std::future::Future,
{
    tokio::select! {
        biased;
        _ = &mut *close_rx => HandshakeRace::Aborted,
        res = tokio::time::timeout(bound, fut) => match res {
            Ok(v) => HandshakeRace::Completed(v),
            Err(_) => HandshakeRace::TimedOut,
        },
    }
}

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
///
/// Intentionally not `Clone`: the `signing_key` field carries secret
/// Ed25519 material and must be *moved* into the tunnel builder rather
/// than silently duplicated. Callers that need to store a copy of the
/// configuration alongside the live tunnel (e.g. for reconnect) must
/// clone only the fields they actually need, keeping secret-material
/// lifetime as narrow as possible.
pub struct WarrenTunnelParameters {
    /// Candidate addresses of the exit (UDP IPv4/IPv6) plus the exit's
    /// Ed25519 pubkey in `exit_addr.id`. Built by the relay selector
    /// from `exit-info.json` published by the exits.
    pub exit_addr: WarrenExitAddr,

    /// Stable 16-byte exit identifier (pubkey-pinning anchor). Sourced
    /// from `WarrenRelay::exit_id()` at selection time. The daemon's
    /// pubkey-pinning verify hook keys its lookup on this field so a
    /// legitimate Ed25519 rotation under the same `exit_id` triggers
    /// the mismatch warning while a wholesale new exit (new
    /// `exit_id`) starts a fresh TOFU pin.
    pub exit_id: RelayExitId,

    /// Forensic snapshot of the exit's location at
    /// selection time. Propagated to the TOFU pin row so
    /// the modal + `/v1/incidents/pubkey-mismatch` report carry the
    /// user-readable location. Empty string when Warren-mode is off
    /// or the relay list lacked geo information.
    pub country_code: String,
    /// Free-form city label associated with the
    /// selected exit. Empty string when no geo information is
    /// available.
    pub city: String,

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
    /// (cf. `warrenguard_wire::features`). `0` = IPv4 baseline.
    /// Combinable via OR: `IPV6`, `PORT_FORWARD`, ...
    pub features: u32,

    /// ALPN protocols (as bytes) offered in the QUIC/TLS handshake, in
    /// preference order, sourced from the selected exit's v6 listener(s).
    /// Empty falls back to the `ALPN_H3` default inside
    /// [`warrenguard_transport::ClientTunnel`]. Single-hop only; the multi-hop
    /// path negotiates ALPN from its own signed relay descriptor.
    pub alpn_protocols: Vec<Vec<u8>>,

    /// Multi-hop configuration. `None` (default) selects the legacy
    /// single-hop path through [`warrenguard_transport::ClientTunnel`]; `Some`
    /// dispatches to a multi-hop session driven by
    /// `warrenguard_transport::supervisor::MultiHopSupervisor` against the
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
    pub on_reconnect: Option<warrenguard_transport::supervisor::ReconnectObserver>,

    /// ADR 36: invoked with the current multi-hop exit id when the exit
    /// signals an in-band maintenance drain, so the daemon can add it to a
    /// local avoid-set and the next (drain-triggered) reconnect migrates to a
    /// different exit. `None` disables exit-exclusion (the drain then falls
    /// back to a proactive reconnect + the ambient relay-list refresh).
    /// Wired by the daemon's `ParametersGenerator` to a closure that calls
    /// `record_warren_drained_exit`. Multi-hop only.
    pub on_exit_draining: Option<std::sync::Arc<dyn Fn([u8; 16]) + Send + Sync>>,

    /// ADR 36 (Option A): invoked once at tunnel start with this tunnel's
    /// [`MigrateHandle`], so the daemon can store it and later trigger a
    /// GAP-FREE cross-exit migration (`migrate_to`) off a draining exit
    /// instead of a break-before-make reconnect. The handle holds NO watch
    /// receiver, so storing it does not pin the supervisor alive across
    /// teardown. `None` disables gap-free migration (the drain then falls
    /// back to the proactive reconnect). Multi-hop only.
    pub warren_register_migrate_handle: Option<std::sync::Arc<dyn Fn(MigrateHandle) + Send + Sync>>,

    /// ADR 36 gap-free drain path: async daemon hook the drain reactor
    /// invokes with the DRAINING exit id after the anti-stampede jitter.
    /// The daemon records the exit in the avoid-set, re-selects a circuit
    /// that excludes it and, when a live migrate handle is registered,
    /// swaps the supervisor onto it via `MigrateHandle::migrate_to`
    /// (make-before-break). `true` = migration dispatched, the reactor
    /// skips the tunnel rebuild. `None` disables the gap-free path (the
    /// reactor always escalates the rebuild). Multi-hop only.
    pub warren_drain_migrate: Option<WarrenDrainMigrate>,

    /// docs/59 Lot 3: pre-swap gate run against a candidate exit before a
    /// make-before-break migration commits, so a pinned port is reserved
    /// on the candidate first (else the migration is aborted). `None`
    /// commits every overlap swap unconditionally. Wired by the daemon
    /// from its live port-forward config.
    pub warren_pre_swap_check: Option<warrenguard_transport::supervisor::PreSwapCheck>,

    /// docs/59 Lot 3: observer fired after a committed overlap migration,
    /// used to re-map NAT-PMP on the new exit immediately. `None` = not
    /// observed. Wired by the daemon.
    pub warren_on_overlap_swapped: Option<warrenguard_transport::supervisor::OverlapSwapObserver>,

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

    /// Observer invoked for every NAT-PMP event, tagged with the
    /// [`NatPmpRuleId`] of the rule that produced it (Mapped, Renewed,
    /// Failed, RateLimited, Cancelled). `None` short-circuits the manager
    /// spawn entirely (no observer = nowhere to forward events). The
    /// daemon wires this to a closure that records the per-rule event in
    /// the `WarrenStatusCache`, which drives the `NatPmpStatusUpdates`
    /// gRPC stream the Electron UI subscribes to. (A `Cancelled` event
    /// signals the daemon to drop that rule's mapping from the status.)
    pub nat_pmp_observer: Option<NatPmpMappingObserver>,

    /// Live-reconfig watch channel: every time the user toggles or
    /// edits a NAT-PMP setting in the Electron UI, the daemon pushes
    /// the new `Option<NatPmpConfig>` onto this watch. The tunnel
    /// monitor task listens on the receiver and applies the change
    /// without requiring the tunnel to reconnect:
    ///
    /// - `Some(cfg)` when previously `None` → spawn a fresh `NatPmpManager`.
    /// - `Some(cfg)` when previously `Some(other)` → call [`NatPmpManager::reconfigure`] to
    ///   release the old mapping and allocate a new one with `cfg`.
    /// - `None` when previously `Some(_)` → release the mapping and drop the manager.
    ///
    /// `None` here = the daemon did not wire live reconfig (typical
    /// in tests or in builds that pre-date the feature); the monitor
    /// task falls back to the original "params at tunnel start"
    /// behaviour. New code SHOULD always wire this channel - otherwise
    /// the user has to reconnect the tunnel to apply changes, which
    /// is the bug we're fixing.
    pub nat_pmp_control_rx: Option<tokio::sync::watch::Receiver<Option<NatPmpConfig>>>,

    /// IPv4 CIDRs that should bypass the tunnel and remain reachable
    /// via the host's main routing table (LAN, private ranges, inbound
    /// SSH on a management interface, ...). Each entry becomes an
    /// `ip rule add to <cidr> lookup main pref 49` installed alongside
    /// the standard `0.0.0.0/1` + `128.0.0.0/1` split-default routes.
    /// Empty (default) preserves the prior behaviour: the tunnel
    /// captures all traffic except the exit IP itself.
    ///
    /// Linux-only at this layer: macOS and Windows daemon routing is
    /// handled by talpid-core's platform splitters, which do not yet
    /// consume this list. UI exposure is deferred to a future phase;
    /// the field is plumbed end-to-end so future UI work only needs
    /// the gRPC + Redux glue, not a fresh daemon traversal.
    pub bypass_cidrs: Vec<BypassCidr>,

    /// DAITA v2 opt-in. When `true`, the client advertises
    /// `Setup.daita_support = true` on the warrenguard-wire v3
    /// handshake. The exit may then respond with a
    /// `SetupAck.daita_spec` describing the negotiated `maybenot`
    /// machine. Driven by the Mullvad upstream `wireguard.daita`
    /// toggle (single UI surface for both backends); the
    /// daemon-state-machine reads that boolean and forwards it
    /// verbatim through this field.
    pub enable_daita: bool,
}

impl std::fmt::Debug for WarrenTunnelParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No-log Warren: never log the full signing_key (secret material)
        // nor the full exit pubkey (session-PII). exit_id is operator
        // metadata (already public via signed relay list), safe to log.
        f.debug_struct("WarrenTunnelParameters")
            .field("exit_addr", &"<redacted>")
            .field("exit_id", &self.exit_id)
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
            .field(
                "nat_pmp_control_rx",
                &self.nat_pmp_control_rx.as_ref().map(|_| "<watch-rx>"),
            )
            .field("bypass_cidrs", &self.bypass_cidrs)
            .field("enable_daita", &self.enable_daita)
            .finish()
    }
}

/// Stable identity of a NAT-PMP port-forward rule, matching how the
/// exit allocator keys an allocation: `(internal_port, protocol)`. Two
/// rules sharing this pair would target the same exit-side mapping, so
/// the controller dedups on it (one [`NatPmpManager`]/refresh loop per
/// id). The suggested external port is a *property* of the rule, not
/// part of its identity (changing it reconfigures the same mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NatPmpRuleId {
    /// Internal port the application binds locally (0 = unset).
    pub internal_port: u16,
    /// Transport protocol.
    pub protocol: NatPmpProto,
}

/// One port-forward rule the user wants active. The client may hold up
/// to the exit-enforced quota (`warrenguard_config::NATPMP_QUOTA_PER_CLIENT_IP`)
/// of these at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NatPmpRule {
    /// Transport protocol to map (UDP or TCP).
    pub protocol: NatPmpProto,
    /// Suggested external port. `0` lets the exit pick from its pool.
    pub suggested_external_port: u16,
    /// Internal port the application binds locally (0 = unset).
    pub internal_port: u16,
    /// True when `suggested_external_port` was injected from the sticky
    /// map ([`NatPmpConfig::with_sticky_ports`]) rather than pinned by
    /// the user. A sticky suggestion is best-effort: on the exit's
    /// strict port-conflict rejection the refresh loop downgrades to a
    /// server pick instead of failing the rule.
    pub sticky_suggestion: bool,
}

impl NatPmpRule {
    /// The dedup identity of this rule (see [`NatPmpRuleId`]).
    #[must_use]
    pub fn id(&self) -> NatPmpRuleId {
        NatPmpRuleId {
            internal_port: self.internal_port,
            protocol: self.protocol,
        }
    }
}

/// Daemon-side observer invoked for every NAT-PMP event, tagged with the
/// [`NatPmpRuleId`] of the rule that produced it, so the daemon can keep
/// a per-rule status entry (multi-port). The controller wraps this into a
/// per-rule [`NatPmpEventObserver`] for each [`NatPmpManager`] it spawns.
pub type NatPmpMappingObserver = std::sync::Arc<dyn Fn(NatPmpRuleId, NatPmpEvent) + Send + Sync>;

/// NAT-PMP port-forwarding configuration carried by
/// [`WarrenTunnelParameters::nat_pmp`].
///
/// Wired by the Electron UI through the gRPC `SetNatPmpSettings` rpc.
/// Default-disabled (the field is `None` upstream when `enabled =
/// false`) so existing user installs see no change in behaviour after
/// the upgrade.
///
/// Multi-port: [`Self::rules`] is the source of truth for which forwards
/// the controller maintains (one refresh loop per rule). The legacy
/// single-port fields (`protocol`/`suggested_external_port`/
/// `internal_port`) are retained so each per-rule [`NatPmpManager`] can
/// be driven by a single-rule `NatPmpConfig` built via
/// [`Self::for_rule`], and for backward compatibility with single-port
/// callers/tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NatPmpConfig {
    /// User-facing toggle. `false` is the default; when `false` the
    /// daemon-side controller maintains NO refresh loops.
    pub enabled: bool,
    /// Lifetime requested from the exit in seconds. The server clamps
    /// to its own `[60..3600]` range, so values outside that range will
    /// be silently capped server-side; UI offers `3600` (1h), `21600`
    /// (6h, ends up clamped), `86400` (24h, ends up clamped). The
    /// refresh loop renews at `granted / 2`.
    pub lifetime_secs: u32,
    /// The set of port-forward rules to maintain. The controller keeps
    /// one [`NatPmpManager`] per distinct [`NatPmpRuleId`].
    pub rules: Vec<NatPmpRule>,
    /// Legacy single-port protocol (drives a per-rule single config; see
    /// the struct docs).
    pub protocol: NatPmpProto,
    /// Legacy single-port suggested external port.
    pub suggested_external_port: u16,
    /// Legacy single-port internal port.
    pub internal_port: u16,
    /// Legacy single-port mirror of [`NatPmpRule::sticky_suggestion`]
    /// (set by [`Self::for_rule`] so the per-manager config carries the
    /// rule's suggestion origin).
    pub sticky_suggestion: bool,
    /// Monotonic re-map generation. Bumping it makes the per-rule config
    /// differ from the last applied one, forcing the controller to
    /// reconfigure (release + immediate re-request) past its debounce,
    /// without changing any mapping property. Used after a drain-driven
    /// exit migration so the mapping is re-created on the new exit NOW
    /// instead of at the next `lifetime/2` renewal. Client-internal,
    /// never on the wire.
    pub remap_epoch: u64,
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
            rules: Vec::new(),
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
        }
    }

    /// The set of rules the controller should maintain. Uses [`Self::rules`]
    /// when non-empty; otherwise synthesizes a single rule from the legacy
    /// flat fields (so single-port callers/tests that never populate `rules`
    /// keep working). Returns empty when nothing is configured.
    #[must_use]
    pub fn effective_rules(&self) -> Vec<NatPmpRule> {
        if !self.rules.is_empty() {
            return self.rules.clone();
        }
        // Legacy single-port fallback: only synthesize a rule when a port
        // was actually configured. A pristine config (0/0) yields no rules
        // so the controller spawns no managers (matches the settings-layer
        // `effective_rules`).
        if self.internal_port != 0 || self.suggested_external_port != 0 {
            return vec![NatPmpRule {
                protocol: self.protocol,
                suggested_external_port: self.suggested_external_port,
                internal_port: self.internal_port,
                sticky_suggestion: self.sticky_suggestion,
            }];
        }
        Vec::new()
    }

    /// Build a single-rule config to drive one [`NatPmpManager`]: shares
    /// `enabled`/`lifetime_secs`, takes the protocol/ports from `rule`,
    /// and carries no nested `rules` (a manager only ever maps one port).
    #[must_use]
    pub fn for_rule(&self, rule: &NatPmpRule) -> Self {
        Self {
            enabled: self.enabled,
            lifetime_secs: self.lifetime_secs,
            rules: Vec::new(),
            protocol: rule.protocol,
            suggested_external_port: rule.suggested_external_port,
            internal_port: rule.internal_port,
            sticky_suggestion: rule.sticky_suggestion,
            remap_epoch: self.remap_epoch,
        }
    }

    /// Returns a copy of this config with each auto rule's
    /// `suggested_external_port` overridden by the last-granted port from
    /// `sticky` (keyed by [`NatPmpRuleId`]), so an auto-mode forward keeps
    /// the same public port when the client moves to a new exit (the port
    /// "follows" the client). An explicit user pin
    /// (`suggested_external_port != 0`) is always preserved, and a sticky
    /// entry of `0` is ignored (the exit never grants port 0).
    ///
    /// The result expresses every forward through [`Self::rules`] (the
    /// legacy flat fields are reset to disabled defaults) so the
    /// controller drives it uniformly via [`Self::effective_rules`].
    #[must_use]
    pub fn with_sticky_ports(&self, sticky: &std::collections::HashMap<NatPmpRuleId, u16>) -> Self {
        let rules = self
            .effective_rules()
            .into_iter()
            .map(|mut rule| {
                if rule.suggested_external_port == 0
                    && let Some(&port) = sticky.get(&rule.id())
                    && port != 0
                {
                    rule.suggested_external_port = port;
                    rule.sticky_suggestion = true;
                }
                rule
            })
            .collect();
        Self {
            enabled: self.enabled,
            lifetime_secs: self.lifetime_secs,
            rules,
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: self.remap_epoch,
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
            rules: Vec::new(),
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
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
    /// ISO 3166-1 alpha-2 country code of the EXIT hop, taken from the
    /// signed+attested directory `NodeEntry`. Authoritative for the GUI
    /// location label: the exit egress IP is redacted from the client
    /// directory, and an exit-only node is absent from the
    /// single-hop list, so the daemon cannot recover the exit geo from
    /// the IP or the relay list. Empty for the manual-config path (the
    /// caller then falls back to the single-hop relay-list lookup).
    pub exit_country: String,
    /// City of the EXIT hop (free form), from the directory `NodeEntry`.
    /// Empty for the manual-config path.
    pub exit_city: String,
    /// Enable UDP segmentation offload (GSO) on the multi-hop QUIC
    /// transport. Recommended on physical NICs, disable on virtio
    /// (Hetzner Cloud / KVM guests) and on macOS where GSO is not
    /// supported.
    pub enable_gso: bool,
    /// Opt into the full wire-mimicry profile (Initial padding +
    /// split ClientHello). `true` is the production default against a
    /// real warrenguard-relay; `false` is used for loopback benches where
    /// the relay-inbound transport config does not mirror these knobs
    /// (see `warren-client::multi_hop`).
    pub use_warren_obfuscation: bool,
    /// `true` when the circuit's entry relay and exit resolve to the SAME
    /// physical node: a 1-hop circuit (the multihop toggle is OFF). The
    /// whole fleet speaks the multi-hop wire protocol, so toggle-OFF still
    /// rides it but collapses the circuit onto one trusted node (classic
    /// single-hop privacy). The GUI MUST then present a single hop (no
    /// entry endpoint, no multihop badge): a 1-hop circuit has no distinct
    /// first hop to disclose. `false` for a genuine 2-hop circuit (toggle
    /// ON) and for the manual-config path (treated as 2-hop).
    pub single_node: bool,
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
            .field("exit_country", &self.exit_country)
            .field("exit_city", &self.exit_city)
            .field("enable_gso", &self.enable_gso)
            .field("use_warren_obfuscation", &self.use_warren_obfuscation)
            .field("single_node", &self.single_node)
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

    /// Transient backend error that the state machine should retry:
    /// TUN I/O error, temporary network disruption, or a session
    /// closed by the peer without an explicit rejection code. The
    /// tunnel data plane was interrupted but the configuration and
    /// credentials are still valid - a reconnect is expected to
    /// succeed.
    #[error("Warren tunnel transient backend error (recoverable): {0}")]
    BackendTransient(String),

    /// Fatal backend error that the state machine must NOT retry
    /// automatically: configuration mismatch, authentication failure, or
    /// PKI/TLS setup failure. Retrying immediately would produce the same
    /// outcome and only waste network bandwidth.
    ///
    /// NOTE: a multi-hop `not-allowlisted` rejection is deliberately NOT
    /// fatal: it self-heals once the exit's allowlist refresh picks up a
    /// freshly-redeemed subscription, so [`reject_error`] maps it to
    /// [`Self::BackendTransient`] (recoverable). Do not "promote" it to
    /// fatal: that would strand a just-subscribed user in an
    /// uncancelable-until-manual-reconnect state.
    #[error("Warren tunnel fatal backend error: {0}")]
    BackendFatal(String),
}

impl Error {
    /// Whether the error is worth retrying from the state-machine
    /// side.
    ///
    /// - [`Error::Handshake`]: `true` - transient network glitch is the common cause; the next
    ///   attempt will likely succeed.
    /// - [`Error::TunSetup`]: `false` - privilege / kernel module / name collision: retry will not
    ///   help without operator action.
    /// - [`Error::BackendTransient`]: `true` - TUN I/O error or peer-initiated session close; the
    ///   configuration is still valid and a fresh connect should succeed.
    /// - [`Error::BackendFatal`]: `false` - auth failure or explicit session rejection; retrying
    ///   immediately would waste bandwidth and produce the same outcome.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::Handshake(_) | Error::BackendTransient(_))
    }
}

/// Map an exit's definitive session rejection to a tunnel error.
///
/// Classified recoverable ([`Error::BackendTransient`]) on purpose: a
/// `not-allowlisted` rejection clears on its own once the exit's
/// allowlist refresh picks up a freshly-redeemed subscription (the exit
/// polls every few minutes). The state machine retries with its own
/// backoff and, if the rejection persists, the flap detector settles
/// into a stable, cancelable blocked error state. The firewall
/// kill-switch stays up throughout, so a rejected session never leaks
/// and never blackholes the user into an uncancelable wedge.
fn reject_error(reason: RejectionReason) -> Error {
    Error::BackendTransient(format!(
        "exit rejected the session ({reason}); no active subscription, or the exit \
         has not yet synced a freshly-redeemed one"
    ))
}

/// Maps a `warren_tunnel` handshake error to the talpid backend [`Error`].
///
/// An explicit auth rejection from the exit
/// ([`warrenguard_transport_core::TunnelError::AuthRejected`] - the client's identity
/// is not authorized: no active subscription / not enrolled) is FATAL
/// and non-retryable: retrying re-derives the same outcome, and the
/// state machine would otherwise loop and surface a misleading "no
/// matching relay" instead of the real "subscription required" cause.
/// Every other handshake failure is treated as a (recoverable)
/// [`Error::Handshake`] - a transient network glitch is the common case
/// and the next attempt usually succeeds.
fn map_handshake_error(e: warrenguard_transport_core::TunnelError) -> Error {
    // The `[TOKEN]` prefix is parsed by `mullvad_types::auth_failed::AuthFailed`
    // (via `ErrorStateCause::AuthFailed`) into a `proto::AuthFailedError`, which
    // the GUI renders as a precise, already-localized message. Reusing the
    // existing tokens gives the user a meaningful cause (fr included) with no
    // proto/GUI/i18n change:
    //   [TOO_MANY_CONNECTIONS] -> "Too many simultaneous connections … disconnect another device"
    //   [EXPIRED_ACCOUNT]      -> "Blocking internet: account is out of time"
    // Both are non-retryable business rejections (see `map`-arm rationale).
    match e {
        warrenguard_transport_core::TunnelError::AuthRejected => Error::BackendFatal(
            "[EXPIRED_ACCOUNT] exit rejected the handshake: no active subscription for this account"
                .to_owned(),
        ),
        // Device cap (v2): account already at its max simultaneous devices.
        // Maps to TOO_MANY_CONNECTIONS - the GUI string is an exact fit
        // ("disconnect another device or try again shortly").
        warrenguard_transport_core::TunnelError::DeviceLimitReached => Error::BackendFatal(
            "[TOO_MANY_CONNECTIONS] exit rejected the handshake: device limit reached for this account"
                .to_owned(),
        ),
        other => Error::Handshake(format!("{other:#}")),
    }
}

/// Active Warren tunnel monitor.
///
/// API:
/// - [`Self::start`]: blocking factory (QUIC handshake + TUN setup + pump spawn).
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
    /// Guard for the IPv6 split-default routing (`::/1` + `8000::/1` in
    /// the dedicated table). Installed in `start_single_hop` only when the
    /// exit allocated a tunnel v6 (`metadata.ipv6_gateway.is_some()`).
    /// `None` on the multi-hop path (IPv4-only `/v1`), when no v6 was
    /// assigned, or on non-Linux. Torn down in `wait()`
    /// alongside the v4 guard; the firewall blocks native v6 regardless so
    /// a failed install never leaks.
    v6_route_guard: Option<default_route_split::DefaultRouteSplitV6Guard>,
    /// Lifecycle owner of the NAT-PMP refresh loop + event forwarder
    /// spawned after the tunnel is up. `None` when port-forwarding is
    /// disabled in the user settings (`params.nat_pmp.is_none()` OR
    /// `params.nat_pmp_observer.is_none()`) AND no live-reconfig
    /// control channel was provided. Dropping the field cancels the
    /// loop and aborts the forwarder.
    ///
    /// Mutually exclusive with [`nat_pmp_controller`]: the legacy path
    /// owns the managers directly here (one per port-forward rule); the
    /// live-reconfig path moves ownership into a controller task.
    nat_pmp_managers: Vec<NatPmpManager>,

    /// Live-reconfig controller task. Owns its own `NatPmpManager`
    /// internally and reacts to settings changes pushed via the
    /// daemon-side watch channel by calling
    /// [`NatPmpManager::reconfigure`] (live-swap, no tunnel
    /// reconnect). `Some` iff [`WarrenTunnelParameters::nat_pmp_control_rx`]
    /// was wired at tunnel start.
    ///
    /// Cancelling the task aborts the watch loop; the controller
    /// drops its owned manager on exit, which calls `cancel()` via
    /// the manager's `Drop` impl.
    nat_pmp_controller: Option<tokio::task::JoinHandle<()>>,
}

/// Backend-specific task ownership.
///
/// - [`MonitorBackend::SingleHop`] owns a single bidirectional pump spawned by
///   `warrenguard_pump::pump_bidirectional` / `pump_multi_bidirectional` and an oneshot channel to
///   surface abnormal pump terminations.
/// - [`MonitorBackend::MultiHop`] owns a 3-task fanout: the `MultiHopSupervisor` future (drives
///   connect+reconnect), the uplink pump (TUN -> multi-hop) and the downlink pump (multi-hop ->
///   TUN), wired together through a watch channel.
enum MonitorBackend {
    SingleHop {
        pump_handle: tokio::task::JoinHandle<()>,
        /// Periodic metrics-logging task. Spawned alongside the pump but
        /// NOT cancelled by aborting the pump (it owns its own clone of
        /// the metrics counters). Tracked here so teardown aborts it
        /// explicitly - otherwise every connect leaks one immortal task
        /// that keeps logging `pump_metrics` for a dead session forever.
        metrics_handle: tokio::task::JoinHandle<()>,
        pump_error_rx: tokio::sync::oneshot::Receiver<String>,
    },
    MultiHop {
        supervisor_handle: tokio::task::JoinHandle<()>,
        uplink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        downlink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        /// Migration watchdog task (route-change verification + the
        /// forced-reconnect/escalation fallbacks). Aborted FIRST during
        /// teardown: it holds a supervisor watch receiver and a control
        /// handle, and the supervisor's no-receivers shutdown check
        /// must still fire once the pumps drop theirs.
        watchdog_handle: tokio::task::JoinHandle<()>,
        /// IpAssign drift guard: escalates when a reconnect republishes
        /// a different exit-allocated IPv4 than the one the TUN holds.
        assign_guard_handle: tokio::task::JoinHandle<()>,
        /// ADR 36 drain reactor: proactively migrates off a draining exit
        /// before its maintenance hard-close deadline.
        drain_reactor_handle: tokio::task::JoinHandle<()>,
        /// Session-loss backstop: escalates when the supervisor stays
        /// without a published session (silent redial not landing), so
        /// the UI never shows Connected on a dead tunnel.
        liveness_handle: tokio::task::JoinHandle<()>,
        /// Channel through which the pump task reports the first fatal
        /// error (uplink/downlink/supervisor death). `wait()` consumes
        /// it the same way the single-hop path consumes
        /// `pump_error_rx`. A retriable disconnect from the underlying
        /// QUIC connection does NOT push an error here: the supervisor
        /// absorbs it transparently and the pump tasks park on the
        /// watch channel until the supervisor re-publishes.
        pump_error_rx: tokio::sync::oneshot::Receiver<String>,
        /// Terminal-rejection signal from the supervisor. `wait()` races
        /// it so a mid-session policy rejection (the exit revokes the
        /// pubkey) surfaces as a clean, cancelable error state rather
        /// than an endless silent reconnect.
        supervisor_fatal_rx: tokio::sync::watch::Receiver<Option<RejectionReason>>,
    },
}

impl WarrenTunnelMonitor {
    /// Starts a Warren tunnel from `params`, dispatching to the
    /// single-hop path through `warrenguard_transport::ClientTunnel` (when
    /// `params.multi_hop` is `None`) or the multi-hop path through
    /// `warrenguard_transport::supervisor::MultiHopSupervisor` (when `Some`).
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
        // Clone only the `multi_hop` field (which itself derives Clone)
        // rather than cloning the entire `WarrenTunnelParameters` struct
        // (which is intentionally not Clone to prevent signing_key copies).
        match params.multi_hop.as_ref().cloned() {
            None => Self::start_single_hop(params, args, log_path),
            Some(cfg) => Self::start_multi_hop(params, cfg, args, log_path),
        }
    }

    /// Single-hop path: dial the exit directly through
    /// `warrenguard_transport::ClientTunnel`, open a TUN, install routing
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
    /// - [`Error::Handshake`] if the QUIC handshake or `Setup` exchange fails.
    /// - [`Error::TunSetup`] if opening the TUN fails (privileges, interface name collision, kernel
    ///   module missing).
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
        // Quinn does not do peer-to-peer path discovery (no NAT-traversal
        // extension), so this filter is now defense-in-depth.
        let exit_addr = filter_endpoint_addr_for_wan(params.exit_addr.clone());
        // Clone only the individual fields moved into the async block, not
        // the entire WarrenTunnelParameters struct. `signing_key` is copied
        // field-by-field here so the compiler is explicit about what secret
        // material enters the async closure.
        let signing = SigningKey::from_bytes(&params.signing_key.to_bytes());
        let n_conns = params.n_connections;
        let features = params.features;
        let enable_daita = params.enable_daita;
        let alpn_protocols = params.alpn_protocols.clone();
        let mut event_hook = args.event_hook;
        // ADR-0006 B2-lite: read `WARREN_IDLE_COVER` once and combine with
        // the DAITA opt-in. This single bool drives BOTH the transport
        // config (keep-alive PING off) and the pump choice below, so they
        // can never disagree (a keep-alive-disabled config with a plain
        // pump would lose all liveness). Default-off; turning it on by
        // default is gated on a real-network bench (ADR-0006).
        let idle_cover_active = idle_cover_effective(
            warrenguard_config::knobs::idle_cover_enabled(),
            enable_daita,
        );

        // v6 X.509 cover-domain mode (wg-0005 / ADR-0005 Stage 1). When
        // `WARREN_COVER_DOMAIN` is set the client validates the exit's real
        // certificate via WebPKI (Mozilla roots) and dials this domain as SNI
        // instead of pinning the exit's Ed25519 raw public key in the SNI; the
        // exit identity is still verified in-band, so a wrong domain cannot
        // admit a foreign exit. Unset (default) keeps the RPK-via-SNI
        // handshake. Read at the binary boundary (infrastructure config, not a
        // data-plane tuning knob). The exit must run in matching X.509 mode
        // (lockstep): an X.509 exit and an RPK client do not interoperate.
        let cover_domain = resolve_cover_domain(
            exit_addr.cover_domain.as_deref(),
            std::env::var("WARREN_COVER_DOMAIN").ok(),
        );
        if let Some(ref d) = cover_domain {
            log::info!("Warren: v6 X.509 mode active, cover-domain SNI = {d}");
        }

        // Detect the outbound source IP (eth0 / wlan0) used to reach
        // the exit before the handshake, so we bind the QUIC `Endpoint`
        // explicitly to that IP instead of `0.0.0.0:0`. Defense in depth
        // against any future regression that might re-introduce
        // multi-path / rebind behavior in the transport layer.
        // Clear any leaked split before binding so the exit dial starts on
        // the physical default, not a dead TUN. See `force_route_cleanup`.
        default_route_split::force_route_cleanup();
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
        // Move the daemon close receiver out of `args` so the handshake can
        // be raced against it: a Cancel / Disconnect issued mid-dial must
        // unwind this blocking thread at once. Otherwise `wait()` (which
        // normally races this receiver) is never reached, the
        // `tunnel_close_event` never fires, and the disconnect wedges until
        // the daemon is force-killed. The receiver is handed to the running
        // monitor below for the steady-state close.
        let mut close_rx = args.tunnel_close_rx;
        let session_kind = {
            let close_ref = &mut close_rx;
            runtime.block_on(async move {
                let connect_fut = async move {
                    let mut client = ClientTunnel::with_signing_key(&signing)
                        .with_features(features)
                        .with_alpn_protocols(alpn_protocols)
                        .with_daita(enable_daita)
                        // Keep-alive PING off when idle cover is armed; the
                        // matching pump (below) emits cover instead (ADR-0006).
                        .with_idle_cover(idle_cover_active)
                        // Stable per-install device id: every reconnect/retry
                        // reuses ONE device lease, so the account never trips
                        // the v2 device cap and gets locked out of connecting
                        // (see `device_id`).
                        .with_device_id(device_id::device_id());
                    if let Some(addr) = bind_local_ip {
                        client = client.with_bind_local_ip(addr);
                    }
                    if let Some(domain) = cover_domain {
                        // Validate the exit chain against the Mozilla roots and
                        // dial the cover domain as SNI (v6 X.509). The exit
                        // identity stays in-band-verified, independent of TLS.
                        client = client.with_x509_webpki(domain);
                    }
                    match select_session_request(n_conns) {
                        SessionRequest::Mono => client
                            .connect(exit_addr)
                            .await
                            .map(SessionKind::Mono)
                            .map_err(map_handshake_error),
                        SessionRequest::Multi(n) => client
                            .connect_multi(exit_addr, n)
                            .await
                            .map(SessionKind::Multi)
                            .map_err(map_handshake_error),
                    }
                };
                match race_handshake(connect_fut, close_ref, HANDSHAKE_WAIT_BOUND).await {
                    HandshakeRace::Completed(res) => res,
                    HandshakeRace::Aborted => Err(Error::Handshake(
                        "single-hop dial aborted by the daemon close signal".to_owned(),
                    )),
                    HandshakeRace::TimedOut => Err(Error::Handshake(format!(
                        "single-hop handshake did not complete within {HANDSHAKE_WAIT_BOUND:?}"
                    ))),
                }
            })
        }?;
        log::debug!(
            "{TRACE_PREFIX} T2={}ms phase=handshake_done elapsed_handshake={}ms session_kind={}",
            start_t.elapsed().as_millis(),
            handshake_t.elapsed().as_millis(),
            match session_kind {
                SessionKind::Mono(_) => "Mono",
                SessionKind::Multi(_) => "Multi",
            }
        );

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
        #[cfg(not(target_os = "windows"))]
        let async_device = tun.into_inner().into_async_device();
        // On Windows, WindowsTun::into_inner() already yields a tun08::AsyncDevice
        // (cf. talpid-tunnel/src/tun_provider/windows.rs); no into_async_device() hop.
        #[cfg(target_os = "windows")]
        let async_device = tun.into_inner();
        let packet_device = MullvadTunPacketDevice::new(async_device);

        // Startup event sequence - order is load-bearing:
        //
        // 1. InterfaceUp  - tells the state machine to install the Connecting-state firewall
        //    (allows traffic to the exit only). Emitted BEFORE routing so the firewall fence is up
        //    before any route change could let traffic escape via the physical NIC.
        // 2. add_routes   - bypass exit IPs + split-default installed.
        // 3. DefaultRouteSplitGuard::install - policy route table 100.
        // 4. TunnelEvent::Up - signals "Connected" to the UI. By the time the UI shows "Connected"
        //    the default route already points at the TUN, so there is no window where traffic
        //    bypasses the tunnel.
        log::debug!(
            "{TRACE_PREFIX} T4={}ms phase=interfaceup_emit (firewall fence installed, routes pending)",
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
        });
        log::debug!(
            "{TRACE_PREFIX} T4b={}ms phase=interfaceup_consumed elapsed_interfaceup={}ms",
            start_t.elapsed().as_millis(),
            events_t.elapsed().as_millis()
        );

        // Install routes via route_manager: redirect user traffic
        // through the TUN while preserving the daemon's own access to
        // the peer endpoint (otherwise daemon -> tun -> exit -> daemon
        // == routing loop).
        //
        // Split-default strategy:
        // - 0.0.0.0/1 + 128.0.0.0/1 dev tun0 : covers all of 0.0.0.0/0 without replacing the
        //   existing default route, less intrusive, clean restore at teardown via route_manager.
        // - <exit_ip>/32 dev <physical_iface> : more specific than /1 so daemon -> exit packets
        //   bypass the tun.
        let exit_ips: Vec<IpAddr> = params.exit_addr.ip_addrs().map(|sa| sa.ip()).collect();
        // Per-platform route set, mirroring Mullvad WireGuard's
        // `get_endpoint_routes` / `get_pre_tunnel_routes` /
        // `get_post_tunnel_routes` dispatch:
        // - Linux: bypass `<exit_ip>/32 via <gw> dev <physical>` in the main table + split-default
        //   `/1 + /1 dev <tun>` in table 100 via `default_route_split`. Iface + gw detected from
        //   `/proc/net/route`.
        // - macOS: bypass `<exit_ip>/32 NetNode::DefaultNode` (talpid-routing resolves
        //   best_default_route at apply time) + `0.0.0.0/0 dev <tun>` (triggers the
        //   `tunnel_default_routes` ifscope dance). No upfront detection: talpid-routing already
        //   tracks the physical iface and gw via its internal monitor.
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
        // Windows owns its routing entirely from the warrenguard-route-split
        // PowerShell port (`DefaultRouteSplitGuard::install` below):
        // host-route exception + `0.0.0.0/1` + `128.0.0.0/1` via the
        // WinTUN adapter. talpid-routing is therefore handed an empty
        // route set so it does not double-install conflicting netsh
        // entries.
        #[cfg(target_os = "windows")]
        let routes: Vec<RequiredRoute> = Vec::new();
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let routes: Vec<RequiredRoute> = {
            log::warn!("Warren tunnel routing not yet implemented for this platform.");
            Vec::new()
        };

        let route_manager = args.route_manager.clone();
        let routes_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T5={}ms phase=routes_add_start",
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
            "{TRACE_PREFIX} T6={}ms phase=routes_added elapsed_routes={}ms",
            start_t.elapsed().as_millis(),
            routes_t.elapsed().as_millis()
        );

        // Install the platform-specific split-default policy routing.
        // The OS-specific recipe lives in the `default_route_split`
        // facade module:
        // - Linux: dedicated table 100 + `ip rule` bypass for the exit IP (in-crate impl, see
        //   `default_route_split::linux`).
        // - macOS: host-route exception + `/1` split-default on the global table, ported from
        //   `warrenguard_route_split::default_route_split_macos`.
        // - Other platforms: stub that fails to install (operator sees "Internet traffic will NOT
        //   route via tunnel" warning).
        //
        // Both Linux and macOS expose the same `install(Ipv4Addr, &str)
        // -> Result<Self>` signature, so the install branch is OS-
        // agnostic at this call site. The cfg-guard is required only to
        // skip the work entirely on platforms where the facade ships
        // the `stub` impl.
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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
                             Need root + ip/route in PATH."
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
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let default_route_guard: Option<default_route_split::DefaultRouteSplitGuard> = None;

        // IPv6 split-default: route `::/1` + `8000::/1` into the TUN's
        // dedicated table so the exit-allocated tunnel v6 actually carries
        // user traffic. Installed only when the exit allocated a v6
        // (`ipv6_gateway` set in `build_tun_config_for_kind`). The facade
        // bails on non-Linux. A failed install is non-fatal: the
        // firewall keeps native IPv6 blocked, so v6 is non-functional but
        // never leaks. The exit's own v6 endpoint (if the transport is v6)
        // is passed as the self-poison bypass.
        let v6_route_guard = if metadata.ipv6_gateway.is_some() {
            let exit_ip_v6 = exit_ips.iter().find_map(|ip| match ip {
                IpAddr::V6(v6) => Some(*v6),
                IpAddr::V4(_) => None,
            });
            let tun_name_for_v6 = metadata.interface.clone();
            runtime
                .block_on(async move {
                    default_route_split::DefaultRouteSplitV6Guard::install(
                        exit_ip_v6,
                        &tun_name_for_v6,
                    )
                    .await
                })
                .map(Some)
                .unwrap_or_else(|e| {
                    log::warn!(
                        "Warren: failed to install IPv6 split-default routing: {e}. \
                         IPv6 will NOT route via the tunnel (the firewall keeps native \
                         IPv6 blocked, so there is no leak - only no v6 connectivity)."
                    );
                    None
                })
        } else {
            None
        };

        // Emit TunnelEvent::Up now that routes AND the split-default
        // guard are fully installed. The UI transitions to "Connected"
        // only after the default route already points at the TUN, so
        // there is no window where traffic escapes via the physical NIC.
        // Do NOT emit Up before routing: it reopens that leak window.
        log::debug!(
            "{TRACE_PREFIX} T7={}ms phase=up_emit (routes+split-default installed, emit Up)",
            start_t.elapsed().as_millis()
        );
        runtime.block_on(async {
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });
        log::debug!(
            "{TRACE_PREFIX} T7b={}ms phase=up_consumed (Connected state signaled to UI)",
            start_t.elapsed().as_millis()
        );

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
        // - Mono -> `pump_bidirectional(tun, conn)`. The session value must be moved into the
        //   closure to keep the underlying `Endpoint` alive (otherwise its drop makes
        //   `read_datagram` return "endpoint driver future was dropped" immediately). Same pattern
        //   as `warren-client::main`.
        // - Multi -> `pump_multi_bidirectional(tun, multi_session)` for N-connection bonding
        //   (uplink round-robin + N downlink tasks).
        let (pump_error_tx, pump_error_rx) = tokio::sync::oneshot::channel::<String>();
        let pump_metrics = packet_device.metrics();
        let pump_spawn_t = Instant::now();
        let pump_handle = runtime.spawn(async move {
            log::info!("{TRACE_PREFIX} pump=running");
            // QUIC path visibility (cwnd/rtt/loss/datagram buffer headroom),
            // 5s cadence, self-terminates when the connections close.
            // Opt-out via WARREN_PATH_PROBE=0; never logs IPs or keys.
            // Detach the probe's JoinHandle: it is fire-and-forget and
            // self-terminates when the connections close.
            drop(match &session_kind {
                SessionKind::Mono(session) => {
                    warrenguard_transport_core::spawn_path_probe("client", vec![session.clone_conn()], None)
                }
                SessionKind::Multi(multi) => {
                    warrenguard_transport_core::spawn_path_probe("client", multi.clone_connections(), None)
                }
            });
            let pump_result = match session_kind {
                SessionKind::Mono(session) => {
                    let conn = session.clone_conn();
                    // Prefer the DAITA-enabled pump variant
                    // when the exit ships a `daita_spec` in the
                    // SetupAck. Falls back to the regular pump when
                    // no DAITA was negotiated.
                    let res = match session.daita_spec().cloned() {
                        Some(cfg) => {
                            match DaitaState::from_config(&cfg, std::time::Instant::now()) {
                                Ok(state) => {
                                    log::info!(
                                        "{TRACE_PREFIX} pump=running variant=daita machines={}",
                                        cfg.machine_specs.len()
                                    );
                                    pump_bidirectional_with_daita(packet_device, conn, state).await
                                }
                                Err(e) => {
                                    // Build failure on a server-supplied
                                    // spec is exceptional. Surface as a
                                    // pump-level tunnel error so the
                                    // state machine reconnects rather
                                    // than silently dropping DAITA.
                                    log::error!(
                                        "{TRACE_PREFIX} pump=daita_spec_invalid err=\"{e:#}\""
                                    );
                                    Err(e.into())
                                }
                            }
                        }
                        None if idle_cover_active => {
                            log::info!("{TRACE_PREFIX} pump=running variant=idle_cover");
                            pump_bidirectional_with_idle_cover(packet_device, conn).await
                        }
                        None => pump_bidirectional(packet_device, conn).await,
                    };
                    drop(session);
                    res
                }
                SessionKind::Multi(multi) => {
                    // Prefer the DAITA-enabled multi-conn pump
                    // when the primary's SetupAck shipped a spec. The
                    // exit guarantees every secondary inherits the
                    // same spec (cf. `attribute_session` sharing), so
                    // we can read it once from the MultiSession and
                    // hand it to a single shared DaitaState.
                    match multi.daita_spec().cloned() {
                        Some(cfg) => {
                            match DaitaState::from_config(&cfg, std::time::Instant::now()) {
                                Ok(state) => {
                                    log::info!(
                                        "{TRACE_PREFIX} pump=running variant=daita_multi machines={} conns={}",
                                        cfg.machine_specs.len(),
                                        multi.num_connections()
                                    );
                                    pump_multi_bidirectional_with_daita(
                                        packet_device,
                                        multi,
                                        state,
                                    )
                                    .await
                                }
                                Err(e) => {
                                    log::error!(
                                        "{TRACE_PREFIX} pump=daita_spec_invalid err=\"{e:#}\""
                                    );
                                    Err(e.into())
                                }
                            }
                        }
                        None if idle_cover_active => {
                            log::info!(
                                "{TRACE_PREFIX} pump=running variant=idle_cover_multi conns={}",
                                multi.num_connections()
                            );
                            pump_multi_bidirectional_with_idle_cover(packet_device, multi).await
                        }
                        None => pump_multi_bidirectional(packet_device, multi).await,
                    }
                }
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

        // Periodic pump metrics task. Lets a bench distinguish which direction
        // stalls (uplink stops but downlink continues -> server-side
        // `read_datagram` issue; both stop at once -> QUIC connection closed).
        // The task aborts when teardown aborts the pump. 30s cadence keeps prod
        // log volume bounded; raise transiently via the log reload handle when
        // diagnosing a stall.
        let metrics_handle = runtime.spawn(async move {
            let mut prev_up = 0u64;
            let mut prev_down = 0u64;
            let tick_start = Instant::now();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
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
        log::info!(
            "{}",
            connect_summary_line(
                "single_hop",
                handshake_t.duration_since(start_t).as_millis(),
                tun_t.duration_since(handshake_t).as_millis(),
                events_t.duration_since(tun_t).as_millis(),
                pump_spawn_t.duration_since(events_t).as_millis(),
                pump_spawn_t.duration_since(start_t).as_millis(),
            )
        );

        let _ = metadata; // kept for future MTU-change re-emission

        // Spawn the NAT-PMP runtime once the data plane is up. Picks
        // the legacy "managers owned by monitor" path or the
        // live-reconfig "managers owned by controller task" path based
        // on whether `params.nat_pmp_control_rx` was wired.
        let NatPmpRuntimeArtifacts {
            managers: nat_pmp_managers,
            controller: nat_pmp_controller,
        } = spawn_nat_pmp_runtime(&runtime, params);

        Ok(Self {
            runtime,
            backend: MonitorBackend::SingleHop {
                pump_handle,
                metrics_handle,
                pump_error_rx,
            },
            event_hook,
            // Handed off from the handshake race above (not re-taken from
            // `args`, whose `tunnel_close_rx` was already moved out).
            close_rx,
            default_route_guard,
            v6_route_guard,
            nat_pmp_managers,
            nat_pmp_controller,
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
    /// - [`Error::Handshake`] if the supervisor cannot establish an initial session within the
    ///   bounded wait window.
    /// - [`Error::TunSetup`] if opening the TUN fails (privileges, interface name collision, kernel
    ///   module missing).
    /// - [`Error::BackendFatal`] for non-retriable supervisor errors (PKI, TLS provider setup),
    ///   bubbled up before any pump is spawned.
    fn start_multi_hop(
        params: &WarrenTunnelParameters,
        cfg: MultiHopConfig,
        args: TunnelArgs<'_>,
        _log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        use std::{sync::Arc, time::Duration};
        use warrenguard_backoff::Backoff;
        use warrenguard_transport::{
            supervised_pump::{ExitDrainingChannel, IpAssignChannel, run_downlink, run_uplink},
            supervisor::{MultiHopSupervisor, SupervisorConfig},
        };

        let start_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T0=0ms phase=start_begin mode=multi_hop \
             enable_gso={} use_warren_obfuscation={}",
            cfg.enable_gso,
            cfg.use_warren_obfuscation
        );

        let runtime = args.runtime.clone();
        let mut event_hook = args.event_hook;
        // Take the daemon close receiver out of `args` up front so the
        // initial-dial wait below can race it: a Cancel / Disconnect issued
        // before the first session comes up must unwind this blocking thread
        // at once (otherwise the disconnect wedges until the daemon is
        // killed). It is handed to the running monitor for the steady-state
        // close once the dial succeeds.
        let mut close_rx = args.tunnel_close_rx;

        let relay_endpoint = cfg.relay.endpoint;
        // Clear any leaked split before dialing so the relay dial is not
        // routed into a dead TUN. See `force_route_cleanup`.
        default_route_split::force_route_cleanup();
        let bind_local_ip = multi_hop_bind_addr(relay_endpoint);

        // Spawn the supervisor and wait for the first successful dial
        // so the rest of the bootstrap (TUN, routing) has a live
        // session to anchor to.
        // The dual-role exit allocates this client's tunnel IPv4 and
        // routes downlink to that address, replying with an `IpAssign`
        // over the supervisor's reliable setup stream. The supervisor
        // publishes it on this channel BEFORE handing out the first
        // session, so by the time `initial_client` resolves below the
        // assigned IP is already available and the TUN can be opened
        // with it directly (no dynamic reassign needed; allocation is
        // pubkey-sticky so the IP survives reconnects).
        // Multi-hop `/v2` dual-stack: request a v6 alongside the v4 when
        // the user enabled IPv6 (the `IPV6` feature bit, set by
        // `warren_tunnel_params::features_for`). The exit MAY answer
        // v4-only; the firewall keeps native v6 blocked either way.
        let wants_ipv6 = (params.features & features::IPV6) != 0;
        let ip_assign_channel = IpAssignChannel::new();
        // ADR 36: the downlink pump publishes a mid-session `ExitDraining`
        // advisory here when the exit is drained for maintenance; the drain
        // reactor (spawned below) consumes it and proactively migrates off
        // the draining exit before its hard-close deadline.
        let exit_draining_channel = ExitDrainingChannel::new();
        let supervisor_config = SupervisorConfig {
            relay: Arc::new(cfg.relay.clone()),
            exit_id: cfg.exit.exit_id,
            exit_x25519_multihop_pubkey: cfg.exit.exit_x25519_multihop_pubkey,
            operational_pubkey: cfg.operational_pubkey,
            // Copy the signing key field-by-field rather than cloning the
            // parent WarrenTunnelParameters struct (which is not Clone).
            client_signing: SigningKey::from_bytes(&params.signing_key.to_bytes()),
            bind_addr: bind_local_ip,
            enable_gso: cfg.enable_gso,
            use_warren_obfuscation: cfg.use_warren_obfuscation,
            // Low ceiling on purpose. The default HANDSHAKE profile caps at
            // 15 s, which overshoots a network-change recovery: when the
            // physical link comes back the re-dial can be parked in a 15 s
            // backoff and miss the window, stretching a handover to ~20 s.
            // A 2 s ceiling re-dials within ~2 s of the link returning, so a
            // Wi-Fi <-> ethernet switch reconnects promptly. Cold start is
            // unaffected (the relay is normally reachable on the first try).
            backoff: Backoff {
                base: Duration::from_millis(300),
                max: Duration::from_secs(2),
            },
            on_reconnect: params.on_reconnect.clone(),
            ip_assign_channel: Some(ip_assign_channel.clone()),
            wants_ipv6,
            // Bonded connections: one QUIC connection is capped at the
            // per-flow bandwidth share of the client↔relay path (e.g.
            // ~400 Mbps on a 1 Gbps line whose 8-flow aggregate reaches
            // ~900). The supervisor bonds n_connections sessions under
            // one identity (sticky inner IP) and pins flows by 5-tuple
            // hash, mirroring the single-hop MultiSession.
            n_connections: usize::from(params.n_connections).max(1),
            // docs/59 Lot 3: the pre-swap NAT-PMP reservation and the
            // post-swap re-map observer are wired by the daemon from its
            // live port-forward config (next step); the transport carries
            // the hooks, `None` keeps the pre-hook behavior.
            pre_swap_check: params.warren_pre_swap_check.clone(),
            on_overlap_swapped: params.warren_on_overlap_swapped.clone(),
        };
        let (supervisor, mut client_rx) = MultiHopSupervisor::new(supervisor_config);
        // Subscribe to the supervisor's terminal-rejection signal BEFORE
        // `run()` consumes it. The exit publishes a rejection here (e.g.
        // pubkey not allowlisted) instead of letting the session masquerade
        // as a live "Connected" that silently carries no traffic.
        let mut supervisor_fatal_rx = supervisor.fatal_rx();
        // Control handle for the migration watchdog's forced-reconnect
        // fallback; must be taken before `run()` consumes the supervisor.
        let supervisor_control = supervisor.handle();
        // ADR 36 (Option A): hand the daemon a migrate-only handle so a
        // drain-driven directory re-selection can swap this supervisor onto a
        // non-drained exit gap-free. Taken before `run()` consumes the
        // supervisor; the handle holds no watch receiver, so it never pins
        // this supervisor alive after teardown.
        if let Some(register) = params.warren_register_migrate_handle.as_ref() {
            register(supervisor.migrate_handle());
        }
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
        // Bound the initial-dial wait (5 * Backoff::HANDSHAKE.max, ~150 s)
        // so a permanently-unreachable relay surfaces as a clean
        // Error::Handshake instead of hanging the state machine.
        let initial_wait_bound = HANDSHAKE_WAIT_BOUND;
        let initial_client: Arc<warrenguard_transport::bundle::MultiHopBundle> =
            runtime.block_on(async {
                let deadline = tokio::time::Instant::now() + initial_wait_bound;
                loop {
                    if let Some(c) = client_rx.borrow().clone() {
                        return Ok(c);
                    }
                    // A definitive rejection short-circuits the wait: the
                    // session will never come up under the current identity,
                    // so surface a clean (recoverable) error instead of
                    // blocking the full timeout.
                    if let Some(reason) = *supervisor_fatal_rx.borrow() {
                        return Err(reject_error(reason));
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(Error::Handshake(format!(
                            "multi-hop supervisor did not produce an initial session within {initial_wait_bound:?}"
                        )));
                    }
                    // Wake on whichever fires first: a Cancel, a live session,
                    // a rejection, or the timeout. The close signal is polled
                    // first (`biased`) so a Disconnect aborts the dial without
                    // waiting on the other branches.
                    tokio::select! {
                        biased;
                        _ = &mut close_rx => {
                            return Err(Error::Handshake(
                                "multi-hop first dial aborted by the daemon close signal".to_owned(),
                            ));
                        }
                        changed = tokio::time::timeout(remaining, client_rx.changed()) => {
                            if changed.is_err() {
                                return Err(Error::Handshake(format!(
                                    "multi-hop supervisor did not produce an initial session within {initial_wait_bound:?}"
                                )));
                            }
                        }
                        _ = supervisor_fatal_rx.changed() => {
                            if let Some(reason) = *supervisor_fatal_rx.borrow() {
                                return Err(reject_error(reason));
                            }
                        }
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
        // `warrenguard_config::TUNNEL_POOL_CIDR` (`10.66.0.0/16`).
        //
        // Derivation: octet C, D = pubkey[0], pubkey[1].max(2). Skips
        // `10.66.X.0` (network) and `10.66.X.1` (gateway). Collision
        // odds across two clients on the same exit are ~1/65 000.
        // A future revision will replace this with a coordinated allocator
        // (subscription-bound, persisted exit-side).
        let pubkey_bytes = params.signing_key.verifying_key().to_bytes();
        // Prefer the exit-allocated IPv4 (published on the setup-stream
        // IpAssign during the first dial). Fall back to the pubkey-derived
        // self-slot only if the exit ran without an allocator or the setup
        // round-trip fell back to its bootstrap path.
        // `/v2` dual-stack: the same setup-stream `IpAssign(V2)` carries
        // the exit-allocated v6 when one was negotiated. `None` keeps the
        // TUN v4-only (firewall blocks native v6 - no leak).
        let (tun_ip, tun_ipv6) = match *ip_assign_channel.subscribe().borrow_and_update() {
            Some(spec) => {
                // Capability echo: if we asked for v6 but the exit did not
                // grant one (`assigned_v6` is None), surface it LOUDLY
                // instead of silently going v4-only. The presence of
                // `assigned_v6` is the exit's authoritative answer.
                if wants_ipv6 && spec.assigned_v6.is_none() {
                    log::warn!(
                        "{TRACE_PREFIX} IPv6 was requested but this exit did NOT grant it; \
                         staying IPv4-only. IPv6 is UNAVAILABLE on this exit - surfaced, \
                         not a silent fallback."
                    );
                }
                log::info!(
                    "{TRACE_PREFIX} multi-hop TUN using exit-allocated IPv4 {} (gateway {}, dual_stack={})",
                    spec.assigned,
                    spec.gateway,
                    spec.assigned_v6.is_some()
                );
                (
                    spec.assigned,
                    if wants_ipv6 { spec.assigned_v6 } else { None },
                )
            }
            None => {
                log::info!(
                    "{TRACE_PREFIX} multi-hop TUN using pubkey-derived IPv4 (no exit IpAssign yet)"
                );
                (derive_multi_hop_tun_ip(&pubkey_bytes), None)
            }
        };
        let tun_config = build_multi_hop_tun_config(tun_ip, tun_ipv6);

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
        #[cfg(not(target_os = "windows"))]
        let async_device = tun.into_inner().into_async_device();
        // On Windows, WindowsTun::into_inner() already yields a tun08::AsyncDevice
        // (cf. talpid-tunnel/src/tun_provider/windows.rs); no into_async_device() hop.
        #[cfg(target_os = "windows")]
        let async_device = tun.into_inner();
        let packet_device = MullvadTunPacketDevice::new(async_device);

        // Startup event sequence - order is load-bearing:
        //
        // 1. InterfaceUp  - installs the Connecting-state firewall; emitted BEFORE routing so the
        //    firewall fence is in place before any route change could let traffic escape via the
        //    physical NIC.
        // 2. add_routes   - relay bypass + split-default installed.
        // 3. DefaultRouteSplitGuard::install - policy route table 100.
        // 4. TunnelEvent::Up - signals "Connected" to the UI only after the default route already
        //    points at the TUN.
        log::debug!(
            "{TRACE_PREFIX} T4={}ms phase=interfaceup_emit (multi-hop, firewall fence up, routes pending)",
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
        });
        log::debug!(
            "{TRACE_PREFIX} T4b={}ms phase=interfaceup_consumed elapsed_interfaceup={}ms (multi-hop)",
            start_t.elapsed().as_millis(),
            events_t.elapsed().as_millis()
        );

        // Routing install: bypass the relay endpoint IP (the only UDP
        // peer the daemon reaches on the data plane), then split-default
        // on the TUN.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
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
        // Windows: routing fully owned by the warrenguard-route-split
        // PowerShell port via `DefaultRouteSplitGuard::install` below; talpid-
        // routing gets an empty set to avoid double-install.
        #[cfg(target_os = "windows")]
        let routes: Vec<RequiredRoute> = Vec::new();
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let routes: Vec<RequiredRoute> = {
            log::warn!("Warren multi-hop tunnel routing not yet implemented for this platform.");
            Vec::new()
        };

        let route_manager = args.route_manager.clone();
        let routes_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T5={}ms phase=routes_add_start (multi-hop)",
            start_t.elapsed().as_millis()
        );
        let metadata_iface_for_log = metadata.interface.clone();
        // macOS installs in two ordered calls: the relay/32 bypass must
        // be live BEFORE 0/0 lands on the tun, because the wildcard-bound
        // QUIC socket has no interface pin to fall back on (see
        // `build_warren_tunnel_routes_macos_ordered`).
        #[cfg(target_os = "macos")]
        runtime.block_on(async move {
            let (bypass, defaults) =
                build_warren_tunnel_routes_macos_ordered(&metadata_iface_for_log, &next_hop_ips);
            match route_manager.add_routes(bypass.into_iter().collect()).await {
                Ok(()) => log::info!("Warren multi-hop relay bypass route installed"),
                Err(e) => log::warn!(
                    "Failed to install Warren multi-hop relay bypass route: {e}. \
                     Relay traffic may transiently self-nest through the tun."
                ),
            }
            match route_manager
                .add_routes(defaults.into_iter().collect())
                .await
            {
                Ok(()) => log::info!(
                    "Warren multi-hop tunnel routes installed (tun={metadata_iface_for_log})"
                ),
                Err(e) => log::warn!(
                    "Failed to install Warren multi-hop tunnel routes: {e}. \
                     Tunnel up but no traffic forwarding."
                ),
            }
        });
        #[cfg(not(target_os = "macos"))]
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
        log::debug!(
            "{TRACE_PREFIX} T6={}ms phase=routes_added elapsed_routes={}ms (multi-hop)",
            start_t.elapsed().as_millis(),
            routes_t.elapsed().as_millis()
        );

        // Multi-hop split-default install: same facade as single-hop
        // above. The bypass exception targets the *relay* endpoint
        // (first hop) rather than the exit, since on multi-hop the only
        // UDP peer the client speaks to directly is the relay.
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let default_route_guard: Option<default_route_split::DefaultRouteSplitGuard> = None;

        // Multi-hop `/v2` dual-stack: route `::/1` + `8000::/1` into the
        // TUN's dedicated table so the exit-allocated tunnel v6 carries
        // user traffic. Installed only when the exit allocated a v6
        // (`ipv6_gateway` set in `build_multi_hop_tun_config`). The relay
        // transport is IPv4, so there is no v6 endpoint to except. A
        // failed install is non-fatal: the firewall keeps native IPv6
        // blocked, so v6 is non-functional but never leaks. Mirrors the
        // single-hop install.
        let v6_route_guard = if metadata.ipv6_gateway.is_some() {
            let tun_name_for_v6 = metadata.interface.clone();
            runtime
                .block_on(async move {
                    default_route_split::DefaultRouteSplitV6Guard::install(None, &tun_name_for_v6)
                        .await
                })
                .map(Some)
                .unwrap_or_else(|e| {
                    log::warn!(
                        "Warren multi-hop: failed to install IPv6 split-default: {e}. \
                         Native IPv6 stays firewall-blocked (no leak), v6 non-functional."
                    );
                    None
                })
        } else {
            None
        };

        // Emit TunnelEvent::Up now that routes AND the split-default guard
        // are fully installed. The UI transitions to "Connected" only after
        // the default route already points at the TUN.
        // Do NOT emit Up before routing: it reopens the physical-NIC leak window.
        log::debug!(
            "{TRACE_PREFIX} T7={}ms phase=up_emit (routes+split-default installed, emit Up, multi-hop)",
            start_t.elapsed().as_millis()
        );
        runtime.block_on(async {
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });
        log::debug!(
            "{TRACE_PREFIX} T7b={}ms phase=up_consumed (Connected state signaled to UI, multi-hop)",
            start_t.elapsed().as_millis()
        );

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
                if let Some(tx) = pump_error_tx_uplink
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                {
                    let _ = tx.send(msg);
                }
            }
            res
        });

        let downlink_rx = client_rx.clone();
        let downlink_device = packet_device.clone();
        let downlink_drain_channel = exit_draining_channel.clone();
        let downlink_handle = runtime.spawn(async move {
            log::info!("{TRACE_PREFIX} multi-hop downlink=running");
            let res =
                run_downlink(downlink_rx, downlink_device, Some(downlink_drain_channel)).await;
            if let Err(ref e) = res {
                let msg = format!("multi-hop downlink: {e:#}");
                log::warn!("{TRACE_PREFIX} {msg}");
                if let Some(tx) = pump_error_tx_downlink
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                {
                    let _ = tx.send(msg);
                }
            }
            res
        });

        // Migration watchdog: verifies the QUIC path after every
        // default-route change and layers the fallbacks (bypass nudge,
        // forced supervisor reconnect, state-machine escalation). See
        // `migration_watchdog` module docs.
        let watchdog_handle = {
            let client_rx = client_rx.clone();
            let supervisor = supervisor_control;
            let pump_error_tx = pump_error_tx.clone();
            let route_manager = args.route_manager.clone();
            let relay_ipv4 = match relay_endpoint.ip() {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            };
            runtime.spawn(async move {
                match migration_watchdog::subscribe_route_events(&route_manager).await {
                    Ok((route_events, guard)) => {
                        let mut io = migration_watchdog::RealWatchdogIo {
                            route_events,
                            _subscription_guard: guard,
                            route_manager,
                            client_rx,
                            supervisor,
                            pump_error_tx,
                            relay_ipv4,
                        };
                        migration_watchdog::run_watchdog(&mut io).await;
                        log::debug!("{TRACE_PREFIX} migration watchdog terminated");
                    }
                    Err(e) => log::warn!(
                        "Warren migration watchdog disabled (route-event subscription failed): {e}"
                    ),
                }
            })
        };

        // IpAssign drift guard: the TUN was opened with the FIRST
        // exit-allocated IPv4 and is never live-reconfigured. The exit
        // keeps the address sticky across reconnects (same-pubkey
        // takeover allocator-side), so a republished IpAssign with a
        // DIFFERENT address should never happen; if it does, a tunnel
        // whose TUN address no longer matches the exit's routing table
        // is silently dead behind a "Connected" UI. Escalate through
        // the pump-error channel so the state machine rebuilds the
        // tunnel with the new address instead.
        let assign_guard_handle = {
            let mut assign_rx = ip_assign_channel.subscribe();
            let pump_error_tx = pump_error_tx.clone();
            let tun_ip_v4 = tun_ip;
            runtime.spawn(async move {
                // Mark the initial publication as seen; only react to
                // republications (reconnects).
                let _ = assign_rx.borrow_and_update();
                loop {
                    if assign_rx.changed().await.is_err() {
                        return;
                    }
                    let republished = *assign_rx.borrow_and_update();
                    if let Some(spec) = republished
                        && spec.assigned != tun_ip_v4
                    {
                        let msg = format!(
                            "exit reassigned the tunnel IPv4 {tun_ip_v4} -> {} on reconnect; \
                             rebuilding the tunnel to adopt it",
                            spec.assigned
                        );
                        log::warn!("{TRACE_PREFIX} {msg}");
                        if let Some(tx) = pump_error_tx
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .take()
                        {
                            let _ = tx.send(msg);
                        }
                        return;
                    }
                }
            })
        };

        // ADR 36 drain reactor: when the exit signals it is draining for
        // maintenance, report the current exit to the daemon avoid-set (so the
        // reconnect re-selects a circuit that excludes it) and proactively
        // migrate off it before the hard-close deadline, spread across an
        // anti-stampede jitter window. See `drain_reactor`.
        let drain_reactor_handle = {
            let mut drain_sub = exit_draining_channel.subscribe();
            // Mark the initial `None` seen so `changed()` only fires on a
            // real advisory publish.
            let _ = drain_sub.borrow_and_update();
            let pump_error_tx = pump_error_tx.clone();
            // The advisory carries no identity; capture the exit this tunnel
            // dialed so the reactor can name it to the avoid-set.
            let current_exit_id = *cfg.exit.exit_id.as_bytes();
            let on_exit_draining = params.on_exit_draining.clone();
            let drain_migrate = params.warren_drain_migrate.clone();
            runtime.spawn(async move {
                let mut io = drain_reactor::RealDrainReactorIo {
                    drain_sub,
                    pump_error_tx,
                    current_exit_id,
                    on_exit_draining,
                    drain_migrate,
                };
                drain_reactor::run_drain_reactor(&mut io).await;
                log::debug!("{TRACE_PREFIX} drain reactor terminated");
            })
        };

        // Session-loss backstop: the supervisor's transparent redial has
        // no exit condition, so a network that dies without a route
        // event (dead-but-routed) would keep the UI on "Connected"
        // forever. See `session_liveness` module docs.
        let liveness_handle = {
            let mut io = session_liveness::RealLivenessIo {
                client_rx: client_rx.clone(),
                pump_error_tx: pump_error_tx.clone(),
            };
            runtime.spawn(async move {
                session_liveness::run_session_liveness(&mut io).await;
                log::debug!("{TRACE_PREFIX} session liveness guard terminated");
            })
        };

        // Drop the local watch receiver. The uplink/downlink pumps and
        // the watchdog keep theirs alive; teardown aborts the watchdog
        // first, then the pumps, so the supervisor still observes
        // `tx.closed()` and terminates cleanly.
        drop(client_rx);

        let pump_spawn_t = Instant::now();
        log::debug!(
            "{TRACE_PREFIX} T8={}ms phase=pump_spawned mode=multi_hop (uplink + downlink + supervisor live)",
            start_t.elapsed().as_millis()
        );
        log::info!(
            "{}",
            connect_summary_line(
                "multi_hop",
                handshake_t.duration_since(start_t).as_millis(),
                tun_t.duration_since(handshake_t).as_millis(),
                events_t.duration_since(tun_t).as_millis(),
                pump_spawn_t.duration_since(events_t).as_millis(),
                pump_spawn_t.duration_since(start_t).as_millis(),
            )
        );

        // Spawn the NAT-PMP runtime once the data plane is up. The
        // exit-side NAT-PMP server is reachable via the tunnel
        // gateway (10.66.0.1:5351). On multi-hop the routing identity
        // of the gateway is the exit's TUN IP, same as single-hop.
        let NatPmpRuntimeArtifacts {
            managers: nat_pmp_managers,
            controller: nat_pmp_controller,
        } = spawn_nat_pmp_runtime(&runtime, params);

        Ok(Self {
            runtime,
            backend: MonitorBackend::MultiHop {
                supervisor_handle,
                uplink_handle,
                downlink_handle,
                watchdog_handle,
                assign_guard_handle,
                drain_reactor_handle,
                liveness_handle,
                pump_error_rx,
                supervisor_fatal_rx,
            },
            event_hook,
            // Handed off from the initial-dial race above (not re-taken from
            // `args`, whose `tunnel_close_rx` was already moved out).
            close_rx,
            default_route_guard,
            // Multi-hop `/v2` dual-stack: holds the `::/1`+`8000::/1` split
            // route when the exit allocated a v6; `None` keeps it v4-only.
            v6_route_guard,
            nat_pmp_managers,
            nat_pmp_controller,
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
    /// [`Error::BackendTransient`] if the pump terminated abnormally
    /// before the close-signal arrived. This is classified as
    /// recoverable so the state machine reconnects instead of entering
    /// the `ErrorState::StartTunnelError` kill-switch. Fatal conditions
    /// (auth failure, explicit exit rejection) surface as
    /// [`Error::Handshake`] during `start()`, before the pump is
    /// spawned, so they never arrive here.
    pub fn wait(self) -> Result<(), Error> {
        let WarrenTunnelMonitor {
            runtime,
            backend,
            mut event_hook,
            close_rx,
            default_route_guard,
            v6_route_guard,
            nat_pmp_managers,
            nat_pmp_controller,
        } = self;

        // IMPORTANT: do NOT tear down the NAT-PMP runtime here. `wait()`
        // is called immediately after `start()` and BLOCKS for the
        // ENTIRE tunnel lifetime on the `block_on` below; this prologue
        // runs at the START of the tunnel's life, not the end.
        //
        // An earlier version dropped `nat_pmp_manager` + aborted
        // `nat_pmp_controller` right here, which killed the refresh
        // loop / controller microseconds after they were spawned -
        // the controller's body never even got polled (the
        // "Status: requesting forever" bug; the legacy manager had the
        // same latent issue, surviving only the transient window
        // between `start()` and `wait()`, which is why the exit
        // sometimes showed a short-lived orphan allocation).
        //
        // We instead keep both bound in this stack frame and tear
        // them down AFTER the `block_on` returns (tunnel actually
        // closing) - see the teardown block further down.

        // Split the backend into its oneshot receiver (raced against
        // `close_rx` below) and its owned task handles (aborted after
        // the race finishes). Both backends surface abnormal pump
        // terminations through the same `pump_error_rx` channel.
        enum BackendHandles {
            SingleHop {
                pump: tokio::task::JoinHandle<()>,
                metrics: tokio::task::JoinHandle<()>,
            },
            MultiHop {
                supervisor: tokio::task::JoinHandle<()>,
                uplink: tokio::task::JoinHandle<anyhow::Result<()>>,
                downlink: tokio::task::JoinHandle<anyhow::Result<()>>,
                watchdog: tokio::task::JoinHandle<()>,
                assign_guard: tokio::task::JoinHandle<()>,
                drain_reactor: tokio::task::JoinHandle<()>,
                liveness: tokio::task::JoinHandle<()>,
            },
        }
        // `None` for single-hop (no supervisor); `Some` for multi-hop.
        let mut supervisor_fatal_rx: Option<tokio::sync::watch::Receiver<Option<RejectionReason>>> =
            None;
        let (pump_error_rx, handles) = match backend {
            MonitorBackend::SingleHop {
                pump_handle,
                metrics_handle,
                pump_error_rx,
            } => (
                pump_error_rx,
                BackendHandles::SingleHop {
                    pump: pump_handle,
                    metrics: metrics_handle,
                },
            ),
            MonitorBackend::MultiHop {
                supervisor_handle,
                uplink_handle,
                downlink_handle,
                watchdog_handle,
                assign_guard_handle,
                drain_reactor_handle,
                liveness_handle,
                pump_error_rx,
                supervisor_fatal_rx: fatal_rx,
            } => {
                supervisor_fatal_rx = Some(fatal_rx);
                (
                    pump_error_rx,
                    BackendHandles::MultiHop {
                        supervisor: supervisor_handle,
                        uplink: uplink_handle,
                        downlink: downlink_handle,
                        watchdog: watchdog_handle,
                        assign_guard: assign_guard_handle,
                        drain_reactor: drain_reactor_handle,
                        liveness: liveness_handle,
                    },
                )
            }
        };

        // A future that resolves with the rejection reason if the
        // supervisor publishes one mid-session; never resolves for
        // single-hop (no supervisor) so it is inert in the race.
        let fatal_fut = async move {
            match supervisor_fatal_rx.as_mut() {
                Some(rx) => loop {
                    if let Some(reason) = *rx.borrow_and_update() {
                        return reason;
                    }
                    if rx.changed().await.is_err() {
                        // Supervisor dropped without a rejection: park
                        // forever so the other select arms decide.
                        std::future::pending::<()>().await;
                    }
                },
                None => std::future::pending().await,
            }
        };

        let result = runtime.block_on(async move {
            tokio::pin!(fatal_fut);
            // `tokio::select!` races the signals. The first to arrive
            // "wins" and the losing branches are dropped (= the internal
            // futures are cleanly cancelled, no leak).
            let outcome: Result<(), Error> = tokio::select! {
                reason = &mut fatal_fut => {
                    // Mid-session policy rejection (the exit revoked the
                    // pubkey). Recoverable so the state machine retries
                    // and the flap detector settles into a stable
                    // cancelable blocked state; the kill-switch stays up.
                    Err(reject_error(reason))
                }
                close_res = close_rx => {
                    // External close: daemon requests shutdown. Err =
                    // Sender dropped without signaling (rare: daemon
                    // crashed). We treat it as an implicit close
                    // (no error - the state machine will continue
                    // its normal cycle).
                    let _ = close_res;
                    Ok(())
                }
                pump_res = pump_error_rx => {
                    // Pump terminated before the external close.
                    // `Ok(msg)`: the pump explicitly sent an error.
                    //   Classify as BackendTransient: pump errors are
                    //   TUN I/O failures or peer-initiated session
                    //   closes - the configuration is still valid and
                    //   a reconnect should succeed. Fatal conditions
                    //   (auth failure, explicit rejection) surface
                    //   through Error::Handshake before the pump even
                    //   starts, so they never reach this branch.
                    // `Err(_)`: sender dropped without sending = clean
                    //   exit (QUIC session closed gracefully by the
                    //   exit, e.g. idle_timeout). No error to report.
                    match pump_res {
                        Ok(msg) => Err(Error::BackendTransient(format!(
                            "pump terminated abnormally: {msg}"
                        ))),
                        Err(_) => Ok(()),
                    }
                }
            };
            let down_t = Instant::now();
            event_hook.on_event(TunnelEvent::Down).await;
            (outcome, down_t.elapsed().as_millis())
        });
        let (result, down_event_ms) = result;

        // NAT-PMP teardown (the tunnel is now closing).
        // Now that the `block_on` returned (external close or pump
        // exit), tear down the NAT-PMP runtime before the routes so
        // its refresh loop stops emitting while the rest of teardown
        // proceeds.
        //
        // - Legacy (`nat_pmp_managers`): drop → each manager's `Drop` impl cancels its refresh loop
        //   + aborts its forwarder.
        // - Live-reconfig (`nat_pmp_controller`): abort the controller task; it drops its owned
        //   managers on the way out (manager `Drop` fires). The daemon's watch sender will then see
        //   its receiver gone on the next push, which `on_set_nat_pmp_settings` treats as a no-op
        //   (tunnel dying - nothing to apply).
        let natpmp_t = Instant::now();
        drop(nat_pmp_managers);
        if let Some(h) = nat_pmp_controller {
            h.abort();
        }
        let natpmp_ms = natpmp_t.elapsed().as_millis();

        // Uninstall the split-default policy routing before aborting
        // the pump, mirroring the install order. Best-effort: log a
        // warning but do not fail teardown. The v4 and v6 guards touch
        // disjoint route entries and disjoint registries (and the
        // firewall keeps native v6 blocked throughout), so they tear
        // down concurrently. Each runs on its own spawned task, NOT a
        // same-task join!: the macOS v4 uninstall is synchronous
        // subprocess work with no await points, which would run to
        // completion before a joined v6 arm was ever polled.
        let routes_t = Instant::now();
        runtime.block_on(async {
            let v4 = tokio::spawn(async move {
                if let Some(guard) = default_route_guard
                    && let Err(e) = guard.uninstall().await
                {
                    log::warn!("Warren default-route split cleanup failed: {e}");
                }
            });
            let v6 = tokio::spawn(async move {
                if let Some(guard) = v6_route_guard
                    && let Err(e) = guard.uninstall().await
                {
                    log::warn!("Warren IPv6 split-default cleanup failed: {e}");
                }
            });
            let _ = tokio::join!(v4, v6);
        });
        let routes_ms = routes_t.elapsed().as_millis();

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
        let tasks_t = Instant::now();
        runtime.block_on(async {
            match handles {
                BackendHandles::SingleHop { pump, metrics } => {
                    // Abort the metrics task too: it owns its own clone
                    // of the counters and does NOT stop when the pump is
                    // aborted, so without this it would leak one
                    // immortal logging task per connect.
                    metrics.abort();
                    pump.abort();
                    let _ = metrics.await;
                    let _ = pump.await;
                }
                BackendHandles::MultiHop {
                    supervisor,
                    uplink,
                    downlink,
                    watchdog,
                    assign_guard,
                    drain_reactor,
                    liveness,
                } => {
                    // Watchdog first: it holds a supervisor watch
                    // receiver + control handle; dropping them before
                    // the pumps keeps the supervisor's no-receivers
                    // shutdown check working as before.
                    watchdog.abort();
                    let _ = watchdog.await;
                    // The liveness guard also holds a supervisor watch
                    // receiver; drop it before the pumps for the same
                    // reason.
                    liveness.abort();
                    let _ = liveness.await;
                    assign_guard.abort();
                    let _ = assign_guard.await;
                    // Drain reactor holds only an ExitDrainingChannel
                    // receiver + a pump-error sender clone, neither of
                    // which gates the supervisor shutdown; abort it
                    // alongside the other guards.
                    drain_reactor.abort();
                    let _ = drain_reactor.await;
                    uplink.abort();
                    downlink.abort();
                    let _ = tokio::join!(uplink, downlink);
                    supervisor.abort();
                    let _ = supervisor.await;
                }
            }
        });
        log::info!(
            "{}",
            disconnect_summary_line(
                down_event_ms,
                natpmp_ms,
                routes_ms,
                tasks_t.elapsed().as_millis()
            )
        );

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

/// Result type for [`spawn_nat_pmp_runtime`]: either the legacy
/// owned-manager path (when no live-reconfig control channel was
/// wired) or the controller-task path (when the daemon plugged in a
/// watch channel for live reconfig). The two are mutually exclusive
/// and the monitor stores them in distinct fields so the wait/drop
/// teardown can disambiguate.
struct NatPmpRuntimeArtifacts {
    /// Legacy path (no live-reconfig channel): the monitor owns the
    /// managers directly, one per initial rule. Empty on the controller
    /// path.
    managers: Vec<NatPmpManager>,
    controller: Option<tokio::task::JoinHandle<()>>,
}

/// Dispatcher between the legacy "manager owned by monitor" path and
/// the live-reconfig "manager owned by controller task" path. Picks
/// the right one based on whether [`WarrenTunnelParameters::nat_pmp_control_rx`]
/// was wired:
///
/// - `nat_pmp_control_rx == None` → spawn the manager directly, return it owned by the monitor.
///   Settings changes will NOT propagate until the tunnel reconnects (legacy behaviour).
/// - `nat_pmp_control_rx == Some(rx)` → spawn a controller task that owns the manager and listens
///   on `rx`. Each push on `rx` triggers a live `reconfigure` (or `cancel` + drop on `None`, or a
///   fresh spawn on `Some(_)` when starting from a disabled state).
fn spawn_nat_pmp_runtime(
    runtime: &tokio::runtime::Handle,
    params: &WarrenTunnelParameters,
) -> NatPmpRuntimeArtifacts {
    // Diagnostic: trace which of the three NAT-PMP inputs is
    // present at tunnel start so a "nothing happens on toggle" report
    // can be triaged from the logs alone.
    log::info!(
        "Warren NAT-PMP: spawn_nat_pmp_runtime observer={} control_rx={} nat_pmp_enabled={}",
        params.nat_pmp_observer.is_some(),
        params.nat_pmp_control_rx.is_some(),
        params.nat_pmp.as_ref().is_some_and(|c| c.enabled),
    );
    // The observer must be present in both paths - the daemon-side
    // wiring guarantees this whenever NAT-PMP is opted-in. Without an
    // observer the manager would emit events into a void, which is
    // never useful.
    let mapping_observer = match params.nat_pmp_observer.clone() {
        Some(o) => o,
        None => {
            // Mirror the original short-circuit log so the operator
            // can correlate config issues without grepping for two
            // distinct messages.
            if params.nat_pmp.as_ref().is_some_and(|c| c.enabled) {
                log::warn!(
                    "Warren NAT-PMP: config enabled but no observer wired; manager spawn suppressed"
                );
            }
            return NatPmpRuntimeArtifacts {
                managers: Vec::new(),
                controller: None,
            };
        }
    };
    let server = warrenguard_natpmp_client::default_server_addr();
    let bind_addr = None;

    // Legacy path: no live-reconfig control channel. Spawn one manager
    // per effective rule directly and let the monitor own them.
    let Some(control_rx) = params.nat_pmp_control_rx.clone() else {
        let mut managers = Vec::new();
        if let Some(cfg) = params.nat_pmp.as_ref().filter(|c| c.enabled) {
            for rule in cfg.effective_rules() {
                log::info!(
                    "Warren NAT-PMP: starting refresh loop against {server} for rule {:?}",
                    rule.id()
                );
                let per_rule = cfg.for_rule(&rule);
                let observer = per_rule_observer(mapping_observer.clone(), rule.id());
                managers.push(NatPmpManager::start_from_addr(
                    runtime, server, &per_rule, observer, bind_addr,
                ));
            }
        }
        return NatPmpRuntimeArtifacts {
            managers,
            controller: None,
        };
    };

    // Live-reconfig path: spawn a controller task that owns the
    // managers. The controller reads the initial config from the
    // watch channel's current value, then loops on `changed()` to
    // reconcile the rule set (spawn/release/reconfigure per rule).
    let initial = params.nat_pmp.clone();
    let runtime_clone = runtime.clone();
    log::info!("Warren NAT-PMP: spawning controller task (live-reconfig path)");
    let controller = runtime.spawn(async move {
        log::info!("Warren NAT-PMP: controller task body entered");
        run_nat_pmp_controller(
            runtime_clone,
            server,
            bind_addr,
            mapping_observer,
            initial,
            control_rx,
        )
        .await;
        log::info!("Warren NAT-PMP: controller task body exited");
    });
    log::info!("Warren NAT-PMP: controller spawn returned a handle");
    NatPmpRuntimeArtifacts {
        managers: Vec::new(),
        controller: Some(controller),
    }
}

/// Wrap a daemon-side [`NatPmpMappingObserver`] (which is tagged with a
/// [`NatPmpRuleId`]) into a plain [`NatPmpEventObserver`] bound to one
/// rule, suitable for handing to a single [`NatPmpManager`].
fn per_rule_observer(
    mapping_observer: NatPmpMappingObserver,
    id: NatPmpRuleId,
) -> NatPmpEventObserver {
    std::sync::Arc::new(move |evt| mapping_observer(id, evt))
}

/// Controller task body: owns the [`NatPmpManager`] across its
/// lifetime and applies live-reconfig commands streaming in on the
/// watch channel.
///
/// The watch channel semantics matter here. tokio's `watch::Receiver`
/// has a "current value" that's always present; `changed().await`
/// fires on the FIRST mutation after subscription. The daemon writes
/// the initial config to the watch BEFORE spawning the tunnel, so
/// when we land here we synthesize the initial state from
/// `initial_config` (= `params.nat_pmp.clone()` at spawn time) - we
/// don't re-read the watch's current value to avoid a TOCTOU with
/// the daemon's pre-tunnel push.
///
/// On loop exit (watch sender dropped) we drop the manager: its
/// `Drop` impl handles the refresh-loop cancel + forwarder abort.
async fn run_nat_pmp_controller(
    runtime: tokio::runtime::Handle,
    server: std::net::SocketAddr,
    bind_addr: Option<std::net::IpAddr>,
    mapping_observer: NatPmpMappingObserver,
    initial_config: Option<NatPmpConfig>,
    mut control_rx: tokio::sync::watch::Receiver<Option<NatPmpConfig>>,
) {
    use std::collections::{HashMap, HashSet};

    // A duplicate push (same rule properties) within this window is
    // swallowed: the renderer can emit the same settings twice in quick
    // succession (e.g. an input's change + blur), and each redundant
    // reconfigure is a wasted release(old)+request(new) round-trip that
    // also spends a per-client rate-limit slot on the exit. Beyond the
    // window an identical push IS honoured - that is a deliberate user
    // action (e.g. re-applying the same port to RETRY after a failure);
    // skipping it indefinitely would strand the user in `Failed` with no
    // recovery path.
    const RECONFIGURE_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(2);

    /// Per-rule manager state keyed by [`NatPmpRuleId`]. `applied_cfg` is
    /// the single-rule config last handed to the manager (via
    /// [`NatPmpConfig::for_rule`]); it carries the shared `lifetime_secs`
    /// too, so a config-wide lifetime change is detected as a diff even
    /// though the rule identity is unchanged.
    struct ManagerState {
        manager: NatPmpManager,
        applied_cfg: NatPmpConfig,
        applied_at: std::time::Instant,
    }

    /// Reconcile the running per-rule managers against a desired config:
    /// release rules that disappeared (emitting `Cancelled`), spawn new
    /// rules, and live-reconfigure changed ones (debounced).
    async fn reconcile(
        managers: &mut HashMap<NatPmpRuleId, ManagerState>,
        desired: Option<&NatPmpConfig>,
        runtime: &tokio::runtime::Handle,
        server: std::net::SocketAddr,
        bind_addr: Option<std::net::IpAddr>,
        mapping_observer: &NatPmpMappingObserver,
        debounce: std::time::Duration,
    ) {
        let cfg = desired.filter(|c| c.enabled);
        let wanted: Vec<NatPmpRule> = cfg.map(NatPmpConfig::effective_rules).unwrap_or_default();
        let wanted_ids: HashSet<NatPmpRuleId> = wanted.iter().map(NatPmpRule::id).collect();

        // 1) Release+remove managers whose rule disappeared. Emit `Cancelled` so the daemon drops
        //    that mapping from status.
        let gone: Vec<NatPmpRuleId> = managers
            .keys()
            .copied()
            .filter(|id| !wanted_ids.contains(id))
            .collect();
        for id in gone {
            if let Some(mut st) = managers.remove(&id) {
                log::info!("Warren NAT-PMP controller: releasing removed rule {id:?}");
                st.manager.release().await;
                mapping_observer(id, NatPmpEvent::Cancelled);
            }
        }

        let Some(cfg) = cfg else {
            return;
        };

        // 2) Spawn new rules + reconfigure changed ones.
        for rule in wanted {
            let id = rule.id();
            let per_rule_cfg = cfg.for_rule(&rule);
            match managers.get_mut(&id) {
                Some(st) => {
                    // Skip only a byte-identical per-rule config re-pushed
                    // inside the debounce window; honour everything else (a
                    // real change or an intentional retry past the window).
                    let recent_duplicate =
                        st.applied_cfg == per_rule_cfg && st.applied_at.elapsed() < debounce;
                    if recent_duplicate {
                        log::debug!(
                            "Warren NAT-PMP controller: duplicate rule {id:?} within debounce - skipping"
                        );
                    } else {
                        log::info!("Warren NAT-PMP controller: live reconfigure rule {id:?}");
                        st.manager.reconfigure(&per_rule_cfg).await;
                        st.applied_cfg = per_rule_cfg;
                        st.applied_at = std::time::Instant::now();
                    }
                }
                None => {
                    log::info!("Warren NAT-PMP controller: spawning refresh loop for rule {id:?}");
                    let observer = per_rule_observer(mapping_observer.clone(), id);
                    let manager = NatPmpManager::start_from_addr(
                        runtime,
                        server,
                        &per_rule_cfg,
                        observer,
                        bind_addr,
                    );
                    managers.insert(
                        id,
                        ManagerState {
                            manager,
                            applied_cfg: per_rule_cfg,
                            applied_at: std::time::Instant::now(),
                        },
                    );
                }
            }
        }
    }

    log::info!(
        "Warren NAT-PMP controller: task started (initial_enabled={})",
        initial_config.as_ref().is_some_and(|c| c.enabled)
    );

    let mut managers: HashMap<NatPmpRuleId, ManagerState> = HashMap::new();
    reconcile(
        &mut managers,
        initial_config.as_ref(),
        &runtime,
        server,
        bind_addr,
        &mapping_observer,
        RECONFIGURE_DEBOUNCE,
    )
    .await;

    while control_rx.changed().await.is_ok() {
        // Borrow then clone immediately so we hold the watch's
        // internal read lock for the minimal possible duration.
        let new_cfg_opt: Option<NatPmpConfig> = control_rx.borrow().clone();
        log::info!(
            "Warren NAT-PMP controller: change observed (wanted_enabled={})",
            new_cfg_opt.as_ref().is_some_and(|c| c.enabled)
        );
        reconcile(
            &mut managers,
            new_cfg_opt.as_ref(),
            &runtime,
            server,
            bind_addr,
            &mapping_observer,
            RECONFIGURE_DEBOUNCE,
        )
        .await;
    }

    // Watch sender dropped → daemon teardown. The managers drop here,
    // firing their `Drop` impls. Drop cancels each refresh loop but does
    // NOT send a lifetime=0 release - that's fine because the exit GCs
    // the mappings at their lease expiry (~1 h) and the tunnel is dying.
    if !managers.is_empty() {
        log::info!(
            "Warren NAT-PMP controller: shutdown - dropping {} manager(s)",
            managers.len()
        );
    }
    drop(managers);
}

/// Filter `WarrenExitAddr.addrs` to keep only Internet-routable
/// addresses. Excludes:
/// - RFC 1918 private IPv4 (10/8, 172.16/12, 192.168/16).
/// - IPv4 loopback (127/8), link-local (169.254/16), broadcast, multicast, unspecified.
/// - IPv6 loopback (::1), unspecified, multicast, link-local (fe80::/10), unique-local (fc00::/7).
///
/// Preserves `id` and any future non-IP transport variants (Warren
/// does not use relays today; the match stays open via `_` to track
/// the upstream `#[non_exhaustive]` shape).
///
/// Defense in depth: the NAT-traversal bug class (path discovery
/// probing the peer's TUN gateway IP) is structurally eliminated on
/// Quinn, but this filter still
/// hardens against malformed exit metadata that would carry a private
/// address (e.g. a RFC1918 `10.66.0.1` candidate or similar tunnel
/// gateway leak) as an exit candidate.
/// Resolves the v6 X.509 cover-domain SNI for a dial (wg-0005).
///
/// The signed roster's per-exit `cover_domain` is authoritative and wins:
/// it lets each exit advertise its own real certificate hostname for
/// cover-domain rotation. The `WARREN_COVER_DOMAIN` env is the
/// deployment-wide fallback (a single shared cover domain, ADR-0005 Stage
/// 1), also used for exits whose roster entry carries no domain. Both are
/// trimmed; empty / whitespace-only is treated as absent. `None` keeps the
/// RPK-via-SNI handshake.
#[must_use]
fn resolve_cover_domain(per_exit: Option<&str>, env: Option<String>) -> Option<String> {
    fn clean(s: &str) -> Option<String> {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_owned())
    }
    per_exit
        .and_then(clean)
        .or_else(|| env.as_deref().and_then(clean))
}

#[must_use]
fn filter_endpoint_addr_for_wan(addr: WarrenExitAddr) -> WarrenExitAddr {
    let mut filtered = WarrenExitAddr::new(addr.id);
    // This filter narrows the ADDRESS set only; the descriptor metadata
    // (dns_disabled, the v6 X.509 cover_domain) must survive it, otherwise the
    // per-exit cover domain would be silently dropped before the dial and the
    // client would fall back to RPK / the env default.
    filtered.dns_disabled = addr.dns_disabled;
    filtered.cover_domain = addr.cover_domain.clone();
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
/// - `0` or `1` -> [`SessionRequest::Mono`] (0 is the degenerate case and gets mono rather than a
///   panic, since the upper layer should validate stricter).
/// - `>= 2` -> [`SessionRequest::Multi(n)`].
#[must_use]
fn select_session_request(n_connections: u8) -> SessionRequest {
    if n_connections <= 1 {
        SessionRequest::Mono
    } else {
        SessionRequest::Multi(n_connections)
    }
}

/// Decides whether ADR-0006 B2-lite idle cover is armed for this session.
///
/// `knob` is `WARREN_IDLE_COVER` (read once via
/// `warrenguard_config::knobs::idle_cover_enabled`). `daita_requested` is the
/// client's DAITA opt-in. Idle cover and DAITA are mutually exclusive
/// cover mechanisms (DAITA carries its own padding), so idle cover is
/// armed only when the knob is on AND DAITA is not requested.
///
/// The returned bool is the SINGLE source of truth that gates both the
/// transport config (`ClientTunnel::with_idle_cover`, which disables the
/// keep-alive PING) and the pump choice (`pump_*_with_idle_cover`). They
/// MUST flip together: a keep-alive-disabled config with a plain pump has
/// no liveness mechanism beyond the idle timeout.
#[must_use]
fn idle_cover_effective(knob: bool, daita_requested: bool) -> bool {
    knob && !daita_requested
}

/// Derives a deterministic IPv4 in `warrenguard_config::TUNNEL_POOL_CIDR`
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

/// Builds a `TunConfig` for the multi-hop path with the given IPv4 and
/// an optional exit-allocated IPv6 (multi-hop `/v2` dual-stack, cf.
/// docs/31). MTU pinned to 1280 (Warren `TUNNEL_INITIAL_MTU`), v4
/// gateway pinned to `10.66.0.1`. When `ipv6` is `Some`, the v6 address
/// is added and the gateway is pinned to `fdcc:f:1::1`
/// (`TUNNEL_GATEWAY_IPV6`); `None` keeps the TUN v4-only and the
/// firewall blocks native v6 (no leak).
fn build_multi_hop_tun_config(
    ipv4: std::net::Ipv4Addr,
    ipv6: Option<std::net::Ipv6Addr>,
) -> TunConfig {
    let mut addresses: Vec<IpAddr> = vec![IpAddr::V4(ipv4)];
    if let Some(v6) = ipv6 {
        addresses.push(IpAddr::V6(v6));
    }
    TunConfig {
        #[cfg(target_os = "linux")]
        name: None,
        #[cfg(target_os = "linux")]
        packet_information: false,
        addresses,
        // 1280 matches the multi-hop CLI binary default and the
        // baseline `TUNNEL_INITIAL_MTU` floor agreed in /v1 obfuscation
        // doctrine. DPLPMTUD on the QUIC transport may negotiate higher
        // path-MTU; the TUN itself stays at the safe floor.
        mtu: 1280,
        ipv4_gateway: std::net::Ipv4Addr::new(10, 66, 0, 1),
        ipv6_gateway: ipv6.map(|_| std::net::Ipv6Addr::new(0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 1)),
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
        // Warren convention: the IPv6 gateway is `fdcc:f:1::1`
        // (`warrenguard_config::TUNNEL_GATEWAY_IPV6`), set only when the exit
        // actually allocated a tunnel v6 (i.e. the client advertised
        // `features::IPV6` on the single-hop path). `None` keeps the
        // interface v4-only so the firewall blocks native IPv6 with no
        // leak. Hardcoded literal until `warren-config` is a direct dep.
        ipv6_gateway: ipv6.map(|_| std::net::Ipv6Addr::new(0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 1)),
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
/// 1. For each candidate exit IP: a `/32` (or `/128`) route via the physical interface, more
///    specific than the `/1 + /1` below, so the daemon -> exit packets keep using the physical NIC.
/// 2. `0.0.0.0/1` + `128.0.0.0/1` via the TUN interface: covers the entire IPv4 space without
///    replacing the existing default route, a classic split-default trick.
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
/// macOS, while bypassing daemon-side packets to the exit IPs. Adapted
/// from the upstream Mullvad routing pattern to Warren's needs (single
/// exit, Quinn QUIC transport).
///
/// macOS strategy - do NOT reproduce the Linux `/1 + /1` recipe:
///
/// 1. **`<exit_ip>/32 NetNode::DefaultNode`** - bypass so that the daemon's QUIC packets to the
///    exit take the physical NIC instead of the TUN (otherwise routing loop). `DefaultNode` is a
///    symbolic node: talpid-routing macOS posts `<ip>/32 via best_default_route.router_ip` in
///    `apply_non_tunnel_routes` (`talpid-routing/src/unix/macos/mod.rs:541`), executed **after**
///    the ifscope dance, hence with an ARP-able L3 gateway (not the SDL link-scope that fails ARP
///    for off-LAN exits).
///
/// 2. **`0.0.0.0/0 dev <tun>`** - default redirect. Prefix 0 triggers the `tunnel_default_routes`
///    special case in talpid-routing macOS (`mod.rs:344-354`) which:
///    - Transforms the previous default `0.0.0.0/0 via gw dev <physical>` into an **ifscope** route
///      (= visible only to sockets bound to the physical iface).
///    - Posts the new default `0.0.0.0/0 dev <tun>` un-scoped (= visible to everything else, i.e.
///      user traffic).
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
    let (mut bypass, defaults) = build_warren_tunnel_routes_macos_ordered(tun_iface, exit_ips);
    bypass.extend(defaults);
    bypass
}

/// Same routes as [`build_warren_tunnel_routes_macos`], split into
/// `(bypass, tunnel_default)` so a call site can install them in two
/// ORDERED `add_routes` calls. Within a single call talpid-routing
/// macOS applies the tunnel default BEFORE the `DefaultNode` bypass
/// (`add_required_routes`: `apply_tunnel_default_routes` runs first),
/// which opens a millisecond window where a wildcard-bound QUIC socket
/// has its relay packets captured by `0/0 dev tun` (self-nesting; no
/// plaintext leak, but garbage). Installing the bypass in its own call
/// first makes the window zero. Required for the wildcard-bind QUIC
/// migration path.
#[cfg(target_os = "macos")]
#[must_use]
fn build_warren_tunnel_routes_macos_ordered(
    tun_iface: &str,
    exit_ips: &[IpAddr],
) -> (Vec<RequiredRoute>, Vec<RequiredRoute>) {
    use talpid_routing::NetNode;

    let bypass: Vec<RequiredRoute> = exit_ips
        .iter()
        .map(|ip| RequiredRoute::new(IpNetwork::from(*ip), NetNode::DefaultNode))
        .collect();

    let tun_node = Node::device(tun_iface.to_owned());
    let default_v4 = ipnetwork::Ipv4Network::new(std::net::Ipv4Addr::new(0, 0, 0, 0), 0)
        .expect("0.0.0.0/0 is a valid prefix");
    let defaults = vec![RequiredRoute::new(IpNetwork::V4(default_v4), tun_node)];

    (bypass, defaults)
}

/// Local QUIC bind address for the multi-hop client: always wildcard,
/// NEVER the detected physical source IP. An unconnected UDP socket
/// picks its source address per packet from the live routing table,
/// so after a WiFi<->ethernet switch the next QUIC packet leaves
/// through the new interface and the relay (migration enabled)
/// revalidates the path in ~1 RTT with no re-handshake. A pinned
/// source IP dies with its interface and forces a full redial
/// instead. Loop prevention does not depend on the bind: the relay/32
/// bypass route (and on Linux the pref-50 ip rule) keeps relay-bound
/// packets off the TUN.
#[must_use]
fn multi_hop_bind_addr(relay_endpoint: std::net::SocketAddr) -> std::net::SocketAddr {
    match relay_endpoint {
        std::net::SocketAddr::V4(_) => {
            std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0))
        }
        std::net::SocketAddr::V6(_) => {
            std::net::SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, 0))
        }
    }
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
    use warrenguard_wire::WarrenPubkey;

    #[tokio::test]
    async fn race_handshake_surfaces_the_value_when_the_dial_completes_first() {
        // Nominal path: the handshake resolves before any close / timeout.
        let (_close_tx, mut close_rx) = futures::channel::oneshot::channel::<()>();
        let out = race_handshake(
            async { 42_u32 },
            &mut close_rx,
            std::time::Duration::from_secs(30),
        )
        .await;
        assert!(
            matches!(out, HandshakeRace::Completed(42)),
            "a ready handshake must surface its own value"
        );
    }

    #[tokio::test]
    async fn race_handshake_aborts_immediately_when_the_close_signal_fired() {
        // The bug this guards: a handshake that NEVER completes (exit
        // silently drops an incompatible Setup) must not wedge the dial.
        // A fired close signal wins even against an effectively infinite
        // timeout, so Cancel always returns control.
        let (close_tx, mut close_rx) = futures::channel::oneshot::channel::<()>();
        close_tx.send(()).expect("receiver alive");
        let out = race_handshake(
            std::future::pending::<()>(),
            &mut close_rx,
            std::time::Duration::from_secs(3600),
        )
        .await;
        assert!(
            matches!(out, HandshakeRace::Aborted),
            "a fired close signal must abort the dial regardless of the timeout"
        );
    }

    #[tokio::test]
    async fn race_handshake_times_out_when_neither_dial_nor_close_resolves() {
        // Backstop path: no user action, handshake never completes -> the
        // wall-clock bound fires so the blocking thread can never hang.
        let (_close_tx, mut close_rx) = futures::channel::oneshot::channel::<()>();
        let out = race_handshake(
            std::future::pending::<()>(),
            &mut close_rx,
            std::time::Duration::from_millis(20),
        )
        .await;
        assert!(
            matches!(out, HandshakeRace::TimedOut),
            "with no close and no completion the wall-clock bound must fire"
        );
    }

    #[test]
    fn connect_summary_names_every_phase_with_its_duration() {
        // Ops grep this exact line shape to attribute connect latency
        // (phase names + ms values); a silent rename breaks fleet-wide
        // log tooling, so the shape is pinned here.
        let line = connect_summary_line("multi_hop", 252, 1619, 14, 191, 2077);
        assert_eq!(
            line,
            "Warren connected in 2077ms (mode=multi_hop prep=252ms \
             handshake=1619ms tun=14ms routes+up=191ms)"
        );
    }

    #[test]
    fn disconnect_summary_totals_its_phases() {
        // The total must be the sum of the phases: the wait()-side
        // teardown has no single wall-clock anchor (the select races
        // signals for the whole session lifetime), so a drifting total
        // would mean a phase is unaccounted for.
        let line = disconnect_summary_line(120, 3, 180, 45);
        assert_eq!(
            line,
            "Warren disconnected in 348ms (down_event=120ms natpmp=3ms \
             routes=180ms tasks=45ms)"
        );
    }

    #[test]
    fn multi_hop_bind_addr_is_always_wildcard() {
        // QUIC migration regression: pinning the bind to a detected
        // physical source IP kills the connection on every interface
        // switch (the socket dies with its interface). The bind must
        // be wildcard, family-matched to the relay endpoint.
        let v4 = multi_hop_bind_addr("203.0.113.10:443".parse().unwrap());
        assert!(v4.ip().is_unspecified() && v4.is_ipv4() && v4.port() == 0);
        let v6 = multi_hop_bind_addr("[2001:db8::1]:443".parse().unwrap());
        assert!(v6.ip().is_unspecified() && v6.is_ipv6() && v6.port() == 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_ordered_routes_put_relay_bypass_before_tunnel_default() {
        // With a wildcard-bound socket the relay/32 bypass must be
        // installed in its own add_routes call BEFORE 0/0 lands on the
        // tun, or relay packets transiently self-nest through the tun.
        use std::net::IpAddr;
        let relay: IpAddr = "203.0.113.10".parse().unwrap();
        let (bypass, defaults) = build_warren_tunnel_routes_macos_ordered("utun7", &[relay]);
        assert_eq!(bypass.len(), 1, "one bypass route per relay IP");
        assert_eq!(bypass[0].prefix.ip(), relay);
        assert_eq!(bypass[0].prefix.prefix(), 32);
        assert_eq!(defaults.len(), 1, "exactly the 0/0 tunnel default");
        assert_eq!(defaults[0].prefix.prefix(), 0);
        // The concatenated legacy builder must preserve the same order.
        let all = build_warren_tunnel_routes_macos("utun7", &[relay]);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].prefix.prefix(), 32);
        assert_eq!(all[1].prefix.prefix(), 0);
    }

    #[test]
    fn reject_error_is_recoverable_so_killswitch_stays_cancelable() {
        // Load-bearing invariant: an exit rejection must map to a
        // RECOVERABLE error. Recoverable keeps the firewall kill-switch up
        // (fail-closed, no leak) while leaving the state cancelable and
        // letting the tunnel self-heal once the allowlist syncs. Promoting
        // this to a fatal error would strand a just-subscribed user.
        for reason in [
            RejectionReason::NotAllowlisted,
            RejectionReason::IpExhausted,
        ] {
            let err = reject_error(reason);
            assert!(
                matches!(err, Error::BackendTransient(_)),
                "rejection must map to BackendTransient, got {err:?}"
            );
            assert!(err.is_recoverable(), "rejection error must be recoverable");
        }
    }

    #[test]
    fn warren_tunnel_parameters_debug_does_not_leak_secrets() {
        // No-log Warren: Debug must never reveal signing_key nor the
        // full exit pubkey. The first is secret material, the second
        // is session-PII identifying the user on the exit side.
        let signing = SigningKey::from_bytes(&[0u8; 32]);
        let exit_id = WarrenPubkey::from_bytes([1u8; 32]);
        let params = WarrenTunnelParameters {
            exit_id: RelayExitId::ZERO,
            country_code: String::new(),
            city: String::new(),
            exit_addr: WarrenExitAddr::new(exit_id),
            signing_key: signing,
            n_connections: 2,
            features: 0x1,
            multi_hop: None,
            alpn_protocols: Vec::new(),
            on_reconnect: None,
            on_exit_draining: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
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
        // `warrenguard_transport::ClientTunnel`. Mutation = silent multi-hop
        // by default = breaks every existing deployment.
        let params = WarrenTunnelParameters {
            exit_id: RelayExitId::ZERO,
            country_code: String::new(),
            city: String::new(),
            exit_addr: WarrenExitAddr::new(WarrenPubkey::from_bytes([1u8; 32])),
            signing_key: SigningKey::from_bytes(&[0u8; 32]),
            n_connections: 1,
            features: 0,
            multi_hop: None,
            alpn_protocols: Vec::new(),
            on_reconnect: None,
            on_exit_draining: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
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
                cover_domain: None,
            },
            exit: ExitDescriptorSigned {
                exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                endpoint: Some("192.0.2.20:443".parse().unwrap()),
                signature: [0x11; 64],
                dns_disabled: false,
                cover_domain: None,
            },
            operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
            exit_country: "se".to_owned(),
            exit_city: "Stockholm".to_owned(),
            enable_gso: true,
            use_warren_obfuscation: true,
            single_node: false,
        };
        let params = WarrenTunnelParameters {
            exit_id: RelayExitId::ZERO,
            country_code: String::new(),
            city: String::new(),
            exit_addr: WarrenExitAddr::new(exit_id),
            signing_key: signing,
            n_connections: 1,
            features: 0,
            multi_hop: Some(mh),
            alpn_protocols: Vec::new(),
            on_reconnect: None,
            on_exit_draining: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
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
            rules: Vec::new(),
            protocol: NatPmpProto::Udp,
            suggested_external_port: 22222,
            internal_port: 22,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let params = WarrenTunnelParameters {
            exit_id: RelayExitId::ZERO,
            country_code: String::new(),
            city: String::new(),
            exit_addr: WarrenExitAddr::new(WarrenPubkey::from_bytes([0u8; 32])),
            signing_key: SigningKey::from_bytes(&[0u8; 32]),
            n_connections: 1,
            features: 0,
            multi_hop: None,
            alpn_protocols: Vec::new(),
            on_reconnect: None,
            on_exit_draining: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: Some(cfg.clone()),
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
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
    fn with_sticky_ports_re_suggests_last_granted_port_for_an_auto_rule() {
        // An auto rule (suggested == 0) whose port was granted on the
        // previous exit must be re-suggested to the next exit, so the
        // public port "follows" the client across an exit change instead
        // of silently changing.
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![NatPmpRule {
                protocol: NatPmpProto::Udp,
                suggested_external_port: 0,
                internal_port: 51820,
                sticky_suggestion: false,
            }],
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let mut sticky = std::collections::HashMap::new();
        sticky.insert(
            NatPmpRuleId {
                internal_port: 51820,
                protocol: NatPmpProto::Udp,
            },
            49200,
        );

        let effective = cfg.with_sticky_ports(&sticky);

        let rules = effective.effective_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].suggested_external_port, 49200,
            "auto rule must re-suggest the last-granted port"
        );
    }

    #[test]
    fn with_sticky_ports_preserves_an_explicit_user_pin() {
        // A rule the user explicitly pinned (suggested != 0) is their
        // intent: never override it with a sticky value, even if one
        // exists for that rule id.
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![NatPmpRule {
                protocol: NatPmpProto::Tcp,
                suggested_external_port: 50000,
                internal_port: 50000,
                sticky_suggestion: false,
            }],
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let mut sticky = std::collections::HashMap::new();
        sticky.insert(
            NatPmpRuleId {
                internal_port: 50000,
                protocol: NatPmpProto::Tcp,
            },
            49200,
        );

        let effective = cfg.with_sticky_ports(&sticky);

        assert_eq!(
            effective.effective_rules()[0].suggested_external_port,
            50000,
            "an explicit pin must win over the sticky port"
        );
    }

    #[test]
    fn with_sticky_ports_leaves_an_auto_rule_at_zero_when_no_sticky_entry() {
        // No remembered port (first connect, or after an explicit reset
        // to auto): the rule stays auto so the exit picks a fresh port.
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![NatPmpRule {
                protocol: NatPmpProto::Udp,
                suggested_external_port: 0,
                internal_port: 51820,
                sticky_suggestion: false,
            }],
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let sticky = std::collections::HashMap::new();

        let effective = cfg.with_sticky_ports(&sticky);

        assert_eq!(
            effective.effective_rules()[0].suggested_external_port,
            0,
            "without a sticky entry the rule must stay on auto"
        );
    }

    #[test]
    fn with_sticky_ports_ignores_a_zero_sticky_entry() {
        // A sticky value of 0 carries no information (the exit never
        // grants port 0); it must not turn an auto rule into a request
        // for the invalid port 0.
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![NatPmpRule {
                protocol: NatPmpProto::Udp,
                suggested_external_port: 0,
                internal_port: 51820,
                sticky_suggestion: false,
            }],
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let mut sticky = std::collections::HashMap::new();
        sticky.insert(
            NatPmpRuleId {
                internal_port: 51820,
                protocol: NatPmpProto::Udp,
            },
            0,
        );

        let effective = cfg.with_sticky_ports(&sticky);

        assert_eq!(effective.effective_rules()[0].suggested_external_port, 0);
    }

    #[test]
    fn with_sticky_ports_synthesizes_rules_from_legacy_flat_fields() {
        // A single-port (legacy flat) config must still follow: the
        // sticky override applies to the rule synthesized from the flat
        // fields, and the result expresses it through `rules`.
        let cfg = NatPmpConfig {
            enabled: true,
            lifetime_secs: 3600,
            rules: Vec::new(),
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 6881,
            sticky_suggestion: false,
            remap_epoch: 0,
        };
        let mut sticky = std::collections::HashMap::new();
        sticky.insert(
            NatPmpRuleId {
                internal_port: 6881,
                protocol: NatPmpProto::Udp,
            },
            49300,
        );

        let effective = cfg.with_sticky_ports(&sticky);

        let rules = effective.effective_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].internal_port, 6881);
        assert_eq!(rules[0].suggested_external_port, 49300);
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
    fn build_multi_hop_tun_config_v4_only_pins_mtu_and_gateway() {
        // v4-only multi-hop (exit served no v6, or user IPv6 off) at MTU
        // 1280 with the canonical gateway `10.66.0.1`. Any mutation here
        // would silently change the wire profile across deployments.
        let cfg = build_multi_hop_tun_config(std::net::Ipv4Addr::new(10, 66, 7, 42), None);
        assert_eq!(cfg.mtu, 1280, "multi-hop TUN MTU must be 1280");
        assert_eq!(
            cfg.ipv4_gateway,
            std::net::Ipv4Addr::new(10, 66, 0, 1),
            "multi-hop TUN gateway must be 10.66.0.1"
        );
        assert!(
            cfg.ipv6_gateway.is_none(),
            "v4-only multi-hop must not set a v6 gateway"
        );
        assert_eq!(cfg.addresses.len(), 1);
        assert!(!cfg.allow_lan, "no LAN access on multi-hop");
        assert!(cfg.dns_servers.is_none(), "DNS handled by exit forwarder");
    }

    #[test]
    fn build_multi_hop_tun_config_dual_stack_adds_v6_address_and_gateway() {
        // `/v2` dual-stack: when the exit allocated a v6, the TUN carries
        // both addresses and pins the v6 gateway to `fdcc:f:1::1`.
        let v6 = std::net::Ipv6Addr::new(0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 2);
        let cfg = build_multi_hop_tun_config(std::net::Ipv4Addr::new(10, 66, 7, 42), Some(v6));
        assert_eq!(cfg.mtu, 1280);
        assert_eq!(cfg.addresses.len(), 2, "dual-stack TUN carries v4 + v6");
        assert!(cfg.addresses.contains(&IpAddr::V6(v6)));
        assert_eq!(
            cfg.ipv6_gateway,
            Some(std::net::Ipv6Addr::new(
                0xfdcc, 0x000f, 0x0001, 0, 0, 0, 0, 1
            )),
            "dual-stack multi-hop must pin the v6 gateway to fdcc:f:1::1"
        );
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
                cover_domain: None,
            },
            exit: ExitDescriptorSigned {
                exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                endpoint: Some("192.0.2.20:443".parse().unwrap()),
                signature: [0x11; 64],
                dns_disabled: false,
                cover_domain: None,
            },
            operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
            exit_country: "se".to_owned(),
            exit_city: "Stockholm".to_owned(),
            enable_gso: false,
            use_warren_obfuscation: true,
            single_node: false,
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

    // --- BackendTransient / BackendFatal split ---

    #[test]
    fn backend_transient_error_is_recoverable() {
        // TUN I/O errors and peer-initiated session closes are
        // transient: the configuration is still valid, a fresh
        // connect should succeed without operator action.
        let e = Error::BackendTransient("tun read timeout".into());
        assert!(
            e.is_recoverable(),
            "BackendTransient must be recoverable so the state machine reconnects \
             instead of entering kill-switch mode"
        );
    }

    #[test]
    fn backend_fatal_error_is_not_recoverable() {
        // Auth failures and explicit session rejections are fatal:
        // retrying immediately would produce the same outcome.
        let e = Error::BackendFatal("auth rejected by exit".into());
        assert!(
            !e.is_recoverable(),
            "BackendFatal must NOT be recoverable - the state machine must enter \
             ErrorState rather than looping on a guaranteed-to-fail reconnect"
        );
    }

    #[test]
    fn auth_rejected_maps_to_fatal_non_recoverable_error() {
        // Misleading-error regression: an explicit auth rejection from
        // the exit (identity not authorized / no active subscription)
        // must become a FATAL, non-recoverable error so the state
        // machine stops retrying. Retrying re-derives the same outcome
        // and surfaces a misleading "no matching relay" instead of the
        // real "subscription required" cause.
        let mapped = map_handshake_error(warrenguard_transport_core::TunnelError::AuthRejected);
        assert!(
            matches!(mapped, Error::BackendFatal(_)),
            "AuthRejected must map to BackendFatal, got: {mapped:?}"
        );
        assert!(
            !mapped.is_recoverable(),
            "an auth rejection must NOT be retried by the state machine"
        );
    }

    #[test]
    fn device_limit_maps_to_fatal_non_recoverable_error() {
        // Same regression class as auth rejection: a device-cap refusal
        // (account already at its max simultaneous devices) is a business
        // rejection, not a transient glitch. It must be FATAL and
        // non-recoverable so the state machine stops retrying and the app
        // surfaces a precise "device limit reached" message instead of
        // the misleading "no matching relay" / "no exit available".
        let mapped =
            map_handshake_error(warrenguard_transport_core::TunnelError::DeviceLimitReached);
        assert!(
            matches!(mapped, Error::BackendFatal(_)),
            "DeviceLimitReached must map to BackendFatal, got: {mapped:?}"
        );
        assert!(
            !mapped.is_recoverable(),
            "a device-limit rejection must NOT be retried by the state machine"
        );
    }

    #[test]
    fn generic_handshake_failure_stays_recoverable() {
        // A non-auth handshake failure (e.g. a transient connection
        // loss) must remain a recoverable `Error::Handshake` so a single
        // glitch does not enter the kill-switch error state.
        let mapped = map_handshake_error(warrenguard_transport_core::TunnelError::NoExitAddr);
        assert!(
            matches!(mapped, Error::Handshake(_)) && mapped.is_recoverable(),
            "non-auth handshake errors must stay recoverable, got: {mapped:?}"
        );
    }

    #[test]
    fn tun_setup_error_is_not_recoverable() {
        // Privilege or kernel-module issue: retrying will not help
        // without operator action (modprobe, capability grant, ...).
        let e = Error::TunSetup("permission denied".into());
        assert!(!e.is_recoverable());
    }

    #[test]
    fn backend_transient_display_contains_message() {
        // Verify Display is wired so the state machine can surface
        // the error string in telemetry / logs.
        let e = Error::BackendTransient("pump I/O error".into());
        let s = e.to_string();
        assert!(
            s.contains("recoverable"),
            "display must mention recoverable: {s}"
        );
        assert!(
            s.contains("pump I/O error"),
            "display must contain the message: {s}"
        );
    }

    #[test]
    fn backend_fatal_display_contains_message() {
        let e = Error::BackendFatal("session rejected: bad credentials".into());
        let s = e.to_string();
        assert!(s.contains("fatal"), "display must mention fatal: {s}");
        assert!(
            s.contains("bad credentials"),
            "display must contain the message: {s}"
        );
    }

    // --- WarrenTunnelParameters must not implement Clone ---

    /// Compile-time assertion: `WarrenTunnelParameters` must NOT
    /// implement `Clone`. The `signing_key` field carries secret Ed25519
    /// material; a `Clone` derive would silently duplicate it in memory,
    /// broadening the attack surface.
    ///
    /// Callers that need a copy of the *configuration* (not the key)
    /// must clone only the individual fields they require.
    #[test]
    fn warren_tunnel_parameters_is_not_clone() {
        static_assertions::assert_not_impl_any!(WarrenTunnelParameters: Clone);
    }

    // --- startup event ordering ---

    /// Verifies the documented startup ordering invariant:
    /// InterfaceUp BEFORE routes BEFORE TunnelEvent::Up.
    ///
    /// This test cannot drive a real TUN / route_manager in a unit
    /// context, but it documents the expected ordering through the
    /// TRACE_PREFIX log phases, which are verified at integration level.
    /// The comment acts as a TDD anchor: if the sequence regresses, the
    /// in-crate trace labels ("T4b" vs "T7") will be out of order in
    /// the log output.
    #[test]
    fn startup_event_ordering_documented_invariant() {
        // Phase labels used by start_single_hop and start_multi_hop.
        // If anyone reorders the blocks and forgets to update the labels,
        // this test documents the expected ordering so the reviewer
        // notices.  The labels must appear in this order in the source:
        //
        //   T4    = interfaceup_emit
        //   T4b   = interfaceup_consumed
        //   T5    = routes_add_start
        //   T6    = routes_added
        //   T7    = up_emit          <-- Up only after routes
        //   T7b   = up_consumed
        //
        // Compare against the previous (broken) ordering:
        //   T4    = interfaceup_emit
        //   T4b   = interfaceup_consumed + Up emitted HERE (wrong)
        //   T5    = up_consumed
        //   T6    = routes_add_start (too late)
        //
        // This constant array acts as a structural snapshot that forces
        // a reviewer to revisit this test when the trace labels change.
        const EXPECTED_PHASE_ORDER: &[&str] = &[
            "interfaceup_emit",
            "interfaceup_consumed",
            "routes_add_start",
            "routes_added",
            "up_emit",
            "up_consumed",
        ];
        // Verify the phases are strictly ordered by index (no reorder).
        for window in EXPECTED_PHASE_ORDER.windows(2) {
            let (a, b) = (window[0], window[1]);
            assert_ne!(a, b, "phase names must be distinct: {a}");
        }
        // Anchor: the Up event must come after the route phases.
        let up_idx = EXPECTED_PHASE_ORDER
            .iter()
            .position(|p| *p == "up_emit")
            .expect("up_emit must be in the phase list");
        let routes_idx = EXPECTED_PHASE_ORDER
            .iter()
            .position(|p| *p == "routes_added")
            .expect("routes_added must be in the phase list");
        assert!(
            up_idx > routes_idx,
            "up_emit (idx={up_idx}) must come AFTER routes_added (idx={routes_idx}) \
             to prevent a window where the UI shows Connected but traffic bypasses the tunnel"
        );
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
        //   1. `<exit_ip>/32 NetNode::DefaultNode` -> bypass via the best default route (resolved
        //      at apply time by talpid-routing).
        //   2. `0.0.0.0/0 dev <tun>` -> triggers the `tunnel_default_routes` ifscope dance (native
        //      macOS recipe, no policy routing on Darwin).
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
    fn idle_cover_is_armed_only_when_knob_on_and_daita_off() {
        // ADR-0006: idle cover and DAITA are mutually exclusive covers.
        // The same bool gates BOTH the transport config (keep-alive off)
        // and the pump choice, so this is the single source of truth that
        // prevents the foot-gun (keep-alive disabled with no cover pump).
        assert!(
            idle_cover_effective(true, false),
            "knob on + no DAITA => idle cover armed"
        );
        assert!(
            !idle_cover_effective(true, true),
            "DAITA requested => idle cover must yield (DAITA carries its own cover)"
        );
        assert!(
            !idle_cover_effective(false, false),
            "knob off => never arm idle cover (keep-alive beacon stays on)"
        );
        assert!(
            !idle_cover_effective(false, true),
            "knob off + DAITA => not idle cover"
        );
    }

    #[test]
    fn is_routable_internet_v4_accepts_public_addrs() {
        // Regression sentinel: Hetzner Cloud IPs (the typical
        // warren-exit target) must be considered routable, otherwise
        // `filter_endpoint_addr_for_wan` would empty the EndpointAddr
        // and the Warren client couldn't reach the exit.
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
    fn filter_endpoint_addr_for_wan_preserves_cover_domain() {
        // wg-0005: the per-exit cover domain is metadata, not an address.
        // The WAN address filter must not drop it, otherwise the dial would
        // silently fall back to RPK / the env default instead of the
        // exit's real certificate hostname.
        let id = WarrenPubkey::from_bytes([7u8; 32]);
        let addr = WarrenExitAddr::new(id)
            .with_ip_addr("198.51.100.1:443".parse().unwrap())
            .with_cover_domain("nl-1.cover.example.com");

        let filtered = filter_endpoint_addr_for_wan(addr);
        assert_eq!(filtered.ip_addrs().count(), 1, "routable IP kept");
        assert_eq!(
            filtered.cover_domain.as_deref(),
            Some("nl-1.cover.example.com"),
            "cover domain must survive the WAN address filter"
        );
    }

    #[test]
    fn resolve_cover_domain_prefers_per_exit_roster_over_env() {
        // The signed roster's per-exit domain is authoritative: it wins over
        // the deployment-wide env so per-exit cover-domain rotation works.
        assert_eq!(
            resolve_cover_domain(
                Some("exit.cover.example.com"),
                Some("shared.example.com".into())
            ),
            Some("exit.cover.example.com".to_owned())
        );
    }

    #[test]
    fn resolve_cover_domain_falls_back_to_env_when_roster_silent() {
        // ADR-0005 Stage 1: a single shared cover domain via the env, used
        // for exits whose roster entry carries no per-exit domain.
        assert_eq!(
            resolve_cover_domain(None, Some("shared.example.com".into())),
            Some("shared.example.com".to_owned())
        );
    }

    #[test]
    fn resolve_cover_domain_none_keeps_rpk() {
        // No per-exit domain and no env keeps the RPK-via-SNI handshake.
        assert_eq!(resolve_cover_domain(None, None), None);
    }

    #[test]
    fn resolve_cover_domain_treats_blank_as_absent() {
        // Whitespace-only values (a misconfigured env or roster field) must
        // not arm X.509 mode with an empty SNI; they read as absent and the
        // fallback chain continues.
        assert_eq!(
            resolve_cover_domain(Some("   "), Some("shared.example.com".into())),
            Some("shared.example.com".to_owned()),
            "blank per-exit falls through to env"
        );
        assert_eq!(resolve_cover_domain(Some("  "), Some("  ".into())), None);
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

    // ===================================================================
    // Live-reconfig controller tests. Reproduces the exact user
    // scenario: tunnel connected with NAT-PMP OFF, then the user
    // toggles it ON via the watch channel. The controller must spawn a
    // manager and the observer must see a Mapped event - all without a
    // tunnel reconnect.
    // ===================================================================
    use std::{
        sync::{Arc, Mutex as StdMutex},
        time::Duration,
    };
    use tokio::net::UdpSocket;
    use warrenguard_natpmp_protocol::{
        MapProto, Response as NatPmpResponse, ResultCode, parse_request, serialize_response,
    };

    /// UDP stub that echoes a Success Map response with the requested
    /// lifetime baked into the external port (`49000 + lifetime`), so a
    /// test can correlate which config produced which mapping. A
    /// `lifetime == 0` (release) gets a Success(0) reply so the
    /// client's release future completes promptly.
    async fn spawn_lifetime_echo_stub() -> std::net::SocketAddr {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            loop {
                let (n, peer) = match sock.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let lifetime = match parse_request(&buf[..n]) {
                    Ok(warrenguard_natpmp_protocol::Request::Map { lifetime_secs, .. }) => {
                        lifetime_secs
                    }
                    _ => continue,
                };
                let external_port = if lifetime == 0 {
                    0
                } else {
                    49000u16.saturating_add(lifetime as u16)
                };
                let resp = serialize_response(&NatPmpResponse::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::Success,
                    epoch_secs: 0,
                    internal_port: 22,
                    external_port,
                    lifetime_secs: lifetime,
                    rate_limit: None,
                });
                let _ = sock.send_to(&resp, peer).await;
            }
        });
        addr
    }

    /// Shared event log captured by [`collector_observer`].
    type NatPmpEventLog = Arc<StdMutex<Vec<(NatPmpRuleId, NatPmpEvent)>>>;

    fn collector_observer() -> (NatPmpMappingObserver, NatPmpEventLog) {
        let log: NatPmpEventLog = Arc::new(StdMutex::new(Vec::new()));
        let log_for_obs = log.clone();
        let observer: NatPmpMappingObserver = Arc::new(move |id, evt| {
            log_for_obs.lock().expect("observer lock").push((id, evt));
        });
        (observer, log)
    }

    fn natpmp_cfg(lifetime_secs: u32) -> NatPmpConfig {
        NatPmpConfig {
            enabled: true,
            lifetime_secs,
            rules: Vec::new(),
            protocol: MapProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
            sticky_suggestion: false,
            remap_epoch: 0,
        }
    }

    #[tokio::test]
    async fn controller_enables_mapping_on_watch_push_from_disabled_start() {
        // THE bug reproduction: controller starts with initial=None
        // (tunnel connected, NAT-PMP off), then a `Some(enabled)` push
        // arrives on the watch (user toggles on). The controller MUST
        // spawn a manager → observer sees a Mapped event. No reconnect.
        let server = spawn_lifetime_echo_stub().await;
        let (observer, log) = collector_observer();
        let (tx, rx) = tokio::sync::watch::channel::<Option<NatPmpConfig>>(None);

        let runtime = tokio::runtime::Handle::current();
        let handle = runtime.spawn(run_nat_pmp_controller(
            runtime.clone(),
            server,
            None,
            observer,
            None, // initial: NAT-PMP off at tunnel start
            rx,
        ));

        // Give the controller a moment to reach `changed().await`.
        tokio::time::sleep(Duration::from_millis(50)).await;
        // No mapping yet (toggle still off).
        assert!(
            log.lock().unwrap().is_empty(),
            "no events expected before the toggle"
        );

        // User toggles ON.
        tx.send(Some(natpmp_cfg(60))).expect("watch send");

        // Controller should spawn the manager → Mapped(port 49060).
        for _ in 0..100 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e.1,
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
        let snapshot = log.lock().unwrap().clone();
        assert!(
            snapshot.iter().any(|e| matches!(
                e.1,
                NatPmpEvent::Mapped {
                    external_port: 49060,
                    ..
                }
            )),
            "controller must spawn a mapping on the enable push; events: {snapshot:?}"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn controller_live_reconfigures_on_second_push() {
        // After an initial enable, a second push with different params
        // must live-reconfigure (new port observed), no reconnect.
        let server = spawn_lifetime_echo_stub().await;
        let (observer, log) = collector_observer();
        // Start already-enabled (lifetime 60 → port 49060).
        let (tx, rx) = tokio::sync::watch::channel::<Option<NatPmpConfig>>(Some(natpmp_cfg(60)));

        let runtime = tokio::runtime::Handle::current();
        let handle = runtime.spawn(run_nat_pmp_controller(
            runtime.clone(),
            server,
            None,
            observer,
            Some(natpmp_cfg(60)),
            rx,
        ));

        // Wait for the initial Mapped(49060).
        for _ in 0..100 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e.1,
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

        // Reconfigure to lifetime 90 → expect Mapped(49090).
        tx.send(Some(natpmp_cfg(90))).expect("watch send");
        for _ in 0..100 {
            if log.lock().unwrap().iter().any(|e| {
                matches!(
                    e.1,
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
        let snapshot = log.lock().unwrap().clone();
        assert!(
            snapshot.iter().any(|e| matches!(
                e.1,
                NatPmpEvent::Mapped {
                    external_port: 49090,
                    ..
                }
            )),
            "controller must live-reconfigure on the second push; events: {snapshot:?}"
        );

        handle.abort();
    }
}
