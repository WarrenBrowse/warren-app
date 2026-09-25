//! The Windows resolver: the IP Helper owner-pid tables and the process
//! image name, both readable without elevation for other users' processes
//! through `PROCESS_QUERY_LIMITED_INFORMATION`.

use std::{ffi::OsString, io, os::windows::ffi::OsStringExt, path::PathBuf};

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, FILETIME, HANDLE, NO_ERROR},
    NetworkManagement::IpHelper::{
        GetExtendedTcpTable, GetExtendedUdpTable, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
    },
    Networking::WinSock::{AF_INET, AF_INET6},
    System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    },
};

use super::{
    OwnerError, OwnerResolver, SocketTable,
    win_tables::{self, TableKind},
};
use crate::flow::FlowKey;

const TABLES: [TableKind; 4] = [
    TableKind::Tcp4,
    TableKind::Tcp6,
    TableKind::Udp4,
    TableKind::Udp6,
];

/// Reads the four owner-pid tables on each refresh.
#[derive(Default)]
pub struct SystemResolver {
    table: SocketTable,
    /// `u32` words so the tables the API writes are aligned for its structs.
    buffer: Vec<u32>,
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
        let result = TABLES.iter().try_for_each(|kind| {
            let len = read_table(*kind, &mut self.buffer).map_err(OwnerError::SocketTable)?;
            // SAFETY: the buffer holds at least `len` initialized bytes, and a
            // `u32` slice is valid to read as bytes.
            let bytes =
                unsafe { std::slice::from_raw_parts(self.buffer.as_ptr().cast::<u8>(), len) };
            win_tables::parse(bytes, *kind, &mut self.table)
        });
        if result.is_err() {
            self.table.clear();
        }
        self.table.finish();
        result
    }

    fn start_time(&mut self, pid: u32) -> Option<u64> {
        let process = Process::open(pid)?;
        let mut creation = FILETIME::default();
        let mut unused = [FILETIME::default(); 3];
        let [exit, kernel, user] = &mut unused;
        // SAFETY: the handle is open with query rights and every out pointer
        // is a valid FILETIME.
        let ok = unsafe { GetProcessTimes(process.0, &raw mut creation, exit, kernel, user) };
        (ok != 0)
            .then(|| (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    fn executable(&mut self, pid: u32) -> Option<PathBuf> {
        let process = Process::open(pid)?;
        let mut name = vec![0u16; 32_768];
        let mut len = name.len() as u32;
        // SAFETY: `name` is writable for `len` UTF-16 units, the length passed.
        let ok = unsafe {
            QueryFullProcessImageNameW(
                process.0,
                PROCESS_NAME_WIN32,
                name.as_mut_ptr(),
                &raw mut len,
            )
        };
        (ok != 0).then(|| PathBuf::from(OsString::from_wide(&name[..len as usize])))
    }
}

/// A process handle, closed on drop.
struct Process(HANDLE);

impl Process {
    fn open(pid: u32) -> Option<Self> {
        // SAFETY: plain call; a null handle is the failure value.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        (!handle.is_null()).then_some(Self(handle))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `Process::open` and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Reads one owner-pid table into `buffer`, growing it as the API asks, and
/// returns its length in bytes.
fn read_table(kind: TableKind, buffer: &mut Vec<u32>) -> io::Result<usize> {
    let family = u32::from(match kind {
        TableKind::Tcp4 | TableKind::Udp4 => AF_INET,
        TableKind::Tcp6 | TableKind::Udp6 => AF_INET6,
    });
    for _ in 0..4 {
        let mut size = u32::try_from(buffer.len() * 4).unwrap_or(u32::MAX);
        let table = buffer.as_mut_ptr().cast::<core::ffi::c_void>();
        // SAFETY: `table` is writable for `size` bytes, the size passed.
        let status = unsafe {
            match kind {
                TableKind::Tcp4 | TableKind::Tcp6 => {
                    GetExtendedTcpTable(table, &raw mut size, 0, family, TCP_TABLE_OWNER_PID_ALL, 0)
                }
                TableKind::Udp4 | TableKind::Udp6 => {
                    GetExtendedUdpTable(table, &raw mut size, 0, family, UDP_TABLE_OWNER_PID, 0)
                }
            }
        };
        match status {
            NO_ERROR => return Ok(size as usize),
            ERROR_INSUFFICIENT_BUFFER => {
                let wanted = size as usize + size as usize / 4 + 1024;
                buffer.resize(wanted.div_ceil(4), 0);
            }
            error => return Err(io::Error::from_raw_os_error(error as i32)),
        }
    }
    Err(io::Error::from_raw_os_error(
        ERROR_INSUFFICIENT_BUFFER as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::Transport;
    use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};

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
    fn finds_this_process_behind_its_own_udp_socket() {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        let mut resolver = SystemResolver::new();
        resolver.refresh().unwrap();

        let owner = resolver.socket_owner(&FlowKey {
            transport: Transport::Udp,
            local: SocketAddr::new([10, 64, 0, 2].into(), socket.local_addr().unwrap().port()),
            remote: "198.51.100.9:53".parse().unwrap(),
        });

        assert_eq!(owner, Some(std::process::id()));
    }

    #[test]
    fn reads_the_executable_and_a_stable_start_time_of_this_process() {
        let mut resolver = SystemResolver::new();
        let pid = std::process::id();

        let executable = resolver.executable(pid);
        let first = resolver.start_time(pid);

        assert_eq!(
            executable.map(|path| std::fs::canonicalize(path).unwrap()),
            Some(std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap())
        );
        assert!(first.is_some());
        assert_eq!(first, resolver.start_time(pid));
    }
}
