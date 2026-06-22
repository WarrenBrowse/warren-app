// 1:1 inner-IP NAT wrapper around the Android VpnService TUN.
//
// The multi-hop exit allocates tunnel-inner addresses (an IPv4, e.g. 10.66.0.4,
// and optionally an IPv6, e.g. fdcc:f:1::x) via the `IpAssign` control message
// and then ENFORCES them: every uplink packet whose source is not the assigned
// address is dropped as spoofed (`warren_exit_core::multihop` gates on
// `source_ip_matches`). The desktop client satisfies this by reassigning its
// TUN to the allocated addresses (`RealTun::reassign_ipv4`). Android cannot: a
// `VpnService` interface address is fixed when `establish()` returns the fd, and
// the fd is handed to JNI before the IPs are negotiated, so the interface stays
// on the static `WarrenTunDefaults` addresses (10.64.0.1 / fd00::1).
//
// This wrapper bridges the gap with a stateless 1:1 NAT in the data pump:
//   - uplink (TUN -> network): rewrite source local -> assigned
//   - downlink (network -> TUN): rewrite destination assigned -> local
// for IPv4 always, and for IPv6 only when the exit granted a v6 address. Header
// and L4 checksums are fixed incrementally (RFC 1624). Without this the tunnel
// reports CONNECTED but carries no traffic (every uplink packet is dropped at
// the exit).

#[cfg(all(target_os = "android", feature = "tunnel"))]
use std::net::{Ipv4Addr, Ipv6Addr};

#[cfg(all(target_os = "android", feature = "tunnel"))]
use warren_tunnel::PacketDevice;

/// Wraps a [`PacketDevice`] and rewrites the tunnel-inner client addresses so
/// the kernel-fixed Android TUN addresses are presented to the exit as the
/// exit-assigned addresses. IPv6 remap is active only when `v6` is `Some`
/// (the exit granted a dual-stack address); otherwise v6 packets pass through
/// untouched and are blackholed at the exit, exactly as before.
#[cfg(all(target_os = "android", feature = "tunnel"))]
#[derive(Clone)]
pub struct RemapTun<T> {
    inner: T,
    local_v4: [u8; 4],
    assigned_v4: [u8; 4],
    v6: Option<([u8; 16], [u8; 16])>, // (local_v6, assigned_v6)
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
impl<T> RemapTun<T> {
    /// Build a remapping TUN. `local_*` are the addresses the Android interface
    /// is configured with; `assigned_*` are the exit-allocated inner addresses.
    /// `v6` is `Some((local, assigned))` only when the exit granted IPv6.
    pub fn new(
        inner: T,
        local_v4: Ipv4Addr,
        assigned_v4: Ipv4Addr,
        v6: Option<(Ipv6Addr, Ipv6Addr)>,
    ) -> Self {
        Self {
            inner,
            local_v4: local_v4.octets(),
            assigned_v4: assigned_v4.octets(),
            v6: v6.map(|(l, a)| (l.octets(), a.octets())),
        }
    }

    /// Rewrite uplink (TUN -> network): client source -> exit-assigned.
    fn remap_uplink(&self, pkt: &mut [u8]) {
        match ip_version(pkt) {
            Some(4) => rewrite_v4(pkt, Field::Source, &self.local_v4, &self.assigned_v4),
            Some(6) => {
                if let Some((local, assigned)) = &self.v6 {
                    rewrite_v6(pkt, Field::Source, local, assigned);
                }
            }
            _ => {}
        }
    }
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
impl<T: PacketDevice + Clone> PacketDevice for RemapTun<T> {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let mut pkt = self.inner.recv().await?;
        self.remap_uplink(&mut pkt);
        Ok(pkt)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        // Downlink: exit-assigned destination -> client address. Only allocate
        // a rewritten copy when the destination actually matches.
        match ip_version(packet) {
            Some(4) if packet.get(16..20) == Some(&self.assigned_v4[..]) => {
                let mut pkt = packet.to_vec();
                rewrite_v4(
                    &mut pkt,
                    Field::Destination,
                    &self.assigned_v4,
                    &self.local_v4,
                );
                self.inner.send(&pkt).await
            }
            Some(6) => match &self.v6 {
                Some((local, assigned)) if packet.get(24..40) == Some(&assigned[..]) => {
                    let mut pkt = packet.to_vec();
                    rewrite_v6(&mut pkt, Field::Destination, assigned, local);
                    self.inner.send(&pkt).await
                }
                _ => self.inner.send(packet).await,
            },
            _ => self.inner.send(packet).await,
        }
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        match self.inner.try_recv()? {
            Some(mut pkt) => {
                self.remap_uplink(&mut pkt);
                Ok(Some(pkt))
            }
            None => Ok(None),
        }
    }
}

#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
#[derive(Clone, Copy)]
enum Field {
    Source,
    Destination,
}

/// IP version nibble (4 or 6), or `None` if the packet is too short.
// Used only by the Android-gated `PacketDevice` impl; dead on the host test build.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn ip_version(packet: &[u8]) -> Option<u8> {
    packet.first().map(|b| b >> 4)
}

// ---- IPv4 ----

/// Rewrite an IPv4 source/destination from `from` to `to`, fixing the IPv4
/// header checksum and (for the first fragment of TCP/UDP) the transport
/// checksum incrementally. No-op for short packets or a non-matching field.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn rewrite_v4(packet: &mut [u8], field: Field, from: &[u8; 4], to: &[u8; 4]) {
    if packet.len() < 20 || (packet[0] >> 4) != 4 {
        return;
    }
    let addr_off = match field {
        Field::Source => 12usize,
        Field::Destination => 16usize,
    };
    if packet.get(addr_off..addr_off + 4) != Some(&from[..]) {
        return;
    }
    let ihl = ((packet[0] & 0x0f) as usize) * 4;
    if ihl < 20 || packet.len() < ihl {
        return;
    }

    packet[addr_off..addr_off + 4].copy_from_slice(to);

    // IPv4 header checksum (bytes 10..12).
    let ip_check = u16::from_be_bytes([packet[10], packet[11]]);
    let new_ip_check = checksum_replace(ip_check, from, to);
    packet[10..12].copy_from_slice(&new_ip_check.to_be_bytes());

    // Transport checksum only on the first fragment (offset 0).
    let frag_off = u16::from_be_bytes([packet[6], packet[7]]) & 0x1fff;
    if frag_off != 0 {
        return;
    }
    match packet[9] {
        6 => fix_l4_checksum(packet, ihl + 16, from, to, false), // TCP
        17 => fix_l4_checksum(packet, ihl + 6, from, to, true),  // UDP
        _ => {}
    }
}

// ---- IPv6 ----

/// Rewrite an IPv6 source/destination from `from` to `to`. IPv6 has no header
/// checksum; the transport checksum (TCP/UDP/ICMPv6) covers the address via the
/// pseudo-header and is fixed incrementally. Only the base-header case (next
/// header is directly an upper-layer protocol) fixes the L4 checksum; with
/// extension headers the address is still rewritten (so the exit accepts it)
/// but the L4 checksum is left alone (rare on this path; v6 is opt-in).
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn rewrite_v6(packet: &mut [u8], field: Field, from: &[u8; 16], to: &[u8; 16]) {
    if packet.len() < 40 || (packet[0] >> 4) != 6 {
        return;
    }
    let addr_off = match field {
        Field::Source => 8usize,
        Field::Destination => 24usize,
    };
    if packet.get(addr_off..addr_off + 16) != Some(&from[..]) {
        return;
    }

    packet[addr_off..addr_off + 16].copy_from_slice(to);

    // next_header at byte 6. Only fix the L4 checksum for a directly-attached
    // upper-layer protocol (no extension headers), where the L4 header starts
    // at byte 40.
    match packet[6] {
        6 => fix_l4_checksum(packet, 40 + 16, from, to, false), // TCP
        17 => fix_l4_checksum(packet, 40 + 6, from, to, true),  // UDP
        58 => fix_l4_checksum(packet, 40 + 2, from, to, false), // ICMPv6
        _ => {}
    }
}

/// Incrementally fix a transport checksum located at byte `off`.
///
/// `udp` marks the UDP quirk where a literal `0x0000` means "no checksum"
/// (left untouched) and a computed `0x0000` must be transmitted as `0xffff`.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn fix_l4_checksum(packet: &mut [u8], off: usize, from: &[u8], to: &[u8], udp: bool) {
    if packet.len() < off + 2 {
        return;
    }
    let old = u16::from_be_bytes([packet[off], packet[off + 1]]);
    if udp && old == 0 {
        return; // UDP without a checksum: nothing to fix.
    }
    let mut new = checksum_replace(old, from, to);
    if udp && new == 0 {
        new = 0xffff;
    }
    packet[off..off + 2].copy_from_slice(&new.to_be_bytes());
}

/// RFC 1624 incremental checksum update for a changed field (4 or 16 bytes):
/// `HC' = ~(~HC + sum(~m_i + m'_i))`, over the 16-bit halves of the change.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
fn checksum_replace(check: u16, old: &[u8], new: &[u8]) -> u16 {
    debug_assert_eq!(old.len(), new.len());
    debug_assert_eq!(old.len() % 2, 0);
    let mut sum: u32 = (!check) as u32;
    let mut i = 0;
    while i + 1 < old.len() {
        sum += (!u16::from_be_bytes([old[i], old[i + 1]])) as u32;
        sum += u16::from_be_bytes([new[i], new[i + 1]]) as u32;
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- IPv4 UDP fixture ----
    fn udp4_packet(src: [u8; 4], dst: [u8; 4]) -> Vec<u8> {
        let mut p = vec![0u8; 20 + 8];
        p[0] = 0x45;
        let total = (20u16 + 8).to_be_bytes();
        p[2] = total[0];
        p[3] = total[1];
        p[9] = 17;
        p[12..16].copy_from_slice(&src);
        p[16..20].copy_from_slice(&dst);
        p[20..22].copy_from_slice(&1234u16.to_be_bytes());
        p[22..24].copy_from_slice(&53u16.to_be_bytes());
        p[24..26].copy_from_slice(&8u16.to_be_bytes());
        let ipc = ip4_header_csum(&p);
        p[10..12].copy_from_slice(&ipc.to_be_bytes());
        let uc = udp4_csum(&p);
        p[26..28].copy_from_slice(&uc.to_be_bytes());
        p
    }

    fn ip4_header_csum(p: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        let mut i = 0;
        while i < 20 {
            if i == 10 {
                i += 2;
                continue;
            }
            sum += u16::from_be_bytes([p[i], p[i + 1]]) as u32;
            i += 2;
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        !(sum as u16)
    }

    fn udp4_csum(p: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        for c in p[12..20].chunks(2) {
            sum += u16::from_be_bytes([c[0], c[1]]) as u32;
        }
        sum += 17;
        sum += u16::from_be_bytes([p[24], p[25]]) as u32;
        let mut i = 20;
        while i + 1 < p.len() {
            if i == 26 {
                i += 2;
                continue;
            }
            sum += u16::from_be_bytes([p[i], p[i + 1]]) as u32;
            i += 2;
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        let c = !(sum as u16);
        if c == 0 { 0xffff } else { c }
    }

    // ---- IPv6 UDP fixture ----
    fn udp6_packet(src: [u8; 16], dst: [u8; 16]) -> Vec<u8> {
        let mut p = vec![0u8; 40 + 8];
        p[0] = 0x60; // version 6
        p[4..6].copy_from_slice(&8u16.to_be_bytes()); // payload length
        p[6] = 17; // next header UDP
        p[7] = 64; // hop limit
        p[8..24].copy_from_slice(&src);
        p[24..40].copy_from_slice(&dst);
        p[40..42].copy_from_slice(&1234u16.to_be_bytes());
        p[42..44].copy_from_slice(&53u16.to_be_bytes());
        p[44..46].copy_from_slice(&8u16.to_be_bytes());
        let uc = udp6_csum(&p);
        p[46..48].copy_from_slice(&uc.to_be_bytes());
        p
    }

    fn udp6_csum(p: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        for c in p[8..40].chunks(2) {
            sum += u16::from_be_bytes([c[0], c[1]]) as u32;
        }
        let udp_len = u16::from_be_bytes([p[44], p[45]]) as u32;
        sum += udp_len; // upper-layer length
        sum += 17; // next header
        let mut i = 40;
        while i + 1 < p.len() {
            if i == 46 {
                i += 2;
                continue;
            }
            sum += u16::from_be_bytes([p[i], p[i + 1]]) as u32;
            i += 2;
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        let c = !(sum as u16);
        if c == 0 { 0xffff } else { c }
    }

    #[test]
    fn v4_rewrites_source_and_keeps_checksums_valid() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let mut p = udp4_packet(local, [1, 1, 1, 1]);
        rewrite_v4(&mut p, Field::Source, &local, &assigned);
        assert_eq!(&p[12..16], &assigned);
        assert_eq!(u16::from_be_bytes([p[10], p[11]]), ip4_header_csum(&p));
        assert_eq!(u16::from_be_bytes([p[26], p[27]]), udp4_csum(&p));
    }

    #[test]
    fn v4_rewrites_destination_back() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let mut p = udp4_packet([1, 1, 1, 1], assigned);
        rewrite_v4(&mut p, Field::Destination, &assigned, &local);
        assert_eq!(&p[16..20], &local);
        assert_eq!(u16::from_be_bytes([p[10], p[11]]), ip4_header_csum(&p));
        assert_eq!(u16::from_be_bytes([p[26], p[27]]), udp4_csum(&p));
    }

    #[test]
    fn v4_leaves_non_matching_untouched() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let mut p = udp4_packet([192, 168, 0, 2], [1, 1, 1, 1]);
        let before = p.clone();
        rewrite_v4(&mut p, Field::Source, &local, &assigned);
        assert_eq!(p, before);
    }

    #[test]
    fn v6_rewrites_source_and_keeps_checksum_valid() {
        let local: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let assigned: [u8; 16] = [
            0xfd, 0xcc, 0, 0x0f, 0, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
        ];
        let mut p = udp6_packet(
            local,
            [
                0x20, 1, 0x48, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0x88,
            ],
        );
        rewrite_v6(&mut p, Field::Source, &local, &assigned);
        assert_eq!(&p[8..24], &assigned);
        assert_eq!(
            u16::from_be_bytes([p[46], p[47]]),
            udp6_csum(&p),
            "IPv6 UDP checksum stays valid after source rewrite"
        );
    }

    #[test]
    fn v6_rewrites_destination_back() {
        let local: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let assigned: [u8; 16] = [
            0xfd, 0xcc, 0, 0x0f, 0, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
        ];
        let mut p = udp6_packet(
            [
                0x20, 1, 0x48, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0x88,
            ],
            assigned,
        );
        rewrite_v6(&mut p, Field::Destination, &assigned, &local);
        assert_eq!(&p[24..40], &local);
        assert_eq!(u16::from_be_bytes([p[46], p[47]]), udp6_csum(&p));
    }

    #[test]
    fn ignores_short_packets() {
        let mut p = vec![0x45, 0, 0];
        let before = p.clone();
        rewrite_v4(&mut p, Field::Source, &[10, 64, 0, 1], &[10, 66, 0, 4]);
        assert_eq!(p, before);
    }
}
