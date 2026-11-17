//! Owner-only writes for the tunnel's state files.
//!
//! They live in the daemon's cache directory, which every local account can list and read, and
//! they name the exit in use and the networks the machine joined. A file is written to a fresh
//! owner-only file and renamed over the old one, so a file an older build left readable is
//! replaced rather than reused, and a reader holding the old one never sees the new content.

use std::io::{self, Write};
use std::path::Path;

pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".{}.tmp", std::process::id()));
    let temp_path = path.with_file_name(temp_name);

    let _ = std::fs::remove_file(&temp_path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let result = options
        .open(&temp_path)
        .and_then(|mut file| file.write_all(contents))
        .and_then(|()| std::fs::rename(&temp_path, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}
