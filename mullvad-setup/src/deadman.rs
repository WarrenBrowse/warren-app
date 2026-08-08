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
//! Its verdict is the one that matters and nothing else: **is a daemon managing
//! this machine**. If one answers, the update finished and the guard removes
//! itself. If none does, the firewall has no owner and the guard resets it.
//! `reset-firewall` already refuses to run while a daemon answers, so the
//! verdict is enforced twice, independently.
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
/// channel so a beta and a prod install on one machine never disarm each other.
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
/// reading a platform script. `daemon_answers` is the whole verdict: a daemon
/// answering means the update finished and someone owns the firewall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadmanVerdict {
    /// A daemon is managing the machine: remove the guard, touch nothing else.
    DaemonAlive,
    /// Nobody is managing the machine while it is sealed: reset the firewall,
    /// then remove the guard.
    ResetFirewall,
}

/// The guard's decision. Pure on purpose (see [`DeadmanVerdict`]).
#[must_use]
pub fn deadman_verdict(daemon_answers: bool) -> DeadmanVerdict {
    if daemon_answers {
        DeadmanVerdict::DaemonAlive
    } else {
        DeadmanVerdict::ResetFirewall
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verdict rests on one question, and it is the same question
    /// `reset-firewall` already asks before it does anything. A guard that
    /// convicted on anything else (a marker file, a version, a timestamp) could
    /// tear down a firewall a live daemon is deliberately holding.
    #[test]
    fn the_verdict_is_only_ever_whether_a_daemon_owns_the_machine() {
        assert_eq!(deadman_verdict(true), DeadmanVerdict::DaemonAlive);
        assert_eq!(deadman_verdict(false), DeadmanVerdict::ResetFirewall);
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

    /// Two channels can be installed side by side, and each update must disarm
    /// only its own guard.
    #[test]
    fn each_channel_owns_a_distinct_job_name() {
        assert_ne!(deadman_job_name(""), deadman_job_name(".beta"));
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
    let dir = deadman_staging_dir(&state_dir()?);
    std::fs::create_dir_all(&dir).map_err(Error::Stage)?;
    let target = deadman_binary(&state_dir()?);
    let current = std::env::current_exe().map_err(Error::Stage)?;
    // Copy rather than symlink: the point is to outlive an `rm -rf` of the
    // bundle the running binary lives in.
    std::fs::copy(&current, &target).map_err(Error::Stage)?;
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

/// Channel suffix baked into the job name, so a beta and a prod install on one
/// machine never disarm each other.
fn channel_suffix() -> &'static str {
    // The daemon's own directories already carry it; reuse the same shape.
    if mullvad_paths::PRODUCT_NAME.contains("beta") {
        ".beta"
    } else {
        ""
    }
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
    let _ = unschedule(&job);
    unstage()
}

/// Registers a one-shot job that runs `binary deadman-fire` in `delay` seconds.
///
/// One implementation per platform because there is no portable one-shot timer,
/// and each uses the scheduler that survives a logout and a reboot: launchd on
/// macOS, a systemd transient timer on Linux, the task scheduler on Windows.
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
    let _ = std::process::Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&path)
        .status();
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Schedule(error)),
    }
}

#[cfg(target_os = "linux")]
fn schedule(job: &str, binary: &Path, delay: u64) -> Result<(), Error> {
    let status = std::process::Command::new("systemd-run")
        .arg(format!("--unit={job}"))
        .arg(format!("--on-active={delay}"))
        .arg("--timer-property=AccuracySec=1s")
        .arg(binary)
        .arg("deadman-fire")
        .status()
        .map_err(Error::Schedule)?;
    status
        .success()
        .then_some(())
        .ok_or(Error::SchedulerRefused)
}

#[cfg(target_os = "linux")]
fn unschedule(job: &str) -> Result<(), Error> {
    // Stopping the timer is enough; a transient unit disappears with it.
    let _ = std::process::Command::new("systemctl")
        .arg("stop")
        .arg(format!("{job}.timer"))
        .status();
    Ok(())
}

#[cfg(target_os = "windows")]
fn schedule(job: &str, binary: &Path, delay: u64) -> Result<(), Error> {
    // PowerShell rather than `schtasks`: the latter has no relative one-shot and
    // would need the absolute start time computed and formatted here, in the
    // machine's locale. `New-ScheduledTaskTrigger -Once -At` takes a computed
    // DateTime, so the arithmetic stays where the clock is.
    let script = format!(
        "$a = New-ScheduledTaskAction -Execute '{binary}' -Argument 'deadman-fire'; \
         $t = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddSeconds({delay})); \
         Register-ScheduledTask -TaskName '{job}' -Action $a -Trigger $t \
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
