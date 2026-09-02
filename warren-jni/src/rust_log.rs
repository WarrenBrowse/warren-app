//! The Rust log file on Android, and the tee that feeds it next to logcat.
//!
//! Until this sink existed every Rust line (the JNI glue and the whole
//! engine, which logs through `tracing`'s `log` bridge) went to logcat only:
//! a ring buffer the app can read for its own process and that a reboot
//! empties. A failed forum login left no trace a user could send. The file
//! uses the desktop daemon's line format so a report reads the same to staff
//! whichever platform it came from, rotates once per process start with one
//! level of history (the daemon's `rotate_log`), and is capped so it can
//! never outgrow the report collector's per-file read limit by much.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Directory under the app's files dir holding the Rust log files.
#[cfg_attr(
    not(target_os = "android"),
    expect(dead_code, reason = "only the Android logger init reads it")
)]
pub const RUST_LOG_DIR_NAME: &str = "rust_logs";
/// The live file.
pub const RUST_LOG_FILE: &str = "warren.log";
/// The previous process's file.
pub const RUST_LOG_OLD_FILE: &str = "warren.old.log";
/// Size past which the live file is rotated in place: a long session must not
/// fill the disk, and the collector reads the tail of at most 4 MiB anyway.
pub const MAX_LIVE_BYTES: u64 = 4 * 1024 * 1024;

/// The file half of the tee. One writer, one lock: every log call site in the
/// process serialises through it, which is what the daemon's appender does.
pub struct FileSink {
    dir: PathBuf,
    writer: Mutex<Option<BufWriter<File>>>,
    written: Mutex<u64>,
    max_bytes: u64,
}

impl FileSink {
    /// Opens the sink in `dir`, rotating the previous process's file first.
    ///
    /// # Errors
    ///
    /// The io error of the directory creation or the file open.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        Self::open_capped(dir, MAX_LIVE_BYTES)
    }

    /// [`Self::open`] with an explicit cap, for the tests.
    ///
    /// # Errors
    ///
    /// The io error of the directory creation or the file open.
    pub fn open_capped(dir: &Path, max_bytes: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        rotate(dir)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(RUST_LOG_FILE))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            writer: Mutex::new(Some(BufWriter::new(file))),
            written: Mutex::new(0),
            max_bytes,
        })
    }

    /// Appends one formatted line, flushing it at once: a crash right after
    /// the line that explains it must not lose that line.
    pub fn write_line(&self, line: &str) {
        let mut written = self
            .written
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut guard = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *written + line.len() as u64 + 1 > self.max_bytes {
            // Rotate in place: the live file becomes the history, a fresh
            // file continues. Both halves stay under the collector's cap.
            if let Some(mut w) = guard.take() {
                let _ = w.flush();
            }
            if rotate(&self.dir).is_ok()
                && let Ok(file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.dir.join(RUST_LOG_FILE))
            {
                *guard = Some(BufWriter::new(file));
                *written = 0;
            }
        }
        if let Some(w) = guard.as_mut()
            && w.write_all(line.as_bytes()).is_ok()
            && w.write_all(b"\n").is_ok()
        {
            let _ = w.flush();
            *written += line.len() as u64 + 1;
        }
    }
}

/// `warren.log` becomes `warren.old.log` (replacing the previous history).
fn rotate(dir: &Path) -> std::io::Result<()> {
    let live = dir.join(RUST_LOG_FILE);
    if live.exists() {
        std::fs::rename(&live, dir.join(RUST_LOG_OLD_FILE))?;
    }
    Ok(())
}

/// One log record in the daemon's line format:
/// `[YYYY-mm-dd HH:MM:SS.mmm][target][LEVEL] message`.
#[must_use]
pub fn format_line(
    now: chrono::DateTime<chrono::Utc>,
    target: &str,
    level: log::Level,
    message: &str,
) -> String {
    format!(
        "[{}][{target}][{level}] {message}",
        now.format("%Y-%m-%d %H:%M:%S%.3f")
    )
}

/// Whether a record is worth the file. Logcat keeps everything down to
/// debug; the file keeps info and above, minus the HTTP stack's per-frame
/// chatter, which would drown the lines a report is read for.
#[must_use]
pub fn file_wants(target: &str, level: log::Level) -> bool {
    if level > log::Level::Info {
        return false;
    }
    let noisy = ["h2", "hyper", "hyper_util", "rustls", "reqwest", "tower"];
    !noisy
        .iter()
        .any(|prefix| target == *prefix || target.starts_with(&format!("{prefix}::")))
}

/// A `log::Log` that writes every record to logcat and the selected ones to
/// the file.
#[cfg_attr(
    not(target_os = "android"),
    expect(dead_code, reason = "only the Android logger init constructs it")
)]
pub struct Tee<L: log::Log> {
    pub logcat: L,
    pub file: FileSink,
}

impl<L: log::Log> log::Log for Tee<L> {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.logcat.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        self.logcat.log(record);
        if file_wants(record.target(), record.level()) {
            let line = format_line(
                chrono::Utc::now(),
                record.target(),
                record.level(),
                &record.args().to_string(),
            );
            self.file.write_line(&line);
        }
    }

    fn flush(&self) {
        self.logcat.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_process_rotates_the_previous_file_and_appends_to_a_fresh_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = FileSink::open(dir.path()).expect("opens");
        sink.write_line("first process");
        drop(sink);
        let sink = FileSink::open(dir.path()).expect("reopens");
        sink.write_line("second process");
        let live = std::fs::read_to_string(dir.path().join(RUST_LOG_FILE)).expect("live");
        let old = std::fs::read_to_string(dir.path().join(RUST_LOG_OLD_FILE)).expect("old");
        assert_eq!(live, "second process\n");
        assert_eq!(old, "first process\n");
    }

    #[test]
    fn the_live_file_rotates_in_place_past_the_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = FileSink::open_capped(dir.path(), 40).expect("opens");
        for i in 0..6 {
            sink.write_line(&format!("line number {i}"));
        }
        let live = std::fs::read_to_string(dir.path().join(RUST_LOG_FILE)).expect("live");
        let old = std::fs::read_to_string(dir.path().join(RUST_LOG_OLD_FILE)).expect("old");
        assert!(
            live.len() <= 40 + 20,
            "live stays near the cap: {}",
            live.len()
        );
        assert!(
            live.contains("line number 5"),
            "the newest line is in the live file"
        );
        assert!(old.contains("line number 0") || old.contains("line number 2"));
    }

    #[test]
    fn lines_use_the_daemon_format_and_the_file_filters_the_http_chatter() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-02T18:07:05.642Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc);
        assert_eq!(
            format_line(
                now,
                "warren_jni::forum",
                log::Level::Warn,
                "forumLogin: transport error"
            ),
            "[2026-09-02 18:07:05.642][warren_jni::forum][WARN] forumLogin: transport error"
        );
        assert!(file_wants("warren_jni", log::Level::Info));
        assert!(file_wants(
            "warrenguard_transport::path_probe",
            log::Level::Warn
        ));
        assert!(!file_wants("warren_jni", log::Level::Debug));
        assert!(!file_wants("h2::codec::framed_write", log::Level::Info));
        assert!(!file_wants("rustls::common_state", log::Level::Warn));
        assert!(
            file_wants("h2like_crate", log::Level::Info),
            "prefix match is on the path"
        );
    }
}
