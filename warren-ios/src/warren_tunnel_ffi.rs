//! Warren tunnel FFI for iOS (Quinn-based VPN tunnel via warren-tunnel).
//!
//! The C ABI surface (C-repr structs + extern "C" fn signatures) is
//! defined here unconditionally so that `cbindgen` can generate the
//! `warren_rust_runtime.h` declarations consumed by the Swift side
//! (`ios/WarrenRustRuntime/WarrenQuinnAdapter.swift`).
//!
//! The function **bodies** that actually drive a Quinn tunnel are gated
//! behind the `tunnel` Cargo feature ; without that feature, the FFI
//! returns null / sentinel error codes. The feature is OFF by default
//! because `warren-tunnel` currently uses `tun_rs::DeviceBuilder` which
//! has no iOS backend in `tun_rs 2.8` (same blocker as Android Session
//! D). The C.4 implementation brief either (a) contributes an iOS
//! backend to `tun_rs` upstream or (b) bridges via
//! `NEPacketTunnelFlow.readPackets/writePackets` from Swift to a
//! Rust-managed queue. Approach (b) is preferred (cf.
//! `.planning/c4-packet-tunnel-provider-quinn-design.md` §3.2).

use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int};

/// Opaque handle representing an active Warren tunnel. Created by
/// `warren_tunnel_start` ; destroyed by `warren_tunnel_stop`.
/// The Swift side treats this as an `OpaquePointer`.
#[repr(C)]
pub struct WarrenTunnelHandle {
    _private: [u8; 0],
}

/// Parameters passed from Swift to start a Warren tunnel. Mirrors the
/// public surface of `warren_tunnel::WarrenTunnelParameters`.
///
/// String fields are null-terminated UTF-8 ; the Swift side allocates
/// + retains them for the duration of the `warren_tunnel_start` call
/// (Rust only borrows during marshalling).
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
/// dispatcher once the `tunnel` feature is wired (C.4 implementation).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
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
/// the warren-tunnel dispatcher once the `tunnel` feature is wired
/// (C.4 implementation).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
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

// ---- Return codes shared by all tunnel FFI fns ----

const RC_OK: c_int = 0;
const RC_INVALID_INPUT: c_int = -1;
#[allow(dead_code)] // returned only on the `tunnel` feature path
const RC_NOT_CONNECTED: c_int = -3;
#[allow(dead_code)] // returned only on the `not(tunnel)` feature path
const RC_TUNNEL_FEATURE_DISABLED: c_int = -10;

// ---- Public FFI entry points ----

/// Starts a Warren tunnel with the given parameters. Returns an opaque
/// handle on success, or null on failure (invalid parameters, tunnel
/// feature disabled at build time).
///
/// `packet_fd` is the iOS NEPacketTunnelFlow file descriptor. When the
/// `NEPacketTunnelFlow.readPackets/writePackets` bridge approach is
/// used (preferred), pass `-1` and configure the Swift-side bridge
/// separately.
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
        // TODO C.4.2 wire warren-tunnel::WarrenTunnelParameters + spawn
        // Quinn connection task. For now return null until the
        // NEPacketTunnelFlow bridge or tun_rs iOS backend lands.
        let _ = parameters;
        std::ptr::null_mut()
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
/// `handle` must have been returned by `warren_tunnel_start` and must
/// not have been stopped already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_stop(handle: *mut WarrenTunnelHandle) {
    if handle.is_null() {
        return;
    }
    #[cfg(feature = "tunnel")]
    {
        // TODO C.4.2 drop the Tokio task + Quinn connection bound to
        // this handle. Reconstitute the Box<WarrenTunnelHandleImpl>
        // from the raw pointer and let Drop clean up.
    }
    #[cfg(not(feature = "tunnel"))]
    {
        // No body to drop when tunnel feature is off (handle is null,
        // we returned early above).
    }
    // Reserved : when tunnel feature lands, the box drop happens here.
}

/// Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
/// Uses `warren_backoff::Backoff::HANDSHAKE` (15s, cf. M4.H.G).
///
/// Returns `0` on success, `-3` if the tunnel is not connected.
///
/// # Safety
/// `handle` must be a valid pointer from `warren_tunnel_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_reconnect(handle: *mut WarrenTunnelHandle) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(feature = "tunnel")]
    {
        // TODO C.4.2 signal the warren-tunnel reconnect future.
        let _ = handle;
        RC_NOT_CONNECTED
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
/// `handle` must be a valid pointer from `warren_tunnel_start`.
/// `out_status` must point to a writable `WarrenTunnelStatusC`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_status(
    handle: *mut WarrenTunnelHandle,
    out_status: *mut WarrenTunnelStatusC,
) -> c_int {
    if out_status.is_null() {
        return RC_INVALID_INPUT;
    }
    let state = if handle.is_null() {
        WarrenTunnelStateC::Disconnected
    } else {
        #[cfg(feature = "tunnel")]
        {
            // TODO C.4.2 read state from the warren-tunnel handle.
            WarrenTunnelStateC::Disconnected
        }
        #[cfg(not(feature = "tunnel"))]
        {
            WarrenTunnelStateC::Disconnected
        }
    };
    // Safety: caller-provided writable buffer (precondition).
    unsafe {
        std::ptr::write(
            out_status,
            WarrenTunnelStatusC {
                state,
                bytes_in: 0,
                bytes_out: 0,
                connected_duration_seconds: 0,
                failover_count: 0,
            },
        );
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
/// `handle` must be a valid pointer from `warren_tunnel_start`.
/// `callback` (if non-null) must outlive the call to
/// `warren_tunnel_stop`. `context` is passed back unchanged ; lifetime
/// is the caller's responsibility.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_tunnel_set_event_callback(
    handle: *mut WarrenTunnelHandle,
    callback: Option<WarrenTunnelEventCallback>,
    context: *mut c_void,
) -> c_int {
    if handle.is_null() {
        return RC_INVALID_INPUT;
    }
    #[cfg(feature = "tunnel")]
    {
        // TODO C.4.2 store callback + context inside the handle's
        // event dispatcher state.
        let _ = (callback, context);
        RC_OK
    }
    #[cfg(not(feature = "tunnel"))]
    {
        let _ = (callback, context);
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
#[allow(dead_code)]
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // Safety: caller upholds the lifetime + null-termination invariant.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}
