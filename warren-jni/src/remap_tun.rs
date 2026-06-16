// 1:1 inner-IPv4 NAT wrapper around the Android VpnService TUN.
//
// The multi-hop exit allocates a tunnel-inner IPv4 (e.g. 10.66.0.4) via the
// `IpAssign` control message and then ENFORCES it: every uplink packet whose
// source address is not the assigned IP is dropped as spoofed
// (`warren_exit_core::multihop` gates on `source_ip_matches`). The desktop
// client satisfies this by reassigning its TUN to the allocated address
// (`RealTun::reassign_ipv4`). Android cannot: a `VpnService` interface address
// is fixed when `establish()` returns the fd, and the fd is handed to JNI
// before the IP is negotiated, so the interface stays on the static
// `WarrenTunDefaults.IPV4_ADDRESS` (10.64.0.1).
//
// This wrapper bridges the gap with a stateless 1:1 NAT in the data pump:
//   - uplink (TUN -> network): rewrite source 10.64.0.1 -> assigned
//   - downlink (network -> TUN): rewrite destination assigned -> 10.64.0.1
// so the exit sees packets sourced from the address it assigned, and apps
// (bound to 10.64.0.1) accept the replies. Header and L4 checksums are fixed
// incrementally (RFC 1624). Without this the tunnel reports CONNECTED but
// carries no traffic (every uplink packet is silently dropped at the exit).

#[cfg(all(target_os = "android", feature = "tunnel"))]
use std::net::Ipv4Addr;

#[cfg(all(target_os = "android", feature = "tunnel"))]
use warren_tunnel::PacketDevice;

/// Wraps a [`PacketDevice`] and rewrites the tunnel-inner client IPv4 so the
/// kernel-fixed Android TUN address (`local`) is presented to the exit as the
/// exit-assigned address (`assigned`).
#[cfg(all(target_os = "android", feature = "tunnel"))]
#[derive(Clone)]
pub struct RemapTun<T> {
    inner: T,
    local: [u8; 4],
    assigned: [u8; 4],
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
impl<T> RemapTun<T> {
    /// Build a remapping TUN. `local` is the address the Android interface is
    /// configured with (10.64.0.1); `assigned` is the exit-allocated inner
    /// IPv4. A no-op remap (`local == assigned`) still works but is wasteful;
    /// callers skip the wrapper in that case.
    pub fn new(inner: T, local: Ipv4Addr, assigned: Ipv4Addr) -> Self {
        Self {
            inner,
            local: local.octets(),
            assigned: assigned.octets(),
        }
    }
}

#[cfg(all(target_os = "android", feature = "tunnel"))]
impl<T: PacketDevice + Clone> PacketDevice for RemapTun<T> {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let mut pkt = self.inner.recv().await?;
        // Uplink: app source 10.64.0.1 -> exit-assigned address.
        rewrite_address(&mut pkt, Field::Source, &self.local, &self.assigned);
        Ok(pkt)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        // Downlink: exit-assigned destination -> app address 10.64.0.1.
        // Only allocate a rewritten copy when the destination actually
        // matches; otherwise forward the borrow untouched.
        if needs_dst_rewrite(packet, &self.assigned) {
            let mut pkt = packet.to_vec();
            rewrite_address(&mut pkt, Field::Destination, &self.assigned, &self.local);
            self.inner.send(&pkt).await
        } else {
            self.inner.send(packet).await
        }
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        match self.inner.try_recv()? {
            Some(mut pkt) => {
                rewrite_address(&mut pkt, Field::Source, &self.local, &self.assigned);
                Ok(Some(pkt))
            }
            None => Ok(None),
        }
    }
}

#[derive(Clone, Copy)]
enum Field {
    Source,
    Destination,
}

/// True if `packet` is an IPv4 packet whose destination equals `want_dst`.
fn needs_dst_rewrite(packet: &[u8], want_dst: &[u8; 4]) -> bool {
    if !is_ipv4(packet) {
        return false;
    }
    packet.get(16..20) == Some(&want_dst[..])
}

/// True if `packet` looks like an IPv4 packet (version nibble 4, min header).
fn is_ipv4(packet: &[u8]) -> bool {
    packet.len() >= 20 && (packet[0] >> 4) == 4
}

/// Rewrite the source or destination IPv4 address from `from` to `to`,
/// fixing the IPv4 header checksum and (for the first fragment of TCP/UDP)
/// the transport checksum incrementally. No-op for non-IPv4 packets, packets
/// too short, or packets whose field does not match `from`.
fn rewrite_address(packet: &mut [u8], field: Field, from: &[u8; 4], to: &[u8; 4]) {
    if !is_ipv4(packet) {
        return;
    }
    let (addr_off, _is_src) = match field {
        Field::Source => (12usize, true),
        Field::Destination => (16usize, false),
    };
    if packet.get(addr_off..addr_off + 4) != Some(&from[..]) {
        return;
    }
    let ihl = ((packet[0] & 0x0f) as usize) * 4;
    if ihl < 20 || packet.len() < ihl {
        return;
    }

    // Write the new address.
    packet[addr_off..addr_off + 4].copy_from_slice(to);

    // Fix the IPv4 header checksum (bytes 10..12).
    let ip_check = u16::from_be_bytes([packet[10], packet[11]]);
    let new_ip_check = checksum_replace(ip_check, from, to);
    packet[10..12].copy_from_slice(&new_ip_check.to_be_bytes());

    // Transport checksum: only the first fragment carries it. The
    // fragment-offset field is the low 13 bits of bytes 6..8; MF/DF live in
    // the top 3 bits. A non-zero offset means a later fragment with no L4
    // header, so leave the transport payload untouched.
    let frag_off = u16::from_be_bytes([packet[6], packet[7]]) & 0x1fff;
    if frag_off != 0 {
        return;
    }

    let protocol = packet[9];
    match protocol {
        6 => fix_l4_checksum(packet, ihl, 16, from, to, false), // TCP
        17 => fix_l4_checksum(packet, ihl, 6, from, to, true),  // UDP
        _ => {} // ICMP and others do not checksum the IP addresses.
    }
}

/// Incrementally fix a transport checksum located at `ihl + check_off`.
///
/// `udp` marks the UDP quirk where a literal `0x0000` means "no checksum"
/// (left untouched) and a computed `0x0000` must be transmitted as `0xffff`.
fn fix_l4_checksum(packet: &mut [u8], ihl: usize, check_off: usize, from: &[u8; 4], to: &[u8; 4], udp: bool) {
    let off = ihl + check_off;
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

/// RFC 1624 incremental checksum update for a changed 4-byte field:
/// `HC' = ~(~HC + ~m + m')`, computed over the two 16-bit halves.
fn checksum_replace(check: u16, old: &[u8; 4], new: &[u8; 4]) -> u16 {
    let mut sum: u32 = (!check) as u32;
    let mut i = 0;
    while i < 4 {
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

    // Build a minimal IPv4 + UDP packet for checksum assertions.
    fn udp_packet(src: [u8; 4], dst: [u8; 4]) -> Vec<u8> {
        let mut p = vec![0u8; 20 + 8];
        p[0] = 0x45; // version 4, IHL 5
        let total = (20u16 + 8).to_be_bytes();
        p[2] = total[0];
        p[3] = total[1];
        p[9] = 17; // UDP
        p[12..16].copy_from_slice(&src);
        p[16..20].copy_from_slice(&dst);
        // UDP header: sports/dports/len then a non-zero checksum.
        p[20..22].copy_from_slice(&1234u16.to_be_bytes());
        p[22..24].copy_from_slice(&53u16.to_be_bytes());
        p[24..26].copy_from_slice(&8u16.to_be_bytes());
        // Set IP + UDP checksums to their correct initial values.
        let ipc = ones_complement_ip_header(&p);
        p[10..12].copy_from_slice(&ipc.to_be_bytes());
        let uc = ones_complement_udp(&p);
        p[26..28].copy_from_slice(&uc.to_be_bytes());
        p
    }

    fn ones_complement_ip_header(p: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        let mut i = 0;
        while i < 20 {
            if i == 10 {
                i += 2;
                continue; // skip the checksum field itself
            }
            sum += u16::from_be_bytes([p[i], p[i + 1]]) as u32;
            i += 2;
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        !(sum as u16)
    }

    fn ones_complement_udp(p: &[u8]) -> u16 {
        // Pseudo-header: src(4) dst(4) zero+proto(2) udp_len(2) + udp datagram.
        let mut sum: u32 = 0;
        for chunk in p[12..20].chunks(2) {
            sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        }
        sum += 17; // protocol
        let udp_len = u16::from_be_bytes([p[24], p[25]]);
        sum += udp_len as u32;
        let mut i = 20;
        while i + 1 < p.len() {
            if i == 26 {
                i += 2;
                continue; // skip the checksum field itself
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
    fn rewrites_source_and_keeps_checksums_valid() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let mut p = udp_packet(local, [1, 1, 1, 1]);
        rewrite_address(&mut p, Field::Source, &local, &assigned);
        assert_eq!(&p[12..16], &assigned, "source rewritten");
        // Recompute from scratch: the incremental result must match.
        assert_eq!(
            u16::from_be_bytes([p[10], p[11]]),
            ones_complement_ip_header(&p),
            "IP header checksum stays valid"
        );
        assert_eq!(
            u16::from_be_bytes([p[26], p[27]]),
            ones_complement_udp(&p),
            "UDP checksum stays valid"
        );
    }

    #[test]
    fn rewrites_destination_back() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let mut p = udp_packet([1, 1, 1, 1], assigned);
        assert!(needs_dst_rewrite(&p, &assigned));
        rewrite_address(&mut p, Field::Destination, &assigned, &local);
        assert_eq!(&p[16..20], &local, "destination rewritten");
        assert_eq!(
            u16::from_be_bytes([p[10], p[11]]),
            ones_complement_ip_header(&p),
            "IP header checksum stays valid"
        );
        assert_eq!(
            u16::from_be_bytes([p[26], p[27]]),
            ones_complement_udp(&p),
            "UDP checksum stays valid"
        );
    }

    #[test]
    fn leaves_non_matching_source_untouched() {
        let local = [10, 64, 0, 1];
        let assigned = [10, 66, 0, 4];
        let other = [192, 168, 0, 2];
        let mut p = udp_packet(other, [1, 1, 1, 1]);
        let before = p.clone();
        rewrite_address(&mut p, Field::Source, &local, &assigned);
        assert_eq!(p, before, "non-matching source is not rewritten");
    }

    #[test]
    fn ignores_non_ipv4() {
        let mut p = vec![0x60, 0, 0, 0, 0, 0, 0, 0]; // IPv6 version nibble
        let before = p.clone();
        rewrite_address(&mut p, Field::Source, &[10, 64, 0, 1], &[10, 66, 0, 4]);
        assert_eq!(p, before);
    }
}
