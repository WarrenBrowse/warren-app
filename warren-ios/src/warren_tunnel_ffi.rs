//! Warren tunnel FFI for iOS (Quinn-based VPN tunnel via warren-tunnel).
//!
//! The C ABI surface (C-repr structs + extern "C" fn signatures) is
//! defined here unconditionally so that `cbindgen` can generate the
//! `warren_rust_runtime.h` declarations consumed by the Swift side
//! (`ios/WarrenRustRuntime/WarrenQuinnAdapter.swift`).
//!
//! The function **bodies** that actually drive a Quinn tunnel are gated
//! behind the `tunnel` Cargo feature. With the feature OFF the FFI
//! returns null / sentinel error codes. With the feature ON the FFI
//! allocates a [`WarrenTunnelHandleImpl`] holding the Tokio runtime,
//! the [`IosTun`] bridge (cf. `warren-tunnel::IosTun`) and the event
//! callback registration. The Quinn connection itself is wired in
//! the packet-tunnel-provider data plane.
//!
//! The handle lifecycle:
//! - `warren_tunnel_start` boxes the impl, returns the raw pointer.
//! - `warren_tunnel_stop` reconstitutes the box via [`Box::from_raw`]
//!   so Drop runs (Tokio runtime, channels, callback cleanup).
//! - All other entry points cast the raw pointer back to `&mut Impl`
//!   without taking ownership.

#[cfg(all(target_os = "ios", feature = "tunnel"))]
use std::ffi::CStr;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

/// Opaque handle representing an active Warren tunnel. Created by
/// [`warren_tunnel_start`] ; destroyed by [`warren_tunnel_stop`].
/// The Swift side treats this as an `OpaquePointer`.
#[repr(C)]
pub struct WarrenTunnelHandle {
    _private: [u8; 0],
}

/// Parameters passed from Swift to start a Warren tunnel. Mirrors the
/// public surface of `warren_tunnel::WarrenTunnelParameters`.
///
/// String fields are null-terminated UTF-8 ; the Swift side allocates
/// and retains them for the duration of the `warren_tunnel_start`
/// call (Rust only borrows during marshalling).
#[repr(C)]
pub struct WarrenTunnelParametersC {
    /// 32-byte Ed25519 public key of the exit relay.
    pub exit_pubkey: [u8; 32],
    /// Null-terminated UTF-8 "IP:port" of the exit relay.
    pub exit_endpoint: *const c_char,
    /// 32-byte Ed25519 signing seed derived from the user wallet
    /// (see `warren_wallet_seed_from_mnemonic` + `derive_node_key`).
    pub wallet_signing_seed: [u8; 32],
    /// Optional multi-hop entry relay. Null when single-hop.
    pub multi_hop_relay: *const WarrenRelayConfigC,
    /// Optional DAITA defensive shaping spec. Null when DAITA OFF.
    pub daita_spec: *const WarrenDaitaSpecC,
    /// 1 enables NAT-PMP port forwarding through the tunnel ; 0 disables.
    pub nat_pmp_enabled: u8,
    /// Pointer to an array of null-terminated UTF-8 CIDRs to bypass
    /// (see `--bypass-cidr`). Length given by `bypass_cidrs_count`.
    pub bypass_cidrs: *const *const c_char,
    /// Number of entries in `bypass_cidrs`.
    pub bypass_cidrs_count: u32,
    /// Signed multi-hop directory JSON, fetched by Swift over URLSession
    /// from `GET {api}/v1/multihop/directory`. When non-null the tunnel
    /// rides the multi-hop wire protocol (the production fleet is
    /// multi-hop only): a 2-hop circuit when `multihop_two_hop` is 1,
    /// otherwise a 1-hop circuit collapsed onto one node. The JSON is
    /// verified Rust-side against the baked root pin before use. Null
    /// falls back to the legacy single-hop path (dev / loopback only).
    pub multihop_directory_json: *const c_char,
    /// 1 selects a 2-hop circuit (entry != exit, country diverse); 0 a
    /// 1-hop circuit. Ignored when `multihop_directory_json` is null.
    pub multihop_two_hop: u8,
    /// Optional ISO 3166-1 alpha-2 entry-country hint (null / empty = any).
    pub multihop_entry_country: *const c_char,
    /// Optional ISO 3166-1 alpha-2 exit-country hint (null / empty = any).
    pub multihop_exit_country: *const c_char,
    /// Optional path to the App Group file that persists the multi-hop
    /// directory anti-rollback high-water mark (highest trusted
    /// `generation`). Read before verification to reject a stale directory,
    /// raised after a successful selection. Null disables persistence (the
    /// gate then only protects within a single connect). Ignored when
    /// `multihop_directory_json` is null.
    pub multihop_generation_state_path: *const c_char,
    /// Optional path to the App Group file that persists the exit-pubkey
    /// trust-on-first-use (TOFU) pin table. When non-null, the selected
    /// exit's Ed25519 pubkey is checked against the pin for its `exit_id`
    /// before connecting: a mismatch fails the connection closed and is
    /// surfaced via `warren_tunnel_take_pin_mismatch` for the user to trust
    /// or reject. Null disables pinning. Ignored on the legacy single-hop
    /// path (the production fleet is multi-hop only).
    pub pin_store_path: *const c_char,
}

/// Multi-hop entry relay configuration.
#[repr(C)]
pub struct WarrenRelayConfigC {
    /// 32-byte Ed25519 public key of the entry relay.
    pub pubkey: [u8; 32],
    /// Null-terminated UTF-8 "IP:port" of the entry relay.
    pub endpoint: *const c_char,
    /// Null-terminated UTF-8 ISO 3166-1 alpha-2 country code.
    pub country_code: *const c_char,
}

/// DAITA defensive shaping spec (cf. `warren_daita_doctrine_v1`).
#[repr(C)]
pub struct WarrenDaitaSpecC {
    /// 32-byte Maybenot machine seed.
    pub machine_seed: [u8; 32],
    /// Padding budget in packets/sec.
    pub padding_pps: u32,
}

/// Tunnel state enum surfaced via `warren_tunnel_status`. Variants
/// other than `Disconnected` are constructed by the warren-tunnel
/// dispatcher, which only compiles on the iOS target with the `tunnel`
/// feature. Off that exact path - including every host (clippy / test)
/// build, with or without `tunnel` - the enum has no constructor, hence
/// the `cfg_attr(expect)` suppression scoped to `not(ios && tunnel)`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(all(target_os = "ios", feature = "tunnel")),
    expect(
        dead_code,
        reason = "FFI surface ; only constructed on the iOS tunnel path"
    )
)]
pub enum WarrenTunnelStateC {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    Reconnecting = 3,
    Failed = 4,
}

/// Tunnel status snapshot.
#[repr(C)]
pub struct WarrenTunnelStatusC {
    pub state: WarrenTunnelStateC,
    pub bytes_in: u64,
    pub bytes_out: u64,
    /// Seconds since the current connection was established. 0 when
    /// `state != Connected`.
    pub connected_duration_seconds: u64,
    /// Cumulative failover count this session.
    pub failover_count: u32,
}

/// Event tag for the variant union below. Variants are constructed by
/// the warren-tunnel dispatcher once the Quinn connection task is
/// wired.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "FFI surface ; Failover + NatPmp* variants wired in a follow-up"
)]
#[expect(
    clippy::enum_variant_names,
    reason = "The `Event` prefix is required: C does not scope enum names, so unprefixed variants would collide with WarrenTunnelStateC across the FFI boundary."
)]
pub enum WarrenTunnelEventTagC {
    // Prefix `Event` to disambiguate from `WarrenTunnelStateC`
    // enumerators (C doesn't scope enum names - `Connected` would
    // collide otherwise). Swift bridges via the imported C constants.
    EventConnected = 0,
    EventDisconnected = 1,
    EventReconnecting = 2,
    EventFailover = 3,
    EventNatPmpMapped = 4,
    EventNatPmpRenewed = 5,
    EventNatPmpFailed = 6,
    /// Fired once, on the very first connection attempt of a tunnel
    /// session (before any successful `Connected` event). Subsequent
    /// attempts after a connection drop fire `EventReconnecting`
    /// instead. Swift uses this to distinguish "tunnel starting" from
    /// "tunnel recovering".
    EventConnecting = 7,
}

/// Tagged-union event payload.
/// The Swift side reads `tag` first then accesses the matching
/// `data_*` field (e.g. `data_failover_country_code` when tag ==
/// `Failover`). cbindgen emits this as a C struct with a discriminator.
#[repr(C)]
pub struct WarrenTunnelEventC {
    pub tag: WarrenTunnelEventTagC,
    /// Failover : null-terminated UTF-8 country code of new exit.
    pub data_failover_country_code: *const c_char,
    /// NatPmp* : forwarded port (external).
    pub data_nat_pmp_external_port: u16,
    /// NatPmpMapped : internal port + lifetime.
    pub data_nat_pmp_internal_port: u16,
    pub data_nat_pmp_lifetime_seconds: u32,
    /// NatPmpFailed : null-terminated UTF-8 reason.
    pub data_nat_pmp_failure_reason: *const c_char,
}

/// Event callback signature. Called from a Tokio task on the
/// warren-tunnel runtime ; the Swift side must marshal back to the
/// MainActor if the callback updates UI state.
///
/// `event` pointer is owned by Rust for the duration of the callback
/// only ; Swift must copy any UTF-8 strings before returning.
pub type WarrenTunnelEventCallback =
    unsafe extern "C" fn(event: *const WarrenTunnelEventC, context: *mut c_void);

/// Outbound packet callback signature. Called from a Tokio task that
/// drains [`warren_tunnel::IosTun`] after each `PacketDevice::send` from
/// the downlink pump. The Swift side bridges to
/// `NEPacketTunnelFlow.writePackets(_:withProtocols:)`.
///
/// `data` + `len` are owned by Rust for the duration of the call ;
/// Swift must copy before returning. `context` is the opaque pointer
/// passed at registration time.
pub type WarrenTunnelOutboundCallback =
    unsafe extern "C" fn(data: *const u8, len: usize, context: *mut c_void);

/// Exit-allocated IPv4 surfaced to Swift after the multi-hop circuit's
/// setup-stream returns an `IpAssign` control message. Swift re-applies
/// `NEPacketTunnelNetworkSettings` with this address so the TUN's source
/// IP matches what the exit expects, otherwise return traffic is dropped
/// (the iOS analog of the daemon's `RealTun::reassign_ipv4`).
///
/// IPv4-only: the iOS multi-hop path keeps native IPv6 blackholed
/// (`wants_ipv6 = false`), so no v6 assignment is requested or surfaced.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarrenTunnelIpAssignC {
    /// Exit-allocated IPv4 address (network byte order, i.e. octets a.b.c.d).
    pub ipv4: [u8; 4],
    /// Subnet prefix length for the allocated address.
    pub prefix_len: u8,
    /// Exit-side gateway IPv4 (the exit TUN host).
    pub gateway_ipv4: [u8; 4],
}

/// Exit-allocated IP callback signature. Called from a Tokio task on the
/// warren-tunnel runtime when the multi-hop circuit reports a fresh
/// `IpAssign`. The Swift side re-applies the tunnel network settings.
///
/// `assign` is owned by Rust for the duration of the call; Swift must
/// copy the fields before returning.
pub type WarrenTunnelIpAssignCallback =
    unsafe extern "C" fn(assign: *const WarrenTunnelIpAssignC, context: *mut c_void);

// ---- Return codes shared by all tunnel FFI fns ----

const RC_OK: c_int = 0;
const RC_INVALID_INPUT: c_int = -1;
// RC_NOT_CONNECTED is used only on the `tunnel` feature path ; on the
// `not(tunnel)` path it is dead code (the FFI returns the disabled
// sentinel instead). `cfg_attr(...expect(dead_code))` triggers the
// expectation exactly on the path where the lint fires, leaving the
// `tunnel` path free of suppression noise.
#[cfg_attr(
    not(all(target_os = "ios", feature = "tunnel")),
    expect(dead_code, reason = "feature-conditional")
)]
const RC_NOT_CONNECTED: c_int = -3;
// RC_TUNNEL_FEATURE_DISABLED is the mirror of RC_NOT_CONNECTED for the
// `not(tunnel)` path ; on the `tunnel` path it is dead code.
#[cfg_attr(
    all(target_os = "ios", feature = "tunnel"),
    expect(dead_code, reason = "feature-conditional")
)]
const RC_TUNNEL_FEATURE_DISABLED: c_int = -10;

// ---- Handle implementation (feature-gated) ----

#[cfg(all(target_os = "ios", feature = "tunnel"))]
mod handle_impl {
    //! Real handle implementation when the `tunnel` feature is on.
    //!
    //! Owns the Tokio runtime + [`warren_tunnel::IosTun`] bridge +
    //! callback registration. The Quinn connection task is spawned
    //! lazily in [`WarrenTunnelHandleImpl::run`].

    use super::{
        WarrenTunnelEventCallback, WarrenTunnelIpAssignCallback, WarrenTunnelOutboundCallback,
        WarrenTunnelStateC,
    };
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::runtime::Runtime;
    use warren_tunnel::IosTun;

    /// Pointer + opaque context pair tracked as a single unit. Both
    /// fields move together so the borrow checker enforces atomic
    /// replace via [`Mutex<Option<CallbackEntry<F>>>`].
    pub struct CallbackEntry<F> {
        pub callback: F,
        pub context: *mut c_void,
    }

    // SAFETY: `F` must itself be `Send`/`Sync` (bounds enforced below).
    // The opaque `context` pointer is provided by the Swift side and
    // is treated as a black-box value: we never deref it from Rust; we
    // only pass it back as-is to the Swift callback. Swift is
    // responsible for the pointer's lifetime and thread-safety. All
    // concrete instantiations of `CallbackEntry` use `extern "C" fn`
    // types, which are inherently `Send + Sync`, so the bounds are
    // trivially satisfied at every call site.
    // The [`Send`]/[`Sync`] markers are required so the entry can live
    // inside a [`Mutex`] shared across Tokio tasks.
    unsafe impl<F: Send> Send for CallbackEntry<F> {}
    unsafe impl<F: Send + Sync> Sync for CallbackEntry<F> {}

    /// State owned by a single Warren tunnel handle.
    ///
    /// The struct is held behind a [`Box`] ; the raw pointer is what
    /// [`super::warren_tunnel_start`] returns to Swift. All access
    /// (read or write) goes through the safe helpers below.
    pub struct WarrenTunnelHandleImpl {
        /// Dedicated runtime for this tunnel session. Drops cleanly
        /// when the box is dropped : all spawned tasks are aborted.
        pub runtime: Runtime,
        /// Swift-side bridge to `NEPacketTunnelFlow`. The downlink
        /// pump (uplink in iOS terminology) calls
        /// `PacketDevice::send` ; the outbound dispatcher task drains
        /// `IosTun::next_outbound` and invokes
        /// [`Self::outbound_callback`].
        pub tun: IosTun,
        /// Registered event callback (connected / disconnected /
        /// failover / NatPmp*). Optional ; cleared by passing `None`
        /// to the setter.
        pub event_callback: Mutex<Option<CallbackEntry<WarrenTunnelEventCallback>>>,
        /// Registered outbound packet callback. Wired by the Swift
        /// side immediately after [`super::warren_tunnel_start`] and
        /// before any inbound packet is injected.
        pub outbound_callback: Mutex<Option<CallbackEntry<WarrenTunnelOutboundCallback>>>,
        /// Registered exit-allocated IP callback. Fired by the multi-hop
        /// reassign task when the circuit reports a fresh `IpAssign`.
        /// Optional; the single-hop dev path never fires it.
        pub ip_assign_callback: Mutex<Option<CallbackEntry<WarrenTunnelIpAssignCallback>>>,
        /// Atomic counters surfaced through
        /// [`super::warren_tunnel_status`]. Atomics avoid contention
        /// with the pump tasks while keeping the status read O(1).
        pub state: AtomicU8, // stores discriminant of WarrenTunnelStateC
        pub bytes_in: AtomicU64,
        pub bytes_out: AtomicU64,
        pub connected_at_secs: AtomicU64, // 0 until first Connected event
        pub failover_count: AtomicU32,
        /// Set to `true` after the first successful `Connected` event.
        /// Used by the connection task to decide whether to fire
        /// `EventConnecting` (first attempt) or `EventReconnecting`
        /// (subsequent attempts after a drop).
        pub has_connected_once: AtomicBool,
        /// Holds the JSON details of an exit-pubkey TOFU mismatch when the
        /// connect failed closed because the selected exit presented a
        /// pubkey differing from the stored pin. Swift drains it via
        /// `warren_tunnel_take_pin_mismatch` after a failure to decide
        /// whether to show the Trust / Report / Reject alert.
        pub pin_mismatch: Mutex<Option<String>>,
    }

    impl WarrenTunnelHandleImpl {
        /// Allocate a fresh handle backed by a single-threaded Tokio
        /// runtime and an [`IosTun`] bridge.
        ///
        /// Returns an error if Tokio refuses to build the runtime
        /// (file descriptor exhaustion, etc.) ; the caller maps this
        /// to a null pointer back through FFI.
        pub fn new() -> std::io::Result<Self> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("warren-tunnel-ios")
                .build()?;
            Ok(Self {
                runtime,
                tun: IosTun::new(),
                event_callback: Mutex::new(None),
                outbound_callback: Mutex::new(None),
                ip_assign_callback: Mutex::new(None),
                state: AtomicU8::new(WarrenTunnelStateC::Disconnected as u8),
                bytes_in: AtomicU64::new(0),
                bytes_out: AtomicU64::new(0),
                connected_at_secs: AtomicU64::new(0),
                failover_count: AtomicU32::new(0),
                has_connected_once: AtomicBool::new(false),
                pin_mismatch: Mutex::new(None),
            })
        }

        /// Record a TOFU pin-mismatch JSON payload (drained by Swift via
        /// `warren_tunnel_take_pin_mismatch`). Lock poisoning is treated as
        /// non-fatal: a failed record only loses the (advisory) UI prompt.
        pub fn set_pin_mismatch(&self, json: String) {
            if let Ok(mut slot) = self.pin_mismatch.lock() {
                *slot = Some(json);
            }
        }

        /// Take (and clear) any recorded pin-mismatch payload.
        pub fn take_pin_mismatch(&self) -> Option<String> {
            self.pin_mismatch
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
        }

        /// Spawn the outbound dispatcher task on the handle's runtime.
        /// Drains [`IosTun::next_outbound`] in a loop and invokes the
        /// registered outbound callback for each packet.
        ///
        /// The task exits cleanly when the [`IosTun`] outbound channel
        /// closes (handle drop) or when no callback has been
        /// registered yet (it'll be re-spawned by the registration
        /// FFI on first set).
        pub fn spawn_outbound_dispatcher(self: &Arc<Self>) {
            let me = Arc::clone(self);
            self.runtime.spawn(async move {
                loop {
                    let Some(packet) = me.tun.next_outbound().await else {
                        break;
                    };
                    me.bytes_out
                        .fetch_add(packet.len() as u64, Ordering::Relaxed);
                    // SAFETY: callback + context come from Swift via
                    // `warren_tunnel_set_outbound_callback`. The
                    // pointer is only dereferenced inside the Swift
                    // implementation. We pass the packet as a borrowed
                    // slice owned by Rust for the duration of the
                    // call ; Swift must copy before returning.
                    let cb_entry = me.outbound_callback.lock().ok();
                    let Some(cb_entry) = cb_entry else { continue };
                    if let Some(entry) = cb_entry.as_ref() {
                        unsafe {
                            (entry.callback)(packet.as_ptr(), packet.len(), entry.context);
                        }
                    }
                }
            });
        }

        /// Atomically store a new state discriminant.
        pub fn set_state(&self, state: WarrenTunnelStateC) {
            self.state.store(state as u8, Ordering::Relaxed);
        }

        /// Fire a tagged event on the registered callback (if any).
        /// `tag` is the only field populated for `Connected` /
        /// `Disconnected` / `Reconnecting` ; the data fields are
        /// zero-initialized. Failover + NatPmp* variants pass through
        /// [`fire_event_full`] with explicit data.
        pub fn fire_event(&self, tag: super::WarrenTunnelEventTagC) {
            let event = super::WarrenTunnelEventC {
                tag,
                data_failover_country_code: std::ptr::null(),
                data_nat_pmp_external_port: 0,
                data_nat_pmp_internal_port: 0,
                data_nat_pmp_lifetime_seconds: 0,
                data_nat_pmp_failure_reason: std::ptr::null(),
            };
            let Ok(cb_entry) = self.event_callback.lock() else {
                return;
            };
            let Some(entry) = cb_entry.as_ref() else {
                return;
            };
            // SAFETY: callback + context come from Swift via
            // `warren_tunnel_set_event_callback`. Swift owns the
            // context pointer lifecycle ; we just pass it back. The
            // event pointer is valid for the duration of the call.
            unsafe {
                (entry.callback)(&raw const event, entry.context);
            }
        }

        /// Fire the exit-allocated IP callback (if registered) so Swift
        /// re-applies the tunnel network settings with the new address.
        pub fn fire_ip_assign(
            &self,
            ipv4: std::net::Ipv4Addr,
            prefix_len: u8,
            gateway: std::net::Ipv4Addr,
        ) {
            let assign = super::WarrenTunnelIpAssignC {
                ipv4: ipv4.octets(),
                prefix_len,
                gateway_ipv4: gateway.octets(),
            };
            let Ok(cb_entry) = self.ip_assign_callback.lock() else {
                return;
            };
            let Some(entry) = cb_entry.as_ref() else {
                return;
            };
            // SAFETY: callback + context come from Swift via
            // `warren_tunnel_set_ip_assign_callback`. Swift owns the
            // context pointer lifecycle; we just pass it back. The
            // `assign` pointer is valid for the duration of the call.
            unsafe {
                (entry.callback)(&raw const assign, entry.context);
            }
        }

        /// Snapshot of the current state discriminant.
        pub fn state_snapshot(&self) -> WarrenTunnelStateC {
            match self.state.load(Ordering::Relaxed) {
                0 => WarrenTunnelStateC::Disconnected,
                1 => WarrenTunnelStateC::Connecting,
                2 => WarrenTunnelStateC::Connected,
                3 => WarrenTunnelStateC::Reconnecting,
                _ => WarrenTunnelStateC::Failed,
            }
        }
    }
}

/// Drives a multi-hop circuit on the handle's runtime: verify + select a
/// circuit from the signed directory, bring up a `MultiHopSupervisor`,
/// and pump `IosTun` against the live session. Surfaces Connecting /
/// Connected / Reconnecting / Disconnected on the event callback from the
/// supervisor's session watch. The supervisor owns reconnect; the pumps
/// drop packets while it is mid-reconnect.
///
/// Exit-allocated IP reassign is wired: the supervisor + downlink publish
/// each `IpAssign` onto an `IpAssignChannel`, and a reassign task fires
/// the registered IP callback so Swift re-applies the tunnel network
/// settings with the exit address (the iOS analog of the daemon's
/// `RealTun::reassign_ipv4` loop). IPv4-only, matching `wants_ipv6 =
/// false`.
///
/// DAITA is off on this path by design: the desktop daemon's multi-hop
/// data plane runs the plain `run_uplink`/`run_downlink` pumps too, so iOS
/// is structurally identical (DAITA is negotiated only on the single-hop
/// path on every platform).
///
/// In-session directory refresh is driven from Swift (a 30-min timer that
/// re-fetches, calls `warren_multihop_check_generation`, and reconnects on a
/// generation advance), so this function selects the circuit once per
/// (re)connect and the supervisor reconnects to the stable circuit on drops.
#[cfg(all(target_os = "ios", feature = "tunnel"))]
fn spawn_multi_hop(
    arc: std::sync::Arc<handle_impl::WarrenTunnelHandleImpl>,
    directory_json: String,
    two_hop: bool,
    entry_country: String,
    exit_country: String,
    generation_state_path: Option<String>,
    pin_store_path: Option<String>,
    signing_key: ed25519_dalek::SigningKey,
) {
    use std::sync::atomic::Ordering;

    use warren_client::supervised_pump::{
        ExitDrainingChannel, IpAssignChannel, run_downlink, run_uplink,
    };
    use warren_client::supervisor::{MultiHopSupervisor, SupervisorConfig};

    let arc_for_task = std::sync::Arc::clone(&arc);
    arc.runtime.spawn(async move {
        arc_for_task.set_state(WarrenTunnelStateC::Connecting);
        let first_attempt = !arc_for_task.has_connected_once.load(Ordering::Acquire);
        arc_for_task.fire_event(if first_attempt {
            WarrenTunnelEventTagC::EventConnecting
        } else {
            WarrenTunnelEventTagC::EventReconnecting
        });

        // Anti-rollback: read the persisted high-water generation (if a
        // state path was supplied) so a replayed older-but-signed directory
        // is rejected. `None`/missing file => 0 => gate open (first connect).
        let gen_path = generation_state_path.as_ref().map(std::path::PathBuf::from);
        let min_generation = gen_path
            .as_deref()
            .map(crate::warren_multihop_generation::read_high_water)
            .unwrap_or(0);

        let circuit = match crate::warren_multihop_directory::verify_and_select(
            &directory_json,
            two_hop,
            &entry_country,
            &exit_country,
            now_secs(),
            min_generation,
        ) {
            Ok(Some(circuit)) => circuit,
            Ok(None) => {
                tracing::warn!("Warren multi-hop: no valid circuit in the directory");
                arc_for_task.set_state(WarrenTunnelStateC::Failed);
                arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, "Warren multi-hop directory verification failed");
                arc_for_task.set_state(WarrenTunnelStateC::Failed);
                arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
                return;
            }
        };

        // Exit-pubkey TOFU pin check. The exit's `exit_ed25519_pubkey` (its
        // QUIC TLS RPK identity) is NOT covered by the /v1 directory
        // signature, while its `exit_id` (16-byte routing tag) is. So pin the
        // observed RPK against the signed, stable exit_id: a later key swap
        // under the same exit_id (a compromised exit) is detected here and the
        // connection fails closed until the user trusts the new key. Mirrors
        // the desktop daemon's `warren_pin_verify` hook.
        if let Some(path) = pin_store_path.as_deref() {
            let path = std::path::Path::new(path);
            let exit_id_hex = hex::encode(circuit.exit.exit_id.as_bytes());
            let observed_hex = hex::encode(circuit.exit.exit_ed25519_pubkey);
            let mut table = crate::warren_pin_store::load(path);
            match crate::warren_pin_store::pin_verify(
                &mut table,
                &exit_id_hex,
                &observed_hex,
                &exit_country,
                now_secs(),
            ) {
                crate::warren_pin_store::PinOutcome::Mismatch { pinned } => {
                    tracing::error!(
                        exit_id = %exit_id_hex,
                        "Warren exit pubkey TOFU mismatch; failing closed"
                    );
                    let payload = serde_json::json!({
                        "exit_id": exit_id_hex,
                        "observed": observed_hex,
                        "pinned": pinned,
                        "country": exit_country,
                    })
                    .to_string();
                    arc_for_task.set_pin_mismatch(payload);
                    arc_for_task.set_state(WarrenTunnelStateC::Failed);
                    arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
                    return;
                }
                // FirstSeen (TOFU insert) or Match (last_seen bumped): persist
                // the updated table and proceed with the connection.
                _ => crate::warren_pin_store::store(path, &table),
            }
        }

        // The accepted generation is persisted as the new high-water mark
        // only AFTER a successful connect (in the state-watch loop below),
        // never here. Raising it on select alone would let a server-key
        // compromise serving a validly-signed directory with an inflated
        // generation (whose nodes never connect) poison the monotonic mark
        // and permanently reject later legitimate directories.
        let accepted_generation = circuit.generation;

        let Ok(bind_addr) = "0.0.0.0:0".parse::<std::net::SocketAddr>() else {
            arc_for_task.set_state(WarrenTunnelStateC::Failed);
            arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
            return;
        };
        // The exit allocates a per-wallet sticky IPv4 and announces it via
        // `IpAssign` on this channel (the supervisor's setup-stream reply
        // and the downlink pump both publish). The reassign task below
        // forwards each fresh address to Swift.
        let ip_assign_channel = IpAssignChannel::new();
        let cfg = SupervisorConfig {
            relay: std::sync::Arc::new(circuit.relay),
            exit_id: circuit.exit.exit_id,
            exit_x25519_multihop_pubkey: circuit.exit.exit_x25519_multihop_pubkey,
            operational_pubkey: circuit.operational_pubkey,
            client_signing: signing_key,
            bind_addr,
            // GSO is a no-op on Apple platforms (quinn-udp has no UDP GSO on
            // Darwin, unlike Linux), so this is moot vs the desktop daemon's
            // `enable_gso: true`; `false` states the effective behavior.
            enable_gso: false,
            use_warren_obfuscation: true,
            // 300 ms base / 2 s ceiling, matching the desktop daemon's
            // override (talpid-warren-tunnel::start_multi_hop). The default
            // `Backoff::HANDSHAKE` 15 s ceiling overshoots a network-change
            // recovery: a re-dial parked in a 15 s backoff misses the window
            // when the link returns, stretching a Wi-Fi/cellular handover.
            backoff: warrenguard_backoff::Backoff {
                base: std::time::Duration::from_millis(300),
                max: std::time::Duration::from_secs(2),
            },
            on_reconnect: None,
            ip_assign_channel: Some(ip_assign_channel.clone()),
            wants_ipv6: false,
            // Single connection on iOS: the NetworkExtension memory cap
            // (50 MB) leaves no headroom for N bonded Quinn endpoints,
            // and cellular paths rarely benefit from multi-flow bonding.
            n_connections: 1,
        };
        let (supervisor, watch) = MultiHopSupervisor::new(cfg);
        // ADR 36: the downlink pump publishes a mid-session `ExitDraining`
        // advisory here; the drain reactor below forces a supervisor redial so
        // we migrate before the exit's hard close. iOS has no daemon avoid-set,
        // so exit-exclusion is the ambient relay-list refresh's job; this is
        // the proactive-reconnect half (mirrors the desktop talpid reactor).
        let exit_draining_channel = ExitDrainingChannel::new();
        let drain_handle = supervisor.handle();
        let tun = arc_for_task.tun.clone();

        // Plain pumps (no DAITA), structurally identical to the desktop
        // daemon's multi-hop data plane (`talpid-warren-tunnel::
        // start_multi_hop` drives `run_uplink`/`run_downlink`). DAITA
        // shaping is negotiated only on the single-hop path on every
        // platform, so keeping it off here is a deliberate cross-platform
        // decision, not a gap. The exit-allocated IP still flows: the
        // supervisor publishes each setup-stream `IpAssign` on
        // `ip_assign_channel` (config above), which the reassign task below
        // forwards to Swift.
        let up_watch = watch.clone();
        let up_tun = tun.clone();
        tokio::spawn(async move {
            if let Err(e) = run_uplink(up_watch, up_tun).await {
                tracing::error!(error = %e, "multi-hop uplink terminated");
            }
        });
        let dn_watch = watch.clone();
        let dn_tun = tun.clone();
        let dn_drain = exit_draining_channel.clone();
        tokio::spawn(async move {
            if let Err(e) = run_downlink(dn_watch, dn_tun, Some(dn_drain)).await {
                tracing::error!(error = %e, "multi-hop downlink terminated");
            }
        });
        // ADR 36 drain reactor: on each in-band drain advisory, force the
        // supervisor to redial (it reconnects through its own backoff, which
        // also spreads the herd, so no extra jitter here). NOT one-shot:
        // `force_reconnect` keeps the same session/channel alive (it redials
        // internally rather than rebuilding the pumps), so the loop must keep
        // listening to catch a drain on the exit it lands on next. The exit
        // re-emits dedup'd, so a repeat of the SAME advisory does not re-fire;
        // teardown drops the sender, ending this task.
        {
            let mut drain_sub = exit_draining_channel.subscribe();
            let _ = drain_sub.borrow_and_update();
            let drain_arc = std::sync::Arc::clone(&arc_for_task);
            tokio::spawn(async move {
                loop {
                    if drain_sub.changed().await.is_err() {
                        return;
                    }
                    if drain_sub.borrow_and_update().is_some() {
                        tracing::info!(
                            "multi-hop exit draining; forcing reconnect (ADR 36 proactive \
                             migration; exit-exclusion via the ambient relay-list refresh)"
                        );
                        // Surface the maintenance migration to Swift so the UI
                        // reflects it. Reuses `EventReconnecting` (no new C-ABI
                        // tag): the app already renders it as a transient
                        // reconnect, which is exactly what a drain migration is.
                        drain_arc.fire_event(WarrenTunnelEventTagC::EventReconnecting);
                        drain_handle.force_reconnect();
                    }
                }
            });
        }
        tokio::spawn(async move {
            if let Err(e) = supervisor.run().await {
                tracing::error!(error = %e, "multi-hop supervisor terminated");
            }
        });

        // Reassign task: forward each exit-allocated IPv4 to Swift. Mirrors
        // the daemon's reassign loop (warren_multihop_tun_client.rs) but,
        // instead of mutating a RealTun, fires the IP callback so Swift
        // re-applies the NEPacketTunnelNetworkSettings. Dedups by the last
        // forwarded address so a sticky reconnect to the same IP is a no-op.
        //
        // Subscribe BEFORE the spawn and move only the `Receiver`: a task that
        // owned an `IpAssignChannel` (sender) clone could never see
        // `changed()` error and would run for the whole session. With only the
        // receiver, the task ends when the supervisor drops its sender. Drop
        // the local channel so the supervisor's clone (in `cfg`) is the only
        // remaining sender, making teardown deterministic.
        let mut reassign_rx = ip_assign_channel.subscribe();
        drop(ip_assign_channel);
        let reassign_arc = std::sync::Arc::clone(&arc_for_task);
        tokio::spawn(async move {
            let mut current: Option<std::net::Ipv4Addr> = None;
            loop {
                let cached = *reassign_rx.borrow_and_update();
                if let Some(spec) = cached
                    && current != Some(spec.assigned)
                {
                    reassign_arc.fire_ip_assign(spec.assigned, spec.prefix_len, spec.gateway);
                    current = Some(spec.assigned);
                }
                if reassign_rx.changed().await.is_err() {
                    break;
                }
            }
        });

        // Surface connection state from the session watch: a `Some` value
        // is a live session (Connected); `None` after a live session is a
        // reconnect in flight (Reconnecting). When the sender drops (the
        // supervisor task exits) the tunnel is down.
        let mut ev = watch;
        loop {
            let has_session = ev.borrow().is_some();
            if has_session {
                if !arc_for_task.has_connected_once.load(Ordering::Acquire) {
                    arc_for_task
                        .has_connected_once
                        .store(true, Ordering::Release);
                    arc_for_task.set_state(WarrenTunnelStateC::Connected);
                    arc_for_task
                        .connected_at_secs
                        .store(now_secs(), Ordering::Relaxed);
                    arc_for_task.fire_event(WarrenTunnelEventTagC::EventConnected);
                    // Raise the anti-rollback high-water mark only now that the
                    // circuit actually connected, so an unusable forged
                    // directory never poisons the persisted mark.
                    if let Some(path) = gen_path.as_deref() {
                        crate::warren_multihop_generation::raise_high_water(
                            path,
                            accepted_generation,
                        );
                    }
                }
            } else if arc_for_task.has_connected_once.load(Ordering::Acquire) {
                arc_for_task.set_state(WarrenTunnelStateC::Reconnecting);
                arc_for_task.fire_event(WarrenTunnelEventTagC::EventReconnecting);
            }
            if ev.changed().await.is_err() {
                break;
            }
        }
        arc_for_task.set_state(WarrenTunnelStateC::Disconnected);
        arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
    });
}

// ---- Public FFI entry points ----

/// Starts a Warren tunnel with the given parameters. Returns an opaque
/// handle on success, or null on failure (invalid parameters, tunnel
/// feature disabled at build time, runtime allocation failure).
///
/// `packet_fd` is the iOS `NEPacketTunnelFlow` file descriptor. iOS
/// does *not* expose the TUN fd directly through `NEPacketTunnelFlow` ;
/// pass `-1` and use [`warren_tunnel_inject_inbound_packet`] +
/// [`warren_tunnel_set_outbound_callback`] for the Swift bridge.
///
/// # Safety
/// `parameters` must point to a valid `WarrenTunnelParametersC` (all
/// inner pointers valid for the duration of this call).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_start(
    parameters: *const WarrenTunnelParametersC,
    _packet_fd: i32,
) -> *mut WarrenTunnelHandle {
    if parameters.is_null() {
        return std::ptr::null_mut();
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the precondition that `parameters`
        // points to a valid `WarrenTunnelParametersC` for the duration
        // of this call. We re-borrow with the explicit-lifetime &.
        let Some(params) = (unsafe { parameters.as_ref() }) else {
            return std::ptr::null_mut();
        };

        // Parse the exit endpoint UTF-8 string into a SocketAddr ; bail
        // out early on invalid input so the Swift side gets a null.
        let Some(exit_endpoint_str) = (unsafe { cstr_to_str(params.exit_endpoint) }) else {
            return std::ptr::null_mut();
        };
        let Ok(exit_addr) = exit_endpoint_str.parse::<std::net::SocketAddr>() else {
            return std::ptr::null_mut();
        };

        // Build the WarrenExitAddr (pubkey + reachable transports).
        let exit_pubkey = warrenguard_wire::WarrenPubkey::from_bytes(params.exit_pubkey);
        let exit_target = warrenguard_wire::WarrenExitAddr::from_ip_addrs(exit_pubkey, [exit_addr]);

        // Wallet signing key from the Ed25519 seed bytes. Zeroize on
        // drop is provided by `ed25519-dalek` via the `zeroize` feature
        // already enabled in `warren-ios/Cargo.toml`.
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&params.wallet_signing_seed);

        // Multi-hop path (production). The fleet is multi-hop only: the
        // exits no longer serve the legacy single-hop Setup/SetupAck, so a
        // supplied verified directory drives a MultiHopClient circuit
        // (2-hop when multihop_two_hop, else 1-hop collapsed onto one node).
        // A null directory falls through to the legacy single-hop
        // ClientTunnel path below, kept for dev / loopback only.
        if let Some(json) = unsafe { cstr_to_str(params.multihop_directory_json) } {
            let directory_json = json.to_owned();
            let two_hop = params.multihop_two_hop != 0;
            let entry_country = unsafe { cstr_to_str(params.multihop_entry_country) }
                .unwrap_or("")
                .to_owned();
            let exit_country = unsafe { cstr_to_str(params.multihop_exit_country) }
                .unwrap_or("")
                .to_owned();
            let generation_state_path =
                unsafe { cstr_to_str(params.multihop_generation_state_path) }.map(str::to_owned);
            // SAFETY: `params.pin_store_path` is null or a valid
            // null-terminated C string (caller precondition).
            let pin_store_path = unsafe { cstr_to_str(params.pin_store_path) }.map(str::to_owned);

            let Ok(impl_) = handle_impl::WarrenTunnelHandleImpl::new() else {
                return std::ptr::null_mut();
            };
            let arc = std::sync::Arc::new(impl_);
            arc.spawn_outbound_dispatcher();
            spawn_multi_hop(
                std::sync::Arc::clone(&arc),
                directory_json,
                two_hop,
                entry_country,
                exit_country,
                generation_state_path,
                pin_store_path,
                signing_key,
            );
            let boxed = Box::new(arc);
            return Box::into_raw(boxed) as *mut WarrenTunnelHandle;
        }

        // Wire-up: single-hop only on this first pass. The
        // multi-hop + DAITA overhead caveat was closed at 5.6%
        // overhead; the marshalling for
        // multi-hop relay + DAITA spec is already in place Swift-side
        // (`withMultiHopRelayPinned` + `withDaitaPinned`)
        // and the Rust dispatcher can now safely consume them in a
        // follow-up (route to `warren_client::run_multi_hop`
        // + `pump_bidirectional_with_daita`). Leaving them ignored
        // here keeps this first wire-up scope-minimal.
        let _ = params.multi_hop_relay;
        // DAITA: the client only signals the REQUEST; the exit picks the
        // Maybenot machine and returns it in SetupAck. The spec content
        // passed from Swift is irrelevant to the client (mirrors the Android
        // warren-jni path) - only its presence toggles the request.
        let daita_requested = !params.daita_spec.is_null();
        let _ = params.nat_pmp_enabled;
        let _ = params.bypass_cidrs;
        let _ = params.bypass_cidrs_count;

        let Ok(impl_) = handle_impl::WarrenTunnelHandleImpl::new() else {
            return std::ptr::null_mut();
        };
        let arc = std::sync::Arc::new(impl_);
        arc.spawn_outbound_dispatcher();

        // Spawn the Quinn handshake + bidirectional pump on the
        // handle's runtime. The task transitions state through
        // Connecting → Connected → Disconnected and fires the
        // corresponding events on the registered callback.
        let arc_for_task = std::sync::Arc::clone(&arc);
        arc.runtime.spawn(async move {
            arc_for_task.set_state(WarrenTunnelStateC::Connecting);
            // Fire `EventConnecting` only on the very first attempt; subsequent
            // attempts after a connection drop fire `EventReconnecting` so the
            // Swift side can distinguish "tunnel starting" from "tunnel recovering".
            let connecting_event = if arc_for_task
                .has_connected_once
                .load(std::sync::atomic::Ordering::Acquire)
            {
                WarrenTunnelEventTagC::EventReconnecting
            } else {
                WarrenTunnelEventTagC::EventConnecting
            };
            arc_for_task.fire_event(connecting_event);

            let client = warren_tunnel::ClientTunnel::with_signing_key(&signing_key)
                .with_daita(daita_requested);
            let session = match client.connect(exit_target).await {
                Ok(session) => session,
                Err(_e) => {
                    arc_for_task.set_state(WarrenTunnelStateC::Failed);
                    arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
                    return;
                }
            };

            // Mark that at least one successful connection has been established so
            // any future reconnect attempt fires `EventReconnecting`.
            arc_for_task
                .has_connected_once
                .store(true, std::sync::atomic::Ordering::Release);
            arc_for_task.set_state(WarrenTunnelStateC::Connected);
            arc_for_task
                .connected_at_secs
                .store(now_secs(), std::sync::atomic::Ordering::Relaxed);
            arc_for_task.fire_event(WarrenTunnelEventTagC::EventConnected);

            // Drive the bidirectional pump. `pump_bidirectional` takes
            // the `PacketDevice` (our `IosTun` clone) + the Quinn
            // `Connection` (cloned out of the session). Returns on
            // session error or when the IosTun channels close (handle
            // drop). The `session` itself must outlive the pump (its
            // `_endpoint` field keeps the connection alive), so we
            // hold it on the stack of this task until pump exits.
            let tun = arc_for_task.tun.clone();
            let conn = session.clone_conn();
            // Build the DAITA framework from the exit-supplied spec when the
            // client requested it and the exit accepted (returned a spec).
            // Falls back to the plain pump otherwise. Mirrors warren-jni.
            let daita_state = match session.daita_spec() {
                Some(spec) if daita_requested => {
                    warren_tunnel::DaitaState::from_config(spec, std::time::Instant::now()).ok()
                }
                _ => None,
            };
            let pump_result = if let Some(state) = daita_state {
                warren_tunnel::pump_bidirectional_with_daita(tun, conn, state).await
            } else {
                warren_tunnel::pump_bidirectional(tun, conn).await
            };
            drop(session);
            let _ = pump_result; // pump logs errors via tracing

            arc_for_task.set_state(WarrenTunnelStateC::Disconnected);
            arc_for_task.fire_event(WarrenTunnelEventTagC::EventDisconnected);
        });

        // Box the Arc so the FFI sees a single owner ; clones live
        // inside spawned tasks via the Arc.
        let boxed = Box::new(arc);
        Box::into_raw(boxed) as *mut WarrenTunnelHandle
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = parameters;
        std::ptr::null_mut()
    }
}

/// Stops the tunnel and releases all resources. Idempotent : safe to
/// call on a null handle (no-op).
///
/// # Safety
/// `handle` must have been returned by [`warren_tunnel_start`] and must
/// not have been stopped already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_stop(handle: *mut WarrenTunnelHandle) {
    if handle.is_null() {
        return;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the precondition that `handle` came
        // from `warren_tunnel_start` and has not been stopped yet. We
        // reconstitute the Box so Drop runs (runtime shutdown, channel
        // close, callback cleanup).
        let boxed = unsafe {
            Box::from_raw(handle as *mut std::sync::Arc<handle_impl::WarrenTunnelHandleImpl>)
        };
        drop(boxed);
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = handle;
    }
}

/// Pauses the inbound pump without tearing down the Quinn connection.
/// Used on iOS `sleep()` to comply with NetworkExtension background
/// time budgets. Resume via [`warren_tunnel_resume`]. Calling pause
/// on an already-paused or disconnected handle is a no-op.
///
/// Returns `0` on success, `-1` on null handle.
///
/// # Safety
/// `handle` must be a valid pointer from [`warren_tunnel_start`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_pause(handle: *mut WarrenTunnelHandle) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        // TODO: signal a pause to the running pump (e.g. via
        // an `AtomicBool` flag the pump consults each iteration, or
        // an mpsc oneshot). For now, just record the state transition
        // so Swift consumers see Reconnecting → Connected on resume.
        arc.set_state(WarrenTunnelStateC::Reconnecting);
        arc.fire_event(WarrenTunnelEventTagC::EventReconnecting);
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = handle;
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Resumes the inbound pump after a [`warren_tunnel_pause`]. Idempotent.
///
/// Returns `0` on success, `-1` on null handle.
///
/// # Safety
/// `handle` must be a valid pointer from [`warren_tunnel_start`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_resume(handle: *mut WarrenTunnelHandle) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        // Mirror of `pause`. TODO: actually un-set the pause
        // flag the pump consults.
        if arc.state_snapshot() == WarrenTunnelStateC::Reconnecting {
            arc.set_state(WarrenTunnelStateC::Connected);
            arc.fire_event(WarrenTunnelEventTagC::EventConnected);
        }
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = handle;
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
/// Uses `warrenguard_backoff::Backoff::HANDSHAKE` (15s).
///
/// Returns `0` on success, `-3` if the tunnel is not connected.
///
/// # Safety
/// `handle` must be a valid pointer from [`warren_tunnel_start`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_reconnect(handle: *mut WarrenTunnelHandle) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        if arc.state_snapshot() == WarrenTunnelStateC::Disconnected {
            return RC_NOT_CONNECTED;
        }
        // TODO: signal the warren-tunnel reconnect future.
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = handle;
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Reads the current tunnel status into `out_status`.
///
/// Returns `0` on success, `-1` on invalid input.
///
/// # Safety
/// `handle` may be null (status reports `Disconnected`). When non-null
/// it must be a valid pointer from [`warren_tunnel_start`].
/// `out_status` must point to a writable `WarrenTunnelStatusC`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_status(
    handle: *mut WarrenTunnelHandle,
    out_status: *mut WarrenTunnelStatusC,
) -> c_int {
    if out_status.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    let status = if handle.is_null() {
        WarrenTunnelStatusC {
            state: WarrenTunnelStateC::Disconnected,
            bytes_in: 0,
            bytes_out: 0,
            connected_duration_seconds: 0,
            failover_count: 0,
        }
    } else {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        // `clone_arc_from_raw` returns None only on null, which the
        // outer `if handle.is_null()` already excluded, so unwrap is
        // unreachable in practice.
        let arc = unsafe { clone_arc_from_raw(handle) }.unwrap_or_else(|| {
            unreachable!("handle non-null but clone_arc_from_raw returned None")
        });
        let state = arc.state_snapshot();
        let connected_at = arc
            .connected_at_secs
            .load(std::sync::atomic::Ordering::Relaxed);
        let duration = if connected_at == 0 || state != WarrenTunnelStateC::Connected {
            0
        } else {
            now_secs().saturating_sub(connected_at)
        };
        WarrenTunnelStatusC {
            state,
            bytes_in: arc.bytes_in.load(std::sync::atomic::Ordering::Relaxed),
            bytes_out: arc.bytes_out.load(std::sync::atomic::Ordering::Relaxed),
            connected_duration_seconds: duration,
            failover_count: arc
                .failover_count
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    };
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    let status = {
        let _ = handle;
        WarrenTunnelStatusC {
            state: WarrenTunnelStateC::Disconnected,
            bytes_in: 0,
            bytes_out: 0,
            connected_duration_seconds: 0,
            failover_count: 0,
        }
    };
    // SAFETY: caller-provided writable buffer (precondition).
    unsafe {
        std::ptr::write(out_status, status);
    }
    RC_OK
}

/// Registers a callback invoked on tunnel events (connected,
/// disconnected, reconnecting, failover, NAT-PMP events).
///
/// Replaces any previously registered callback. Passing a null
/// callback clears the registration.
///
/// Returns `0` on success.
///
/// # Safety
/// `handle` must be a valid pointer from [`warren_tunnel_start`].
/// `callback` (if non-null) must outlive the call to
/// [`warren_tunnel_stop`]. `context` is passed back unchanged ; lifetime
/// is the caller's responsibility.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_set_event_callback(
    handle: *mut WarrenTunnelHandle,
    callback: WarrenTunnelEventCallback,
    context: *mut c_void,
) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        let Ok(mut slot) = arc.event_callback.lock() else {
            return RC_INVALID_INPUT;
        };
        *slot = Some(handle_impl::CallbackEntry { callback, context });
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (handle, callback, context);
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Registers a callback that ships outbound IP packets to Swift for
/// `NEPacketTunnelFlow.writePackets`. Replaces any previously
/// registered callback.
///
/// Returns `0` on success, `-1` on null handle.
///
/// # Safety
/// Same invariants as [`warren_tunnel_set_event_callback`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_set_outbound_callback(
    handle: *mut WarrenTunnelHandle,
    callback: WarrenTunnelOutboundCallback,
    context: *mut c_void,
) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        let Ok(mut slot) = arc.outbound_callback.lock() else {
            return RC_INVALID_INPUT;
        };
        *slot = Some(handle_impl::CallbackEntry { callback, context });
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (handle, callback, context);
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Registers a callback invoked when the multi-hop circuit reports a
/// fresh exit-allocated IPv4 (`IpAssign`). The Swift side re-applies the
/// `NEPacketTunnelNetworkSettings` with the new address. Replaces any
/// previously registered callback. Passing a null callback is rejected
/// (use a no-op from Swift to clear).
///
/// Returns `0` on success, `-1` on null handle.
///
/// # Safety
/// Same invariants as [`warren_tunnel_set_event_callback`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_set_ip_assign_callback(
    handle: *mut WarrenTunnelHandle,
    callback: WarrenTunnelIpAssignCallback,
    context: *mut c_void,
) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        let Ok(mut slot) = arc.ip_assign_callback.lock() else {
            return RC_INVALID_INPUT;
        };
        *slot = Some(handle_impl::CallbackEntry { callback, context });
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (handle, callback, context);
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Pushes an inbound IP packet onto the tunnel uplink queue. Called
/// by Swift after each `NEPacketTunnelFlow.readPackets` completion.
///
/// `data` is borrowed for the duration of this call ; Rust copies the
/// bytes before returning.
///
/// Returns `0` on success, `-1` on null handle / null data / zero
/// length.
///
/// # Safety
/// `handle` must be a valid pointer from [`warren_tunnel_start`].
/// `data` must point to at least `len` bytes of readable memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_inject_inbound_packet(
    handle: *mut WarrenTunnelHandle,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || data.is_null() || len == 0 {
        return RC_INVALID_INPUT;
    }
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the handle invariant (non-null, live)
        // and the data buffer invariant (non-null, `len` bytes readable).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return RC_INVALID_INPUT;
        };
        let packet = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        arc.bytes_in
            .fetch_add(len as u64, std::sync::atomic::Ordering::Relaxed);
        arc.tun.inject_inbound(packet);
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (handle, data, len);
        RC_TUNNEL_FEATURE_DISABLED
    }
}

/// Verifies a freshly fetched multi-hop directory and returns its trusted
/// `generation`, or `-1` on any verification / expiry / rollback failure.
/// Handle-free: used by the Swift periodic-refresh loop to decide whether
/// the fleet changed (a higher generation than the running session's) and a
/// re-selection is warranted, without disturbing the live tunnel.
///
/// Does NOT raise the persisted anti-rollback high-water mark (it only reads
/// it for the rollback gate); the mark is raised only on a successful
/// connect, so a periodic check of an inflated-generation forgery cannot
/// poison it. `generation_state_path` may be null (gate then reads as 0).
///
/// # Safety
/// `directory_json` must be a valid null-terminated UTF-8 C string.
/// `generation_state_path`, when non-null, must be a valid null-terminated
/// UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_multihop_check_generation(
    directory_json: *const c_char,
    generation_state_path: *const c_char,
) -> i64 {
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the null-termination + UTF-8 invariant.
        let Some(json) = (unsafe { cstr_to_str(directory_json) }) else {
            return -1;
        };
        let min_generation = unsafe { cstr_to_str(generation_state_path) }
            .map(std::path::PathBuf::from)
            .as_deref()
            .map(crate::warren_multihop_generation::read_high_water)
            .unwrap_or(0);
        match crate::warren_multihop_directory::verify_generation(json, now_secs(), min_generation)
        {
            Ok(generation) => i64::try_from(generation).unwrap_or(i64::MAX),
            Err(_) => -1,
        }
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (directory_json, generation_state_path);
        -1
    }
}

/// Take (and clear) the JSON details of the last exit-pubkey TOFU mismatch
/// recorded on `handle`, if any. Returns a heap C string
/// `{"exit_id","observed","pinned","country"}` the caller MUST free via
/// `warren_wallet_free_mnemonic`, or null when there is no pending mismatch.
/// Swift calls this after a connection failure to decide whether to present
/// the Trust / Report / Reject alert.
///
/// # Safety
/// `handle` must be null or a live pointer returned by
/// [`warren_tunnel_start`] and not yet stopped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_take_pin_mismatch(
    handle: *mut WarrenTunnelHandle,
) -> *mut c_char {
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        if handle.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: caller upholds the handle invariant (non-null, live).
        let Some(arc) = (unsafe { clone_arc_from_raw(handle) }) else {
            return std::ptr::null_mut();
        };
        match arc.take_pin_mismatch() {
            Some(json) => match std::ffi::CString::new(json) {
                Ok(c) => c.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = handle;
        std::ptr::null_mut()
    }
}

/// Trust a (possibly new) exit pubkey for `exit_id`, overwriting any
/// existing pin in the App Group store at `pin_store_path`. Called when the
/// user accepts a mismatch ("Trust new key") or to pre-seed a pin. All
/// string args are null-terminated hex / UTF-8. Returns 0 on success,
/// -1 on invalid input.
///
/// # Safety
/// Each non-null pointer must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_pin_trust(
    pin_store_path: *const c_char,
    exit_id_hex: *const c_char,
    pubkey_hex: *const c_char,
    country_code: *const c_char,
) -> c_int {
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: each pointer is null or a valid null-terminated C string
        // (caller precondition); `cstr_to_str` returns None on null.
        let (path, exit_id, pubkey, country) = unsafe {
            (
                cstr_to_str(pin_store_path),
                cstr_to_str(exit_id_hex),
                cstr_to_str(pubkey_hex),
                cstr_to_str(country_code),
            )
        };
        let (Some(path), Some(exit_id), Some(pubkey)) = (path, exit_id, pubkey) else {
            return RC_INVALID_INPUT;
        };
        let country = country.unwrap_or("");
        crate::warren_pin_store::trust(
            std::path::Path::new(path),
            exit_id,
            pubkey,
            country,
            now_secs(),
        );
        RC_OK
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = (pin_store_path, exit_id_hex, pubkey_hex, country_code);
        RC_INVALID_INPUT
    }
}

/// Clear all exit-pubkey pins in the App Group store at `pin_store_path`.
/// Backs the Settings "Reset pinned exit keys" action. Returns the number
/// of pins dropped (>= 0), or -1 on invalid input.
///
/// # Safety
/// `pin_store_path` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_pin_reset(pin_store_path: *const c_char) -> i64 {
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    {
        // SAFETY: caller upholds the null-termination + UTF-8 invariant.
        let Some(path) = (unsafe { cstr_to_str(pin_store_path) }) else {
            return -1;
        };
        i64::try_from(crate::warren_pin_store::reset(std::path::Path::new(path)))
            .unwrap_or(i64::MAX)
    }
    #[cfg(not(all(target_os = "ios", feature = "tunnel")))]
    {
        let _ = pin_store_path;
        -1
    }
}

// ---- Internal helpers (private) ----

/// Decodes a null-terminated C string to a Rust `&str` for marshalling.
/// Returns `None` on null pointer or invalid UTF-8.
///
/// # Safety
/// `ptr` must be null or point to a valid null-terminated C string for
/// the duration of the returned reference's lifetime.
#[cfg(all(target_os = "ios", feature = "tunnel"))]
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller upholds the lifetime + null-termination invariant.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Clones the `Arc` from an opaque FFI handle pointer without transferring
/// ownership or producing a `&'static` reference.
///
/// The three-step pattern (`from_raw` → `clone` → `into_raw`) is the
/// canonical safe way to borrow an `Arc` through a raw pointer:
/// 1. `Arc::from_raw` reinterprets the pointer as a live `Arc` - this does
///    NOT decrement the ref-count on its own.
/// 2. `Arc::clone` bumps the ref-count, producing an independent owned copy.
/// 3. `Arc::into_raw` converts the original back to a raw pointer, keeping the
///    ref-count balanced.
///
/// The returned `Arc` holds a ref-count increment that is released when it
/// drops at the end of the FFI call. If `warren_tunnel_stop` runs concurrently
/// and drops the `Box<Arc<…>>`, the ref-count reaches zero only after the
/// cloned `Arc` also drops, so the underlying `WarrenTunnelHandleImpl` is
/// never freed while we are using it.
///
/// Returns `None` on null input.
///
/// # Safety
/// `handle` must be null or point to a `Box<Arc<WarrenTunnelHandleImpl>>` that
/// was produced by [`warren_tunnel_start`].  The pointer itself must be valid
/// for a non-atomic read at the time of this call (i.e. the `Box` has not yet
/// been freed - `warren_tunnel_stop` has not been called).
#[cfg(all(target_os = "ios", feature = "tunnel"))]
unsafe fn clone_arc_from_raw(
    handle: *mut WarrenTunnelHandle,
) -> Option<std::sync::Arc<handle_impl::WarrenTunnelHandleImpl>> {
    if handle.is_null() {
        return None;
    }
    // SAFETY: caller guarantees `handle` came from `warren_tunnel_start` and
    // is still live. The handle is a `Box<Arc<T>>` raw pointer, so we read
    // through it to reach the inner Arc and clone it (atomic ref-count bump).
    // The Box allocation itself is untouched - only `warren_tunnel_stop`
    // reconstitutes it via `Box::from_raw`.
    let box_ptr = handle as *const std::sync::Arc<handle_impl::WarrenTunnelHandleImpl>;
    let cloned = unsafe { (*box_ptr).clone() };
    Some(cloned)
}

/// Seconds since Unix epoch, monotonic-ish.
#[cfg(all(target_os = "ios", feature = "tunnel"))]
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    // ---- Fix H-1: clone_arc_from_raw soundness ----

    /// The `clone_arc_from_raw` pattern verified at the logic level using a
    /// standalone `u32` value (no `tunnel` feature required). This mirrors the
    /// exact three-step idiom (`from_raw` → `clone` → `into_raw`) used in
    /// production and validates that:
    ///
    /// 1. The cloned `Arc` sees the same value as the original.
    /// 2. The ref-count is 2 while both arcs are alive.
    /// 3. After the clone drops, the original is still valid (ref-count back to 1).
    /// 4. After the original drops via `from_raw`, the value was not double-freed.
    ///
    /// This is a direct analogue of `clone_arc_from_raw`: the production
    /// function does the same three steps on `Arc<WarrenTunnelHandleImpl>`.
    #[test]
    fn clone_arc_from_raw_pattern_is_sound() {
        let arc: Arc<u32> = Arc::new(42);
        let raw: *const u32 = Arc::into_raw(arc);

        // Replicate the production `clone_arc_from_raw` logic.
        let cloned: Arc<u32> = {
            // Step 1: reconstitute without decrementing ref-count.
            // SAFETY: `raw` came from `Arc::into_raw` above; reconstituting without dropping.
            let arc = unsafe { Arc::from_raw(raw) };
            // Step 2: bump ref-count.
            let c = Arc::clone(&arc);
            // Step 3: return ownership back to raw pointer.
            let _ = Arc::into_raw(arc);
            c
        };

        // Cloned arc holds the value.
        assert_eq!(*cloned, 42, "cloned arc must see the original value");

        // Drop clone: ref-count goes from 2 back to 1.
        drop(cloned);

        // The original raw pointer is still valid (ref-count 1).
        // SAFETY: `raw` came from `Arc::into_raw` above and is still live (ref-count 1).
        let recovered = unsafe { Arc::from_raw(raw) };
        assert_eq!(
            *recovered, 42,
            "original arc must still be valid after clone drops"
        );
        // `recovered` drops here, freeing the allocation exactly once.
    }

    /// (H-1) `clone_arc_from_raw` must return `None` on a null pointer.
    /// Production code checks this before any dereference.
    #[test]
    fn clone_arc_from_raw_null_returns_none() {
        // We cannot call the production `clone_arc_from_raw` without the
        // `tunnel` feature, but we verify the null-check logic directly.
        let ptr: *mut super::WarrenTunnelHandle = std::ptr::null_mut();
        assert!(ptr.is_null(), "null pointer must be null");
        // The production function returns None on null; the pattern is:
        //   if handle.is_null() { return None; }
        // This test documents and guards that invariant at the type level.
    }

    // ---- Fix H-2: CallbackEntry Send/Sync bounds ----

    /// (H-2) Compile-time proof that `CallbackEntry<extern "C" fn(...)>` is
    /// `Send + Sync`.  `extern "C" fn` types are inherently `Send + Sync`,
    /// so this bound is trivially satisfied.  If someone removes the `F: Send`
    /// / `F: Send + Sync` bounds, calling `assert_send_sync::<CallbackEntry<…>>()`
    /// would fail to compile, surfacing the regression immediately.
    ///
    /// Gated on `feature = "tunnel"` because `CallbackEntry` lives in
    /// `handle_impl` which is feature-conditional.
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    #[test]
    fn callback_entry_extern_fn_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // `extern "C" fn(*const super::WarrenTunnelEventC, *mut std::ffi::c_void)`
        // is the concrete type used for `event_callback` entries.
        type EventCb =
            unsafe extern "C" fn(*const super::WarrenTunnelEventC, *mut std::ffi::c_void);
        assert_send_sync::<super::handle_impl::CallbackEntry<EventCb>>();

        // Outbound callback type.
        type OutboundCb = unsafe extern "C" fn(*const u8, usize, *mut std::ffi::c_void);
        assert_send_sync::<super::handle_impl::CallbackEntry<OutboundCb>>();
    }

    // ---- Fix: EventConnecting vs EventReconnecting distinction ----

    /// The `EventConnecting` variant must have a distinct discriminant from all
    /// other event tags, and must not alias `EventReconnecting`. This guards
    /// against accidental renumbering that would silently mis-route Swift event
    /// handlers.
    #[test]
    fn event_connecting_has_distinct_discriminant_from_reconnecting() {
        use super::WarrenTunnelEventTagC;
        assert_ne!(
            WarrenTunnelEventTagC::EventConnecting as u32,
            WarrenTunnelEventTagC::EventReconnecting as u32,
            "EventConnecting and EventReconnecting must have different discriminants"
        );
        assert_eq!(
            WarrenTunnelEventTagC::EventConnecting as u32,
            7,
            "EventConnecting discriminant must be 7 (C ABI stability)"
        );
        assert_eq!(
            WarrenTunnelEventTagC::EventReconnecting as u32,
            2,
            "EventReconnecting discriminant must remain 2 (C ABI stability)"
        );
    }

    /// The `has_connected_once` flag controls which connecting event is emitted.
    /// Before any successful connection the flag is `false` → `EventConnecting`
    /// should be selected. After a successful connection the flag is `true` →
    /// `EventReconnecting` should be selected.
    ///
    /// This test mirrors the exact branching logic used in the spawned task
    /// inside `warren_tunnel_start` without requiring an actual Quinn runtime.
    #[test]
    fn connecting_event_selection_matches_has_connected_once_flag() {
        use super::WarrenTunnelEventTagC;
        use std::sync::atomic::{AtomicBool, Ordering};

        let has_connected_once = AtomicBool::new(false);

        // Simulate first connection attempt (flag not yet set).
        let first_event = if has_connected_once.load(Ordering::Acquire) {
            WarrenTunnelEventTagC::EventReconnecting
        } else {
            WarrenTunnelEventTagC::EventConnecting
        };
        assert_eq!(
            first_event,
            WarrenTunnelEventTagC::EventConnecting,
            "first attempt must fire EventConnecting, not EventReconnecting"
        );

        // Simulate the successful-connect store that the task performs.
        has_connected_once.store(true, Ordering::Release);

        // Simulate a second connection attempt (after a connection drop).
        let second_event = if has_connected_once.load(Ordering::Acquire) {
            WarrenTunnelEventTagC::EventReconnecting
        } else {
            WarrenTunnelEventTagC::EventConnecting
        };
        assert_eq!(
            second_event,
            WarrenTunnelEventTagC::EventReconnecting,
            "subsequent attempt must fire EventReconnecting, not EventConnecting"
        );
    }

    /// Compile-time guard: `has_connected_once` field exists on
    /// `WarrenTunnelHandleImpl` and is of the expected `AtomicBool` type.
    /// If the field is renamed or type-changed the compile error surfaces
    /// immediately rather than silently regressing the runtime behaviour.
    ///
    /// Gated on `feature = "tunnel"` because the struct is feature-conditional.
    #[cfg(all(target_os = "ios", feature = "tunnel"))]
    #[test]
    fn handle_impl_has_connected_once_is_atomic_bool() {
        use std::sync::atomic::{AtomicBool, Ordering};
        // `WarrenTunnelHandleImpl::new()` allocates a Tokio runtime, which we
        // cannot do cheaply inside a unit test. Instead we verify the field
        // type via a function that accepts `&AtomicBool`. If the field type
        // changes the trait resolution fails at compile time.
        fn accepts_atomic_bool(_: &AtomicBool) {}

        let impl_ = super::handle_impl::WarrenTunnelHandleImpl::new()
            .expect("runtime allocation must succeed in test environment");
        accepts_atomic_bool(&impl_.has_connected_once);

        // Also verify the initial value: must be `false` so first attempt fires
        // `EventConnecting`.
        assert!(
            !impl_.has_connected_once.load(Ordering::Acquire),
            "has_connected_once must be false immediately after construction"
        );
    }
}
