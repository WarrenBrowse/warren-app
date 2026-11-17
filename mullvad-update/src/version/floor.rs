//! The anti-rollback floor of the version metadata, kept across restarts.
//!
//! A verified manifest carries a `metadata_version` counter. Refusing any
//! manifest below the highest one accepted so far stops a replayed older
//! manifest (one that still names a vulnerable release as current) only if
//! that highest value survives the process: an in-memory floor starts from
//! zero again at every launch.

use std::path::{Path, PathBuf};

use super::MIN_VERIFY_METADATA_VERSION;

/// The persisted floor, one decimal number in a file of its own.
#[derive(Debug, Clone)]
pub struct MetadataVersionFloor {
    path: PathBuf,
}

impl MetadataVersionFloor {
    /// The floor stored at `path`. The file is created on the first raise.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The lowest `metadata_version` a manifest may carry: the highest one
    /// accepted before, never below [`MIN_VERIFY_METADATA_VERSION`]. A
    /// missing or unreadable file reads as that minimum.
    pub fn load(&self) -> usize {
        match read_floor(&self.path) {
            Some(stored) if stored > MIN_VERIFY_METADATA_VERSION => stored,
            _ => MIN_VERIFY_METADATA_VERSION,
        }
    }

    /// Record that a manifest carrying `accepted` was verified. The floor
    /// only ever rises; the file is replaced atomically.
    ///
    /// # Errors
    /// The file could not be written.
    pub fn raise(&self, accepted: usize) -> std::io::Result<()> {
        if accepted <= self.load() {
            return Ok(());
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, accepted.to_string())?;
        std::fs::rename(&tmp, &self.path)
    }
}

fn read_floor(path: &Path) -> Option<usize> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_floor_never_written_is_the_minimum() {
        let dir = tempfile::tempdir().unwrap();
        let floor = MetadataVersionFloor::at(dir.path().join("floor"));

        assert_eq!(floor.load(), MIN_VERIFY_METADATA_VERSION);
    }

    #[test]
    fn a_raised_floor_survives_a_new_reader() {
        let dir = tempfile::tempdir().unwrap();
        MetadataVersionFloor::at(dir.path().join("floor"))
            .raise(42)
            .unwrap();

        assert_eq!(
            MetadataVersionFloor::at(dir.path().join("floor")).load(),
            42
        );
    }

    #[test]
    fn the_floor_never_goes_down() {
        let dir = tempfile::tempdir().unwrap();
        let floor = MetadataVersionFloor::at(dir.path().join("floor"));
        floor.raise(42).unwrap();

        floor.raise(7).unwrap();

        assert_eq!(floor.load(), 42);
    }

    #[test]
    fn an_unreadable_floor_reads_as_the_minimum() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("floor"), "not a number").unwrap();

        assert_eq!(
            MetadataVersionFloor::at(dir.path().join("floor")).load(),
            MIN_VERIFY_METADATA_VERSION
        );
    }
}
