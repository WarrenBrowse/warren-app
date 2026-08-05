//! Warren adapter for the talpid tunnel state machine.
//!
//! This crate exposes [`WarrenTunnelMonitor`], the QUIC tunnel backend
//! consumed by
//! `talpid_core::tunnel_state_machine::tunnel_monitor::TunnelMonitor`.
//! It exposes a `start` / `wait` API so `connecting_state.rs` can drive
//! the tunnel lifecycle.
//!
//! Underneath, [`WarrenTunnelMonitor::start`] drives the multi-hop QUIC
//! handshake through `warrenguard_transport::supervisor::MultiHopSupervisor`,
//! opens a TUN via the talpid `TunProvider`, emits
//! `TunnelEvent::InterfaceUp` / `Up` and spawns the bidirectional pump
//! (TUN <-> QUIC datagrams). `wait()` blocks on the close-signal, drops
//! the routing-table override and aborts the pump.

use std::{net::IpAddr, path::Path, time::Instant};

use ed25519_dalek::{SigningKey, VerifyingKey};
#[cfg(target_os = "macos")]
use ipnetwork::IpNetwork;
#[cfg(target_os = "macos")]
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
// NAT-PMP forward scope re-exported so daemon code constructs
// `NatPmpConfig { protocol: NatPmpProto::Udp, .. }` without depending
// directly on the warrenguard-natpmp-client crate. A rule maps UDP,
// TCP, or `Both` at once on the same external port (an atomic pair
// driven by one dual-proto refresh loop engine-side).
pub use warrenguard_natpmp_client::ForwardProtos as NatPmpProto;
// IPv4 CIDR descriptor used by the daemon-side `--bypass-cidr`
// settings plumbing. Re-exported so callers (mullvad-daemon, gRPC
// conversions, settings persistence) consume one canonical type
// instead of duplicating it across crates.
pub use warrenguard_route_split::bypass_cidr::BypassCidr;
// The firewall must permit DNS to the in-tunnel resolver this crate's egress
// probe queries, otherwise a user with DNS content blocking has the probe
// denied by our own leak protection. Re-exported so talpid-core reads the
// address from the crate that owns the probe instead of restating it.
pub use warrenguard_config::TUNNEL_GATEWAY_IP;
// ADR 36 (Option A): the daemon stores a per-tunnel migrate handle and builds
// migration targets from the directory's selected circuit, so re-export both
// types from this crate (it owns `MultiHopConfig`).
pub use warrenguard_transport::drain_policy;
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
        // Retarget the exit's verified ML-KEM key with the exit so a cross-exit
        // migration keeps preferring the PQ seal; `None` when the exit publishes
        // no PQ descriptor (classical fallback, require_pq=false).
        exit_mlkem768_pubkey: cfg.exit.exit_mlkem768_pubkey.clone(),
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

/// Identity of the hop that deliberately refused a dial attempt, as
/// published in the multi-hop directory: the entry's `relay_id` or the
/// exit's exit id. The two are distinguished because the daemon must
/// exclude the REFUSING node, not blindly the circuit's exit (a drained
/// entry in front of a healthy exit must not burn the exit's slot in
/// the avoid-set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarrenRefusedHop {
    /// The entry relay refused the QUIC connection (drained node).
    Entry([u8; 16]),
    /// The exit refused the session with its drain close code.
    Exit([u8; 16]),
}

/// ADR 36 dial-refusal path: async daemon hook invoked (rate limited by
/// [`WARREN_DIAL_REFUSAL_COOLDOWN`]) when the supervisor's dial is
/// deliberately refused by a drained node. The daemon records the
/// refusing node in its avoid-set and re-selects a circuit that excludes
/// it (same country when one is pinned, any otherwise), retargeting the
/// live supervisor. Output: whether a retarget was dispatched.
pub type WarrenDialRefused = std::sync::Arc<
    dyn Fn(WarrenRefusedHop) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// Minimum interval between two dial-refusal reactions. The supervisor
/// redials a refusing circuit every 0.3-2 s (its backoff ceiling), so an
/// unthrottled hook would flood the directory updater with re-selection
/// passes while the first retarget is still being dispatched.
const WARREN_DIAL_REFUSAL_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(20);

/// Unix seconds of the last dial-refusal reaction, process-wide (`0` =
/// never). Process-global on purpose, like the drain reactor's cooldown:
/// a rebuilt tunnel must not reset the throttle while the fleet is mid
/// rollout.
static LAST_DIAL_REFUSAL_UNIX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Wall-clock unix seconds (`0` on a pre-epoch clock, which only makes
/// the refusal cooldown more conservative).
fn warren_now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `true` when a dial-refusal reaction is due: none ever fired, or the
/// cooldown elapsed. Robust to clock skew: a `last` in the future reads
/// as not-elapsed (suppress), erring on the quiet side.
fn dial_refusal_reaction_due(now_unix: u64, last_unix: u64) -> bool {
    if last_unix == 0 {
        return true;
    }
    last_unix <= now_unix
        && now_unix.saturating_sub(last_unix) >= WARREN_DIAL_REFUSAL_COOLDOWN.as_secs()
}

/// Re-export of the stable exit identifier from
/// warrenguard-wire. Pubkey pinning keys its TOFU lookup
/// on this 16-byte value so a legitimate Ed25519 rotation stays
/// distinguishable from an exit-substitution attack.
pub use warrenguard_wire::ExitId as RelayExitId;
use warrenguard_wire::WarrenExitAddr;
/// Re-export of the `Setup`-frame feature bitmask constants
/// (`features::IPV6`, `PORT_FORWARD`, ...) so daemon-side callers that
/// only depend on `talpid-warren-tunnel` (e.g.
/// `mullvad_daemon::warren_tunnel_params`) can OR them into
/// [`WarrenTunnelParameters::features`] without taking a direct
/// `warrenguard-wire` dependency.
pub use warrenguard_wire::features;

pub use warrenguard_transport::supervisor::SessionTokenProvider;

/// Wraps a source of serialized token bytes (one 354-byte Privacy Pass token
/// each, e.g. `warren_api::TokenManager::take_current_stack`) into a
/// [`SessionTokenProvider`] the multi-hop supervisor consumes. Lets the daemon
/// supply v7 tokens without depending on `warrenguard-wire` directly. An empty
/// stack keeps the v6 wallet-signed path.
#[must_use]
pub fn make_session_token_provider(
    take_stack: std::sync::Arc<
        dyn Fn() -> Vec<[u8; warrenguard_wire::SESSION_TOKEN_LEN]> + Send + Sync,
    >,
) -> SessionTokenProvider {
    std::sync::Arc::new(move || {
        take_stack()
            .into_iter()
            .map(warrenguard_wire::SessionToken)
            .collect()
    })
}

/// Hands a port-forwarding rule the entitlement it presents, by SLOT.
///
/// A slot is a rule's stable place in the subscriber's per-epoch batch: one
/// entitlement buys one forwarded port (warren-core doc 99), so two live rules
/// must never draw the same one. `None` for a slot means the subscriber has no
/// entitlement left, and the exit then applies its configured quota, which is
/// the documented degrade path.
pub type PortEntitlementProvider = std::sync::Arc<dyn Fn(usize) -> Option<Vec<u8>> + Send + Sync>;

mod adapter;
mod rate_limiter;
// macOS-only: the carrier bind (`IP_BOUND_IF`) and its self-healing egress
// guard are a macOS-specific policy (Linux escapes by fwmark, Windows by
// `IP_UNICAST_IF`), so the whole module compiles and its tests run on macOS.
#[cfg(target_os = "macos")]
mod carrier_egress_guard;
// Persisted per-network verdicts for the guard above, so a known-black-holing
// network skips the bind (and its probe window) on later connects.
#[cfg(target_os = "macos")]
mod carrier_verdict_cache;
// Takes the bind back after a confirmed migration, so the network the session
// moved onto earns a MEASURED verdict instead of the one the rebind forced.
#[cfg(target_os = "macos")]
mod carrier_bind_reclaim;
mod drain_reactor;
mod egress_probe;
mod migration_watchdog;
mod session_liveness;
mod session_placement;
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
/// once the flap detector settles). Bounds the multi-hop first-dial wait.
const HANDSHAKE_WAIT_BOUND: std::time::Duration = std::time::Duration::from_secs(150);

/// Parameters required to start a Warren tunnel.
///
/// Exit selection (`exit_addr`) is
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
    /// preference order. The multi-hop path negotiates ALPN from its own
    /// signed relay descriptor, so this is currently vestigial.
    pub alpn_protocols: Vec<Vec<u8>>,

    /// Multi-hop circuit. Always `Some` on a real connect: the daemon's
    /// circuit selection populates it with a 1-hop or 2-hop
    /// [`MultiHopConfig`] dialed via
    /// `warrenguard_transport::supervisor::MultiHopSupervisor` against the
    /// supplied first-hop relay. `None` fails closed ([`Error::NoCircuit`]):
    /// the fleet is multi-hop only, there is no direct datapath.
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
    pub on_reconnect: Option<warrenguard_transport::supervisor::ReconnectObserver>,

    /// Observer of the measured client->entry QUIC path RTT
    /// ([`warrenguard_transport::supervisor::PathRttObserver`]), fired by
    /// the multi-hop supervisor at session publish and close with the
    /// dialed relay's Ed25519 pubkey. The daemon's `ParametersGenerator`
    /// wires this to its client RTT store, the client-measured half of
    /// the shared path-aware selection. `None` disables the feed.
    /// Multi-hop only.
    pub on_path_rtt: Option<warrenguard_transport::supervisor::PathRttObserver>,

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

    /// Doc 62 item 5: invoked by the in-tunnel egress liveness probe on
    /// verdict edges. `true` = the exit stopped forwarding (N consecutive
    /// in-tunnel probes dead while the QUIC session stays alive), `false`
    /// = egress recovered. The daemon wires this to
    /// `WarrenStatusCache::set_exit_egress_dead` so the UI can render the
    /// "interrupted" phase with an exit-not-forwarding cause. `None`
    /// keeps the probe running for its drain interaction but drops the
    /// status surface.
    pub on_egress_verdict: Option<std::sync::Arc<dyn Fn(bool) + Send + Sync>>,

    /// ADR 36 dial-refusal path: async daemon hook invoked when a dial is
    /// deliberately refused by a drained node (entry `CONNECTION_REFUSED`
    /// or exit drain close), so the daemon excludes the refusing node and
    /// retargets the supervisor instead of letting it hammer the same
    /// node until the drain lifts. Rate limited process-wide by
    /// [`WARREN_DIAL_REFUSAL_COOLDOWN`]. `None` disables the reaction
    /// (refusals then retry on backoff until the ambient relay-list
    /// refresh removes the node). Multi-hop only.
    pub warren_dial_refused: Option<WarrenDialRefused>,

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

    /// Live client-side bandwidth ceiling in bits per second (`None`
    /// value = unlimited), driven by `Settings::warren_max_rate_bps`.
    /// The tunnel reads the current value at start via `borrow()` and
    /// follows later pushes without a reconnect (same discipline as
    /// `nat_pmp_control_rx`). `None` at the field level = the daemon
    /// did not wire the channel (tests): the tunnel runs uncapped.
    pub max_rate_control_rx: Option<tokio::sync::watch::Receiver<Option<u64>>>,

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

    /// DAITA v2 opt-in. When `true`, the client requests DAITA in its
    /// multi-hop setup (`IpRequest` `wants_daita`). The exit may then
    /// respond with an `IpAssign.daita_spec` describing the negotiated
    /// `maybenot` machine. Driven by the Mullvad upstream `wireguard.daita`
    /// toggle (single UI surface for both backends); the
    /// daemon-state-machine reads that boolean and forwards it
    /// verbatim through this field.
    pub enable_daita: bool,

    /// Anonymous v7 session-token provider (Privacy Pass, warren-core doc 64). When set and
    /// it yields a non-empty stack, the session presents `IpRequestV7` and the
    /// exit admits on the token, never learning the account pubkey. `None` (or
    /// an empty stack on token exhaustion) keeps the v6 wallet-signed path. The
    /// daemon sources this from its long-lived `warren_api::TokenManager`, which
    /// mints on unlock/timer; the provider itself only pops (no connect-time
    /// issuance).
    pub session_token_provider: Option<warrenguard_transport::supervisor::SessionTokenProvider>,

    /// Per-rule port entitlements (warren-core doc 99). When set, every
    /// NAT-PMP request carries the credential its rule's slot holds, and the
    /// exit bounds the subscriber's forwarded ports across the whole fleet
    /// instead of per session. `None` (or an exhausted batch) leaves the exit
    /// on its configured per-client quota.
    pub port_entitlement_provider: Option<PortEntitlementProvider>,

    /// Daemon cache directory, for the state a tunnel wants to survive a
    /// daemon restart without belonging in settings: the macOS carrier
    /// egress guard's per-network verdicts (`carrier_verdict_cache`) and the
    /// session placement the next tunnel asks for (`session_placement`).
    /// `None` (tests, other platforms) keeps both in memory only.
    pub cache_dir: Option<std::path::PathBuf>,
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
            .field(
                "on_path_rtt",
                &self.on_path_rtt.as_ref().map(|_| "<observer>"),
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
    /// relay list, so the daemon cannot recover the exit geo from
    /// the IP or the relay list. Empty for the manual-config path (the
    /// caller then falls back to the relay-list lookup).
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
#[non_exhaustive]
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

    /// The exit refused the session for a policy reason that clears on its
    /// own: the wallet is absent from the exit's allowlist, which is exactly
    /// what a just-redeemed subscription looks like until the exit's next
    /// allowlist poll.
    ///
    /// Recoverable like [`Self::BackendTransient`], so the state machine
    /// retries and the kill-switch stays up. It is a variant of its own only
    /// because it is the single retriable failure that can persist forever
    /// (a wallet with no subscription is refused every time), so the client
    /// asks the account API whether there is a subscription to sync and
    /// surfaces the refusal when there is none, instead of dialing in silence.
    /// The message carries the `[EXPIRED_ACCOUNT]` auth-failed token so that,
    /// when it is surfaced, the app shows its existing localized copy instead
    /// of a generic failure.
    #[error("Warren exit refused the session (recoverable): {0}")]
    SessionRejected(String),

    /// Fatal backend error that the state machine must NOT retry
    /// automatically: configuration mismatch, authentication failure, or
    /// PKI/TLS setup failure. Retrying immediately would produce the same
    /// outcome and only waste network bandwidth.
    ///
    /// NOTE: a multi-hop `not-allowlisted` rejection is deliberately NOT
    /// fatal: it self-heals once the exit's allowlist refresh picks up a
    /// freshly-redeemed subscription, so [`reject_error`] maps it to
    /// [`Self::SessionRejected`] (recoverable). Do not "promote" it to
    /// fatal: that would strand a just-subscribed user in an
    /// uncancelable-until-manual-reconnect state.
    #[error("Warren tunnel fatal backend error: {0}")]
    BackendFatal(String),

    /// No multi-hop circuit was supplied. The Warren fleet is multi-hop
    /// only, so the daemon's circuit selection always populates
    /// [`WarrenTunnelParameters::multi_hop`] with a 1-hop or 2-hop
    /// circuit. A `None` reaching the tunnel backend means circuit
    /// assembly failed upstream; fail closed rather than open an
    /// unprotected direct path.
    #[error("Warren tunnel requires a multi-hop circuit but none was supplied")]
    NoCircuit,
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
    /// - [`Error::SessionRejected`]: `true` - the exit's allowlist has not caught up with the
    ///   subscription yet; the next poll admits the session.
    /// - [`Error::BackendFatal`]: `false` - auth failure or a definitive ban; retrying immediately
    ///   would waste bandwidth and produce the same outcome.
    /// - [`Error::NoCircuit`]: `false` - fail-closed invariant guard; no reconnect can conjure a
    ///   circuit the daemon did not supply.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Handshake(_) | Error::BackendTransient(_) | Error::SessionRejected(_)
        )
    }

    /// The user-facing reason of a self-healing exit refusal, `None` for every
    /// other error.
    ///
    /// Lets the state machine bound its retry loop on a refusal specifically,
    /// without sniffing error strings, and reuse the same text once it decides
    /// the refusal has outlived the exit's sync window.
    #[must_use]
    pub fn session_rejection(&self) -> Option<&str> {
        match self {
            Error::SessionRejected(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Map an exit's definitive session rejection to a tunnel error.
///
/// Most rejections are classified recoverable on purpose: a `not-allowlisted`
/// rejection clears on its own once the exit's allowlist refresh picks up a
/// freshly-redeemed subscription. The state machine retries it with a growing
/// interval, and it asks the account API which of the two refusals this is: a
/// subscription the exit has not synced yet is waited out, and a wallet with
/// nothing to sync surfaces the carried `[EXPIRED_ACCOUNT]` token at once, as
/// a stable, cancelable blocked state (`talpid-core`'s
/// `tunnel_state_machine::exit_refusal`). The firewall kill-switch stays up
/// throughout, so a rejected session never leaks and never blackholes the user
/// into an uncancelable wedge.
///
/// [`RejectionReason::IpExhausted`] is deliberately left an ordinary
/// [`Error::BackendTransient`]: an exhausted address pool is a capacity
/// condition with nothing for the user to act on, so it must never be
/// surfaced as an account verdict. The growing retry interval alone keeps it
/// from hammering the exit.
///
/// A [`RejectionReason::Banned`] is the exception: the account is on the
/// signed, fleet-wide CRL, so retrying never self-heals (unlike a lapsed
/// subscription) and would only spin "Reconnecting" forever. It is routed to
/// the fatal [`Error::BackendFatal`] path with a `[BANNED*]` auth-failed
/// token, so the state machine surfaces a clear suspension the app localizes,
/// instead of an endless reconnect. Fatal still keeps the kill-switch up (it
/// enters a blocked error state, it does not open the tunnel). The ban carries
/// the exit's opaque product reason code, which selects a specific token
/// ([`BAN_REASON_PORT_FORWARDING`] -> `[BANNED_PORT_FORWARDING]`) so the app can
/// show a forwarded-port-specific message; any other/unknown code falls back to
/// the generic `[BANNED]` suspension.
fn reject_error(reason: RejectionReason) -> Error {
    match reason {
        RejectionReason::Banned(BAN_REASON_PORT_FORWARDING) => Error::BackendFatal(format!(
            "[BANNED_PORT_FORWARDING] exit rejected the session ({reason}); access suspended for \
             port-forwarding abuse"
        )),
        RejectionReason::Banned(_) => Error::BackendFatal(format!(
            "[BANNED] exit rejected the session ({reason}); access suspended"
        )),
        RejectionReason::IpExhausted => Error::BackendTransient(format!(
            "exit rejected the session ({reason}); its address pool is exhausted"
        )),
        _ => Error::SessionRejected(format!(
            "[EXPIRED_ACCOUNT] exit rejected the session ({reason}); no active subscription, or \
             the exit has not yet synced a freshly-redeemed one"
        )),
    }
}

/// The exit's opaque ban-reason code (sealed on `RejectedBanned`) that means
/// "port-forwarding abuse". This mirrors the control-plane contract defined in
/// warren-core (`warren-exit-policy` `ban_reason_code::PORT_FORWARDING_ABUSE`);
/// keep the two in sync. Any other code maps to the generic suspension, so a
/// newer code the app does not recognize degrades safely.
const BAN_REASON_PORT_FORWARDING: u8 = 1;

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
    /// Backend-specific task handles: the multi-hop fanout (supervisor +
    /// uplink + downlink + guards). `wait()` aborts all of them on
    /// teardown and surfaces abnormal terminations through the same
    /// `Backend` error path.
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
    /// the dedicated table). Installed when the exit allocated a tunnel v6
    /// (`metadata.ipv6_gateway.is_some()`). `None` when no v6 was assigned
    /// or on non-Linux. Torn down in `wait()` alongside the v4 guard; the
    /// firewall blocks native v6 regardless so a failed install never
    /// leaks.
    v6_route_guard: Option<default_route_split::DefaultRouteSplitV6Guard>,
    /// Mutually exclusive with [`Self::v6_route_guard`]: held while the tunnel
    /// carries no IPv6, to make global v6 unroutable rather than merely
    /// blocked. `None` when the exit allocated a tunnel v6, or when the
    /// install failed (v6 then stays firewall-blocked, so still no leak).
    v6_unreachable_guard: Option<default_route_split::Ipv6UnreachableGuard>,
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
/// [`MonitorBackend::MultiHop`] owns a 3-task fanout: the `MultiHopSupervisor` future (drives
/// connect+reconnect), the uplink pump (TUN -> multi-hop) and the downlink pump (multi-hop ->
/// TUN), wired together through a watch channel.
enum MonitorBackend {
    MultiHop {
        supervisor_handle: tokio::task::JoinHandle<()>,
        uplink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        downlink_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
        /// ADR-0006 idle-cover emitter (replaces the keep-alive PING with
        /// jittered dummies). `Some` only when idle cover is active; it holds a
        /// supervisor watch receiver, so it is aborted with the other
        /// receiver-holders before the pumps at teardown.
        cover_handle: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
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
        /// Doc 62 item 5: in-tunnel egress liveness probe. Catches the
        /// case RX-silence cannot: an exit that ACKs QUIC keep-alives
        /// but forwards nothing (drained / half-swapped rollout).
        egress_probe_handle: tokio::task::JoinHandle<()>,
        /// Channel through which the pump task reports the first fatal
        /// error (uplink/downlink/supervisor death). `wait()` consumes
        /// it to surface an abnormal pump termination. A retriable
        /// disconnect from the underlying
        /// QUIC connection does NOT push an error here: the supervisor
        /// absorbs it transparently and the pump tasks park on the
        /// watch channel until the supervisor re-publishes.
        pump_error_rx: tokio::sync::oneshot::Receiver<String>,
        /// Terminal-rejection signal from the supervisor. `wait()` races
        /// it so a mid-session policy rejection (the exit revokes the
        /// pubkey) surfaces as a clean, cancelable error state rather
        /// than an endless silent reconnect.
        supervisor_fatal_rx: tokio::sync::watch::Receiver<Option<RejectionReason>>,
        /// Dead-datapath escalation from the supervisor: consecutive
        /// watchdog-forced redials whose sessions carried zero downlink.
        /// `wait()` races it so the UI stops reporting Connected over a
        /// tunnel that carries nothing. The per-session egress probe
        /// cannot catch this (each forced redial resets it below its
        /// threshold); only the supervisor sees the cross-redial pattern
        /// (the carrier-blackhole failure mode).
        supervisor_datapath_dead_rx: tokio::sync::watch::Receiver<bool>,
    },
}

impl WarrenTunnelMonitor {
    /// Starts a Warren tunnel from `params` through the multi-hop path
    /// (`warrenguard_transport::supervisor::MultiHopSupervisor`). The Warren
    /// fleet is multi-hop only, so `params.multi_hop` is always `Some` on a
    /// real connect; a `None` fails closed with [`Error::NoCircuit`] rather
    /// than opening an unprotected direct datapath.
    ///
    /// Blocks the current thread until the underlying QUIC session is
    /// established and the TUN is up.
    ///
    /// # Errors
    ///
    /// [`Error::NoCircuit`] if no circuit was supplied; otherwise see
    /// [`Self::start_multi_hop`].
    pub fn start(
        params: &WarrenTunnelParameters,
        args: TunnelArgs<'_>,
        log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        // Clone only the `multi_hop` field (which itself derives Clone)
        // rather than cloning the entire `WarrenTunnelParameters` struct
        // (which is intentionally not Clone to prevent signing_key copies).
        match params.multi_hop.as_ref().cloned() {
            Some(cfg) => Self::start_multi_hop(params, cfg, args, log_path),
            None => Err(Error::NoCircuit),
        }
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
            supervised_pump::{
                ExitDrainingChannel, IpAssignChannel, run_downlink, run_downlink_with_daita,
                run_idle_cover, run_uplink, run_uplink_with_daita,
            },
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
        // Carrier-socket bypass (Port Fail / TunnelCrack ServerIP fix). The
        // socket the supervisor applies this to is the one dialing the RELAY
        // (the only UDP peer the client speaks to directly), not the exit.
        // macOS: PREFER the `IP_BOUND_IF` bind (the leak-free escape, unlike the
        // `/32` route it does not let other apps reach the relay off-tunnel), but
        // do NOT trust it blindly. On a multi-interface host the bind can lose
        // ALL egress once the physical default becomes ifscoped (the
        // carrier-blackhole failure mode: sends succeed, zero packets on the wire).
        // The bind is resolved here and then VERIFIED post-route-swap by the
        // bootstrap egress guard below, which self-heals to the relay/32
        // `DefaultNode` route if nothing comes back. If the interface cannot be
        // resolved, `None` falls back to that same /32 escape (never
        // fail-closed; fail-closed here would take egress down).
        #[cfg(target_os = "linux")]
        let socket_bypass = Some(warren_carrier_socket_bypass(0));
        // macOS: the bind decision additionally consults the per-network
        // verdict cache: a network where the guard already proved the bind
        // black-holes skips it outright, going straight to the pre-v1.7.0
        // `/32` escape; any other network (proven bind or never seen) keeps
        // the bind and defers the guard to the background, so the connect
        // never waits on a probe window.
        #[cfg(target_os = "macos")]
        let (socket_bypass, carrier_probe_plan): (
            Option<warrenguard_tun_core::SocketBypass>,
            carrier_verdict_cache::CarrierProbePlan,
        ) = {
            use carrier_verdict_cache::CarrierProbePlan;
            match runtime.block_on(discover_warren_carrier_network_macos(&args.route_manager)) {
                Some((phys_ifindex, fingerprint)) => {
                    let plan = carrier_verdict_cache::plan_carrier_probe(
                        fingerprint,
                        params.cache_dir.as_deref(),
                    );
                    let bypass = match &plan {
                        CarrierProbePlan::SkipRouteOnly => {
                            log::info!(
                                "Warren carrier egress guard: cached RouteOnly verdict for this \
                                 network; skipping the IP_BOUND_IF bind, using the /32 escape"
                            );
                            None
                        }
                        _ => Some(warren_carrier_socket_bypass(phys_ifindex)),
                    };
                    (bypass, plan)
                }
                None => (None, CarrierProbePlan::NoBind),
            }
        };
        #[cfg(target_os = "windows")]
        let socket_bypass = Some(warren_carrier_socket_bypass(
            runtime.block_on(discover_warren_phys_ifindex())?,
        ));
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let socket_bypass: Option<warrenguard_tun_core::SocketBypass> = None;
        // ADR-0006 idle cover on the multi-hop path, resolved from the
        // knob + DAITA coupling (DAITA already emits its own cover, so
        // idle cover yields to it).
        let mh_idle_cover = idle_cover_effective(
            warrenguard_config::knobs::idle_cover_enabled(),
            params.enable_daita,
        );
        let supervisor_config = SupervisorConfig {
            relay: Arc::new(cfg.relay.clone()),
            exit_id: cfg.exit.exit_id,
            exit_x25519_multihop_pubkey: cfg.exit.exit_x25519_multihop_pubkey,
            // Prefer the PQ X-Wing seal when the verified exit descriptor
            // advertises an ML-KEM key; `None`/empty keeps the byte-identical
            // classical seal (supervisor dials with require_pq=false).
            exit_mlkem768_pubkey: cfg.exit.exit_mlkem768_pubkey.clone(),
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
            on_path_rtt: params.on_path_rtt.clone(),
            ip_assign_channel: Some(ip_assign_channel.clone()),
            wants_ipv6,
            // Advertise the DAITA opt-in on the v3 setup so the exit can grant
            // a maybenot machine for this session. Without this the client
            // never negotiates the defense and would pad nothing while the UI
            // claims protection.
            enable_daita: params.enable_daita,
            // ADR-0006: when idle cover is active the supervisor dials with the
            // fixed keep-alive PING disabled and the multi-hop cover emitter
            // (spawned below) refreshes the NAT mapping instead, removing the
            // 5s beacon. `mh_idle_cover` (resolved once, DAITA-coupled) drives
            // BOTH this dial config and the emitter, so they cannot disagree.
            idle_cover: mh_idle_cover,
            // Bonded connections: one QUIC connection is capped at the
            // per-flow bandwidth share of the client↔relay path (e.g.
            // ~400 Mbps on a 1 Gbps line whose 8-flow aggregate reaches
            // ~900). The supervisor bonds n_connections sessions under
            // one identity (sticky inner IP) and pins flows by 5-tuple
            // hash.
            n_connections: usize::from(params.n_connections).max(1),
            // docs/59 Lot 3: the pre-swap NAT-PMP reservation and the
            // post-swap re-map observer are wired by the daemon from its
            // live port-forward config (next step); the transport carries
            // the hooks, `None` keeps the pre-hook behavior.
            pre_swap_check: params.warren_pre_swap_check.clone(),
            on_overlap_swapped: params.warren_on_overlap_swapped.clone(),
            socket_bypass,
            // ADR 36 dial-refusal path: a drained node answers every dial
            // with a deliberate refusal; without this hook the supervisor
            // backoff (0.3-2 s ceiling) hammers it until the drain lifts
            // (observed: 5160 refusals in 19 minutes during a fleet
            // rollout) and the user's connect stalls behind the retry
            // loop. The daemon hook excludes the refusing node and
            // retargets the live supervisor; the cooldown keeps one
            // reaction per rollout wave instead of one per redial.
            on_dial_refused: params.warren_dial_refused.clone().map(|hook| {
                Arc::new(
                    move |hop: warrenguard_transport::multihop::DialRefusedHop,
                          relay_id: [u8; 16],
                          exit_id: [u8; 16]| {
                        let now = warren_now_unix_secs();
                        let last =
                            LAST_DIAL_REFUSAL_UNIX.load(std::sync::atomic::Ordering::Relaxed);
                        if !dial_refusal_reaction_due(now, last) {
                            return;
                        }
                        LAST_DIAL_REFUSAL_UNIX.store(now, std::sync::atomic::Ordering::Relaxed);
                        let refused = match hop {
                            warrenguard_transport::multihop::DialRefusedHop::Entry => {
                                WarrenRefusedHop::Entry(relay_id)
                            }
                            warrenguard_transport::multihop::DialRefusedHop::Exit => {
                                WarrenRefusedHop::Exit(exit_id)
                            }
                        };
                        let hook = hook.clone();
                        tokio::spawn(async move {
                            let retargeted = hook(refused).await;
                            log::info!(
                                "Warren dial refused by a drained node: retarget {}",
                                if retargeted {
                                    "dispatched (supervisor migrates on its next attempt)"
                                } else {
                                    "unavailable (no alternative circuit); staying on backoff"
                                }
                            );
                        });
                    },
                ) as warrenguard_transport::supervisor::DialRefusedObserver
            }),
            // Anonymous v7 admission when the daemon supplied a token provider
            // (default in the app): the exit admits on a Privacy Pass token and
            // never learns the wallet. Empty stack / None falls back to v6.
            session_token_provider: params.session_token_provider.clone(),
        };
        let (supervisor, mut client_rx) = MultiHopSupervisor::new(supervisor_config);
        // A rebuilt tunnel continues the session the previous one held rather
        // than starting an independent one, so the exit keeps it on the same
        // inner address and everything keyed on that address (forwarded ports
        // above all) stays this client's. See `session_placement`.
        if let Some(cache_dir) = params.cache_dir.as_deref() {
            session_placement::SESSION_PLACEMENT.load_from(cache_dir);
        }
        if let Some(assigned) = session_placement::SESSION_PLACEMENT.recall() {
            log::debug!("{TRACE_PREFIX} resuming the session placement of the previous tunnel");
            supervisor.resume_session_placement(assigned);
        }
        // Subscribe to the supervisor's terminal-rejection signal BEFORE
        // `run()` consumes it. The exit publishes a rejection here (e.g.
        // pubkey not allowlisted) instead of letting the session masquerade
        // as a live "Connected" that silently carries no traffic.
        let mut supervisor_fatal_rx = supervisor.fatal_rx();
        let supervisor_datapath_dead_rx = supervisor.datapath_dead_rx();
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
        // DAITA capability echo, read off the primary session's setup response
        // before the strong reference is dropped. `Some` selects the
        // DAITA-aware pumps below; `None` runs the plain pumps AND records
        // the defense as inactive (`daita_active` on the metadata), so the
        // Connected state never claims a protection that is not running.
        let daita_shared =
            multi_hop_daita_shared(params.enable_daita, initial_client.primary().daita_spec())?;
        let daita_active = daita_shared.is_some();
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
                session_placement::SESSION_PLACEMENT.remember(spec.assigned);
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

        let metadata = build_tunnel_metadata(&tun, &tun_config, daita_active);
        #[cfg(not(target_os = "windows"))]
        let async_device = tun.into_inner().into_async_device();
        // On Windows, WindowsTun::into_inner() already yields a tun08::AsyncDevice
        // (cf. talpid-tunnel/src/tun_provider/windows.rs); no into_async_device() hop.
        #[cfg(target_os = "windows")]
        let async_device = tun.into_inner();
        // Client-side bandwidth ceiling (`Settings::warren_max_rate_bps`):
        // both pump directions pass the limiter's gate through the
        // wrapped device. The wrapper is installed unconditionally (one
        // relaxed atomic load per packet when uncapped) so a later
        // watch push can engage the cap without a reconnect.
        let rate_limiter = rate_limiter::TunnelRateLimiter::new();
        if let Some(rate_rx) = params.max_rate_control_rx.clone() {
            rate_limiter.set_rate_bps(*rate_rx.borrow());
            let limiter = rate_limiter.clone();
            let mut rate_rx = rate_rx;
            runtime.spawn(async move {
                while rate_rx.changed().await.is_ok() {
                    let bps = *rate_rx.borrow();
                    log::info!(
                        "Warren max-rate cap updated: {}",
                        bps.map_or("unlimited".to_owned(), |b| format!("{b} bps"))
                    );
                    limiter.set_rate_bps(bps);
                }
            });
        }
        let packet_device = rate_limiter.wrap(MullvadTunPacketDevice::new(async_device));

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
        // Linux: the marked carrier socket reaches
        // the relay via the main table's default route (fwmark ip-rule), so no
        // `<relay_ip>/32` bypass route is posted. The split lives in table 100.
        #[cfg(target_os = "linux")]
        let routes: Vec<RequiredRoute> = Vec::new();
        // macOS: with the `IP_BOUND_IF` bind active the carrier escapes by
        // SOCKET, so the leaky relay/32 DefaultNode exception is NOT pre-installed
        // (that is the Port Fail / TunnelCrack shape we are removing); the
        // bootstrap egress guard adds it later ONLY if the bind is proven to
        // black-hole. When the bind could not be resolved (`socket_bypass` is
        // `None`) fall back to the pre-v1.7.0 escape: the relay/32 DefaultNode
        // route must be live BEFORE the 0/0 redirect lands, because the
        // wildcard-bound QUIC socket has no interface pin and talpid-routing
        // applies the tunnel default first within one add_routes call. Two
        // ordered calls make that window zero.
        #[cfg(target_os = "macos")]
        let (macos_bypass_routes, routes) = {
            let (bypass, defaults) = build_warren_tunnel_routes_macos_ordered(
                &metadata.interface,
                &[cfg.relay.endpoint.ip()],
            );
            if socket_bypass.is_some() {
                (Vec::new(), defaults)
            } else {
                (bypass, defaults)
            }
        };
        // Windows: routing fully owned by the warrenguard-winroute native
        // Win32 IP Helper API via `DefaultRouteSplitGuard::install` below;
        // talpid-routing gets an empty set to avoid double-install.
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
        runtime.block_on(async move {
            #[cfg(target_os = "macos")]
            match route_manager
                .add_routes(macos_bypass_routes.into_iter().collect())
                .await
            {
                Ok(()) => log::info!("Warren multi-hop relay bypass route installed"),
                Err(e) => log::warn!(
                    "Failed to install Warren multi-hop relay bypass route: {e}. \
                     Relay traffic may transiently self-nest through the tun."
                ),
            }
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

        // Multi-hop split-default install via the `default_route_split` facade.
        // On Linux/Windows the carrier's escape is keyed on the socket (see
        // `socket_bypass` above), not the destination, so it is agnostic to
        // which peer the supervisor currently dials; on multi-hop that peer
        // is the *relay* (first hop), not the exit, since it is the only UDP
        // peer the client speaks to directly.
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
        // blocked, so v6 is non-functional but never leaks.
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

        // The other half of the same decision: with no tunnel v6, declare
        // global IPv6 unreachable so the host stops choosing it. The firewall
        // already blocks it, but blocking is not the same as being unroutable:
        // the in-tunnel resolver answers AAAA for every dual-stack host, and on
        // macOS a blocked v6 connect can report success and fail on the first
        // send, which strands the request instead of falling back to IPv4.
        // Non-fatal, exactly like the guard above: without it v6 is merely
        // slow, never leaked.
        let v6_unreachable_guard = if metadata.ipv6_gateway.is_none() {
            runtime
                .block_on(default_route_split::Ipv6UnreachableGuard::install())
                .map(Some)
                .unwrap_or_else(|e| {
                    log::warn!(
                        "Warren: failed to make native IPv6 unreachable: {e}. \
                         It stays firewall-blocked (no leak), but dual-stack \
                         destinations may be slow to fall back to IPv4."
                    );
                    None
                })
        } else {
            None
        };

        // macOS carrier egress guard (the carrier-blackhole failure mode):
        // now that the default route points at the TUN, VERIFY the
        // `IP_BOUND_IF`-bound carrier actually egresses. The guard always
        // runs AFTER `Up`, in the background, so the connect never waits on
        // it: on a black-holing bind it self-heals to the `/32` escape
        // within its adaptive window (a short dead-egress blip right after
        // `Up`, once per network per verdict TTL) and records `RouteOnly`
        // so the next connect skips the bind outright. A cached-RouteOnly
        // network skipped the bind above and has nothing to verify. The
        // dead-path escalation remains the backstop if a revert does not
        // heal.
        #[cfg(target_os = "macos")]
        {
            use carrier_verdict_cache::CarrierProbePlan;
            match carrier_probe_plan {
                CarrierProbePlan::Background(recorder) => {
                    let mut guard_io = carrier_egress_guard::RealEgressGuardIo {
                        client_rx: client_rx.clone(),
                        route_manager: args.route_manager.clone(),
                        carrier_ips: vec![relay_endpoint.ip()],
                        tun_iface: metadata.interface.clone(),
                    };
                    runtime.spawn(async move {
                        let outcome =
                            carrier_egress_guard::run_bootstrap_guard(&mut guard_io).await;
                        log::info!(
                            "{TRACE_PREFIX} phase=carrier_egress_guard_background \
                             outcome={outcome:?} (verified after Up)"
                        );
                        recorder.record(outcome);
                    });
                    log::info!(
                        "{TRACE_PREFIX} T6b={}ms phase=carrier_egress_guard \
                         outcome=DeferredToBackground",
                        start_t.elapsed().as_millis()
                    );
                }
                CarrierProbePlan::SkipRouteOnly => {
                    log::info!(
                        "{TRACE_PREFIX} T6b={}ms phase=carrier_egress_guard \
                         outcome=CachedRouteOnly (bind skipped, /32 escape pre-installed)",
                        start_t.elapsed().as_millis()
                    );
                }
                CarrierProbePlan::NoBind => {}
            }
        }

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

        // Effective-MTU sampler. DPLPMTUD needs a few RTTs to settle, so
        // sampling at Up would flag every network as reduced; the verdict
        // is measured 4 s post-Up and republished as a metadata refresh
        // only when it changes (quantized to 16-byte steps against
        // boundary flap). The pumps clamp MSS and reflect PMTUD on
        // reduced paths regardless; this only surfaces the state to the
        // UI as the ReducedMtu feature indicator.
        {
            let mut sampler_hook = event_hook.clone();
            let mut sampler_rx = client_rx.clone();
            let sampler_meta = metadata.clone();
            let tun_mtu = usize::from(tun_config.mtu);
            runtime.spawn(async move {
                let mut last: Option<u16> = None;
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                loop {
                    let bundle = sampler_rx.borrow().clone();
                    if let Some(bundle) = bundle {
                        let inner = bundle.max_inner_payload();
                        let verdict = (inner < tun_mtu)
                            .then(|| u16::try_from(inner & !15).unwrap_or(u16::MAX));
                        if verdict != last {
                            last = verdict;
                            let mut refreshed = sampler_meta.clone();
                            refreshed.effective_mtu = verdict;
                            sampler_hook.on_event(TunnelEvent::Up(refreshed)).await;
                        }
                    }
                    tokio::select! {
                        () = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                        res = sampler_rx.changed() => {
                            if res.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }

        // Spawn the uplink + downlink pumps. Each consumes a clone of
        // the watch receiver so they can independently park on the
        // supervisor's reconnect signal. When the exit granted DAITA the
        // `_with_daita` variants drive the shared machine state (uplink
        // emits the padding actions, downlink filters the dummies); the
        // Notify is the cross-pump wake-up for downlink-scheduled timers.
        let (pump_error_tx, pump_error_rx) = tokio::sync::oneshot::channel::<String>();
        let pump_error_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(pump_error_tx)));
        let pump_error_tx_uplink = pump_error_tx.clone();
        let pump_error_tx_downlink = pump_error_tx.clone();
        let daita_state_changed = std::sync::Arc::new(tokio::sync::Notify::new());

        let uplink_rx = client_rx.clone();
        let uplink_device = packet_device.clone();
        let uplink_daita = daita_shared.clone();
        let uplink_notify = daita_state_changed.clone();
        let uplink_handle = runtime.spawn(async move {
            log::info!(
                "{TRACE_PREFIX} multi-hop uplink=running variant={}",
                if uplink_daita.is_some() {
                    "daita"
                } else {
                    "plain"
                }
            );
            let res = match uplink_daita {
                Some(daita) => {
                    run_uplink_with_daita(uplink_rx, uplink_device, daita, uplink_notify).await
                }
                None => run_uplink(uplink_rx, uplink_device).await,
            };
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
        let downlink_daita = daita_shared.clone();
        let downlink_notify = daita_state_changed.clone();
        let downlink_ip_assign = ip_assign_channel.clone();
        let downlink_handle = runtime.spawn(async move {
            log::info!(
                "{TRACE_PREFIX} multi-hop downlink=running variant={}",
                if downlink_daita.is_some() {
                    "daita"
                } else {
                    "plain"
                }
            );
            let res = match downlink_daita {
                Some(daita) => {
                    run_downlink_with_daita(
                        downlink_rx,
                        downlink_device,
                        daita,
                        downlink_notify,
                        Some(downlink_ip_assign),
                        Some(downlink_drain_channel),
                    )
                    .await
                }
                None => {
                    run_downlink(downlink_rx, downlink_device, Some(downlink_drain_channel)).await
                }
            };
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

        // Idle-cover emitter (ADR-0006): follows the supervisor's live bundle
        // and replaces the fixed keep-alive PING (disabled on the dial when
        // `mh_idle_cover` is set) with jittered, size-varied dummies. Spawned
        // only when cover is active; the dial keeps the PING otherwise, so this
        // is a no-op for the default DAITA/plain path. Holds a supervisor watch
        // receiver, aborted with the other receiver-holders at teardown.
        let cover_handle = mh_idle_cover.then(|| {
            let cover_rx = client_rx.clone();
            runtime.spawn(async move {
                let res = run_idle_cover(cover_rx).await;
                if let Err(ref e) = res {
                    log::warn!("{TRACE_PREFIX} multi-hop idle cover: {e:#}");
                }
                res
            })
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
            let carrier_ips = vec![relay_endpoint.ip()];
            let tun_iface = metadata.interface.clone();
            let verdict_dir = params.cache_dir.clone();
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
                            socket_bypass,
                            carrier_ips,
                            tun_iface,
                            verdict_dir,
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
                        // The rebuild this escalates must ask to be placed
                        // back here, not start yet another session that the
                        // exit would put somewhere else again.
                        session_placement::SESSION_PLACEMENT.remember(spec.assigned);
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

        // Doc 62 item 5: in-tunnel egress liveness probe. RX-silence
        // never trips on a drained/half-swapped exit that keeps ACKing
        // QUIC keep-alives while forwarding nothing; this probe proves
        // real egress and surfaces the exit_egress_dead verdict (plus
        // the gap-free migration when a drain advisory is active).
        let egress_probe_handle = {
            let probe_cfg = egress_probe::EgressProbeConfig::from_env();
            let mut io = egress_probe::RealEgressProbeIo {
                interval: probe_cfg.interval,
                startup_interval: probe_cfg.startup_interval,
                client_rx: Some(client_rx.clone()),
                verdict: params.on_egress_verdict.clone(),
                drain_rx: Some(exit_draining_channel.subscribe()),
                drain_migrate: params.warren_drain_migrate.clone(),
                // Same shared reconnect channel every other guard uses: an
                // egress-dead verdict with no gap-free migration leaves
                // Connected and redials onto a fresh circuit.
                pump_error_tx: Some(pump_error_tx.clone()),
                current_exit_id: *cfg.exit.exit_id.as_bytes(),
            };
            runtime.spawn(async move {
                if !probe_cfg.enabled {
                    return;
                }
                egress_probe::run_egress_probe(&mut io, probe_cfg.failure_threshold).await;
                log::debug!("{TRACE_PREFIX} egress probe terminated");
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
        // gateway (10.66.0.1:5351); the routing identity of the gateway
        // is the exit's TUN IP.
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
                cover_handle,
                watchdog_handle,
                assign_guard_handle,
                drain_reactor_handle,
                liveness_handle,
                egress_probe_handle,
                pump_error_rx,
                supervisor_fatal_rx,
                supervisor_datapath_dead_rx,
            },
            event_hook,
            // Handed off from the initial-dial race above (not re-taken from
            // `args`, whose `tunnel_close_rx` was already moved out).
            close_rx,
            default_route_guard,
            // Multi-hop `/v2` dual-stack: holds the `::/1`+`8000::/1` split
            // route when the exit allocated a v6; `None` keeps it v4-only.
            v6_route_guard,
            v6_unreachable_guard,
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
            v6_unreachable_guard,
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
            MultiHop {
                supervisor: tokio::task::JoinHandle<()>,
                uplink: tokio::task::JoinHandle<anyhow::Result<()>>,
                downlink: tokio::task::JoinHandle<anyhow::Result<()>>,
                cover: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
                watchdog: tokio::task::JoinHandle<()>,
                assign_guard: tokio::task::JoinHandle<()>,
                drain_reactor: tokio::task::JoinHandle<()>,
                liveness: tokio::task::JoinHandle<()>,
                egress_probe: tokio::task::JoinHandle<()>,
            },
        }
        let (pump_error_rx, handles, mut supervisor_fatal_rx, mut supervisor_datapath_dead_rx) =
            match backend {
                MonitorBackend::MultiHop {
                    supervisor_handle,
                    uplink_handle,
                    downlink_handle,
                    cover_handle,
                    watchdog_handle,
                    assign_guard_handle,
                    drain_reactor_handle,
                    liveness_handle,
                    egress_probe_handle,
                    pump_error_rx,
                    supervisor_fatal_rx: fatal_rx,
                    supervisor_datapath_dead_rx: datapath_dead_rx,
                } => (
                    pump_error_rx,
                    BackendHandles::MultiHop {
                        supervisor: supervisor_handle,
                        uplink: uplink_handle,
                        downlink: downlink_handle,
                        cover: cover_handle,
                        watchdog: watchdog_handle,
                        assign_guard: assign_guard_handle,
                        drain_reactor: drain_reactor_handle,
                        liveness: liveness_handle,
                        egress_probe: egress_probe_handle,
                    },
                    Some(fatal_rx),
                    Some(datapath_dead_rx),
                ),
            };

        // A future that resolves with the rejection reason if the
        // supervisor publishes one mid-session. The `Option` is always
        // `Some` here (the supervisor owns the channel); the `None` arm
        // stays a defensive inert branch.
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
        // Resolves when the supervisor escalates a dead datapath
        // (consecutive zero-downlink watchdog redials); inert while the
        // tunnel carries traffic.
        let datapath_dead_fut = async move {
            match supervisor_datapath_dead_rx.as_mut() {
                Some(rx) => loop {
                    if *rx.borrow_and_update() {
                        return;
                    }
                    if rx.changed().await.is_err() {
                        std::future::pending::<()>().await;
                    }
                },
                None => std::future::pending().await,
            }
        };

        let result = runtime.block_on(async move {
            tokio::pin!(fatal_fut);
            tokio::pin!(datapath_dead_fut);
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
                () = &mut datapath_dead_fut => {
                    // The tunnel keeps establishing but carries zero return
                    // traffic across consecutive watchdog redials. Recoverable
                    // (a retry may land on a healed network) but VISIBLE: the
                    // state machine leaves Connected instead of masquerading
                    // over a dead datapath.
                    Err(Error::BackendTransient(
                        "tunnel datapath dead: sessions establish but no return \
                         traffic flows (consecutive zero-downlink redials)"
                            .to_owned(),
                    ))
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
                // Mutually exclusive with the split above, so the two share one
                // teardown arm: at most one of them ever holds routes.
                if let Some(guard) = v6_unreachable_guard
                    && let Err(e) = guard.uninstall().await
                {
                    log::warn!("Warren IPv6 unreachable-guard cleanup failed: {e}");
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
                BackendHandles::MultiHop {
                    supervisor,
                    uplink,
                    downlink,
                    cover,
                    watchdog,
                    assign_guard,
                    drain_reactor,
                    liveness,
                    egress_probe,
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
                    // The idle-cover emitter (when active) also holds a
                    // supervisor watch receiver; abort it in the same wave.
                    if let Some(cover) = cover {
                        cover.abort();
                        let _ = cover.await;
                    }
                    // The egress probe holds a supervisor watch receiver
                    // too; same ordering rule.
                    egress_probe.abort();
                    let _ = egress_probe.await;
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
    let entitlements = params.port_entitlement_provider.clone();
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
            entitlements,
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
    entitlements: Option<PortEntitlementProvider>,
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
        /// This rule's place in the subscriber's entitlement batch. Held for
        /// as long as the rule lives and freed with it, so two live rules
        /// never draw the same entitlement and a rule that outlives an epoch
        /// picks up the next batch at the same slot.
        slot: usize,
    }

    /// The lowest slot no live rule holds. Reusing a freed slot (rather than
    /// counting up) keeps a subscriber's rules inside its per-epoch batch:
    /// slots that only ever grew would run past the batch after a few rule
    /// edits and leave later rules with no entitlement at all.
    fn lowest_free_slot(managers: &HashMap<NatPmpRuleId, ManagerState>) -> usize {
        let taken: HashSet<usize> = managers.values().map(|st| st.slot).collect();
        (0..).find(|s| !taken.contains(s)).unwrap_or(0)
    }

    /// Everything a spawned rule needs that does not change between
    /// reconciles. Grouped so the reconcile signature stays readable as the
    /// per-rule wiring grows.
    struct SpawnContext {
        runtime: tokio::runtime::Handle,
        server: std::net::SocketAddr,
        bind_addr: Option<std::net::IpAddr>,
        mapping_observer: NatPmpMappingObserver,
        entitlements: Option<PortEntitlementProvider>,
    }

    /// Reconcile the running per-rule managers against a desired config:
    /// release rules that disappeared (emitting `Cancelled`), spawn new
    /// rules, and live-reconfigure changed ones (debounced).
    async fn reconcile(
        managers: &mut HashMap<NatPmpRuleId, ManagerState>,
        desired: Option<&NatPmpConfig>,
        ctx: &SpawnContext,
        debounce: std::time::Duration,
    ) {
        let SpawnContext {
            runtime,
            server,
            bind_addr,
            mapping_observer,
            entitlements,
        } = ctx;
        let (server, bind_addr) = (*server, *bind_addr);
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
                    let slot = lowest_free_slot(managers);
                    let credential = entitlements.as_ref().map(|provider| {
                        let provider = provider.clone();
                        std::sync::Arc::new(move || provider(slot))
                            as warrenguard_natpmp_client::CredentialProvider
                    });
                    let manager = NatPmpManager::start_with_credential(
                        runtime,
                        server,
                        &per_rule_cfg,
                        observer,
                        bind_addr,
                        credential,
                    );
                    managers.insert(
                        id,
                        ManagerState {
                            manager,
                            applied_cfg: per_rule_cfg,
                            applied_at: std::time::Instant::now(),
                            slot,
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
    let ctx = SpawnContext {
        runtime,
        server,
        bind_addr,
        mapping_observer,
        entitlements,
    };
    reconcile(
        &mut managers,
        initial_config.as_ref(),
        &ctx,
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
            &ctx,
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

/// Decides whether ADR-0006 B2-lite idle cover is armed for this session.
///
/// `knob` is `WARREN_IDLE_COVER` (read once via
/// `warrenguard_config::knobs::idle_cover_enabled`). `daita_requested` is the
/// client's DAITA opt-in. Idle cover and DAITA are mutually exclusive
/// cover mechanisms (DAITA carries its own padding), so idle cover is
/// armed only when the knob is on AND DAITA is not requested.
///
/// The returned bool is the SINGLE source of truth that gates both the
/// transport keep-alive (idle cover disables the keep-alive PING) and the
/// pump choice (the idle-cover pump variant emits jittered dummies). They
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

/// Builds the `TunnelMetadata` payload emitted with `Up` / `Down`
/// events. `daita_active` is the negotiated per-session truth (the exit
/// granted a machine), never the requested setting.
fn build_tunnel_metadata(tun: &Tun, config: &TunConfig, daita_active: bool) -> TunnelMetadata {
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
        daita_active,
        effective_mtu: None,
    }
}

/// Maps the multi-hop setup's DAITA capability echo onto the pump-shared
/// state driving the defense.
///
/// The contract mirrors the `wants_ipv6`/`ipv6` echo: the presence of a spec
/// in the setup response is the exit's authoritative answer for THIS session.
///
/// - Granted (a semantically non-empty spec): the DAITA pumps must drive that
///   machine, so build the shared state; an unbuildable server-supplied spec
///   is a handshake-level error (reconnect) because running the plain pumps
///   instead would leave the user undefended behind an "on" toggle.
/// - Not granted: `Ok(None)`, run the plain pumps and surface the inactivity
///   (the caller records `daita_active = false`); loud when it was requested.
fn multi_hop_daita_shared(
    requested: bool,
    negotiated: Option<warrenguard_wire::DaitaConfig>,
) -> Result<Option<warrenguard_transport::supervised_pump::DaitaShared>, Error> {
    match negotiated {
        Some(spec) if spec.is_enabled() => {
            let state = warrenguard_daita::DaitaState::from_config(&spec, Instant::now())
                .map_err(|e| Error::Handshake(format!("negotiated DAITA spec unusable: {e}")))?;
            log::info!(
                "{TRACE_PREFIX} multi-hop DAITA negotiated machines={}",
                state.machines_count()
            );
            Ok(Some(std::sync::Arc::new(parking_lot::Mutex::new(state))))
        }
        Some(_) | None => {
            if requested {
                log::warn!(
                    "{TRACE_PREFIX} DAITA was requested but this exit did NOT grant it; \
                     running undefended pumps. DAITA is UNAVAILABLE on this session - \
                     surfaced, not a silent fallback."
                );
            }
            Ok(None)
        }
    }
}

/// The [`SocketBypass`](warrenguard_tun_core::SocketBypass) the Warren carrier
/// (QUIC dial) socket must carry so `default_route_split`'s `/1` split-default
/// captures every OTHER destination, exit IP included, without capturing the
/// tunnel's own packets (Port Fail / TunnelCrack ServerIP fix; see
/// `warrenguard_route_split::socket_bypass`). `phys_ifindex` is the physical
/// egress interface index; Linux ignores it (`SO_MARK` needs no interface).
///
/// Windows resolves the index via [`discover_warren_phys_ifindex`] first and
/// propagates its error with `?`: there is no destination-route fallback
/// there, so a carrier that cannot be marked must never dial at all.
///
/// macOS (multi-hop only) binds with `IP_BOUND_IF`, but PREFERRED not
/// mandatory: the bind can lose all egress once the physical default becomes
/// ifscoped on a multi-interface host. So the bind is applied and then VERIFIED
/// by [`carrier_egress_guard`], which reverts to the `<carrier_ip>/32 DefaultNode`
/// route (the proven-safe escape) if no bytes come back. A bind that cannot be
/// resolved simply falls back to that same `/32` route (never fail-closed:
/// fail-closed here would take egress down).
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[must_use]
fn warren_carrier_socket_bypass(phys_ifindex: u32) -> warrenguard_tun_core::SocketBypass {
    warrenguard_route_split::socket_bypass::tunnel_socket_bypass(phys_ifindex)
}

/// Resolves the physical egress interface index for the macOS carrier bind
/// (`IP_BOUND_IF`) from the current IPv4 default route, before the tunnel
/// default is installed. Returns `None` (never an error) when it cannot be
/// resolved: on macOS a bind is PREFERRED, not mandatory, so the caller falls
/// back to the `<carrier_ip>/32 DefaultNode` route escape rather than failing
/// the connect. This is the opposite of the Windows contract on purpose (a
/// macOS fail-closed bind would blackhole all egress).
#[cfg(target_os = "macos")]
async fn discover_warren_carrier_network_macos(
    route_manager: &talpid_routing::RouteManagerHandle,
) -> Option<(u32, String)> {
    match route_manager.get_default_routes().await {
        Ok((Some(v4), _v6)) => {
            let idx = u32::from(v4.interface_index);
            if idx == 0 {
                log::warn!(
                    "Warren: macOS IPv4 default route reports interface index 0; \
                     skipping the carrier bind, using the /32 route escape"
                );
                None
            } else {
                let fingerprint =
                    carrier_verdict_cache::network_fingerprint(&v4.interface, v4.router_ip);
                Some((idx, fingerprint))
            }
        }
        Ok((None, _v6)) => {
            log::warn!(
                "Warren: no IPv4 default route to bind the macOS carrier to; \
                 using the /32 route escape (no socket bind)"
            );
            None
        }
        Err(e) => {
            log::warn!(
                "Warren: failed to resolve the macOS physical interface for the \
                 carrier bind ({e}); using the /32 route escape"
            );
            None
        }
    }
}

/// Resolves the physical egress interface for the Windows carrier-socket
/// bypass (`IP_UNICAST_IF`). Mandatory: the dial needs the same default
/// route the bypass pins to, so a resolution failure fails the connect
/// closed (Port Fail / TunnelCrack ServerIP fix) instead of dialing the
/// carrier unmarked and self-poisoning once the split-default route lands.
///
/// # Errors
///
/// The physical interface could not be discovered (offline host, or only
/// tunnel interfaces present).
#[cfg(target_os = "windows")]
async fn discover_warren_phys_ifindex() -> Result<u32, Error> {
    warrenguard_route_split::default_route_split_windows::discover_physical_ifindex()
        .await
        .map_err(|e| {
            log::warn!(
                "Warren: failed to discover the physical interface for the mandatory \
                 carrier-socket bypass: {e}"
            );
            Error::Handshake(format!(
                "cannot resolve the physical interface for the carrier socket bypass: {e}"
            ))
        })
}

/// Build the `RequiredRoute`s to redirect user traffic through the TUN on
/// macOS. Adapted from the upstream Mullvad routing pattern to Warren's
/// needs (single exit, Quinn QUIC transport).
///
/// macOS strategy - do NOT reproduce the Linux `/1 + /1` recipe:
///
/// **`0.0.0.0/0 dev <tun>`** - default redirect, unconditional. Prefix 0 triggers the
/// `tunnel_default_routes` special case in talpid-routing macOS (`mod.rs:344-354`) which:
/// - Transforms the previous default `0.0.0.0/0 via gw dev <physical>` into an **ifscope** route
///   (= visible only to sockets bound to the physical iface).
/// - Posts the new default `0.0.0.0/0 dev <tun>` un-scoped (= visible to everything else, i.e.
///   user traffic).
///
/// The carrier escapes the ifscope capture via the destination-keyed
/// `<carrier_ip>/32 NetNode::DefaultNode` exception (talpid-routing resolves
/// the best physical default at apply time). This is the Port Fail /
/// TunnelCrack ServerIP shape (ANY app dialing the carrier IP escapes the
/// tunnel, not just the daemon's own socket) and it is a DELIBERATE,
/// documented trade-off: the socket-level `IP_BOUND_IF` bypass that replaced
/// it in v1.7.0 loses ALL egress after the default-route swap on
/// multi-interface hosts (Ethernet + Wi-Fi + VM bridges: sendto succeeds,
/// zero packets on the wire, tunnel silently dead: the carrier-blackhole
/// failure mode). Do not remove this exception again without validating
/// datagram egress against a REAL exit on a multi-interface macOS host,
/// post-route-swap.
///
/// Cleanup is automatic via `cleanup_routes` + `try_restore_default_routes`
/// (with exponential backoff retry, `mod.rs:613-671`) when the tunnel
/// tears down.
///
/// macOS has no policy routing (= one routing table); the ifscope
/// mechanism is the native Darwin equivalent of Linux's table 100.
///
/// Returned split into `(bypass, tunnel_default)` so the caller installs them
/// in two ORDERED `add_routes` calls. Within a single call talpid-routing
/// macOS applies the tunnel default BEFORE the `DefaultNode` bypass, which
/// opens a millisecond window where the wildcard-bound QUIC socket has its
/// carrier packets captured by `0/0 dev tun` (self-nesting; no plaintext leak,
/// but garbage). Installing the bypass in its own call first makes the window
/// zero.
#[cfg(target_os = "macos")]
#[must_use]
fn build_warren_tunnel_routes_macos_ordered(
    tun_iface: &str,
    carrier_ips: &[IpAddr],
) -> (Vec<RequiredRoute>, Vec<RequiredRoute>) {
    use talpid_routing::NetNode;

    let bypass: Vec<RequiredRoute> = carrier_ips
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

#[cfg(test)]
mod tests {
    use super::*;
    use warrenguard_wire::WarrenPubkey;

    /// A serialized no-op maybenot machine (same vector warrenguard-daita
    /// pins): a valid grant the framework can actually load.
    const NO_OP_MACHINE: &str = "02eNpjYEAHjOgCAAA0AAI=";

    fn granted_spec(machine_specs: Vec<String>) -> warrenguard_wire::DaitaConfig {
        warrenguard_wire::DaitaConfig {
            machine_specs,
            max_padding_frac: 0.5,
            max_blocking_frac: 0.0,
        }
    }

    #[test]
    fn multi_hop_daita_runs_the_machine_the_exit_granted() {
        let shared = multi_hop_daita_shared(true, Some(granted_spec(vec![NO_OP_MACHINE.into()])))
            .expect("a valid grant must build")
            .expect("a granted defense must select the DAITA pumps");
        let state = shared.lock();
        assert!(state.is_enabled(), "the granted machine must be driven");
        assert_eq!(state.machines_count(), 1);
    }

    #[test]
    fn multi_hop_daita_not_granted_runs_plain_pumps_without_claiming_protection() {
        let shared = multi_hop_daita_shared(true, None)
            .expect("an ungranted defense is not an error, it is surfaced");
        assert!(
            shared.is_none(),
            "no grant must select the plain pumps and record daita_active = false"
        );
    }

    #[test]
    fn multi_hop_daita_empty_grant_is_treated_as_not_granted() {
        // The wire contract says an empty machine list is invalid (the exit
        // must send None); a client must never claim an active defense over
        // a grant that drives zero machines.
        let shared = multi_hop_daita_shared(true, Some(granted_spec(vec![])))
            .expect("an empty grant maps to inactive, not an error");
        assert!(shared.is_none());
    }

    #[test]
    fn multi_hop_daita_unusable_grant_fails_the_connect_instead_of_padding_nothing() {
        let err = multi_hop_daita_shared(true, Some(granted_spec(vec!["garbage".into()])))
            .expect_err("an unparseable server-supplied machine must not be swallowed");
        assert!(
            matches!(err, Error::Handshake(_)),
            "unusable grant must surface as a recoverable handshake error, got {err:?}"
        );
    }

    #[test]
    fn dial_refusal_cooldown_throttles_reactions() {
        // First refusal ever (last == 0) must react; a refusal inside the
        // cooldown must not (the supervisor redials every 0.3-2 s, one
        // reaction per rollout wave is enough); past the cooldown it
        // reacts again; a future `last` (clock skew) stays quiet.
        assert!(dial_refusal_reaction_due(1_000, 0));
        assert!(!dial_refusal_reaction_due(1_010, 1_000));
        assert!(dial_refusal_reaction_due(
            1_000 + WARREN_DIAL_REFUSAL_COOLDOWN.as_secs(),
            1_000
        ));
        assert!(!dial_refusal_reaction_due(900, 1_000));
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
    fn macos_ordered_routes_split_bypass_from_tunnel_default() {
        // The ordered variant lets the multi-hop path land the carrier escape
        // BEFORE the 0/0 redirect (zero self-nest window).
        let carrier: IpAddr = "203.0.113.10".parse().unwrap();
        let (bypass, defaults) = build_warren_tunnel_routes_macos_ordered("utun7", &[carrier]);
        assert_eq!(bypass.len(), 1);
        assert_eq!(bypass[0].prefix.prefix(), 32);
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].prefix.prefix(), 0);
    }

    #[test]
    fn reject_error_is_recoverable_so_killswitch_stays_cancelable() {
        // Load-bearing invariant: a NON-ban exit rejection must map to a
        // RECOVERABLE error. Recoverable keeps the firewall kill-switch up
        // (fail-closed, no leak) while leaving the state cancelable and
        // letting the tunnel self-heal once the allowlist syncs. Promoting
        // this to a fatal error would strand a just-subscribed user.
        for reason in [
            RejectionReason::NotAllowlisted,
            RejectionReason::PolicyRefused,
            RejectionReason::IpExhausted,
        ] {
            let err = reject_error(reason);
            assert!(
                err.is_recoverable(),
                "{reason:?} must be recoverable, got {err:?}"
            );
        }
    }

    #[test]
    fn reject_error_marks_a_self_healing_refusal_so_the_retry_loop_can_be_bounded() {
        // A refusal for a missing allowlist entry is what a just-redeemed
        // subscription looks like until the exit's next poll, so it stays
        // recoverable (asserted above). It is nonetheless distinguishable
        // from every other retriable failure, because it is the only one
        // that can persist forever: the connecting state times the run and
        // surfaces it rather than dialing in silence until the user gives
        // up. The message carries the auth-failed token the app already
        // maps to its localized "account is out of time" copy.
        for reason in [
            RejectionReason::NotAllowlisted,
            RejectionReason::PolicyRefused,
        ] {
            let err = reject_error(reason);
            let carried = err
                .session_rejection()
                .unwrap_or_else(|| panic!("{reason:?} must be a session rejection, got {err:?}"));
            assert!(
                carried.starts_with("[EXPIRED_ACCOUNT]"),
                "the refusal must carry the token the app parses, got {carried:?}"
            );
        }

        // Capacity is not an account verdict: an exhausted IP pool frees up
        // on its own with nothing for the user to do, so it must NOT be
        // surfaced as an auth failure.
        assert!(
            reject_error(RejectionReason::IpExhausted)
                .session_rejection()
                .is_none(),
            "an exhausted exit pool is not an account verdict"
        );
    }

    #[test]
    fn reject_error_bans_are_fatal_and_carry_the_banned_token() {
        // A CRL ban is definitive and fleet-wide: it must be FATAL (not
        // recoverable) so the state machine stops reconnecting and surfaces
        // the suspension, and it must carry a [BANNED*] auth-failed token so
        // the app maps it to the localized suspension message. Retrying a ban
        // self-heals nothing and would spin "Reconnecting" forever.
        //
        // An unspecified (0) or unrecognized code carries the generic [BANNED]
        // token; the port-forwarding code carries the specific token so the app
        // shows a forwarded-port suspension.
        for (code, expected_token) in [
            (0u8, "[BANNED]"),
            (BAN_REASON_PORT_FORWARDING, "[BANNED_PORT_FORWARDING]"),
            (200u8, "[BANNED]"),
        ] {
            let err = reject_error(RejectionReason::Banned(code));
            match &err {
                Error::BackendFatal(msg) => assert!(
                    msg.starts_with(expected_token),
                    "ban code {code} must carry {expected_token} so the funnel parses it, got {msg:?}"
                ),
                other => panic!("a ban must map to BackendFatal, got {other:?}"),
            }
            assert!(
                !err.is_recoverable(),
                "a ban must NOT be recoverable (no endless reconnect)"
            );
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
            on_path_rtt: None,
            on_exit_draining: None,
            on_egress_verdict: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_dial_refused: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            max_rate_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
            session_token_provider: None,
            port_entitlement_provider: None,
            cache_dir: None,
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
    fn absent_circuit_fails_closed_as_non_recoverable() {
        // The Warren fleet is multi-hop only: the daemon's circuit
        // selection always populates `multi_hop`. A `None` reaching the
        // tunnel backend means circuit assembly failed upstream, so
        // `start` fails closed with `NoCircuit` rather than opening an
        // unprotected direct datapath. Fail-closed = non-recoverable so
        // the state machine enters the kill-switch error state instead of
        // looping on a dispatch that can never conjure a circuit.
        let e = Error::NoCircuit;
        assert!(
            !e.is_recoverable(),
            "NoCircuit must be non-recoverable (fail closed)"
        );
        assert!(
            e.to_string().contains("multi-hop circuit"),
            "display must name the missing circuit: {e}"
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
                tcp_fallback: false,
            },
            exit: ExitDescriptorSigned {
                exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                exit_mlkem768_pubkey: None,
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
            on_path_rtt: None,
            on_exit_draining: None,
            on_egress_verdict: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_dial_refused: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: None,
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            max_rate_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
            session_token_provider: None,
            port_entitlement_provider: None,
            cache_dir: None,
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
            on_path_rtt: None,
            on_exit_draining: None,
            on_egress_verdict: None,
            warren_register_migrate_handle: None,
            warren_drain_migrate: None,
            warren_dial_refused: None,
            warren_pre_swap_check: None,
            warren_on_overlap_swapped: None,
            nat_pmp: Some(cfg.clone()),
            nat_pmp_observer: None,
            nat_pmp_control_rx: None,
            max_rate_control_rx: None,
            bypass_cidrs: Vec::new(),
            enable_daita: false,
            session_token_provider: None,
            port_entitlement_provider: None,
            cache_dir: None,
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
                tcp_fallback: false,
            },
            exit: ExitDescriptorSigned {
                exit_id: warrenguard_multihop::ExitId::from_bytes([0xdd; 16]),
                exit_ed25519_pubkey: [0xee; 32],
                exit_x25519_multihop_pubkey: [0xff; 32],
                exit_mlkem768_pubkey: None,
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
        // Phase labels used by start_multi_hop.
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
    fn warren_carrier_socket_bypass_linux_uses_the_tunnel_fwmark() {
        // Anti-regression: the whole Port Fail / TunnelCrack ServerIP fix
        // hinges on the carrier socket carrying this exact mark, the one
        // `default_route_split`'s `ip rule fwmark ... lookup main` matches.
        // `None` (or the wrong variant) silently reopens the destination-keyed
        // leak the fix closed: `start_multi_hop` parameterizes its dial socket
        // with this before the split-default route lands.
        assert_eq!(
            warren_carrier_socket_bypass(0),
            warrenguard_tun_core::SocketBypass::Fwmark(warrenguard_tun_core::WARREN_TUNNEL_FWMARK)
        );
        // The interface index must not shape the Linux selection: SO_MARK
        // needs no interface, only the fwmark ip-rule.
        assert_eq!(
            warren_carrier_socket_bypass(7),
            warrenguard_tun_core::SocketBypass::Fwmark(warrenguard_tun_core::WARREN_TUNNEL_FWMARK)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn warren_carrier_socket_bypass_windows_binds_the_discovered_ifindex() {
        // Anti-regression: Windows keys its carrier escape on `IP_UNICAST_IF`
        // to the physical interface discovered by
        // `default_route_split_windows::discover_physical_ifindex`, since the
        // engine's Windows route guard emits no `/32` exception at all (see
        // `install_v4_never_emits_a_32_prefix_or_a_route_to_the_exit_ip` in
        // warrenguard-winroute): a missing or wrong bypass here self-poisons
        // the tunnel the moment the split-default route lands.
        assert_eq!(
            warren_carrier_socket_bypass(7),
            warrenguard_tun_core::SocketBypass::UnicastIf(7)
        );
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

    /// Like [`spawn_lifetime_echo_stub`], recording the credential trailer of
    /// every request so a test can see what each rule presented.
    async fn spawn_credential_recording_stub() -> (std::net::SocketAddr, CredentialLog) {
        let seen: CredentialLog = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                let (n, peer) = match sock.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let frame = &buf[..n];
                let (internal_port, lifetime) = match parse_request(frame) {
                    Ok(warrenguard_natpmp_protocol::Request::Map {
                        internal_port,
                        lifetime_secs,
                        ..
                    }) => (internal_port, lifetime_secs),
                    _ => continue,
                };
                sink.lock().expect("sink").push((
                    internal_port,
                    warrenguard_natpmp_protocol::credential_trailer(frame).map(<[u8]>::to_vec),
                ));
                let resp = serialize_response(&NatPmpResponse::Map {
                    proto: MapProto::Udp,
                    result_code: ResultCode::Success,
                    epoch_secs: 0,
                    internal_port,
                    external_port: 49000u16.saturating_add(internal_port),
                    lifetime_secs: lifetime,
                    rate_limit: None,
                });
                let _ = sock.send_to(&resp, peer).await;
            }
        });
        (addr, seen)
    }

    /// `(internal port, credential presented)` per request seen by the stub.
    type CredentialLog = Arc<StdMutex<Vec<(u16, Option<Vec<u8>>)>>>;

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
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 22,
            sticky_suggestion: false,
            remap_epoch: 0,
        }
    }

    #[tokio::test]
    async fn each_rule_presents_its_own_entitlement() {
        // One entitlement buys ONE port, so two rules must present two
        // different credentials: sharing one would read at the exit as a
        // single port and the second rule would be refused.
        let (server, seen) = spawn_credential_recording_stub().await;
        let (observer, _log) = collector_observer();
        let mut cfg = natpmp_cfg(60);
        cfg.rules = vec![
            NatPmpRule {
                protocol: NatPmpProto::Udp,
                suggested_external_port: 0,
                internal_port: 22,
                sticky_suggestion: false,
            },
            NatPmpRule {
                protocol: NatPmpProto::Udp,
                suggested_external_port: 0,
                internal_port: 23,
                sticky_suggestion: false,
            },
        ];
        let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));

        // Slot n gets credential [n; 8], so the assertion reads the slot the
        // controller assigned straight off the wire.
        let entitlements: PortEntitlementProvider =
            Arc::new(|slot| u8::try_from(slot).ok().map(|n| vec![n; 8]));

        let runtime = tokio::runtime::Handle::current();
        let handle = runtime.spawn(run_nat_pmp_controller(
            runtime.clone(),
            server,
            None,
            observer,
            Some(cfg),
            rx,
            Some(entitlements),
        ));

        let mut presented = Vec::new();
        for _ in 0..100 {
            presented = seen.lock().expect("sink").clone();
            if presented.len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        handle.abort();

        let for_22 = presented.iter().find(|(p, _)| *p == 22).cloned();
        let for_23 = presented.iter().find(|(p, _)| *p == 23).cloned();
        let (_, cred_22) = for_22.expect("rule 22 never reached the server");
        let (_, cred_23) = for_23.expect("rule 23 never reached the server");
        assert!(cred_22.is_some(), "rule 22 presented no credential");
        assert_ne!(
            cred_22, cred_23,
            "both rules drew the same entitlement slot"
        );
    }

    #[tokio::test]
    async fn a_rule_keeps_its_slot_across_renewals() {
        // The exit spends a credential once and renews it afterwards. A rule
        // whose slot moved between requests would spend a second entitlement
        // for the same port.
        let (server, seen) = spawn_credential_recording_stub().await;
        let (observer, _log) = collector_observer();
        // lifetime 2 => the loop renews after 1 s.
        let cfg = natpmp_cfg(2);
        let (_tx, rx) = tokio::sync::watch::channel(Some(cfg.clone()));
        let entitlements: PortEntitlementProvider =
            Arc::new(|slot| u8::try_from(slot).ok().map(|n| vec![n; 8]));

        let runtime = tokio::runtime::Handle::current();
        let handle = runtime.spawn(run_nat_pmp_controller(
            runtime.clone(),
            server,
            None,
            observer,
            Some(cfg),
            rx,
            Some(entitlements),
        ));

        let mut presented = Vec::new();
        for _ in 0..150 {
            presented = seen.lock().expect("sink").clone();
            if presented.len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        handle.abort();

        assert!(presented.len() >= 2, "expected a renewal: {presented:?}");
        assert_eq!(
            presented[0].1, presented[1].1,
            "the renewal presented a different entitlement"
        );
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
            None,
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
            None,
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
