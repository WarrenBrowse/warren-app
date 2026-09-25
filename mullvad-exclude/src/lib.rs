//! The Linux split tunnel launchers: `warren-exclude` runs a program outside
//! the tunnel, `warren-include` runs one inside it while "VPN only for these
//! apps" is on. Both are setuid root and never talk to the daemon: they put
//! themselves in the daemon's cgroup, drop root, and exec the program.

/// Which way a launcher splits the program it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// Outside the tunnel (`warren-exclude`).
    Exclude,
    /// Inside the tunnel while include-only is on (`warren-include`).
    Include,
}

/// Whether the account `uid` may choose how a program is routed, given the
/// daemon's wallet owner record (`None` when there is none).
///
/// Leaving the tunnel escapes the kill switch the owner chose for the whole
/// machine, and choosing which programs use the tunnel is the same kind of
/// change to the machine's network, so both are the owner's or root's to make.
/// Without a record nobody but root may: the wallet's owner is recorded as
/// soon as the app claims it.
#[cfg_attr(not(any(target_os = "linux", test)), expect(dead_code))]
fn may_split(uid: u32, owner_record: Option<&str>) -> bool {
    uid == 0 || owner_record.and_then(recorded_owner_uid) == Some(uid)
}

/// The uid an owner record names, if it is a record of a Unix account in a
/// version this program understands.
fn recorded_owner_uid(record: &str) -> Option<u32> {
    let record: serde_json::Value = serde_json::from_str(record).ok()?;
    if record.get("version")?.as_u64()? != 1 {
        return None;
    }
    u32::try_from(record.get("owner")?.get("uid")?.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNED_BY_1000: &str = r#"{"version":1,"owner":{"uid":1000}}"#;

    #[test]
    fn root_and_the_owner_may_split() {
        assert!(may_split(0, None));
        assert!(may_split(0, Some(OWNED_BY_1000)));
        assert!(may_split(1000, Some(OWNED_BY_1000)));
    }

    #[test]
    fn nobody_else_may() {
        assert!(!may_split(1001, Some(OWNED_BY_1000)));
        assert!(!may_split(1000, None), "no recorded owner");
        assert!(!may_split(1000, Some("{not json")));
        assert!(!may_split(
            1000,
            Some(r#"{"version":2,"owner":{"uid":1000}}"#)
        ));
        assert!(!may_split(
            1000,
            Some(r#"{"version":1,"owner":{"sid":"S-1-5-21-1-2-3-1000"}}"#)
        ));
    }
}

#[cfg(target_os = "linux")]
pub use inner::main;

#[cfg(target_os = "linux")]
mod inner {
    use super::Launch;
    use nix::unistd::{Pid, execvp, getgid, getpid, getuid, setgid, setuid};
    use std::{
        convert::Infallible,
        env,
        error::Error as StdError,
        ffi::{CString, NulError},
        fmt::Write as _,
        os::unix::ffi::OsStrExt,
        path::Path,
    };
    use talpid_cgroup::{
        CGROUP2_DEFAULT_MOUNT_PATH, INCLUDE_CGROUP_NAME, SPLIT_TUNNEL_CGROUP_NAME,
        find_net_cls_mount, v1::CGroup1, v2::CGroup2,
    };

    #[derive(thiserror::Error, Debug)]
    enum Error {
        #[error("Invalid arguments")]
        InvalidArguments,

        #[error("Cannot assign process to cgroup")]
        AddProcToCGroup(#[from] talpid_cgroup::Error),

        #[error("Include-only is not set up on this machine: start the Warren daemon first")]
        NoIncludeCGroup(#[source] talpid_cgroup::Error),

        #[error("Failed to drop root user privileges for the process")]
        DropRootUid(#[source] nix::Error),

        #[error("Failed to drop root group privileges for the process")]
        DropRootGid(#[source] nix::Error),

        #[error("Failed to launch the process")]
        Exec(#[source] nix::Error),

        #[error("An argument contains interior nul bytes")]
        ArgumentNul(#[source] NulError),

        #[error("Failed to stat /proc/mounts")]
        NoProcMounts,

        #[error("Only the account that set Warren up, or root, may choose how programs are routed")]
        NotTheOwner,
    }

    /// Launch a program in the cgroup that routes it the way `launch` says.
    ///
    /// Both launchers use the default cgroup2 mount and find the net_cls one
    /// in `/proc/mounts`, never a path from the caller's environment.
    pub fn main(launch: Launch) {
        let Err(error) = run(launch);

        match error {
            Error::InvalidArguments => {
                let mut args = env::args();
                let program = args
                    .next()
                    .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string());
                eprintln!("Usage: {program} COMMAND [ARGS]");
                std::process::exit(1);
            }
            e => {
                let mut s = format!("Error: {e}");
                let mut source = e.source();
                while let Some(error) = source {
                    write!(&mut s, "\nCaused by: {error}").expect("formatting failed");
                    source = error.source();
                }
                eprintln!("{s}");

                std::process::exit(1);
            }
        }
    }

    fn add_to_cgroups_v1_if_exists(pid: Pid) -> Result<(), Error> {
        let Some(net_cls_dir) = find_net_cls_mount().map_err(|_| Error::NoProcMounts)? else {
            return Ok(());
        };

        let cgroup_path = net_cls_dir.join(SPLIT_TUNNEL_CGROUP_NAME);

        CGroup1::open(cgroup_path)
            .and_then(|cgroup| cgroup.add_pid(pid))
            .map_err(Error::from)
    }

    fn run(launch: Launch) -> Result<Infallible, Error> {
        let mut args_iter = env::args_os().skip(1);
        let program = args_iter.next().ok_or(Error::InvalidArguments)?;
        let program = CString::new(program.as_bytes()).map_err(Error::ArgumentNul)?;

        let args: Vec<CString> = env::args_os()
            .skip(1)
            .map(|arg| CString::new(arg.as_bytes()))
            .collect::<Result<Vec<CString>, NulError>>()
            .map_err(Error::ArgumentNul)?;

        let real_uid = getuid();
        if !super::may_split(real_uid.as_raw(), owner_record().as_deref()) {
            return Err(Error::NotTheOwner);
        }

        match launch {
            Launch::Exclude => exclude(getpid())?,
            Launch::Include => include(getpid())?,
        }

        // Drop root privileges, the group first: once the uid is no longer
        // root, changing the gid is no longer permitted.
        let real_gid = getgid();
        setgid(real_gid).map_err(Error::DropRootGid)?;
        setuid(real_uid).map_err(Error::DropRootUid)?;

        // Launch the process
        execvp(&program, &args).map_err(Error::Exec)
    }

    /// The daemon's wallet owner record, from the compiled settings directory:
    /// this program runs setuid root, so nothing its caller controls, like
    /// the environment, may choose which file it trusts.
    fn owner_record() -> Option<String> {
        let settings_dir = mullvad_paths::get_default_settings_dir().ok()?;
        std::fs::read_to_string(settings_dir.join(mullvad_paths::WALLET_OWNER_FILENAME)).ok()
    }

    /// Joins the include-only cgroup the daemon created. Never creates it:
    /// the firewall selects included sockets by the inode of the cgroup the
    /// daemon opened, so a program in any other cgroup would not be tunneled.
    /// A missing cgroup fails the launch instead of running the program
    /// outside the tunnel.
    fn include(pid: Pid) -> Result<(), Error> {
        let path = Path::new(CGROUP2_DEFAULT_MOUNT_PATH).join(INCLUDE_CGROUP_NAME);
        let cgroup = CGroup2::open(path).map_err(Error::NoIncludeCGroup)?;
        Ok(cgroup.add_pid(pid)?)
    }

    #[cfg(feature = "cgroup2")]
    fn exclude(pid: Pid) -> Result<(), Error> {
        // The fixed mount, never `open_root`: that one honours an override
        // from the environment, which a setuid program must not trust.
        let result = CGroup2::open(CGROUP2_DEFAULT_MOUNT_PATH)
            .and_then(|root_cgroup2| root_cgroup2.create_or_open_child(SPLIT_TUNNEL_CGROUP_NAME))
            .and_then(|exclusion_cgroup2| exclusion_cgroup2.add_pid(pid));

        // Always add current PID to cgroup1 (deprecated solution). It does not hurt to be in both cgroup1 and cgroup2 at
        // the same time, the firewall will have to promise to behave appropriately.
        if let Err(add_err) = add_to_cgroups_v1_if_exists(pid)
            && result.is_err()
        {
            eprintln!("Failed to add process to v1 cgroup: {add_err}");
        }

        Ok(result?)
    }

    #[cfg(not(feature = "cgroup2"))]
    fn exclude(pid: Pid) -> Result<(), Error> {
        add_to_cgroups_v1_if_exists(pid)
    }
}
