//! Where the programs run as root are taken from.
//!
//! The daemon and the installer helpers run system tools (`ip`, `ifconfig`, `route`,
//! `launchctl`, `systemd-run`, `systemd-creds`, ...) as root. Resolved through `PATH`, a
//! directory on it that another account can write would hand that account root, so they are
//! taken from the system's own directories instead.

use std::path::{Path, PathBuf};

/// The directories system tools live in, in search order: the usual Unix ones, then the
/// system profiles of NixOS and Guix, which have no tool under `/usr`.
const SYSTEM_DIRS: &[&str] = &[
    "/usr/sbin",
    "/usr/bin",
    "/sbin",
    "/bin",
    "/run/current-system/sw/bin",
    "/run/current-system/profile/bin",
];

/// The absolute path of the system tool `name`. A tool none of the system directories holds
/// keeps its bare name, so a system laid out in a way this list does not know still runs it.
pub fn path(name: &str) -> PathBuf {
    resolve_in(name, SYSTEM_DIRS, |candidate| candidate.is_file())
}

fn resolve_in(name: &str, dirs: &[&str], exists: impl Fn(&Path) -> bool) -> PathBuf {
    dirs.iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| exists(candidate))
        .unwrap_or_else(|| {
            log::warn!("{name} is in none of the system directories; resolving it through PATH");
            PathBuf::from(name)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_is_taken_from_the_first_system_directory_holding_it() {
        let found = resolve_in("ip", SYSTEM_DIRS, |candidate| {
            candidate == Path::new("/sbin/ip") || candidate == Path::new("/bin/ip")
        });

        assert_eq!(found, PathBuf::from("/sbin/ip"));
    }

    #[test]
    fn a_tool_found_in_no_system_directory_keeps_its_name() {
        let found = resolve_in("ip", SYSTEM_DIRS, |_| false);

        assert_eq!(found, PathBuf::from("ip"));
    }

    #[test]
    fn a_tool_present_on_this_machine_resolves_to_an_absolute_path() {
        assert!(path("sh").is_absolute());
    }
}
