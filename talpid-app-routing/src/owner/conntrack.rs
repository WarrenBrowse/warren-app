//! The conntrack messages the Linux resolver exchanges to see through an
//! address translation: a flow the firewall translated on its way into the
//! tunnel is known to the TUN device by its translated pair, while its
//! socket holds the original one. Include-only masquerades every included
//! connection that way, since an included socket picks its source address
//! from the main table before the include mark sends it to the tunnel.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::flow::{FlowKey, Transport};

const NLMSG_HDR_LEN: usize = 16;
const NFGENMSG_LEN: usize = 4;
const NLMSG_ERROR: u16 = 2;
const NLM_F_REQUEST: u16 = 1;
const NFNL_SUBSYS_CTNETLINK: u16 = 1;
const IPCTNL_MSG_CT_NEW: u16 = 0;
const IPCTNL_MSG_CT_GET: u16 = 1;
const NLA_F_NESTED: u16 = 0x8000;
const NLA_TYPE_MASK: u16 = 0x3fff;

const CTA_TUPLE_ORIG: u16 = 1;
const CTA_TUPLE_REPLY: u16 = 2;
const CTA_TUPLE_IP: u16 = 1;
const CTA_TUPLE_PROTO: u16 = 2;
const CTA_IP_V4_SRC: u16 = 1;
const CTA_IP_V4_DST: u16 = 2;
const CTA_IP_V6_SRC: u16 = 3;
const CTA_IP_V6_DST: u16 = 4;
const CTA_PROTO_NUM: u16 = 1;
const CTA_PROTO_SRC_PORT: u16 = 2;
const CTA_PROTO_DST_PORT: u16 = 3;

const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;
const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;
const ENOENT: i32 = 2;

/// A request for the connection whose reply direction is `flow` seen from
/// the far end, which is how the TUN device shows a translated connection.
/// `None` for a transport conntrack is not asked about.
pub fn request(sequence: u32, flow: &FlowKey) -> Option<Vec<u8>> {
    let protocol = match flow.transport {
        Transport::Tcp => IPPROTO_TCP,
        Transport::Udp => IPPROTO_UDP,
        Transport::IcmpEcho => return None,
    };
    let (family, src, dst) = match (flow.remote.ip(), flow.local.ip()) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => (
            AF_INET,
            attribute(CTA_IP_V4_SRC, &src.octets()),
            attribute(CTA_IP_V4_DST, &dst.octets()),
        ),
        (IpAddr::V6(src), IpAddr::V6(dst)) => (
            AF_INET6,
            attribute(CTA_IP_V6_SRC, &src.octets()),
            attribute(CTA_IP_V6_DST, &dst.octets()),
        ),
        _ => return None,
    };
    let ip = attribute(CTA_TUPLE_IP | NLA_F_NESTED, &[src, dst].concat());
    let proto = attribute(
        CTA_TUPLE_PROTO | NLA_F_NESTED,
        &[
            attribute(CTA_PROTO_NUM, &[protocol]),
            attribute(CTA_PROTO_SRC_PORT, &flow.remote.port().to_be_bytes()),
            attribute(CTA_PROTO_DST_PORT, &flow.local.port().to_be_bytes()),
        ]
        .concat(),
    );
    let tuple = attribute(CTA_TUPLE_REPLY | NLA_F_NESTED, &[ip, proto].concat());

    let length = NLMSG_HDR_LEN + NFGENMSG_LEN + tuple.len();
    let mut message = Vec::with_capacity(length);
    message.extend_from_slice(&(length as u32).to_ne_bytes());
    message.extend_from_slice(&((NFNL_SUBSYS_CTNETLINK << 8) | IPCTNL_MSG_CT_GET).to_ne_bytes());
    message.extend_from_slice(&NLM_F_REQUEST.to_ne_bytes());
    message.extend_from_slice(&sequence.to_ne_bytes());
    message.extend_from_slice(&0u32.to_ne_bytes());
    // nfgenmsg: the family, NFNETLINK_V0, and a zero resource id.
    message.extend_from_slice(&[family, 0, 0, 0]);
    message.extend_from_slice(&tuple);
    Some(message)
}

/// What the kernel answered to a [`request`].
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    /// The connection, as its socket sees it.
    Original(FlowKey),
    /// No such connection.
    Absent,
    /// The kernel refuses to answer (no privilege, no conntrack): asking
    /// again will not help.
    Refused,
    /// A reply to an earlier request that timed out, or one this cannot read.
    Other,
}

/// Reads the answer to the [`request`] numbered `sequence` about a flow of
/// `transport`.
pub fn parse_reply(reply: &[u8], sequence: u32, transport: Transport) -> Reply {
    let (Some(length), Some(kind), Some(seq)) = (word(reply, 0), half(reply, 4), word(reply, 8))
    else {
        return Reply::Other;
    };
    if seq != sequence {
        return Reply::Other;
    }
    let Some(message) = reply.get(..length as usize) else {
        return Reply::Other;
    };
    if kind == NLMSG_ERROR {
        return match word(message, NLMSG_HDR_LEN).map(|code| -(code as i32)) {
            Some(ENOENT) => Reply::Absent,
            Some(_) => Reply::Refused,
            None => Reply::Other,
        };
    }
    if kind != (NFNL_SUBSYS_CTNETLINK << 8) | IPCTNL_MSG_CT_NEW {
        return Reply::Other;
    }
    let Some(attributes) = message.get(NLMSG_HDR_LEN + NFGENMSG_LEN..) else {
        return Reply::Other;
    };
    find(attributes, CTA_TUPLE_ORIG)
        .and_then(|tuple| original(tuple, transport))
        .map_or(Reply::Other, Reply::Original)
}

/// The flow a `CTA_TUPLE_ORIG` names: its source is the socket's end.
fn original(tuple: &[u8], transport: Transport) -> Option<FlowKey> {
    let ip = find(tuple, CTA_TUPLE_IP)?;
    let proto = find(tuple, CTA_TUPLE_PROTO)?;
    let (src, dst) = match (find(ip, CTA_IP_V4_SRC), find(ip, CTA_IP_V4_DST)) {
        (Some(src), Some(dst)) => (
            IpAddr::from(Ipv4Addr::from(<[u8; 4]>::try_from(src).ok()?)),
            IpAddr::from(Ipv4Addr::from(<[u8; 4]>::try_from(dst).ok()?)),
        ),
        _ => (
            IpAddr::from(Ipv6Addr::from(
                <[u8; 16]>::try_from(find(ip, CTA_IP_V6_SRC)?).ok()?,
            )),
            IpAddr::from(Ipv6Addr::from(
                <[u8; 16]>::try_from(find(ip, CTA_IP_V6_DST)?).ok()?,
            )),
        ),
    };
    let port = |kind| {
        find(proto, kind)
            .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
            .map(u16::from_be_bytes)
    };
    Some(FlowKey {
        transport,
        local: SocketAddr::new(src, port(CTA_PROTO_SRC_PORT)?),
        remote: SocketAddr::new(dst, port(CTA_PROTO_DST_PORT)?),
    })
}

fn attribute(kind: u16, payload: &[u8]) -> Vec<u8> {
    let length = 4 + payload.len();
    let mut attribute = Vec::with_capacity(aligned(length));
    attribute.extend_from_slice(&(length as u16).to_ne_bytes());
    attribute.extend_from_slice(&kind.to_ne_bytes());
    attribute.extend_from_slice(payload);
    attribute.resize(aligned(length), 0);
    attribute
}

/// The payload of the first attribute of `kind` in `attributes`.
fn find(attributes: &[u8], kind: u16) -> Option<&[u8]> {
    let mut at = 0;
    while let (Some(length), Some(found)) = (half(attributes, at), half(attributes, at + 2)) {
        let length = usize::from(length);
        let payload = attributes.get(at + 4..at + length.max(4))?;
        if found & NLA_TYPE_MASK == kind {
            return Some(payload);
        }
        if length < 4 {
            return None;
        }
        at += aligned(length);
    }
    None
}

const fn aligned(length: usize) -> usize {
    (length + 3) & !3
}

fn half(bytes: &[u8], at: usize) -> Option<u16> {
    let bytes = bytes.get(at..at + 2)?;
    Some(u16::from_ne_bytes([bytes[0], bytes[1]]))
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let bytes = bytes.get(at..at + 4)?;
    Some(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masqueraded() -> FlowKey {
        FlowKey {
            transport: Transport::Tcp,
            local: "10.64.0.2:40001".parse().unwrap(),
            remote: "198.51.100.9:443".parse().unwrap(),
        }
    }

    /// A kernel answer about a connection from `original` masqueraded to
    /// `masqueraded`, laid out as `ctnetlink_fill_info` writes it.
    fn answer(sequence: u32, original: FlowKey, masqueraded: FlowKey) -> Vec<u8> {
        let tuple = |kind, src: SocketAddr, dst: SocketAddr| {
            let (IpAddr::V4(src_ip), IpAddr::V4(dst_ip)) = (src.ip(), dst.ip()) else {
                unreachable!("the fixtures are IPv4");
            };
            attribute(
                kind | NLA_F_NESTED,
                &[
                    attribute(
                        CTA_TUPLE_IP | NLA_F_NESTED,
                        &[
                            attribute(CTA_IP_V4_SRC, &src_ip.octets()),
                            attribute(CTA_IP_V4_DST, &dst_ip.octets()),
                        ]
                        .concat(),
                    ),
                    attribute(
                        CTA_TUPLE_PROTO | NLA_F_NESTED,
                        &[
                            attribute(CTA_PROTO_NUM, &[IPPROTO_TCP]),
                            attribute(CTA_PROTO_SRC_PORT, &src.port().to_be_bytes()),
                            attribute(CTA_PROTO_DST_PORT, &dst.port().to_be_bytes()),
                        ]
                        .concat(),
                    ),
                ]
                .concat(),
            )
        };
        let body = [
            tuple(CTA_TUPLE_ORIG, original.local, original.remote),
            tuple(CTA_TUPLE_REPLY, masqueraded.remote, masqueraded.local),
            // CTA_STATUS, which comes next, is skipped.
            attribute(3, &8u32.to_be_bytes()),
        ]
        .concat();
        let length = NLMSG_HDR_LEN + NFGENMSG_LEN + body.len();
        let mut message = Vec::new();
        message.extend_from_slice(&(length as u32).to_ne_bytes());
        message.extend_from_slice(&(NFNL_SUBSYS_CTNETLINK << 8).to_ne_bytes());
        message.extend_from_slice(&0u16.to_ne_bytes());
        message.extend_from_slice(&sequence.to_ne_bytes());
        message.extend_from_slice(&0u32.to_ne_bytes());
        message.extend_from_slice(&[AF_INET, 0, 0, 0]);
        message.extend_from_slice(&body);
        message
    }

    fn error(sequence: u32, code: i32) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(&36u32.to_ne_bytes());
        message.extend_from_slice(&NLMSG_ERROR.to_ne_bytes());
        message.extend_from_slice(&0u16.to_ne_bytes());
        message.extend_from_slice(&sequence.to_ne_bytes());
        message.extend_from_slice(&0u32.to_ne_bytes());
        message.extend_from_slice(&(-code).to_ne_bytes());
        message.resize(36, 0);
        message
    }

    #[test]
    fn asks_for_the_connection_by_its_reply_direction() {
        let request = request(7, &masqueraded()).unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(&72u32.to_ne_bytes());
        expected.extend_from_slice(&0x0101u16.to_ne_bytes());
        expected.extend_from_slice(&1u16.to_ne_bytes());
        expected.extend_from_slice(&7u32.to_ne_bytes());
        expected.extend_from_slice(&0u32.to_ne_bytes());
        expected.extend_from_slice(&[2, 0, 0, 0]);
        // CTA_TUPLE_REPLY, nested.
        expected.extend_from_slice(&52u16.to_ne_bytes());
        expected.extend_from_slice(&0x8002u16.to_ne_bytes());
        // CTA_TUPLE_IP: the far end is the source, the tunnel address the
        // destination.
        expected.extend_from_slice(&20u16.to_ne_bytes());
        expected.extend_from_slice(&0x8001u16.to_ne_bytes());
        expected.extend_from_slice(&8u16.to_ne_bytes());
        expected.extend_from_slice(&1u16.to_ne_bytes());
        expected.extend_from_slice(&[198, 51, 100, 9]);
        expected.extend_from_slice(&8u16.to_ne_bytes());
        expected.extend_from_slice(&2u16.to_ne_bytes());
        expected.extend_from_slice(&[10, 64, 0, 2]);
        // CTA_TUPLE_PROTO: TCP, from port 443 to port 40001, each padded.
        expected.extend_from_slice(&28u16.to_ne_bytes());
        expected.extend_from_slice(&0x8002u16.to_ne_bytes());
        expected.extend_from_slice(&5u16.to_ne_bytes());
        expected.extend_from_slice(&1u16.to_ne_bytes());
        expected.extend_from_slice(&[6, 0, 0, 0]);
        expected.extend_from_slice(&6u16.to_ne_bytes());
        expected.extend_from_slice(&2u16.to_ne_bytes());
        expected.extend_from_slice(&[0x01, 0xbb, 0, 0]);
        expected.extend_from_slice(&6u16.to_ne_bytes());
        expected.extend_from_slice(&3u16.to_ne_bytes());
        expected.extend_from_slice(&[0x9c, 0x41, 0, 0]);
        assert_eq!(request, expected);
    }

    #[test]
    fn asks_nothing_about_an_echo() {
        let echo = FlowKey {
            transport: Transport::IcmpEcho,
            ..masqueraded()
        };

        assert_eq!(request(1, &echo), None);
    }

    #[test]
    fn reads_the_pair_the_socket_holds_from_the_original_direction() {
        let original = FlowKey {
            local: "192.168.1.20:40000".parse().unwrap(),
            ..masqueraded()
        };

        let reply = parse_reply(&answer(9, original, masqueraded()), 9, Transport::Tcp);

        assert_eq!(reply, Reply::Original(original));
    }

    #[test]
    fn a_missing_connection_is_absent_and_any_other_error_a_refusal() {
        assert_eq!(
            parse_reply(&error(3, ENOENT), 3, Transport::Tcp),
            Reply::Absent
        );
        assert_eq!(parse_reply(&error(3, 1), 3, Transport::Tcp), Reply::Refused);
    }

    #[test]
    fn skips_a_reply_to_an_earlier_request() {
        let reply = answer(4, masqueraded(), masqueraded());

        assert_eq!(parse_reply(&reply, 5, Transport::Tcp), Reply::Other);
    }
}
