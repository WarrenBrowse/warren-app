//! The Linux resolver: an exact `NETLINK_SOCK_DIAG` lookup for the socket of
//! one flow, which is live, then its inode's owner through an index of
//! `/proc/*/fd`. An index entry is checked against that process's own
//! descriptors before it is trusted, and a miss rebuilds the whole index at
//! most once per refresh, since the walk reads every process's descriptors.

use std::{
    collections::HashMap,
    ffi::c_void,
    io,
    net::{IpAddr, SocketAddr},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::PathBuf,
};

use super::{OwnerError, OwnerResolver};
use crate::{
    app::ProcessKey,
    flow::{FlowKey, Transport},
};

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLM_F_REQUEST: u16 = 1;
const NLMSG_HDR_LEN: usize = 16;
const REQUEST_LEN: usize = NLMSG_HDR_LEN + 56;
/// `idiag_inode` inside a reply: the netlink header, then `inet_diag_msg`
/// (four bytes, the 48-byte socket id, four `u32` fields).
const INODE_AT: usize = NLMSG_HDR_LEN + 4 + 48 + 16;

/// Looks each flow up in the kernel, and pids up in an inode index.
#[derive(Default)]
pub struct SystemResolver {
    socket: Option<OwnedFd>,
    inodes: HashMap<u64, u32>,
    /// Set by a refresh, spent by the first index rebuild after it.
    may_rebuild: bool,
    sequence: u32,
    reply: Vec<u8>,
}

impl SystemResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// The inode of the socket that owns `flow`.
    fn inode(&mut self, flow: &FlowKey) -> Option<u64> {
        let protocol = match flow.transport {
            Transport::Tcp => libc::IPPROTO_TCP as u8,
            Transport::Udp => libc::IPPROTO_UDP as u8,
            Transport::IcmpEcho => return None,
        };
        // The TCP lookup takes the socket's own pair, the UDP one the pair a
        // packet reaching the socket carries (measured on Linux 6.8; the UDP
        // query with the socket's pair finds nothing).
        let (src, dst) = match flow.transport {
            Transport::Udp => (flow.remote, flow.local),
            _ => (flow.local, flow.remote),
        };
        // A dual-stack socket's IPv4 flows are found by this IPv4 query too.
        self.query(protocol, src, dst)
    }

    fn query(&mut self, protocol: u8, src: SocketAddr, dst: SocketAddr) -> Option<u64> {
        let fd = self.socket().ok()?;
        self.sequence = self.sequence.wrapping_add(1);
        let request = request(self.sequence, protocol, src, dst);
        // SAFETY: `request` is a readable buffer of the length passed, and a
        // zeroed sockaddr_nl addresses the kernel.
        let sent = unsafe {
            let mut kernel: libc::sockaddr_nl = std::mem::zeroed();
            kernel.nl_family = libc::AF_NETLINK as libc::sa_family_t;
            libc::sendto(
                fd,
                request.as_ptr().cast::<c_void>(),
                request.len(),
                0,
                std::ptr::from_ref(&kernel).cast::<libc::sockaddr>(),
                size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if sent < 0 {
            self.socket = None;
            return None;
        }
        self.reply.resize(8192, 0);
        loop {
            // SAFETY: `reply` is writable for the length passed.
            let received = unsafe {
                libc::recv(
                    fd,
                    self.reply.as_mut_ptr().cast::<c_void>(),
                    self.reply.len(),
                    0,
                )
            };
            let received = usize::try_from(received).ok()?;
            match parse_reply(&self.reply[..received], self.sequence) {
                Reply::Inode(inode) => return Some(inode),
                Reply::Absent => return None,
                Reply::Other => continue,
            }
        }
    }

    fn socket(&mut self) -> io::Result<i32> {
        if let Some(socket) = &self.socket {
            return Ok(socket.as_raw_fd());
        }
        // SAFETY: plain socket creation; the result is checked.
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                libc::NETLINK_SOCK_DIAG,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is a fresh descriptor this struct now owns.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };
        // A reply that never comes must not stall the packet path, so a
        // socket that cannot time out is not used at all.
        let timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: 200_000,
        };
        // SAFETY: `timeout` is a valid timeval of the length passed.
        let status = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                std::ptr::from_ref(&timeout).cast::<c_void>(),
                size_of::<libc::timeval>() as libc::socklen_t,
            )
        };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        self.socket = Some(socket);
        Ok(fd)
    }
}

impl OwnerResolver for SystemResolver {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        // No socket for this flow is a final answer: the lookup is live.
        let inode = self.inode(flow)?;
        if let Some(pid) = self.inodes.get(&inode).copied()
            && holds_socket(pid, inode)
        {
            return Some(pid);
        }
        if !self.may_rebuild {
            return None;
        }
        self.may_rebuild = false;
        self.rebuild_index();
        self.inodes.get(&inode).copied()
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        self.may_rebuild = true;
        Ok(())
    }

    fn process_key(&mut self, pid: u32) -> Option<ProcessKey> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let executable = std::fs::metadata(format!("/proc/{pid}/exe")).ok()?;
        Some(ProcessKey {
            pid,
            start_time: parse_start_time(&stat)?,
            image: executable.ino(),
        })
    }

    fn executable(&mut self, pid: u32) -> Option<PathBuf> {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    }
}

impl SystemResolver {
    fn rebuild_index(&mut self) {
        self.inodes.clear();
        let Ok(processes) = std::fs::read_dir("/proc") else {
            return;
        };
        for process in processes.flatten() {
            let Some(pid) = process
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            for inode in socket_inodes(pid) {
                self.inodes.entry(inode).or_insert(pid);
            }
        }
    }
}

/// The inodes of the sockets `pid` holds. A process that exits or is not
/// ours to inspect holds none as far as this can see.
fn socket_inodes(pid: u32) -> impl Iterator<Item = u64> {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|fd| socket_inode(std::fs::read_link(fd.path()).ok()?.as_os_str().to_str()?))
}

fn holds_socket(pid: u32, inode: u64) -> bool {
    socket_inodes(pid).any(|held| held == inode)
}

/// The inode in a `/proc/<pid>/fd` link to a socket, `socket:[12345]`.
fn socket_inode(target: &str) -> Option<u64> {
    target
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// Field 22 of `/proc/<pid>/stat`, counted after the command name, which may
/// itself contain spaces and parentheses.
fn parse_start_time(stat: &str) -> Option<u64> {
    let (_, fields) = stat.rsplit_once(')')?;
    fields.split_whitespace().nth(19)?.parse().ok()
}

fn request(sequence: u32, protocol: u8, src: SocketAddr, dst: SocketAddr) -> [u8; REQUEST_LEN] {
    let mut request = [0u8; REQUEST_LEN];
    request[0..4].copy_from_slice(&(REQUEST_LEN as u32).to_ne_bytes());
    request[4..6].copy_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    request[6..8].copy_from_slice(&NLM_F_REQUEST.to_ne_bytes());
    request[8..12].copy_from_slice(&sequence.to_ne_bytes());
    let body = &mut request[NLMSG_HDR_LEN..];
    body[0] = match src.ip() {
        IpAddr::V4(_) => libc::AF_INET as u8,
        IpAddr::V6(_) => libc::AF_INET6 as u8,
    };
    body[1] = protocol;
    body[4..8].copy_from_slice(&u32::MAX.to_ne_bytes());
    let id = &mut body[8..];
    id[0..2].copy_from_slice(&src.port().to_be_bytes());
    id[2..4].copy_from_slice(&dst.port().to_be_bytes());
    id[4..20].copy_from_slice(&address_words(src.ip()));
    id[20..36].copy_from_slice(&address_words(dst.ip()));
    // INET_DIAG_NOCOOKIE: match on the addresses, not a cookie.
    id[40..48].copy_from_slice(&[0xff; 8]);
    request
}

fn address_words(ip: IpAddr) -> [u8; 16] {
    match ip {
        IpAddr::V4(v4) => {
            let mut words = [0u8; 16];
            words[..4].copy_from_slice(&v4.octets());
            words
        }
        IpAddr::V6(v6) => v6.octets(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Inode(u64),
    Absent,
    /// A reply to an earlier request that timed out.
    Other,
}

fn parse_reply(reply: &[u8], sequence: u32) -> Reply {
    let word = |at: usize| {
        reply
            .get(at..at + 4)
            .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
    };
    let kind = reply.get(4..6).map(|b| u16::from_ne_bytes([b[0], b[1]]));
    if word(8) != Some(sequence) {
        return Reply::Other;
    }
    match kind {
        Some(SOCK_DIAG_BY_FAMILY) => {
            word(INODE_AT).map_or(Reply::Absent, |inode| Reply::Inode(u64::from(inode)))
        }
        Some(NLMSG_ERROR | NLMSG_DONE) | None => Reply::Absent,
        Some(_) => Reply::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream, UdpSocket};

    #[test]
    fn reads_the_inode_of_a_socket_link() {
        assert_eq!(socket_inode("socket:[123456]"), Some(123456));
        assert_eq!(socket_inode("pipe:[123456]"), None);
        assert_eq!(socket_inode("/dev/null"), None);
    }

    #[test]
    fn reads_the_start_time_after_a_command_name_with_spaces() {
        let stat =
            "42 (Web Content (x)) S 1 42 42 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 3 0 987654 1 2";

        assert_eq!(parse_start_time(stat), Some(987654));
    }

    #[test]
    fn finds_this_process_behind_its_own_tcp_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();

        let owner = resolver.socket_owner(&FlowKey {
            transport: Transport::Tcp,
            local: client.local_addr().unwrap(),
            remote: client.peer_addr().unwrap(),
        });

        assert_eq!(owner, Some(std::process::id()));
    }

    #[test]
    fn finds_this_process_behind_its_own_udp_sockets() {
        let connected = UdpSocket::bind("127.0.0.1:0").unwrap();
        connected.connect("127.0.0.1:9").unwrap();
        let unconnected = UdpSocket::bind("0.0.0.0:0").unwrap();
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();

        let to_connected = resolver.socket_owner(&FlowKey {
            transport: Transport::Udp,
            local: connected.local_addr().unwrap(),
            remote: connected.peer_addr().unwrap(),
        });
        let from_unconnected = resolver.socket_owner(&FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new(
                [10, 64, 0, 2].into(),
                unconnected.local_addr().unwrap().port(),
            ),
            remote: "198.51.100.9:53".parse().unwrap(),
        });

        assert_eq!(to_connected, Some(std::process::id()));
        assert_eq!(from_unconnected, Some(std::process::id()));
    }

    #[test]
    fn finds_this_process_behind_dual_stack_sockets() {
        let Ok(listener) = TcpListener::bind("[::]:0") else {
            return;
        };
        let port = listener.local_addr().unwrap().port();
        let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let (_accepted, _) = listener.accept().unwrap();
        let udp = UdpSocket::bind("[::]:0").unwrap();
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();

        let accepted_side = resolver.socket_owner(&FlowKey {
            transport: Transport::Tcp,
            local: SocketAddr::new([127, 0, 0, 1].into(), port),
            remote: client.local_addr().unwrap(),
        });
        let udp_side = resolver.socket_owner(&FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new([10, 64, 0, 2].into(), udp.local_addr().unwrap().port()),
            remote: "198.51.100.9:53".parse().unwrap(),
        });

        assert_eq!(accepted_side, Some(std::process::id()));
        assert_eq!(udp_side, Some(std::process::id()));
    }

    #[test]
    fn reads_the_executable_and_a_stable_key_of_this_process() {
        let mut resolver = SystemResolver::new();
        let pid = std::process::id();

        let executable = resolver.executable(pid);
        let first = resolver.process_key(pid);

        assert_eq!(
            executable.map(|path| std::fs::canonicalize(path).unwrap()),
            Some(std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap())
        );
        assert_eq!(first.map(|key| key.pid), Some(pid));
        assert_eq!(first, resolver.process_key(pid));
    }

    #[test]
    fn a_process_that_execs_another_program_gets_another_key() {
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "read line; exec sleep 5"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut resolver = SystemResolver::new();
        let pid = child.id();
        let shell = resolver.process_key(pid).unwrap();

        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(b"go\n").unwrap();
        let sleeping = (0..200).find_map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let path = resolver.executable(pid)?;
            (path.ends_with("sleep"))
                .then(|| resolver.process_key(pid))
                .flatten()
        });
        child.kill().unwrap();
        child.wait().unwrap();

        let sleeping = sleeping.expect("the shell execs sleep");
        assert_eq!(
            (sleeping.pid, sleeping.start_time),
            (shell.pid, shell.start_time)
        );
        assert_ne!(sleeping.image, shell.image);
    }

    #[test]
    fn a_socket_handed_to_another_process_is_attributed_to_it() {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        let flow = FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new([10, 64, 0, 2].into(), socket.local_addr().unwrap().port()),
            remote: "198.51.100.9:53".parse().unwrap(),
        };
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();
        assert_eq!(resolver.socket_owner(&flow), Some(std::process::id()));
        let mut command = std::process::Command::new("sleep");
        command
            .arg("5")
            .stdin(std::process::Stdio::from(OwnedFd::from(socket)));
        let mut child = command.spawn().unwrap();
        drop(command);

        resolver.refresh().unwrap();
        let owner = resolver.socket_owner(&flow);
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(owner, Some(child.id()));
    }

    #[test]
    fn a_socket_opened_after_the_index_was_built_is_found_after_a_refresh() {
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();
        let early = UdpSocket::bind("0.0.0.0:0").unwrap();
        let early_flow = FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new([10, 64, 0, 2].into(), early.local_addr().unwrap().port()),
            remote: "198.51.100.9:53".parse().unwrap(),
        };
        assert_eq!(resolver.socket_owner(&early_flow), Some(std::process::id()));
        let late = UdpSocket::bind("0.0.0.0:0").unwrap();
        let late_flow = FlowKey {
            local: SocketAddr::new([10, 64, 0, 2].into(), late.local_addr().unwrap().port()),
            ..early_flow
        };

        let before_refresh = resolver.socket_owner(&late_flow);
        resolver.refresh().unwrap();
        let after_refresh = resolver.socket_owner(&late_flow);

        assert_eq!(before_refresh, None);
        assert_eq!(after_refresh, Some(std::process::id()));
    }
}
