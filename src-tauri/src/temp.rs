//! Scratch files in the system temp folder (never next to the user's files), removed on drop.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TempFile(PathBuf);

impl TempFile {
    /// A fresh, unused name like `quickconvert-upright-412-3.jpg`.
    pub fn new(purpose: &str, extension: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        TempFile(std::env::temp_dir().join(format!(
            "quickconvert-{purpose}-{}-{}.{extension}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A scratch folder in the system temp folder, removed (with everything in it) on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(purpose: &str) -> std::io::Result<Self> {
        let dir = TempFile::new(purpose, "d").path().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(TempDir(dir))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
