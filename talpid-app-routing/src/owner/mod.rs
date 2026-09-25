//! Which process owns the local socket of a flow, and what it runs.
//!
//! This is the system boundary of the crate: [`OwnerResolver`] is the one
//! interface the router calls, and [`SystemResolver`] implements it with the
//! host's own tables. The macOS and Windows tables are read as a whole into a
//! [`SocketTable`] snapshot; Linux asks the kernel for the one socket and keeps
//! only its inode-to-pid index as a snapshot.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use crate::{
    app::ProcessKey,
    flow::{FlowKey, Transport},
};

#[cfg(any(target_os = "linux", test))]
mod conntrack;
#[cfg(any(target_os = "macos", test))]
mod pcblist;
#[cfg(any(windows, test))]
mod win_tables;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::SystemResolver;
#[cfg(target_os = "macos")]
pub use macos::SystemResolver;
#[cfg(windows)]
pub use windows::SystemResolver;

/// Why the socket tables could not be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OwnerError {
    #[error("reading a socket table failed")]
    SocketTable(#[source] std::io::Error),
    #[error("a socket table has a layout this build does not know")]
    UnknownLayout,
}

/// Answers, for a flow seen on the TUN device, which process owns it.
pub trait OwnerResolver {
    /// The pid owning the local socket of `flow`, answered from a view of
    /// the OS no older than the last [`Self::refresh`]. `None` when that view
    /// has no such socket, or no single owner for it.
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32>;

    /// Makes the next answers reflect the OS as it is now.
    ///
    /// # Errors
    ///
    /// An [`OwnerError`] when the OS tables cannot be read or parsed; the
    /// view is then empty rather than stale.
    fn refresh(&mut self) -> Result<(), OwnerError>;

    /// The identity of a live process and of the program it runs, so neither
    /// a recycled pid nor an `exec` inherits a decision. `None` once the
    /// process has exited.
    fn process_key(&mut self, pid: u32) -> Option<ProcessKey>;

    /// The executable a live process runs.
    fn executable(&mut self, pid: u32) -> Option<PathBuf>;
}

/// One socket as an OS table lists it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SocketRecord {
    pub transport: Transport,
    /// The local address may be unspecified for a socket bound to all.
    pub local: SocketAddr,
    /// `None` for a socket that is not connected.
    pub remote: Option<SocketAddr>,
    pub pid: u32,
}

/// A snapshot of the host's sockets, indexed by transport and local port.
#[derive(Default)]
pub struct SocketTable {
    records: Vec<SocketRecord>,
}

impl SocketTable {
    /// Empties the table, keeping its storage for the next snapshot.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Adds a socket. IPv4-mapped IPv6 addresses, which dual-stack sockets
    /// report, are stored as the IPv4 addresses packets carry.
    pub fn push(&mut self, record: SocketRecord) {
        self.records.push(SocketRecord {
            local: unmapped(record.local),
            remote: record.remote.map(unmapped),
            ..record
        });
    }

    /// Makes the table searchable after the last [`Self::push`].
    pub fn finish(&mut self) {
        self.records
            .sort_unstable_by_key(|record| (transport_rank(record.transport), record.local.port()));
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The pid owning the local socket of `flow`: the connected socket with
    /// exactly this pair of ends, else for UDP the unconnected socket on the
    /// local port, when a single process holds it. ICMP sockets are not in
    /// these tables.
    pub fn owner(&self, flow: &FlowKey) -> Option<u32> {
        let key = (transport_rank(flow.transport), flow.local.port());
        let start = self.records.partition_point(|record| {
            (transport_rank(record.transport), record.local.port()) < key
        });
        let candidates = self.records[start..]
            .iter()
            .take_while(|record| (transport_rank(record.transport), record.local.port()) == key)
            .filter(|record| record.pid != 0 && local_matches(record.local.ip(), flow.local.ip()));
        let mut unconnected = None;
        let mut shared = false;
        for record in candidates {
            match record.remote {
                Some(remote) if remote == flow.remote => return Some(record.pid),
                None if flow.transport == Transport::Udp => match unconnected {
                    None => unconnected = Some(record.pid),
                    Some(pid) => shared |= pid != record.pid,
                },
                _ => {}
            }
        }
        // Two processes on one port (SO_REUSEPORT, mDNS): which one sent the
        // packet cannot be told.
        if shared { None } else { unconnected }
    }
}

fn transport_rank(transport: Transport) -> u8 {
    match transport {
        Transport::Tcp => 0,
        Transport::Udp => 1,
        Transport::IcmpEcho => 2,
    }
}

fn local_matches(bound: IpAddr, packet: IpAddr) -> bool {
    bound.is_unspecified() || bound == packet
}

fn unmapped(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(addr.ip().to_canonical(), addr.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: [u8; 4] = [10, 64, 0, 2];
    const REMOTE: [u8; 4] = [198, 51, 100, 9];

    fn addr(ip: impl Into<IpAddr>, port: u16) -> SocketAddr {
        SocketAddr::new(ip.into(), port)
    }

    fn flow(transport: Transport, local_port: u16, remote_port: u16) -> FlowKey {
        FlowKey {
            transport,
            local: addr(LOCAL, local_port),
            remote: addr(REMOTE, remote_port),
        }
    }

    fn table(records: &[SocketRecord]) -> SocketTable {
        let mut table = SocketTable::default();
        for record in records {
            table.push(*record);
        }
        table.finish();
        table
    }

    fn record(
        transport: Transport,
        local: SocketAddr,
        remote: Option<SocketAddr>,
        pid: u32,
    ) -> SocketRecord {
        SocketRecord {
            transport,
            local,
            remote,
            pid,
        }
    }

    #[test]
    fn finds_the_connected_socket_with_both_ends() {
        let table = table(&[
            record(
                Transport::Udp,
                addr(LOCAL, 50000),
                Some(addr(REMOTE, 443)),
                3,
            ),
            record(
                Transport::Tcp,
                addr(LOCAL, 60000),
                Some(addr(REMOTE, 443)),
                4,
            ),
            record(
                Transport::Tcp,
                addr(LOCAL, 50000),
                Some(addr(REMOTE, 80)),
                1,
            ),
            record(
                Transport::Tcp,
                addr(LOCAL, 50000),
                Some(addr(REMOTE, 443)),
                2,
            ),
        ]);

        assert_eq!(table.owner(&flow(Transport::Tcp, 50000, 443)), Some(2));
        assert_eq!(table.owner(&flow(Transport::Udp, 50000, 443)), Some(3));
        assert_eq!(table.owner(&flow(Transport::Tcp, 50001, 443)), None);
    }

    #[test]
    fn a_socket_bound_to_all_addresses_owns_the_flows_of_its_port() {
        let any = IpAddr::from([0, 0, 0, 0]);
        let table = table(&[record(
            Transport::Tcp,
            addr(any, 50000),
            Some(addr(REMOTE, 443)),
            4,
        )]);

        assert_eq!(table.owner(&flow(Transport::Tcp, 50000, 443)), Some(4));
    }

    #[test]
    fn an_unconnected_udp_socket_owns_any_flow_from_its_port() {
        let any = IpAddr::from([0u16; 8]);
        let table = table(&[record(Transport::Udp, addr(any, 5353), None, 5)]);

        assert_eq!(table.owner(&flow(Transport::Udp, 5353, 53)), Some(5));
    }

    #[test]
    fn a_connected_udp_socket_wins_over_an_unconnected_one_on_the_same_port() {
        let any = IpAddr::from([0, 0, 0, 0]);
        let table = table(&[
            record(Transport::Udp, addr(any, 5353), None, 5),
            record(Transport::Udp, addr(LOCAL, 5353), Some(addr(REMOTE, 53)), 6),
        ]);

        assert_eq!(table.owner(&flow(Transport::Udp, 5353, 53)), Some(6));
    }

    #[test]
    fn a_port_shared_by_unconnected_sockets_of_two_processes_has_no_single_owner() {
        let any = IpAddr::from([0, 0, 0, 0]);
        let table = table(&[
            record(Transport::Udp, addr(any, 5353), None, 5),
            record(Transport::Udp, addr(any, 5353), None, 5),
            record(Transport::Udp, addr(any, 5354), None, 6),
            record(Transport::Udp, addr(any, 5354), None, 7),
        ]);

        assert_eq!(table.owner(&flow(Transport::Udp, 5353, 53)), Some(5));
        assert_eq!(table.owner(&flow(Transport::Udp, 5354, 53)), None);
    }

    #[test]
    fn a_listening_tcp_socket_owns_no_flow() {
        let any = IpAddr::from([0, 0, 0, 0]);
        let table = table(&[record(Transport::Tcp, addr(any, 8080), None, 7)]);

        assert_eq!(table.owner(&flow(Transport::Tcp, 8080, 443)), None);
    }

    #[test]
    fn a_socket_bound_to_another_address_does_not_own_the_flow() {
        let table = table(&[record(Transport::Udp, addr([10, 0, 0, 9], 5353), None, 8)]);

        assert_eq!(table.owner(&flow(Transport::Udp, 5353, 53)), None);
    }

    #[test]
    fn a_dual_stack_socket_owns_its_ipv4_flows() {
        let mapped =
            |octets: [u8; 4]| IpAddr::from(std::net::Ipv4Addr::from(octets).to_ipv6_mapped());
        let table = table(&[record(
            Transport::Tcp,
            addr(mapped(LOCAL), 50000),
            Some(addr(mapped(REMOTE), 443)),
            9,
        )]);

        assert_eq!(table.owner(&flow(Transport::Tcp, 50000, 443)), Some(9));
    }

    #[test]
    fn a_socket_without_a_process_owns_nothing() {
        let table = table(&[record(
            Transport::Tcp,
            addr(LOCAL, 50000),
            Some(addr(REMOTE, 443)),
            0,
        )]);

        assert_eq!(table.owner(&flow(Transport::Tcp, 50000, 443)), None);
    }

    #[test]
    fn icmp_flows_have_no_owner_in_socket_tables() {
        let table = table(&[record(Transport::Udp, addr(LOCAL, 7), None, 10)]);

        assert_eq!(table.owner(&flow(Transport::IcmpEcho, 7, 0)), None);
    }

    #[test]
    fn a_cleared_table_forgets_every_socket() {
        let mut table = table(&[record(
            Transport::Tcp,
            addr(LOCAL, 50000),
            Some(addr(REMOTE, 443)),
            1,
        )]);

        table.clear();
        table.finish();

        assert!(table.is_empty());
        assert_eq!(table.owner(&flow(Transport::Tcp, 50000, 443)), None);
    }
}
