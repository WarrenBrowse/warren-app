//! The XNU `net.inet.{tcp,udp}.pcblist_n` export.
//!
//! Its structs (`xinpgen`, `xinpcb_n`, `xsocket_n`) are private to the kernel
//! (bsd/netinet/in_pcb.h and bsd/sys/socketvar.h, declared under
//! `#pragma pack(4)`) and absent from the SDK, so their offsets are written
//! out here. The export is an `xinpgen`, then for each PCB a run of records
//! that each begin with `{u32 len; u32 kind}` and are padded to 8 bytes, then
//! a closing `xinpgen`. A record of a known kind with an unexpected length
//! means the layout moved, and the whole snapshot is refused rather than read
//! at the wrong offsets. The real-socket test in `macos.rs` pins the offsets
//! against the running kernel.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::{OwnerError, SocketRecord, SocketTable};
use crate::flow::Transport;

const XINPGEN_LEN: usize = 24;
const KIND_SOCKET: u32 = 0x001;
const KIND_INPCB: u32 = 0x010;

const XINPCB_N_LEN: usize = 104;
const INP_FPORT: usize = 16;
const INP_LPORT: usize = 18;
const INP_VFLAG: usize = 44;
const INP_FADDR: usize = 48;
const INP_LADDR: usize = 64;
const INP_IPV4: u8 = 0x1;
const INP_IPV6: u8 = 0x2;
/// An IPv4 address sits in the last word of the 16-byte address union
/// (`in_addr_4in6`).
const V4_IN_UNION: usize = 12;

const XSOCKET_N_MIN_LEN: usize = 76;
const SO_LAST_PID: usize = 68;
/// The effective pid: the app a system daemon opened the socket for, when it
/// delegated it (`SO_DELEGATED`).
const SO_E_PID: usize = 72;

/// Adds the sockets of one `pcblist_n` export to `table`.
///
/// # Errors
///
/// [`OwnerError::UnknownLayout`] when the export does not have the layout
/// described above.
pub(super) fn parse(
    buf: &[u8],
    transport: Transport,
    table: &mut SocketTable,
) -> Result<(), OwnerError> {
    if read_u32(buf, 0).map(|len| len as usize) != Some(XINPGEN_LEN) {
        return Err(OwnerError::UnknownLayout);
    }
    let mut offset = XINPGEN_LEN;
    let mut pending = None;
    while let (Some(len), Some(kind)) = (read_u32(buf, offset), read_u32(buf, offset + 4)) {
        let len = len as usize;
        // The closing xinpgen is told apart by its place, not its fields: its
        // second word is a PCB count that can equal any kind.
        if len == 0 || (len == XINPGEN_LEN && offset + len >= buf.len()) {
            break;
        }
        let record = buf
            .get(offset..offset + len)
            .ok_or(OwnerError::UnknownLayout)?;
        match kind {
            KIND_INPCB => {
                if len != XINPCB_N_LEN {
                    return Err(OwnerError::UnknownLayout);
                }
                pending = ends(record);
            }
            KIND_SOCKET => {
                if len < XSOCKET_N_MIN_LEN {
                    return Err(OwnerError::UnknownLayout);
                }
                if let Some((local, remote)) = pending.take() {
                    push(table, transport, local, remote, record);
                }
            }
            _ => {}
        }
        offset += len.next_multiple_of(8);
    }
    Ok(())
}

fn push(
    table: &mut SocketTable,
    transport: Transport,
    local: SocketAddr,
    remote: Option<SocketAddr>,
    socket: &[u8],
) {
    let last_pid = read_i32(socket, SO_LAST_PID).unwrap_or(0);
    let e_pid = read_i32(socket, SO_E_PID).unwrap_or(0);
    let pid = if e_pid > 0 { e_pid } else { last_pid };
    if let Ok(pid) = u32::try_from(pid) {
        table.push(SocketRecord {
            transport,
            local,
            remote,
            pid,
        });
    }
}

/// The local and remote ends of an `xinpcb_n`, `None` for a PCB of neither
/// IP version.
fn ends(inpcb: &[u8]) -> Option<(SocketAddr, Option<SocketAddr>)> {
    let vflag = inpcb[INP_VFLAG];
    let addr = |at: usize| -> Option<IpAddr> {
        if vflag & INP_IPV4 != 0 {
            let octets: [u8; 4] = inpcb[at + V4_IN_UNION..at + 16].try_into().ok()?;
            Some(IpAddr::V4(Ipv4Addr::from(octets)))
        } else if vflag & INP_IPV6 != 0 {
            let octets: [u8; 16] = inpcb[at..at + 16].try_into().ok()?;
            Some(IpAddr::V6(Ipv6Addr::from(octets)))
        } else {
            None
        }
    };
    let port = |at: usize| u16::from_be_bytes([inpcb[at], inpcb[at + 1]]);
    let local = SocketAddr::new(addr(INP_LADDR)?, port(INP_LPORT));
    let remote = SocketAddr::new(addr(INP_FADDR)?, port(INP_FPORT));
    let connected = !(remote.ip().is_unspecified() && remote.port() == 0);
    Some((local, connected.then_some(remote)))
}

fn read_u32(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

fn read_i32(buf: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_ne_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowKey;

    struct Export(Vec<u8>);

    impl Export {
        fn new() -> Self {
            let mut buf = vec![0; XINPGEN_LEN];
            buf[..4].copy_from_slice(&(XINPGEN_LEN as u32).to_ne_bytes());
            Self(buf)
        }

        fn record(mut self, kind: u32, len: usize, fill: impl FnOnce(&mut [u8])) -> Self {
            let mut record = vec![0; len];
            record[..4].copy_from_slice(&(len as u32).to_ne_bytes());
            record[4..8].copy_from_slice(&kind.to_ne_bytes());
            fill(&mut record);
            self.0.extend_from_slice(&record);
            self.0.resize(self.0.len().next_multiple_of(8), 0);
            self
        }

        fn inpcb(self, local: SocketAddr, remote: SocketAddr) -> Self {
            self.record(KIND_INPCB, XINPCB_N_LEN, |record| {
                record[INP_FPORT..INP_FPORT + 2].copy_from_slice(&remote.port().to_be_bytes());
                record[INP_LPORT..INP_LPORT + 2].copy_from_slice(&local.port().to_be_bytes());
                for (at, ip) in [(INP_FADDR, remote.ip()), (INP_LADDR, local.ip())] {
                    match ip {
                        IpAddr::V4(v4) => {
                            record[INP_VFLAG] = INP_IPV4;
                            record[at + V4_IN_UNION..at + 16].copy_from_slice(&v4.octets());
                        }
                        IpAddr::V6(v6) => {
                            record[INP_VFLAG] = INP_IPV6;
                            record[at..at + 16].copy_from_slice(&v6.octets());
                        }
                    }
                }
            })
        }

        fn socket(self, last_pid: i32, e_pid: i32) -> Self {
            self.record(KIND_SOCKET, 104, |record| {
                record[SO_LAST_PID..SO_LAST_PID + 4].copy_from_slice(&last_pid.to_ne_bytes());
                record[SO_E_PID..SO_E_PID + 4].copy_from_slice(&e_pid.to_ne_bytes());
            })
        }

        /// A receive buffer record, whose odd length exercises the padding.
        fn sockbuf(self) -> Self {
            self.record(0x002, 20, |_| {})
        }

        fn close(mut self) -> Vec<u8> {
            let mut trailer = vec![0; XINPGEN_LEN];
            trailer[..4].copy_from_slice(&(XINPGEN_LEN as u32).to_ne_bytes());
            trailer[4..8].copy_from_slice(&KIND_INPCB.to_ne_bytes());
            self.0.extend_from_slice(&trailer);
            self.0
        }
    }

    fn v4(octets: [u8; 4], port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), port)
    }

    fn owner(
        buf: &[u8],
        transport: Transport,
        local: SocketAddr,
        remote: SocketAddr,
    ) -> Option<u32> {
        let mut table = SocketTable::default();
        parse(buf, transport, &mut table).unwrap();
        table.finish();
        table.owner(&FlowKey {
            transport,
            local,
            remote,
        })
    }

    #[test]
    fn reads_the_ends_and_the_pid_of_each_socket() {
        let local = v4([10, 64, 0, 2], 50000);
        let remote = v4([198, 51, 100, 9], 443);
        let buf = Export::new()
            .inpcb(v4([10, 64, 0, 2], 50001), remote)
            .socket(41, 0)
            .sockbuf()
            .sockbuf()
            .inpcb(local, remote)
            .socket(42, 0)
            .sockbuf()
            .close();

        assert_eq!(owner(&buf, Transport::Tcp, local, remote), Some(42));
    }

    #[test]
    fn a_socket_record_belongs_to_the_pcb_just_before_it() {
        let orphan = v4([10, 64, 0, 2], 50001);
        let local = v4([10, 64, 0, 2], 50000);
        let remote = v4([198, 51, 100, 9], 443);
        let buf = Export::new()
            .inpcb(orphan, remote)
            .inpcb(local, remote)
            .socket(46, 0)
            .close();

        assert_eq!(owner(&buf, Transport::Tcp, local, remote), Some(46));
        assert_eq!(owner(&buf, Transport::Tcp, orphan, remote), None);
    }

    #[test]
    fn prefers_the_pid_a_socket_was_delegated_for() {
        let local = v4([10, 64, 0, 2], 50000);
        let remote = v4([198, 51, 100, 9], 443);
        let buf = Export::new().inpcb(local, remote).socket(300, 501).close();

        assert_eq!(owner(&buf, Transport::Tcp, local, remote), Some(501));
    }

    #[test]
    fn reads_ipv6_ends() {
        let local = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 2)),
            50000,
        );
        let remote = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 9)),
            443,
        );
        let buf = Export::new().inpcb(local, remote).socket(43, 0).close();

        assert_eq!(owner(&buf, Transport::Udp, local, remote), Some(43));
    }

    #[test]
    fn an_unconnected_socket_has_no_remote_end() {
        let bound = v4([0, 0, 0, 0], 5353);
        let buf = Export::new()
            .inpcb(bound, v4([0, 0, 0, 0], 0))
            .socket(44, 0)
            .close();

        let flow_local = v4([10, 64, 0, 2], 5353);
        assert_eq!(
            owner(&buf, Transport::Udp, flow_local, v4([198, 51, 100, 9], 53)),
            Some(44)
        );
    }

    #[test]
    fn refuses_a_pcb_record_of_another_length() {
        let buf = Export::new()
            .record(KIND_INPCB, XINPCB_N_LEN + 8, |_| {})
            .socket(45, 0)
            .close();

        let result = parse(&buf, Transport::Tcp, &mut SocketTable::default());

        assert!(
            matches!(result, Err(OwnerError::UnknownLayout)),
            "{result:?}"
        );
    }

    #[test]
    fn refuses_a_socket_record_too_short_for_its_pids() {
        let buf = Export::new()
            .inpcb(v4([10, 64, 0, 2], 1), v4([198, 51, 100, 9], 2))
            .record(KIND_SOCKET, XSOCKET_N_MIN_LEN - 4, |_| {})
            .close();

        let result = parse(&buf, Transport::Tcp, &mut SocketTable::default());

        assert!(
            matches!(result, Err(OwnerError::UnknownLayout)),
            "{result:?}"
        );
    }

    #[test]
    fn refuses_an_export_with_another_header() {
        let mut buf = Export::new().close();
        buf[..4].copy_from_slice(&32u32.to_ne_bytes());

        let result = parse(&buf, Transport::Tcp, &mut SocketTable::default());

        assert!(
            matches!(result, Err(OwnerError::UnknownLayout)),
            "{result:?}"
        );
    }

    #[test]
    fn refuses_a_record_running_past_the_export() {
        let mut buf = Export::new()
            .inpcb(v4([10, 64, 0, 2], 1), v4([198, 51, 100, 9], 2))
            .0;
        buf.truncate(buf.len() - 8);

        let result = parse(&buf, Transport::Tcp, &mut SocketTable::default());

        assert!(
            matches!(result, Err(OwnerError::UnknownLayout)),
            "{result:?}"
        );
    }
}
