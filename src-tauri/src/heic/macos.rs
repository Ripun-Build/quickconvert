//! macOS: HEIC → temporary TIFF with `/usr/bin/sips` (ships with every Mac).

use std::path::Path;
use std::process::Command;

use image::DynamicImage;

use crate::convert::image::{self as img, OutputFormat};
use crate::errors::AppError;
use crate::temp::TempFile;

pub fn decode(path: &Path, max: Option<u32>) -> Result<TempFile, AppError> {
    let copy = TempFile::new("heic", "tiff");
    let mut sips = Command::new("/usr/bin/sips");
    sips.args(["-s", "format", "tiff"]);
    if let Some(max) = max {
        sips.arg("-Z").arg(max.to_string());
    }
    let out = sips.arg(path).arg("--out").arg(copy.path()).output()?;
    if !out.status.success() || !copy.path().exists() {
        return Err(AppError::Corrupt {
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(copy)
}

/// Clean pixels (PNG with only the colour profile) → HEIC at the default, camera-like quality.
pub fn encode(image: &DynamicImage, icc: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
    let source = TempFile::new("heic-in", "png");
    let mut file = std::fs::File::create(source.path())?;
    img::encode(image, icc, OutputFormat::Png, 100, &mut file)?;
    drop(file);
    let target = TempFile::new("heic-out", "heic");
    let out = Command::new("/usr/bin/sips")
        .args(["-s", "format", "heic"])
        .arg(source.path())
        .arg("--out")
        .arg(target.path())
        .output()?;
    if !out.status.success() || !target.path().exists() {
        return Err(AppError::internal(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(std::fs::read(target.path())?)
}
