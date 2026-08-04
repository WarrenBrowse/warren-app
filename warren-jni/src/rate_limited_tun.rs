// Client-side bandwidth ceiling on the Android tunnel datapath.
//
// Compact sibling of the desktop wrapper
// (`talpid-warren-tunnel::rate_limiter`): the multi-hop pump moves
// packets through the `PacketDevice` it is handed, so wrapping the
// (already remapped) Android TUN enforces the cap on both directions:
// an uplink packet over budget is dropped before it reaches the QUIC
// session, a downlink packet over budget is dropped instead of being
// written to the TUN. Both directions run independent token buckets
// fed the same bits-per-second figure; dropping (not queueing) lets
// TCP converge below the cap. Android applies the cap at tunnel start
// (`WarrenTunnelConfig.max_rate_bps`); a change takes effect on the
// next tunnel start, mirroring the MTU setting.

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
use warrenguard_ratelimit::{RatePolicyHandle, RateSpec};
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
use warrenguard_transport_core::PacketDevice;

/// Burst floor in bytes: keeps the largest possible IP packet
/// admissible at very low caps while sustained throughput still
/// converges on the configured rate.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
const MIN_BURST_BYTES: u64 = 64 * 1024;

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn direction_bucket(rate_bps_bits: u64) -> RatePolicyHandle<()> {
    let handle = RatePolicyHandle::new();
    let bytes_per_sec = (rate_bps_bits / 8).max(1);
    handle.set_policy(
        RateSpec::new(bytes_per_sec.max(MIN_BURST_BYTES), bytes_per_sec),
        std::collections::HashMap::new(),
    );
    handle
}

/// [`PacketDevice`] decorator enforcing a fixed per-direction cap.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
#[derive(Clone)]
pub struct RateLimitedTun<T> {
    inner: T,
    uplink: RatePolicyHandle<()>,
    downlink: RatePolicyHandle<()>,
}

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
impl<T> RateLimitedTun<T> {
    /// Wrap `inner` with a `rate_bps` (bits per second) ceiling applied
    /// to each direction independently.
    pub fn new(inner: T, rate_bps: u64) -> Self {
        Self {
            inner,
            uplink: direction_bucket(rate_bps),
            downlink: direction_bucket(rate_bps),
        }
    }
}

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
impl<T: PacketDevice + Clone> PacketDevice for RateLimitedTun<T> {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        loop {
            let pkt = self.inner.recv().await?;
            if self.uplink.try_consume(&(), pkt.len() as u64) {
                return Ok(pkt);
            }
            // Over budget: drop and keep draining the TUN queue.
        }
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        if self.downlink.try_consume(&(), packet.len() as u64) {
            self.inner.send(packet).await
        } else {
            Ok(())
        }
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        match self.inner.try_recv()? {
            Some(pkt) if !self.uplink.try_consume(&(), pkt.len() as u64) => Ok(None),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warrenguard_transport_core::FakeTun;

    const CAP_64KIB_PER_SEC: u64 = 64 * 1024 * 8;

    #[tokio::test]
    async fn capped_downlink_drops_over_budget_packets_silently() {
        let tun = FakeTun::new();
        let dev = RateLimitedTun::new(tun.clone(), CAP_64KIB_PER_SEC);

        dev.send(&vec![0u8; 64 * 1024]).await.expect("in budget");
        dev.send(&vec![0u8; 1400]).await.expect("drop is silent Ok");
        assert_eq!(
            tun.take_outbound().len(),
            1,
            "the over-budget packet must be dropped, not written"
        );
    }

    #[tokio::test]
    async fn capped_uplink_drops_in_place_and_keeps_draining() {
        let tun = FakeTun::new();
        let dev = RateLimitedTun::new(tun.clone(), CAP_64KIB_PER_SEC);

        tun.inject_inbound(vec![1u8; 64 * 1024]);
        assert_eq!(dev.recv().await.expect("in budget").len(), 64 * 1024);
        tun.inject_inbound(vec![2u8; 1400]);
        assert!(
            dev.try_recv().expect("drop maps to None").is_none(),
            "an over-budget packet must surface as not-ready, never as data"
        );
    }

    #[tokio::test]
    async fn directions_are_budgeted_independently() {
        let tun = FakeTun::new();
        let dev = RateLimitedTun::new(tun.clone(), CAP_64KIB_PER_SEC);

        dev.send(&vec![0u8; 64 * 1024]).await.expect("in budget");
        dev.send(&vec![0u8; 1400]).await.expect("silent drop");
        assert_eq!(tun.take_outbound().len(), 1);

        tun.inject_inbound(vec![1u8; 64 * 1024]);
        assert_eq!(
            dev.recv().await.expect("uplink budget independent").len(),
            64 * 1024
        );
    }
}
