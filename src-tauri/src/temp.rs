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
