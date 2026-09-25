//! The tables `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)` and
//! `GetExtendedUdpTable(UDP_TABLE_OWNER_PID)` return, read as bytes.
//!
//! Each is a `u32` row count followed by `MIB_*ROW_OWNER_PID` rows. Addresses
//! are stored in network order, and each port in network order in the first
//! two bytes of its `DWORD`, so both read the same on any host. Parsing the
//! bytes rather than casting to the Windows structs keeps the layout under
//! test on every platform.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::{OwnerError, SocketRecord, SocketTable};
use crate::flow::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TableKind {
    Tcp4,
    Tcp6,
    Udp4,
    Udp6,
}

impl TableKind {
    fn row_len(self) -> usize {
        match self {
            TableKind::Tcp4 => 24,
            TableKind::Tcp6 => 56,
            TableKind::Udp4 => 12,
            TableKind::Udp6 => 28,
        }
    }
}

/// Adds the rows of one owner-pid table to `table`.
///
/// # Errors
///
/// [`OwnerError::UnknownLayout`] when the row count runs past the buffer.
pub(super) fn parse(
    buf: &[u8],
    kind: TableKind,
    table: &mut SocketTable,
) -> Result<(), OwnerError> {
    let count = buf
        .get(..4)
        .map(|word| u32::from_ne_bytes([word[0], word[1], word[2], word[3]]) as usize)
        .ok_or(OwnerError::UnknownLayout)?;
    let row_len = kind.row_len();
    let rows = count
        .checked_mul(row_len)
        .and_then(|len| buf.get(4..4 + len))
        .ok_or(OwnerError::UnknownLayout)?;
    for row in rows.chunks_exact(row_len) {
        table.push(record(row, kind));
    }
    Ok(())
}

fn record(row: &[u8], kind: TableKind) -> SocketRecord {
    let v4 = |at: usize| {
        IpAddr::V4(Ipv4Addr::new(
            row[at],
            row[at + 1],
            row[at + 2],
            row[at + 3],
        ))
    };
    let v6 = |at: usize| {
        let octets: [u8; 16] = row[at..at + 16].try_into().expect("16 address bytes");
        IpAddr::V6(Ipv6Addr::from(octets))
    };
    let port = |at: usize| u16::from_be_bytes([row[at], row[at + 1]]);
    let dword = |at: usize| u32::from_ne_bytes([row[at], row[at + 1], row[at + 2], row[at + 3]]);
    // A listening TCP row has 0.0.0.0:0 as its remote end, which no flow has,
    // so it needs no special case.
    let (transport, local, remote, pid) = match kind {
        TableKind::Tcp4 => (
            Transport::Tcp,
            SocketAddr::new(v4(4), port(8)),
            Some(SocketAddr::new(v4(12), port(16))),
            dword(20),
        ),
        TableKind::Tcp6 => (
            Transport::Tcp,
            SocketAddr::new(v6(0), port(20)),
            Some(SocketAddr::new(v6(24), port(44))),
            dword(52),
        ),
        TableKind::Udp4 => (
            Transport::Udp,
            SocketAddr::new(v4(0), port(4)),
            None,
            dword(8),
        ),
        TableKind::Udp6 => (
            Transport::Udp,
            SocketAddr::new(v6(0), port(20)),
            None,
            dword(24),
        ),
    };
    SocketRecord {
        transport,
        local,
        remote,
        pid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowKey;

    fn port_dword(port: u16) -> [u8; 4] {
        let be = port.to_be_bytes();
        [be[0], be[1], 0, 0]
    }

    fn table_bytes(rows: &[Vec<u8>]) -> Vec<u8> {
        let mut buf = (rows.len() as u32).to_ne_bytes().to_vec();
        for row in rows {
            buf.extend_from_slice(row);
        }
        buf
    }

    fn tcp4_row(state: u32, local: SocketAddr, remote: SocketAddr, pid: u32) -> Vec<u8> {
        let (IpAddr::V4(local_ip), IpAddr::V4(remote_ip)) = (local.ip(), remote.ip()) else {
            panic!("IPv4 row");
        };
        let mut row = state.to_ne_bytes().to_vec();
        row.extend_from_slice(&local_ip.octets());
        row.extend_from_slice(&port_dword(local.port()));
        row.extend_from_slice(&remote_ip.octets());
        row.extend_from_slice(&port_dword(remote.port()));
        row.extend_from_slice(&pid.to_ne_bytes());
        row
    }

    fn tcp6_row(state: u32, local: SocketAddr, remote: SocketAddr, pid: u32) -> Vec<u8> {
        let (IpAddr::V6(local_ip), IpAddr::V6(remote_ip)) = (local.ip(), remote.ip()) else {
            panic!("IPv6 row");
        };
        let mut row = local_ip.octets().to_vec();
        row.extend_from_slice(&0u32.to_ne_bytes());
        row.extend_from_slice(&port_dword(local.port()));
        row.extend_from_slice(&remote_ip.octets());
        row.extend_from_slice(&0u32.to_ne_bytes());
        row.extend_from_slice(&port_dword(remote.port()));
        row.extend_from_slice(&state.to_ne_bytes());
        row.extend_from_slice(&pid.to_ne_bytes());
        row
    }

    fn udp4_row(local: SocketAddr, pid: u32) -> Vec<u8> {
        let IpAddr::V4(ip) = local.ip() else {
            panic!("IPv4 row")
        };
        let mut row = ip.octets().to_vec();
        row.extend_from_slice(&port_dword(local.port()));
        row.extend_from_slice(&pid.to_ne_bytes());
        row
    }

    fn udp6_row(local: SocketAddr, pid: u32) -> Vec<u8> {
        let IpAddr::V6(ip) = local.ip() else {
            panic!("IPv6 row")
        };
        let mut row = ip.octets().to_vec();
        row.extend_from_slice(&0u32.to_ne_bytes());
        row.extend_from_slice(&port_dword(local.port()));
        row.extend_from_slice(&pid.to_ne_bytes());
        row
    }

    fn owner(buf: &[u8], kind: TableKind, flow: FlowKey) -> Option<u32> {
        let mut table = SocketTable::default();
        parse(buf, kind, &mut table).unwrap();
        table.finish();
        table.owner(&flow)
    }

    fn flow(transport: Transport, local: &str, remote: &str) -> FlowKey {
        FlowKey {
            transport,
            local: local.parse().unwrap(),
            remote: remote.parse().unwrap(),
        }
    }

    const ESTABLISHED: u32 = 5;

    #[test]
    fn reads_the_ends_and_pid_of_ipv4_tcp_rows() {
        let buf = table_bytes(&[
            tcp4_row(
                ESTABLISHED,
                "10.64.0.2:50001".parse().unwrap(),
                "198.51.100.9:443".parse().unwrap(),
                11,
            ),
            tcp4_row(
                ESTABLISHED,
                "10.64.0.2:50000".parse().unwrap(),
                "198.51.100.9:443".parse().unwrap(),
                12,
            ),
        ]);

        let found = owner(
            &buf,
            TableKind::Tcp4,
            flow(Transport::Tcp, "10.64.0.2:50000", "198.51.100.9:443"),
        );

        assert_eq!(found, Some(12));
    }

    #[test]
    fn reads_the_ends_and_pid_of_ipv6_tcp_rows() {
        let buf = table_bytes(&[tcp6_row(
            ESTABLISHED,
            "[fd00::2]:50000".parse().unwrap(),
            "[2001:db8::9]:443".parse().unwrap(),
            14,
        )]);

        let found = owner(
            &buf,
            TableKind::Tcp6,
            flow(Transport::Tcp, "[fd00::2]:50000", "[2001:db8::9]:443"),
        );

        assert_eq!(found, Some(14));
    }

    #[test]
    fn a_udp_row_owns_the_flows_of_its_port() {
        let v4 = table_bytes(&[udp4_row("0.0.0.0:5353".parse().unwrap(), 15)]);
        let v6 = table_bytes(&[udp6_row("[::]:5354".parse().unwrap(), 16)]);

        let found4 = owner(
            &v4,
            TableKind::Udp4,
            flow(Transport::Udp, "10.64.0.2:5353", "198.51.100.9:53"),
        );
        let found6 = owner(
            &v6,
            TableKind::Udp6,
            flow(Transport::Udp, "[fd00::2]:5354", "[2001:db8::9]:53"),
        );

        assert_eq!((found4, found6), (Some(15), Some(16)));
    }

    #[test]
    fn refuses_a_row_count_past_the_buffer() {
        let mut buf = table_bytes(&[udp4_row("0.0.0.0:5353".parse().unwrap(), 15)]);
        buf[..4].copy_from_slice(&2u32.to_ne_bytes());

        let result = parse(&buf, TableKind::Udp4, &mut SocketTable::default());

        assert!(
            matches!(result, Err(OwnerError::UnknownLayout)),
            "{result:?}"
        );
    }
}
