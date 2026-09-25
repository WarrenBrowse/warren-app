//! In-app upgrades on Linux.
//!
//! macOS and Windows ship one installer per architecture and the GUI launches
//! it. A Linux release ships one package per format (`.deb`, `.rpm`,
//! `.pacman`), only the package manager that owns the running install can
//! upgrade it, and only root may run it. So on Linux the daemon, which already
//! runs as root and verified the download against the signed manifest, picks
//! the package of its own format and hands it to that package manager itself.
//!
//! Everything here is pure (no process is spawned), so the decisions are
//! testable on any host: which format owns the install, and the script that
//! upgrades it.

use std::ffi::OsString;
use std::path::Path;

/// The Linux package formats a release publishes an installer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageFormat {
    /// `.deb` for systemd distributions (Debian, Ubuntu, Mint, ...)
    Deb,
    /// `.deb` repacked for distributions without systemd (Devuan, MX Linux,
    /// antiX). A distinct package, `<name>-sysvinit`, which conflicts with the
    /// systemd one, so an install must be upgraded with its own flavor.
    DebSysvinit,
    /// `.rpm` (Fedora, RHEL and derivatives, openSUSE)
    Rpm,
    /// `.pacman` (Arch Linux, Manjaro, EndeavourOS)
    Pacman,
}

impl PackageFormat {
    /// The `package_format` of this format's installers in the signed manifest.
    pub const fn manifest_name(self) -> &'static str {
        match self {
            PackageFormat::Deb => "deb",
            PackageFormat::DebSysvinit => "deb-sysvinit",
            PackageFormat::Rpm => "rpm",
            PackageFormat::Pacman => "pacman",
        }
    }

    /// Extension the downloaded package is stored under. It is not cosmetic:
    /// `apt-get install` and `dnf install` only read an argument as a local
    /// file when it ends in `.deb` or `.rpm`, and treat anything else as the
    /// name of a package to fetch from a repository.
    pub const fn file_extension(self) -> &'static str {
        match self {
            PackageFormat::Deb | PackageFormat::DebSysvinit => "deb",
            PackageFormat::Rpm => "rpm",
            PackageFormat::Pacman => "pacman",
        }
    }
}

/// One way of asking a package manager which package owns a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerQuery {
    Dpkg,
    Rpm,
    Pacman,
}

impl OwnerQuery {
    /// Every query, in the order they are tried. A distribution can carry a
    /// second package manager (rpm on Debian, dpkg from the AUR on Arch), but
    /// only the one that installed the app owns its files, so the order does
    /// not decide the answer; it only decides how many queries run.
    pub const ALL: [OwnerQuery; 3] = [OwnerQuery::Dpkg, OwnerQuery::Rpm, OwnerQuery::Pacman];

    /// Program and arguments that print the package owning `path`, and exit
    /// with an error when no package owns it.
    pub fn command(self, path: &Path) -> (&'static str, Vec<OsString>) {
        let path = path.as_os_str().to_owned();
        match self {
            OwnerQuery::Dpkg => ("dpkg-query", vec!["-S".into(), path]),
            OwnerQuery::Rpm => (
                "rpm",
                vec![
                    "-qf".into(),
                    "--queryformat".into(),
                    "%{NAME}\\n".into(),
                    path,
                ],
            ),
            OwnerQuery::Pacman => ("pacman", vec!["-Qqo".into(), path]),
        }
    }

    /// The format of `package`, when the output of a query that succeeded
    /// names it as the owner. Any other owner is not an install this app can
    /// upgrade: the headless daemon package ships the same daemon binary, and
    /// upgrading it with the desktop package would replace it with the GUI.
    pub fn format(self, stdout: &str, package: &str) -> Option<PackageFormat> {
        let owner = stdout.lines().find_map(|line| {
            let line = line.trim();
            match self {
                // `name[:arch]: path`, possibly after a diversion notice.
                OwnerQuery::Dpkg => {
                    let (owner, _path) = line.split_once(": ")?;
                    let name = owner.split(':').next()?;
                    (!name.is_empty() && !name.contains(' ')).then_some(name)
                }
                OwnerQuery::Rpm | OwnerQuery::Pacman => (!line.is_empty()).then_some(line),
            }
        })?;
        match self {
            OwnerQuery::Dpkg if owner == package => Some(PackageFormat::Deb),
            OwnerQuery::Dpkg if owner.strip_suffix("-sysvinit") == Some(package) => {
                Some(PackageFormat::DebSysvinit)
            }
            OwnerQuery::Rpm if owner == package => Some(PackageFormat::Rpm),
            OwnerQuery::Pacman if owner == package => Some(PackageFormat::Pacman),
            _ => None,
        }
    }
}

/// The format of the package `package` when it owns `installed_file`, one of
/// the app's own files.
///
/// `run` executes a query and returns its standard output when it exited
/// successfully, `None` otherwise (the program is missing, or no package owns
/// the file). `None` overall means no package manager owns the install: a Nix
/// store path, a tarball, a developer build. Such an install cannot be
/// upgraded by the app and is sent to the manual path.
pub fn detect_package_format(
    installed_file: &Path,
    package: &str,
    mut run: impl FnMut(&str, &[OsString]) -> Option<String>,
) -> Option<PackageFormat> {
    // NixOS installs are declarative: even when a nix-env happens to be
    // around, the system configuration owns the version, not the app.
    if installed_file.starts_with("/nix/store") {
        return None;
    }
    OwnerQuery::ALL.into_iter().find_map(|query| {
        let (program, args) = query.command(installed_file);
        query.format(&run(program, &args)?, package)
    })
}

/// `PATH` for the upgrade script. The job runs detached from the daemon, and
/// under systemd it starts with an environment of its own, so it is spelled
/// out rather than inherited.
pub const UPGRADE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// The shell script that upgrades the install to `package`, run as root
/// detached from the daemon (the package's own scripts stop and restart the
/// daemon in the middle of it).
///
/// It appends everything the package manager prints to `log`, then writes
/// `exit <code>` to `status` in one atomic rename, which is how the app learns
/// the outcome: the daemon that started the job may no longer exist by then.
pub fn upgrade_script(
    format: PackageFormat,
    package: &Path,
    status: &Path,
    log: &Path,
    path_env: &str,
) -> String {
    let install = match format {
        // apt resolves the dependencies a new version may add, which a bare
        // `dpkg -i` leaves broken. The lock timeout waits out an unattended
        // upgrade holding the dpkg lock instead of failing on it at once.
        PackageFormat::Deb | PackageFormat::DebSysvinit => {
            r#"if command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
        -o DPkg::Lock::Timeout=600 -o Dpkg::Options::=--force-confold "$package"
else
    dpkg -i "$package"
fi"#
        }
        // The packages are not GPG-signed: their integrity comes from the
        // signed manifest's checksum, which the daemon verified. dnf checks no
        // signature on a local file by default; zypper has to be told.
        PackageFormat::Rpm => {
            r#"if command -v dnf >/dev/null 2>&1; then
    dnf install -y "$package"
elif command -v zypper >/dev/null 2>&1; then
    zypper --non-interactive install --allow-unsigned-rpm "$package"
elif command -v yum >/dev/null 2>&1; then
    yum install -y "$package"
else
    rpm -U "$package"
fi"#
        }
        PackageFormat::Pacman => r#"pacman -U --noconfirm "$package""#,
    };
    format!(
        r#"umask 022
PATH={path}
export PATH
package={package}
status={status}
log={log}
upgrade() {{
{install}
}}
{{ printf 'Upgrading from %s\n' "$package"; upgrade; }} >>"$log" 2>&1
code=$?
printf 'exit %s\n' "$code" >"$status.tmp" && mv -f "$status.tmp" "$status"
exit "$code"
"#,
        path = shell_quote(path_env),
        package = shell_quote(&package.to_string_lossy()),
        status = shell_quote(&status.to_string_lossy()),
        log = shell_quote(&log.to_string_lossy()),
    )
}

/// The outcome an upgrade job recorded in its status file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStatus {
    /// The job was started and has not finished.
    Running,
    /// The package manager exited with this code.
    Exited(i32),
}

/// The line written to the status file when a job is started.
pub const STATUS_RUNNING: &str = "running\n";

/// Parse a status file written by [`upgrade_script`] (or [`STATUS_RUNNING`]).
pub fn parse_status(contents: &str) -> Option<UpgradeStatus> {
    let line = contents.trim();
    if line == STATUS_RUNNING.trim() {
        return Some(UpgradeStatus::Running);
    }
    line.strip_prefix("exit ")?
        .parse()
        .ok()
        .map(UpgradeStatus::Exited)
}

/// Single-quote `value` for a POSIX shell.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod test {
    use super::*;

    const DAEMON: &str = "/usr/bin/warren-daemon-beta";
    const PACKAGE: &str = "warren-vpn-beta";

    /// A fake query runner answering only for `program`, with `stdout`.
    fn only(
        program: &'static str,
        stdout: &'static str,
    ) -> impl FnMut(&str, &[OsString]) -> Option<String> {
        move |p, _| (p == program).then(|| stdout.to_owned())
    }

    #[test]
    fn a_dpkg_owned_install_is_a_deb() {
        let format = detect_package_format(
            Path::new(DAEMON),
            PACKAGE,
            only(
                "dpkg-query",
                "warren-vpn-beta: /usr/bin/warren-daemon-beta\n",
            ),
        );
        assert_eq!(format, Some(PackageFormat::Deb));
    }

    #[test]
    fn the_sysvinit_flavor_is_upgraded_with_its_own_package() {
        let format = detect_package_format(
            Path::new(DAEMON),
            PACKAGE,
            only(
                "dpkg-query",
                "warren-vpn-beta-sysvinit:amd64: /usr/bin/warren-daemon-beta\n",
            ),
        );
        assert_eq!(format, Some(PackageFormat::DebSysvinit));
    }

    #[test]
    fn a_dpkg_diversion_notice_does_not_hide_the_owner() {
        let stdout = "diversion by foo from: /usr/bin/warren-daemon-beta\n\
                      warren-vpn-beta: /usr/bin/warren-daemon-beta\n";
        let format = OwnerQuery::Dpkg.format(stdout, PACKAGE);
        assert_eq!(format, Some(PackageFormat::Deb));
    }

    #[test]
    fn an_rpm_owned_install_is_an_rpm() {
        let format =
            detect_package_format(Path::new(DAEMON), PACKAGE, only("rpm", "warren-vpn-beta\n"));
        assert_eq!(format, Some(PackageFormat::Rpm));
    }

    #[test]
    fn a_pacman_owned_install_is_a_pacman() {
        let format = detect_package_format(
            Path::new(DAEMON),
            PACKAGE,
            only("pacman", "warren-vpn-beta\n"),
        );
        assert_eq!(format, Some(PackageFormat::Pacman));
    }

    #[test]
    fn an_install_no_package_manager_owns_has_no_format() {
        let format = detect_package_format(Path::new(DAEMON), PACKAGE, |_, _| None);
        assert_eq!(format, None);
    }

    #[test]
    fn a_headless_install_is_never_upgraded_with_the_desktop_package() {
        for (program, stdout) in [
            (
                "dpkg-query",
                "warren-vpn-daemon: /usr/bin/warren-daemon-beta\n",
            ),
            ("rpm", "warren-vpn-daemon\n"),
            ("pacman", "warren-vpn-daemon\n"),
        ] {
            let format = detect_package_format(Path::new(DAEMON), PACKAGE, only(program, stdout));
            assert_eq!(format, None, "{program}");
        }
    }

    #[test]
    fn another_environment_is_not_this_install() {
        // A prod and a beta install coexist; each upgrades only itself.
        let format = OwnerQuery::Rpm.format("warren-vpn\n", PACKAGE);
        assert_eq!(format, None);
    }

    #[test]
    fn a_nix_store_install_is_never_upgraded_in_place() {
        let format = detect_package_format(
            Path::new("/nix/store/abc-warren-vpn/bin/warren-daemon"),
            "warren-vpn",
            |_, _| Some("warren-vpn: /nix/store/abc-warren-vpn/bin/warren-daemon\n".to_owned()),
        );
        assert_eq!(format, None);
    }

    #[test]
    fn the_queries_name_the_installed_file() {
        for query in OwnerQuery::ALL {
            let (_, args) = query.command(Path::new(DAEMON));
            assert_eq!(args.last(), Some(&OsString::from(DAEMON)), "{query:?}");
        }
    }

    #[test]
    fn status_round_trips() {
        assert_eq!(parse_status(STATUS_RUNNING), Some(UpgradeStatus::Running));
        assert_eq!(parse_status("exit 0\n"), Some(UpgradeStatus::Exited(0)));
        assert_eq!(parse_status("exit 100\n"), Some(UpgradeStatus::Exited(100)));
        assert_eq!(parse_status("garbage"), None);
    }

    /// Runs the real upgrade script under `sh` against fake package managers.
    #[cfg(unix)]
    mod script {
        use super::super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        struct Sandbox {
            dir: tempfile::TempDir,
        }

        impl Sandbox {
            /// A sandbox whose PATH holds only `tools`, each recording its
            /// argv and exiting with `code`.
            fn with_tools(tools: &[&str], code: i32) -> Self {
                let dir = tempfile::tempdir().unwrap();
                std::fs::create_dir(dir.path().join("bin")).unwrap();
                for tool in tools {
                    let path = dir.path().join("bin").join(tool);
                    std::fs::write(
                        &path,
                        format!(
                            "#!/bin/sh\nprintf '%s' \"${{0##*/}}\" >> '{calls}'\n\
                             for a in \"$@\"; do printf ' %s' \"$a\" >> '{calls}'; done\n\
                             echo >> '{calls}'\nexit {code}\n",
                            calls = dir.path().join("calls").display()
                        ),
                    )
                    .unwrap();
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
                // The script's one external utility. Nothing else is on its
                // PATH, so a real package manager on the host is never found.
                let mv = ["/bin/mv", "/usr/bin/mv"]
                    .into_iter()
                    .find(|path| Path::new(path).exists())
                    .unwrap();
                std::os::unix::fs::symlink(mv, dir.path().join("bin").join("mv")).unwrap();
                Sandbox { dir }
            }

            fn path(&self, name: &str) -> PathBuf {
                self.dir.path().join(name)
            }

            /// Run the script for `format`, with a PATH holding only the fakes.
            fn run(&self, format: PackageFormat) -> std::process::ExitStatus {
                let path_env = self.path("bin").display().to_string();
                let script = upgrade_script(
                    format,
                    &self.path("warren 1.2.3.pkg"),
                    &self.path("status"),
                    &self.path("upgrade.log"),
                    &path_env,
                );
                std::process::Command::new("/bin/sh")
                    .arg("-c")
                    .arg(script)
                    .env_clear()
                    .status()
                    .unwrap()
            }

            fn calls(&self) -> String {
                std::fs::read_to_string(self.path("calls")).unwrap_or_default()
            }

            fn status(&self) -> Option<UpgradeStatus> {
                parse_status(&std::fs::read_to_string(self.path("status")).ok()?)
            }
        }

        #[test]
        fn a_deb_is_installed_with_apt_so_new_dependencies_resolve() {
            let sandbox = Sandbox::with_tools(&["apt-get", "dpkg"], 0);
            assert!(sandbox.run(PackageFormat::Deb).success());
            let calls = sandbox.calls();
            assert!(calls.starts_with("apt-get install -y"), "{calls}");
            assert!(calls.contains(&format!("{}", sandbox.path("warren 1.2.3.pkg").display())));
            assert!(!calls.contains("dpkg "), "{calls}");
            assert_eq!(sandbox.status(), Some(UpgradeStatus::Exited(0)));
        }

        #[test]
        fn a_deb_falls_back_to_dpkg_without_apt() {
            let sandbox = Sandbox::with_tools(&["dpkg"], 0);
            sandbox.run(PackageFormat::DebSysvinit);
            assert!(
                sandbox.calls().starts_with("dpkg -i "),
                "{}",
                sandbox.calls()
            );
        }

        #[test]
        fn an_rpm_prefers_dnf() {
            let sandbox = Sandbox::with_tools(&["dnf", "zypper", "rpm"], 0);
            sandbox.run(PackageFormat::Rpm);
            assert!(
                sandbox.calls().starts_with("dnf install -y "),
                "{}",
                sandbox.calls()
            );
        }

        #[test]
        fn zypper_is_told_the_rpm_carries_no_signature() {
            let sandbox = Sandbox::with_tools(&["zypper", "rpm"], 0);
            sandbox.run(PackageFormat::Rpm);
            let calls = sandbox.calls();
            assert!(
                calls.starts_with("zypper --non-interactive install --allow-unsigned-rpm "),
                "{calls}"
            );
        }

        #[test]
        fn a_bare_rpm_system_upgrades_with_rpm() {
            let sandbox = Sandbox::with_tools(&["rpm"], 0);
            sandbox.run(PackageFormat::Rpm);
            assert!(
                sandbox.calls().starts_with("rpm -U "),
                "{}",
                sandbox.calls()
            );
        }

        #[test]
        fn a_pacman_package_is_installed_without_a_prompt() {
            let sandbox = Sandbox::with_tools(&["pacman"], 0);
            sandbox.run(PackageFormat::Pacman);
            assert!(
                sandbox.calls().starts_with("pacman -U --noconfirm "),
                "{}",
                sandbox.calls()
            );
        }

        #[test]
        fn a_failed_install_records_its_exit_code() {
            let sandbox = Sandbox::with_tools(&["pacman"], 3);
            let status = sandbox.run(PackageFormat::Pacman);
            assert_eq!(status.code(), Some(3));
            assert_eq!(sandbox.status(), Some(UpgradeStatus::Exited(3)));
        }

        #[test]
        fn the_package_manager_output_goes_to_the_log() {
            let sandbox = Sandbox::with_tools(&["pacman"], 0);
            sandbox.run(PackageFormat::Pacman);
            let log = std::fs::read_to_string(sandbox.path("upgrade.log")).unwrap();
            assert!(log.contains("Upgrading from"), "{log}");
        }
    }
}
