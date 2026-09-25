//! Packet builders and checksum verifiers for the unit tests.
//!
//! Every checksum here is computed from scratch over the whole header or
//! segment, the textbook way, so a test that verifies a translated packet
//! with these functions does not share any code with the incremental updates
//! it is checking.

pub const TCP_SYN: u8 = 0x02;
pub const TCP_ACK: u8 = 0x10;
pub const TCP_FIN: u8 = 0x01;
pub const TCP_RST: u8 = 0x04;

/// The Internet checksum of the concatenation of `parts`.
pub fn checksum(parts: &[&[u8]]) -> u16 {
    let bytes: Vec<u8> = parts.iter().flat_map(|part| part.iter().copied()).collect();
    let mut sum: u32 = 0;
    for chunk in bytes.chunks(2) {
        let word = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], 0])
        };
        sum += u32::from(word);
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn ipv4_header(src: [u8; 4], dst: [u8; 4], protocol: u8, payload_len: usize) -> Vec<u8> {
    let total = 20 + payload_len;
    let mut header = vec![
        0x45,
        0,
        (total >> 8) as u8,
        total as u8,
        0x12,
        0x34,
        0x40, // don't fragment
        0,
        64,
        protocol,
        0,
        0,
    ];
    header.extend_from_slice(&src);
    header.extend_from_slice(&dst);
    let sum = checksum(&[&header]);
    header[10..12].copy_from_slice(&sum.to_be_bytes());
    header
}

fn ipv6_header(src: [u8; 16], dst: [u8; 16], next_header: u8, payload_len: usize) -> Vec<u8> {
    let mut header = vec![0x60, 0, 0, 0];
    header.extend_from_slice(&(payload_len as u16).to_be_bytes());
    header.push(next_header);
    header.push(64);
    header.extend_from_slice(&src);
    header.extend_from_slice(&dst);
    header
}

fn pseudo_v4(src: [u8; 4], dst: [u8; 4], protocol: u8, len: usize) -> Vec<u8> {
    let mut pseudo = Vec::with_capacity(12);
    pseudo.extend_from_slice(&src);
    pseudo.extend_from_slice(&dst);
    pseudo.push(0);
    pseudo.push(protocol);
    pseudo.extend_from_slice(&(len as u16).to_be_bytes());
    pseudo
}

fn pseudo_v6(src: [u8; 16], dst: [u8; 16], next_header: u8, len: usize) -> Vec<u8> {
    let mut pseudo = Vec::with_capacity(40);
    pseudo.extend_from_slice(&src);
    pseudo.extend_from_slice(&dst);
    pseudo.extend_from_slice(&(len as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0, next_header]);
    pseudo
}

fn tcp_segment(sport: u16, dport: u16, flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut segment = Vec::with_capacity(20 + payload.len());
    segment.extend_from_slice(&sport.to_be_bytes());
    segment.extend_from_slice(&dport.to_be_bytes());
    segment.extend_from_slice(&0x0102_0304u32.to_be_bytes());
    segment.extend_from_slice(&0u32.to_be_bytes());
    segment.push(0x50);
    segment.push(flags);
    segment.extend_from_slice(&0xffffu16.to_be_bytes());
    segment.extend_from_slice(&[0, 0, 0, 0]);
    segment.extend_from_slice(payload);
    segment
}

fn udp_datagram(sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let len = 8 + payload.len();
    let mut datagram = Vec::with_capacity(len);
    datagram.extend_from_slice(&sport.to_be_bytes());
    datagram.extend_from_slice(&dport.to_be_bytes());
    datagram.extend_from_slice(&(len as u16).to_be_bytes());
    datagram.extend_from_slice(&[0, 0]);
    datagram.extend_from_slice(payload);
    datagram
}

fn icmp_echo(kind: u8, id: u16, payload: &[u8]) -> Vec<u8> {
    let mut message = vec![kind, 0, 0, 0];
    message.extend_from_slice(&id.to_be_bytes());
    message.extend_from_slice(&7u16.to_be_bytes());
    message.extend_from_slice(payload);
    message
}

fn finish_v4(
    src: [u8; 4],
    dst: [u8; 4],
    protocol: u8,
    mut l4: Vec<u8>,
    sum_at: Option<usize>,
) -> Vec<u8> {
    if let Some(at) = sum_at {
        let pseudo = if protocol == 1 {
            Vec::new()
        } else {
            pseudo_v4(src, dst, protocol, l4.len())
        };
        let mut sum = checksum(&[&pseudo, &l4]);
        if protocol == 17 && sum == 0 {
            sum = 0xffff;
        }
        l4[at..at + 2].copy_from_slice(&sum.to_be_bytes());
    }
    let mut packet = ipv4_header(src, dst, protocol, l4.len());
    packet.extend_from_slice(&l4);
    packet
}

fn finish_v6(
    src: [u8; 16],
    dst: [u8; 16],
    next_header: u8,
    mut l4: Vec<u8>,
    sum_at: usize,
) -> Vec<u8> {
    let pseudo = pseudo_v6(src, dst, next_header, l4.len());
    let mut sum = checksum(&[&pseudo, &l4]);
    if next_header == 17 && sum == 0 {
        sum = 0xffff;
    }
    l4[sum_at..sum_at + 2].copy_from_slice(&sum.to_be_bytes());
    let mut packet = ipv6_header(src, dst, next_header, l4.len());
    packet.extend_from_slice(&l4);
    packet
}

pub fn tcp_v4(
    src: [u8; 4],
    dst: [u8; 4],
    sport: u16,
    dport: u16,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    finish_v4(
        src,
        dst,
        6,
        tcp_segment(sport, dport, flags, payload),
        Some(16),
    )
}

pub fn udp_v4(src: [u8; 4], dst: [u8; 4], sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    finish_v4(src, dst, 17, udp_datagram(sport, dport, payload), Some(6))
}

/// A UDP over IPv4 datagram sent without a checksum (the field is zero).
pub fn udp_v4_unchecked(
    src: [u8; 4],
    dst: [u8; 4],
    sport: u16,
    dport: u16,
    payload: &[u8],
) -> Vec<u8> {
    finish_v4(src, dst, 17, udp_datagram(sport, dport, payload), None)
}

pub fn icmp_echo_v4(src: [u8; 4], dst: [u8; 4], id: u16, request: bool) -> Vec<u8> {
    let kind = if request { 8 } else { 0 };
    finish_v4(src, dst, 1, icmp_echo(kind, id, b"ping"), Some(2))
}

/// An ICMPv4 "fragmentation needed" error quoting `quoted`, as a router sends it.
pub fn icmp_v4_error(src: [u8; 4], dst: [u8; 4], quoted: &[u8]) -> Vec<u8> {
    let mut message = vec![3, 4, 0, 0, 0, 0, 0x05, 0x00];
    message.extend_from_slice(quoted);
    finish_v4(src, dst, 1, message, Some(2))
}

pub fn tcp_v6(
    src: [u8; 16],
    dst: [u8; 16],
    sport: u16,
    dport: u16,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    finish_v6(src, dst, 6, tcp_segment(sport, dport, flags, payload), 16)
}

pub fn udp_v6(src: [u8; 16], dst: [u8; 16], sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    finish_v6(src, dst, 17, udp_datagram(sport, dport, payload), 6)
}

pub fn icmp_echo_v6(src: [u8; 16], dst: [u8; 16], id: u16, request: bool) -> Vec<u8> {
    let kind = if request { 128 } else { 129 };
    finish_v6(src, dst, 58, icmp_echo(kind, id, b"ping"), 2)
}

/// An ICMPv6 "packet too big" error quoting `quoted`.
pub fn icmp_v6_error(src: [u8; 16], dst: [u8; 16], quoted: &[u8]) -> Vec<u8> {
    let mut message = vec![2, 0, 0, 0, 0, 0, 0x05, 0x00];
    message.extend_from_slice(quoted);
    finish_v6(src, dst, 58, message, 2)
}

/// Turns an IPv4 packet into a fragment: `offset_units` in 8-byte units,
/// `more` sets the MF flag. The header checksum is recomputed.
pub fn as_v4_fragment(mut packet: Vec<u8>, id: u16, offset_units: u16, more: bool) -> Vec<u8> {
    packet[4..6].copy_from_slice(&id.to_be_bytes());
    let flags = if more { 0x2000 } else { 0 } | offset_units;
    packet[6..8].copy_from_slice(&flags.to_be_bytes());
    packet[10] = 0;
    packet[11] = 0;
    let sum = checksum(&[&packet[..20]]);
    packet[10..12].copy_from_slice(&sum.to_be_bytes());
    packet
}

/// Inserts an IPv6 fragment header (and optionally a hop-by-hop header
/// before it) after the fixed header of `packet`.
pub fn as_v6_fragment(
    packet: Vec<u8>,
    id: u32,
    offset_units: u16,
    more: bool,
    hop_by_hop: bool,
) -> Vec<u8> {
    let upper = packet[6];
    let mut out = packet[..40].to_vec();
    let mut extensions = Vec::new();
    if hop_by_hop {
        out[6] = 0;
        extensions.extend_from_slice(&[44, 0, 1, 4, 0, 0, 0, 0]);
    } else {
        out[6] = 44;
    }
    let frag_field = (offset_units << 3) | u16::from(more);
    extensions.push(upper);
    extensions.push(0);
    extensions.extend_from_slice(&frag_field.to_be_bytes());
    extensions.extend_from_slice(&id.to_be_bytes());
    let payload_len = extensions.len() + packet.len() - 40;
    out[4..6].copy_from_slice(&(payload_len as u16).to_be_bytes());
    out.extend_from_slice(&extensions);
    out.extend_from_slice(&packet[40..]);
    out
}

/// Whether the IPv4 header checksum of `packet` verifies.
pub fn ipv4_header_ok(packet: &[u8]) -> bool {
    let ihl = usize::from(packet[0] & 0x0f) * 4;
    checksum(&[&packet[..ihl]]) == 0
}

/// Whether the transport checksum of an unfragmented packet without IPv6
/// extension headers verifies, pseudo-header included. A zero UDP over IPv4
/// checksum ("none") counts as valid.
pub fn transport_ok(packet: &[u8]) -> bool {
    if packet[0] >> 4 == 4 {
        let ihl = usize::from(packet[0] & 0x0f) * 4;
        let protocol = packet[9];
        let l4 = &packet[ihl..];
        if protocol == 17 && l4[6] == 0 && l4[7] == 0 {
            return true;
        }
        let pseudo = if protocol == 1 {
            Vec::new()
        } else {
            let src: [u8; 4] = packet[12..16].try_into().unwrap();
            let dst: [u8; 4] = packet[16..20].try_into().unwrap();
            pseudo_v4(src, dst, protocol, l4.len())
        };
        checksum(&[&pseudo, l4]) == 0
    } else {
        let src: [u8; 16] = packet[8..24].try_into().unwrap();
        let dst: [u8; 16] = packet[24..40].try_into().unwrap();
        let l4 = &packet[40..];
        checksum(&[&pseudo_v6(src, dst, packet[6], l4.len()), l4]) == 0
    }
}
