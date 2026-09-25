//! In-place address translation for routed flows.
//!
//! A route session has its own inner address. Uplink, the source of a routed
//! packet is rewritten from the main tunnel address to the route address;
//! downlink, the destination is rewritten back. Ports never change, so each
//! rewrite touches only addresses and the checksums that cover them, and every
//! checksum is updated incrementally (RFC 1624) rather than recomputed.

use std::net::IpAddr;

use crate::ip::{self, Family, Layout, PacketError};

/// Why a packet could not be translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NatError {
    #[error("the packet cannot be parsed")]
    Packet(#[from] PacketError),
    #[error("the new address is not of the packet's IP version")]
    FamilyMismatch,
}

/// Rewrites the source address of `packet` to `new`.
///
/// # Errors
///
/// [`NatError::Packet`] for a malformed packet and [`NatError::FamilyMismatch`]
/// when `new` is not of the packet's IP version. The packet is left untouched
/// on error.
pub fn rewrite_source(packet: &mut [u8], new: IpAddr) -> Result<(), NatError> {
    rewrite(packet, new, Side::Source)
}

/// Rewrites the destination address of `packet` to `new`. For an ICMP error,
/// the source of the packet it quotes is the same address, and is rewritten
/// too so the local stack recognises its own packet.
///
/// # Errors
///
/// As [`rewrite_source`].
pub fn rewrite_destination(packet: &mut [u8], new: IpAddr) -> Result<(), NatError> {
    rewrite(packet, new, Side::Destination)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Source,
    Destination,
}

/// An address as the bytes it occupies in a header.
struct AddrBytes {
    bytes: [u8; 16],
    len: usize,
}

impl AddrBytes {
    fn of(addr: IpAddr) -> Self {
        let mut bytes = [0; 16];
        let len = match addr {
            IpAddr::V4(v4) => {
                bytes[..4].copy_from_slice(&v4.octets());
                4
            }
            IpAddr::V6(v6) => {
                bytes.copy_from_slice(&v6.octets());
                16
            }
        };
        Self { bytes, len }
    }

    fn from_slice(slice: &[u8]) -> Self {
        let mut bytes = [0; 16];
        bytes[..slice.len()].copy_from_slice(slice);
        Self {
            bytes,
            len: slice.len(),
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn rewrite(packet: &mut [u8], new: IpAddr, side: Side) -> Result<(), NatError> {
    let layout = ip::locate(packet)?;
    let new = AddrBytes::of(new);
    let family_len = match layout.family {
        Family::V4 => 4,
        Family::V6 => 16,
    };
    if new.len != family_len {
        return Err(NatError::FamilyMismatch);
    }
    let range = match side {
        Side::Source => layout.src_range(),
        Side::Destination => layout.dst_range(),
    };
    let old = AddrBytes::from_slice(&packet[range.clone()]);
    packet[range].copy_from_slice(new.as_slice());
    if layout.family == Family::V4 {
        update_checksum(packet, 10, old.as_slice(), new.as_slice());
    }
    if layout.has_transport() {
        rewrite_transport(packet, &layout, &old, &new, side);
    }
    Ok(())
}

/// Fixes the transport checksum after an address change, and for a
/// translated ICMP error the packet it quotes.
fn rewrite_transport(
    packet: &mut [u8],
    layout: &Layout,
    old: &AddrBytes,
    new: &AddrBytes,
    side: Side,
) {
    let l4 = layout.l4_offset;
    let available = layout.end.saturating_sub(l4);
    match layout.protocol {
        ip::PROTO_TCP if available >= 18 => {
            update_checksum(packet, l4 + 16, old.as_slice(), new.as_slice());
        }
        ip::PROTO_UDP if available >= 8 => {
            let absent = packet[l4 + 6] == 0 && packet[l4 + 7] == 0;
            // Over IPv4 a zero UDP checksum means "none", and stays none.
            if layout.family == Family::V6 || !absent {
                update_checksum(packet, l4 + 6, old.as_slice(), new.as_slice());
                if packet[l4 + 6] == 0 && packet[l4 + 7] == 0 {
                    packet[l4 + 6] = 0xff;
                    packet[l4 + 7] = 0xff;
                }
            }
        }
        ip::PROTO_ICMP | ip::PROTO_ICMPV6 if available >= 8 => {
            if layout.protocol == ip::PROTO_ICMPV6 {
                update_checksum(packet, l4 + 2, old.as_slice(), new.as_slice());
            }
            if side == Side::Destination && is_icmp_error(layout.family, packet[l4]) {
                rewrite_quoted_source(packet, layout, old, new);
            }
        }
        _ => {}
    }
}

fn is_icmp_error(family: Family, message_type: u8) -> bool {
    match family {
        Family::V4 => matches!(message_type, 3 | 11 | 12),
        Family::V6 => matches!(message_type, 1..=4),
    }
}

/// The packet an ICMP error quotes was sent from the address being restored:
/// rewrite its source as well, keeping its header checksum and the ICMP
/// checksum that covers it valid.
fn rewrite_quoted_source(packet: &mut [u8], layout: &Layout, old: &AddrBytes, new: &AddrBytes) {
    let icmp = layout.l4_offset;
    let quoted_start = icmp + 8;
    let Ok(quoted) = ip::locate_quoted(&packet[quoted_start..layout.end]) else {
        return;
    };
    if quoted.family != layout.family {
        return;
    }
    let src = quoted.src_range();
    let src = quoted_start + src.start..quoted_start + src.end;
    if &packet[src.clone()] != old.as_slice() {
        return;
    }
    packet[src].copy_from_slice(new.as_slice());
    update_checksum(packet, icmp + 2, old.as_slice(), new.as_slice());
    if quoted.family == Family::V4 {
        let field = quoted_start + 10;
        let before = [packet[field], packet[field + 1]];
        update_checksum(packet, field, old.as_slice(), new.as_slice());
        let after = [packet[field], packet[field + 1]];
        update_checksum(packet, icmp + 2, &before, &after);
    }
}

/// Updates the checksum stored at `at` for a change of the covered bytes from
/// `old` to `new` (RFC 1624, equation 3). Both are whole 16-bit words.
fn update_checksum(packet: &mut [u8], at: usize, old: &[u8], new: &[u8]) {
    let stored = u16::from_be_bytes([packet[at], packet[at + 1]]);
    let mut sum = u32::from(!stored);
    for word in old.chunks_exact(2) {
        sum += u32::from(!u16::from_be_bytes([word[0], word[1]]));
    }
    for word in new.chunks_exact(2) {
        sum += u32::from(u16::from_be_bytes([word[0], word[1]]));
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    packet[at..at + 2].copy_from_slice(&(!(sum as u16)).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const MAIN: [u8; 4] = [10, 64, 0, 2];
    const ROUTE: [u8; 4] = [10, 99, 17, 201];
    const REMOTE: [u8; 4] = [198, 51, 100, 9];
    const MAIN6: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
    const ROUTE6: [u8; 16] = [0xfd, 0x99, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xab, 0xcd, 0, 7];
    const REMOTE6: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];

    fn v4(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }

    fn v6(octets: [u8; 16]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(octets))
    }

    #[test]
    fn rewrites_the_source_of_a_tcp_packet_with_valid_checksums() {
        let mut packet = tcp_v4(MAIN, REMOTE, 50000, 443, TCP_ACK, b"hello");

        rewrite_source(&mut packet, v4(ROUTE)).unwrap();

        assert_eq!(packet, tcp_v4(ROUTE, REMOTE, 50000, 443, TCP_ACK, b"hello"));
        assert!(ipv4_header_ok(&packet));
        assert!(transport_ok(&packet));
    }

    #[test]
    fn rewrites_the_source_of_a_udp_packet_with_valid_checksums() {
        let mut packet = udp_v4(MAIN, REMOTE, 50000, 443, b"quic initial");

        rewrite_source(&mut packet, v4(ROUTE)).unwrap();

        assert_eq!(packet, udp_v4(ROUTE, REMOTE, 50000, 443, b"quic initial"));
    }

    #[test]
    fn leaves_an_absent_udp_checksum_absent() {
        let mut packet = udp_v4_unchecked(MAIN, REMOTE, 50000, 53, b"query");

        rewrite_source(&mut packet, v4(ROUTE)).unwrap();

        assert_eq!(packet, udp_v4_unchecked(ROUTE, REMOTE, 50000, 53, b"query"));
    }

    #[test]
    fn never_turns_a_udp_checksum_into_the_absent_value() {
        // Find a datagram whose translated checksum computes to zero, which
        // UDP must transmit as all ones.
        let (mut packet, expected) = (0..=u16::MAX)
            .find_map(|word| {
                let payload = word.to_be_bytes();
                let expected = udp_v4(ROUTE, REMOTE, 50000, 443, &payload);
                let mut zeroed = expected.clone();
                zeroed[26] = 0;
                zeroed[27] = 0;
                let mut pseudo = ROUTE.to_vec();
                pseudo.extend_from_slice(&REMOTE);
                pseudo.extend_from_slice(&[0, 17, 0, 10]);
                (checksum(&[&pseudo, &zeroed[20..]]) == 0)
                    .then(|| (udp_v4(MAIN, REMOTE, 50000, 443, &payload), expected))
            })
            .expect("some payload word sums to a zero checksum");

        rewrite_source(&mut packet, v4(ROUTE)).unwrap();

        assert_eq!(&packet[26..28], &[0xff, 0xff]);
        assert_eq!(packet, expected);
    }

    #[test]
    fn an_icmp_echo_keeps_its_checksum_since_icmpv4_has_no_pseudo_header() {
        let mut packet = icmp_echo_v4(MAIN, REMOTE, 9, true);

        rewrite_source(&mut packet, v4(ROUTE)).unwrap();

        assert_eq!(packet, icmp_echo_v4(ROUTE, REMOTE, 9, true));
    }

    #[test]
    fn rewrites_ipv6_transports_with_valid_checksums() {
        let mut tcp = tcp_v6(MAIN6, REMOTE6, 50000, 443, TCP_SYN, b"");
        let mut udp = udp_v6(MAIN6, REMOTE6, 50000, 443, b"quic");
        let mut echo = icmp_echo_v6(MAIN6, REMOTE6, 3, true);

        rewrite_source(&mut tcp, v6(ROUTE6)).unwrap();
        rewrite_source(&mut udp, v6(ROUTE6)).unwrap();
        rewrite_source(&mut echo, v6(ROUTE6)).unwrap();

        assert_eq!(tcp, tcp_v6(ROUTE6, REMOTE6, 50000, 443, TCP_SYN, b""));
        assert_eq!(udp, udp_v6(ROUTE6, REMOTE6, 50000, 443, b"quic"));
        assert_eq!(echo, icmp_echo_v6(ROUTE6, REMOTE6, 3, true));
    }

    #[test]
    fn rewrites_the_destination_of_an_answer() {
        let mut packet = tcp_v4(REMOTE, ROUTE, 443, 50000, TCP_ACK, b"response");

        rewrite_destination(&mut packet, v4(MAIN)).unwrap();

        assert_eq!(
            packet,
            tcp_v4(REMOTE, MAIN, 443, 50000, TCP_ACK, b"response")
        );
    }

    #[test]
    fn an_icmp_error_gets_its_quoted_source_translated_too() {
        let sent_translated = udp_v4(ROUTE, REMOTE, 50000, 443, &[7; 32]);
        let sent_original = udp_v4(MAIN, REMOTE, 50000, 443, &[7; 32]);
        let router = [203, 0, 113, 1];
        let mut packet = icmp_v4_error(router, ROUTE, &sent_translated[..28]);

        rewrite_destination(&mut packet, v4(MAIN)).unwrap();

        // The quoted UDP checksum still covers the translated source: the
        // local stack does not verify a quoted transport checksum.
        let mut quoted = sent_original[..28].to_vec();
        quoted[26..28].copy_from_slice(&sent_translated[26..28]);
        assert_eq!(packet, icmp_v4_error(router, MAIN, &quoted));
        assert!(ipv4_header_ok(&packet[28..]));
    }

    #[test]
    fn an_icmpv6_error_gets_its_quoted_source_translated_too() {
        let sent_translated = udp_v6(ROUTE6, REMOTE6, 50000, 443, &[7; 32]);
        let mut packet = icmp_v6_error(REMOTE6, ROUTE6, &sent_translated[..48]);

        rewrite_destination(&mut packet, v6(MAIN6)).unwrap();

        let mut quoted = sent_translated[..48].to_vec();
        quoted[8..24].copy_from_slice(&MAIN6);
        assert_eq!(packet, icmp_v6_error(REMOTE6, MAIN6, &quoted));
    }

    #[test]
    fn a_first_fragment_carries_the_checksum_of_the_whole_translated_datagram() {
        let whole = udp_v4(MAIN, REMOTE, 50000, 443, &[3; 64]);
        let mut first = as_v4_fragment(whole, 5, 0, true);

        rewrite_source(&mut first, v4(ROUTE)).unwrap();

        let translated = udp_v4(ROUTE, REMOTE, 50000, 443, &[3; 64]);
        assert_eq!(&first[12..16], &ROUTE);
        assert_eq!(&first[20..28], &translated[20..28]);
        assert!(ipv4_header_ok(&first));
    }

    #[test]
    fn a_later_fragment_changes_only_its_ip_header() {
        let original = as_v4_fragment(udp_v4(MAIN, REMOTE, 50000, 443, &[3; 64]), 5, 3, false);
        let mut later = original.clone();

        rewrite_source(&mut later, v4(ROUTE)).unwrap();

        assert_eq!(&later[12..16], &ROUTE);
        assert_eq!(&later[20..], &original[20..]);
        assert!(ipv4_header_ok(&later));
    }

    #[test]
    fn refuses_an_address_of_the_other_family_and_leaves_the_packet_alone() {
        let original = tcp_v4(MAIN, REMOTE, 50000, 443, TCP_SYN, b"");
        let mut packet = original.clone();

        let result = rewrite_source(&mut packet, v6(ROUTE6));

        assert_eq!(result, Err(NatError::FamilyMismatch));
        assert_eq!(packet, original);
    }

    #[test]
    fn refuses_a_malformed_packet() {
        let mut packet = tcp_v4(MAIN, REMOTE, 50000, 443, TCP_SYN, b"");
        packet.truncate(10);

        assert_eq!(
            rewrite_destination(&mut packet, v4(ROUTE)),
            Err(NatError::Packet(PacketError::Truncated))
        );
    }
}
