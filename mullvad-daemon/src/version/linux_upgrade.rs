//! Installs a downloaded upgrade on Linux, where no installer program exists:
//! the verified package goes to the package manager that owns this install.
//!
//! The package's own scripts stop the daemon, replace it and start the new
//! one, so the package manager cannot run as a child of the daemon. Under
//! systemd a child lives in the daemon's cgroup and `systemctl stop` kills the
//! whole group, which would interrupt dpkg or rpm halfway through the upgrade.
//! The job therefore runs as a transient systemd unit of its own, or, on a
//! machine without systemd, in a session of its own. The latter still shares
//! the daemon's cgroup, so an OpenRC set to `rc_cgroup_cleanup="YES"` (off by
//! default) would kill it with the daemon; the sysvinit systems the sysvinit
//! package targets have no such cleanup.

use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mullvad_update::linux::{self, PackageFormat, UpgradeStatus};
use mullvad_update::verify::{AppVerifier, Sha256Verifier};
use tokio::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No package manager owns this install")]
    NotPackaged,

    #[error("An upgrade started by this daemon is still running")]
    InstallInProgress,

    #[error("The downloaded package no longer matches its verified checksum")]
    Verification(#[source] anyhow::Error),

    #[error("Failed to get the log directory")]
    LogDir(#[source] mullvad_paths::Error),

    #[error("Failed to write the upgrade status file")]
    WriteStatus(#[source] std::io::Error),

    #[error("Failed to start the upgrade job")]
    Spawn(#[source] std::io::Error),

    #[error("The upgrade job could not be started ({0})")]
    SpawnFailed(ExitStatus),
}

/// How long one package-manager query may take. They answer from a local
/// database in milliseconds; one that hangs must not hold up version checks.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// The package format of this install. Only a positive answer is kept: a
/// package manager changes owners only across a reinstall, which restarts the
/// daemon, but a query can fail for a moment (a daemon started by the previous
/// upgrade's scripts runs while that transaction still holds the database),
/// and caching that failure would turn in-app upgrades off until a restart.
///
/// Blocking: runs up to three package-manager queries until one succeeds.
pub fn installed_package_format() -> Option<PackageFormat> {
    static FORMAT: OnceLock<PackageFormat> = OnceLock::new();
    if let Some(format) = FORMAT.get() {
        return Some(*format);
    }
    let Some(format) = detect() else {
        log::info!("No package manager owns this install; upgrades are manual");
        return None;
    };
    if FORMAT.set(format).is_ok() {
        log::info!(
            "In-app upgrades install the {} package",
            format.manifest_name()
        );
    }
    Some(format)
}

fn detect() -> Option<PackageFormat> {
    let exe = std::env::current_exe().ok()?;
    linux::detect_package_format(&exe, warren_product_env::UNIX_PRODUCT_DIR, run_query)
}

/// Standard output of a query that exited successfully within the timeout.
fn run_query(program: &str, args: &[std::ffi::OsString]) -> Option<String> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .env("PATH", linux::UPGRADE_PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < QUERY_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                log::warn!("Package-manager query `{program}` did not answer");
                return None;
            }
        }
    };
    let mut stdout = String::new();
    child.stdout.take()?.read_to_string(&mut stdout).ok()?;
    status.success().then_some(stdout)
}

/// Where the upgrade job reports its outcome. Under `/run`, so only root can
/// write it and a reboot clears it, and world-readable, so the GUI can follow
/// a job whose daemon is being replaced under it.
pub fn status_path() -> PathBuf {
    Path::new("/run").join(format!(
        "{}-upgrade.status",
        warren_product_env::UNIX_PRODUCT_DIR
    ))
}

/// Set once this daemon has started an upgrade job, cleared if it failed to.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Install `package`, after checking once more that it is the file that was
/// verified against the signed manifest. Returns the status file's path once
/// the job has started; the job outlives this daemon.
pub async fn start(package: &Path, expected_sha256: [u8; 32]) -> Result<PathBuf, Error> {
    let format = tokio::task::spawn_blocking(installed_package_format)
        .await
        .ok()
        .flatten()
        .ok_or(Error::NotPackaged)?;
    let log = mullvad_paths::log_dir()
        .map_err(Error::LogDir)?
        .join("app-upgrade.log");
    let job = Job {
        format,
        package,
        expected_sha256,
        status: status_path(),
        log,
    };
    start_job(job, &IN_FLIGHT, spawn_detached).await
}

struct Job<'a> {
    format: PackageFormat,
    package: &'a Path,
    expected_sha256: [u8; 32],
    status: PathBuf,
    log: PathBuf,
}

/// The decisions of [`start`], with the process spawn injected.
///
/// A second request while a job is running is refused: it would overwrite
/// the status file the first job is followed through, and a second package
/// manager would fail on the first one's lock and report that failure for an
/// upgrade that is succeeding. A job counts as over once it wrote its exit.
async fn start_job<F, Fut>(job: Job<'_>, in_flight: &AtomicBool, spawn: F) -> Result<PathBuf, Error>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = std::io::Result<ExitStatus>>,
{
    if in_flight.swap(true, Ordering::SeqCst) {
        let status = tokio::fs::read_to_string(&job.status).await.ok();
        if !matches!(
            status.as_deref().and_then(linux::parse_status),
            Some(UpgradeStatus::Exited(_))
        ) {
            return Err(Error::InstallInProgress);
        }
    }

    let mut wrote_status = false;
    let result = start_owned_job(&job, &mut wrote_status, spawn).await;
    if result.is_err() {
        in_flight.store(false, Ordering::SeqCst);
        if wrote_status {
            // Nothing runs: no reader may wait on a job that never started.
            let _ = tokio::fs::remove_file(&job.status).await;
        }
    }
    result
}

async fn start_owned_job<F, Fut>(
    job: &Job<'_>,
    wrote_status: &mut bool,
    spawn: F,
) -> Result<PathBuf, Error>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = std::io::Result<ExitStatus>>,
{
    Sha256Verifier::verify(job.package, job.expected_sha256)
        .await
        .map_err(Error::Verification)?;

    let script = linux::upgrade_script(
        job.format,
        job.package,
        &job.status,
        &job.log,
        linux::UPGRADE_PATH,
    );
    tokio::fs::write(&job.status, linux::STATUS_RUNNING)
        .await
        .map_err(Error::WriteStatus)?;
    *wrote_status = true;
    set_world_readable(&job.status).await?;

    log::info!("Starting the upgrade from {}", job.package.display());
    match spawn(script).await {
        Ok(exit) if exit.success() => Ok(job.status.clone()),
        Ok(exit) => Err(Error::SpawnFailed(exit)),
        Err(error) => Err(Error::Spawn(error)),
    }
}

/// Run `script` detached from this daemon; resolves once it is started.
async fn spawn_detached(script: String) -> std::io::Result<ExitStatus> {
    let mut command = if Path::new("/run/systemd/system").exists() {
        let mut command = Command::new("systemd-run");
        command
            .arg(format!(
                "--unit={}-upgrade",
                warren_product_env::UNIX_PRODUCT_DIR
            ))
            .arg("--collect")
            .arg("--quiet")
            .arg(format!(
                "--description={} upgrade",
                warren_product_env::DISPLAY_NAME
            ))
            .arg("/bin/sh")
            .arg("-c")
            .arg(script);
        command
    } else {
        // `-f` forks and returns at once, so the job is nobody's child.
        let mut command = Command::new("setsid");
        command.arg("-f").arg("/bin/sh").arg("-c").arg(script);
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
}

async fn set_world_readable(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
        .await
        .map_err(Error::WriteStatus)
}

#[cfg(test)]
mod test {
    use super::*;
    use sha2::Digest;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::atomic::AtomicUsize;

    const PACKAGE: &[u8] = b"a verified package";

    struct Fixture {
        dir: tempfile::TempDir,
        package: PathBuf,
        in_flight: AtomicBool,
        spawned: AtomicUsize,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let package = dir.path().join("warren.deb");
            std::fs::write(&package, PACKAGE).unwrap();
            Fixture {
                dir,
                package,
                in_flight: AtomicBool::new(false),
                spawned: AtomicUsize::new(0),
            }
        }

        fn status(&self) -> PathBuf {
            self.dir.path().join("status")
        }

        fn job(&self, expected_sha256: [u8; 32]) -> Job<'_> {
            Job {
                format: PackageFormat::Deb,
                package: &self.package,
                expected_sha256,
                status: self.status(),
                log: self.dir.path().join("upgrade.log"),
            }
        }

        /// Start a job whose spawn exits with `code`.
        async fn start(&self, expected_sha256: [u8; 32], code: i32) -> Result<PathBuf, Error> {
            start_job(self.job(expected_sha256), &self.in_flight, |_script| {
                self.spawned.fetch_add(1, Ordering::SeqCst);
                async move { Ok(ExitStatus::from_raw(code << 8)) }
            })
            .await
        }

        fn status_contents(&self) -> Option<String> {
            std::fs::read_to_string(self.status()).ok()
        }
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        sha2::Sha256::digest(bytes).into()
    }

    #[tokio::test]
    async fn a_verified_package_starts_a_job_reported_as_running() {
        let fixture = Fixture::new();
        let status = fixture.start(sha256(PACKAGE), 0).await.unwrap();
        assert_eq!(status, fixture.status());
        assert_eq!(
            fixture.status_contents().as_deref(),
            Some(linux::STATUS_RUNNING)
        );
        assert_eq!(fixture.spawned.load(Ordering::SeqCst), 1);
    }

    /// The last guard before root installs the file: one that changed since it
    /// was verified is never handed to the package manager.
    #[tokio::test]
    async fn a_package_that_no_longer_matches_is_never_installed() {
        let fixture = Fixture::new();
        let error = fixture
            .start(sha256(b"another package"), 0)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Verification(_)), "{error:?}");
        assert_eq!(fixture.spawned.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.status_contents(), None);
    }

    #[tokio::test]
    async fn a_job_that_could_not_start_leaves_no_status_to_wait_on() {
        let fixture = Fixture::new();
        let error = fixture.start(sha256(PACKAGE), 1).await.unwrap_err();
        assert!(matches!(error, Error::SpawnFailed(_)), "{error:?}");
        assert_eq!(fixture.status_contents(), None);
        // and it does not block the next attempt
        fixture.start(sha256(PACKAGE), 0).await.unwrap();
    }

    #[tokio::test]
    async fn a_second_request_while_the_job_runs_is_refused_and_touches_nothing() {
        let fixture = Fixture::new();
        fixture.start(sha256(PACKAGE), 0).await.unwrap();

        let error = fixture.start(sha256(PACKAGE), 1).await.unwrap_err();

        assert!(matches!(error, Error::InstallInProgress), "{error:?}");
        assert_eq!(fixture.spawned.load(Ordering::SeqCst), 1);
        assert_eq!(
            fixture.status_contents().as_deref(),
            Some(linux::STATUS_RUNNING)
        );
    }

    /// A package manager that failed without taking the daemon down leaves it
    /// running: the next request is a retry, not a concurrent job.
    #[tokio::test]
    async fn a_finished_job_can_be_retried() {
        let fixture = Fixture::new();
        fixture.start(sha256(PACKAGE), 0).await.unwrap();
        std::fs::write(fixture.status(), "exit 100\n").unwrap();

        fixture.start(sha256(PACKAGE), 0).await.unwrap();

        assert_eq!(fixture.spawned.load(Ordering::SeqCst), 2);
    }

    /// The test binary belongs to no package, which is exactly an install the
    /// daemon must refuse to upgrade in place.
    #[tokio::test]
    async fn an_install_no_package_manager_owns_is_refused() {
        let fixture = Fixture::new();
        let error = start(&fixture.package, sha256(PACKAGE)).await.unwrap_err();
        assert!(matches!(error, Error::NotPackaged), "{error:?}");
    }

    #[test]
    fn a_query_is_run_with_the_pinned_path_and_its_output_read() {
        let output = run_query("sh", &["-c".into(), "echo $PATH".into()]);
        assert_eq!(output.as_deref().map(str::trim), Some(linux::UPGRADE_PATH));
        assert_eq!(run_query("sh", &["-c".into(), "exit 1".into()]), None);
    }
}
