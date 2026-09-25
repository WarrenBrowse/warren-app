//! Whether an executable belongs to an app the user chose.
//!
//! An app id and a running executable both reduce to the same key, and a
//! process belongs to the app whose key its executable reduces to:
//!
//! - macOS: an app id is a `.app` bundle path, and every executable inside the
//!   outermost bundle of a path belongs to it, so Chromium helpers and Electron
//!   renderers, which are nested bundles, count as their app. A path outside
//!   any bundle is its own key.
//! - Windows: the executable path, compared case-insensitively and with either
//!   separator, as the file system does.
//! - Linux: the executable path.

use std::{collections::HashMap, ffi::OsStr, hash::Hash};

/// The path conventions of an operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFlavor {
    MacOs,
    Windows,
    Linux,
}

impl PathFlavor {
    /// The conventions of the system this code runs on.
    pub const HOST: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Linux
    };
}

/// The key an app id or an executable path reduces to under `flavor`.
pub fn app_key(flavor: PathFlavor, path: &OsStr) -> String {
    let path = path.to_string_lossy();
    match flavor {
        PathFlavor::MacOs => macos_key(&path),
        PathFlavor::Windows => windows_key(&path),
        PathFlavor::Linux => linux_key(&path),
    }
}

fn macos_key(path: &str) -> String {
    let mut end = 0;
    for component in path.split('/') {
        end += component.len();
        let is_bundle =
            component.len() > 4 && component[component.len() - 4..].eq_ignore_ascii_case(".app");
        if is_bundle {
            return path[..end].to_owned();
        }
        end += 1;
    }
    path.to_owned()
}

fn windows_key(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    path.replace('/', "\\").to_lowercase()
}

fn linux_key(path: &str) -> String {
    // The kernel appends this to /proc/<pid>/exe once the file it ran was
    // replaced, which a package upgrade does to a running app.
    let path = path.strip_suffix(" (deleted)").unwrap_or(path);
    path.to_owned()
}

/// Maps executables to the value chosen for their app.
#[derive(Debug, Clone)]
pub struct AppMatcher<V> {
    flavor: PathFlavor,
    apps: HashMap<String, V>,
}

impl<V: Copy> AppMatcher<V> {
    /// A matcher for `apps`. On the host flavor an app id that resolves
    /// through a symbolic link also matches the executable it points to,
    /// since the OS reports a process by its resolved path.
    pub fn new<P: AsRef<OsStr>>(
        flavor: PathFlavor,
        apps: impl IntoIterator<Item = (P, V)>,
    ) -> Self {
        let mut map = HashMap::new();
        for (app, value) in apps {
            let app = app.as_ref();
            if flavor == PathFlavor::HOST
                && let Ok(resolved) = std::fs::canonicalize(app)
            {
                map.insert(app_key(flavor, resolved.as_os_str()), value);
            }
            map.insert(app_key(flavor, app), value);
        }
        Self { flavor, apps: map }
    }

    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    /// The value of the app `executable` belongs to.
    pub fn lookup(&self, executable: &OsStr) -> Option<V> {
        self.apps.get(&app_key(self.flavor, executable)).copied()
    }
}

/// A process instance: a pid is recycled, a pid with its start time is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessKey {
    pub pid: u32,
    /// Start time in the OS's own unit; only compared for equality.
    pub start_time: u64,
}

/// The decision made for each process instance, so the executable of a
/// process is looked up once however many flows it opens.
#[derive(Debug)]
pub struct DecisionCache<V> {
    decisions: HashMap<ProcessKey, Option<V>>,
    capacity: usize,
}

impl<V: Copy> DecisionCache<V> {
    /// A cache of at most `capacity` processes (at least one).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            decisions: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// `Some(decision)` when this process instance was decided before, where
    /// the decision is `None` for a process that belongs to no chosen app.
    pub fn get(&self, process: ProcessKey) -> Option<Option<V>> {
        self.decisions.get(&process).copied()
    }

    /// Records a decision; a full cache starts over rather than tracking age.
    pub fn insert(&mut self, process: ProcessKey, decision: Option<V>) {
        if self.decisions.len() >= self.capacity && !self.decisions.contains_key(&process) {
            self.decisions.clear();
        }
        self.decisions.insert(process, decision);
    }

    pub fn len(&self) -> usize {
        self.decisions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
    }

    pub fn clear(&mut self) {
        self.decisions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(flavor: PathFlavor, path: &str) -> String {
        app_key(flavor, OsStr::new(path))
    }

    #[test]
    fn a_macos_executable_reduces_to_its_outermost_bundle() {
        let helper = "/Applications/Chromium.app/Contents/Frameworks/Chromium Framework.framework/Helpers/Chromium Helper (Renderer).app/Contents/MacOS/Chromium Helper (Renderer)";

        assert_eq!(key(PathFlavor::MacOs, helper), "/Applications/Chromium.app");
        assert_eq!(
            key(PathFlavor::MacOs, "/Applications/Chromium.app/"),
            "/Applications/Chromium.app"
        );
    }

    #[test]
    fn a_macos_path_outside_any_bundle_is_its_own_key() {
        assert_eq!(key(PathFlavor::MacOs, "/usr/bin/curl"), "/usr/bin/curl");
    }

    #[test]
    fn a_windows_path_compares_case_insensitively_with_either_separator() {
        assert_eq!(
            key(
                PathFlavor::Windows,
                r"C:\Program Files\Mozilla Firefox\firefox.exe"
            ),
            key(
                PathFlavor::Windows,
                "c:/program files/mozilla firefox/FIREFOX.EXE"
            ),
        );
        assert_eq!(
            key(PathFlavor::Windows, r"\\?\C:\Tools\app.exe"),
            key(PathFlavor::Windows, r"C:\Tools\app.exe"),
        );
    }

    #[test]
    fn a_linux_executable_replaced_on_disk_keeps_its_path() {
        assert_eq!(
            key(PathFlavor::Linux, "/usr/lib/firefox/firefox (deleted)"),
            "/usr/lib/firefox/firefox"
        );
        assert_eq!(
            key(PathFlavor::Linux, "/usr/lib/firefox/FIREFOX"),
            "/usr/lib/firefox/FIREFOX"
        );
    }

    #[test]
    fn matches_every_executable_of_a_chosen_bundle_and_nothing_else() {
        let matcher = AppMatcher::new(
            PathFlavor::MacOs,
            [("/Applications/Firefox.app", 1u8), ("/usr/bin/curl", 2)],
        );

        let main = matcher.lookup(OsStr::new(
            "/Applications/Firefox.app/Contents/MacOS/firefox",
        ));
        let plugin = matcher.lookup(OsStr::new(
            "/Applications/Firefox.app/Contents/MacOS/plugin-container.app/Contents/MacOS/plugin-container",
        ));
        let curl = matcher.lookup(OsStr::new("/usr/bin/curl"));
        let other = matcher.lookup(OsStr::new(
            "/Applications/Firefox Developer Edition.app/Contents/MacOS/firefox",
        ));

        assert_eq!(
            (main, plugin, curl, other),
            (Some(1), Some(1), Some(2), None)
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_app_id_through_a_symbolic_link_matches_its_target() {
        let dir = std::env::temp_dir().join(format!("app-routing-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("real-tool");
        std::fs::write(&target, b"").unwrap();
        let link = dir.join("tool-link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let resolved = std::fs::canonicalize(&target).unwrap();

        let matcher = AppMatcher::new(PathFlavor::HOST, [(link.as_os_str(), 7u8)]);

        assert_eq!(matcher.lookup(resolved.as_os_str()), Some(7));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_matcher_says_so() {
        let empty = AppMatcher::<u8>::new(PathFlavor::Linux, Vec::<(&str, u8)>::new());
        let one = AppMatcher::new(PathFlavor::Linux, [("/bin/true", 1u8)]);

        assert!(empty.is_empty());
        assert!(!one.is_empty());
    }

    #[test]
    fn a_recycled_pid_does_not_inherit_the_decision() {
        let mut cache = DecisionCache::new(8);
        cache.insert(
            ProcessKey {
                pid: 42,
                start_time: 1000,
            },
            Some('r'),
        );

        let same = cache.get(ProcessKey {
            pid: 42,
            start_time: 1000,
        });
        let recycled = cache.get(ProcessKey {
            pid: 42,
            start_time: 2000,
        });

        assert_eq!(same, Some(Some('r')));
        assert_eq!(recycled, None);
    }

    #[test]
    fn remembers_that_a_process_belongs_to_no_chosen_app() {
        let mut cache = DecisionCache::<char>::new(8);
        cache.insert(
            ProcessKey {
                pid: 7,
                start_time: 1,
            },
            None,
        );

        assert_eq!(
            cache.get(ProcessKey {
                pid: 7,
                start_time: 1
            }),
            Some(None)
        );
    }

    #[test]
    fn a_full_cache_starts_over() {
        let mut cache = DecisionCache::new(4);
        for pid in 0..4 {
            cache.insert(ProcessKey { pid, start_time: 0 }, Some(pid));
        }

        cache.insert(
            ProcessKey {
                pid: 99,
                start_time: 0,
            },
            Some(99),
        );

        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.get(ProcessKey {
                pid: 99,
                start_time: 0
            }),
            Some(Some(99))
        );
    }
}
