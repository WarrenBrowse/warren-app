//! Apps locked to the VPN: blocked whenever the tunnel does not carry them.
//! Windows holds a list of executables ([`WinFwEnforcer`]); Linux the cgroup
//! the locked programs are opened in ([`NftEnforcer`]).
//!
//! The lock belongs to no tunnel state. It is installed once and follows the
//! tunnel interface, which only the connected state has: every other state,
//! and a stopped daemon, leave the locked apps loopback (and the LAN when it
//! is shared) and nothing else. See `docs/app-routing.md`, section 8.

use std::ffi::OsString;

/// What the firewall is asked to hold.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lock {
    /// The executables of the locked apps.
    pub apps: Vec<OsString>,
    /// The tunnel interface the locked apps may use, while connected.
    pub tunnel_interface: Option<String>,
    /// Whether the locked apps may reach the LAN.
    pub allow_lan: bool,
}

/// Puts a [`Lock`] in force, replacing the previous one.
pub trait Enforcer {
    /// Why a lock could not be put in force.
    type Error;

    /// Replaces the lock in force with `lock`.
    fn enforce(&mut self, lock: &Lock) -> Result<(), Self::Error>;
}

/// The lock the user asked for, kept in force by an [`Enforcer`].
pub struct AppLocks<E> {
    enforcer: E,
    wanted: Lock,
    applied: Option<Lock>,
    own_executables: Vec<OsString>,
}

impl<E: Enforcer> AppLocks<E> {
    /// A lock on `apps` with no tunnel. Nothing is enforced until the first
    /// change or [`Self::refresh`].
    ///
    /// `own_executables` are never locked, whatever the list says: the
    /// daemon's own connection to its relays leaves outside the tunnel, so
    /// locking it would cut the tunnel it waits for.
    pub fn new(enforcer: E, own_executables: Vec<OsString>, allow_lan: bool) -> Self {
        Self {
            enforcer,
            wanted: Lock {
                allow_lan,
                ..Lock::default()
            },
            applied: None,
            own_executables,
        }
    }

    /// Locks exactly `apps`. On failure the previous apps stay locked.
    ///
    /// # Errors
    ///
    /// The enforcer's error, when it cannot put the new lock in force.
    pub fn set_apps(&mut self, apps: Vec<OsString>) -> Result<(), E::Error> {
        let apps = apps
            .into_iter()
            .filter(|app| !self.is_own_executable(app))
            .collect();
        self.change(|lock| lock.apps = apps)
    }

    /// # Errors
    ///
    /// The enforcer's error; the previous LAN setting stays in force.
    pub fn set_allow_lan(&mut self, allow_lan: bool) -> Result<(), E::Error> {
        self.change(|lock| lock.allow_lan = allow_lan)
    }

    /// Lets the locked apps use `tunnel_interface`, or none.
    ///
    /// # Errors
    ///
    /// The enforcer's error; the previous interface stays in force.
    pub fn follow_tunnel(&mut self, tunnel_interface: Option<String>) -> Result<(), E::Error> {
        self.change(|lock| lock.tunnel_interface = tunnel_interface)
    }

    /// Enforces the wanted lock again, even unchanged: the firewall may hold
    /// one left by an earlier run of the daemon.
    ///
    /// # Errors
    ///
    /// The enforcer's error.
    pub fn refresh(&mut self) -> Result<(), E::Error> {
        self.applied = None;
        self.change(|_| ())
    }

    /// The apps locked.
    pub fn apps(&self) -> &[OsString] {
        &self.wanted.apps
    }

    fn change(&mut self, edit: impl FnOnce(&mut Lock)) -> Result<(), E::Error> {
        let mut next = self.wanted.clone();
        edit(&mut next);
        if self.applied.as_ref() == Some(&next) {
            self.wanted = next;
            return Ok(());
        }
        self.enforcer.enforce(&next)?;
        self.applied = Some(next.clone());
        self.wanted = next;
        Ok(())
    }

    fn is_own_executable(&self, app: &OsString) -> bool {
        self.own_executables.iter().any(|own| same_path(own, app))
    }
}

/// Windows paths name a file whatever their case.
fn same_path(a: &OsString, b: &OsString) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// The lock held in WFP, by persistent filters of their own (winfw
/// `WinFw_SetLockedApps`).
#[cfg(windows)]
pub struct WinFwEnforcer;

#[cfg(windows)]
impl Enforcer for WinFwEnforcer {
    type Error = crate::firewall::Error;

    fn enforce(&mut self, lock: &Lock) -> Result<(), Self::Error> {
        crate::firewall::set_locked_apps(
            &lock.apps,
            lock.tunnel_interface.as_deref(),
            lock.allow_lan,
        )
    }
}

/// The lock held by nftables over the cgroup the locked programs are opened
/// in (`warren-include --locked`): the apps are the cgroup's, so [`Lock::apps`]
/// plays no part, and only whether a tunnel is up and the LAN shared do.
#[cfg(target_os = "linux")]
pub struct NftEnforcer {
    locked: talpid_cgroup::v2::CGroup2,
}

#[cfg(target_os = "linux")]
impl NftEnforcer {
    /// Opens the cgroup of the locked programs, creating it (and the included
    /// cgroup it sits in) when it is missing.
    ///
    /// # Errors
    ///
    /// When cgroup2 is not mounted where it is expected, or cannot be written.
    pub fn new() -> Result<Self, talpid_cgroup::Error> {
        // The fixed mount, as `warren-include` uses: it runs setuid root and
        // trusts nothing of its caller's environment.
        let locked = talpid_cgroup::v2::CGroup2::open(talpid_cgroup::CGROUP2_DEFAULT_MOUNT_PATH)?
            .create_or_open_child(talpid_cgroup::INCLUDE_CGROUP_NAME)?
            .create_or_open_child(talpid_cgroup::LOCKED_CGROUP_NAME)?;
        Ok(Self { locked })
    }
}

#[cfg(target_os = "linux")]
impl Enforcer for NftEnforcer {
    type Error = crate::firewall::Error;

    fn enforce(&mut self, lock: &Lock) -> Result<(), Self::Error> {
        crate::firewall::set_app_lock(
            &self.locked,
            lock.tunnel_interface.is_some(),
            lock.allow_lan,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        enforced: Vec<Lock>,
        fail: bool,
    }

    impl Enforcer for &mut Recorder {
        type Error = ();

        fn enforce(&mut self, lock: &Lock) -> Result<(), ()> {
            if self.fail {
                return Err(());
            }
            self.enforced.push(lock.clone());
            Ok(())
        }
    }

    fn apps(raw: &[&str]) -> Vec<OsString> {
        raw.iter().map(OsString::from).collect()
    }

    #[test]
    fn locking_apps_enforces_them_with_no_tunnel() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(&mut recorder, vec![], true);

        locks.set_apps(apps(&["/a"])).unwrap();

        drop(locks);
        assert_eq!(
            recorder.enforced,
            [Lock {
                apps: apps(&["/a"]),
                tunnel_interface: None,
                allow_lan: true,
            }]
        );
    }

    #[test]
    fn following_the_tunnel_lets_the_locked_apps_use_its_interface_and_no_other() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(&mut recorder, vec![], false);
        locks.set_apps(apps(&["/a"])).unwrap();

        locks.follow_tunnel(Some("wg0".to_owned())).unwrap();
        locks.follow_tunnel(None).unwrap();

        drop(locks);
        let interfaces: Vec<Option<&str>> = recorder
            .enforced
            .iter()
            .map(|lock| lock.tunnel_interface.as_deref())
            .collect();
        assert_eq!(interfaces, [None, Some("wg0"), None]);
    }

    #[test]
    fn an_unchanged_lock_is_not_enforced_again() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(&mut recorder, vec![], false);
        locks.set_apps(apps(&["/a"])).unwrap();

        locks.follow_tunnel(None).unwrap();
        locks.set_apps(apps(&["/a"])).unwrap();

        drop(locks);
        assert_eq!(recorder.enforced.len(), 1);
    }

    #[test]
    fn a_refresh_enforces_the_same_lock_again() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(&mut recorder, vec![], false);
        locks.set_apps(apps(&["/a"])).unwrap();

        locks.refresh().unwrap();

        drop(locks);
        assert_eq!(recorder.enforced.len(), 2);
    }

    #[test]
    fn a_lock_the_firewall_refuses_leaves_the_previous_one_wanted() {
        let mut recorder = Recorder {
            fail: true,
            ..Default::default()
        };
        let mut locks = AppLocks::new(&mut recorder, vec![], false);

        let result = locks.set_apps(apps(&["/a"]));

        assert_eq!(result, Err(()));
        assert!(locks.apps().is_empty());
    }

    #[test]
    fn the_daemon_never_locks_its_own_executable() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(&mut recorder, apps(&["/opt/warren/daemon"]), false);

        locks
            .set_apps(apps(&["/opt/warren/daemon", "/opt/browser"]))
            .unwrap();

        assert_eq!(locks.apps(), apps(&["/opt/browser"]));
    }

    #[cfg(windows)]
    #[test]
    fn its_own_executable_is_recognised_whatever_the_case_on_windows() {
        let mut recorder = Recorder::default();
        let mut locks = AppLocks::new(
            &mut recorder,
            apps(&[r"C:\Program Files\Warren\daemon.exe"]),
            false,
        );

        locks
            .set_apps(apps(&[r"c:\program files\warren\DAEMON.EXE"]))
            .unwrap();

        assert!(locks.apps().is_empty());
    }
}
