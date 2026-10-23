//! Output paths and collision-safe writing (SPECS §9). Originals are never overwritten:
//! the final name is reserved with an exclusive create, data goes to a hidden temp file in the
//! same folder, and the temp file is renamed over the reservation only once it is complete.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, ErrorKind};
use std::path::{Path, PathBuf};

use crate::errors::AppError;

/// A finished output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub path: PathBuf,
    /// The input's folder wasn't writable, so the file went to the fallback (Downloads).
    pub fell_back: bool,
}

/// `photo.png` + `jpg` → `photo.jpg`.
pub fn converted_name(input: &Path, ext: &str) -> String {
    format!("{}.{ext}", stem(input))
}

/// `photo.jpg` + `resized` → `photo-resized.jpg`.
pub fn suffixed_name(input: &Path, suffix: &str, ext: &str) -> String {
    format!("{}-{suffix}.{ext}", stem(input))
}

/// `IMG_1.jpg` → `IMG_1.pdf` for one image, `IMG_1 +4.pdf` for five.
pub fn combined_pdf_name(first: &Path, count: usize) -> String {
    if count <= 1 {
        format!("{}.pdf", stem(first))
    } else {
        format!("{} +{}.pdf", stem(first), count - 1)
    }
}

/// `report.pdf` → `report parts` (the folder Split PDF writes).
pub fn parts_folder_name(pdf: &Path) -> String {
    format!("{} parts", stem(pdf))
}

/// `report.pdf` + pages → `report (pages 1-3).pdf`, `report (page 4).pdf`, or for a long list
/// `report (12 pages).pdf`.
pub fn pages_pdf_name(pdf: &Path, pages: &[u32]) -> String {
    let list = compact_pages(pages);
    let label = if pages.len() == 1 {
        format!("page {list}")
    } else if list.len() <= 30 {
        format!("pages {list}")
    } else {
        format!("{} pages", pages.len())
    };
    format!("{} ({label}).pdf", stem(pdf))
}

/// [1, 2, 3, 5] → "1-3, 5" (runs where pages follow on; order kept).
pub fn compact_pages(pages: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < pages.len() {
        let start = pages[i];
        while i + 1 < pages.len() && pages[i + 1] == pages[i] + 1 {
            i += 1;
        }
        parts.push(if pages[i] == start {
            start.to_string()
        } else {
            format!("{start}-{}", pages[i])
        });
        i += 1;
    }
    parts.join(", ")
}

/// `report.pdf` → `report merged.pdf`.
pub fn merged_pdf_name(first: &Path) -> String {
    format!("{} merged.pdf", stem(first))
}

/// Lower-case extension of the input (`JPEG` → `jpeg`), or "" if none.
pub fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into())
}

/// `photo.jpg`, then `photo (1).jpg`, `photo (2).jpg`, …
fn numbered(name: &str, n: u32) -> String {
    if n == 0 {
        return name.to_string();
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}

/// Write `name` into `dir` (or `fallback` if `dir` isn't writable) without overwriting anything.
pub fn write_output<F>(
    dir: &Path,
    fallback: Option<&Path>,
    name: &str,
    write: F,
) -> Result<Written, AppError>
where
    F: FnOnce(&mut BufWriter<File>) -> Result<(), AppError>,
{
    let (path, fell_back) = match reserve(dir, name) {
        Ok(path) => (path, false),
        Err(err) if is_not_writable(&err) => match fallback {
            Some(fallback) => (reserve(fallback, name)?, true),
            None => return Err(err.into()),
        },
        Err(err) => return Err(err.into()),
    };

    match write_via_temp(&path, write) {
        Ok(()) => Ok(Written { path, fell_back }),
        Err(err) => {
            let _ = fs::remove_file(&path); // drop the empty reservation
            Err(err)
        }
    }
}

/// Create a new folder `name` in `dir` (or `fallback`), numbering it `name (1)`, `name (2)`… if
/// taken. Returns the folder and whether the fallback was used.
pub fn create_folder(
    dir: &Path,
    fallback: Option<&Path>,
    name: &str,
) -> Result<(PathBuf, bool), AppError> {
    match create_numbered_dir(dir, name) {
        Ok(path) => Ok((path, false)),
        Err(err) if is_not_writable(&err) => match fallback {
            Some(fallback) => Ok((create_numbered_dir(fallback, name)?, true)),
            None => Err(err.into()),
        },
        Err(err) => Err(err.into()),
    }
}

fn create_numbered_dir(dir: &Path, name: &str) -> io::Result<PathBuf> {
    for n in 0..10_000 {
        let candidate = if n == 0 {
            name.to_string()
        } else {
            format!("{name} ({n})")
        };
        let path = dir.join(candidate);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "no free folder name",
    ))
}

/// `report.pdf` → `report pages`.
pub fn pages_folder_name(pdf: &Path) -> String {
    format!("{} pages", stem(pdf))
}

/// `report`, page 3 of 120 → `report-003.jpg` (zero-padded to the page count, at least 2 digits).
pub fn page_file_name(pdf: &Path, page: u32, page_count: u32, ext: &str) -> String {
    let width = page_count.to_string().len().max(2);
    format!("{}-{page:0width$}.{ext}", stem(pdf))
}

/// Create an empty file under the first free name; it holds the name until the rename.
fn reserve(dir: &Path, name: &str) -> io::Result<PathBuf> {
    for n in 0..10_000 {
        let path = dir.join(numbered(name, n));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "no free output name",
    ))
}

fn write_via_temp<F>(path: &Path, write: F) -> Result<(), AppError>
where
    F: FnOnce(&mut BufWriter<File>) -> Result<(), AppError>,
{
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{file_name}.qc-tmp-{}", std::process::id()));

    let result = (|| {
        let mut writer = BufWriter::new(File::create(&temp)?);
        write(&mut writer)?;
        let file = writer
            .into_inner()
            .map_err(|e| AppError::Io(e.into_error()))?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn is_not_writable(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-output-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn names_follow_specs() {
        let p = Path::new("/x/holiday photo.PNG");
        assert_eq!(converted_name(p, "jpg"), "holiday photo.jpg");
        assert_eq!(
            suffixed_name(p, "resized", "png"),
            "holiday photo-resized.png"
        );
        assert_eq!(combined_pdf_name(Path::new("IMG_1.jpg"), 1), "IMG_1.pdf");
        assert_eq!(combined_pdf_name(Path::new("IMG_1.jpg"), 5), "IMG_1 +4.pdf");
        assert_eq!(
            merged_pdf_name(Path::new("a/report.pdf")),
            "report merged.pdf"
        );
        assert_eq!(extension(p), "png");
    }

    #[test]
    fn numbering_keeps_extension() {
        assert_eq!(numbered("photo.jpg", 0), "photo.jpg");
        assert_eq!(numbered("photo.jpg", 2), "photo (2).jpg");
        assert_eq!(numbered("IMG_1 +4.pdf", 1), "IMG_1 +4 (1).pdf");
        assert_eq!(numbered("README", 1), "README (1)");
    }

    #[test]
    fn never_overwrites_existing_files() {
        let dir = temp_dir("collide");
        fs::write(dir.join("photo.jpg"), b"original").unwrap();
        fs::write(dir.join("photo (1).jpg"), b"other").unwrap();

        let written = write_output(&dir, None, "photo.jpg", |w| Ok(w.write_all(b"new")?)).unwrap();
        assert_eq!(written.path, dir.join("photo (2).jpg"));
        assert!(!written.fell_back);
        assert_eq!(fs::read(dir.join("photo.jpg")).unwrap(), b"original");
        assert_eq!(fs::read(dir.join("photo (1).jpg")).unwrap(), b"other");
        assert_eq!(fs::read(&written.path).unwrap(), b"new");
        assert_eq!(names(&dir), ["photo (1).jpg", "photo (2).jpg", "photo.jpg"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_write_leaves_nothing_behind() {
        let dir = temp_dir("fail");
        let result = write_output(&dir, None, "out.pdf", |w| {
            w.write_all(b"half")?;
            Err(AppError::Cancelled)
        });
        assert!(matches!(result, Err(AppError::Cancelled)));
        assert!(names(&dir).is_empty(), "left: {:?}", names(&dir));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn falls_back_when_folder_is_read_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("readonly");
        let fallback = temp_dir("fallback");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

        let written =
            write_output(&dir, Some(&fallback), "a.png", |w| Ok(w.write_all(b"x")?)).unwrap();
        assert!(written.fell_back);
        assert_eq!(written.path, fallback.join("a.png"));

        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(names(&dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
        fs::remove_dir_all(fallback).unwrap();
    }

    #[test]
    fn page_names_are_zero_padded_to_the_page_count() {
        let pdf = Path::new("/x/report.pdf");
        assert_eq!(pages_folder_name(pdf), "report pages");
        assert_eq!(page_file_name(pdf, 3, 9, "jpg"), "report-03.jpg");
        assert_eq!(page_file_name(pdf, 3, 120, "png"), "report-003.png");
        assert_eq!(page_file_name(pdf, 1, 1, "jpg"), "report-01.jpg");
    }

    #[test]
    fn folders_get_numbered_instead_of_reused() {
        let dir = temp_dir("folders");
        let (a, _) = create_folder(&dir, None, "v1.2 pages").unwrap();
        let (b, fell_back) = create_folder(&dir, None, "v1.2 pages").unwrap();
        assert_eq!(a, dir.join("v1.2 pages"));
        assert_eq!(b, dir.join("v1.2 pages (1)"));
        assert!(!fell_back);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn page_list_names() {
        let pdf = Path::new("/x/report.pdf");
        assert_eq!(pages_pdf_name(pdf, &[1, 2, 3]), "report (pages 1-3).pdf");
        assert_eq!(pages_pdf_name(pdf, &[4]), "report (page 4).pdf");
        assert_eq!(
            pages_pdf_name(pdf, &[2, 5, 6, 7, 9]),
            "report (pages 2, 5-7, 9).pdf"
        );
        let scattered: Vec<u32> = (1..=40).step_by(3).collect();
        assert_eq!(pages_pdf_name(pdf, &scattered), "report (14 pages).pdf");
        assert_eq!(parts_folder_name(pdf), "report parts");
    }
}
