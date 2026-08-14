//! Read-only observations the daemon can report about the Warren datapath.
//!
//! # Why
//!
//! When a tunnel is up but wrong, the app shows "Connected" and a row of
//! feature chips, none of which is about the path. Everything the answer needs
//! already exists inside the daemon (the requested bundle width, the carrier
//! bind verdict remembered for this network, the interfaces the host holds); it
//! is simply unreachable from outside. This type is that reach.
//!
//! Every field is an OBSERVATION, never a conclusion, and none of it is
//! identity material: counts, a verdict kind, an age, and interface names. No
//! address, no key, and not the network fingerprint the verdict is keyed by.

use serde::{Deserialize, Serialize};

/// One snapshot of what the daemon can say about the datapath without probing
/// anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarrenDiagnostics {
    /// Parallel QUIC connections the next connect will ask the exit for, after
    /// the env var, the persisted setting and the compiled default have been
    /// resolved against each other. This is the REQUEST; how many legs a live
    /// tunnel actually bonds is on the tunnel endpoint.
    pub requested_n_connections: u8,
    /// Carrier-bind verdict remembered for the network the default route
    /// currently points at. `None` when the platform has no such guard (only
    /// macOS does), when nothing fresh is cached for this network, or when
    /// there is no default route to identify it by.
    pub carrier_verdict: Option<CarrierVerdictReport>,
    /// Active non-tunnel interfaces holding an address on the default
    /// gateway's subnet, reported only when two or more do. macOS only. The
    /// LAN then picks which interface carries the carrier's replies, which can
    /// cost a large part of the downlink with nothing else showing it.
    pub dual_homed_interfaces: Vec<String>,
}

/// A remembered carrier-bind verdict and how stale it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierVerdictReport {
    pub kind: CarrierVerdictKind,
    /// Seconds since the verdict was measured.
    pub age_seconds: u64,
    /// Seconds after which the verdict stops being replayed and the bind is
    /// measured again.
    pub ttl_seconds: u64,
}

/// What the guard measured about binding the carrier socket to the physical
/// interface on this network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrierVerdictKind {
    /// The bound carrier egressed: the leak-free configuration is in use.
    BindOk,
    /// The bind black-holed here, so connects install the wider
    /// `<carrier_ip>/32` route exception instead.
    RouteOnly,
}
