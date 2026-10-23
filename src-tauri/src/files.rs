//! Describing input files for the UI: supported kind, name, size, and small thumbnails.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;

use crate::errors::AppError;

/// Longest side of a thumbnail in px (FileChip shows 32 px at up to 2x).
const THUMB_SIZE: u32 = 64;
/// Cached thumbnails before the cache is cleared.
const THUMB_CACHE_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Image,
    Pdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
}

/// Supported files in selection order, plus how many inputs were skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSet {
    pub files: Vec<FileInfo>,
    pub unsupported: usize,
}

/// Supported input type by extension (SPECS §11).
pub fn kind_of(path: &Path) -> Option<FileKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "tif" | "tiff" | "heic" | "heif" => {
            Some(FileKind::Image)
        }
        "pdf" => Some(FileKind::Pdf),
        _ => None,
    }
}

/// Folders, missing files and unsupported types count as `unsupported`.
pub fn describe(paths: &[PathBuf]) -> FileSet {
    let mut set = FileSet::default();
    for path in paths {
        let info = std::fs::metadata(path)
            .ok()
            .filter(|meta| meta.is_file())
            .and_then(|meta| Some((kind_of(path)?, meta.len())));
        match info {
            Some((kind, size)) => set.files.push(FileInfo {
                path: path.to_string_lossy().into_owned(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                kind,
                size,
            }),
            None => set.unsupported += 1,
        }
    }
    set
}

/// Thumbnails cached by path + modification time.
#[derive(Default)]
pub struct ThumbnailCache(Mutex<HashMap<(PathBuf, SystemTime), String>>);

impl ThumbnailCache {
    pub fn get_or_create(&self, path: &Path) -> Result<String, AppError> {
        let modified = std::fs::metadata(path)?.modified()?;
        let key = (path.to_path_buf(), modified);
        if let Some(hit) = self.0.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return Ok(hit);
        }
        let url = thumbnail_data_url(path)?;
        if let Ok(mut cache) = self.0.lock() {
            if cache.len() >= THUMB_CACHE_LIMIT {
                cache.clear();
            }
            cache.insert(key, url.clone());
        }
        Ok(url)
    }
}

/// Longest side of the editor preview in px.
const PREVIEW_SIZE: u32 = 1024;

/// Editor preview: a JPEG data URL (colour profile kept) and the image's upright size in px.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

pub fn preview(path: &Path) -> Result<Preview, AppError> {
    let picture = crate::convert::image::load(path)?;
    let (width, height) = (picture.image.width(), picture.image.height());
    Ok(Preview {
        url: jpeg_data_url(
            &picture.image.thumbnail(PREVIEW_SIZE, PREVIEW_SIZE),
            picture.icc.as_deref(),
        )?,
        width,
        height,
    })
}

/// Longest side of page-tray thumbnails in px (sharp at the tile size on Retina screens).
const TRAY_SIZE: u32 = 240;

/// Page tray tile: a thumbnail plus the photo's upright shape (the size of the thumbnail; only the
/// proportions matter for the page preview).
pub fn tray_thumbnail(path: &Path) -> Result<Preview, AppError> {
    let picture = crate::convert::image::load_small(path, TRAY_SIZE * 2)?;
    let small = picture.image.thumbnail(TRAY_SIZE, TRAY_SIZE);
    Ok(Preview {
        url: jpeg_data_url(&small, picture.icc.as_deref())?,
        width: small.width(),
        height: small.height(),
    })
}

/// `image` as a JPEG data URL (quality 85), colour profile kept.
pub fn jpeg_data_url(image: &image::DynamicImage, icc: Option<&[u8]>) -> Result<String, AppError> {
    let mut jpg = Vec::new();
    crate::convert::image::encode(
        image,
        icc,
        crate::convert::image::OutputFormat::Jpg,
        85,
        &mut Cursor::new(&mut jpg),
    )?;
    Ok(format!("data:image/jpeg;base64,{}", base64(&jpg)))
}

/// Small PNG data URL for an image file, rotated per its EXIF orientation.
pub fn thumbnail_data_url(path: &Path) -> Result<String, AppError> {
    if kind_of(path) != Some(FileKind::Image) {
        return Err(AppError::Unsupported {
            extension: extension_of(path),
        });
    }
    let picture = crate::convert::image::load_small(path, THUMB_SIZE * 4)?;

    // Keep the colour profile so thumbnails show the same colours as the file.
    let mut png = Vec::new();
    crate::convert::image::encode(
        &picture.image.thumbnail(THUMB_SIZE, THUMB_SIZE),
        picture.icc.as_deref(),
        crate::convert::image::OutputFormat::Png,
        100,
        &mut Cursor::new(&mut png),
    )?;
    Ok(format!("data:image/png;base64,{}", base64(&png)))
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `data:<mime>;base64,…` URL for the UI.
pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}

/// Standard base64 with padding (RFC 4648).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-files-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected, "input {input:?}");
        }
    }

    #[test]
    fn kinds_by_extension_ignore_case() {
        assert_eq!(kind_of(Path::new("a/IMG_1.JPG")), Some(FileKind::Image));
        assert_eq!(kind_of(Path::new("scan.tiff")), Some(FileKind::Image));
        assert_eq!(kind_of(Path::new("doc.PDF")), Some(FileKind::Pdf));
        assert_eq!(kind_of(Path::new("notes.docx")), None);
        assert_eq!(kind_of(Path::new("Makefile")), None);
    }

    #[test]
    fn describe_keeps_order_and_counts_unsupported() {
        let dir = temp_dir("describe");
        let b = dir.join("b.png");
        let a = dir.join("a.pdf");
        let txt = dir.join("notes.txt");
        std::fs::write(&b, [0u8; 10]).unwrap();
        std::fs::write(&a, [0u8; 3]).unwrap();
        std::fs::write(&txt, "x").unwrap();
        let missing = dir.join("gone.jpg");

        let set = describe(&[b.clone(), txt, a.clone(), missing, dir.clone()]);
        let names: Vec<_> = set.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["b.png", "a.pdf"]);
        assert_eq!(set.files[0].size, 10);
        assert_eq!(set.files[1].kind, FileKind::Pdf);
        assert_eq!(set.unsupported, 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn thumbnail_is_small_png_data_url() {
        let dir = temp_dir("thumb");
        let path = dir.join("wide.png");
        image::RgbImage::from_pixel(400, 100, image::Rgb([200, 30, 30]))
            .save(&path)
            .unwrap();

        let url = thumbnail_data_url(&path).unwrap();
        // "iVBORw0KGgo" is the base64 of the PNG signature.
        assert!(url.starts_with("data:image/png;base64,iVBORw0KGgo"));

        let cache = ThumbnailCache::default();
        assert_eq!(cache.get_or_create(&path).unwrap(), url);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn thumbnail_of_damaged_image_is_corrupt_error() {
        let dir = temp_dir("corrupt");
        let path = dir.join("broken.jpg");
        std::fs::write(&path, b"not really a jpeg").unwrap();
        assert!(matches!(
            thumbnail_data_url(&path),
            Err(AppError::Corrupt { .. })
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preview_reports_full_size_and_is_small() {
        let dir = temp_dir("preview");
        let path = dir.join("big.png");
        image::RgbImage::from_pixel(3000, 1500, image::Rgb([9, 9, 9]))
            .save(&path)
            .unwrap();
        let p = preview(&path).unwrap();
        assert_eq!((p.width, p.height), (3000, 1500));
        assert!(p.url.starts_with("data:image/jpeg;base64,/9j/"));
        assert!(p.url.len() < 200_000);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
