//! Per-app routing inside the Warren tunnel.
//!
//! Every packet of every tunneled app reaches the daemon through the TUN
//! device. This crate classifies each new flow by the process that owns its
//! local socket, decides whether it leaves through the main session or through
//! the route session of that app's exit, rewrites the addresses so a route
//! session sees its own inner address, and translates the answers back.
//!
//! The design contract is `docs/app-routing.md`. The layers, from the system
//! boundary up:
//!
//! - [`owner`]: which process owns a socket, and what executable it runs. The
//!   only module that calls the OS.
//! - [`app`]: whether an executable belongs to an app the user chose.
//! - [`flow`]: flow identities parsed from packets, and the table of live flows.
//! - [`nat`]: in-place address translation with incremental checksums.
//! - [`router`]: the per-packet decision that ties them together.
//!
//! Nothing here logs an address, a port, a pid or a path: the router keeps
//! counters, and callers log those.

pub mod app;
pub mod flow;
mod ip;
pub mod nat;
pub mod owner;
pub mod router;

#[cfg(test)]
mod testutil;

pub use ip::PacketError;
