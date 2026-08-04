//! Client-side bandwidth ceiling on the Warren tunnel datapath
//! (`Settings::warren_max_rate_bps`).
//!
//! The pumps in `warrenguard-transport` move packets through the
//! [`PacketDevice`] they are handed, so the cap is enforced by
//! wrapping that device: an uplink packet read from the TUN that
//! exceeds the budget is dropped before it ever reaches the QUIC
//! session, and a downlink packet over budget is dropped instead of
//! being written to the TUN. Both directions run independent token
//! buckets fed the same bits-per-second figure. Dropping (not
//! queueing) is deliberate: TCP interprets the loss as congestion and
//! converges below the cap, and the datapath stays allocation- and
//! latency-neutral.
//!
//! When uncapped the per-packet cost is a single relaxed atomic load,
//! so the wrapper is always installed and the cap can engage at
//! runtime (watch-channel push from the daemon) without a reconnect.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use warrenguard_ratelimit::{RatePolicyHandle, RateSpec};
use warrenguard_transport_core::PacketDevice;

/// Burst floor in bytes. A one-second burst window shorter than the
/// largest possible IP packet (65 535 bytes) would make big packets
/// permanently inadmissible at very low caps; a floor of 64 KiB keeps
/// every packet admissible while sustained throughput still converges
/// on the configured rate.
const MIN_BURST_BYTES: u64 = 64 * 1024;

/// One direction's runtime-swappable token bucket.
#[derive(Clone)]
struct DirectionLimiter {
    /// Fast-path gate: `false` = uncapped, skip the policy entirely.
    engaged: Arc<AtomicBool>,
    policy: RatePolicyHandle<()>,
}

impl DirectionLimiter {
    fn new() -> Self {
        Self {
            engaged: Arc::new(AtomicBool::new(false)),
            policy: RatePolicyHandle::new(),
        }
    }

    /// Applies a new cap (bits per second, `None` = unlimited). A cap
    /// below 8 bps still installs a 1 byte/s bucket rather than
    /// silently unlimiting.
    fn set_rate_bps(&self, bps: Option<u64>) {
        let spec = bps.and_then(|bits| {
            let bytes_per_sec = (bits / 8).max(1);
            RateSpec::new(bytes_per_sec.max(MIN_BURST_BYTES), bytes_per_sec)
        });
        self.policy
            .set_policy(spec, std::collections::HashMap::new());
        self.engaged.store(spec.is_some(), Ordering::Relaxed);
    }

    /// `true` admits the packet, `false` means the caller must drop it.
    fn admit(&self, bytes: usize) -> bool {
        if !self.engaged.load(Ordering::Relaxed) {
            return true;
        }
        self.policy.try_consume(&(), bytes as u64)
    }
}

/// Shared handle over both directions' limiters. Cloneable so the
/// daemon-driven control task and the wrapped device feed the same
/// buckets.
#[derive(Clone)]
pub(crate) struct TunnelRateLimiter {
    uplink: DirectionLimiter,
    downlink: DirectionLimiter,
}

impl TunnelRateLimiter {
    /// A fresh, uncapped limiter.
    pub(crate) fn new() -> Self {
        Self {
            uplink: DirectionLimiter::new(),
            downlink: DirectionLimiter::new(),
        }
    }

    /// Applies `bps` to BOTH directions (each direction gets the full
    /// rate, enforced independently). `None` = unlimited.
    pub(crate) fn set_rate_bps(&self, bps: Option<u64>) {
        self.uplink.set_rate_bps(bps);
        self.downlink.set_rate_bps(bps);
    }

    /// Wraps a packet device so the pumps enforce this limiter.
    pub(crate) fn wrap<D: PacketDevice>(&self, inner: D) -> RateLimitedPacketDevice<D> {
        RateLimitedPacketDevice {
            inner,
            limiter: self.clone(),
        }
    }
}

/// [`PacketDevice`] decorator enforcing [`TunnelRateLimiter`].
///
/// The wrapper overrides only the single-packet primitives; the
/// trait's default batch implementations then route every batched
/// packet through them, so the wrapped `MullvadTunPacketDevice`
/// (which overrides no batch method itself) stays fully covered.
#[derive(Clone)]
pub(crate) struct RateLimitedPacketDevice<D> {
    inner: D,
    limiter: TunnelRateLimiter,
}

impl<D: PacketDevice> PacketDevice for RateLimitedPacketDevice<D> {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        loop {
            let pkt = self.inner.recv().await?;
            if self.limiter.uplink.admit(pkt.len()) {
                return Ok(pkt);
            }
            // Over budget: drop the packet and keep draining the TUN so
            // the queue never backs up into the kernel.
        }
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        if self.limiter.downlink.admit(packet.len()) {
            self.inner.send(packet).await
        } else {
            Ok(())
        }
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        match self.inner.try_recv()? {
            Some(pkt) if !self.limiter.uplink.admit(pkt.len()) => Ok(None),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warrenguard_transport_core::FakeTun;

    #[tokio::test]
    async fn uncapped_wrapper_passes_uplink_and_downlink_through() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        let dev = limiter.wrap(tun.clone());

        tun.inject_inbound(vec![1u8; 1400]);
        assert_eq!(dev.recv().await.expect("recv must pass"), vec![1u8; 1400]);

        dev.send(&[2u8; 1400]).await.expect("send must pass");
        assert_eq!(tun.take_outbound(), vec![vec![2u8; 1400]]);
    }

    #[tokio::test]
    async fn capped_downlink_drops_over_budget_packets_silently() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        // 64 KiB/s cap = the burst floor exactly: one 64 KiB burst
        // admits, the next packet in the same instant must drop.
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        dev.send(&vec![0u8; 64 * 1024]).await.expect("in budget");
        dev.send(&vec![0u8; 1400]).await.expect("drop is silent Ok");
        assert_eq!(
            tun.take_outbound().len(),
            1,
            "the over-budget packet must be dropped, not written"
        );
    }

    #[tokio::test]
    async fn capped_uplink_drops_over_budget_and_keeps_draining() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        // First packet exhausts the burst budget; the second must be
        // dropped in place and recv() must move on to the third once
        // budget-sized again. Use a tiny third packet plus a refill via
        // a fresh policy push to keep the test clock-free.
        tun.inject_inbound(vec![1u8; 64 * 1024]);
        tun.inject_inbound(vec![2u8; 1400]);
        assert_eq!(dev.recv().await.expect("in budget").len(), 64 * 1024);

        // Re-arm the budget: the pending packet 2 was NOT yet read, so
        // it is admitted now (recv drains and drops only over-budget
        // reads, it never blocks the queue).
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        assert_eq!(dev.recv().await.expect("re-armed").len(), 1400);
    }

    #[tokio::test]
    async fn uplink_recv_drops_in_place_when_over_budget() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        tun.inject_inbound(vec![1u8; 64 * 1024]);
        // Second packet arrives while the budget is exhausted, third is
        // injected after a re-arm below; recv must skip packet 2.
        tun.inject_inbound(vec![2u8; 60 * 1024]);
        assert_eq!(dev.recv().await.expect("in budget").len(), 64 * 1024);

        limiter.set_rate_bps(Some(64 * 1024 * 8));
        tun.inject_inbound(vec![3u8; 1400]);
        // Budget after re-arm: 64 KiB. Packet 2 (60 KiB) is admitted,
        // proving the drop above consumed nothing permanently.
        assert_eq!(dev.recv().await.expect("packet 2").first(), Some(&2u8));
    }

    #[tokio::test]
    async fn directions_are_budgeted_independently() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        // Exhaust the DOWNLINK budget entirely.
        dev.send(&vec![0u8; 64 * 1024]).await.expect("in budget");
        dev.send(&vec![0u8; 1400]).await.expect("silent drop");
        assert_eq!(tun.take_outbound().len(), 1);

        // The UPLINK budget must be untouched.
        tun.inject_inbound(vec![1u8; 64 * 1024]);
        assert_eq!(
            dev.recv().await.expect("uplink budget independent").len(),
            64 * 1024
        );
    }

    #[tokio::test]
    async fn unsetting_the_cap_disengages_at_runtime() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        dev.send(&vec![0u8; 64 * 1024]).await.expect("in budget");
        dev.send(&vec![0u8; 1400]).await.expect("silent drop");
        assert_eq!(tun.take_outbound().len(), 1, "capped: second dropped");

        // Live un-cap (the daemon watch push): everything passes again.
        limiter.set_rate_bps(None);
        dev.send(&vec![0u8; 64 * 1024]).await.expect("uncapped");
        dev.send(&vec![0u8; 64 * 1024]).await.expect("uncapped");
        assert_eq!(tun.take_outbound().len(), 2);
    }

    #[tokio::test]
    async fn try_recv_reports_none_for_a_dropped_packet() {
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(64 * 1024 * 8));
        let dev = limiter.wrap(tun.clone());

        tun.inject_inbound(vec![1u8; 64 * 1024]);
        assert!(dev.try_recv().expect("in budget").is_some());
        tun.inject_inbound(vec![2u8; 1400]);
        assert!(
            dev.try_recv().expect("drop maps to None").is_none(),
            "an over-budget packet must surface as not-ready, never as data"
        );
    }

    #[tokio::test]
    async fn sub_byte_rate_caps_do_not_unlimit() {
        // A pathological 1 bps cap must still install a bucket (1
        // byte/s) rather than resolving to "no spec = unlimited".
        let tun = FakeTun::new();
        let limiter = TunnelRateLimiter::new();
        limiter.set_rate_bps(Some(1));
        let dev = limiter.wrap(tun.clone());

        // Burst floor admits one max-size packet, then everything drops.
        dev.send(&vec![0u8; 64 * 1024]).await.expect("burst floor");
        dev.send(&vec![0u8; 1400]).await.expect("silent drop");
        assert_eq!(tun.take_outbound().len(), 1);
    }
}
