use regex::Regex;
use std::{
    borrow::Cow,
    cmp::min,
    collections::{BTreeMap, HashSet},
    ffi::OsStr,
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};
use talpid_types::ErrorExt;

pub mod metadata;

/// Maximum number of bytes to read from each log file
/// How much of each log file ends up in a report, counted from the END so the
/// most recent events always survive.
///
/// Bounded by what the forum attach-logs transport accepts, not by taste:
/// warren-connect (>= 0.9.13) caps the upload at ~12 MiB of gzip and 32 MiB
/// decompressed, and Discourse accepts a 40 MB attachment. A report can carry
/// up to about seven files (daemon, its rotation, the previous install's, plus
/// the frontend main/renderer and their rotations), so 4 MiB each is 28 MiB
/// decompressed, inside that ceiling.
///
/// Raising this further needs the connect-side caps AND the Discourse setting
/// raised first, in that order, or reports get refused outright, which is
/// worse for the reporter than a truncated tail. And note what actually eats
/// the budget: a report is overwhelmingly `path probe` INFO lines from the
/// engine, so quietening that source buys far more history than any cap here.
const LOG_MAX_READ_BYTES: usize = 4 * 1024 * 1024;

/// Field delimiter in generated problem report
const LOG_DELIMITER: &str = "====================";

/// Line separator character sequence
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";

/// Custom macro to write a line to an output formatter that uses platform-specific newline
/// character sequences.
macro_rules! write_line {
    ($fmt:expr_2021 $(,)*) => { write!($fmt, "{}", LINE_SEPARATOR) };
    ($fmt:expr_2021, $pattern:expr_2021 $(, $arg:expr_2021)* $(,)*) => {
        write!($fmt, $pattern, $( $arg ),*)
            .and_then(|_| write!($fmt, "{}", LINE_SEPARATOR))
    };
}

/// These are critical errors that can happen when using the tool, that stops
/// it from working. Meaning it will print the error and exit.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to write the problem report to {path}")]
    WriteReportError {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// These are errors that can happen during problem report collection.
/// They are not critical, but they will be added inside the problem report,
/// instead of whatever content was supposed to be there.
#[derive(thiserror::Error, Debug)]
pub enum LogError {
    #[cfg(not(target_os = "android"))]
    #[error("Unable to get log directory")]
    GetLogDir(#[source] mullvad_paths::Error),

    #[error("Failed to list the files in the log directory: {path}")]
    ListLogDir {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("Error reading the contents of log file: {path}")]
    ReadLogError { path: String },
}

/// Problem report collector
#[derive(Debug, Default)]
pub struct ProblemReportCollector {
    pub extra_logs: Vec<PathBuf>,
    pub redact_custom_strings: Vec<String>,

    #[cfg(target_os = "android")]
    pub android_log_dir: PathBuf,
    #[cfg(target_os = "android")]
    pub extra_logs_dir: PathBuf,
    #[cfg(target_os = "android")]
    pub unverified_purchases: i32,
    #[cfg(target_os = "android")]
    pub pending_purchases: i32,
}

impl ProblemReportCollector {
    /// Collect the problem report and writes it to the specified path
    pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.write(open_output_file(path)?)
    }

    /// Collect the problem report and writes it to the specified output
    pub fn write(self, output: WriteSource<impl Write>) -> Result<(), Error> {
        let mut problem_report = ProblemReport::new(self.redact_custom_strings);

        let daemon_logs_dir = {
            #[cfg(target_os = "android")]
            {
                Ok(&self.android_log_dir)
            }
            #[cfg(not(target_os = "android"))]
            {
                mullvad_paths::get_log_dir().map_err(LogError::GetLogDir)
            }
        };

        let daemon_logs = daemon_logs_dir.and_then(list_logs);
        match daemon_logs {
            Ok(daemon_logs) => {
                for log in daemon_logs {
                    match log {
                        Ok(path) => problem_report.add_log(&path),
                        Err(error) => problem_report.add_error("Unable to get log path", &error),
                    }
                }
            }
            Err(error) => {
                problem_report.add_error("Failed to list logs in daemon log directory", &error)
            }
        };
        // Every candidate that answers contributes its logs. A candidate that
        // does not exist is the normal case for all but one of them, so it is
        // skipped in silence: only a report with NO frontend logs at all says
        // so, and it says it as a fact rather than as a failure.
        #[cfg(not(target_os = "android"))]
        {
            let candidates = FrontendLogDirs::from_env().candidates();
            let mut dirs_read = 0usize;
            for dir in &candidates {
                let Ok(frontend_logs) = list_logs(dir) else {
                    continue;
                };
                dirs_read += 1;
                for log in frontend_logs {
                    match log {
                        Ok(path) => problem_report.add_log(&path),
                        Err(error) => problem_report.add_error("Unable to get log path", &error),
                    }
                }
            }
            if dirs_read == 0 {
                let looked_in = candidates
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                problem_report.add_note(
                    "Frontend logs",
                    &format!(
                        "No frontend log directory found. Looked in: {}",
                        if looked_in.is_empty() {
                            "nowhere (no home or app-data directory resolved)".to_owned()
                        } else {
                            looked_in
                        }
                    ),
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            match write_logcat_to_file(&self.android_log_dir) {
                Ok(logcat_path) => problem_report.add_log(&logcat_path),
                Err(error) => problem_report.add_error("Failed to collect logcat", &error),
            }

            match list_logs(self.extra_logs_dir) {
                Ok(android_app_logs) => {
                    for log in android_app_logs {
                        match log {
                            Ok(path) => problem_report.add_log(&path),
                            Err(error) => {
                                problem_report.add_error("Unable to get log path", &error)
                            }
                        }
                    }
                }
                Err(error) => problem_report
                    .add_error("Failed to list logs in android app log directory", &error),
            }

            problem_report.add_metadata(
                "unverified-purchases".to_string(),
                self.unverified_purchases.to_string(),
            );
            problem_report.add_metadata(
                "pending-purchases".to_string(),
                self.pending_purchases.to_string(),
            );
        }

        problem_report.add_logs(self.extra_logs);

        problem_report
            .write_to(output.write)
            .map_err(|source| Error::WriteReportError {
                path: output.source,
                source,
            })
    }
}

/// A [Write] with a named source.
pub struct WriteSource<W: Write> {
    pub write: W,
    pub source: String,
}

/// Open a file to write the problem report to.
pub fn open_output_file(path: impl AsRef<Path>) -> Result<WriteSource<BufWriter<File>>, Error> {
    fn inner(path: impl AsRef<Path>) -> io::Result<BufWriter<File>> {
        let file = File::create(path)?;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
        Ok(BufWriter::new(file))
    }

    let file_path = path.as_ref().display().to_string();

    let write = inner(path).map_err(|source| Error::WriteReportError {
        path: file_path.clone(),
        source,
    })?;

    Ok(WriteSource {
        write,
        source: file_path,
    })
}

impl<W: Write> From<(W, String)> for WriteSource<W> {
    fn from((write, source): (W, String)) -> Self {
        WriteSource { write, source }
    }
}

/// Returns an iterator over all files in the given directory that has the `.log` extension.
fn list_logs(
    log_dir: impl AsRef<Path>,
) -> Result<impl Iterator<Item = Result<PathBuf, LogError>>, LogError> {
    fs::read_dir(log_dir.as_ref())
        .map_err(|source| LogError::ListLogDir {
            path: log_dir.as_ref().display().to_string(),
            source,
        })
        .map(|dir_entries| {
            let log_extension = Some(OsStr::new("log"));

            dir_entries.filter_map(move |dir_entry_result| match dir_entry_result {
                Ok(dir_entry) => {
                    let path = dir_entry.path();

                    if path.extension() == log_extension {
                        Some(Ok(path))
                    } else {
                        None
                    }
                }
                Err(source) => Some(Err(LogError::ListLogDir {
                    path: log_dir.as_ref().display().to_string(),
                    source,
                })),
            })
        })
}

/// Returns the directory where the Warren GUI frontend stores its logs.
/// If the current platform has a separate directory for frontend logs.
//
// These paths must match the Electron frontend's resolved `logs` dir, which
// derives from the app `productName` (the per-environment display name,
// "Warren VPN" for prod). If they drift, problem reports silently omit the
// GUI logs.
/// The platform whose log layout to resolve. Explicit so every arm is
/// reachable from a test on any host.
#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetOs {
    Linux,
    MacOs,
    Windows,
    Other,
}

#[cfg(not(target_os = "android"))]
impl TargetOs {
    const HOST: Self = if cfg!(target_os = "linux") {
        Self::Linux
    } else if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(target_os = "windows") {
        Self::Windows
    } else {
        Self::Other
    };
}

/// Where the desktop frontend may have put its logs, on any platform.
///
/// Deliberately several paths instead of one: Electron resolves its own base at
/// runtime (`XDG_CONFIG_HOME` on Linux, the roaming/local split on Windows,
/// `Library/Logs` vs the app-support dir on macOS), so a single hardcoded guess
/// is one environment away from dropping the whole frontend half of a report.
/// That is not hypothetical: a beta report from a Linux tester carried an error
/// block where the frontend logs should have been, because only
/// `~/.config/<display name>/logs` was ever looked at.
///
/// Built from explicit inputs rather than read straight from the environment so
/// the resolution is testable without touching the machine's real env.
#[cfg(not(target_os = "android"))]
#[derive(Debug, Default, Clone)]
struct FrontendLogDirs {
    home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    roaming_app_data: Option<PathBuf>,
}

#[cfg(not(target_os = "android"))]
impl FrontendLogDirs {
    fn from_env() -> Self {
        Self {
            home: dirs::home_dir(),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            local_app_data: std::env::var_os("LOCALAPPDATA")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            roaming_app_data: std::env::var_os("APPDATA")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }

    /// Candidate directories for the host platform.
    fn candidates(&self) -> Vec<PathBuf> {
        self.candidates_for(TargetOs::HOST)
    }

    /// Candidate directories, most likely first, deduplicated.
    ///
    /// The platform is a parameter, not a `#[cfg]`: the bug this exists to fix
    /// happened on Linux, and a Linux-only `cfg` arm is untestable from a macOS
    /// or Windows dev machine, which is exactly how it stayed broken.
    /// Directory names the frontend may have used, this environment first.
    ///
    /// Every environment's names are included, not just the current one: an
    /// install predating the per-channel rename writes under the prod name
    /// while a beta-built collector looks for the beta name, and that skew
    /// alone loses the frontend logs. Each log block is labelled with its full
    /// path in the report, so a stray section is readable rather than
    /// confusing.
    fn app_dir_names() -> Vec<&'static str> {
        let mut names = vec![
            warren_product_env::DISPLAY_NAME,
            warren_product_env::UNIX_PRODUCT_DIR,
        ];
        for env in warren_product_env::ALL {
            names.push(env.display_name());
            names.push(env.unix_product_dir());
        }
        names.dedup();
        let mut seen: Vec<&'static str> = Vec::with_capacity(names.len());
        names.retain(|n| {
            if seen.contains(n) {
                false
            } else {
                seen.push(n);
                true
            }
        });
        names
    }

    fn candidates_for(&self, os: TargetOs) -> Vec<PathBuf> {
        let names = Self::app_dir_names();
        let mut out: Vec<PathBuf> = Vec::new();

        if os == TargetOs::Linux {
            // Electron honours XDG_CONFIG_HOME; when it is unset it falls back
            // to ~/.config, so both are live possibilities on the same distro.
            for name in &names {
                if let Some(xdg) = &self.xdg_config_home {
                    out.push(xdg.join(name).join("logs"));
                }
                if let Some(home) = &self.home {
                    out.push(home.join(".config").join(name).join("logs"));
                }
            }
        }

        if os == TargetOs::MacOs
            && let Some(home) = &self.home
        {
            for name in &names {
                out.push(home.join("Library/Logs").join(name));
                out.push(
                    home.join("Library/Application Support")
                        .join(name)
                        .join("logs"),
                );
            }
        }

        if os == TargetOs::Windows {
            for name in &names {
                if let Some(local) = &self.local_app_data {
                    out.push(local.join(name).join("logs"));
                }
                // The app forces LOCALAPPDATA, but an install predating that
                // override, or one started before it runs, uses roaming.
                if let Some(roaming) = &self.roaming_app_data {
                    out.push(roaming.join(name).join("logs"));
                }
            }
        }

        let mut seen = Vec::with_capacity(out.len());
        out.retain(|p| {
            if seen.contains(p) {
                false
            } else {
                seen.push(p.clone());
                true
            }
        });
        out
    }
}

#[cfg(target_os = "android")]
fn write_logcat_to_file(log_dir: &Path) -> Result<PathBuf, io::Error> {
    use std::process::{Command, Stdio};

    let logcat_path = log_dir.join("logcat.txt");
    let logcat_file = File::create(&logcat_path)?;
    let _stderr = logcat_file.try_clone()?;
    let stdout = Stdio::from(logcat_file);
    let stderr = Stdio::from(_stderr);

    let _output = Command::new("logcat")
        .arg("-d")
        .stdout(stdout)
        .stderr(stderr)
        .output()?;
    Ok(logcat_path)
}

#[derive(Debug)]
struct ProblemReport {
    metadata: BTreeMap<String, String>,
    logs: Vec<(String, String)>,
    log_paths: HashSet<PathBuf>,
    redact_custom_strings: Vec<String>,
}

impl ProblemReport {
    /// Creates a new problem report with system information. Logs can be added with `add_log`.
    /// Logs will have all strings in `redact_custom_strings` removed from them.
    pub fn new(mut redact_custom_strings: Vec<String>) -> Self {
        redact_custom_strings.retain(|redact| !redact.is_empty());

        ProblemReport {
            metadata: metadata::collect(),
            logs: Vec::new(),
            log_paths: HashSet::new(),
            redact_custom_strings,
        }
    }

    /// Add extra metadata to the problem report that is not possible to access from the daemon
    /// directly.
    #[cfg(target_os = "android")]
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Attach some file logs to this report. This method adds the error chain instead of the log
    /// contents if an error occurs while reading one of the log files.
    pub fn add_logs<I>(&mut self, paths: I)
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        for path in paths {
            self.add_log(path.as_ref());
        }
    }

    /// Attach a file log to this report. This method adds the error chain instead of the log
    /// contents if an error occurs while reading the log file.
    pub fn add_log(&mut self, path: &Path) {
        let expanded_path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if self.log_paths.insert(expanded_path.clone()) {
            let redacted_path = self.redact(&expanded_path.to_string_lossy());
            let content = self.redact(&read_file_lossy(path, LOG_MAX_READ_BYTES).unwrap_or_else(
                |error| {
                    error.display_chain_with_msg(&format!(
                        "Error reading the contents of log file: {}",
                        expanded_path.display()
                    ))
                },
            ));
            self.logs.push((redacted_path, content));
            log::info!("Adding {}", expanded_path.display());
        }
    }

    /// Attach an error to the report.
    /// Adds an informational block. Unlike [`Self::add_error`] this does not
    /// describe a failure: some states (a frontend that never wrote a log) are
    /// normal, and dressing them as errors sends staff chasing a non-problem.
    #[cfg(not(target_os = "android"))]
    pub fn add_note(&mut self, title: &'static str, body: &str) {
        let redacted = self.redact(body);
        self.logs.push((title.to_string(), redacted));
    }

    pub fn add_error(&mut self, message: &'static str, error: &impl ErrorExt) {
        let redacted_error = self.redact(&error.display_chain());
        self.logs.push((message.to_string(), redacted_error));
    }

    fn redact(&self, input: &str) -> String {
        let out1 = Self::redact_account_number(input);
        let out2 = Self::redact_home_dir(&out1);
        let out3 = Self::redact_network_info(&out2);
        let out4 = Self::redact_uuid_v4(&out3);
        self.redact_custom_strings(&out4).to_string()
    }

    fn redact_account_number(input: &str) -> Cow<'_, str> {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("\\d{16}").unwrap());
        RE.replace_all(input, "[REDACTED ACCOUNT NUMBER]")
    }

    fn redact_home_dir(input: &str) -> Cow<'_, str> {
        redact_home_dir_inner(input, dirs::home_dir()).into()
    }

    fn redact_network_info(input: &str) -> Cow<'_, str> {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            let boundary = "[^0-9a-zA-Z.:]";
            let combined_pattern = format!(
                "(?P<start>^|{})(?:{}|{}|{})",
                boundary,
                build_ipv4_regex(),
                build_ipv6_regex(),
                build_mac_regex(),
            );
            Regex::new(&combined_pattern).unwrap()
        });
        RE.replace_all(input, "$start[REDACTED]")
    }

    /// Redact all v4 UUIDs, including:
    /// * Account IDs
    /// * Device IDs
    /// * Network interface GUIDs on Windows
    fn redact_uuid_v4(input: &str) -> Cow<'_, str> {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?i)\{?[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}?")
                .unwrap()
        });
        RE.replace_all(input, "[REDACTED]")
    }

    fn redact_custom_strings<'a>(&self, input: &'a str) -> Cow<'a, str> {
        // Can probably me made a lot faster with aho-corasick if optimization is ever needed.
        let mut out = Cow::from(input);
        for redact in &self.redact_custom_strings {
            out = out.replace(redact, "[REDACTED]").into()
        }
        out
    }

    fn write_to<W: Write>(&self, mut output: W) -> io::Result<()> {
        // IMPORTANT: Make sure this implementation stays in sync with `parse_metadata` below.
        write_line!(output, "System information:")?;
        for (key, value) in &self.metadata {
            write_line!(output, "{}: {}", key, value)?;
        }
        // Write empty line to separate metadata from first log
        write_line!(output)?;
        for (label, content) in &self.logs {
            write_line!(output, "{}", LOG_DELIMITER)?;
            write_line!(output, "Log: {}", label)?;
            write_line!(output, "{}", LOG_DELIMITER)?;
            output.write_all(content.as_bytes())?;
            write_line!(output)?;
        }
        Ok(())
    }

    /// Tries to parse out the metadata map from a string that is supposed to be a report written by
    /// this struct. Only exercised by tests since the in-app send path was
    /// removed, but kept: it pins the report header format `write_to` emits.
    #[cfg(test)]
    pub fn parse_metadata(report: &str) -> Option<BTreeMap<String, String>> {
        // IMPORTANT: Make sure this implementation stays in sync with `write_to` above.
        const PATTERN: &str = ": ";
        let mut lines = report.lines();
        if lines.next() != Some("System information:") {
            return None;
        }
        let mut metadata = BTreeMap::new();
        for line in lines {
            // Abort on first empty line, as this is the separator between the metadata and the
            // first log
            if line.is_empty() {
                break;
            }
            let split_i = line.find(PATTERN)?;
            let key = &line[..split_i];
            let value = &line[split_i + PATTERN.len()..];
            metadata.insert(key.to_owned(), value.to_owned());
        }
        Some(metadata)
    }
}

fn redact_home_dir_inner(input: &str, home_dir: Option<PathBuf>) -> String {
    #[cfg(target_os = "windows")]
    {
        // Redact all paths that match:
        // - <drive letter>:\Users\<username>
        // - \Device\HarddiskVolumeX\Users\<username>
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?i)(?:[A-Z]:\\Users\\[^\\]+|\\Device\\[^\\]+\\Users\\[^\\]+)").unwrap()
        });

        let mut out = RE.replace_all(input, "~").into_owned();

        if let Some(home) = home_dir {
            out = out.replace(home.to_string_lossy().as_ref(), "~");

            // Also redact equivalent paths that use a device prefix instead of a drive letter.
            let mut home = home;
            let prefix = home.components().next();
            if let Some(prefix @ std::path::Component::Prefix(_)) = prefix.as_ref() {
                home = home.strip_prefix(prefix).unwrap().to_path_buf();
            }
            let expr = format!(r"[\w\\]+{}", regex::escape(&home.display().to_string()));
            let regex = Regex::new(&expr).unwrap();

            out = regex.replace_all(&out, "~").to_string();
        }

        out
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Redact all paths that match:
        // - /home/<username>
        // - /Users/<username>
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)(?:/home/[^/]+|/Users/[^/]+)").unwrap());

        let mut out = RE.replace_all(input, "~").into_owned();

        if let Some(home) = home_dir {
            out = out.replace(home.to_string_lossy().as_ref(), "~");
        }

        out
    }
}

fn build_mac_regex() -> String {
    let octet = "[[:xdigit:]]{2}"; // 0 - ff

    // five pairs of two hexadecimal chars followed by colon or dash
    // followed by a pair of hexadecimal chars
    format!("(?:{octet}[:-]){{5}}({octet})")
}

fn build_ipv4_regex() -> String {
    // regex adapted from  https://www.regular-expressions.info/ip.html

    let above_250 = "25[0-5]";
    let above_200 = "2[0-4][0-9]";
    let above_100 = "1[0-9][0-9]";

    // 100-119 | 120-126 | 128-129 | 130 - 199
    let above_100_not_127 = "1(?:[01][0-9]|2[0-6]|2[89]|[3-9][0-9])";

    let above_0 = "0?[0-9][0-9]?";

    // matches 0-255, except 127
    let first_octet = format!("(?:{above_250}|{above_200}|{above_100_not_127}|{above_0})");

    // matches 0-255
    let ip_octet = format!("(?:{above_250}|{above_200}|{above_100}|{above_0})");

    format!("(?:{first_octet}\\.{ip_octet}\\.{ip_octet}\\.{ip_octet})")
}

fn build_ipv6_regex() -> String {
    // Regular expression obtained from:
    // https://stackoverflow.com/a/17871737
    let ipv4_segment = "(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])";
    let ipv4_address = format!("({ipv4_segment}\\.){{3,3}}{ipv4_segment}");

    let ipv6_segment = "[0-9a-fA-F]{1,4}";

    let long = format!("({ipv6_segment}:){{7,7}}{ipv6_segment}");
    let compressed_1 = format!("({ipv6_segment}:){{1,7}}:");
    let compressed_2 = format!("({ipv6_segment}:){{1,6}}:{ipv6_segment}");
    let compressed_3 = format!("({ipv6_segment}:){{1,5}}(:{ipv6_segment}){{1,2}}");
    let compressed_4 = format!("({ipv6_segment}:){{1,4}}(:{ipv6_segment}){{1,3}}");
    let compressed_5 = format!("({ipv6_segment}:){{1,3}}(:{ipv6_segment}){{1,4}}");
    let compressed_6 = format!("({ipv6_segment}:){{1,2}}(:{ipv6_segment}){{1,5}}");
    let compressed_7 = format!("{ipv6_segment}:((:{ipv6_segment}){{1,6}})");
    let compressed_8 = format!(":((:{ipv6_segment}){{1,7}}|:)");
    let link_local = "[Ff][Ee]80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]{1,}";
    let ipv4_mapped = format!("::([fF]{{4}}(:0{{1,4}}){{0,1}}:){{0,1}}{ipv4_address}");
    let ipv4_embedded = format!("({ipv6_segment}:){{1,4}}:{ipv4_address}");

    format!(
        "{long}|{link_local}|{ipv4_mapped}|{ipv4_embedded}|{compressed_8}|{compressed_7}|{compressed_6}|{compressed_5}|{compressed_4}|{compressed_3}|{compressed_2}|{compressed_1}",
    )
}

/// Helper to lossily read a file to a `String`. If the file size exceeds the given `max_bytes`,
/// only the last `max_bytes` bytes of the file are read.
fn read_file_lossy(path: &Path, max_bytes: usize) -> io::Result<String> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let truncated = file_size > max_bytes as u64;

    if truncated {
        file.seek(SeekFrom::Start(file_size - max_bytes as u64))?;
    }

    let capacity = min(file_size, max_bytes as u64) as usize;
    let mut buffer = Vec::with_capacity(capacity);
    file.take(max_bytes as u64).read_to_end(&mut buffer)?;
    let content = String::from_utf8_lossy(&buffer).into_owned();
    if !truncated {
        return Ok(content);
    }
    // Say it out loud. Otherwise the only hint that a log was cut is its first
    // line arriving half-written, which reads as corruption rather than as a
    // deliberate tail, and hides that earlier events exist on the machine.
    Ok(format!(
        "[report] this log was truncated: showing the last {} bytes of {}\n{}",
        max_bytes, file_size, content
    ))
}

#[cfg(test)]
mod tests {

    fn every_base() -> FrontendLogDirs {
        FrontendLogDirs {
            home: Some(PathBuf::from("/home/tester")),
            xdg_config_home: Some(PathBuf::from("/home/tester/.myconfig")),
            local_app_data: Some(PathBuf::from(r"C:\Users\tester\AppData\Local")),
            roaming_app_data: Some(PathBuf::from(r"C:\Users\tester\AppData\Roaming")),
        }
    }

    fn joined(dirs: &FrontendLogDirs, os: TargetOs) -> String {
        dirs.candidates_for(os)
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn a_truncated_log_says_so_instead_of_starting_mid_line() {
        // A cut tail used to surface only as a half-written first line, which
        // reads as corruption and hides that earlier events still exist.
        let dir = std::env::temp_dir().join(format!("warren-report-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let path = dir.join("daemon.log");
        let body: String = (0..2000).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&path, &body).expect("write");
        let cap = 512;

        let out = read_file_lossy(&path, cap).expect("read");
        assert!(
            out.starts_with("[report] this log was truncated"),
            "{out:.80}"
        );
        assert!(
            out.contains(&format!("of {}", body.len())),
            "the full size is stated: {out:.120}"
        );
        assert!(
            out.contains("line 1999"),
            "the tail is what survives, not the head"
        );
        assert!(!out.contains("line 0\n"), "the head is dropped");

        // Under the cap: content is passed through untouched.
        let small = dir.join("small.log");
        std::fs::write(&small, "just this\n").expect("write");
        assert_eq!(read_file_lossy(&small, cap).expect("read"), "just this\n");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_install_predating_the_per_channel_rename_is_still_found() {
        // A beta installed before the rename writes under the prod name while a
        // beta-built collector looks for the beta one. That skew alone lost the
        // whole frontend half of a report.
        let names = FrontendLogDirs::app_dir_names();
        for env in warren_product_env::ALL {
            assert!(
                names.contains(&env.display_name()),
                "{} missing from {names:?}",
                env.display_name()
            );
            assert!(
                names.contains(&env.unix_product_dir()),
                "{} missing from {names:?}",
                env.unix_product_dir()
            );
        }
        assert_eq!(
            names[0],
            warren_product_env::DISPLAY_NAME,
            "this build's own name is tried first"
        );
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "no name repeats: {names:?}");
    }

    #[test]
    fn linux_looks_where_electron_actually_writes() {
        // The real failure: only ~/.config/<display name>/logs was ever tried,
        // so a machine whose Electron used another base reported an error block
        // instead of the frontend logs.
        let out = joined(&every_base(), TargetOs::Linux);
        assert!(
            out.contains("/home/tester/.myconfig/"),
            "XDG_CONFIG_HOME is what Electron honours: {out}"
        );
        assert!(
            out.contains("/home/tester/.config/"),
            "the XDG default stays a candidate: {out}"
        );
        assert!(
            out.contains(warren_product_env::UNIX_PRODUCT_DIR),
            "the kebab-case dir covers an Electron name that fell back to the \
             package name: {out}"
        );
        assert!(
            out.contains(warren_product_env::DISPLAY_NAME),
            "and the product name itself: {out}"
        );
    }

    #[test]
    fn macos_and_windows_cover_both_of_their_bases() {
        let dirs = every_base();
        let mac = joined(&dirs, TargetOs::MacOs);
        assert!(mac.contains("Library/Logs"), "{mac}");
        assert!(mac.contains("Library/Application Support"), "{mac}");

        let win = joined(&dirs, TargetOs::Windows);
        assert!(win.contains("AppData\\Local"), "{win}");
        assert!(
            win.contains("AppData\\Roaming"),
            "an install predating the LOCALAPPDATA override kept them in \
             roaming: {win}"
        );
    }

    #[test]
    fn every_platform_yields_somewhere_to_look() {
        // No platform may come back empty-handed when a base is known, or that
        // platform silently ships reports with no frontend logs.
        for os in [TargetOs::Linux, TargetOs::MacOs, TargetOs::Windows] {
            assert!(
                !every_base().candidates_for(os).is_empty(),
                "{os:?} resolved nothing"
            );
        }
    }

    #[test]
    fn frontend_log_candidates_never_repeat_a_directory() {
        // XDG_CONFIG_HOME set to exactly the default must not make the report
        // read, and append, the same log files twice.
        let dirs = FrontendLogDirs {
            home: Some(PathBuf::from("/home/tester")),
            xdg_config_home: Some(PathBuf::from("/home/tester/.config")),
            ..Default::default()
        };
        let candidates = dirs.candidates_for(TargetOs::Linux);
        let mut unique = candidates.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            candidates.len(),
            unique.len(),
            "duplicate candidate: {candidates:?}"
        );
    }

    #[test]
    fn no_base_at_all_yields_no_candidate() {
        // No home and no app-data: nothing to guess. The caller turns this into
        // a plain note, never an error block.
        for os in [TargetOs::Linux, TargetOs::MacOs, TargetOs::Windows] {
            assert!(FrontendLogDirs::default().candidates_for(os).is_empty());
        }
    }

    #[test]
    fn a_note_is_not_dressed_as_an_error() {
        let mut report = ProblemReport::new(vec![]);
        report.add_note("Frontend logs", "No frontend log directory found.");
        let mut buf: Vec<u8> = Vec::new();
        report.write_to(&mut buf).expect("render");
        let rendered = String::from_utf8_lossy(&buf).into_owned();
        assert!(rendered.contains("Frontend logs"), "{rendered}");
        assert!(
            !rendered.contains("Error:"),
            "a normal state must not read as a failure: {rendered}"
        );
    }
    use super::*;

    #[test]
    fn redacts_ipv4() {
        assert_redacts("1.2.3.4");
        assert_redacts("10.127.0.1");
        assert_redacts("192.168.1.1");
        assert_redacts("10.0.16.1");
        assert_redacts("173.54.12.32");
        assert_redacts("68.4.4.1");
    }

    #[test]
    fn does_not_redact_localhost_ipv4() {
        assert_does_not_redact("127.0.0.1");
    }

    #[test]
    fn redacts_ipv6() {
        assert_redacts("2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert_redacts("2001:db8:85a3:0:0:8a2e:370:7334");
        assert_redacts("2001:db8:85a3::8a2e:370:7334");
        assert_redacts("2001:db8:0:0:0:0:2:1");
        assert_redacts("2001:db8::2:1");
        assert_redacts("2001:db8:0000:1:1:1:1:1");
        assert_redacts("2001:db8:0:1:1:1:1:1");
        assert_redacts("2001:db8:0:0:1:0:0:1");
        assert_redacts("2001:db8::1:0:0:1");
        assert_redacts("abcd:dead:beef::");
        assert_redacts("abcd:dead:beef:1234::");
        assert_redacts("::dead:beef:1234");
        assert_redacts("0::0");
        assert_redacts("0:0:0:0::1");
    }

    #[test]
    fn doesnt_redact_not_ipv6() {
        assert_does_not_redact("[talpid_core::firewall]");
    }

    #[test]
    fn redacts_uuid_v4() {
        assert_redacts("1248e97e-134b-4820-92e1-abaf191c2840");
        assert_redacts("6B29FC40-CA47-1067-B31D-00DD010662DA");
        assert_redacts("123123ab-12ab-89cd-45ef-012345678901");
        assert_redacts("{123123ab-12ab-89cd-45ef-012345678901}");
    }

    #[test]
    #[cfg(windows)]
    fn redacts_home_dir() {
        let assert_redacts_home_dir = |home_dir, test_str| {
            let input = format!(r"pre {test_str}\remaining\path post");
            let actual = redact_home_dir_inner(&input, Some(PathBuf::from(home_dir)));
            assert_eq!(r"pre ~\remaining\path post", actual);
        };

        let home_dir = r"C:\Users\user";

        assert_redacts_home_dir(home_dir, r"\Device\HarddiskVolume1\Users\user");
        assert_redacts_home_dir(home_dir, r"C:\Users\user");
        assert_redacts_home_dir(home_dir, r"C:\Users\other-user");
        assert_redacts_home_dir(home_dir, r"C:\users\other-user");
    }

    #[test]
    #[cfg(windows)]
    fn redacts_windows_user_paths_without_home_dir() {
        let input = r"pre C:\users\other-user\remaining\path post";
        let actual = redact_home_dir_inner(input, None);
        assert_eq!(r"pre ~\remaining\path post", actual);

        let input = r"pre \Device\HarddiskVolume1\Users\other-user\remaining\path post";
        let actual = redact_home_dir_inner(input, None);
        assert_eq!(r"pre ~\remaining\path post", actual);
    }

    #[test]
    #[cfg(not(windows))]
    fn redacts_home_dir() {
        let assert_redacts_home_dir = |home_dir, test_str| {
            let input = format!(r"pre {test_str}/remaining/path post");
            let actual = redact_home_dir_inner(&input, Some(PathBuf::from(home_dir)));
            assert_eq!(r"pre ~/remaining/path post", actual);
        };

        let home_dir = r"/home/user";

        assert_redacts_home_dir(home_dir, r"/home/user");
        assert_redacts_home_dir(home_dir, r"/home/other-user");
        assert_redacts_home_dir(home_dir, r"/Users/other-user");

        let home_dir = r"/Users/user";

        assert_redacts_home_dir(home_dir, r"/home/user");
        assert_redacts_home_dir(home_dir, r"/home/other-user");
        assert_redacts_home_dir(home_dir, r"/Users/other-user");
    }

    #[test]
    fn doesnt_redact_not_uuid_v4() {
        assert_does_not_redact("23123ab-12ab-89cd-45ef-012345678901");
        assert_does_not_redact("GGGGGGGG-GGGG-GGGG-GGGG-GGGGGGGGGGGG");
    }

    #[test]
    fn does_not_redact_time() {
        assert_does_not_redact("09:47:59");
    }

    fn assert_redacts(input: &str) {
        let report = ProblemReport::new(vec![]);
        let actual = report.redact(&format!("pre {input} post"));
        assert_eq!("pre [REDACTED] post", actual);
    }

    fn assert_does_not_redact(input: &str) {
        let report = ProblemReport::new(vec![]);
        let res = report.redact(input);
        assert_eq!(input, res);
    }

    #[test]
    fn parse_metadata() {
        let report = ProblemReport::new(Vec::new());
        let mut report_data = Vec::new();
        report
            .write_to(&mut report_data)
            .expect("Unable to write report to vector");

        let report_string = std::str::from_utf8(&report_data).expect("Report is not correct UTF-8");

        let parsed_metadata = ProblemReport::parse_metadata(report_string)
            .expect("Unable to parse metadata from report");
        let expected_metadata = metadata::collect();

        assert_eq!(parsed_metadata.len(), expected_metadata.len());
        for (key, value) in &expected_metadata {
            let parsed_value = parsed_metadata
                .get(key)
                .expect("Parsed metadata and new one don't match");
            if key == "id" {
                assert_ne!(parsed_value, value, "id not supposed to match");
            } else {
                assert_eq!(parsed_value, value, "value for key '{key}' does not match");
            }
        }
    }
}
