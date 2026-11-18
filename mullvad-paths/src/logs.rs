use crate::Result;
use std::{env, path::PathBuf};

/// Creates and returns the logging directory pointed to by `WARREN_LOG_DIR`,
/// `MULLVAD_LOG_DIR` (back-compat upstream), or the default if neither is set.
pub fn log_dir() -> Result<PathBuf> {
    crate::create_dir(get_log_dir()?, log_dir_permissions())
}

/// The daemon logs narrate the owner's use of the VPN, so only root (Unix) or the administrators
/// and SYSTEM (Windows) read them; the problem report collector, run by the user, gets them
/// through the daemon. On Unix an existing directory left open by an older build is replaced, its
/// logs with it; on Windows its ACL is replaced and the change reaches the files inside. The
/// Windows crash minidump is written there too, so it is no more readable than the logs.
pub(crate) fn log_dir_permissions() -> Option<crate::UserPermissions> {
    Some(crate::UserPermissions {
        read: false,
        write: false,
        execute: false,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn only_root_can_read_the_daemon_logs() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warren-log-dir-{unique}"));

        crate::create_dir(dir.clone(), super::log_dir_permissions()).unwrap();

        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(mode, 0o700);
    }
}

/// Get the logging directory, but don't try to create it.
pub fn get_log_dir() -> Result<PathBuf> {
    match env::var_os("WARREN_LOG_DIR").or_else(|| env::var_os("MULLVAD_LOG_DIR")) {
        Some(path) => Ok(PathBuf::from(path)),
        None => get_default_log_dir(),
    }
}

#[cfg(unix)]
pub fn get_default_log_dir() -> Result<PathBuf> {
    let dir = PathBuf::from("/var/log").join(crate::PRODUCT_NAME);
    Ok(dir)
}

#[cfg(windows)]
pub fn get_default_log_dir() -> Result<PathBuf> {
    let dir = crate::windows::get_allusersprofile_dir()?.join(crate::PRODUCT_NAME);
    Ok(dir)
}
