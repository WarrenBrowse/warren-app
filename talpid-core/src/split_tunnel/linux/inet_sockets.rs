//! Whether a process holds an Internet socket.
//!
//! A socket keeps the cgroup its process was in when it was created, so
//! moving a running process into the include-only cgroup covers only the
//! sockets it opens afterwards: an open connection would carry on outside the
//! tunnel. A process is therefore only included while it holds none.

use std::{collections::HashSet, fs, io, path::Path};

/// The socket tables of a network namespace under `/proc/<pid>/net`.
const INET_TABLES: [&str; 8] = [
    "tcp", "tcp6", "udp", "udp6", "udplite", "udplite6", "raw", "raw6",
];

/// Whether the process whose `/proc` directory is `proc_dir` holds a TCP,
/// UDP or raw socket, over IPv4 or IPv6.
pub fn holds_inet_socket(proc_dir: &Path) -> io::Result<bool> {
    let mut sockets = HashSet::new();
    for entry in fs::read_dir(proc_dir.join("fd"))? {
        // A descriptor closed while the directory is read is gone, not an error.
        let Ok(target) = fs::read_link(entry?.path()) else {
            continue;
        };
        if let Some(inode) = socket_inode(&target.to_string_lossy()) {
            sockets.insert(inode);
        }
    }
    if sockets.is_empty() {
        return Ok(false);
    }
    for table in INET_TABLES {
        let contents = match fs::read_to_string(proc_dir.join("net").join(table)) {
            Ok(contents) => contents,
            // A kernel without the protocol lists no table for it.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if table_inodes(&contents).any(|inode| sockets.contains(&inode)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The inode of a descriptor that is a socket (`socket:[1234]`).
fn socket_inode(link_target: &str) -> Option<u64> {
    link_target
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// The socket inodes of a `/proc/net` socket table: the tenth column of
/// every line after the header.
fn table_inodes(contents: &str) -> impl Iterator<Item = u64> + '_ {
    contents
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().nth(9)?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TCP_TABLE: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:0CEA 00000000:0000 0A 00000000:00000000 00:00000000 00000000   501        0 48291 1 0000000000000000 100 0 0 10 0
   1: 0F05A8C0:0016 0205A8C0:C3A2 01 00000000:00000000 02:0009A2F3 00000000     0        0 17334 4 0000000000000000 20 4 29 10 -1
";

    #[test]
    fn reads_the_inode_of_a_socket_descriptor_only() {
        assert_eq!(socket_inode("socket:[48291]"), Some(48291));
        assert_eq!(socket_inode("pipe:[48291]"), None);
        assert_eq!(socket_inode("/dev/null"), None);
    }

    #[test]
    fn reads_every_inode_of_a_socket_table() {
        let inodes: Vec<u64> = table_inodes(TCP_TABLE).collect();

        assert_eq!(inodes, [48291, 17334]);
    }

    #[test]
    fn finds_an_open_internet_socket_of_a_real_process() {
        let _listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

        let holds = holds_inet_socket(Path::new("/proc/self")).unwrap();

        assert!(holds);
    }

    #[test]
    fn a_process_with_only_unix_sockets_holds_none() {
        let child = std::process::Command::new("sleep")
            .arg("5")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .spawn();
        let mut child = child.unwrap();
        let proc_dir = Path::new("/proc").join(child.id().to_string());
        // Until it execs, the child still holds whatever sockets this test
        // binary has open, other tests' included.
        while fs::read_to_string(proc_dir.join("comm")).unwrap_or_default() != "sleep\n" {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let holds = holds_inet_socket(&proc_dir);
        let _ = child.kill();
        let _ = child.wait();

        assert!(!holds.unwrap());
    }
}
