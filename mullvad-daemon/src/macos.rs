use std::{ffi::OsStr, fmt, io, path::Path, process::Stdio};

use anyhow::{Context, anyhow};
use libc::{PROX_FDTYPE_VNODE, pid_t};
use notify::{RecursiveMode, Watcher};
use std::io::Write;
use talpid_macos::process::{
    get_file_desc_vnode_path, list_pids, process_bsdinfo, process_file_descriptors, process_path,
};
use tokio::{fs::File, process::Command};

use crate::device::AccountManagerHandle;

/// Warren app install path, per product environment (the beta app installs
/// as its own bundle next to prod).
const APP_PATH: &str = match warren_product_env::CURRENT {
    warren_product_env::ProductEnv::Prod => "/Applications/Warren VPN.app",
    warren_product_env::ProductEnv::Staging => "/Applications/Warren VPN Staging.app",
    warren_product_env::ProductEnv::Beta => "/Applications/Warren VPN Beta.app",
};

/// Uninstall script to run if the .app disappears. The file names the prod install; see
/// [`uninstall_script_for`].
const UNINSTALL_SCRIPT: &str = include_str!("../../dist-assets/uninstall_macos.sh");

/// Arguments the daemon runs [`UNINSTALL_SCRIPT`] with, as root.
const UNINSTALL_SCRIPT_ARGS: &[&str] = &[
    // Don't prompt for confirmation.
    "--yes",
    // The daemon resets the firewall and logs out itself, and the script must not run anything
    // from the app bundle as root.
    "--from-daemon",
];

/// [`UNINSTALL_SCRIPT`] with the names `env` installs under. A beta daemon running the prod
/// script would unload and delete the prod install and leave its own behind.
///
/// The same renames as `transformEnvAssetText` in `tasks/distribution.cjs`, for the names this
/// script contains. Each rule leaves a match alone when the text after it shows the name is
/// already another one, as the JS lookaheads do.
fn uninstall_script_for(env: warren_product_env::ProductEnv) -> String {
    if env == warren_product_env::ProductEnv::Prod {
        return UNINSTALL_SCRIPT.to_owned();
    }
    let suffix = format!("-{}", env.name());
    let name = env.display_name();
    let app_id = env.application_id();
    let other_env = |sep: &str| {
        let beta = format!("{sep}Beta");
        let staging = format!("{sep}Staging");
        move |rest: &str| rest.starts_with(&beta) || rest.starts_with(&staging)
    };
    let continues_name = |rest: &str| {
        rest.starts_with(|c: char| c == '.' || c == '-' || c == '_' || c.is_alphanumeric())
    };
    let continues_word =
        |rest: &str| rest.starts_with(|c: char| c == '-' || c == '_' || c.is_alphanumeric());

    let mut script = UNINSTALL_SCRIPT.to_owned();
    script = replace_unless(
        &script,
        r"Warren\ VPN",
        &name.replace(' ', r"\ "),
        other_env(r"\ "),
    );
    script = replace_unless(&script, "Warren VPN", name, other_env(" "));
    script = script.replace("com.warrenbrowse.vpn.daemon", &format!("{app_id}.daemon"));
    script = replace_unless(&script, "com.warrenbrowse.vpn", app_id, continues_name);
    script = replace_unless(
        &script,
        "/usr/local/bin/warren-problem-report",
        &format!("/usr/local/bin/warren-problem-report{suffix}"),
        continues_word,
    );
    script = replace_unless(
        &script,
        "/usr/local/bin/warren",
        &format!("/usr/local/bin/warren{suffix}"),
        continues_word,
    );
    script = replace_unless(
        &script,
        "warren-vpn",
        &format!("warren-vpn{suffix}"),
        continues_word,
    );
    script = replace_unless(
        &script,
        "/zsh/site-functions/_warren",
        &format!("/zsh/site-functions/_warren{suffix}"),
        continues_word,
    );
    script.replace(
        "/fish/vendor_completions.d/warren.fish",
        &format!("/fish/vendor_completions.d/warren{suffix}.fish"),
    )
}

/// Replaces each `needle` in `text` unless `blocked` holds for the text that follows it.
fn replace_unless(text: &str, needle: &str, with: &str, blocked: impl Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(needle) {
        let after = &rest[at + needle.len()..];
        out.push_str(&rest[..at]);
        out.push_str(if blocked(after) { needle } else { with });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Clears what would keep the kill switch up once this daemon stops: the target state, lockdown
/// and auto-connect. The block a secured target or lockdown leaves outlives the daemon on
/// purpose, and after an uninstall nothing would be left to lift it.
async fn disarm_for_uninstall(commands: &crate::DaemonEventSender<crate::DaemonCommand>) {
    use crate::DaemonCommand;
    use talpid_core::mpsc::Sender;

    let (tx, rx) = futures::channel::oneshot::channel();
    if commands
        .send(DaemonCommand::SetTargetState(
            tx,
            mullvad_types::states::TargetState::Unsecured,
        ))
        .is_ok()
    {
        let _ = rx.await;
    }
    let (tx, rx) = futures::channel::oneshot::channel();
    if commands
        .send(DaemonCommand::SetLockdownMode(tx, false))
        .is_ok()
    {
        let _ = rx.await;
    }
    let (tx, rx) = futures::channel::oneshot::channel();
    if commands
        .send(DaemonCommand::SetAutoConnect(tx, false))
        .is_ok()
    {
        let _ = rx.await;
    }
}

/// Bump filehandle limit
pub fn bump_filehandle_limit() {
    let mut limits = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `&mut limits` is a valid pointer parameter for the getrlimit syscall
    let status = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limits) };
    if status != 0 {
        log::error!(
            "Failed to get file handle limits: {}-{}",
            io::Error::from_raw_os_error(status),
            status
        );
        return;
    }

    const INCREASED_FILEHANDLE_LIMIT: u64 = 1024;
    // if file handle limit is already big enough, there's no reason to decrease it.
    if limits.rlim_cur >= INCREASED_FILEHANDLE_LIMIT {
        return;
    }

    limits.rlim_cur = INCREASED_FILEHANDLE_LIMIT;
    // SAFETY: `&limits` is a valid pointer parameter for the getrlimit syscall
    let status = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limits) };
    if status != 0 {
        log::error!(
            "Failed to set file handle limit to {}: {}-{}",
            INCREASED_FILEHANDLE_LIMIT,
            io::Error::from_raw_os_error(status),
            status
        );
    }
}

/// Detect when the app bundle is deleted
pub async fn handle_app_bundle_removal(
    account_manager_handle: AccountManagerHandle,
    commands: crate::DaemonEventSender<crate::DaemonCommand>,
) -> anyhow::Result<()> {
    /// Path to extract the uninstall script to.
    /// This directory must be owned by root to prevent privilege escalation.
    const UNINSTALL_SCRIPT_PATH: &str = "/var/root/uninstall_warren.sh";

    let mullvad_daemon = std::env::current_exe().context("Failed to get daemon path")?;
    let daemon_path = mullvad_daemon.clone();

    // Ignore app removal if the daemon isn't installed in the app directory
    if !daemon_path.starts_with(APP_PATH) {
        log::trace!("Stopping handle_app_bundle_removal as the daemon is not installed");
        return Ok(());
    }

    let (fs_notify_tx, mut fs_notify_rx) = tokio::sync::mpsc::channel(1);
    let mut fs_watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            // Ignore access events
            let is_access_event = event.map(|evt| evt.kind.is_access()).unwrap_or(false);

            // Check if the daemon binary still exists
            if !is_access_event && !daemon_path.exists() {
                _ = fs_notify_tx.try_send(());
            }
        })
        .context("Failed to start filesystem watcher")?;

    fs_watcher
        .watch(Path::new(APP_PATH), RecursiveMode::Recursive)
        .context(anyhow!("Failed to watch {APP_PATH}"))?;

    fs_notify_rx
        .recv()
        .await
        .context("Filesystem watcher stopped unexpectedly")?;
    drop(fs_watcher);

    // Create file to log output from uninstallation process.
    // This is useful since the daemon will be killed during uninstallation.
    let mut log_file = async {
        let log_path: std::path::PathBuf = mullvad_paths::log_dir()?.join("uninstall.log");

        let file = File::create(log_path).await?;
        anyhow::Ok(file.into_std().await)
    }
    .await
    .inspect_err(|e| {
        log::warn!("Failed to create uninstaller log-file: {e:#?}");
    })
    .ok();

    // Log to both daemon log and uninstaller log.
    let mut log = |msg: fmt::Arguments<'_>| {
        log::info!("{msg}");
        if let Some(log_file) = &mut log_file {
            let _ = writeln!(log_file, "{msg}");
        }
    };

    log(format_args!("{APP_PATH} was removed."));

    // TODO: This check can be removed once we no longer care about downgrades to
    // versions that didn't unload the daemon in preinstall instead of postinstall.
    // E.g., a year after we released version 2025.7
    if mullvad_installer_is_running() {
        log(format_args!(
            "Found installer process. Ignoring app removal"
        ));
        return Ok(());
    } else {
        log(format_args!(
            "Did not find installer process. Running uninstaller"
        ));
    }

    tokio::fs::write(
        UNINSTALL_SCRIPT_PATH,
        uninstall_script_for(warren_product_env::CURRENT),
    )
    .await
    .context("Failed to write uninstall script")?;

    // First, so the daemon lifts its own block and the stop the script causes leaves none.
    log(format_args!(
        "Disconnecting, and turning lockdown and auto-connect off"
    ));
    disarm_for_uninstall(&commands).await;

    // If reset_firewall errors, log the error and continue anyway.
    log(format_args!("Resetting firewall"));
    if let Err(error) = reset_firewall() {
        log(format_args!("{error:#?}"));
    }

    // Remove the current device from the account
    log(format_args!("Logging out"));
    if let Err(error) = account_manager_handle.logout().await {
        log(format_args!("Failed to remove device: {error:#?}"));
    }

    // This will kill the daemon.
    log(format_args!("Running {UNINSTALL_SCRIPT_PATH:?}"));
    let mut cmd = Command::new("/bin/bash");
    cmd
        .arg(UNINSTALL_SCRIPT_PATH)
        .args(UNINSTALL_SCRIPT_ARGS)
        // Spawn as its own process group.
        // This prevents the command from being killed when the daemon is killed.
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(log_file) = log_file {
        cmd.stdout(log_file.try_clone().context("Failed to clone log fd")?);
        cmd.stderr(log_file);
    };
    cmd.spawn().context("Failed to spawn uninstaller script")?;

    Ok(())
}

fn reset_firewall() -> anyhow::Result<()> {
    talpid_core::firewall::Firewall::new()
        .context("Failed to create firewall instance")?
        .reset_policy()
        .context("Failed to reset firewall policy")
}

/// Figure out if a Mullvad installer is active
fn mullvad_installer_is_running() -> bool {
    let Ok(pids) = list_pids() else {
        // If we can't retrieve any PIDs, assume installer isn't running
        return false;
    };
    pids.into_iter()
        .any(|pid| process_has_mullvad_installer(pid).unwrap_or(false))
}

/// Figure out if the 'pid' process is privileged and has a file open that matches a Mullvad pkg
fn process_has_mullvad_installer(pid: pid_t) -> io::Result<bool> {
    // Ignore process if it isn't running as root
    // This is because the filename is easily spoofable
    if process_bsdinfo(pid)?.pbi_uid != 0 {
        return Ok(false);
    }

    // We're only interested in installer processes
    let process_path = process_path(pid)?;
    if !process_path.starts_with("/System")
        || process_path.file_name() != Some(OsStr::new("installd"))
    {
        return Ok(false);
    }

    // Figure out if one of the file descriptors refers to a Mullvad installer
    for fd in process_file_descriptors(pid)? {
        // Only check vnodes
        if fd.proc_fdtype != PROX_FDTYPE_VNODE as u32 {
            continue;
        }

        let Ok(path) = get_file_desc_vnode_path(pid, &fd) else {
            continue;
        };

        // Check if file refers to a Mullvad .pkg
        let lower_path = path.to_bytes().to_ascii_lowercase();
        let is_pkg = lower_path.ends_with(b".pkg");
        let seq_to_find = b"mullvad";

        if is_pkg
            && lower_path
                .windows(seq_to_find.len())
                .any(|seq| seq == &seq_to_find[..])
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn an_uninstall_clears_everything_that_keeps_the_kill_switch_up() {
        use crate::{DaemonCommand, DaemonCommandChannel, InternalDaemonEvent};
        use futures::StreamExt;
        use mullvad_types::states::TargetState;

        let channel = DaemonCommandChannel::new();
        // The event sender only holds the channel weakly; the daemon keeps it alive otherwise.
        let _alive = channel.sender();
        let (events, mut received) = channel.destructure();
        let answering = tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Some(InternalDaemonEvent::Command(command)) = received.next().await {
                match command {
                    DaemonCommand::SetTargetState(tx, state) => {
                        seen.push(format!("target {state:?}"));
                        let _ = tx.send(Ok(true));
                    }
                    DaemonCommand::SetLockdownMode(tx, on) => {
                        seen.push(format!("lockdown {on}"));
                        let _ = tx.send(Ok(()));
                    }
                    DaemonCommand::SetAutoConnect(tx, on) => {
                        seen.push(format!("auto-connect {on}"));
                        let _ = tx.send(Ok(()));
                    }
                    _ => seen.push("other".to_owned()),
                }
                if seen.len() == 3 {
                    break;
                }
            }
            seen
        });

        disarm_for_uninstall(&events.to_specialized_sender()).await;
        drop(events);

        let seen = tokio::time::timeout(std::time::Duration::from_secs(5), answering)
            .await
            .expect("the disarm sent fewer than three commands")
            .unwrap();
        assert_eq!(
            seen,
            [
                format!("target {:?}", TargetState::Unsecured),
                "lockdown false".to_owned(),
                "auto-connect false".to_owned(),
            ]
        );
    }

    /// The GUI package's copy is renamed by `transformEnvAssetText`; both copies are held to the
    /// same fixtures (`product-env-uninstall.spec.ts` regenerates them).
    #[test]
    fn the_uninstall_script_carries_the_names_of_the_environment_that_runs_it() {
        use warren_product_env::ProductEnv;
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/uninstall-macos");

        assert_eq!(uninstall_script_for(ProductEnv::Prod), UNINSTALL_SCRIPT);
        for (env, fixture) in [
            (ProductEnv::Beta, "beta.sh"),
            (ProductEnv::Staging, "staging.sh"),
        ] {
            let expected = std::fs::read_to_string(fixtures.join(fixture)).unwrap();
            assert_eq!(uninstall_script_for(env), expected, "{}", env.name());
        }
    }

    /// An admin user can write into /Applications, so anything the root uninstall runs from the
    /// app bundle is a privilege escalation. The script runs with a `PATH` holding only a fake
    /// `sudo` that records each command instead of running it, so nothing real is executed.
    #[test]
    fn the_uninstall_the_daemon_runs_executes_nothing_from_the_app_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let record = dir.path().join("sudo.log");
        let sudo = bin.join("sudo");
        std::fs::write(
            &sudo,
            format!("#!/bin/sh\necho \"$1\" >> '{}'\n", record.display()),
        )
        .unwrap();
        std::fs::set_permissions(&sudo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = dir.path().join("uninstall.sh");
        std::fs::write(&script, uninstall_script_for(warren_product_env::CURRENT)).unwrap();

        let status = std::process::Command::new("/bin/bash")
            .arg(&script)
            .args(UNINSTALL_SCRIPT_ARGS)
            .env_clear()
            .env("PATH", &bin)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());

        let commands = std::fs::read_to_string(&record).unwrap();
        let from_bundle: Vec<&str> = commands
            .lines()
            .filter(|command| command.starts_with("/Applications/"))
            .collect();
        assert!(from_bundle.is_empty(), "ran {from_bundle:?} as root");
    }
}
