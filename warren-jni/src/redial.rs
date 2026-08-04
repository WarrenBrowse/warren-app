//! Tunnel session status contract published to the Kotlin layer.
//!
//! The Android tunnel runs a single session with no daemon state machine,
//! so its lifecycle is surfaced to Kotlin as one `i32` polled via
//! `WarrenJni.getTunnelStatus()`. The multi-hop client owns the actual
//! reconnect supervision (its own `SupervisorConfig`); this type only
//! encodes what the Kotlin layer must react to:
//!
//! - `Reconnecting` (3) means "quick blip": a transparent redial is in
//!   flight and expected to land within seconds. The VpnService TUN is
//!   still established and captures all traffic (the dead pump drops it),
//!   so no kill-switch action is needed during this window.
//! - `Disconnected` (0) after a `Reconnecting` window means "network gone":
//!   the Kotlin fail-closed policy takes over (blackhole interface +
//!   connectivity gated retry), exactly as it does for any other session
//!   death.
//! - `Unauthorized` (4) is terminal: retrying cannot recover a lapsed
//!   subscription, so the session is never redialed past it.

/// Tunnel session status reported back to Kotlin via
/// `WarrenJni.getTunnelStatus()`. Encoded as an `i32` rather than an enum
/// to match the existing JNI int contract.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    /// Transparent redial in flight after a session loss. See the module
    /// doc for the Kotlin-side contract.
    Reconnecting = 3,
    /// The exit refused the setup with a policy rejection: the account is
    /// not authorized, i.e. the subscription has lapsed or was revoked.
    /// Distinct from `Disconnected` so the Kotlin layer can surface
    /// "subscription expired" and STOP the reconnect loop (retrying cannot
    /// recover an unauthorized account until it is renewed).
    Unauthorized = 4,
}
