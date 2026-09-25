//! Flow identities parsed from packets, and the bounded table of live flows.

use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

use crate::ip::{self, Family, Layout, PacketError};

/// How long an open TCP flow may stay silent before its entry is dropped.
pub const TCP_IDLE: Duration = Duration::from_secs(2 * 60 * 60);
/// How long a UDP flow may stay silent before its entry is dropped.
pub const UDP_IDLE: Duration = Duration::from_secs(180);
/// How long an ICMP echo flow may stay silent before its entry is dropped.
pub const ICMP_IDLE: Duration = Duration::from_secs(30);
/// How long a TCP flow is kept after a reset or a FIN in both directions, so
/// the last acknowledgements still find it.
pub const CLOSING_LINGER: Duration = Duration::from_secs(10);

const FLAG_FIN: u8 = 0x01;
const FLAG_RST: u8 = 0x04;

/// The transport of a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    Tcp,
    Udp,
    /// ICMP or ICMPv6 echo, identified by its echo identifier.
    IcmpEcho,
}

/// A flow as the local host sees it.
///
/// For [`Transport::IcmpEcho`] the local port is the echo identifier and the
/// remote port is zero.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub transport: Transport,
    pub local: SocketAddr,
    pub remote: SocketAddr,
}

// Addresses stay out of any rendering of a flow, so a stray `{:?}` cannot put
// one in a log.
impl fmt::Debug for FlowKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowKey")
            .field("transport", &self.transport)
            .field("local_port", &self.local.port())
            .field("remote_port", &self.remote.port())
            .finish_non_exhaustive()
    }
}

/// Which way a packet crosses the TUN device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// From a local app towards the network.
    Uplink,
    /// From the network towards a local app.
    Downlink,
}

/// Identifies the fragments of one datagram.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentKey {
    pub src: IpAddr,
    pub dst: IpAddr,
    pub protocol: u8,
    pub id: u32,
}

impl fmt::Debug for FragmentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FragmentKey")
            .field("protocol", &self.protocol)
            .finish_non_exhaustive()
    }
}

/// What a packet is, for routing purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classified {
    /// A packet of a flow. `fragment` is set when it is the first fragment of
    /// a datagram, whose later fragments follow the same route.
    Flow {
        key: FlowKey,
        tcp_flags: u8,
        fragment: Option<FragmentKey>,
    },
    /// A fragment after the first: its flow is known only through the first.
    LaterFragment(FragmentKey),
    /// An ICMP error about a packet of the flow `key`.
    IcmpError { key: FlowKey },
    /// Any other protocol or ICMP message.
    Other,
}

/// Classifies a packet crossing the TUN device in `direction`.
///
/// # Errors
///
/// A [`PacketError`] when the IP or transport headers are malformed.
pub fn classify(packet: &[u8], direction: Direction) -> Result<Classified, PacketError> {
    let layout = ip::locate(packet)?;
    let src = layout.src(packet);
    let dst = layout.dst(packet);
    let Some(fragment) = layout.fragment else {
        return classify_transport(packet, &layout, src, dst, direction, None);
    };
    let fragment_key = FragmentKey {
        src,
        dst,
        protocol: fragment.protocol,
        id: fragment.id,
    };
    if fragment.first {
        classify_transport(packet, &layout, src, dst, direction, Some(fragment_key))
    } else {
        Ok(Classified::LaterFragment(fragment_key))
    }
}

fn classify_transport(
    packet: &[u8],
    layout: &Layout,
    src: IpAddr,
    dst: IpAddr,
    direction: Direction,
    fragment: Option<FragmentKey>,
) -> Result<Classified, PacketError> {
    let l4 = &packet[layout.l4_offset..layout.end];
    let flow = |transport, src_port, dst_port, tcp_flags| {
        let (local, remote) = oriented(
            SocketAddr::new(src, src_port),
            SocketAddr::new(dst, dst_port),
            direction,
        );
        Classified::Flow {
            key: FlowKey {
                transport,
                local,
                remote,
            },
            tcp_flags,
            fragment,
        }
    };
    match layout.protocol {
        ip::PROTO_TCP => {
            let header = l4.get(..20).ok_or(PacketError::Truncated)?;
            let (src_port, dst_port) = ports(header);
            Ok(flow(Transport::Tcp, src_port, dst_port, header[13]))
        }
        ip::PROTO_UDP => {
            let header = l4.get(..8).ok_or(PacketError::Truncated)?;
            let (src_port, dst_port) = ports(header);
            Ok(flow(Transport::Udp, src_port, dst_port, 0))
        }
        ip::PROTO_ICMP | ip::PROTO_ICMPV6 => {
            let header = l4.get(..8).ok_or(PacketError::Truncated)?;
            match icmp_kind(layout.family, header[0]) {
                IcmpKind::Echo => {
                    let id = u16::from_be_bytes([header[4], header[5]]);
                    Ok(echo_flow(src, dst, id, direction, fragment))
                }
                IcmpKind::Error => quoted_flow(&l4[8..], direction),
                IcmpKind::Other => Ok(Classified::Other),
            }
        }
        _ => Ok(Classified::Other),
    }
}

fn echo_flow(
    src: IpAddr,
    dst: IpAddr,
    id: u16,
    direction: Direction,
    fragment: Option<FragmentKey>,
) -> Classified {
    let (local, remote) = match direction {
        Direction::Uplink => (src, dst),
        Direction::Downlink => (dst, src),
    };
    Classified::Flow {
        key: FlowKey {
            transport: Transport::IcmpEcho,
            local: SocketAddr::new(local, id),
            remote: SocketAddr::new(remote, 0),
        },
        tcp_flags: 0,
        fragment,
    }
}

/// The flow of the packet an ICMP error quotes. The quoted packet travelled
/// the other way: an error arriving from the network quotes a packet this
/// host sent.
fn quoted_flow(quoted: &[u8], direction: Direction) -> Result<Classified, PacketError> {
    let layout = ip::locate_quoted(quoted)?;
    if !layout.has_transport() {
        return Ok(Classified::Other);
    }
    let quoted_direction = match direction {
        Direction::Uplink => Direction::Downlink,
        Direction::Downlink => Direction::Uplink,
    };
    let src = layout.src(quoted);
    let dst = layout.dst(quoted);
    let l4 = &quoted[layout.l4_offset..layout.end];
    let transport = match layout.protocol {
        ip::PROTO_TCP => Transport::Tcp,
        ip::PROTO_UDP => Transport::Udp,
        ip::PROTO_ICMP | ip::PROTO_ICMPV6 => {
            let header = l4.get(..6).ok_or(PacketError::Truncated)?;
            if icmp_kind(layout.family, header[0]) != IcmpKind::Echo {
                return Ok(Classified::Other);
            }
            let id = u16::from_be_bytes([header[4], header[5]]);
            let Classified::Flow { key, .. } = echo_flow(src, dst, id, quoted_direction, None)
            else {
                unreachable!("an echo is always a flow");
            };
            return Ok(Classified::IcmpError { key });
        }
        _ => return Ok(Classified::Other),
    };
    let header = l4.get(..4).ok_or(PacketError::Truncated)?;
    let (src_port, dst_port) = ports(header);
    let (local, remote) = oriented(
        SocketAddr::new(src, src_port),
        SocketAddr::new(dst, dst_port),
        quoted_direction,
    );
    Ok(Classified::IcmpError {
        key: FlowKey {
            transport,
            local,
            remote,
        },
    })
}

fn oriented(src: SocketAddr, dst: SocketAddr, direction: Direction) -> (SocketAddr, SocketAddr) {
    match direction {
        Direction::Uplink => (src, dst),
        Direction::Downlink => (dst, src),
    }
}

fn ports(header: &[u8]) -> (u16, u16) {
    (
        u16::from_be_bytes([header[0], header[1]]),
        u16::from_be_bytes([header[2], header[3]]),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IcmpKind {
    Echo,
    Error,
    Other,
}

fn icmp_kind(family: Family, message_type: u8) -> IcmpKind {
    match (family, message_type) {
        (Family::V4, 0 | 8) | (Family::V6, 128 | 129) => IcmpKind::Echo,
        (Family::V4, 3 | 11 | 12) | (Family::V6, 1..=4) => IcmpKind::Error,
        _ => IcmpKind::Other,
    }
}

fn is_expired<V>(transport: Transport, entry: &Entry<V>, now: Instant) -> bool {
    let timeout = match (entry.lifecycle, transport) {
        (Lifecycle::Closing, _) => CLOSING_LINGER,
        (_, Transport::Tcp) => TCP_IDLE,
        (_, Transport::Udp) => UDP_IDLE,
        (_, Transport::IcmpEcho) => ICMP_IDLE,
    };
    now.saturating_duration_since(entry.last_seen) > timeout
}

fn advance(lifecycle: Lifecycle, direction: Direction, tcp_flags: u8) -> Lifecycle {
    if tcp_flags & FLAG_RST != 0 {
        return Lifecycle::Closing;
    }
    if tcp_flags & FLAG_FIN == 0 {
        return lifecycle;
    }
    match lifecycle {
        Lifecycle::Open => Lifecycle::HalfClosed(direction),
        Lifecycle::HalfClosed(from) if from != direction => Lifecycle::Closing,
        other => other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Open,
    HalfClosed(Direction),
    Closing,
}

#[derive(Debug, Clone, Copy)]
struct Entry<V> {
    value: V,
    last_seen: Instant,
    lifecycle: Lifecycle,
}

/// The live flows and what was decided for each, bounded in size.
///
/// The map is allocated once at its full capacity, so neither a lookup nor an
/// insert allocates on the packet path.
pub struct FlowTable<V> {
    entries: HashMap<FlowKey, Entry<V>>,
    capacity: usize,
    eviction: Vec<Instant>,
}

impl<V: Copy> FlowTable<V> {
    /// A table that holds at most `capacity` flows (at least one).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
            eviction: Vec::with_capacity(capacity),
        }
    }

    /// The value recorded for `key`, if the flow is live. Counts the packet as
    /// activity and follows the TCP lifecycle through `tcp_flags`.
    pub fn lookup(
        &mut self,
        key: &FlowKey,
        direction: Direction,
        tcp_flags: u8,
        now: Instant,
    ) -> Option<V> {
        let entry = self.entries.get_mut(key)?;
        if is_expired(key.transport, entry, now) {
            self.entries.remove(key);
            return None;
        }
        entry.last_seen = now;
        entry.lifecycle = advance(entry.lifecycle, direction, tcp_flags);
        Some(entry.value)
    }

    /// Records a new flow, evicting expired flows first and then the least
    /// recently active ones when the table is full.
    pub fn insert(
        &mut self,
        key: FlowKey,
        value: V,
        direction: Direction,
        tcp_flags: u8,
        now: Instant,
    ) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.expire(now);
            if self.entries.len() >= self.capacity {
                self.evict_least_recent();
            }
        }
        let lifecycle = if key.transport == Transport::Tcp {
            advance(Lifecycle::Open, direction, tcp_flags)
        } else {
            Lifecycle::Open
        };
        self.entries.insert(
            key,
            Entry {
                value,
                last_seen: now,
                lifecycle,
            },
        );
    }

    /// Drops every flow that has been silent past its timeout.
    pub fn expire(&mut self, now: Instant) {
        self.entries
            .retain(|key, entry| !is_expired(key.transport, entry, now));
    }

    /// Frees a sixteenth of the table, oldest activity first, so a table
    /// under pressure pays for one scan per many inserts. Flows seen at the
    /// same instant are counted one by one, so a batch sharing one timestamp
    /// does not empty the table.
    fn evict_least_recent(&mut self) {
        let count = (self.entries.len() / 16).max(1);
        self.eviction.clear();
        self.eviction
            .extend(self.entries.values().map(|entry| entry.last_seen));
        let (_, cutoff, _) = self.eviction.select_nth_unstable(count - 1);
        let cutoff = *cutoff;
        let older = self.eviction.iter().filter(|seen| **seen < cutoff).count();
        let mut ties = count - older;
        self.entries.retain(|_, entry| {
            if entry.last_seen < cutoff {
                return false;
            }
            if entry.last_seen == cutoff && ties > 0 {
                ties -= 1;
                return false;
            }
            true
        });
    }

    /// The value recorded for `key` if the flow is live, without counting
    /// anything as activity: for a packet not yet known to belong to it.
    pub fn peek(&self, key: &FlowKey, now: Instant) -> Option<V> {
        self.entries
            .get(key)
            .filter(|entry| !is_expired(key.transport, entry, now))
            .map(|entry| entry.value)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    const LOCAL: [u8; 4] = [10, 64, 0, 2];
    const REMOTE: [u8; 4] = [198, 51, 100, 9];
    const LOCAL6: [u8; 16] = [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
    const REMOTE6: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];

    fn key(transport: Transport, local_port: u16, remote_port: u16) -> FlowKey {
        FlowKey {
            transport,
            local: SocketAddr::new(IpAddr::from(LOCAL), local_port),
            remote: SocketAddr::new(IpAddr::from(REMOTE), remote_port),
        }
    }

    fn flow_key(classified: Classified) -> FlowKey {
        match classified {
            Classified::Flow { key, .. } => key,
            other => panic!("not a flow: {other:?}"),
        }
    }

    #[test]
    fn an_uplink_packet_has_its_source_as_the_local_side() {
        let packet = tcp_v4(LOCAL, REMOTE, 50000, 443, TCP_SYN, b"");

        let classified = classify(&packet, Direction::Uplink).unwrap();

        assert_eq!(
            classified,
            Classified::Flow {
                key: key(Transport::Tcp, 50000, 443),
                tcp_flags: TCP_SYN,
                fragment: None,
            }
        );
    }

    #[test]
    fn a_downlink_packet_has_its_destination_as_the_local_side() {
        let packet = udp_v4(REMOTE, LOCAL, 53, 50001, b"answer");

        let classified = classify(&packet, Direction::Downlink).unwrap();

        assert_eq!(flow_key(classified), key(Transport::Udp, 50001, 53));
    }

    #[test]
    fn classifies_ipv6_flows() {
        let packet = udp_v6(LOCAL6, REMOTE6, 50002, 443, b"quic");

        let classified = classify(&packet, Direction::Uplink).unwrap();

        assert_eq!(
            flow_key(classified),
            FlowKey {
                transport: Transport::Udp,
                local: SocketAddr::new(IpAddr::from(LOCAL6), 50002),
                remote: SocketAddr::new(IpAddr::from(REMOTE6), 443),
            }
        );
    }

    #[test]
    fn an_echo_request_and_its_reply_share_one_flow_keyed_by_identifier() {
        let request = icmp_echo_v4(LOCAL, REMOTE, 0x77, true);
        let reply = icmp_echo_v4(REMOTE, LOCAL, 0x77, false);

        let up = flow_key(classify(&request, Direction::Uplink).unwrap());
        let down = flow_key(classify(&reply, Direction::Downlink).unwrap());

        assert_eq!(up, key(Transport::IcmpEcho, 0x77, 0));
        assert_eq!(down, up);
    }

    #[test]
    fn an_icmpv6_echo_is_keyed_by_identifier() {
        let request = icmp_echo_v6(LOCAL6, REMOTE6, 0x78, true);

        let up = flow_key(classify(&request, Direction::Uplink).unwrap());

        assert_eq!(up.transport, Transport::IcmpEcho);
        assert_eq!(up.local.port(), 0x78);
    }

    #[test]
    fn an_icmp_error_names_the_flow_of_the_quoted_packet() {
        let sent = udp_v4(LOCAL, REMOTE, 50003, 443, &[0u8; 64]);
        let error = icmp_v4_error([203, 0, 113, 1], LOCAL, &sent[..28]);

        let classified = classify(&error, Direction::Downlink).unwrap();

        assert_eq!(
            classified,
            Classified::IcmpError {
                key: key(Transport::Udp, 50003, 443)
            }
        );
    }

    #[test]
    fn an_icmpv6_error_names_the_flow_of_the_quoted_packet() {
        let sent = tcp_v6(LOCAL6, REMOTE6, 50004, 443, TCP_ACK, &[0u8; 64]);
        let error = icmp_v6_error(REMOTE6, LOCAL6, &sent[..60]);

        let classified = classify(&error, Direction::Downlink).unwrap();

        let Classified::IcmpError { key } = classified else {
            panic!("not an ICMP error: {classified:?}");
        };
        assert_eq!(key.transport, Transport::Tcp);
        assert_eq!(key.local, SocketAddr::new(IpAddr::from(LOCAL6), 50004));
    }

    #[test]
    fn an_uplink_icmp_error_quotes_a_received_packet() {
        let received = udp_v4(REMOTE, LOCAL, 443, 50005, b"x");
        let error = icmp_v4_error(LOCAL, REMOTE, &received[..28]);

        let classified = classify(&error, Direction::Uplink).unwrap();

        assert_eq!(
            classified,
            Classified::IcmpError {
                key: key(Transport::Udp, 50005, 443)
            }
        );
    }

    #[test]
    fn other_icmp_messages_and_protocols_are_other() {
        let mut timestamp = icmp_echo_v4(LOCAL, REMOTE, 1, true);
        timestamp[20] = 13;
        let mut gre = udp_v4(LOCAL, REMOTE, 1, 2, b"");
        gre[9] = 47;

        assert_eq!(
            classify(&timestamp, Direction::Uplink),
            Ok(Classified::Other)
        );
        assert_eq!(classify(&gre, Direction::Uplink), Ok(Classified::Other));
    }

    #[test]
    fn a_first_fragment_names_the_key_its_later_fragments_carry() {
        let first = as_v4_fragment(udp_v4(LOCAL, REMOTE, 50006, 443, &[1; 16]), 99, 0, true);
        let later = as_v4_fragment(udp_v4(LOCAL, REMOTE, 50006, 443, &[1; 16]), 99, 2, false);

        let Classified::Flow {
            fragment,
            key: first_key,
            ..
        } = classify(&first, Direction::Uplink).unwrap()
        else {
            panic!("the first fragment is a flow packet");
        };
        let later = classify(&later, Direction::Uplink).unwrap();

        assert_eq!(first_key, key(Transport::Udp, 50006, 443));
        assert_eq!(later, Classified::LaterFragment(fragment.unwrap()));
    }

    #[test]
    fn ipv6_fragments_share_a_key_when_an_extension_header_follows_the_fragment_header() {
        let mut first = as_v6_fragment(
            udp_v6(LOCAL6, REMOTE6, 50008, 443, &[1; 16]),
            5,
            0,
            true,
            false,
        );
        first[40] = 60;
        first.splice(48..48, [17u8, 0, 1, 4, 0, 0, 0, 0]);
        let payload_len = (first.len() - 40) as u16;
        first[4..6].copy_from_slice(&payload_len.to_be_bytes());
        let mut later = as_v6_fragment(
            udp_v6(LOCAL6, REMOTE6, 50008, 443, &[1; 16]),
            5,
            3,
            false,
            false,
        );
        later[40] = 60;

        let Classified::Flow { fragment, .. } = classify(&first, Direction::Uplink).unwrap() else {
            panic!("the first fragment is a flow packet");
        };
        let later = classify(&later, Direction::Uplink).unwrap();

        assert_eq!(later, Classified::LaterFragment(fragment.unwrap()));
    }

    #[test]
    fn rejects_a_truncated_transport_header() {
        // Long enough for the flags byte, short of a full TCP header.
        let mut packet = tcp_v4(LOCAL, REMOTE, 50007, 443, TCP_SYN, b"");
        packet.truncate(36);
        packet[2..4].copy_from_slice(&36u16.to_be_bytes());

        assert_eq!(
            classify(&packet, Direction::Uplink),
            Err(PacketError::Truncated)
        );
    }

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn returns_the_value_recorded_for_a_live_flow() {
        let mut table = FlowTable::new(8);
        let now = t0();
        table.insert(
            key(Transport::Tcp, 1, 443),
            'a',
            Direction::Uplink,
            TCP_SYN,
            now,
        );

        let found = table.lookup(
            &key(Transport::Tcp, 1, 443),
            Direction::Downlink,
            TCP_ACK,
            now,
        );
        let missing = table.lookup(
            &key(Transport::Tcp, 2, 443),
            Direction::Uplink,
            TCP_ACK,
            now,
        );

        assert_eq!(found, Some('a'));
        assert_eq!(missing, None);
    }

    #[test]
    fn a_silent_udp_flow_expires_after_its_idle_timeout() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Udp, 1, 53);
        table.insert(flow, 'u', Direction::Uplink, 0, now);

        let before = table.lookup(
            &flow,
            Direction::Uplink,
            0,
            now + UDP_IDLE - Duration::from_secs(1),
        );
        let after = table.lookup(&flow, Direction::Uplink, 0, now + 2 * UDP_IDLE);

        assert_eq!(before, Some('u'));
        assert_eq!(after, None);
    }

    #[test]
    fn activity_keeps_a_flow_alive() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Udp, 1, 53);
        table.insert(flow, 'u', Direction::Uplink, 0, now);

        let step = UDP_IDLE - Duration::from_secs(1);
        table.lookup(&flow, Direction::Downlink, 0, now + step);
        let later = table.lookup(&flow, Direction::Uplink, 0, now + 2 * step);

        assert_eq!(later, Some('u'));
    }

    #[test]
    fn an_icmp_flow_uses_the_shorter_icmp_timeout() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::IcmpEcho, 1, 0);
        table.insert(flow, 'i', Direction::Uplink, 0, now);

        assert_eq!(
            table.lookup(
                &flow,
                Direction::Downlink,
                0,
                now + ICMP_IDLE + Duration::from_secs(1)
            ),
            None
        );
    }

    #[test]
    fn a_reset_tcp_flow_lingers_briefly_then_expires() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Tcp, 1, 443);
        table.insert(flow, 't', Direction::Uplink, TCP_SYN, now);

        table.lookup(&flow, Direction::Downlink, TCP_RST, now);
        let during = table.lookup(&flow, Direction::Uplink, TCP_ACK, now + CLOSING_LINGER / 2);
        let after = table.lookup(&flow, Direction::Uplink, TCP_ACK, now + CLOSING_LINGER * 2);

        assert_eq!(during, Some('t'));
        assert_eq!(after, None);
    }

    #[test]
    fn a_tcp_flow_closed_in_both_directions_expires_after_the_linger() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Tcp, 1, 443);
        table.insert(flow, 't', Direction::Uplink, TCP_SYN, now);

        table.lookup(&flow, Direction::Uplink, TCP_FIN | TCP_ACK, now);
        table.lookup(&flow, Direction::Downlink, TCP_FIN | TCP_ACK, now);

        assert_eq!(
            table.lookup(&flow, Direction::Uplink, TCP_ACK, now + CLOSING_LINGER * 2),
            None
        );
    }

    #[test]
    fn a_half_closed_tcp_flow_stays_open() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Tcp, 1, 443);
        table.insert(flow, 't', Direction::Uplink, TCP_SYN, now);

        table.lookup(&flow, Direction::Uplink, TCP_FIN | TCP_ACK, now);
        table.lookup(&flow, Direction::Uplink, TCP_FIN | TCP_ACK, now);

        assert_eq!(
            table.lookup(
                &flow,
                Direction::Downlink,
                TCP_ACK,
                now + CLOSING_LINGER * 2
            ),
            Some('t')
        );
    }

    #[test]
    fn a_full_table_evicts_the_least_recently_active_flows() {
        let capacity = 32;
        let mut table = FlowTable::new(capacity);
        let now = t0();
        for port in 0..capacity as u16 {
            table.insert(
                key(Transport::Tcp, port, 443),
                port,
                Direction::Uplink,
                TCP_SYN,
                now + Duration::from_millis(u64::from(port)),
            );
        }
        let later = now + Duration::from_secs(1);

        table.insert(
            key(Transport::Tcp, 1000, 443),
            1000,
            Direction::Uplink,
            TCP_SYN,
            later,
        );

        assert!(table.len() <= capacity);
        assert_eq!(
            table.lookup(&key(Transport::Tcp, 1000, 443), Direction::Uplink, 0, later),
            Some(1000)
        );
        assert_eq!(
            table.lookup(&key(Transport::Tcp, 0, 443), Direction::Uplink, 0, later),
            None
        );
        let newest = capacity as u16 - 1;
        assert_eq!(
            table.lookup(
                &key(Transport::Tcp, newest, 443),
                Direction::Uplink,
                0,
                later
            ),
            Some(newest)
        );
    }

    #[test]
    fn a_full_table_evicts_an_expired_flow_before_any_live_one() {
        // The TCP flow is the least recently active, but only the more
        // recent ICMP flow has outlived its timeout.
        let mut table = FlowTable::new(2);
        let now = t0();
        table.insert(
            key(Transport::Tcp, 1, 443),
            1,
            Direction::Uplink,
            TCP_SYN,
            now,
        );
        let echo_seen = now + Duration::from_secs(10);
        table.insert(
            key(Transport::IcmpEcho, 2, 0),
            2,
            Direction::Uplink,
            0,
            echo_seen,
        );
        let later = echo_seen + ICMP_IDLE + Duration::from_secs(1);

        table.insert(
            key(Transport::Tcp, 3, 443),
            3,
            Direction::Uplink,
            TCP_SYN,
            later,
        );

        assert_eq!(table.len(), 2);
        assert_eq!(
            table.lookup(&key(Transport::Tcp, 1, 443), Direction::Uplink, 0, later),
            Some(1)
        );
    }

    #[test]
    fn eviction_frees_a_sixteenth_even_when_every_flow_was_seen_at_once() {
        let capacity = 64;
        let mut table = FlowTable::new(capacity);
        let now = t0();
        for port in 0..capacity as u16 {
            table.insert(
                key(Transport::Tcp, port, 443),
                port,
                Direction::Uplink,
                TCP_SYN,
                now,
            );
        }

        table.insert(
            key(Transport::Tcp, 1000, 443),
            1000,
            Direction::Uplink,
            TCP_SYN,
            now,
        );

        assert_eq!(table.len(), capacity - capacity / 16 + 1);
    }

    #[test]
    fn peeking_at_a_flow_does_not_count_as_activity() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Udp, 1, 53);
        table.insert(flow, 'u', Direction::Uplink, 0, now);

        let peeked = table.peek(&flow, now + UDP_IDLE - Duration::from_secs(1));
        let later = table.lookup(
            &flow,
            Direction::Uplink,
            0,
            now + UDP_IDLE + Duration::from_secs(1),
        );

        assert_eq!((peeked, later), (Some('u'), None));
    }

    #[test]
    fn peeking_misses_an_expired_flow() {
        let mut table = FlowTable::new(8);
        let now = t0();
        let flow = key(Transport::Udp, 1, 53);
        table.insert(flow, 'u', Direction::Uplink, 0, now);

        assert_eq!(table.peek(&flow, now + UDP_IDLE * 2), None);
    }

    #[test]
    fn clearing_forgets_every_flow() {
        let mut table = FlowTable::new(8);
        let now = t0();
        table.insert(
            key(Transport::Tcp, 1, 443),
            1,
            Direction::Uplink,
            TCP_SYN,
            now,
        );

        table.clear();

        assert!(table.is_empty());
        assert_eq!(
            table.lookup(&key(Transport::Tcp, 1, 443), Direction::Uplink, 0, now),
            None
        );
    }

    #[test]
    fn expire_drops_every_silent_flow() {
        let mut table = FlowTable::new(8);
        let now = t0();
        table.insert(key(Transport::Udp, 1, 53), 1, Direction::Uplink, 0, now);
        table.insert(
            key(Transport::Tcp, 2, 443),
            2,
            Direction::Uplink,
            TCP_SYN,
            now,
        );

        table.expire(now + UDP_IDLE + Duration::from_secs(1));

        assert_eq!(table.len(), 1);
    }

    #[test]
    fn a_flow_key_renders_without_its_addresses() {
        let rendered = format!("{:?}", key(Transport::Tcp, 1, 443));

        assert!(!rendered.contains("10.64.0.2"), "{rendered}");
        assert!(!rendered.contains("198.51.100.9"), "{rendered}");
    }
}
