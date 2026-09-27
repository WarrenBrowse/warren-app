//! Resets toward an app: the TCP segments that end, on the app's side, a
//! connection whose route changed under it.
//!
//! A connection lives at the exit it was opened through. Once its app leaves
//! through another session, the old exit no longer sees its packets and the
//! new one drops them as unknown, so the app would wait on the connection
//! until its own timeouts. The router answers for the remote end instead,
//! with a reset the app's stack accepts: one whose sequence number is exactly
//! the next the app expects (RFC 9293 section 3.10.7.4, RFC 5961 section 3).
//! A UDP flow is told its port is unreachable (RFC 1122 section 4.1.3.3),
//! which a connected socket reports to its app as a refused connection.

use std::net::IpAddr;

use crate::{
    flow::{FlowKey, Transport},
    ip,
};

const FLAG_FIN: u8 = 0x01;
const FLAG_SYN: u8 = 0x02;
const FLAG_RST: u8 = 0x04;
const FLAG_ACK: u8 = 0x10;

const TCP_HEADER: usize = 20;

/// The numbers of a TCP segment an app sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment {
    seq: u32,
    ack: u32,
    flags: u8,
    /// Sequence space the segment takes: its payload, and one for a SYN and
    /// for a FIN.
    len: u32,
}

/// Where the app's side of a TCP connection stands, from its own segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct AppSide {
    /// The highest acknowledgment the app sent: the next sequence number it
    /// expects from the remote end, as of its latest segment.
    expects: Option<u32>,
    /// The sequence number after the furthest segment the app sent.
    next: Option<u32>,
}

impl AppSide {
    /// Takes in a segment the app sent on the connection.
    pub(crate) fn follow(&mut self, segment: &Segment) {
        if segment.flags & FLAG_RST != 0 {
            return;
        }
        if segment.flags & FLAG_ACK != 0 {
            self.expects = Some(furthest(self.expects, segment.ack));
        }
        self.next = Some(furthest(self.next, segment.seq.wrapping_add(segment.len)));
    }
}

/// The later of `current` and `candidate` in sequence space.
fn furthest(current: Option<u32>, candidate: u32) -> u32 {
    match current {
        // Serial number arithmetic: a sequence number is after another when
        // it is less than half the space ahead of it.
        Some(current) if candidate.wrapping_sub(current).cast_signed() <= 0 => current,
        _ => candidate,
    }
}

/// The numbers of the TCP segment `packet` carries, when it carries one with
/// its header.
pub(crate) fn segment(packet: &[u8]) -> Option<Segment> {
    let layout = ip::locate(packet).ok()?;
    if layout.protocol != ip::PROTO_TCP || !layout.has_transport() {
        return None;
    }
    let tcp = packet.get(layout.l4_offset..layout.end)?;
    let header = tcp.get(..TCP_HEADER)?;
    let data_offset = usize::from(header[12] >> 4) * 4;
    if data_offset < TCP_HEADER {
        return None;
    }
    let payload = tcp.len().checked_sub(data_offset)?;
    let flags = header[13];
    let control = u32::from(flags & FLAG_SYN != 0) + u32::from(flags & FLAG_FIN != 0);
    Some(Segment {
        seq: u32::from_be_bytes([header[4], header[5], header[6], header[7]]),
        ack: u32::from_be_bytes([header[8], header[9], header[10], header[11]]),
        flags,
        len: u32::try_from(payload).ok()?.wrapping_add(control),
    })
}

/// The reset, from the remote end of `flow`, that ends the connection where
/// the app's segments left it. `None` while the app has sent nothing on it.
pub(crate) fn toward_app(flow: &FlowKey, side: AppSide) -> Option<Vec<u8>> {
    if flow.transport != Transport::Tcp {
        return None;
    }
    match (side.expects, side.next) {
        (Some(expects), next) => Some(build(flow, expects, next)),
        // Only a SYN so far: a reset is accepted there when it acknowledges
        // the SYN.
        (None, Some(next)) => Some(build(flow, 0, Some(next))),
        (None, None) => None,
    }
}

/// What a closed connection answers `segment` of `flow` with (RFC 9293
/// section 3.10.7.1): nothing for a reset, otherwise a reset its sender
/// accepts.
pub(crate) fn answer(flow: &FlowKey, segment: &Segment) -> Option<Vec<u8>> {
    if flow.transport != Transport::Tcp || segment.flags & FLAG_RST != 0 {
        return None;
    }
    Some(if segment.flags & FLAG_ACK != 0 {
        build(flow, segment.ack, None)
    } else {
        build(flow, 0, Some(segment.seq.wrapping_add(segment.len)))
    })
}

/// An ICMP port unreachable from the remote end of the UDP `flow` to its
/// local end, quoting a datagram of the flow so the local stack finds the
/// socket.
pub(crate) fn unreachable_toward_app(flow: &FlowKey) -> Option<Vec<u8>> {
    if flow.transport != Transport::Udp {
        return None;
    }
    let mut udp = [0u8; 8];
    udp[0..2].copy_from_slice(&flow.local.port().to_be_bytes());
    udp[2..4].copy_from_slice(&flow.remote.port().to_be_bytes());
    udp[4..6].copy_from_slice(&8u16.to_be_bytes());
    let (local, remote) = (flow.local.ip(), flow.remote.ip());
    Some(match (local, remote) {
        (IpAddr::V4(local), IpAddr::V4(remote)) => {
            let quoted = [
                ipv4_header(local.octets(), remote.octets(), ip::PROTO_UDP, udp.len()).as_slice(),
                &udp,
            ]
            .concat();
            let mut icmp = vec![3, 3, 0, 0, 0, 0, 0, 0];
            icmp.extend_from_slice(&quoted);
            let sum = checksum(&[&icmp]);
            icmp[2..4].copy_from_slice(&sum.to_be_bytes());
            let mut packet =
                ipv4_header(remote.octets(), local.octets(), ip::PROTO_ICMP, icmp.len()).to_vec();
            packet.extend_from_slice(&icmp);
            packet
        }
        _ => {
            let (local, remote) = (v6(local), v6(remote));
            let quoted = [
                ipv6_header(local, remote, ip::PROTO_UDP, udp.len()).as_slice(),
                &udp,
            ]
            .concat();
            let mut icmp = vec![1, 4, 0, 0, 0, 0, 0, 0];
            icmp.extend_from_slice(&quoted);
            let sum = checksum(&[
                &remote,
                &local,
                &(icmp.len() as u32).to_be_bytes(),
                &[0, 0, 0, ip::PROTO_ICMPV6],
                &icmp,
            ]);
            icmp[2..4].copy_from_slice(&sum.to_be_bytes());
            let mut packet = ipv6_header(remote, local, ip::PROTO_ICMPV6, icmp.len()).to_vec();
            packet.extend_from_slice(&icmp);
            packet
        }
    })
}

fn ipv4_header(src: [u8; 4], dst: [u8; 4], protocol: u8, payload: usize) -> [u8; 20] {
    let mut header = [0u8; 20];
    header[0] = 0x45;
    header[2..4].copy_from_slice(&((20 + payload) as u16).to_be_bytes());
    header[6] = 0x40;
    header[8] = 64;
    header[9] = protocol;
    header[12..16].copy_from_slice(&src);
    header[16..20].copy_from_slice(&dst);
    let sum = checksum(&[&header]);
    header[10..12].copy_from_slice(&sum.to_be_bytes());
    header
}

fn ipv6_header(src: [u8; 16], dst: [u8; 16], next_header: u8, payload: usize) -> [u8; 40] {
    let mut header = [0u8; 40];
    header[0] = 0x60;
    header[4..6].copy_from_slice(&(payload as u16).to_be_bytes());
    header[6] = next_header;
    header[7] = 64;
    header[8..24].copy_from_slice(&src);
    header[24..40].copy_from_slice(&dst);
    header
}

/// A bare reset from the remote end of `flow` to its local end, acknowledging
/// `ack` when there is one.
fn build(flow: &FlowKey, seq: u32, ack: Option<u32>) -> Vec<u8> {
    let mut tcp = [0u8; TCP_HEADER];
    tcp[0..2].copy_from_slice(&flow.remote.port().to_be_bytes());
    tcp[2..4].copy_from_slice(&flow.local.port().to_be_bytes());
    tcp[4..8].copy_from_slice(&seq.to_be_bytes());
    tcp[8..12].copy_from_slice(&ack.unwrap_or(0).to_be_bytes());
    tcp[12] = 0x50;
    tcp[13] = FLAG_RST | if ack.is_some() { FLAG_ACK } else { 0 };
    let (src, dst) = (flow.remote.ip(), flow.local.ip());
    let mut packet = match (src, dst) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            let sum = checksum(&[
                &src.octets(),
                &dst.octets(),
                &[0, ip::PROTO_TCP],
                &(TCP_HEADER as u16).to_be_bytes(),
                &tcp,
            ]);
            tcp[16..18].copy_from_slice(&sum.to_be_bytes());
            ipv4_header(src.octets(), dst.octets(), ip::PROTO_TCP, TCP_HEADER).to_vec()
        }
        _ => {
            let (src, dst) = (v6(src), v6(dst));
            let sum = checksum(&[
                &src,
                &dst,
                &(TCP_HEADER as u32).to_be_bytes(),
                &[0, 0, 0, ip::PROTO_TCP],
                &tcp,
            ]);
            tcp[16..18].copy_from_slice(&sum.to_be_bytes());
            ipv6_header(src, dst, ip::PROTO_TCP, TCP_HEADER).to_vec()
        }
    };
    packet.extend_from_slice(&tcp);
    packet
}

fn v6(addr: IpAddr) -> [u8; 16] {
    match addr {
        IpAddr::V4(addr) => addr.to_ipv6_mapped().octets(),
        IpAddr::V6(addr) => addr.octets(),
    }
}

/// The Internet checksum of the concatenation of `parts`, each of an even
/// length.
fn checksum(parts: &[&[u8]]) -> u16 {
    let mut sum: u32 = 0;
    for part in parts {
        for word in part.chunks_exact(2) {
            sum += u32::from(u16::from_be_bytes([word[0], word[1]]));
        }
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;
    use crate::testutil::*;

    const LOCAL: [u8; 4] = [10, 64, 0, 2];
    const REMOTE: [u8; 4] = [198, 51, 100, 9];
    const LOCAL6: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
    const REMOTE6: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];

    fn flow() -> FlowKey {
        FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new(IpAddr::from(LOCAL), 50000),
            remote: SocketAddr::new(IpAddr::from(REMOTE), 443),
        }
    }

    fn sent(flags: u8, seq: u32, ack: u32, payload: &[u8]) -> Segment {
        segment(&tcp_v4_numbered(
            LOCAL, REMOTE, 50000, 443, flags, seq, ack, payload,
        ))
        .unwrap()
    }

    fn side(segments: &[Segment]) -> AppSide {
        let mut side = AppSide::default();
        for segment in segments {
            side.follow(segment);
        }
        side
    }

    #[test]
    fn reads_the_numbers_of_a_segment_counting_its_syn_and_fin() {
        let data = sent(TCP_ACK, 100, 7, b"hello");
        let syn = sent(TCP_SYN, 100, 0, b"");
        let fin = sent(TCP_ACK | TCP_FIN, 100, 7, b"hi");

        assert_eq!((data.seq, data.ack, data.len), (100, 7, 5));
        assert_eq!(syn.len, 1);
        assert_eq!(fin.len, 3);
    }

    #[test]
    fn a_packet_without_a_tcp_header_has_no_segment() {
        let udp = udp_v4(LOCAL, REMOTE, 50000, 443, b"quic");
        let later_fragment = as_v4_fragment(
            tcp_v4(LOCAL, REMOTE, 50000, 443, TCP_ACK, &[1; 64]),
            9,
            4,
            false,
        );

        assert_eq!(segment(&udp), None);
        assert_eq!(segment(&later_fragment), None);
    }

    #[test]
    fn the_reset_toward_the_app_carries_the_sequence_number_it_expects() {
        let side = side(&[
            sent(TCP_ACK, 1000, 5000, b"GET /"),
            sent(TCP_ACK, 1005, 7000, b""),
        ]);

        let reset = toward_app(&flow(), side).unwrap();

        let classified = crate::flow::classify(&reset, crate::flow::Direction::Downlink).unwrap();
        assert_eq!(
            classified,
            crate::flow::Classified::Flow {
                key: flow(),
                tcp_flags: TCP_RST | TCP_ACK,
                fragment: None,
            }
        );
        let numbers = segment(&reset).unwrap();
        assert_eq!((numbers.seq, numbers.ack, numbers.len), (7000, 1005, 0));
        assert!(ipv4_header_ok(&reset));
        assert!(transport_ok(&reset));
    }

    #[test]
    fn a_retransmission_does_not_move_what_the_app_is_known_to_expect_back() {
        let side = side(&[
            sent(TCP_ACK, 1005, 7000, b""),
            sent(TCP_ACK, 1000, 5000, b"GET /"),
        ]);

        let reset = segment(&toward_app(&flow(), side).unwrap()).unwrap();

        assert_eq!((reset.seq, reset.ack), (7000, 1005));
    }

    #[test]
    fn the_numbers_wrap_around_the_sequence_space() {
        let side = side(&[
            sent(TCP_ACK, u32::MAX - 1, u32::MAX, b"abcd"),
            sent(TCP_ACK, 2, 3, b""),
        ]);

        let reset = segment(&toward_app(&flow(), side).unwrap()).unwrap();

        assert_eq!((reset.seq, reset.ack), (3, 2));
    }

    #[test]
    fn a_connection_still_opening_is_reset_by_acknowledging_its_syn() {
        let side = side(&[sent(TCP_SYN, 4242, 0, b"")]);

        let reset = segment(&toward_app(&flow(), side).unwrap()).unwrap();

        assert_eq!(reset.flags, TCP_RST | TCP_ACK);
        assert_eq!((reset.seq, reset.ack), (0, 4243));
    }

    #[test]
    fn nothing_is_sent_toward_an_app_that_sent_nothing() {
        assert_eq!(toward_app(&flow(), AppSide::default()), None);
    }

    #[test]
    fn a_segment_is_answered_with_a_reset_at_the_number_it_acknowledges() {
        let answer = answer(&flow(), &sent(TCP_ACK, 1000, 9000, b"retry")).unwrap();

        let reset = segment(&answer).unwrap();
        assert_eq!(reset.flags, TCP_RST);
        assert_eq!(reset.seq, 9000);
        assert!(transport_ok(&answer));
    }

    #[test]
    fn a_segment_without_acknowledgment_is_answered_by_acknowledging_it() {
        let answer = answer(&flow(), &sent(TCP_SYN, 77, 0, b"")).unwrap();

        let reset = segment(&answer).unwrap();
        assert_eq!(reset.flags, TCP_RST | TCP_ACK);
        assert_eq!((reset.seq, reset.ack), (0, 78));
    }

    #[test]
    fn a_reset_is_never_answered() {
        assert_eq!(answer(&flow(), &sent(TCP_RST | TCP_ACK, 1, 2, b"")), None);
    }

    #[test]
    fn a_segment_claiming_a_header_shorter_than_twenty_bytes_has_no_numbers() {
        let mut packet = tcp_v4(LOCAL, REMOTE, 50000, 443, TCP_ACK, b"");
        packet[20 + 12] = 0x40;

        assert_eq!(segment(&packet), None);
    }

    #[test]
    fn an_ipv6_datagram_flow_is_told_its_port_is_unreachable_in_icmpv6() {
        let flow = FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new(IpAddr::from(LOCAL6), 50000),
            remote: SocketAddr::new(IpAddr::from(REMOTE6), 443),
        };

        let error = unreachable_toward_app(&flow).unwrap();

        assert_eq!((error[6], error[40], error[41]), (58, 1, 4));
        assert!(transport_ok(&error));
        assert_eq!(
            crate::flow::classify(&error, crate::flow::Direction::Downlink).unwrap(),
            crate::flow::Classified::IcmpError { key: flow }
        );
    }

    #[test]
    fn only_a_datagram_flow_is_told_its_port_is_unreachable() {
        assert_eq!(unreachable_toward_app(&flow()), None);
    }

    #[test]
    fn an_ipv6_connection_gets_an_ipv6_reset() {
        let flow = FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new(IpAddr::from(LOCAL6), 50000),
            remote: SocketAddr::new(IpAddr::from(REMOTE6), 443),
        };
        let sent = segment(&tcp_v6_numbered(
            LOCAL6, REMOTE6, 50000, 443, TCP_ACK, 10, 20, b"",
        ))
        .unwrap();

        let reset = answer(&flow, &sent).unwrap();

        assert_eq!(&reset[8..24], &REMOTE6);
        assert_eq!(&reset[24..40], &LOCAL6);
        assert_eq!(segment(&reset).unwrap().seq, 20);
        assert!(transport_ok(&reset));
    }
}
