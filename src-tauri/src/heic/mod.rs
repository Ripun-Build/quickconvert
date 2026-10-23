//! HEIC / HEIF photos (iPhone). macOS turns them into a temporary TIFF with the built-in `sips`
//! tool (pixels, colour profile and orientation tag kept; nothing bundled), which the normal image
//! loader then reads; `encode` writes clean HEIC the same way. Other systems can't open them yet. Like `selection/` and `upright/`, the
//! OS-specific part lives in this module.

#[cfg(target_os = "macos")]
mod macos;

use std::path::Path;

use image::DynamicImage;

use crate::errors::AppError;
use crate::temp::TempFile;

pub fn is_heic(path: &Path) -> bool {
    matches!(crate::output::extension(path).as_str(), "heic" | "heif")
}

/// A readable copy of a HEIC file, deleted when dropped.
pub fn decode(path: &Path) -> Result<TempFile, AppError> {
    decode_scaled(path, None)
}

/// Like `decode`, scaled so the long side is about `max` px (thumbnails, checks).
pub fn decode_scaled(path: &Path, max: Option<u32>) -> Result<TempFile, AppError> {
    #[cfg(target_os = "macos")]
    {
        macos::decode(path, max)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = max;
        Err(AppError::Unsupported {
            extension: crate::output::extension(path),
        })
    }
}

/// `image` as HEIC at the system encoder's normal camera quality, with `icc` and nothing else
/// (no location, camera or date info). Used where a HEIC should stay HEIC (Remove info).
pub fn encode(image: &DynamicImage, icc: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
    #[cfg(target_os = "macos")]
    {
        macos::encode(image, icc)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (image, icc);
        Err(AppError::Unsupported {
            extension: "heic".into(),
        })
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sideways-gps.heic")
    }

    #[test]
    fn sideways_iphone_photo_loads_upright_with_its_colour_profile() {
        // Stored 300 × 200 with a red block top-left, tagged "turn 90° right to view".
        let picture = crate::convert::image::load(&fixture()).unwrap();
        assert_eq!(picture.image.dimensions(), (200, 300));
        let p = picture.image.get_pixel(195, 5).0;
        assert!(p[0] > 200 && p[1] < 80, "red block now top-right: {p:?}");
        assert!(picture.icc.is_some(), "Display P3 profile kept");
    }

    #[test]
    fn writes_heic_that_reads_back_the_same_way_up() {
        let picture = crate::convert::image::load(&fixture()).unwrap();
        let bytes = encode(&picture.image, picture.icc.as_deref()).unwrap();
        assert_eq!(&bytes[4..12], b"ftypheic");
        let copy = TempFile::new("heic-test", "heic");
        std::fs::write(copy.path(), &bytes).unwrap();
        let back = crate::convert::image::load(copy.path()).unwrap();
        assert_eq!(back.image.dimensions(), (200, 300));
        assert!(back.icc.is_some());
        assert!(back.density.is_some());
    }

    #[test]
    fn small_copy_is_scaled_and_still_upright() {
        let picture = crate::convert::image::load_small(&fixture(), 60).unwrap();
        assert_eq!(picture.image.dimensions(), (40, 60));
    }

    #[test]
    fn temporary_copy_is_removed() {
        let copy = decode(&fixture()).unwrap();
        let path = copy.path().to_path_buf();
        assert!(path.exists());
        drop(copy);
        assert!(!path.exists());
    }
}
