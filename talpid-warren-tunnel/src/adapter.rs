//! `PacketDevice` adapter that bridges Mullvad TUN devices
//! ([`tun08::AsyncDevice`]) onto the Warren trait
//! ([`warren_tunnel::PacketDevice`]).
//!
//! Required because Warren and Mullvad use different versions of the
//! `tun` crate:
//! - Mullvad: `tun = "0.8.5"` (re-exposed here as `tun08`).
//! - warren-tunnel: `tun-rs = "2"` (a more recent fork).
//!
//! The adapter wraps an `Arc<tun08::AsyncDevice>` so the handle can be
//! cloned between the uplink (TUN -> QUIC) and downlink (QUIC -> TUN)
//! tasks of Warren's `pump_multi_bidirectional`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use warren_tunnel::PacketDevice;

/// Per-`recv()` allocation size. 65 535 = theoretical IPv4 MTU max.
/// Most packets will be < 1500 bytes, and `Vec::truncate` after the
/// read brings the size down, so we only pay alloc + cap (no copy).
const RECV_BUF_SIZE: usize = u16::MAX as usize;

/// Shared pump counters used by `WarrenTunnelMonitor::start` to spawn a
/// metrics task that logs the counters periodically. Pinpoints which
/// direction (uplink TUN -> QUIC vs downlink QUIC -> TUN) drives the
/// data plane. `Relaxed` is sufficient: no synchronization with other
/// atomics, just monotonic counters.
#[derive(Default)]
pub(crate) struct PumpMetrics {
    uplink_packets: AtomicU64,
    downlink_packets: AtomicU64,
}

impl PumpMetrics {
    pub(crate) fn uplink_packets(&self) -> u64 {
        self.uplink_packets.load(Ordering::Relaxed)
    }

    pub(crate) fn downlink_packets(&self) -> u64 {
        self.downlink_packets.load(Ordering::Relaxed)
    }
}

/// Cloneable wrapper around a `tun08::AsyncDevice`. Implements the
/// Warren [`PacketDevice`] trait by delegating to `recv` / `send`.
///
/// The `Arc` is required because `pump_multi_bidirectional` clones the
/// device for the two directions of the bidirectional pump; `recv` /
/// `send` async on `&self` on `AsyncDevice` may be invoked
/// concurrently (the underlying `AsyncFd` serializes readiness events
/// at the kernel level, see tokio docs).
#[derive(Clone)]
pub(crate) struct MullvadTunPacketDevice {
    dev: Arc<tun08::AsyncDevice>,
    metrics: Arc<PumpMetrics>,
}

impl MullvadTunPacketDevice {
    pub(crate) fn new(dev: tun08::AsyncDevice) -> Self {
        Self {
            dev: Arc::new(dev),
            metrics: Arc::new(PumpMetrics::default()),
        }
    }

    /// Cloneable handle on the pump counters. Lets
    /// `WarrenTunnelMonitor::start` read the counters from a dedicated
    /// metrics task while the pump runs.
    pub(crate) fn metrics(&self) -> Arc<PumpMetrics> {
        self.metrics.clone()
    }
}

impl PacketDevice for MullvadTunPacketDevice {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; RECV_BUF_SIZE];
        let n = self.dev.recv(&mut buf).await?;
        buf.truncate(n);
        self.metrics.uplink_packets.fetch_add(1, Ordering::Relaxed);
        Ok(buf)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        let _ = self.dev.send(packet).await?;
        self.metrics
            .downlink_packets
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        // Fallback `Ok(None)`: the uplink pump then falls back to the
        // default `recv_batch` (single blocking `recv()`), losing the
        // batched coalescing optimization but staying correct. A real
        // non-blocking `try_recv` needs a sync `Device::recv` which
        // tun08 does not re-export publicly.
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
        // `pump_multi_bidirectional` requires `Clone` to split between
        // uplink and downlink tasks. Clone is cheap: just an
        // `Arc::clone` on the inner device + metrics.
        fn requires_clone<T: Clone>() {}
        requires_clone::<MullvadTunPacketDevice>();
    }
}
