//! The Linux resolver: an exact `NETLINK_SOCK_DIAG` lookup for the socket of
//! one flow, which is live, then its inode's owner through an index of
//! `/proc/<pid>/fd`. An index entry is checked against that process's own
//! descriptors before it is trusted, and a miss rebuilds the index at most
//! once per refresh.
//!
//! Reading every process's descriptors costs milliseconds per new flow on a
//! busy host, on the packet path. So once the router names the programs it
//! routes, [`OwnerResolver::owner`] searches only the processes running one of
//! them, and reports a socket none of them holds as unwatched. Those processes
//! are followed through the proc connector, whose fork, exec and exit events
//! are queued before the process they name can open a socket. Every process's
//! program is read again when events may have been lost (a full queue, a gap
//! in a CPU's event numbers, a broken socket), when there are no events (no
//! proc connector, or not for this process), and at least every
//! [`FULL_SCAN_PERIOD`], for what no event reports. [`OwnerResolver::socket_owner`]
//! still searches every process.

use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    io,
    net::{IpAddr, SocketAddr},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::PathBuf,
    time::{Duration, Instant},
};

use super::{OwnerError, OwnerResolver, SocketOwner, conntrack, proc_events};
use crate::{
    app::{AppMatcher, ProcessKey},
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
/// The longest a watched set may go without every process being read, for
/// the changes no event reports: a lost event no gap reveals, or a program
/// swapped in without an exec (`PR_SET_MM_EXE_FILE`, which CRIU uses).
const FULL_SCAN_PERIOD: Duration = Duration::from_secs(1);
/// How long to go without events after the kernel refused them.
const REOPEN_AFTER: Duration = Duration::from_secs(30);
/// At most this many event datagrams are read per refresh, so a process
/// storm cannot hold the packet path; the rest counts as lost.
const DRAIN_CAP: usize = 4096;

/// Looks each flow up in the kernel, and pids up in an inode index.
#[derive(Default)]
pub struct SystemResolver {
    socket: Option<OwnedFd>,
    /// The conntrack socket, and whether the kernel refused to answer on it
    /// once already (no privilege, no conntrack), which asking again would
    /// not change.
    conntrack: Option<OwnedFd>,
    conntrack_refused: bool,
    /// Sockets to the processes holding them, among every process.
    everyone: Index,
    /// Sockets to the processes holding them, among the watched ones.
    among_watched: Index,
    sequence: u32,
    reply: Vec<u8>,
    /// The programs whose processes [`OwnerResolver::owner`] searches; `None`
    /// searches every process.
    programs: Option<AppMatcher<()>>,
    /// The processes running one of `programs`.
    watched: HashSet<u32>,
    process_events: ProcessEvents,
    sequences: proc_events::Sequences,
    /// Set when `watched` has to be read from every process again.
    rescan: bool,
    /// When every process's program was last read.
    scanned: Option<Instant>,
    messages: Vec<proc_events::Message>,
}

#[derive(Default)]
struct Index {
    inodes: HashMap<u64, u32>,
    /// Set by a refresh, spent by the first rebuild after it.
    may_rebuild: bool,
}

/// Where the resolver hears of processes that start, exec or exit.
#[derive(Default)]
enum ProcessEvents {
    /// Not opened yet, or lost: the next refresh opens it.
    #[default]
    Closed,
    Open(EventSocket),
    /// The kernel refused to send events: every refresh reads every process
    /// until it is asked again.
    Unavailable {
        ask_again: Instant,
    },
}

/// A socket the kernel acknowledged as listening to process events.
struct EventSocket(OwnedFd);

impl Drop for EventSocket {
    fn drop(&mut self) {
        let _ = send(self.0.as_raw_fd(), &proc_events::ignore_request(0));
    }
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

    /// The pair the socket of `flow` holds, when the firewall translated the
    /// connection on its way to the TUN device.
    fn original(&mut self, flow: &FlowKey) -> Option<FlowKey> {
        if self.conntrack_refused {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let request = conntrack::request(self.sequence, flow)?;
        let fd = match &self.conntrack {
            Some(socket) => socket.as_raw_fd(),
            None => {
                let Ok(socket) = open_netlink(libc::NETLINK_NETFILTER) else {
                    // A kernel without conntrack over netlink stays without it.
                    self.conntrack_refused = true;
                    return None;
                };
                let fd = socket.as_raw_fd();
                self.conntrack = Some(socket);
                fd
            }
        };
        if send(fd, &request) < 0 {
            self.conntrack = None;
            return None;
        }
        self.reply.resize(8192, 0);
        loop {
            let received = receive(fd, &mut self.reply)?;
            match conntrack::parse_reply(&self.reply[..received], self.sequence, flow) {
                conntrack::Reply::Original(original) => return Some(original),
                conntrack::Reply::AsSeen => return None,
                conntrack::Reply::Refused => {
                    self.conntrack_refused = true;
                    return None;
                }
                conntrack::Reply::Other => continue,
            }
        }
    }

    fn query(&mut self, protocol: u8, src: SocketAddr, dst: SocketAddr) -> Option<u64> {
        let fd = self.socket().ok()?;
        self.sequence = self.sequence.wrapping_add(1);
        let request = request(self.sequence, protocol, src, dst);
        let sent = send(fd, &request);
        if sent < 0 {
            self.socket = None;
            return None;
        }
        self.reply.resize(8192, 0);
        loop {
            let received = receive(fd, &mut self.reply)?;
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
        let socket = open_netlink(libc::NETLINK_SOCK_DIAG)?;
        let fd = socket.as_raw_fd();
        self.socket = Some(socket);
        Ok(fd)
    }
}

impl OwnerResolver for SystemResolver {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        let inode = self.live_inode(flow)?;
        self.holder(inode, Among::Everyone)
    }

    fn owner(&mut self, flow: &FlowKey) -> SocketOwner {
        let Some(inode) = self.live_inode(flow) else {
            return SocketOwner::Unknown;
        };
        if self.programs.is_none() {
            return self
                .holder(inode, Among::Everyone)
                .map_or(SocketOwner::Unknown, SocketOwner::Process);
        }
        if self.watched.is_empty() {
            return SocketOwner::Unwatched;
        }
        // A miss is final: the index is read after this refresh, from every
        // watched process.
        self.holder(inode, Among::Watched)
            .map_or(SocketOwner::Unwatched, SocketOwner::Process)
    }

    fn watch_programs<V: Copy>(&mut self, programs: &AppMatcher<V>) {
        self.programs = Some(programs.apps_only());
        self.among_watched = Index::default();
        self.watched.clear();
        self.rescan = true;
        if programs.is_empty() {
            // Events nobody reads would only fill the queue.
            if matches!(self.process_events, ProcessEvents::Open(_)) {
                self.process_events = ProcessEvents::Closed;
            }
            return;
        }
        // An answer about the new programs may come before the next refresh,
        // and must not come from the processes the old ones ran in.
        self.follow_processes();
        self.among_watched.may_rebuild = true;
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        self.everyone.may_rebuild = true;
        self.among_watched.may_rebuild = true;
        if self
            .programs
            .as_ref()
            .is_some_and(|programs| !programs.is_empty())
        {
            self.follow_processes();
        }
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

#[derive(Clone, Copy)]
enum Among {
    Everyone,
    Watched,
}

/// What a drain of the event socket says about the events and the socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drained {
    /// Every event since the last drain was read.
    All,
    /// Some were dropped; the socket still works.
    Lost,
    /// The socket no longer works.
    Broken,
}

fn drained(result: &io::Result<()>) -> Drained {
    match result {
        Ok(()) => Drained::All,
        Err(error) if error.raw_os_error() == Some(libc::ENOBUFS) => Drained::Lost,
        Err(_) => Drained::Broken,
    }
}

impl SystemResolver {
    /// The inode of the socket `flow` belongs to, when a process may hold
    /// it. A socket no process holds any more (closing, or in TIME_WAIT)
    /// has inode 0.
    fn live_inode(&mut self, flow: &FlowKey) -> Option<u64> {
        // Include-only masquerades an included connection into the tunnel,
        // so its socket is found by the pair it had before.
        let flow = self.original(flow).unwrap_or(*flow);
        // No socket for this flow is a final answer: the lookup is live.
        self.inode(&flow).filter(|inode| *inode != 0)
    }

    /// The process holding the socket `inode`, among `among`.
    fn holder(&mut self, inode: u64, among: Among) -> Option<u32> {
        let index = match among {
            Among::Everyone => &self.everyone,
            Among::Watched => &self.among_watched,
        };
        if let Some(pid) = index.inodes.get(&inode).copied()
            && holds_socket(pid, inode)
        {
            return Some(pid);
        }
        if !index.may_rebuild {
            return None;
        }
        let searched: Vec<u32> = match among {
            Among::Everyone => processes().collect(),
            Among::Watched => self.watched.iter().copied().collect(),
        };
        let index = match among {
            Among::Everyone => &mut self.everyone,
            Among::Watched => &mut self.among_watched,
        };
        index.may_rebuild = false;
        index.inodes.clear();
        for pid in searched {
            for inode in socket_inodes(pid) {
                index.inodes.entry(inode).or_insert(pid);
            }
        }
        index.inodes.get(&inode).copied()
    }

    /// Brings `watched` up to date with the processes running now.
    fn follow_processes(&mut self) {
        self.open_process_events();
        let mut messages = std::mem::take(&mut self.messages);
        messages.clear();
        self.reply.resize(8192, 0);
        let heard_all = match &self.process_events {
            ProcessEvents::Open(socket) => {
                let result =
                    drain_process_events(socket.0.as_raw_fd(), &mut self.reply, &mut messages);
                match drained(&result) {
                    Drained::All => true,
                    Drained::Lost => false,
                    Drained::Broken => {
                        self.process_events = ProcessEvents::Closed;
                        false
                    }
                }
            }
            ProcessEvents::Closed | ProcessEvents::Unavailable { .. } => false,
        };
        self.follow_messages(heard_all, &messages);
        self.messages = messages;
    }

    /// Applies the process events of `messages`, or reads every process
    /// when they may not tell everything: `heard_all` is false, a CPU's
    /// numbers have a gap, or a full read is due.
    fn follow_messages(&mut self, heard_all: bool, messages: &[proc_events::Message]) {
        // Every message is counted, so a gap is seen whatever else happens.
        let mut in_a_row = true;
        for message in messages {
            in_a_row &= self.sequences.continues(message);
        }
        let due = self
            .scanned
            .is_none_or(|scanned| scanned.elapsed() >= FULL_SCAN_PERIOD);
        if heard_all && in_a_row && !self.rescan && !due {
            for message in messages {
                if let proc_events::Event::Changed(pid) = message.event {
                    self.follow(pid);
                }
            }
        } else {
            self.scan_processes();
        }
    }

    /// Opens the event socket when it is closed, or when the kernel refused
    /// it long enough ago to be asked again.
    fn open_process_events(&mut self) {
        let ask = match self.process_events {
            ProcessEvents::Open(_) => false,
            ProcessEvents::Closed => true,
            ProcessEvents::Unavailable { ask_again } => Instant::now() >= ask_again,
        };
        if !ask {
            return;
        }
        self.process_events = match open_process_events() {
            Ok(socket) => ProcessEvents::Open(socket),
            Err(_) => ProcessEvents::Unavailable {
                ask_again: Instant::now() + REOPEN_AFTER,
            },
        };
        self.sequences.clear();
        // What happened before the socket listened was not heard.
        self.rescan = true;
    }

    /// Reads the program of every process.
    fn scan_processes(&mut self) {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            // Nothing read is nothing learnt: try again at the next refresh.
            self.rescan = true;
            return;
        };
        self.watched.clear();
        let pids: Vec<u32> = entries
            .flatten()
            .filter_map(|process| process.file_name().to_str()?.parse().ok())
            .collect();
        for pid in pids {
            self.follow(pid);
        }
        self.rescan = false;
        self.scanned = Some(Instant::now());
    }

    /// Watches `pid` when it runs one of the programs, and forgets it
    /// otherwise, which includes a process that has exited.
    fn follow(&mut self, pid: u32) {
        let runs_a_program = self.programs.as_ref().is_some_and(|programs| {
            std::fs::read_link(format!("/proc/{pid}/exe"))
                .is_ok_and(|executable| programs.lookup(executable.as_os_str()).is_some())
        });
        if runs_a_program {
            self.watched.insert(pid);
        } else {
            self.watched.remove(&pid);
        }
    }
}

/// The pids of every process.
fn processes() -> impl Iterator<Item = u32> {
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|process| process.file_name().to_str()?.parse().ok())
}

/// A socket subscribed to the proc connector's events. Fails when the kernel
/// has no proc connector or refuses this process its events.
fn open_process_events() -> io::Result<EventSocket> {
    let socket = open_netlink(libc::NETLINK_CONNECTOR)?;
    let fd = socket.as_raw_fd();
    // SAFETY: a zeroed sockaddr_nl with a family and a group is valid, and
    // its length is the one passed.
    let bound = unsafe {
        let mut address: libc::sockaddr_nl = std::mem::zeroed();
        address.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        address.nl_groups = proc_events::CN_IDX_PROC;
        libc::bind(
            fd,
            std::ptr::from_ref(&address).cast::<libc::sockaddr>(),
            size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if bound != 0 {
        return Err(io::Error::last_os_error());
    }
    // The larger the queue, the rarer an overflow and the full read it
    // costs. The privileged option goes past the system's limit.
    let queue: libc::c_int = 4 << 20;
    for option in [libc::SO_RCVBUFFORCE, libc::SO_RCVBUF] {
        // SAFETY: `queue` is a valid int of the length passed.
        let status = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                std::ptr::from_ref(&queue).cast::<c_void>(),
                size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if status == 0 {
            break;
        }
    }
    // The kernel answers at once or not at all; a short wait per read
    // bounds how long the packet path waits.
    let timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 50_000,
    };
    // SAFETY: `timeout` is a valid timeval of the length passed.
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            std::ptr::from_ref(&timeout).cast::<c_void>(),
            size_of::<libc::timeval>() as libc::socklen_t,
        );
    }
    // Several programs may listen: the ack tells this request's answer from
    // theirs.
    let ack = std::process::id().rotate_left(16) ^ fd as u32;
    if send(fd, &proc_events::listen_request(1, ack)) < 0 {
        return Err(io::Error::last_os_error());
    }
    // From here the kernel may count this socket as a listener, whatever
    // the wait below ends on, so dropping it unsubscribes.
    let socket = EventSocket(socket);
    let mut reply = vec![0; 8192];
    let mut messages = Vec::new();
    // Events other processes cause may come first; the whole wait is
    // bounded, since it holds the packet path.
    let deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < deadline {
        let Some(received) = receive_from_kernel(fd, &mut reply, 0)? else {
            continue;
        };
        messages.clear();
        proc_events::parse(&reply[..received], &mut messages);
        for message in &messages {
            if let proc_events::Event::Acknowledged {
                ack: answered,
                error,
            } = message.event
                && answered == ack.wrapping_add(1)
            {
                return match error {
                    0 => Ok(socket),
                    error => Err(io::Error::from_raw_os_error(error as i32)),
                };
            }
        }
    }
    Err(io::Error::from(io::ErrorKind::TimedOut))
}

/// Appends the messages queued on `fd` to `messages`, without waiting.
///
/// # Errors
///
/// `ENOBUFS` when events were dropped: the kernel reported its queue full,
/// or more than [`DRAIN_CAP`] datagrams were waiting. After a reported
/// overflow the queue is emptied up to the cap, since the kernel reports the
/// next one only once a read has found it empty. Any other error is the
/// socket's.
fn drain_process_events(
    fd: i32,
    reply: &mut [u8],
    messages: &mut Vec<proc_events::Message>,
) -> io::Result<()> {
    drain_up_to(fd, reply, messages, DRAIN_CAP)
}

fn drain_up_to(
    fd: i32,
    reply: &mut [u8],
    messages: &mut Vec<proc_events::Message>,
    cap: usize,
) -> io::Result<()> {
    let mut overflowed = false;
    let mut read = 0;
    loop {
        // Every datagram counts, overflow or not, so a queue that never
        // empties cannot hold the packet path; what is left is a gap the
        // next drain sees in the numbers.
        if read >= cap {
            return Err(io::Error::from_raw_os_error(libc::ENOBUFS));
        }
        match receive_from_kernel(fd, reply, libc::MSG_DONTWAIT) {
            Ok(Some(received)) => {
                read += 1;
                if !overflowed {
                    proc_events::parse(&reply[..received], messages);
                }
            }
            // A datagram some other sender slipped in.
            Ok(None) => read += 1,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return if overflowed {
                    Err(io::Error::from_raw_os_error(libc::ENOBUFS))
                } else {
                    Ok(())
                };
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.raw_os_error() == Some(libc::ENOBUFS) => overflowed = true,
            Err(error) => return Err(error),
        }
    }
}

/// The length of the next datagram on `fd`, read into `buffer`; `None` for
/// one that did not come from the kernel, which only root could send.
fn receive_from_kernel(fd: i32, buffer: &mut [u8], flags: i32) -> io::Result<Option<usize>> {
    // SAFETY: `buffer` is writable for the length passed, and `sender` is a
    // sockaddr_nl of the length passed.
    let (received, sender) = unsafe {
        let mut sender: libc::sockaddr_nl = std::mem::zeroed();
        let mut sender_len = size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        let received = libc::recvfrom(
            fd,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len(),
            flags,
            std::ptr::from_mut(&mut sender).cast::<libc::sockaddr>(),
            std::ptr::from_mut(&mut sender_len),
        );
        (received, sender)
    };
    let received = usize::try_from(received).map_err(|_| io::Error::last_os_error())?;
    Ok((sender.nl_pid == 0).then_some(received))
}

/// A netlink socket of `protocol` whose replies time out, since a reply that
/// never comes must not stall the packet path.
fn open_netlink(protocol: i32) -> io::Result<OwnedFd> {
    // SAFETY: plain socket creation; the result is checked.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            protocol,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` is a fresh descriptor this function now owns.
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
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
    Ok(socket)
}

/// Sends `request` to the kernel.
fn send(fd: i32, request: &[u8]) -> isize {
    // SAFETY: `request` is a readable buffer of the length passed, and a
    // zeroed sockaddr_nl addresses the kernel.
    unsafe {
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
    }
}

/// The length of the next reply, read into `reply`; `None` on a timeout.
fn receive(fd: i32, reply: &mut [u8]) -> Option<usize> {
    // SAFETY: `reply` is writable for the length passed.
    let received = unsafe { libc::recv(fd, reply.as_mut_ptr().cast::<c_void>(), reply.len(), 0) };
    usize::try_from(received).ok()
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
    use crate::app::PathFlavor;
    use std::{
        net::{TcpListener, TcpStream, UdpSocket},
        path::Path,
    };

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

    /// Runs `command` in the network namespace of the calling thread.
    fn run(command: &str) {
        let status = std::process::Command::new("/bin/sh")
            .args(["-c", command])
            .status()
            .unwrap();
        assert!(status.success(), "{command}");
    }

    /// Moves the calling thread to a network namespace of its own where
    /// connections to 127.0.0.2 are translated from 127.0.0.3, the way
    /// include-only masquerades an included connection into the tunnel.
    /// Conntrack then tracks every connection of the namespace.
    fn enter_translating_namespace() {
        // SAFETY: plain syscall; the result is checked.
        assert_eq!(unsafe { libc::unshare(libc::CLONE_NEWNET) }, 0);
        run("ip link set lo up && ip addr add 127.0.0.3/8 dev lo");
        run(
            "nft add table ip include_test && nft add chain ip include_test post \
             '{ type nat hook postrouting priority 100; }' && \
             nft add rule ip include_test post ip daddr 127.0.0.2 snat to 127.0.0.3",
        );
    }

    fn inode_of(socket: &impl AsRawFd) -> u64 {
        std::fs::metadata(format!("/proc/self/fd/{}", socket.as_raw_fd()))
            .unwrap()
            .ino()
    }

    #[test]
    #[ignore = "needs root and nft; runs in a network namespace of its own"]
    fn finds_a_connection_masqueraded_on_its_way_to_the_tunnel() {
        // A thread of its own, since the namespace it enters is the thread's.
        std::thread::spawn(|| {
            enter_translating_namespace();
            let listener = TcpListener::bind("127.0.0.2:0").unwrap();
            let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (accepted, seen_from) = listener.accept().unwrap();
            assert_ne!(seen_from.ip(), client.local_addr().unwrap().ip());
            let mut resolver = SystemResolver::new();
            resolver.refresh().unwrap();

            let owner = resolver.socket_owner(&FlowKey {
                transport: Transport::Tcp,
                local: seen_from,
                remote: accepted.local_addr().unwrap(),
            });

            assert_eq!(owner, Some(std::process::id()));
        })
        .join()
        .unwrap();
    }

    #[test]
    #[ignore = "needs root and nft; runs in a network namespace of its own"]
    fn finds_the_accepted_socket_of_a_connection_the_far_end_opened() {
        std::thread::spawn(|| {
            enter_translating_namespace();
            let listener = TcpListener::bind("127.0.0.4:0").unwrap();
            let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (accepted, peer) = listener.accept().unwrap();
            let flow = FlowKey {
                transport: Transport::Tcp,
                local: accepted.local_addr().unwrap(),
                remote: peer,
            };
            let mut resolver = SystemResolver::new();

            // Both ends are this process's, so only the socket tells them apart.
            let socket_flow = resolver.original(&flow).unwrap_or(flow);

            assert_eq!(resolver.inode(&socket_flow), Some(inode_of(&accepted)));
        })
        .join()
        .unwrap();
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

    /// A matcher for `programs`, the way the router hands its routed apps.
    fn programs(programs: &[&Path]) -> AppMatcher<()> {
        AppMatcher::new(PathFlavor::Linux, programs.iter().map(|path| (*path, ())))
    }

    fn tcp_flow_of(client: &TcpStream) -> FlowKey {
        FlowKey {
            transport: Transport::Tcp,
            local: client.local_addr().unwrap(),
            remote: client.peer_addr().unwrap(),
        }
    }

    /// A connection whose client end this process holds.
    fn connection() -> (TcpListener, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        (listener, client)
    }

    /// `program` as the kernel names the process running it.
    fn resolved(program: &str) -> PathBuf {
        std::fs::canonicalize(program).unwrap()
    }

    /// `sleep` under a name of this test's own. Another test's child runs
    /// the program it execs while it still holds this process's sockets, so
    /// a program other tests start could be found holding them. A hard link
    /// keeps the name the process is known by, and holds no file open.
    struct PrivateSleep {
        path: PathBuf,
    }

    impl PrivateSleep {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::current_exe()
                .unwrap()
                .with_file_name(format!("app-routing-sleep-{}-{unique}", std::process::id()));
            let _ = std::fs::remove_file(&path);
            std::fs::hard_link(resolved("/bin/sleep"), &path)
                .or_else(|_| std::fs::copy(resolved("/bin/sleep"), &path).map(drop))
                .unwrap();
            Self {
                path: std::fs::canonicalize(&path).unwrap(),
            }
        }
    }

    impl Drop for PrivateSleep {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn a_socket_no_watched_program_holds_is_unwatched() {
        let (_listener, client) = connection();
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[Path::new("/nonexistent/watched")]));
        resolver.refresh().unwrap();

        assert_eq!(
            resolver.owner(&tcp_flow_of(&client)),
            SocketOwner::Unwatched
        );
    }

    #[test]
    fn a_socket_a_watched_program_holds_is_found() {
        let (_listener, client) = connection();
        let me = std::env::current_exe().unwrap();
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&me]));
        resolver.refresh().unwrap();

        let owner = resolver.owner(&tcp_flow_of(&client));

        assert!(runs(&mut resolver, owner, &me));
    }

    /// Whether `owner` is a process running `program`. Another test's child
    /// shares this process's sockets and program until it execs, so the
    /// holder found may be that child rather than this process.
    fn runs(resolver: &mut SystemResolver, owner: SocketOwner, program: &Path) -> bool {
        let SocketOwner::Process(pid) = owner else {
            return false;
        };
        resolver.executable(pid).is_some_and(|executable| {
            std::fs::canonicalize(executable).ok().as_deref()
                == std::fs::canonicalize(program).ok().as_deref()
        })
    }

    /// A resolver that reads every process at each refresh, as it does when
    /// the kernel sends it no process events.
    fn resolver_without_process_events() -> SystemResolver {
        SystemResolver {
            process_events: ProcessEvents::Unavailable {
                ask_again: Instant::now() + std::time::Duration::from_secs(3600),
            },
            ..SystemResolver::default()
        }
    }

    #[test]
    fn a_watched_program_is_searched_from_the_moment_it_is_named() {
        let (_listener, client) = connection();
        let me = std::env::current_exe().unwrap();
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();

        resolver.watch_programs(&programs(&[&me]));
        let owner = resolver.owner(&tcp_flow_of(&client));

        assert!(runs(&mut resolver, owner, &me));
    }

    #[test]
    fn with_no_program_to_watch_the_resolver_stops_following_processes() {
        let me = std::env::current_exe().unwrap();
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&me]));
        resolver.refresh().unwrap();

        resolver.watch_programs(&programs(&[]));
        resolver.refresh().unwrap();

        assert!(resolver.watched.is_empty());
        assert!(matches!(resolver.process_events, ProcessEvents::Closed));
    }

    #[test]
    fn a_closed_connection_whose_socket_nobody_holds_has_no_owner() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        let flow = tcp_flow_of(&client);
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[Path::new("/nonexistent/watched")]));
        // The client closes first, so its end waits in TIME_WAIT, held by no
        // process.
        drop(client);
        std::thread::sleep(std::time::Duration::from_millis(20));
        drop(accepted);
        std::thread::sleep(std::time::Duration::from_millis(20));
        resolver.refresh().unwrap();

        assert_eq!(resolver.inode(&flow), Some(0));
        assert_eq!(resolver.owner(&flow), SocketOwner::Unknown);
    }

    #[test]
    fn an_overflow_loses_events_and_any_other_error_breaks_the_socket() {
        assert_eq!(drained(&Ok(())), Drained::All);
        assert_eq!(
            drained(&Err(io::Error::from_raw_os_error(libc::ENOBUFS))),
            Drained::Lost
        );
        assert_eq!(
            drained(&Err(io::Error::from_raw_os_error(libc::EBADF))),
            Drained::Broken
        );
    }

    /// Waits until `pid` runs `program` and has entered it: its exec event,
    /// queued before the program runs, is then there to be read.
    fn wait_until_running(pid: u32, program: &Path) {
        let running = (0..400).any(|_| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let sleeping = stat
                .rsplit_once(')')
                .is_some_and(|(_, fields)| fields.trim_start().starts_with('S'));
            let runs =
                std::fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|exe| exe == program);
            if !(sleeping && runs) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            sleeping && runs
        });
        assert!(running, "the child runs its program");
    }

    /// A socket that is never readable, standing in for an event socket that
    /// silently receives nothing.
    fn silent_event_socket() -> ProcessEvents {
        ProcessEvents::Open(EventSocket(OwnedFd::from(
            UdpSocket::bind("127.0.0.1:0").unwrap(),
        )))
    }

    fn message(cpu: u32, sequence: u32) -> proc_events::Message {
        proc_events::Message {
            cpu,
            sequence,
            event: proc_events::Event::Other,
        }
    }

    #[test]
    fn a_drain_stops_at_its_cap_and_counts_what_is_left_as_lost() {
        let queue = UdpSocket::bind("127.0.0.1:0").unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        for _ in 0..5 {
            sender.send_to(b"x", queue.local_addr().unwrap()).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut reply = vec![0; 8192];
        let mut messages = Vec::new();

        let capped = drain_up_to(queue.as_raw_fd(), &mut reply, &mut messages, 3);
        let rest = drain_up_to(queue.as_raw_fd(), &mut reply, &mut messages, 3);

        assert_eq!(drained(&capped), Drained::Lost);
        assert_eq!(drained(&rest), Drained::All);
    }

    #[test]
    fn a_gap_in_the_event_numbers_reads_every_process_again() {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&sleep]));
        resolver.process_events = silent_event_socket();
        resolver.follow_messages(true, &[message(0, 5)]);
        let mut child = std::process::Command::new(&sleep).arg("5").spawn().unwrap();
        wait_until_running(child.id(), &sleep);

        resolver.follow_messages(true, &[message(0, 6)]);
        let without_a_gap = resolver.watched.contains(&child.id());
        resolver.follow_messages(true, &[message(0, 8)]);
        let after_a_gap = resolver.watched.contains(&child.id());
        child.kill().unwrap();
        child.wait().unwrap();

        assert!(!without_a_gap);
        assert!(after_a_gap);
    }

    #[test]
    fn a_process_no_event_announced_is_found_by_the_periodic_full_read() {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&sleep]));
        resolver.process_events = silent_event_socket();
        resolver.refresh().unwrap();
        let mut child = std::process::Command::new(&sleep)
            .arg("5")
            .stdout(std::process::Stdio::from(OwnedFd::from(client)))
            .spawn()
            .unwrap();
        wait_until_running(child.id(), &sleep);

        resolver.scanned = Some(Instant::now());
        resolver.refresh().unwrap();
        let before_the_period = resolver.owner(&flow);
        resolver.scanned = Instant::now().checked_sub(FULL_SCAN_PERIOD);
        resolver.refresh().unwrap();
        let after_the_period = resolver.owner(&flow);
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(before_the_period, SocketOwner::Unwatched);
        assert_eq!(after_the_period, SocketOwner::Process(child.id()));
    }

    #[test]
    fn a_refused_event_socket_is_asked_for_again_once_its_wait_is_over() {
        let me = std::env::current_exe().unwrap();
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&me]));
        let refused_at = Instant::now() - REOPEN_AFTER;
        resolver.process_events = ProcessEvents::Unavailable {
            ask_again: refused_at,
        };

        resolver.refresh().unwrap();

        // Either the kernel answers now, or it refused again, later.
        assert!(match resolver.process_events {
            ProcessEvents::Open(_) => true,
            ProcessEvents::Unavailable { ask_again } => ask_again > refused_at,
            ProcessEvents::Closed => false,
        });
    }

    #[test]
    fn the_full_search_finds_the_holder_the_watched_search_does_not_cover() {
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[Path::new("/nonexistent/watched")]));
        resolver.refresh().unwrap();

        assert_eq!(resolver.owner(&flow), SocketOwner::Unwatched);
        assert_eq!(resolver.socket_owner(&flow), Some(std::process::id()));
    }

    #[test]
    fn a_broken_event_socket_is_reopened_and_its_missed_processes_found() {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&sleep]));
        resolver.refresh().unwrap();
        // A listening socket cannot be read from: it stands in for an event
        // socket that broke.
        let broken = TcpListener::bind("127.0.0.1:0").unwrap();
        resolver.process_events = ProcessEvents::Open(EventSocket(OwnedFd::from(broken)));

        let mut child = std::process::Command::new(&sleep)
            .arg("5")
            .stdout(std::process::Stdio::from(OwnedFd::from(client)))
            .spawn()
            .unwrap();
        wait_until_running(child.id(), &sleep);
        resolver.refresh().unwrap();
        let owner = resolver.owner(&flow);
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(owner, SocketOwner::Process(child.id()));
        assert!(matches!(resolver.process_events, ProcessEvents::Closed));
    }

    #[test]
    fn a_process_that_execs_a_watched_program_is_found_from_the_next_refresh() {
        finds_a_process_that_execs_a_watched_program(SystemResolver::new(), true);
    }

    #[test]
    fn without_process_events_a_process_that_execs_a_watched_program_is_found() {
        finds_a_process_that_execs_a_watched_program(resolver_without_process_events(), false);
    }

    /// Whether the resolver follows processes the way the test means to
    /// exercise; a host whose kernel sends no events to this process (a
    /// container's own network namespace, a kernel before 6.6 without root)
    /// only runs the variant without them.
    fn follows_as_intended(resolver: &SystemResolver, through_events: bool) -> bool {
        let open = matches!(resolver.process_events, ProcessEvents::Open(_));
        if through_events && !open {
            eprintln!("skipped: this host sends no process events to this process");
        }
        open == through_events
    }

    fn finds_a_process_that_execs_a_watched_program(
        mut resolver: SystemResolver,
        through_events: bool,
    ) {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        resolver.watch_programs(&programs(&[&sleep]));
        resolver.refresh().unwrap();
        if !follows_as_intended(&resolver, through_events) {
            return;
        }
        // The shell holds the socket as its stdout, then becomes `sleep`.
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("read line; exec {} 5", sleep.display())])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::from(OwnedFd::from(client)))
            .spawn()
            .unwrap();
        resolver.refresh().unwrap();
        let before_exec = resolver.owner(&flow);

        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(b"go\n").unwrap();
        wait_until_running(child.id(), &sleep);
        resolver.refresh().unwrap();
        let after_exec = resolver.owner(&flow);
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(before_exec, SocketOwner::Unwatched);
        assert_eq!(after_exec, SocketOwner::Process(child.id()));
    }

    #[test]
    fn a_watched_program_started_after_the_watch_is_found() {
        finds_a_watched_program_started_after_the_watch(SystemResolver::new(), true);
    }

    #[test]
    fn without_process_events_a_watched_program_started_after_the_watch_is_found() {
        finds_a_watched_program_started_after_the_watch(resolver_without_process_events(), false);
    }

    fn finds_a_watched_program_started_after_the_watch(
        mut resolver: SystemResolver,
        through_events: bool,
    ) {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        resolver.watch_programs(&programs(&[&sleep]));
        resolver.refresh().unwrap();
        if !follows_as_intended(&resolver, through_events) {
            return;
        }
        assert_eq!(resolver.owner(&flow), SocketOwner::Unwatched);

        let mut child = std::process::Command::new(&sleep)
            .arg("5")
            .stdout(std::process::Stdio::from(OwnedFd::from(client)))
            .spawn()
            .unwrap();
        wait_until_running(child.id(), &sleep);
        resolver.refresh().unwrap();
        let owner = resolver.owner(&flow);
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(owner, SocketOwner::Process(child.id()));
    }

    #[test]
    fn a_watched_process_that_exits_stops_being_searched() {
        let private = PrivateSleep::new();
        let sleep = private.path.clone();
        let (_listener, client) = connection();
        let flow = tcp_flow_of(&client);
        let mut resolver = SystemResolver::new();
        resolver.watch_programs(&programs(&[&sleep]));
        let mut child = std::process::Command::new(&sleep).arg("5").spawn().unwrap();
        wait_until_running(child.id(), &sleep);
        resolver.refresh().unwrap();
        let watched = resolver.watched.contains(&child.id());

        child.kill().unwrap();
        child.wait().unwrap();
        resolver.refresh().unwrap();

        assert!(watched);
        assert!(!resolver.watched.contains(&child.id()));
        assert_eq!(resolver.owner(&flow), SocketOwner::Unwatched);
    }

    #[test]
    #[ignore = "needs a kernel with the proc connector, which kernels before 6.6 open to root only"]
    fn hears_a_started_process_through_the_proc_connector() {
        let socket = open_process_events().expect("the proc connector answers");
        let mut child = std::process::Command::new("/bin/true").spawn().unwrap();
        child.wait().unwrap();
        let mut reply = vec![0; 8192];
        let mut messages = Vec::new();

        let heard = drain_process_events(socket.0.as_raw_fd(), &mut reply, &mut messages);

        assert!(heard.is_ok());
        assert!(
            messages
                .iter()
                .any(|message| message.event == proc_events::Event::Changed(child.id()))
        );
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
