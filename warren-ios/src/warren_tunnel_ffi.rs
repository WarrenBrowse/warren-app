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
//! callback registration. The Quinn connection itself is wired in C.4.1
//! (cf. `.planning/c4-packet-tunnel-provider-quinn-design.md` §3-§4).
//!
//! The handle lifecycle:
//! - `warren_tunnel_start` boxes the impl, returns the raw pointer.
//! - `warren_tunnel_stop` reconstitutes the box via [`Box::from_raw`]
//!   so Drop runs (Tokio runtime, channels, callback cleanup).
//! - All other entry points cast the raw pointer back to `&mut Impl`
//!   without taking ownership.

use std::ffi::c_void;
#[cfg(feature = "tunnel")]
use std::ffi::CStr;
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
    /// (see M4.H.G `--bypass-cidr`). Length given by `bypass_cidrs_count`.
    pub bypass_cidrs: *const *const c_char,
    /// Number of entries in `bypass_cidrs`.
    pub bypass_cidrs_count: u32,
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

/// DAITA defensive shaping spec (cf. memory `warren_session_b_delivered`
/// M5.B.1 / `warren_daita_doctrine_v1`).
#[repr(C)]
pub struct WarrenDaitaSpecC {
    /// 32-byte Maybenot machine seed.
    pub machine_seed: [u8; 32],
    /// Padding budget in packets/sec.
    pub padding_pps: u32,
}

/// Tunnel state enum surfaced via `warren_tunnel_status`. Variants
/// other than `Disconnected` are constructed by the warren-tunnel
/// dispatcher once the Quinn connection task is wired (C.4.1). On the
/// `not(tunnel)` feature path the enum has no constructor at all,
/// hence the `cfg_attr(expect)` suppression scoped to that path.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(feature = "tunnel"),
    expect(dead_code, reason = "FFI surface ; only constructed on tunnel feature path")
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
    /// Cumulative failover count this session (cf. M5.B.2).
    pub failover_count: u32,
}

/// Event tag for the variant union below. Variants are constructed by
/// the warren-tunnel dispatcher once the Quinn connection task is
/// wired (C.4.1).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(dead_code, reason = "FFI surface ; variants constructed C.4.1")]
pub enum WarrenTunnelEventTagC {
    Connected = 0,
    Disconnected = 1,
    Reconnecting = 2,
    Failover = 3,
    NatPmpMapped = 4,
    NatPmpRenewed = 5,
    NatPmpFailed = 6,
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

// ---- Return codes shared by all tunnel FFI fns ----

const RC_OK: c_int = 0;
const RC_INVALID_INPUT: c_int = -1;
// RC_NOT_CONNECTED is used only on the `tunnel` feature path ; on the
// `not(tunnel)` path it is dead code (the FFI returns the disabled
// sentinel instead). `cfg_attr(...expect(dead_code))` triggers the
// expectation exactly on the path where the lint fires, leaving the
// `tunnel` path free of suppression noise.
#[cfg_attr(not(feature = "tunnel"), expect(dead_code, reason = "feature-conditional"))]
const RC_NOT_CONNECTED: c_int = -3;
// RC_TUNNEL_FEATURE_DISABLED is the mirror of RC_NOT_CONNECTED for the
// `not(tunnel)` path ; on the `tunnel` path it is dead code.
#[cfg_attr(feature = "tunnel", expect(dead_code, reason = "feature-conditional"))]
const RC_TUNNEL_FEATURE_DISABLED: c_int = -10;

// ---- Handle implementation (feature-gated) ----

#[cfg(feature = "tunnel")]
mod handle_impl {
    //! Real handle implementation when the `tunnel` feature is on.
    //!
    //! Owns the Tokio runtime + [`warren_tunnel::IosTun`] bridge +
    //! callback registration. The Quinn connection task is spawned
    //! lazily in [`WarrenTunnelHandleImpl::run`] (TODO C.4.1).

    use super::{
        WarrenTunnelEventCallback, WarrenTunnelOutboundCallback, WarrenTunnelStateC,
    };
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
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

    // SAFETY: the opaque `context` pointer is provided by the Swift
    // side and is treated as a black-box value. We never deref it from
    // Rust ; we only pass it back as-is to the Swift callback. Swift
    // is responsible for the pointer's lifetime + thread-safety. The
    // [`Send`]/[`Sync`] markers below are needed so the entry can live
    // inside a [`Mutex`] shared across Tokio tasks.
    unsafe impl<F> Send for CallbackEntry<F> {}
    unsafe impl<F> Sync for CallbackEntry<F> {}

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
        /// Atomic counters surfaced through
        /// [`super::warren_tunnel_status`]. Atomics avoid contention
        /// with the pump tasks while keeping the status read O(1).
        pub state: AtomicU8, // stores discriminant of WarrenTunnelStateC
        pub bytes_in: AtomicU64,
        pub bytes_out: AtomicU64,
        pub connected_at_secs: AtomicU64, // 0 until first Connected event
        pub failover_count: AtomicU32,
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
                state: AtomicU8::new(WarrenTunnelStateC::Disconnected as u8),
                bytes_in: AtomicU64::new(0),
                bytes_out: AtomicU64::new(0),
                connected_at_secs: AtomicU64::new(0),
                failover_count: AtomicU32::new(0),
            })
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
            let Ok(cb_entry) = self.event_callback.lock() else { return };
            let Some(entry) = cb_entry.as_ref() else { return };
            // SAFETY: callback + context come from Swift via
            // `warren_tunnel_set_event_callback`. Swift owns the
            // context pointer lifecycle ; we just pass it back. The
            // event pointer is valid for the duration of the call.
            unsafe {
                (entry.callback)(&raw const event, entry.context);
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
    #[cfg(feature = "tunnel")]
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
        let exit_pubkey = warren_protocol::WarrenPubkey::from_bytes(params.exit_pubkey);
        let exit_target =
            warren_protocol::WarrenExitAddr::from_ip_addrs(exit_pubkey, [exit_addr]);

        // Wallet signing key from the Ed25519 seed bytes. Zeroize on
        // drop is provided by `ed25519-dalek` via the `zeroize` feature
        // already enabled in `warren-ios/Cargo.toml`.
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&params.wallet_signing_seed);

        // C.4.1 wire-up : single-hop only on this first pass. The
        // multi-hop + DAITA bug flagged in memory `warren_session_n`
        // was resolved by Session R (warren-core `f8f2d59`, B.1.8
        // caveat closed at 5.6% overhead) ; the marshalling for
        // multi-hop relay + DAITA spec is already in place Swift-side
        // (cf. C.4.2 `withMultiHopRelayPinned` + `withDaitaPinned`)
        // and the Rust dispatcher can now safely consume them in a
        // C.4.1.X follow-up (route to `warren_client::run_multi_hop`
        // + `pump_bidirectional_with_daita`). Leaving them ignored
        // here keeps this first wire-up scope-minimal.
        let _ = params.multi_hop_relay;
        let _ = params.daita_spec;
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
            arc_for_task.fire_event(WarrenTunnelEventTagC::Reconnecting);

            let client = warren_tunnel::ClientTunnel::with_signing_key(&signing_key);
            let session = match client.connect(exit_target).await {
                Ok(session) => session,
                Err(_e) => {
                    arc_for_task.set_state(WarrenTunnelStateC::Failed);
                    arc_for_task.fire_event(WarrenTunnelEventTagC::Disconnected);
                    return;
                }
            };

            arc_for_task.set_state(WarrenTunnelStateC::Connected);
            arc_for_task
                .connected_at_secs
                .store(now_secs(), std::sync::atomic::Ordering::Relaxed);
            arc_for_task.fire_event(WarrenTunnelEventTagC::Connected);

            // Drive the bidirectional pump. `pump_bidirectional` takes
            // the `PacketDevice` (our `IosTun` clone) + the Quinn
            // `Connection` (cloned out of the session). Returns on
            // session error or when the IosTun channels close (handle
            // drop). The `session` itself must outlive the pump (its
            // `_endpoint` field keeps the connection alive), so we
            // hold it on the stack of this task until pump exits.
            let tun = arc_for_task.tun.clone();
            let conn = session.clone_conn();
            let pump_result = warren_tunnel::pump_bidirectional(tun, conn).await;
            drop(session);
            let _ = pump_result; // pump logs errors via tracing

            arc_for_task.set_state(WarrenTunnelStateC::Disconnected);
            arc_for_task.fire_event(WarrenTunnelEventTagC::Disconnected);
        });

        // Box the Arc so the FFI sees a single owner ; clones live
        // inside spawned tasks via the Arc.
        let boxed = Box::new(arc);
        Box::into_raw(boxed) as *mut WarrenTunnelHandle
    }
    #[cfg(not(feature = "tunnel"))]
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
    #[cfg(feature = "tunnel")]
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
    #[cfg(not(feature = "tunnel"))]
    {
        let _ = handle;
    }
}

/// Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
/// Uses `warren_backoff::Backoff::HANDSHAKE` (15s, cf. M4.H.G).
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
    #[cfg(feature = "tunnel")]
    {
        // SAFETY: caller upholds the handle invariant.
        let arc = unsafe { handle_arc(handle) };
        if arc.state_snapshot() == WarrenTunnelStateC::Disconnected {
            return RC_NOT_CONNECTED;
        }
        // TODO C.4.1 signal the warren-tunnel reconnect future.
        RC_OK
    }
    #[cfg(not(feature = "tunnel"))]
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
    #[cfg(feature = "tunnel")]
    let status = if handle.is_null() {
        WarrenTunnelStatusC {
            state: WarrenTunnelStateC::Disconnected,
            bytes_in: 0,
            bytes_out: 0,
            connected_duration_seconds: 0,
            failover_count: 0,
        }
    } else {
        // SAFETY: caller upholds the handle invariant.
        let arc = unsafe { handle_arc(handle) };
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
            failover_count: arc.failover_count.load(std::sync::atomic::Ordering::Relaxed),
        }
    };
    #[cfg(not(feature = "tunnel"))]
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
    #[cfg(feature = "tunnel")]
    {
        // SAFETY: caller upholds the handle invariant.
        let arc = unsafe { handle_arc(handle) };
        let Ok(mut slot) = arc.event_callback.lock() else {
            return RC_INVALID_INPUT;
        };
        *slot = Some(handle_impl::CallbackEntry {
            callback,
            context,
        });
        RC_OK
    }
    #[cfg(not(feature = "tunnel"))]
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
    #[cfg(feature = "tunnel")]
    {
        // SAFETY: caller upholds the handle invariant.
        let arc = unsafe { handle_arc(handle) };
        let Ok(mut slot) = arc.outbound_callback.lock() else {
            return RC_INVALID_INPUT;
        };
        *slot = Some(handle_impl::CallbackEntry {
            callback,
            context,
        });
        RC_OK
    }
    #[cfg(not(feature = "tunnel"))]
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
    #[cfg(feature = "tunnel")]
    {
        // SAFETY: caller upholds the handle + data invariants.
        let arc = unsafe { handle_arc(handle) };
        let packet = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        arc.bytes_in
            .fetch_add(len as u64, std::sync::atomic::Ordering::Relaxed);
        arc.tun.inject_inbound(packet);
        RC_OK
    }
    #[cfg(not(feature = "tunnel"))]
    {
        let _ = (handle, data, len);
        RC_TUNNEL_FEATURE_DISABLED
    }
}

// ---- Internal helpers (private) ----

/// Decodes a null-terminated C string to a Rust `&str` for marshalling.
/// Returns `None` on null pointer or invalid UTF-8.
///
/// # Safety
/// `ptr` must be null or point to a valid null-terminated C string for
/// the duration of the returned reference's lifetime.
#[cfg(feature = "tunnel")]
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller upholds the lifetime + null-termination invariant.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Cast an opaque FFI handle pointer back to a typed `&Arc<Impl>`
/// reference. The pointer is borrowed (no ownership transfer) ; the
/// `Arc` cloning behaviour is preserved for the duration of the
/// return value's lifetime.
///
/// # Safety
/// `handle` must point to a `Box<Arc<WarrenTunnelHandleImpl>>` that is
/// still live (i.e. [`warren_tunnel_stop`] has not been called on it
/// yet).
#[cfg(feature = "tunnel")]
unsafe fn handle_arc(
    handle: *mut WarrenTunnelHandle,
) -> &'static std::sync::Arc<handle_impl::WarrenTunnelHandleImpl> {
    // SAFETY: caller upholds liveness ; we extend the lifetime to
    // `'static` because Rust cannot express "for the duration of the
    // box on the FFI heap". Callers must not retain the returned
    // reference beyond the FFI call.
    unsafe {
        &*(handle as *const std::sync::Arc<handle_impl::WarrenTunnelHandleImpl>)
    }
}

/// Seconds since Unix epoch, monotonic-ish.
#[cfg(feature = "tunnel")]
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
