fn main() {
    #[cfg(target_os = "linux")]
    inner::main();
}

/// Whether the account `uid` may run a program outside the tunnel, given the
/// daemon's wallet owner record (`None` when there is none).
///
/// Leaving the tunnel escapes the kill switch the owner chose for the whole
/// machine, so it is the owner's or root's to do, like every other change to
/// the machine's network. Without a record nobody but root may: the wallet's
/// owner is recorded as soon as the app claims it.
#[cfg_attr(not(any(target_os = "linux", test)), expect(dead_code))]
fn may_leave_tunnel(uid: u32, owner_record: Option<&str>) -> bool {
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
    fn root_and_the_owner_may_leave_the_tunnel() {
        assert!(may_leave_tunnel(0, None));
        assert!(may_leave_tunnel(0, Some(OWNED_BY_1000)));
        assert!(may_leave_tunnel(1000, Some(OWNED_BY_1000)));
    }

    #[test]
    fn nobody_else_may() {
        assert!(!may_leave_tunnel(1001, Some(OWNED_BY_1000)));
        assert!(!may_leave_tunnel(1000, None), "no recorded owner");
        assert!(!may_leave_tunnel(1000, Some("{not json")));
        assert!(!may_leave_tunnel(
            1000,
            Some(r#"{"version":2,"owner":{"uid":1000}}"#)
        ));
        assert!(!may_leave_tunnel(
            1000,
            Some(r#"{"version":1,"owner":{"sid":"S-1-5-21-1-2-3-1000"}}"#)
        ));
    }
}

#[cfg(target_os = "linux")]
mod inner {
    use nix::unistd::{Pid, execvp, getgid, getpid, getuid, setgid, setuid};
    use std::{
        convert::Infallible,
        env,
        error::Error as StdError,
        ffi::{CString, NulError},
        fmt::Write as _,
        os::unix::ffi::OsStrExt,
    };
    use talpid_cgroup::{SPLIT_TUNNEL_CGROUP_NAME, find_net_cls_mount, v1::CGroup1};

    #[derive(thiserror::Error, Debug)]
    enum Error {
        #[error("Invalid arguments")]
        InvalidArguments,

        #[error("Cannot assign process to cgroup")]
        AddProcToCGroup(#[from] talpid_cgroup::Error),

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

        #[error(
            "Only the account that set Warren up, or root, may run programs outside the tunnel"
        )]
        NotTheOwner,
    }

    /// Launch a program in a cgroup where traffic will be excluded from the VPN tunnel.
    ///
    /// Note: Set the `TALPID_EXCLUSION_CGROUP` env variable to control where the root cgroup is
    /// mounted. See (README.md)[../../README.md#Environment-variables-used-by-the-service] for
    /// details.
    pub fn main() {
        let Err(error) = run();

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

    fn run() -> Result<Infallible, Error> {
        let mut args_iter = env::args_os().skip(1);
        let program = args_iter.next().ok_or(Error::InvalidArguments)?;
        let program = CString::new(program.as_bytes()).map_err(Error::ArgumentNul)?;

        let args: Vec<CString> = env::args_os()
            .skip(1)
            .map(|arg| CString::new(arg.as_bytes()))
            .collect::<Result<Vec<CString>, NulError>>()
            .map_err(Error::ArgumentNul)?;

        let real_uid = getuid();
        if !super::may_leave_tunnel(real_uid.as_raw(), owner_record().as_deref()) {
            return Err(Error::NotTheOwner);
        }

        exclude(getpid())?;

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

    #[cfg(feature = "cgroup2")]
    fn exclude(pid: Pid) -> Result<(), Error> {
        let result = talpid_cgroup::v2::CGroup2::open_root()
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
