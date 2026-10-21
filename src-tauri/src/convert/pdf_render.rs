//! PDF → images with pdfium (BSD/Apache), loaded at runtime from the app's resources.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};

use super::image::{encode, OutputFormat};
use crate::errors::AppError;
use crate::output::{self, Written};

/// Called before each page with (index, total, 1-based page number); `Err(Cancelled)` stops.
pub type PageStep<'a> = &'a mut dyn FnMut(usize, usize, u32) -> Result<(), AppError>;

/// Longest side of a rendered page, so a huge page at 300 DPI can't exhaust memory.
const MAX_SIDE_PX: f32 = 20_000.0;
const PAGE_JPG_QUALITY: u8 = 90;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageSelection {
    All,
    /// e.g. "1-3, 5"
    Range {
        spec: String,
    },
}

static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();
/// pdfium reports load errors through one global "last error"; holding this for a whole render
/// stops two renders from reading each other's error (and pdfium doesn't render in parallel anyway).
static RENDER_LOCK: Mutex<()> = Mutex::new(());

/// The shared pdfium instance, loaded from the first of `dirs` that contains the library.
pub fn pdfium(dirs: &[PathBuf]) -> Result<&'static Pdfium, AppError> {
    PDFIUM
        .get_or_init(|| {
            let path = dirs
                .iter()
                .map(Pdfium::pdfium_platform_library_name_at_path)
                .find(|p| p.exists())
                .ok_or_else(|| format!("pdfium library not found in {dirs:?}"))?;
            Pdfium::bind_to_library(&path)
                .map(Pdfium::new)
                .map_err(|e| format!("{path:?}: {e}"))
        })
        .as_ref()
        .map_err(|detail| AppError::MissingComponent {
            detail: detail.clone(),
        })
}

fn pdfium_error(err: PdfiumError) -> AppError {
    match err {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            AppError::PasswordProtected
        }
        PdfiumError::IoError(io) => AppError::Io(io),
        other => AppError::Corrupt {
            detail: format!("{other:?}"),
        },
    }
}

/// Parse "1-3, 5" into 1-based page numbers, in the order given, without duplicates.
pub fn parse_pages(spec: &str, page_count: u32) -> Result<Vec<u32>, AppError> {
    let bad = || AppError::invalid("Write pages like 1-3, 5.");
    let mut pages = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (start, end) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let start: u32 = start.parse().map_err(|_| bad())?;
        let end: u32 = end.parse().map_err(|_| bad())?;
        if start == 0 || end < start {
            return Err(bad());
        }
        if end > page_count {
            let pages = if page_count == 1 { "page" } else { "pages" };
            return Err(AppError::invalid(format!(
                "This PDF has only {page_count} {pages}."
            )));
        }
        for page in start..=end {
            if !pages.contains(&page) {
                pages.push(page);
            }
        }
    }
    if pages.is_empty() {
        return Err(bad());
    }
    Ok(pages)
}

/// Render the selected pages of `pdf` into a new `<name> pages/` folder next to it. Pages written
/// before a cancel or error are left in `written`.
#[allow(clippy::too_many_arguments)]
pub fn pdf_to_images(
    pdfium: &Pdfium,
    pdf: &Path,
    out_dir: &Path,
    format: OutputFormat,
    dpi: u16,
    selection: &PageSelection,
    fallback: Option<&Path>,
    step: PageStep<'_>,
    written: &mut Vec<Written>,
) -> Result<(), AppError> {
    let _render = RENDER_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(pdfium_error)?;
    let page_count = document.pages().len().max(0) as u32;
    if page_count == 0 {
        return Err(AppError::Corrupt {
            detail: "no pages".into(),
        });
    }
    let pages = match selection {
        PageSelection::All => (1..=page_count).collect(),
        PageSelection::Range { spec } => parse_pages(spec, page_count)?,
    };

    let (folder, fell_back) =
        output::create_folder(out_dir, fallback, &output::pages_folder_name(pdf))?;
    let first_new = written.len();
    let result = (|| {
        for (i, &page_number) in pages.iter().enumerate() {
            step(i, pages.len(), page_number)?;
            let page = document
                .pages()
                .get((page_number - 1) as PdfPageIndex)
                .map_err(pdfium_error)?;
            let image = render_page(&page, dpi)?;
            let name = output::page_file_name(pdf, page_number, page_count, format.extension());
            let mut file = output::write_output(&folder, None, &name, |w| {
                encode(&image, None, format, PAGE_JPG_QUALITY, w)
            })?;
            file.fell_back = fell_back;
            written.push(file);
        }
        Ok(())
    })();

    if written.len() == first_new {
        let _ = std::fs::remove_dir(&folder); // only removes it if still empty
    }
    result
}

// ---- Previews for the pages editor ----

/// Page size in points (1/72 inch); pixel size at a DPI is `points × dpi / 72`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// Size of every page, in order. Also how the UI learns a PDF is password-protected or damaged.
pub fn page_sizes(pdfium: &Pdfium, pdf: &Path) -> Result<Vec<PageSize>, AppError> {
    let _render = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(pdfium_error)?;
    Ok(document
        .pages()
        .iter()
        .map(|page| PageSize {
            width: page.width().value,
            height: page.height().value,
        })
        .collect())
}

/// Small JPEG data URLs for the given 1-based pages (longest side `max_px`).
pub fn page_thumbnails(
    pdfium: &Pdfium,
    pdf: &Path,
    pages: &[u32],
    max_px: u32,
) -> Result<Vec<String>, AppError> {
    let _render = RENDER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(pdfium_error)?;
    pages
        .iter()
        .map(|&number| {
            let page = document
                .pages()
                .get(number.saturating_sub(1) as PdfPageIndex)
                .map_err(pdfium_error)?;
            let longest = page.width().value.max(page.height().value).max(1.0);
            let dpi = (72.0 * max_px as f32 / longest).clamp(1.0, f32::from(u16::MAX));
            let image = render_page(&page, dpi as u16)?;
            let mut jpg = Vec::new();
            encode(
                &image,
                None,
                OutputFormat::Jpg,
                80,
                &mut std::io::Cursor::new(&mut jpg),
            )?;
            Ok(crate::files::data_url("image/jpeg", &jpg))
        })
        .collect()
}

fn render_page(page: &PdfPage, dpi: u16) -> Result<image::DynamicImage, AppError> {
    let scale = f32::from(dpi) / 72.0;
    let (w, h) = (page.width().value * scale, page.height().value * scale);
    let shrink = (MAX_SIDE_PX / w.max(h)).min(1.0);
    let config = PdfRenderConfig::new()
        .set_target_width(((w * shrink).round() as Pixels).max(1))
        .set_target_height(((h * shrink).round() as Pixels).max(1))
        .render_form_data(true)
        .set_clear_color(PdfColor::WHITE);
    page.render_with_config(&config)
        .and_then(|bitmap| bitmap.as_image())
        .map_err(pdfium_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::pdf::{images_to_pdf, Margin, PageSize};
    use image::{DynamicImage, GenericImageView, Rgb, RgbImage};

    fn lib_dirs() -> Vec<PathBuf> {
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium")]
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-render-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A PDF with `n` pages; page i is (100 + 10 i) × 200 pt.
    fn sample_pdf(dir: &Path, name: &str, n: u32) -> PathBuf {
        let images: Vec<PathBuf> = (0..n)
            .map(|i| {
                let path = dir.join(format!("src{i}.png"));
                // At 150 DPI (no DPI in PNG), w px → w × 72 / 150 pt.
                DynamicImage::ImageRgb8(RgbImage::from_pixel(
                    (100 + 10 * i) * 150 / 72,
                    200 * 150 / 72,
                    Rgb([200, 30, 30]),
                ))
                .save(&path)
                .unwrap();
                path
            })
            .collect();
        let mut step = |_| Ok(());
        let pdf = dir.join(name);
        images_to_pdf(&images, PageSize::Fit, Margin::None, &mut step)
            .unwrap()
            .save(&pdf)
            .unwrap();
        pdf
    }

    #[test]
    fn page_ranges() {
        assert_eq!(parse_pages("1-3,5", 10).unwrap(), [1, 2, 3, 5]);
        assert_eq!(parse_pages(" 2 , 2-3 ", 10).unwrap(), [2, 3]);
        assert_eq!(parse_pages("10", 10).unwrap(), [10]);
        for bad in ["", "0", "3-1", "a", "1-", "1-2-3", ","] {
            assert_eq!(
                parse_pages(bad, 10).unwrap_err().code(),
                "invalid_input",
                "{bad:?}"
            );
        }
        assert_eq!(
            parse_pages("8-12", 10).unwrap_err().to_string(),
            "This PDF has only 10 pages."
        );
    }

    #[test]
    fn renders_selected_pages_into_a_pages_folder() {
        let dir = temp_dir("render");
        let pdf = sample_pdf(&dir, "report.pdf", 3);
        let before = std::fs::read(&pdf).unwrap();
        let pdfium = pdfium(&lib_dirs()).expect("run scripts/fetch-pdfium.sh first");

        let mut steps = Vec::new();
        let mut step = |i, total, page| {
            steps.push((i, total, page));
            Ok(())
        };
        let mut written = Vec::new();
        pdf_to_images(
            pdfium,
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Jpg,
            72,
            &PageSelection::All,
            None,
            &mut step,
            &mut written,
        )
        .unwrap();
        assert_eq!(steps, [(0, 3, 1), (1, 3, 2), (2, 3, 3)]);
        let names: Vec<_> = written
            .iter()
            .map(|w| {
                w.path
                    .strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "report pages/report-01.jpg",
                "report pages/report-02.jpg",
                "report pages/report-03.jpg"
            ]
        );
        // 72 DPI: 1 pt = 1 px.
        let first = image::open(&written[0].path).unwrap();
        assert_eq!(first.dimensions(), (100, 200));

        // 150 DPI, PNG, a range; a second run gets a new folder.
        let mut step = |_, _, _| Ok(());
        let mut written = Vec::new();
        pdf_to_images(
            pdfium,
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Png,
            150,
            &PageSelection::Range { spec: "3".into() },
            None,
            &mut step,
            &mut written,
        )
        .unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].path, dir.join("report pages (1)/report-03.png"));
        let page3 = image::open(&written[0].path).unwrap();
        assert_eq!(page3.dimensions(), (250, 416)); // the 250 × 416 px source at 150 DPI
        assert!(
            page3.to_rgb8().get_pixel(10, 10).0[0] > 150,
            "red page rendered"
        );

        assert_eq!(std::fs::read(&pdf).unwrap(), before, "original untouched");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bad_range_and_cancel_leave_no_folder_behind() {
        let dir = temp_dir("cleanup");
        let pdf = sample_pdf(&dir, "a.pdf", 2);
        let pdfium = pdfium(&lib_dirs()).unwrap();

        let mut step = |_, _, _| Ok(());
        let err = pdf_to_images(
            pdfium,
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Jpg,
            72,
            &PageSelection::Range { spec: "5".into() },
            None,
            &mut step,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "This PDF has only 2 pages.");

        let mut cancel = |_, _, _| Err(AppError::Cancelled);
        let err = pdf_to_images(
            pdfium,
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Jpg,
            72,
            &PageSelection::All,
            None,
            &mut cancel,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Cancelled));
        assert!(!dir.join("a pages").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn damaged_pdf_is_a_friendly_error() {
        let dir = temp_dir("damaged");
        let pdf = dir.join("bad.pdf");
        std::fs::write(&pdf, b"%PDF-1.7 not really").unwrap();
        let mut step = |_, _, _| Ok(());
        let err = pdf_to_images(
            pdfium(&lib_dirs()).unwrap(),
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Jpg,
            72,
            &PageSelection::All,
            None,
            &mut step,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "corrupt");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn password_protected_pdf_says_so() {
        // tests/fixtures/locked.pdf: one page, user password "secret" (made with macOS PDFKit).
        let pdf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/locked.pdf");
        let mut step = |_, _, _| Ok(());
        let err = pdf_to_images(
            pdfium(&lib_dirs()).unwrap(),
            &pdf,
            pdf.parent().unwrap(),
            OutputFormat::Jpg,
            72,
            &PageSelection::All,
            None,
            &mut step,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "This PDF is password-protected.");
    }

    #[test]
    fn previews_for_the_pages_editor() {
        let dir = temp_dir("previews");
        let pdf = sample_pdf(&dir, "p.pdf", 2);
        let lib = pdfium(&lib_dirs()).unwrap();

        let sizes = page_sizes(lib, &pdf).unwrap();
        assert_eq!(sizes.len(), 2);
        assert!((sizes[1].width - 110.0).abs() < 1.0, "{sizes:?}");

        let thumbs = page_thumbnails(lib, &pdf, &[2, 1], 64).unwrap();
        assert_eq!(thumbs.len(), 2);
        assert!(thumbs
            .iter()
            .all(|t| t.starts_with("data:image/jpeg;base64,")));

        let locked = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/locked.pdf");
        assert_eq!(
            page_sizes(lib, &locked).unwrap_err().code(),
            "password_protected"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
