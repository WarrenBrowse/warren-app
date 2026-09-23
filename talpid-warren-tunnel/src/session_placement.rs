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
//! address it already holds. The address is remembered with the exit that
//! assigned it and named to that exit only: an exit that has just restarted
//! may hand a named address to whoever names it, so naming it to another
//! exit could carry one inner address across exits.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use warrenguard_multihop::ExitId;

/// File under the daemon cache directory holding the address, so the memory
/// also survives a daemon that was killed rather than stopped: that is the
/// case where the exit still holds the old session and placing the next one
/// independently costs the tunnel its own address.
pub(crate) const PLACEMENT_FILE: &str = "warren-session-placement";

/// Process-wide instance. The daemon runs one Warren tunnel at a time, so
/// successive tunnels are successive lives of one session.
pub(crate) static SESSION_PLACEMENT: SessionPlacement = SessionPlacement::new();

/// Last inner IPv4 an exit assigned, with that exit, or none yet.
pub(crate) struct SessionPlacement {
    last: Mutex<Option<(ExitId, Ipv4Addr)>>,
    /// Where to mirror the address. Absent until the daemon supplies its
    /// cache directory, and on platforms or tests that have none, which keeps
    /// the memory process-local rather than failing.
    file: Mutex<Option<PathBuf>>,
}

/// `<exit id as 32 lowercase hex digits> <IPv4>`, the mirror file's one line.
fn encode(exit: ExitId, assigned: Ipv4Addr) -> String {
    let hex: String = exit.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex} {assigned}")
}

/// Parse [`encode`]'s line. A file written before the exit was recorded holds
/// an address alone and reads as nothing: that address cannot be tied to an
/// exit, so it must not be named to any.
fn decode(line: &str) -> Option<(ExitId, Ipv4Addr)> {
    let (hex, addr) = line.trim().split_once(' ')?;
    if hex.len() != 32 {
        return None;
    }
    let mut exit = [0u8; 16];
    for (i, byte) in exit.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    let addr = addr.parse::<Ipv4Addr>().ok()?;
    (!addr.is_unspecified()).then_some((ExitId::from_bytes(exit), addr))
}

impl SessionPlacement {
    pub(crate) const fn new() -> Self {
        Self {
            last: Mutex::new(None),
            file: Mutex::new(None),
        }
    }

    /// Adopt `cache_dir` as the mirror and read back what the previous daemon
    /// left there. Anything unreadable or unparsable is treated as "no
    /// predecessor": the address is a hint, and a connect must never depend
    /// on it.
    pub(crate) fn load_from(&self, cache_dir: &Path) {
        let path = cache_dir.join(PLACEMENT_FILE);
        if let Some(last) = std::fs::read_to_string(&path)
            .ok()
            .as_deref()
            .and_then(decode)
        {
            *self.last.lock().unwrap_or_else(|p| p.into_inner()) = Some(last);
        }
        *self.file.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
    }

    /// Record the address `exit` just assigned.
    pub(crate) fn remember(&self, exit: ExitId, assigned: Ipv4Addr) {
        if assigned.is_unspecified() {
            return;
        }
        *self.last.lock().unwrap_or_else(|p| p.into_inner()) = Some((exit, assigned));
        if let Some(path) = self
            .file
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_deref()
        {
            // Best effort: a tunnel that cannot write its hint still runs.
            let _ = std::fs::write(path, encode(exit, assigned));
        }
    }

    /// The address a rebuilt tunnel should ask to be placed on, with the
    /// exit it may be named to.
    pub(crate) fn recall(&self) -> Option<(ExitId, Ipv4Addr)> {
        *self.last.lock().unwrap_or_else(|p| p.into_inner())
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

    const EXIT: ExitId = ExitId::from_bytes([0xE1; 16]);

    #[test]
    fn a_restarted_daemon_recalls_the_address_of_the_process_before_it() {
        // The memory above dies with the process, and a daemon that is killed
        // rather than stopped leaves its session live on the exit for the
        // whole idle window. Starting over as an independent session there is
        // what moves the tunnel off its own address and orphans its ports.
        let dir = TempDir::new("restart");
        let first_run = SessionPlacement::new();
        first_run.load_from(dir.path());
        first_run.remember(EXIT, Ipv4Addr::new(10, 66, 0, 206));

        let after_restart = SessionPlacement::new();
        after_restart.load_from(dir.path());

        assert_eq!(
            after_restart.recall(),
            Some((EXIT, Ipv4Addr::new(10, 66, 0, 206)))
        );
    }

    #[test]
    fn an_address_mirrored_without_its_exit_is_not_recalled() {
        // A file from before the exit was recorded cannot say which exit the
        // address belongs to, so it must not be named to any.
        let dir = TempDir::new("legacy");
        std::fs::write(dir.path().join(PLACEMENT_FILE), b"10.66.0.206").expect("write legacy file");

        let placement = SessionPlacement::new();
        placement.load_from(dir.path());

        assert_eq!(placement.recall(), None);
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
    fn recalls_the_address_of_the_latest_session_with_its_exit() {
        let other = ExitId::from_bytes([0xE2; 16]);
        let placement = SessionPlacement::new();
        placement.remember(EXIT, Ipv4Addr::new(10, 66, 0, 179));
        placement.remember(other, Ipv4Addr::new(10, 66, 0, 206));
        assert_eq!(
            placement.recall(),
            Some((other, Ipv4Addr::new(10, 66, 0, 206)))
        );
    }

    /// The all-zero address is the wire sentinel for "no predecessor", so
    /// storing it would erase a perfectly good address and send the next
    /// tunnel back to an independent session start.
    #[test]
    fn keeps_its_address_when_offered_the_unspecified_one() {
        let placement = SessionPlacement::new();
        placement.remember(EXIT, Ipv4Addr::new(10, 66, 0, 179));
        placement.remember(EXIT, Ipv4Addr::UNSPECIFIED);
        assert_eq!(
            placement.recall(),
            Some((EXIT, Ipv4Addr::new(10, 66, 0, 179)))
        );
    }
}
