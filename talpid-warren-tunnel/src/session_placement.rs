//! Where the next tunnel asks the exit to place its session.
//!
//! The exit assigns each session an inner IPv4 and keys everything it owns
//! per client on that address, port forwarding included. A session names its
//! address on every redial so a reconnect stays put, but that memory belongs
//! to the supervisor and dies with it. A tunnel REBUILD (an escalated pump
//! error, a drain reconnect, an adopted address change) starts a new
//! supervisor, which would introduce itself as an independent session; the
//! exit never co-houses an independent session with a live one of the same
//! identity, so while the predecessor lingers, the rebuilt tunnel lands on a
//! different address and inherits none of its own state. Its forwarded ports
//! then read as another client's, and the address change can itself escalate
//! another rebuild.
//!
//! This memory outlives a single tunnel so the rebuilt one can name the
//! address it already holds. Naming a stale or foreign address is safe: the
//! exit only honours it for the identity that holds it, and otherwise falls
//! back to placing an independent session.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// File under the daemon cache directory holding the address, so the memory
/// also survives a daemon that was killed rather than stopped: that is the
/// case where the exit still holds the old session and placing the next one
/// independently costs the tunnel its own address.
pub(crate) const PLACEMENT_FILE: &str = "warren-session-placement";

/// Process-wide instance. The daemon runs one Warren tunnel at a time, so
/// successive tunnels are successive lives of one session.
pub(crate) static SESSION_PLACEMENT: SessionPlacement = SessionPlacement::new();

/// Last inner IPv4 an exit assigned, or none yet.
pub(crate) struct SessionPlacement {
    address: AtomicU32,
    /// Where to mirror the address. Absent until the daemon supplies its
    /// cache directory, and on platforms or tests that have none, which keeps
    /// the memory process-local rather than failing.
    file: Mutex<Option<PathBuf>>,
}

impl SessionPlacement {
    pub(crate) const fn new() -> Self {
        // 0 doubles as "nothing remembered": an exit never assigns 0.0.0.0,
        // which is the wire sentinel for "place me on my own address".
        Self {
            address: AtomicU32::new(0),
            file: Mutex::new(None),
        }
    }

    /// Adopt `cache_dir` as the mirror and read back what the previous daemon
    /// left there. Anything unreadable or unparsable is treated as "no
    /// predecessor": the address is a hint, and a connect must never depend
    /// on it.
    pub(crate) fn load_from(&self, cache_dir: &Path) {
        let path = cache_dir.join(PLACEMENT_FILE);
        if let Ok(contents) = std::fs::read_to_string(&path)
            && let Ok(addr) = contents.trim().parse::<Ipv4Addr>()
            && !addr.is_unspecified()
        {
            self.address.store(addr.to_bits(), Ordering::Relaxed);
        }
        *self.file.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
    }

    /// Record the address the exit just assigned.
    pub(crate) fn remember(&self, assigned: Ipv4Addr) {
        if assigned.is_unspecified() {
            return;
        }
        self.address.store(assigned.to_bits(), Ordering::Relaxed);
        if let Some(path) = self
            .file
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_deref()
        {
            // Best effort: a tunnel that cannot write its hint still runs.
            let _ = std::fs::write(path, assigned.to_string());
        }
    }

    /// The address a rebuilt tunnel should ask to be placed on.
    pub(crate) fn recall(&self) -> Option<Ipv4Addr> {
        match self.address.load(Ordering::Relaxed) {
            0 => None,
            bits => Some(Ipv4Addr::from_bits(bits)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique directory that removes itself, so a failing test cannot leave
    /// one behind and two tests cannot collide on one path.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "warren-session-placement-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp cache dir");
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn recalls_nothing_before_any_session_is_assigned() {
        assert_eq!(SessionPlacement::new().recall(), None);
    }

    #[test]
    fn a_restarted_daemon_recalls_the_address_of_the_process_before_it() {
        // The memory above dies with the process, and a daemon that is killed
        // rather than stopped leaves its session live on the exit for the
        // whole idle window. Starting over as an independent session there is
        // what moves the tunnel off its own address and orphans its ports.
        let dir = TempDir::new("restart");
        let first_run = SessionPlacement::new();
        first_run.load_from(dir.path());
        first_run.remember(Ipv4Addr::new(10, 66, 0, 206));

        let after_restart = SessionPlacement::new();
        after_restart.load_from(dir.path());

        assert_eq!(after_restart.recall(), Some(Ipv4Addr::new(10, 66, 0, 206)));
    }

    #[test]
    fn an_unreadable_cache_leaves_the_tunnel_asking_for_a_fresh_session() {
        // The address is a hint, never a requirement: a missing, empty or
        // corrupt file must degrade to "no predecessor", never fail a connect.
        let dir = TempDir::new("corrupt");
        std::fs::write(dir.path().join(PLACEMENT_FILE), b"not an address")
            .expect("write corrupt file");

        let placement = SessionPlacement::new();
        placement.load_from(dir.path());

        assert_eq!(placement.recall(), None);
    }

    #[test]
    fn recalls_the_address_of_the_latest_session() {
        let placement = SessionPlacement::new();
        placement.remember(Ipv4Addr::new(10, 66, 0, 179));
        placement.remember(Ipv4Addr::new(10, 66, 0, 206));
        assert_eq!(placement.recall(), Some(Ipv4Addr::new(10, 66, 0, 206)));
    }

    /// The all-zero address is the wire sentinel for "no predecessor", so
    /// storing it would erase a perfectly good address and send the next
    /// tunnel back to an independent session start.
    #[test]
    fn keeps_its_address_when_offered_the_unspecified_one() {
        let placement = SessionPlacement::new();
        placement.remember(Ipv4Addr::new(10, 66, 0, 179));
        placement.remember(Ipv4Addr::UNSPECIFIED);
        assert_eq!(placement.recall(), Some(Ipv4Addr::new(10, 66, 0, 179)));
    }
}
