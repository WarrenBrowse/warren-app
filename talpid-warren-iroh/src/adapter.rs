//! Adapter `PacketDevice` pour bridger les TUN devices Mullvad
//! ([`tun08::AsyncDevice`]) vers le trait Warren
//! ([`warren_iroh_tunnel::PacketDevice`]).
//!
//! **Phase 1.B.4.b** : nécessaire parce que Warren et Mullvad utilisent
//! des versions différentes de la crate `tun` :
//! - Mullvad : `tun = "0.8.5"` (re-exposée ici comme `tun08`)
//! - Warren `warren-iroh-tunnel` : `tun-rs = "2"` (un fork plus récent)
//!
//! L'adapter wrappe un `Arc<tun08::AsyncDevice>` pour pouvoir cloner
//! l'handle entre la task uplink (TUN→QUIC) et la task downlink
//! (QUIC→TUN) du `pump_multi_bidirectional` Warren.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use warren_iroh_tunnel::PacketDevice;

/// Taille d'allocation par appel `recv()`. 65 535 = MTU IPv4 maximum
/// théorique. La majorité des paquets seront < 1500 octets, mais
/// `Vec::truncate` après lecture amène la taille au réel — donc on ne
/// paie que l'alloc + le cap (pas la copie).
///
/// Phase optim future (cf. règle `mem-with-capacity`) : utiliser un
/// pool de buffers Bytes pré-alloués pour éliminer ces allocs en hot
/// path.
const RECV_BUF_SIZE: usize = u16::MAX as usize;

/// Compteurs partagés pump (= F10e Option B instrumentation). Permettent
/// au caller `WarrenIrohMonitor::start` de spawn une task métricienne
/// qui log les compteurs périodiquement, pour identifier précisément
/// la direction (uplink TUN→QUIC vs downlink QUIC→TUN) qui casse en
/// daemon-fork. `Relaxed` est suffisant : on ne synchronise pas avec
/// d'autres atomiques, juste pub des compteurs monotonic croissants.
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

/// Adapter clonable autour d'un `tun08::AsyncDevice`. Implémente le
/// trait Warren [`PacketDevice`] en déléguant à `recv` / `send` du
/// device.
///
/// Le `Arc` est nécessaire parce que `pump_multi_bidirectional` clone
/// le device pour les deux directions du pump bidirectionnel ; les
/// `recv`/`send` async sur `&self` sur l'`AsyncDevice` peuvent être
/// invoqués concurrentement (l'`AsyncFd` interne sérialise les
/// readiness events au niveau kernel — cf. tokio docs).
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

    /// Handle clonable des compteurs pump (F10e Option B). Permet à
    /// `WarrenIrohMonitor::start` de lire les compteurs depuis une
    /// task séparée pendant que le pump tourne.
    pub(crate) fn metrics(&self) -> Arc<PumpMetrics> {
        self.metrics.clone()
    }
}

impl PacketDevice for MullvadTunPacketDevice {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; RECV_BUF_SIZE];
        let n = self.dev.recv(&mut buf).await?;
        buf.truncate(n);
        // F10e Option B : compteur uplink (TUN→QUIC). recv depuis
        // le device kernel = paquet user à encapsuler.
        self.metrics.uplink_packets.fetch_add(1, Ordering::Relaxed);
        Ok(buf)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        let _ = self.dev.send(packet).await?;
        // F10e Option B : compteur downlink (QUIC→TUN). send vers
        // le device kernel = paquet décap reçu côté QUIC.
        self.metrics
            .downlink_packets
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        // Phase 1.B.4.b : fallback `Ok(None)`. Le pump uplink rebascule
        // alors sur `recv_batch` (default impl) qui fait 1 `recv()`
        // bloquant — moins efficace que la coalescence M3.K.7 Tier 2.2
        // mais fonctionnellement correct. Optimisation future :
        // exposer `try_recv` non-bloquant via le `Device::recv` sync
        // sous-jacent (tun08 deref AsyncDevice → Device, mais le type
        // `Device` n'est pas re-exporté publiquement par la crate
        // tun08, donc nécessite un PR upstream ou un autre wrapping).
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'adapter doit être Send + Sync + 'static (= bounds requis par
    /// le trait `PacketDevice`). Test compile-only qui garantit que
    /// le contrat est respecté en l'asserting via `fn requires<T:
    /// PacketDevice>()`.
    #[test]
    fn adapter_satisfies_packet_device_bounds() {
        fn requires_packet_device<T: PacketDevice>() {}
        requires_packet_device::<MullvadTunPacketDevice>();
    }

    #[test]
    fn adapter_is_clone() {
        // `pump_multi_bidirectional` requires `Clone` pour split entre
        // task uplink + downlink. Le test compile-only suffit ; le
        // Clone retourne un Arc::clone (réf compteur), donc cheap.
        fn requires_clone<T: Clone>() {}
        requires_clone::<MullvadTunPacketDevice>();
    }
}
