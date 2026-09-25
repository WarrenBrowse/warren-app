//! Where the headers of a raw IP packet, as read from a TUN device, are.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub(crate) const PROTO_ICMP: u8 = 1;
pub(crate) const PROTO_TCP: u8 = 6;
pub(crate) const PROTO_UDP: u8 = 17;
pub(crate) const PROTO_ICMPV6: u8 = 58;

const V6_HOP_BY_HOP: u8 = 0;
const V6_ROUTING: u8 = 43;
const V6_FRAGMENT: u8 = 44;
const V6_AUTH: u8 = 51;
const V6_DEST_OPTS: u8 = 60;

/// Longest chain of IPv6 extension headers walked before giving up. Real
/// traffic carries one or two; the bound keeps a crafted packet from costing
/// more than a fixed amount of work.
const MAX_V6_EXTENSIONS: usize = 8;

/// Why a buffer is not a packet this crate can classify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PacketError {
    #[error("the packet is shorter than its headers")]
    Truncated,
    #[error("the packet is neither IPv4 nor IPv6")]
    NotIp,
    #[error("the packet carries a header chain this router does not follow")]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    V4,
    V6,
}

/// The fragment a packet is, when it is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fragment {
    pub id: u32,
    /// The protocol every fragment of the datagram names: the IPv4 protocol
    /// field, or the Next Header of the IPv6 Fragment header, which a later
    /// fragment carries too while the headers after it are only in the first.
    pub protocol: u8,
    /// The first fragment is the only one that carries the transport header.
    pub first: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    pub family: Family,
    /// The upper-layer protocol, after any IPv6 extension headers.
    pub protocol: u8,
    /// Offset of the upper-layer header, valid only when [`Self::has_transport`].
    pub l4_offset: usize,
    /// End of the datagram in the buffer, from the IP length field.
    pub end: usize,
    pub fragment: Option<Fragment>,
}

impl Layout {
    pub fn has_transport(&self) -> bool {
        self.fragment.is_none_or(|fragment| fragment.first)
    }

    pub fn src(&self, packet: &[u8]) -> IpAddr {
        read_addr(self.family, &packet[self.src_range()])
    }

    pub fn dst(&self, packet: &[u8]) -> IpAddr {
        read_addr(self.family, &packet[self.dst_range()])
    }

    pub fn src_range(&self) -> std::ops::Range<usize> {
        match self.family {
            Family::V4 => 12..16,
            Family::V6 => 8..24,
        }
    }

    pub fn dst_range(&self) -> std::ops::Range<usize> {
        match self.family {
            Family::V4 => 16..20,
            Family::V6 => 24..40,
        }
    }
}

pub(crate) fn read_addr(family: Family, bytes: &[u8]) -> IpAddr {
    match family {
        Family::V4 => IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        Family::V6 => {
            let octets: [u8; 16] = bytes[..16].try_into().expect("16 address bytes");
            IpAddr::V6(Ipv6Addr::from(octets))
        }
    }
}

/// Finds the headers of `packet`.
///
/// # Errors
///
/// [`PacketError::Truncated`] when a length field points past the buffer,
/// [`PacketError::NotIp`] for any other version nibble, and
/// [`PacketError::Unsupported`] for an IPv6 header chain that ends without an
/// upper layer or is longer than this router walks.
pub(crate) fn locate(packet: &[u8]) -> Result<Layout, PacketError> {
    locate_with(packet, false)
}

/// Like [`locate`], for the packet an ICMP error quotes: routers quote only
/// its beginning, so its length field may point past what is there.
pub(crate) fn locate_quoted(packet: &[u8]) -> Result<Layout, PacketError> {
    locate_with(packet, true)
}

fn locate_with(packet: &[u8], quoted: bool) -> Result<Layout, PacketError> {
    match packet.first().map(|byte| byte >> 4) {
        Some(4) => locate_v4(packet, quoted),
        Some(6) => locate_v6(packet, quoted),
        Some(_) => Err(PacketError::NotIp),
        None => Err(PacketError::Truncated),
    }
}

fn locate_v4(packet: &[u8], quoted: bool) -> Result<Layout, PacketError> {
    if packet.len() < 20 {
        return Err(PacketError::Truncated);
    }
    let header_len = usize::from(packet[0] & 0x0f) * 4;
    let mut total_len = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if quoted {
        total_len = total_len.min(packet.len());
    }
    if header_len < 20 || total_len < header_len || total_len > packet.len() {
        return Err(PacketError::Truncated);
    }
    let flags_offset = u16::from_be_bytes([packet[6], packet[7]]);
    let more_fragments = flags_offset & 0x2000 != 0;
    let offset = flags_offset & 0x1fff;
    let fragment = (more_fragments || offset != 0).then(|| Fragment {
        id: u32::from(u16::from_be_bytes([packet[4], packet[5]])),
        protocol: packet[9],
        first: offset == 0,
    });
    Ok(Layout {
        family: Family::V4,
        protocol: packet[9],
        l4_offset: header_len,
        end: total_len,
        fragment,
    })
}

fn locate_v6(packet: &[u8], quoted: bool) -> Result<Layout, PacketError> {
    if packet.len() < 40 {
        return Err(PacketError::Truncated);
    }
    let mut end = 40 + usize::from(u16::from_be_bytes([packet[4], packet[5]]));
    if quoted {
        end = end.min(packet.len());
    }
    if end > packet.len() {
        return Err(PacketError::Truncated);
    }
    let mut next = packet[6];
    let mut offset = 40;
    let mut fragment = None;
    for _ in 0..=MAX_V6_EXTENSIONS {
        let header_len = match next {
            PROTO_TCP | PROTO_UDP | PROTO_ICMPV6 => {
                return Ok(Layout {
                    family: Family::V6,
                    protocol: next,
                    l4_offset: offset,
                    end,
                    fragment,
                });
            }
            V6_HOP_BY_HOP | V6_ROUTING | V6_DEST_OPTS => {
                let header = packet
                    .get(offset..offset + 2)
                    .ok_or(PacketError::Truncated)?;
                (usize::from(header[1]) + 1) * 8
            }
            V6_AUTH => {
                let header = packet
                    .get(offset..offset + 2)
                    .ok_or(PacketError::Truncated)?;
                (usize::from(header[1]) + 2) * 4
            }
            V6_FRAGMENT => {
                let header = packet
                    .get(offset..offset + 8)
                    .ok_or(PacketError::Truncated)?;
                let field = u16::from_be_bytes([header[2], header[3]]);
                fragment = Some(Fragment {
                    id: u32::from_be_bytes([header[4], header[5], header[6], header[7]]),
                    protocol: header[0],
                    first: field >> 3 == 0,
                });
                8
            }
            _ => return Err(PacketError::Unsupported),
        };
        if offset + header_len > end {
            return Err(PacketError::Truncated);
        }
        next = packet[offset];
        offset += header_len;
        // Past a later fragment the upper-layer header is in another packet,
        // so the chain ends here with what the fragment header announced.
        if fragment.is_some_and(|fragment: Fragment| !fragment.first) {
            return Ok(Layout {
                family: Family::V6,
                protocol: next,
                l4_offset: offset,
                end,
                fragment,
            });
        }
    }
    Err(PacketError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    const A: [u8; 4] = [10, 0, 0, 2];
    const B: [u8; 4] = [192, 0, 2, 7];
    const A6: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
    const B6: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7];

    #[test]
    fn locates_the_transport_header_of_a_plain_ipv4_packet() {
        let packet = udp_v4(A, B, 5000, 53, b"q");

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.family, Family::V4);
        assert_eq!(layout.protocol, PROTO_UDP);
        assert_eq!(layout.l4_offset, 20);
        assert_eq!(layout.end, packet.len());
        assert_eq!(layout.fragment, None);
        assert_eq!(layout.src(&packet), IpAddr::from(A));
        assert_eq!(layout.dst(&packet), IpAddr::from(B));
    }

    #[test]
    fn skips_ipv4_options() {
        let mut packet = udp_v4(A, B, 5000, 53, b"q");
        // Grow the header by one word of options (a NOP run).
        packet.splice(20..20, [1u8, 1, 1, 1]);
        packet[0] = 0x46;
        let total = packet.len() as u16;
        packet[2..4].copy_from_slice(&total.to_be_bytes());

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.l4_offset, 24);
    }

    #[test]
    fn ignores_link_padding_after_the_ipv4_datagram() {
        let mut packet = udp_v4(A, B, 5000, 53, b"q");
        let datagram_len = packet.len();
        packet.extend_from_slice(&[0; 6]);

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.end, datagram_len);
    }

    #[test]
    fn marks_the_first_ipv4_fragment_as_carrying_the_transport_header() {
        let packet = as_v4_fragment(udp_v4(A, B, 5000, 53, b"q"), 0x4242, 0, true);

        let layout = locate(&packet).unwrap();

        assert_eq!(
            layout.fragment,
            Some(Fragment {
                id: 0x4242,
                protocol: PROTO_UDP,
                first: true
            })
        );
        assert!(layout.has_transport());
    }

    #[test]
    fn marks_a_later_ipv4_fragment_as_without_transport_header() {
        let packet = as_v4_fragment(udp_v4(A, B, 5000, 53, b"payload!"), 0x4242, 1, false);

        let layout = locate(&packet).unwrap();

        assert_eq!(
            layout.fragment,
            Some(Fragment {
                id: 0x4242,
                protocol: PROTO_UDP,
                first: false
            })
        );
        assert!(!layout.has_transport());
    }

    #[test]
    fn rejects_an_ipv4_length_past_the_buffer() {
        let mut packet = udp_v4(A, B, 5000, 53, b"q");
        packet.truncate(packet.len() - 1);

        assert_eq!(locate(&packet), Err(PacketError::Truncated));
    }

    #[test]
    fn rejects_an_ipv4_header_length_below_the_minimum() {
        let mut packet = udp_v4(A, B, 5000, 53, b"q");
        packet[0] = 0x44;

        assert_eq!(locate(&packet), Err(PacketError::Truncated));
    }

    #[test]
    fn rejects_a_version_that_is_not_ip() {
        let mut packet = udp_v4(A, B, 5000, 53, b"q");
        packet[0] = 0x55;

        assert_eq!(locate(&packet), Err(PacketError::NotIp));
    }

    #[test]
    fn locates_the_transport_header_of_a_plain_ipv6_packet() {
        let packet = tcp_v6(A6, B6, 5000, 443, TCP_SYN, b"");

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.family, Family::V6);
        assert_eq!(layout.protocol, PROTO_TCP);
        assert_eq!(layout.l4_offset, 40);
        assert_eq!(layout.src(&packet), IpAddr::from(A6));
        assert_eq!(layout.dst(&packet), IpAddr::from(B6));
    }

    #[test]
    fn walks_ipv6_extension_headers_to_the_upper_layer() {
        let packet = as_v6_fragment(udp_v6(A6, B6, 5000, 53, b"q"), 7, 0, true, true);

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.protocol, PROTO_UDP);
        assert_eq!(layout.l4_offset, 56);
        assert_eq!(
            layout.fragment,
            Some(Fragment {
                id: 7,
                protocol: PROTO_UDP,
                first: true
            })
        );
    }

    #[test]
    fn marks_a_later_ipv6_fragment_as_without_transport_header() {
        let packet = as_v6_fragment(udp_v6(A6, B6, 5000, 53, b"payload!"), 7, 1, false, false);

        let layout = locate(&packet).unwrap();

        assert_eq!(
            layout.fragment,
            Some(Fragment {
                id: 7,
                protocol: PROTO_UDP,
                first: false
            })
        );
        assert!(!layout.has_transport());
    }

    #[test]
    fn stops_at_the_fragment_header_of_a_later_ipv6_fragment() {
        // The fragment header announces destination options, but those live in
        // the first fragment: what follows here is payload, whatever it reads.
        let mut packet = as_v6_fragment(udp_v6(A6, B6, 5000, 53, &[59; 16]), 7, 1, false, false);
        packet[40] = V6_DEST_OPTS;

        let layout = locate(&packet).unwrap();

        assert_eq!(layout.protocol, V6_DEST_OPTS);
        assert_eq!(
            layout.fragment,
            Some(Fragment {
                id: 7,
                protocol: V6_DEST_OPTS,
                first: false
            })
        );
    }

    #[test]
    fn rejects_an_ipv6_payload_length_past_the_buffer() {
        let mut packet = udp_v6(A6, B6, 5000, 53, b"q");
        packet.truncate(packet.len() - 1);

        assert_eq!(locate(&packet), Err(PacketError::Truncated));
    }

    #[test]
    fn rejects_an_ipv6_chain_without_upper_layer() {
        let mut packet = udp_v6(A6, B6, 5000, 53, b"q");
        packet[6] = 59;

        assert_eq!(locate(&packet), Err(PacketError::Unsupported));
    }

    #[test]
    fn rejects_an_ipv6_chain_longer_than_the_walk_bound() {
        let mut packet = ipv6_with_extensions(MAX_V6_EXTENSIONS + 1);
        assert_eq!(locate(&packet), Err(PacketError::Unsupported));

        packet = ipv6_with_extensions(MAX_V6_EXTENSIONS);
        assert_eq!(locate(&packet).unwrap().protocol, PROTO_UDP);
    }

    /// An IPv6 UDP packet behind `count` destination-options headers.
    fn ipv6_with_extensions(count: usize) -> Vec<u8> {
        let udp = udp_v6(A6, B6, 5000, 53, b"q");
        let mut packet = udp[..40].to_vec();
        packet[6] = V6_DEST_OPTS;
        for index in 0..count {
            let next = if index + 1 == count {
                PROTO_UDP
            } else {
                V6_DEST_OPTS
            };
            packet.extend_from_slice(&[next, 0, 1, 4, 0, 0, 0, 0]);
        }
        packet.extend_from_slice(&udp[40..]);
        let payload_len = (packet.len() - 40) as u16;
        packet[4..6].copy_from_slice(&payload_len.to_be_bytes());
        packet
    }
}
