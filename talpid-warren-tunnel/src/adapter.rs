//! `PacketDevice` adapter that bridges Mullvad TUN devices
//! ([`tun08::AsyncDevice`]) onto the Warren trait
//! ([`warrenguard_transport_core::PacketDevice`]).
//!
//! Required because Warren and Mullvad use different versions of the
//! `tun` crate:
//! - Mullvad: `tun = "0.8.5"` (re-exposed here as `tun08`).
//! - warren-tunnel: `tun-rs = "2"` (a more recent fork).
//!
//! The adapter wraps an `Arc<tun08::AsyncDevice>` so the handle can be
//! cloned between the uplink (TUN -> QUIC) and downlink (QUIC -> TUN)
//! tasks of the multi-hop bidirectional pump.

use std::sync::Arc;

use warrenguard_transport_core::PacketDevice;

/// Per-`recv()` allocation size. 65 535 = theoretical IPv4 MTU max.
/// Most packets will be < 1500 bytes, and `Vec::truncate` after the
/// read brings the size down, so we only pay alloc + cap (no copy).
const RECV_BUF_SIZE: usize = u16::MAX as usize;

/// Cloneable wrapper around a `tun08::AsyncDevice`. Implements the
/// Warren [`PacketDevice`] trait by delegating to `recv` / `send`.
///
/// The `Arc` is required because the multi-hop pump clones the
/// device for the two directions of the bidirectional pump; `recv` /
/// `send` async on `&self` on `AsyncDevice` may be invoked
/// concurrently (the underlying `AsyncFd` serializes readiness events
/// at the kernel level, see tokio docs).
#[derive(Clone)]
pub(crate) struct MullvadTunPacketDevice {
    dev: Arc<tun08::AsyncDevice>,
}

impl MullvadTunPacketDevice {
    pub(crate) fn new(dev: tun08::AsyncDevice) -> Self {
        Self { dev: Arc::new(dev) }
    }
}

impl PacketDevice for MullvadTunPacketDevice {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; RECV_BUF_SIZE];
        let n = self.dev.recv(&mut buf).await?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        let _ = self.dev.send(packet).await?;
        Ok(())
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        // Fallback `Ok(None)`: the uplink pump then falls back to the
        // default `recv_batch` (single blocking `recv()`), losing the
        // batched coalescing optimization but staying correct.
        //
        // Implementing this on top of the tun 0.8.7 public API
        // requires going around the
        // `tokio::AsyncFd` wrapper that backs `AsyncDevice`. The
        // `Deref<Target = Device>` impl does expose the sync
        // `Device::recv` (set to non-blocking by
        // `AsyncDevice::new`), but calling it directly desyncs the
        // `AsyncFd` readiness bookkeeping: a successful sync recv
        // consumes the `readable` notification that tokio later
        // re-yields, producing a phantom `WouldBlock` (`recv_batch`
        // would loop on it) or, worse, a stuck `readable().await`
        // when the kernel does not re-arm before the next packet.
        // The safe pattern (`AsyncFd::try_io(Interest::READABLE, ...)`)
        // needs access to the private `AsyncFd<Device>` field, which
        // tun 0.8.7 does not expose. Until upstream tun adds either
        // a `try_recv` method or a `as_fd` accessor, returning
        // `Ok(None)` here is the correct conservative choice: it
        // costs a small batch-coalescing optimisation on bursts but
        // keeps the pump correctness contract intact.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The adapter must be Send + Sync + 'static (required by the
    /// `PacketDevice` trait bounds). Compile-only check via
    /// `fn requires<T: PacketDevice>()`.
    #[test]
    fn adapter_satisfies_packet_device_bounds() {
        fn requires_packet_device<T: PacketDevice>() {}
        requires_packet_device::<MullvadTunPacketDevice>();
    }

    #[test]
    fn adapter_is_clone() {
        // The bidirectional pump requires `Clone` to split the device
        // between uplink and downlink tasks. Clone is cheap: just an
        // `Arc::clone` on the inner device.
        fn requires_clone<T: Clone>() {}
        requires_clone::<MullvadTunPacketDevice>();
    }
}
