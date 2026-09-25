//! The macOS resolver: the kernel's PCB exports, read unprivileged through
//! `sysctl`, and libproc for the process side.

use std::{
    ffi::{CStr, OsStr, c_int, c_void},
    io,
    mem::MaybeUninit,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    ptr,
};

use super::{OwnerError, OwnerResolver, SocketTable, pcblist};
use crate::flow::{FlowKey, Transport};

const EXPORTS: [(&CStr, Transport); 2] = [
    (c"net.inet.tcp.pcblist_n", Transport::Tcp),
    (c"net.inet.udp.pcblist_n", Transport::Udp),
];

/// Reads the whole TCP and UDP PCB tables on each refresh.
#[derive(Default)]
pub struct SystemResolver {
    table: SocketTable,
    buffer: Vec<u8>,
}

impl SystemResolver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl OwnerResolver for SystemResolver {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        self.table.owner(flow)
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        self.table.clear();
        let result = EXPORTS.iter().try_for_each(|(name, transport)| {
            let len = read_sysctl(name, &mut self.buffer).map_err(OwnerError::SocketTable)?;
            pcblist::parse(&self.buffer[..len], *transport, &mut self.table)
        });
        if result.is_err() {
            self.table.clear();
        }
        self.table.finish();
        result
    }

    fn start_time(&mut self, pid: u32) -> Option<u64> {
        let pid = c_int::try_from(pid).ok()?;
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = c_int::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
        // SAFETY: `info` is writable for `size` bytes, the size passed.
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast::<c_void>(),
                size,
            )
        };
        if written != size {
            return None;
        }
        // SAFETY: the kernel filled the whole struct, and it is plain data.
        let info = unsafe { info.assume_init() };
        Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }

    fn executable(&mut self, pid: u32) -> Option<PathBuf> {
        let pid = c_int::try_from(pid).ok()?;
        let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: `path` is writable for the length passed.
        let len = unsafe {
            libc::proc_pidpath(pid, path.as_mut_ptr().cast::<c_void>(), path.len() as u32)
        };
        let len = usize::try_from(len).ok().filter(|len| *len > 0)?;
        Some(PathBuf::from(OsStr::from_bytes(&path[..len])))
    }
}

/// Reads a sysctl into `buffer`, growing it as needed, and returns the length
/// read. The table can grow between the size query and the read, hence the
/// slack and the retry.
fn read_sysctl(name: &CStr, buffer: &mut Vec<u8>) -> io::Result<usize> {
    for _ in 0..4 {
        let mut needed = 0usize;
        // SAFETY: a null buffer asks for the size only, written to `needed`.
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                ptr::null_mut(),
                &raw mut needed,
                ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        let wanted = needed + needed / 4 + 4096;
        if buffer.len() < wanted {
            buffer.resize(wanted, 0);
        }
        let mut len = buffer.len();
        // SAFETY: `buffer` is writable for `len` bytes, the length passed.
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                buffer.as_mut_ptr().cast::<c_void>(),
                &raw mut len,
                ptr::null_mut(),
                0,
            )
        };
        if status == 0 {
            return Ok(len);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ENOMEM) {
            return Err(error);
        }
    }
    Err(io::Error::from_raw_os_error(libc::ENOMEM))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
        time::Instant,
    };

    fn own_executable() -> PathBuf {
        std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
    }

    fn resolve(resolver: &mut SystemResolver, flow: FlowKey) -> Option<u32> {
        resolver.refresh().unwrap();
        resolver.socket_owner(&flow)
    }

    #[test]
    fn finds_this_process_behind_its_own_tcp_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut resolver = SystemResolver::new();

        let owner = resolve(
            &mut resolver,
            FlowKey {
                transport: Transport::Tcp,
                local: client.local_addr().unwrap(),
                remote: client.peer_addr().unwrap(),
            },
        );

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
    fn finds_this_process_behind_an_ipv6_connection() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return;
        };
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut resolver = SystemResolver::new();

        let owner = resolve(
            &mut resolver,
            FlowKey {
                transport: Transport::Tcp,
                local: client.local_addr().unwrap(),
                remote: client.peer_addr().unwrap(),
            },
        );

        assert_eq!(owner, Some(std::process::id()));
    }

    #[test]
    fn a_connection_opened_after_the_snapshot_needs_a_refresh() {
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let flow = FlowKey {
            transport: Transport::Tcp,
            local: client.local_addr().unwrap(),
            remote: client.peer_addr().unwrap(),
        };

        let stale = resolver.socket_owner(&flow);
        resolver.refresh().unwrap();
        let fresh = resolver.socket_owner(&flow);

        assert_eq!(stale, None);
        assert_eq!(fresh, Some(std::process::id()));
    }

    #[test]
    fn reads_the_executable_and_a_stable_start_time_of_a_live_process() {
        let mut resolver = SystemResolver::new();
        let pid = std::process::id();

        let executable = resolver
            .executable(pid)
            .map(|path| std::fs::canonicalize(path).unwrap());
        let first = resolver.start_time(pid);
        let second = resolver.start_time(pid);

        assert_eq!(executable, Some(own_executable()));
        assert!(first.is_some());
        assert_eq!(first, second);
    }

    #[test]
    fn a_start_time_is_microseconds_since_the_epoch() {
        let mut resolver = SystemResolver::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .unwrap();

        let own = resolver.start_time(std::process::id()).unwrap();
        let later = resolver.start_time(child.id()).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();

        assert!(own <= now && now - own < 3_600_000_000, "{own} vs {now}");
        assert!(later > own);
    }

    #[test]
    fn a_process_that_does_not_exist_has_no_start_time_nor_executable() {
        let mut resolver = SystemResolver::new();
        let unused = i32::MAX as u32;

        assert_eq!(resolver.start_time(unused), None);
        assert_eq!(resolver.executable(unused), None);
    }

    /// The cost the router pays for one fresh snapshot, printed for the
    /// record: `cargo test -p talpid-app-routing -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_the_cost_of_a_snapshot() {
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();
        let rounds = 200u32;
        let start = Instant::now();
        for _ in 0..rounds {
            resolver.refresh().unwrap();
        }
        let per_snapshot = start.elapsed() / rounds;
        println!(
            "snapshot of {} sockets: {:?} per refresh over {rounds} refreshes",
            resolver.table.len(),
            per_snapshot
        );
    }
}
