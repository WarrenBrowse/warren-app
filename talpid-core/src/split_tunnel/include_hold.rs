//! Which apps the Windows firewall holds to the tunnel under include-only.
//!
//! The split tunnel driver moves the included apps onto the tunnel address,
//! but it soft-permits the apps it splits from every local address but the
//! one physical address it holds, even in the blocked states. winfw's hold (a
//! hard block of those apps off the tunnel interface) is what keeps an
//! included app that binds another address from leaving outside the tunnel,
//! so it must cover every app the driver may split for as long as it may
//! split it: every listed app from before the driver is handed its list until
//! the driver confirms a list without it, and every process the driver
//! reports splitting, a child of an included app of another executable
//! included.

use std::{ffi::OsString, fmt};

use talpid_types::split_tunnel::{SplitApps, SplitTunnelMode};

/// The listed apps held to the tunnel while `apps` is in force: the included
/// ones under include-only, none under exclusion.
pub fn held_apps(apps: &SplitApps) -> Vec<OsString> {
    match apps.mode {
        SplitTunnelMode::IncludeOnly => apps.apps.clone(),
        SplitTunnelMode::Exclude => Vec::new(),
    }
}

/// Identifies one list handed to the driver.
pub type RequestId = u64;

/// What the split tunnel driver reports that the hold must follow.
#[derive(Clone, PartialEq, Eq)]
pub enum DriverReport {
    /// The driver took the list of this request.
    Taken(RequestId),
    /// The driver did not take the list of this request.
    Refused(RequestId),
    /// The executables of every process the driver splits right now, as the
    /// NT device paths it reports.
    Splitting(Vec<OsString>),
}

// Paths are the user's: no log line renders them.
impl fmt::Debug for DriverReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Taken(id) => f.debug_tuple("Taken").field(id).finish(),
            Self::Refused(id) => f.debug_tuple("Refused").field(id).finish(),
            Self::Splitting(images) => f.debug_tuple("Splitting").field(&images.len()).finish(),
        }
    }
}

/// A list handed to the driver: the apps it holds, and whether it is an
/// include-only one.
#[derive(Clone)]
struct Request {
    id: RequestId,
    apps: Vec<OsString>,
    include_only: bool,
}

/// The apps held to the tunnel, as the union of what the driver may split.
#[derive(Default, Clone)]
pub struct Hold {
    /// The lists handed to the driver and not confirmed yet, oldest first.
    pending: Vec<Request>,
    /// The held apps of the list the driver last confirmed taking.
    taken: Vec<OsString>,
    /// Whether that list is an include-only one.
    taken_include_only: bool,
    /// The executables of the processes the driver splits, whatever the
    /// mode: under exclusion they are the excluded ones, and they become the
    /// included ones the moment the mode changes, with no new report. They
    /// are held while the driver may be splitting under include-only: until
    /// it confirms a list of the other mode, since it switches modes before
    /// it has the new list, and can then fail to take it.
    splitting: Vec<OsString>,
    next_id: RequestId,
}

impl fmt::Debug for Hold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hold")
            .field("pending", &self.pending.len())
            .field("taken", &self.taken.len())
            .field("splitting", &self.splitting.len())
            .finish()
    }
}

impl Hold {
    /// A hold for the list the driver has already taken, `apps` under
    /// `apps.mode`.
    pub fn new(apps: &SplitApps) -> Self {
        Self {
            taken: held_apps(apps),
            taken_include_only: apps.mode == SplitTunnelMode::IncludeOnly,
            ..Self::default()
        }
    }

    /// Every app held, each once.
    pub fn apps(&self) -> Vec<OsString> {
        let include_only =
            self.taken_include_only || self.pending.iter().any(|request| request.include_only);
        let splitting: &[OsString] = if include_only { &self.splitting } else { &[] };
        let mut apps: Vec<OsString> = Vec::new();
        for app in self
            .pending
            .iter()
            .flat_map(|request| &request.apps)
            .chain(&self.taken)
            .chain(splitting)
        {
            if !apps.contains(app) {
                apps.push(app.clone());
            }
        }
        apps
    }

    /// Before handing the driver `apps`: its held apps join the hold at
    /// once, and nothing leaves it until the driver confirms a list.
    pub fn request(&mut self, apps: &SplitApps) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(Request {
            id,
            apps: held_apps(apps),
            include_only: apps.mode == SplitTunnelMode::IncludeOnly,
        });
        id
    }

    /// Follows a report of the driver.
    pub fn follow(&mut self, report: DriverReport) {
        match report {
            DriverReport::Taken(id) => {
                // The driver takes the lists in order: an older one still
                // pending was either taken before this one or refused.
                if let Some(index) = self.pending.iter().position(|request| request.id == id) {
                    let taken = self
                        .pending
                        .drain(..=index)
                        .next_back()
                        .expect("drained up to index");
                    self.taken = taken.apps;
                    self.taken_include_only = taken.include_only;
                }
            }
            DriverReport::Refused(id) => self.pending.retain(|request| request.id != id),
            DriverReport::Splitting(images) => self.splitting = images,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps(names: &[&str]) -> Vec<OsString> {
        names.iter().map(OsString::from).collect()
    }

    fn include_only(names: &[&str]) -> SplitApps {
        SplitApps {
            mode: SplitTunnelMode::IncludeOnly,
            apps: apps(names),
        }
    }

    fn exclude(names: &[&str]) -> SplitApps {
        SplitApps {
            mode: SplitTunnelMode::Exclude,
            apps: apps(names),
        }
    }

    fn sorted(mut apps: Vec<OsString>) -> Vec<OsString> {
        apps.sort();
        apps
    }

    #[test]
    fn include_only_holds_the_included_apps_and_exclusion_holds_none() {
        assert_eq!(
            held_apps(&include_only(&["C:\\a.exe"])),
            apps(&["C:\\a.exe"])
        );
        assert!(held_apps(&exclude(&["C:\\a.exe"])).is_empty());
    }

    #[test]
    fn a_newly_included_app_is_held_before_the_driver_takes_it() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));

        hold.request(&include_only(&["C:\\a.exe", "C:\\b.exe"]));

        assert_eq!(sorted(hold.apps()), apps(&["C:\\a.exe", "C:\\b.exe"]));
    }

    #[test]
    fn an_app_leaving_the_list_is_released_only_once_the_driver_took_the_new_one() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe", "C:\\b.exe"]));

        let id = hold.request(&include_only(&["C:\\a.exe"]));
        let before = sorted(hold.apps());
        hold.follow(DriverReport::Taken(id));

        assert_eq!(before, apps(&["C:\\a.exe", "C:\\b.exe"]));
        assert_eq!(hold.apps(), apps(&["C:\\a.exe"]));
    }

    #[test]
    fn a_list_still_pending_stays_held_when_a_newer_one_is_requested() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));

        hold.request(&include_only(&["C:\\b.exe"]));
        hold.request(&include_only(&["C:\\c.exe"]));

        assert_eq!(
            sorted(hold.apps()),
            apps(&["C:\\a.exe", "C:\\b.exe", "C:\\c.exe"])
        );
    }

    #[test]
    fn a_confirmation_releases_the_older_lists_but_not_a_newer_one() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));
        let older = hold.request(&include_only(&["C:\\b.exe"]));
        hold.request(&include_only(&["C:\\c.exe"]));

        hold.follow(DriverReport::Taken(older));

        assert_eq!(sorted(hold.apps()), apps(&["C:\\b.exe", "C:\\c.exe"]));
    }

    #[test]
    fn a_confirmation_of_a_newer_list_releases_an_older_one_still_pending() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));
        hold.request(&include_only(&["C:\\b.exe"]));
        let newer = hold.request(&include_only(&["C:\\c.exe"]));

        hold.follow(DriverReport::Taken(newer));

        assert_eq!(hold.apps(), apps(&["C:\\c.exe"]));
    }

    #[test]
    fn a_refused_list_leaves_the_hold_on_what_the_driver_still_splits() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));

        let id = hold.request(&include_only(&["C:\\b.exe"]));
        hold.follow(DriverReport::Refused(id));

        assert_eq!(hold.apps(), apps(&["C:\\a.exe"]));
    }

    #[test]
    fn the_processes_the_driver_splits_are_held_under_include_only_only() {
        let child = apps(&["\\Device\\HarddiskVolume3\\helper.exe"]);
        let mut included = Hold::new(&include_only(&["C:\\a.exe"]));
        let mut excluded = Hold::new(&exclude(&["C:\\a.exe"]));

        included.follow(DriverReport::Splitting(child.clone()));
        excluded.follow(DriverReport::Splitting(child.clone()));

        assert!(included.apps().contains(&child[0]));
        assert!(excluded.apps().is_empty());
    }

    #[test]
    fn processes_reported_under_exclusion_are_held_once_include_only_is_requested() {
        let split = apps(&["\\Device\\HarddiskVolume3\\helper.exe"]);
        let mut hold = Hold::new(&exclude(&["C:\\a.exe"]));
        hold.follow(DriverReport::Splitting(split.clone()));

        hold.request(&include_only(&["C:\\a.exe"]));

        assert!(hold.apps().contains(&split[0]));
    }

    #[test]
    fn processes_stay_held_until_the_driver_confirms_leaving_include_only() {
        let split = apps(&["\\Device\\HarddiskVolume3\\helper.exe"]);
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));
        hold.follow(DriverReport::Splitting(split.clone()));

        let id = hold.request(&exclude(&["C:\\a.exe"]));
        let while_switching = hold.apps();
        hold.follow(DriverReport::Refused(id));
        let after_a_refusal = hold.apps();
        let id = hold.request(&exclude(&["C:\\a.exe"]));
        hold.follow(DriverReport::Taken(id));

        assert!(while_switching.contains(&split[0]));
        assert!(after_a_refusal.contains(&split[0]));
        assert!(hold.apps().is_empty());
    }

    #[test]
    fn an_app_is_held_once_whatever_lists_it() {
        let mut hold = Hold::new(&include_only(&["C:\\a.exe"]));

        hold.request(&include_only(&["C:\\a.exe"]));
        hold.follow(DriverReport::Splitting(apps(&["C:\\a.exe"])));

        assert_eq!(hold.apps(), apps(&["C:\\a.exe"]));
    }
}
