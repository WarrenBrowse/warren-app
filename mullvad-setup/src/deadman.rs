//! The detached dead-man that survives a failed app update.
//!
//! Every installer arms a lockdown in the daemon's memory before it kills the
//! daemon, so the host stays sealed while the bundle is being replaced. That is
//! deliberate: it is what stops traffic escaping during the swap. What was
//! missing is what happens when the daemon never comes back. On 2026-08-08 a
//! macOS upgrade left a host with no internet, no daemon to unblock it, and no
//! app to click, because the preinstall had already deleted the bundle.
//!
//! The workspace rule this implements was bought by an earlier incident: a
//! dead-man must never depend on the thing it protects against. The daemon's own
//! timer dies with the daemon, so it cannot cover a daemon that is gone. This
//! guard therefore lives OUTSIDE the install directory (an `rm -rf` of the
//! bundle cannot take it) and runs on an OS timer (no process of ours needs to
//! survive).
//!
//! Its verdict rests first on one question: **is a daemon managing this
//! machine**. If one answers, the update finished and the guard removes
//! itself. If none does, the firewall has no owner and the guard resets it,
//! unless the user chose Lockdown mode, which promises no traffic outside the
//! tunnel even then: the block stays and the guard only removes itself.
//! `reset-firewall` already refuses to run while a daemon answers, so the
//! daemon half of the verdict is enforced twice, independently.
//!
//! Idempotent in every path: arming twice replaces the timer, firing removes
//! the guard, and disarming a guard that was never armed succeeds.

use std::path::{Path, PathBuf};

/// How long a machine may stay sealed with no daemon before the guard fires.
///
/// It has to outlast a slow installer on a slow disk without leaving a user
/// dark for long. Measured on the 2026-08-08 macOS upgrade: ten seconds from
/// preinstall to the new daemon answering. Ten minutes is two orders of margin,
/// and it is an upper bound on how long a failed update can hold a host offline
/// rather than the indefinite wait that bound used to be.
pub const DEADMAN_DELAY_SECS: u64 = 600;

/// Label / unit / task name the OS scheduler knows the guard by. Carries the
/// product environment's suffix so two environments installed on one machine
/// never unschedule each other's guard.
#[must_use]
pub fn deadman_job_name(channel_suffix: &str) -> String {
    format!("net.warrenbrowse.deadman{channel_suffix}")
}

/// Where the guard's own copy of this binary lives.
///
/// Deliberately NOT under the install directory: the macOS preinstall runs
/// `rm -rf` over the bundle after arming the lockdown, which would take the
/// recovery tool with it. Root-owned, like every other path here.
#[must_use]
pub fn deadman_staging_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("update-guard")
}

/// The staged binary's path inside [`deadman_staging_dir`].
#[must_use]
pub fn deadman_binary(state_dir: &Path) -> PathBuf {
    deadman_staging_dir(state_dir).join(if cfg!(target_os = "windows") {
        "warren-setup.exe"
    } else {
        "warren-setup"
    })
}

/// What the guard should do when its timer fires.
///
/// Split out from the IO so the decision is pinned by a test rather than by
/// reading a platform script. A daemon answering means the update finished
/// and someone owns the firewall; with none, Lockdown mode decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadmanVerdict {
    /// A daemon is managing the machine: remove the guard, touch nothing else.
    DaemonAlive,
    /// Nobody is managing the machine while it is sealed: reset the firewall,
    /// then remove the guard.
    ResetFirewall,
    /// Nobody is managing the machine, and the user chose Lockdown mode: keep
    /// the block, remove the guard. The next daemon start takes it over.
    KeepBlocked,
}

/// The guard's decision. Pure on purpose (see [`DeadmanVerdict`]).
#[must_use]
pub fn deadman_verdict(daemon_answers: bool, lockdown_mode: bool) -> DeadmanVerdict {
    match (daemon_answers, lockdown_mode) {
        (true, _) => DeadmanVerdict::DaemonAlive,
        (false, true) => DeadmanVerdict::KeepBlocked,
        (false, false) => DeadmanVerdict::ResetFirewall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verdict rests on one question, and it is the same question
    /// `reset-firewall` already asks before it does anything. A guard that
    /// convicted on anything else (a marker file, a version, a timestamp) could
    /// tear down a firewall a live daemon is deliberately holding.
    /// The Linux guard must outlive a reboot inside its window: a unit file
    /// enabled under timers.target does, a systemd-run transient timer does not.
    #[test]
    fn the_linux_guard_is_a_timer_enabled_at_boot() {
        let (service, timer) = linux_units(
            Path::new("/var/cache/warren/update-guard/warren-setup"),
            600,
        );

        assert!(
            service
                .contains("ExecStart=\"/var/cache/warren/update-guard/warren-setup\" deadman-fire")
        );
        assert!(timer.contains("OnActiveSec=600"));
        assert!(timer.contains("WantedBy=timers.target"));
    }

    #[test]
    fn a_live_daemon_is_never_overridden() {
        assert_eq!(deadman_verdict(true, false), DeadmanVerdict::DaemonAlive);
        assert_eq!(deadman_verdict(true, true), DeadmanVerdict::DaemonAlive);
    }

    #[test]
    fn with_no_daemon_the_firewall_opens_only_outside_lockdown() {
        assert_eq!(deadman_verdict(false, false), DeadmanVerdict::ResetFirewall);
        // Lockdown promises no traffic outside the tunnel, even with no daemon.
        assert_eq!(deadman_verdict(false, true), DeadmanVerdict::KeepBlocked);
    }

    /// The macOS preinstall deletes the app bundle after arming the lockdown.
    /// A guard staged inside it would be deleted with the recovery tools, which
    /// is exactly the hole this closes.
    #[test]
    fn the_guard_is_staged_outside_any_install_directory() {
        let state = Path::new("/Library/Application Support/warren-vpn-beta");
        let staged = deadman_binary(state);
        assert!(staged.starts_with(state));
        assert!(
            !staged.to_string_lossy().contains("/Applications/"),
            "a guard under the bundle dies with the bundle: {}",
            staged.display()
        );
    }

    /// Every product environment can be installed side by side with every
    /// other, and each update must disarm only its own guard. Driven through
    /// the suffix the binary actually derives, not through hand-written
    /// suffixes: a gate that spells out `""` and `".beta"` itself passes
    /// while a staging install silently shares prod's job name and the two
    /// unschedule each other.
    #[test]
    fn every_product_environment_owns_a_distinct_guard_job_name() {
        for env in warren_product_env::ALL {
            for other in warren_product_env::ALL {
                if env == other {
                    continue;
                }
                assert_ne!(
                    deadman_job_name(env.scheduler_job_suffix()),
                    deadman_job_name(other.scheduler_job_suffix()),
                    "{} and {} share a guard job name, so each disarms the other",
                    env.name(),
                    other.name()
                );
            }
        }
    }

    /// Staging replaces whatever already sits at the guard's path: a link
    /// planted there must never make the privileged copy write into the file
    /// it points at.
    #[cfg(unix)]
    #[test]
    fn staging_replaces_what_sits_at_the_target_rather_than_writing_through_it() {
        let root = std::env::temp_dir().join(format!("wguard-{}", std::process::id()));
        let state = root.join("state");
        std::fs::create_dir_all(deadman_staging_dir(&state)).unwrap();
        let victim = root.join("victim");
        std::fs::write(&victim, b"untouched").unwrap();
        std::os::unix::fs::symlink(&victim, deadman_binary(&state)).unwrap();
        let binary = root.join("binary");
        std::fs::write(&binary, b"guard").unwrap();

        let staged = stage_into(&state, &binary).unwrap();

        assert_eq!(std::fs::read(&victim).unwrap(), b"untouched");
        assert!(
            !std::fs::symlink_metadata(&staged)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&staged).unwrap(), b"guard");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The suffix this binary bakes in is its own environment's, read from
    /// the compile-time selector like every other product anchor.
    #[test]
    fn the_guard_job_name_follows_the_compiled_product_environment() {
        assert_eq!(
            channel_suffix(),
            warren_product_env::CURRENT.scheduler_job_suffix()
        );
    }
}

/// What can go wrong staging or scheduling the guard.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Failed to resolve the state directory for the update guard")]
    StateDir(#[source] mullvad_paths::Error),
    #[error("Failed to stage the update guard binary")]
    Stage(#[source] std::io::Error),
    #[cfg(windows)]
    #[error("Failed to secure the update guard's directory")]
    StageDir(#[source] mullvad_paths::Error),
    #[error("Failed to register the update guard timer")]
    Schedule(#[source] std::io::Error),
    #[error("The update guard scheduler refused the job")]
    SchedulerRefused,
}

/// Root-owned directory the guard is staged into, outside every install dir.
fn state_dir() -> Result<PathBuf, Error> {
    mullvad_paths::cache_dir().map_err(Error::StateDir)
}

/// Copies this binary next to nothing else and makes it executable.
fn stage_self() -> Result<PathBuf, Error> {
    let current = std::env::current_exe().map_err(Error::Stage)?;
    stage_into(&state_dir()?, &current)
}

/// Stages `binary` as the guard under `state_dir`.
///
/// The guard later runs with full privileges, so nothing another account put
/// in its place may survive staging: on Windows, a staging directory created
/// in advance by a standard user under ProgramData keeps that user's ACL
/// unless it is reset here, and anywhere, a file already at the target is
/// removed rather than written through. Either one failing aborts the
/// staging, which leaves the update with the in-daemon dead-man only.
fn stage_into(state_dir: &Path, binary: &Path) -> Result<PathBuf, Error> {
    let dir = deadman_staging_dir(state_dir);
    #[cfg(windows)]
    mullvad_paths::windows::create_privileged_directory(&dir).map_err(Error::StageDir)?;
    #[cfg(not(windows))]
    std::fs::create_dir_all(&dir).map_err(Error::Stage)?;
    let target = deadman_binary(state_dir);
    match std::fs::remove_file(&target) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(Error::Stage(error));
        }
        _ => {}
    }
    // Copy rather than symlink: the point is to outlive an `rm -rf` of the
    // bundle the running binary lives in.
    std::fs::copy(binary, &target).map_err(Error::Stage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))
            .map_err(Error::Stage)?;
    }
    Ok(target)
}

/// Removes the staged binary and its directory. Missing is success.
fn unstage() -> Result<(), Error> {
    let Ok(dir) = state_dir().map(|d| deadman_staging_dir(&d)) else {
        return Ok(());
    };
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Stage(error)),
    }
}

/// Suffix baked into the job name, so two product environments installed on
/// one machine never disarm each other's guard.
///
/// Read off the compiled environment rather than sniffed out of a product
/// name: a substring test for `beta` answers the same empty suffix for prod
/// and for staging, so a staging install and a prod install would share one
/// job name and each unschedule the other's guard.
fn channel_suffix() -> &'static str {
    warren_product_env::CURRENT.scheduler_job_suffix()
}

/// Stages the guard and arms its one-shot timer.
///
/// # Errors
///
/// [`Error::Stage`] if the binary cannot be copied, [`Error::Schedule`] or
/// [`Error::SchedulerRefused`] if the OS scheduler refuses the job.
pub fn arm() -> Result<(), Error> {
    let staged = stage_self()?;
    let job = deadman_job_name(channel_suffix());
    // Re-arming replaces a previous job rather than stacking a second one.
    let _ = unschedule(&job);
    schedule(&job, &staged, DEADMAN_DELAY_SECS)
}

/// Removes the timer and the staged binary. Succeeds when nothing was armed.
///
/// # Errors
///
/// [`Error::Stage`] if the staged copy exists and cannot be removed.
pub fn disarm() -> Result<(), Error> {
    let job = deadman_job_name(channel_suffix());
    // Files first: on macOS the guard disarms itself from inside its own
    // launchd job, and removing that job ends this process.
    let staged = unstage();
    let _ = unschedule(&job);
    staged
}

/// Registers a one-shot job that runs `binary deadman-fire` in `delay` seconds.
///
/// One implementation per platform because there is no portable one-shot timer.
/// Each survives a logout, and a reboot inside the window: launchd reloads the
/// daemon plist at boot (macOS), the systemd timer is a unit file enabled under
/// `timers.target` and counts from boot again (Linux), and the Windows task
/// runs as soon as it can when its start time passed while the machine was off.
#[cfg(target_os = "macos")]
fn schedule(job: &str, binary: &Path, delay: u64) -> Result<(), Error> {
    // A `StartInterval` agent would repeat; the guard removes itself on the
    // first run, so the repeat never happens in practice and the interval is
    // simply the delay. `launchd` has no true one-shot for a system daemon.
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{job}</string>
  <key>ProgramArguments</key><array>
    <string>{binary}</string><string>deadman-fire</string>
  </array>
  <key>StartInterval</key><integer>{delay}</integer>
  <key>RunAtLoad</key><false/>
</dict></plist>
"#,
        binary = binary.display()
    );
    let path = PathBuf::from(format!("/Library/LaunchDaemons/{job}.plist"));
    std::fs::write(&path, plist).map_err(Error::Schedule)?;
    let status = std::process::Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&path)
        .status()
        .map_err(Error::Schedule)?;
    status
        .success()
        .then_some(())
        .ok_or(Error::SchedulerRefused)
}

#[cfg(target_os = "macos")]
fn unschedule(job: &str) -> Result<(), Error> {
    let path = PathBuf::from(format!("/Library/LaunchDaemons/{job}.plist"));
    // The plist goes before the job: removing the job ends the guard's own
    // process when it disarms itself, and a plist left behind would be loaded
    // again at the next boot.
    let removed = match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Schedule(error)),
    };
    let _ = std::process::Command::new("launchctl")
        .args(["remove", job])
        .status();
    removed
}

/// The service and timer unit files of the Linux guard. A unit file enabled
/// under `timers.target` survives a reboot, where a `systemd-run` transient
/// timer does not; `OnActiveSec` then counts from boot again.
#[cfg_attr(not(any(test, target_os = "linux")), expect(dead_code))]
fn linux_units(binary: &Path, delay: u64) -> (String, String) {
    let service = format!(
        "[Unit]\nDescription=Warren update guard\n\n[Service]\nType=oneshot\nExecStart=\"{}\" deadman-fire\n",
        binary.display()
    );
    let timer = format!(
        "[Unit]\nDescription=Warren update guard timer\n\n[Timer]\nOnActiveSec={delay}\nAccuracySec=1s\n\n[Install]\nWantedBy=timers.target\n"
    );
    (service, timer)
}

#[cfg(target_os = "linux")]
fn linux_unit_dir() -> PathBuf {
    PathBuf::from("/etc/systemd/system")
}

#[cfg(target_os = "linux")]
fn systemctl(args: &[&str]) -> Result<(), Error> {
    let status = std::process::Command::new("systemctl")
        .args(args)
        .status()
        .map_err(Error::Schedule)?;
    status
        .success()
        .then_some(())
        .ok_or(Error::SchedulerRefused)
}

#[cfg(target_os = "linux")]
fn schedule(job: &str, binary: &Path, delay: u64) -> Result<(), Error> {
    let (service, timer) = linux_units(binary, delay);
    let dir = linux_unit_dir();
    std::fs::write(dir.join(format!("{job}.service")), service).map_err(Error::Schedule)?;
    std::fs::write(dir.join(format!("{job}.timer")), timer).map_err(Error::Schedule)?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", &format!("{job}.timer")])
}

#[cfg(target_os = "linux")]
fn unschedule(job: &str) -> Result<(), Error> {
    let _ = systemctl(&["disable", "--now", &format!("{job}.timer")]);
    let dir = linux_unit_dir();
    for unit in [format!("{job}.timer"), format!("{job}.service")] {
        match std::fs::remove_file(dir.join(unit)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Schedule(error)),
        }
    }
    // Also clears a transient timer an older guard registered.
    let _ = systemctl(&["stop", &format!("{job}.timer")]);
    let _ = systemctl(&["daemon-reload"]);
    Ok(())
}

#[cfg(target_os = "windows")]
fn schedule(job: &str, binary: &Path, delay: u64) -> Result<(), Error> {
    // PowerShell rather than `schtasks`: the latter has no relative one-shot and
    // would need the absolute start time computed and formatted here, in the
    // machine's locale. `New-ScheduledTaskTrigger -Once -At` takes a computed
    // DateTime, so the arithmetic stays where the clock is.
    // `-StartWhenAvailable`: a start time that passed while the machine was
    // off runs at the next boot instead of never.
    let script = format!(
        "$a = New-ScheduledTaskAction -Execute '{binary}' -Argument 'deadman-fire'; \
         $t = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddSeconds({delay})); \
         $s = New-ScheduledTaskSettingsSet -StartWhenAvailable; \
         Register-ScheduledTask -TaskName '{job}' -Action $a -Trigger $t -Settings $s \
           -User 'SYSTEM' -RunLevel Highest -Force | Out-Null",
        binary = binary.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(Error::Schedule)?;
    status
        .success()
        .then_some(())
        .ok_or(Error::SchedulerRefused)
}

#[cfg(target_os = "windows")]
fn unschedule(job: &str) -> Result<(), Error> {
    // `-ErrorAction SilentlyContinue`: disarming a guard that was never armed
    // is a success, and every installer calls this unconditionally.
    let script = format!(
        "Unregister-ScheduledTask -TaskName '{job}' -Confirm:$false \
         -ErrorAction SilentlyContinue"
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status();
    Ok(())
}
