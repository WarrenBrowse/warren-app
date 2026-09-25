use std::{ffi::OsString, fmt, path::PathBuf};

/// A process that is being excluded from the tunnel.
#[derive(Debug, Clone)]
pub struct ExcludedProcess {
    /// Process identifier.
    pub pid: u32,
    /// Path to the image that this process is an instance of.
    pub image: PathBuf,
    /// If true, then the process is split because its parent was split,
    /// not due to its path being in the config.
    pub inherited: bool,
}

/// Which way the split tunnel diverts the apps it is given.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SplitTunnelMode {
    /// The listed apps leave outside the tunnel, everything else is tunneled.
    #[default]
    Exclude,
    /// Only the listed apps are tunneled, everything else leaves outside it.
    IncludeOnly,
}

/// What the split tunnel diverts: a mode and the apps it applies to.
///
/// On Linux the apps are chosen at launch (`warren-exclude`,
/// `warren-include`), so only the mode is used there.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct SplitApps {
    pub mode: SplitTunnelMode,
    pub apps: Vec<OsString>,
}

impl SplitApps {
    /// Whether the split tunnel must be engaged for these apps. Include-only
    /// always engages it, even for an empty list: the rest of the system
    /// still has to leave outside the tunnel.
    pub fn engages_split_tunnel(&self) -> bool {
        self.mode == SplitTunnelMode::IncludeOnly || !self.apps.is_empty()
    }
}

// The apps are paths on the user's machine: no log line renders them.
impl fmt::Debug for SplitApps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitApps")
            .field("mode", &self.mode)
            .field("apps", &self.apps.len())
            .finish()
    }
}

/// Firewall mark carried by the traffic an include-only tunnel captures on
/// Linux. Distinct from the exclusion mark (`0x6d6f6c65`) and from the
/// tunnel carrier's own mark.
#[cfg(target_os = "linux")]
pub const INCLUDE_FWMARK: u32 = 0x696e_636c;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_only_engages_the_split_tunnel_even_without_apps() {
        let include_only = SplitApps {
            mode: SplitTunnelMode::IncludeOnly,
            apps: vec![],
        };
        let no_exclusions = SplitApps::default();
        let one_exclusion = SplitApps {
            mode: SplitTunnelMode::Exclude,
            apps: vec!["/usr/bin/curl".into()],
        };

        assert!(include_only.engages_split_tunnel());
        assert!(!no_exclusions.engages_split_tunnel());
        assert!(one_exclusion.engages_split_tunnel());
    }

    #[test]
    fn the_debug_form_names_no_app() {
        let apps = SplitApps {
            mode: SplitTunnelMode::Exclude,
            apps: vec!["/home/someone/secret-app".into()],
        };

        let rendered = format!("{apps:?}");

        assert!(!rendered.contains("secret-app"), "{rendered}");
        assert!(rendered.contains("apps: 1"), "{rendered}");
    }
}
