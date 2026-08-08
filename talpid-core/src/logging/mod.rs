use std::{fs, io, path::Path};

pub mod diag;

/// Unable to create new log file
#[derive(thiserror::Error, Debug)]
#[error("Unable to create new log file")]
pub struct RotateLogError(#[from] io::Error);

/// Create a new log file while backing up the two previous versions of it.
///
/// A new log file is created with the given file name; an existing one becomes
/// `.old.log`, and what was `.old.log` becomes `.old2.log`.
///
/// TWO generations, not one, because the daemon rotates at every start and an
/// app update restarts it more than once. With a single backup, the
/// post-update boot (the one that decides whether the host is left blocked) is
/// evicted by the next two restarts: on 2026-08-08 that erased the only record
/// of a four-minute lock-out before anyone could read it.
pub fn rotate_log(file: &Path) -> Result<(), RotateLogError> {
    let backup = file.with_extension("old.log");
    let backup2 = file.with_extension("old2.log");
    // Oldest first, or the second rename would overwrite what the first just
    // wrote. A missing file at either step is the normal first-run case.
    shift(&backup, &backup2);
    shift(file, &backup);

    fs::File::create(file).map_err(RotateLogError)?;
    Ok(())
}

/// Moves `from` onto `to`, tolerating a missing source (nothing to rotate yet).
fn shift(from: &Path, to: &Path) {
    if let Err(error) = fs::rename(from, to)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::warn!("Failed to rotate log file to {}: {}", to.display(), error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One generation of backup is not enough around an app update. The daemon
    /// rotates at every start, so the post-update daemon (the one that decides
    /// whether the host is blocked) is evicted by the next TWO restarts. On
    /// 2026-08-08 that erased the only window that could have explained a
    /// four-minute lock-out: the install was at 12:53, and by the time anyone
    /// looked, two restarts had overwritten it.
    #[test]
    fn two_generations_of_log_survive_so_a_post_update_boot_is_still_readable() {
        let dir = std::env::temp_dir().join(format!("warren-rotate-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let log = dir.join("daemon.log");

        for generation in ["first", "second", "third"] {
            fs::write(&log, generation).expect("write");
            rotate_log(&log).expect("rotate");
        }

        assert_eq!(
            fs::read_to_string(log.with_extension("old.log")).expect("one back"),
            "third"
        );
        assert_eq!(
            fs::read_to_string(log.with_extension("old2.log")).expect("two back"),
            "second",
            "the boot two restarts ago must still be readable"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
