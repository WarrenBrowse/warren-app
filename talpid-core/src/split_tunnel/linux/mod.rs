//! Linux split-tunneling implementation using cgroups.
//!
//! It's recommended to read the kernel docs before delving into this module:
//! <https://docs.kernel.org/admin-guide/cgroup-v2.html>

use anyhow::Context;
use libc::pid_t;
use nftnl::{Batch, Chain, Hook, MsgType, Policy, ProtoFamily, Rule, Table, nft_expr};
use nix::unistd::Pid;
use talpid_cgroup::{
    INCLUDE_CGROUP_NAME, SPLIT_TUNNEL_CGROUP_NAME,
    v1::{CGroup1, NET_CLS_CLASSID},
    v2::CGroup2,
};

use crate::firewall;

/// Value used to mark packets and associated connections.
/// This should be an arbitrary but unique integer.
pub const MARK: u32 = 0xf41;

mod inet_sockets;

/// Errors related to split tunneling.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Errors related to split tunneling.
    #[error("Error in split tunneling")]
    SplitTunnel(#[from] anyhow::Error),
    /// Errors related to cgroups.
    #[error("Error in split tunneling")]
    CGroup(#[from] talpid_cgroup::Error),
    /// The process holds open Internet connections, which would carry on
    /// outside the tunnel.
    #[error(
        "The process has open Internet connections, which would stay outside the tunnel: \
         start it with warren-include instead"
    )]
    HasOpenConnections,
    /// Its open sockets could not be listed, so it cannot be known safe.
    #[error("The process's open connections could not be listed")]
    ListConnections(#[source] std::io::Error),
}

/// Manages PIDs in the linux cgroup used for vpn tunnel exclusion.
///
/// It's recommended to read the kernel docs before delving into this module:
/// <https://docs.kernel.org/admin-guide/cgroup-v2.html>
pub struct PidManager {
    inner: Result<Inner, Error>,
    /// The include-only cgroup. cgroup2 only, whatever the `cgroup2`
    /// feature says for exclusion: the firewall selects its sockets with
    /// `socket cgroupv2`, which has no cgroup v1 counterpart.
    included: Result<IncludedCGroup, Error>,
}

struct IncludedCGroup {
    root: CGroup2,
    included: CGroup2,
}

enum Inner {
    CGroup1(InnerCGroup1),
    #[cfg(feature = "cgroup2")]
    CGroup2(InnerCGroup2),
}

struct InnerCGroup1 {
    root_cgroup1: CGroup1,
    excluded_cgroup1: CGroup1,
    net_cls_classid: u32,
}

#[cfg(feature = "cgroup2")]
struct InnerCGroup2 {
    root_cgroup2: CGroup2,
    excluded_cgroup2: CGroup2,
}

impl PidManager {
    fn new() -> Self {
        let inner = Self::new_inner();

        if let Err(e) = &inner {
            log::error!("Failed to initialize split-tunneling: {e:?}");
        };

        let included = Self::new_included();
        if let Err(e) = &included {
            log::error!("Include-only is unavailable: {e:?}");
        }

        PidManager { inner, included }
    }

    fn new_included() -> Result<IncludedCGroup, Error> {
        // The fixed mount path, never an override from the environment:
        // `warren-include` joins this same cgroup and, running setuid root,
        // cannot trust its caller's environment to find it.
        let root = CGroup2::open(talpid_cgroup::CGROUP2_DEFAULT_MOUNT_PATH)?;
        // Probed on the root, which always exists: creating the included
        // cgroup first would leave one `warren-include` can join while the
        // firewall cannot select it.
        assert_nft_supports_cgroup2(&root)
            .context("cgroup2 not supported by nftables, are you running an old kernel?")?;
        let included = root.create_or_open_child(INCLUDE_CGROUP_NAME)?;
        Ok(IncludedCGroup { root, included })
    }

    /// Add a PID to the include-only cgroup, to have it tunneled while
    /// everything else is not.
    ///
    /// Only the sockets a process opens after the move are selected, so a
    /// process holding an Internet socket is refused, and put back where it
    /// was. The check runs after the move: a socket opened in between is
    /// then selected, and one opened before it is still open and seen.
    pub fn add_included(&self, pid: pid_t) -> Result<(), Error> {
        let cgroups = self.included()?;
        let pid = Pid::from_raw(pid);
        let proc_dir = std::path::PathBuf::from(format!("/proc/{pid}"));
        let previous = previous_cgroup2(&proc_dir);
        cgroups.included.add_pid(pid)?;
        let refusal = match inet_sockets::holds_inet_socket(&proc_dir) {
            Ok(false) => return Ok(()),
            Ok(true) => Error::HasOpenConnections,
            Err(error) => Error::ListConnections(error),
        };
        match previous {
            Some(previous) => previous.add_pid(pid)?,
            None => cgroups.root.add_pid(pid)?,
        }
        Err(refusal)
    }

    /// Move a PID out of the include-only cgroup.
    pub fn remove_included(&self, pid: pid_t) -> Result<(), Error> {
        Ok(self.included()?.root.add_pid(Pid::from_raw(pid))?)
    }

    /// Return the PIDs in the include-only cgroup.
    pub fn list_included(&mut self) -> Result<Vec<pid_t>, Error> {
        Ok(self.included_mut()?.included.list_pids()?)
    }

    /// Move every PID out of the include-only cgroup.
    pub fn clear_included(&mut self) -> Result<(), Error> {
        for pid in self.list_included()? {
            self.remove_included(pid)?;
        }
        Ok(())
    }

    /// Whether include-only can run here.
    pub fn include_only_supported(&self) -> bool {
        self.included.is_ok()
    }

    /// A handle to the include-only cgroup, for the firewall.
    pub fn included_cgroup(&self) -> Option<CGroup2> {
        self.included()
            .ok()?
            .included
            .try_clone()
            .inspect_err(|e| log::error!("Failed to clone file handle to cgroup2: {e}"))
            .ok()
    }

    fn included(&self) -> Result<&IncludedCGroup, Error> {
        self.included
            .as_ref()
            .ok()
            .context("Include-only is not available")
            .map_err(Into::into)
    }

    fn included_mut(&mut self) -> Result<&mut IncludedCGroup, Error> {
        self.included
            .as_mut()
            .ok()
            .context("Include-only is not available")
            .map_err(Into::into)
    }

    fn new_inner() -> Result<Inner, Error> {
        #[cfg(feature = "cgroup2")]
        return Self::new_cgroup2()
            .map(Inner::CGroup2)
            .or_else(|cgroup2_err| {
                log::warn!(
                    "Failed to initialize cgroups v2, falling back to cgroup v1 for split tunneling"
                );
                log::warn!("Note that cgroups v1 is deprecated and will be removed in the future");

                // If it does not succeed, the kernel might be too old, so we fallback on the old cgroup1 solution.
                Self::new_cgroup1()
                    .map(Inner::CGroup1)
                    .map_err(|cgroup1_err| {
                        log::error!("Failed to initialize split-tunneling");
                        log::trace!("{cgroup1_err:?}");
                        log::trace!("{cgroup2_err:?}");
                        cgroup2_err
                    })
            });

        #[cfg(not(feature = "cgroup2"))]
        Self::new_cgroup1().map(Inner::CGroup1).inspect_err(|err| {
            log::error!("Failed to initialize split-tunneling");
            log::trace!("{err:?}");
        })
    }

    #[cfg(feature = "cgroup2")]
    fn new_cgroup2() -> Result<InnerCGroup2, Error> {
        let root_cgroup2 = CGroup2::open_root()?;

        let excluded_cgroup2 = root_cgroup2.create_or_open_child(SPLIT_TUNNEL_CGROUP_NAME)?;

        assert_nft_supports_cgroup2(&excluded_cgroup2)
            .context("cgroup2 not supported by nftables, are you running an old kernel?")?;

        Ok(InnerCGroup2 {
            root_cgroup2,
            excluded_cgroup2,
        })
    }

    fn new_cgroup1() -> Result<InnerCGroup1, Error> {
        let root_cgroup = CGroup1::open_root()?;
        let excluded_cgroup = root_cgroup.create_or_open_child(SPLIT_TUNNEL_CGROUP_NAME)?;
        excluded_cgroup.set_net_cls_id(NET_CLS_CLASSID)?;

        Ok(InnerCGroup1 {
            net_cls_classid: NET_CLS_CLASSID,
            root_cgroup1: root_cgroup,
            excluded_cgroup1: excluded_cgroup,
        })
    }

    /// Add a PID to the cgroup2 to have it excluded from the tunnel.
    pub fn add(&self, pid: pid_t) -> Result<(), Error> {
        let pid = Pid::from_raw(pid);
        self.inner()?.add(pid)
    }

    /// Remove a PID from the cgroup2 to have it included in the tunnel.
    pub fn remove(&self, pid: pid_t) -> Result<(), Error> {
        let pid = Pid::from_raw(pid);
        self.inner()?.remove(pid)
    }

    /// Return a list of all PIDs currently in the Cgroup excluded from the tunnel.
    pub fn list(&mut self) -> Result<Vec<pid_t>, Error> {
        self.inner_mut()?.list()
    }

    /// Removes all PIDs from the Cgroup.
    pub fn clear(&mut self) -> Result<(), Error> {
        self.inner_mut()?.clear()
    }

    /// Return whether it is supported/available
    pub fn is_supported(&self) -> bool {
        matches!(self.inner, Ok(..))
    }

    /// Get a handle to the [CGroup2] used for split-tunneling.
    ///
    /// Returns an option if we prevously failed to set up the cgroup2, or if cloning it fails.
    pub fn excluded_cgroup(&self) -> Option<CGroup2> {
        self.inner()
            .ok()?
            .excluded_cgroup()
            .inspect_err(|e| log::error!("Failed to clone file handle to cgroup2: {e}"))
            .ok()?
    }

    /// Get the net_cls classid of the v1 cgroup used for split tunneling.
    ///
    /// This only exist if cgroup v1 is used for split tunneling.
    pub fn net_cls_classid(&self) -> Option<u32> {
        self.inner().ok()?.net_cls_classid()
    }

    fn inner(&self) -> Result<&Inner, Error> {
        self.inner
            .as_ref()
            .ok()
            .context("Split-tunneling is not available")
            .map_err(Into::into)
    }

    fn inner_mut(&mut self) -> Result<&mut Inner, Error> {
        self.inner
            .as_mut()
            .ok()
            .context("Split-tunneling is not available")
            .map_err(Into::into)
    }
}

impl Inner {
    /// Add a PID to the cgroup2 to have it excluded from the tunnel.
    fn add(&self, pid: Pid) -> Result<(), Error> {
        match self {
            Inner::CGroup1(inner) => inner.excluded_cgroup1.add_pid(pid)?,
            #[cfg(feature = "cgroup2")]
            Inner::CGroup2(inner) => inner.excluded_cgroup2.add_pid(pid)?,
        }
        Ok(())
    }

    /// Remove a PID from the cgroup to have it included in the tunnel.
    fn remove(&self, pid: Pid) -> Result<(), Error> {
        // PIDs can only be removed from a cgroup by adding them to another cgroup.
        match self {
            Inner::CGroup1(inner) => inner.root_cgroup1.add_pid(pid)?,
            #[cfg(feature = "cgroup2")]
            Inner::CGroup2(inner) => inner.root_cgroup2.add_pid(pid)?,
        }
        Ok(())
    }

    /// Return a list of all PIDs currently in the Cgroup excluded from the tunnel.
    fn list(&mut self) -> Result<Vec<pid_t>, Error> {
        Ok(match self {
            Inner::CGroup1(inner) => inner.excluded_cgroup1.list_pids()?,
            #[cfg(feature = "cgroup2")]
            Inner::CGroup2(inner) => inner.excluded_cgroup2.list_pids()?,
        })
    }

    /// Removes all PIDs from the Cgroup.
    fn clear(&mut self) -> Result<(), Error> {
        for pid in self.list()? {
            let pid = Pid::from_raw(pid);
            self.remove(pid)?;
        }
        Ok(())
    }

    /// Get a handle to the [CGroup2] used for split-tunneling, if any.
    ///
    /// Returns an error if cloning the cgroup fails.
    fn excluded_cgroup(&self) -> Result<Option<CGroup2>, Error> {
        match self {
            Inner::CGroup1(..) => Ok(None),
            #[cfg(feature = "cgroup2")]
            Inner::CGroup2(inner) => Ok(inner.excluded_cgroup2.try_clone().map(Some)?),
        }
    }

    /// Get the net_cls classid associated with the v1 cgroup used for split-tunneling, if any.
    ///
    /// This returns none if we're using cgroups v1, or if we failed to create the v1 cgroup.
    fn net_cls_classid(&self) -> Option<u32> {
        match self {
            Inner::CGroup1(inner) => Some(inner.net_cls_classid),
            #[cfg(feature = "cgroup2")]
            Inner::CGroup2(..) => None,
        }
    }
}

/// The cgroup2 a process is in, read from `/proc/<pid>/cgroup`, so a refused
/// inclusion can put it back.
fn previous_cgroup2(proc_dir: &std::path::Path) -> Option<CGroup2> {
    let cgroups = std::fs::read_to_string(proc_dir.join("cgroup")).ok()?;
    let path = cgroups.lines().find_map(|line| line.strip_prefix("0::"))?;
    let path = path.trim_start_matches('/');
    CGroup2::open(std::path::Path::new(talpid_cgroup::CGROUP2_DEFAULT_MOUNT_PATH).join(path)).ok()
}

/// Check whether we can create an nft table with a `socket cgroupv2 level x` rule.
///
/// Assuming that this process has the sufficient privileges, then this function should only fail
/// when the kernel doesn't support this kind of rule. This is the case for kernels predating 5.13.
//
// NOTE:
// Interfacing with firewall outside of the firewall module is spaghetti.
// Consider either having this module take ownership of setting up the split-tunneling nft rules,
// or moving this logic into the firewall module and coupling it with the actual firewall rules we
// set up.
fn assert_nft_supports_cgroup2(cgroup: &CGroup2) -> Result<(), Error> {
    let table_name = c"mullvad-test-cgroup2-capability";

    let mut batch = Batch::new();
    let table = Table::new(table_name, ProtoFamily::Inet);
    batch.add(&table, MsgType::Add);

    let mut chain = Chain::new(c"test", &table);
    chain.set_hook(Hook::Out, 0);
    chain.set_policy(Policy::Accept);
    batch.add(&chain, MsgType::Add);

    let mut rule = Rule::new(&chain);
    rule.add_expr(&nft_expr!(socket cgroupv2 level 1));
    rule.add_expr(&nft_expr!(cmp == cgroup.inode()));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);

    // Remove the table. Since this happens is the same batch, the table will never process any packets.
    // This makes it effectively a dry-run.
    let table = Table::new(table_name, ProtoFamily::Inet);
    batch.add(&table, MsgType::Del);

    let batch = batch.finalize();
    firewall::linux::Firewall::send_and_process(&batch)
        .context("Failed to add nft cgroupv2 rule")?;

    Ok(())
}

impl Default for PidManager {
    fn default() -> Self {
        Self::new()
    }
}
