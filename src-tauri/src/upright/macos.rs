//! macOS: score the four readings with Vision via `osascript -l JavaScript` (see upright.js).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use image::DynamicImage;

use super::Scores;
use crate::convert::image::{self as img, OutputFormat};
use crate::errors::AppError;

const SCRIPT: &str = include_str!("upright.js");
/// Plenty for Vision to read normal text, and quick to write and scan.
const SIZE: u32 = 1280;

/// Removes the temporary copy however `scores` returns.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn scores(image: &DynamicImage) -> Result<Scores, AppError> {
    // Vision reads a file: give it a small upright copy (EXIF already applied, no tag written).
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = TempFile(std::env::temp_dir().join(format!(
        "quickconvert-upright-{}-{}.jpg",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let mut file = std::fs::File::create(&temp.0)?;
    img::encode(
        &image.thumbnail(SIZE, SIZE),
        None,
        OutputFormat::Jpg,
        90,
        &mut file,
    )?;
    drop(file);

    let out = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .arg(&temp.0)
        .output()?;
    if !out.status.success() {
        return Err(AppError::internal(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| AppError::internal(e.to_string()))
}
