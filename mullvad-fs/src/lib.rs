use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};
use talpid_types::ErrorExt;
use tokio::{fs, io};

/// Stores content in a temporary file before moving it to the
/// final destination, ensuring that consumers of the file never
/// end up with partial content. Must be moved with `finalize`.
pub struct AtomicFile {
    file: Option<fs::File>,
    temp_path: PathBuf,
    target_path: PathBuf,
}

impl AtomicFile {
    pub async fn new<P: Into<PathBuf>>(target_path: P) -> io::Result<Self> {
        let target_path = target_path.into();
        let temp_path = target_path.with_file_name(uuid::Uuid::new_v4().to_string());
        // Every file written this way is daemon state, some of it secret, so it is created
        // private: a mode set after creation leaves a window where another account can open it.
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        Ok(Self {
            file: Some(options.open(&temp_path).await?),
            temp_path,
            target_path,
        })
    }

    /// Flushes and moves the file to `self.target_path`, replacing it if it exists.
    pub async fn finalize(mut self) -> io::Result<()> {
        let result = async {
            let file = self.file.take().unwrap();
            file.sync_all().await?;
            let std_file = file.into_std().await;
            let _ = tokio::task::spawn_blocking(move || drop(std_file)).await;
            fs::rename(&self.temp_path, &self.target_path).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::task::spawn_blocking(move || try_remove_file(&self.temp_path)).await;
        }
        result
    }
}

impl Drop for AtomicFile {
    fn drop(&mut self) {
        if self.file.is_some() {
            log::error!("{} was not finalized", self.target_path.display());
            try_remove_file(&self.temp_path);
        }
    }
}

fn try_remove_file(temp_path: &Path) {
    if let Err(error) = std::fs::remove_file(temp_path) {
        let msg = format!("Failed to delete temp file: {}", temp_path.display());
        log::warn!("{}", error.display_chain_with_msg(&msg));
    }
}

impl Deref for AtomicFile {
    type Target = fs::File;

    fn deref(&self) -> &Self::Target {
        self.file.as_ref().unwrap()
    }
}

impl DerefMut for AtomicFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file.as_mut().unwrap()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::AsyncWriteExt;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[tokio::test]
    async fn no_other_account_can_open_the_file_while_it_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = AtomicFile::new(dir.path().join("state.json"))
            .await
            .unwrap();

        let temp_mode = mode(&file.temp_path);
        file.write_all(b"{}").await.unwrap();
        file.finalize().await.unwrap();

        assert_eq!(temp_mode, 0o600);
    }

    #[tokio::test]
    async fn the_written_file_is_readable_by_its_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("state.json");
        let mut file = AtomicFile::new(&target).await.unwrap();
        file.write_all(b"{}").await.unwrap();
        file.finalize().await.unwrap();

        assert_eq!(mode(&target), 0o600);
    }
}
