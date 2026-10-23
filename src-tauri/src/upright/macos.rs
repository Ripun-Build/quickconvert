//! macOS: score the four readings with Vision via `osascript -l JavaScript` (see upright.js).

use std::process::Command;

use image::DynamicImage;

use super::Scores;
use crate::convert::image::{self as img, OutputFormat};
use crate::errors::AppError;
use crate::temp::TempFile;

const SCRIPT: &str = include_str!("upright.js");
/// Plenty for Vision to read normal text, and quick to write and scan.
const SIZE: u32 = 1280;

pub fn scores(image: &DynamicImage) -> Result<Scores, AppError> {
    // Vision reads a file: give it a small upright copy (EXIF already applied, no tag written).
    let temp = TempFile::new("upright", "jpg");
    let mut file = std::fs::File::create(temp.path())?;
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
        .arg(temp.path())
        .output()?;
    if !out.status.success() {
        return Err(AppError::internal(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| AppError::internal(e.to_string()))
}
