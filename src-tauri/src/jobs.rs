//! Job runner: conversions run on a blocking thread, report `job://progress`, finish with
//! `job://done`, and can be cancelled between files/pages.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::convert::compress::{self, CompressTarget, Preset};
use crate::convert::crop::{self, CropRect, Turn};
use crate::convert::image::{self as img, OutputFormat, ResizeMode};
use crate::convert::metadata::{self, Found};
use crate::convert::pdf::{self, Margin, PageSize};
use crate::convert::pdf_compress::{self, Level};
use crate::convert::pdf_edit;
use crate::convert::pdf_render::{self, PageSelection};
use crate::errors::AppError;
use crate::output::{self, Written};
use crate::popup;

/// What to run, with its options (mirrors `JobRequest` in `src/lib/types.ts`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum JobRequest {
    ImagesToPdf {
        page_size: PageSize,
        margin: Margin,
        /// Page tray: a quarter turn for some photos, keyed by path (drawn turned, not re-encoded).
        #[serde(default)]
        turns: HashMap<String, Turn>,
    },
    PdfMerge,
    ImageConvert {
        format: OutputFormat,
        quality: u8,
    },
    ImageResize {
        mode: ResizeMode,
    },
    PdfToImages {
        format: OutputFormat,
        dpi: u16,
        pages: PageSelection,
    },
    ImageCompress {
        target: CompressTarget,
    },
    ApplyPreset {
        preset: Preset,
    },
    /// Turn first (CropEditor's rotate buttons), then keep `rect`.
    ImageCrop {
        rect: CropRect,
        #[serde(default)]
        turn: Turn,
    },
    /// Each image's own turn, keyed by its path (a batch can mix sideways and upside-down images).
    ImageRotate {
        turns: HashMap<String, Turn>,
    },
    /// Remove location, camera, date and other personal info.
    StripMetadata,
    /// Split one PDF into parts (lists of 1-based pages) in a `<name> parts` folder.
    PdfSplit {
        parts: Vec<Vec<u32>>,
    },
    /// Keep only these pages (1-based, in order) as a new PDF.
    PdfExtract {
        pages: Vec<u32>,
    },
    /// Quarter turns added to pages, keyed by 1-based page number (as text: JSON object keys).
    PdfRotate {
        turns: HashMap<String, Turn>,
    },
    /// Make PDFs smaller (pictures re-saved at the level, extras removed).
    PdfCompress {
        level: Level,
    },
    /// Word / PowerPoint / Excel → PDF. `engines` holds the way picked per type ("word",
    /// "slides", "sheet"); a type without a pick uses the best way available.
    OfficeToPdf {
        #[serde(default)]
        engines: HashMap<String, crate::office::Engine>,
        #[serde(default)]
        quick: crate::office::QuickOptions,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    job_id: String,
    file_index: usize,
    file_count: usize,
    percent: f64,
    file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub name: String,
    pub error: AppError,
}

/// What an image output ended up as (shown in the Result view: "48 KB · 200×230 · quality 78").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: Option<u8>,
    /// Remove-info action: what was found and removed.
    pub removed: Option<Found>,
    /// Convert to PDF: how it was made.
    pub made_with: Option<crate::office::Engine>,
    /// Convert to PDF: fonts not on this Mac, and what was used instead.
    pub fonts_replaced: Vec<crate::office::fonts::FontSwap>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputFile {
    pub path: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<Found>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub made_with: Option<crate::office::Engine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fonts_replaced: Vec<crate::office::fonts::FontSwap>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDone {
    pub job_id: String,
    pub outputs: Vec<OutputFile>,
    pub failures: Vec<Failure>,
    /// Some output went to Downloads because the input's folder wasn't writable.
    pub saved_to_fallback: bool,
    pub cancelled: bool,
    /// Make PDF smaller: files already about as small as they get (no copy saved).
    pub unchanged: Vec<String>,
}

/// Running jobs and the files they wrote (only those may be opened/revealed from the UI).
#[derive(Default)]
pub struct Jobs {
    next_id: AtomicU64,
    running: AtomicUsize,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    outputs: Mutex<HashSet<PathBuf>>,
}

impl Jobs {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed) > 0
    }

    pub fn cancel(&self, job_id: &str) {
        if let Some(flag) = self
            .cancels
            .lock()
            .ok()
            .and_then(|c| c.get(job_id).cloned())
        {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Whether `path` was written by a job in this session.
    pub fn wrote(&self, path: &Path) -> bool {
        self.outputs
            .lock()
            .map(|o| o.contains(path))
            .unwrap_or(false)
    }
}

/// Start a job in the background and return its id.
pub fn start(
    app: &AppHandle,
    request: JobRequest,
    paths: Vec<PathBuf>,
) -> Result<String, AppError> {
    if paths.is_empty() {
        return Err(AppError::internal("no files"));
    }
    let jobs = app.state::<Jobs>();
    let job_id = format!("job-{}", jobs.next_id.fetch_add(1, Ordering::Relaxed) + 1);
    let cancel = Arc::new(AtomicBool::new(false));
    jobs.cancels
        .lock()
        .map_err(|_| AppError::internal("jobs poisoned"))?
        .insert(job_id.clone(), cancel.clone());
    jobs.running.fetch_add(1, Ordering::Relaxed);

    let app = app.clone();
    let id = job_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fallback = app.path().download_dir().ok();
        let pdfium_dirs = pdfium_dirs(&app);
        let output_dir = app
            .state::<crate::settings::AppSettings>()
            .get()
            .output_folder()
            .map(Path::to_path_buf);
        if let Some(dir) = &output_dir {
            let _ = std::fs::create_dir_all(dir); // recreate a folder that was removed
        }
        let emit = |index: usize, count: usize, name: &str, percent: f64| {
            let progress = Progress {
                job_id: id.clone(),
                file_index: index,
                file_count: count,
                percent,
                file_name: name.to_string(),
            };
            let _ = app.emit_to(popup::LABEL, "job://progress", progress);
        };
        let places = Places {
            fallback: fallback.as_deref(),
            pdfium_dirs: &pdfium_dirs,
            output_dir: output_dir.as_deref(),
            workers: crate::pool::default_workers(),
        };
        let done = run(&id, &request, &paths, &places, &cancel, &emit);

        let jobs = app.state::<Jobs>();
        if let Ok(mut outputs) = jobs.outputs.lock() {
            outputs.extend(done.outputs.iter().map(|o| PathBuf::from(&o.path)));
        }
        if let Ok(mut cancels) = jobs.cancels.lock() {
            cancels.remove(&id);
        }
        jobs.running.fetch_sub(1, Ordering::Relaxed);
        if let Err(err) = app.emit_to(popup::LABEL, "job://done", &done) {
            eprintln!("[jobs] {err:?}");
        }
    });
    Ok(job_id)
}

type Emit<'a> = &'a (dyn Fn(usize, usize, &str, f64) + Sync);

/// Where a job may write and find bundled components.
pub struct Places<'a> {
    /// Used when an input's folder isn't writable (Downloads).
    pub fallback: Option<&'a Path>,
    /// Folders that may contain the pdfium library.
    pub pdfium_dirs: &'a [PathBuf],
    /// Settings → Output → custom folder; None = next to each original.
    pub output_dir: Option<&'a Path>,
    /// Files (or PDF pages) worked on at once.
    pub workers: usize,
}

impl Places<'_> {
    /// Where the output for `input` goes.
    fn dir_for<'p>(&'p self, input: &'p Path) -> &'p Path {
        self.output_dir.unwrap_or_else(|| parent(input))
    }
}

/// Bundled resources first; in dev builds also the source tree (`scripts/fetch-pdfium.sh`).
pub fn pdfium_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = app
        .path()
        .resource_dir()
        .map(|dir| vec![dir.join("pdfium")])
        .unwrap_or_default();
    if cfg!(debug_assertions) {
        dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium"));
    }
    dirs
}

/// Run a job to completion. Batches continue past individual failures.
pub fn run(
    job_id: &str,
    request: &JobRequest,
    paths: &[PathBuf],
    places: &Places<'_>,
    cancel: &AtomicBool,
    emit: Emit<'_>,
) -> JobDone {
    let fallback = places.fallback;
    let mut done = JobDone {
        job_id: job_id.to_string(),
        outputs: Vec::new(),
        failures: Vec::new(),
        saved_to_fallback: false,
        cancelled: false,
        unchanged: Vec::new(),
    };
    let count = paths.len();
    let is_cancelled = || cancel.load(Ordering::Relaxed);
    // "File i is starting", from any worker thread. Files start in order, but two workers can report
    // a hair apart: never step the progress bar backwards.
    let reported = Mutex::new(None::<usize>);
    let progress = |i: usize| {
        if let Ok(mut last) = reported.lock() {
            if last.is_none_or(|l| i > l) {
                *last = Some(i);
                emit(i, count, &file_name(&paths[i]), percent(i, count));
            }
        }
    };
    let record = |done: &mut JobDone,
                  path: &Path,
                  result: Result<(Written, Option<Detail>), AppError>| match result
    {
        Ok((written, detail)) => {
            let detail = detail.unwrap_or_default();
            done.saved_to_fallback |= written.fell_back;
            done.outputs.push(OutputFile {
                bytes: std::fs::metadata(&written.path)
                    .map(|m| m.len())
                    .unwrap_or(0),
                path: written.path.to_string_lossy().into_owned(),
                width: detail.width,
                height: detail.height,
                quality: detail.quality,
                removed: detail.removed,
                made_with: detail.made_with,
                fonts_replaced: detail.fonts_replaced,
            });
        }
        Err(AppError::Cancelled) => done.cancelled = true,
        Err(AppError::AlreadySmall) => done.unchanged.push(file_name(path)),
        Err(error) => done.failures.push(Failure {
            name: file_name(path),
            error,
        }),
    };

    match request {
        // Many inputs → one PDF.
        JobRequest::ImagesToPdf { .. } | JobRequest::PdfMerge => {
            // The input being read, to name it if it fails.
            let current = AtomicUsize::new(0);
            let step = |i: usize| {
                if is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                progress(i);
                Ok(())
            };
            let first = &paths[0];
            let result = (|| {
                let (mut doc, name) = match request {
                    JobRequest::ImagesToPdf {
                        page_size,
                        margin,
                        turns,
                    } => (
                        pdf::images_to_pdf(
                            paths,
                            *page_size,
                            *margin,
                            places.workers,
                            &step,
                            &paths
                                .iter()
                                .map(|p| {
                                    turns
                                        .get(p.to_string_lossy().as_ref())
                                        .copied()
                                        .unwrap_or_default()
                                })
                                .collect::<Vec<_>>(),
                        )
                        .map_err(|failed| {
                            current.store(failed.index, Ordering::Relaxed);
                            failed.error
                        })?,
                        output::combined_pdf_name(first, count),
                    ),
                    _ => (
                        pdf::merge_pdfs(paths, &mut |i| {
                            current.store(i, Ordering::Relaxed);
                            step(i)
                        })?,
                        output::merged_pdf_name(first),
                    ),
                };
                output::write_output(places.dir_for(first), fallback, &name, |w| {
                    if is_cancelled() {
                        return Err(AppError::Cancelled);
                    }
                    Ok(doc.save_to(w)?)
                })
            })();
            let failed_at = current.load(Ordering::Relaxed);
            record(&mut done, &paths[failed_at], result.map(|w| (w, None)));
        }
        // One PDF → a folder of page images.
        JobRequest::PdfToImages { format, dpi, pages } => {
            let pdf = &paths[0];
            let mut step = |i: usize, total: usize, page: u32| {
                if is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                emit(i, total, &format!("Page {page}"), percent(i, total));
                Ok(())
            };
            let mut written = Vec::new();
            let result = pdf_render::pdfium(places.pdfium_dirs).and_then(|lib| {
                pdf_render::pdf_to_images(
                    lib,
                    pdf,
                    places.dir_for(pdf),
                    *format,
                    *dpi,
                    pages,
                    fallback,
                    &mut step,
                    &mut written,
                )
            });
            for file in written {
                record(&mut done, pdf, Ok((file, None)));
            }
            if let Err(err) = result {
                record(&mut done, pdf, Err(err));
            }
        }
        // One PDF → edited copies.
        JobRequest::PdfSplit { .. }
        | JobRequest::PdfExtract { .. }
        | JobRequest::PdfRotate { .. } => {
            let pdf = &paths[0];
            emit(0, 1, &file_name(pdf), 0.0);
            let mut written = Vec::new();
            let result = edit_pdf(request, pdf, places, &is_cancelled, emit, &mut written);
            for file in written {
                record(&mut done, pdf, Ok((file, None)));
            }
            if let Err(err) = result {
                record(&mut done, pdf, Err(err));
            }
        }
        // One output per input.
        JobRequest::ImageConvert { .. }
        | JobRequest::ImageResize { .. }
        | JobRequest::ImageCompress { .. }
        | JobRequest::ApplyPreset { .. }
        | JobRequest::ImageCrop { .. }
        | JobRequest::ImageRotate { .. }
        | JobRequest::StripMetadata
        | JobRequest::PdfCompress { .. }
        | JobRequest::OfficeToPdf { .. } => {
            // A few files at once; files already started finish when the job is cancelled.
            let results = crate::pool::map(count, places.workers, |i| {
                if is_cancelled() {
                    return None;
                }
                progress(i);
                let path = &paths[i];
                Some(convert_one(request, path, places.dir_for(path), fallback))
            });
            for (path, result) in paths.iter().zip(results) {
                match result.flatten() {
                    Some(result) => record(&mut done, path, result),
                    None => done.cancelled = true,
                }
            }
        }
    }
    if !done.cancelled {
        emit(count, count, "", 100.0);
    }
    done
}

fn convert_one(
    request: &JobRequest,
    path: &Path,
    out_dir: &Path,
    fallback: Option<&Path>,
) -> Result<(Written, Option<Detail>), AppError> {
    match request {
        JobRequest::StripMetadata => return strip_one(path, out_dir, fallback),
        JobRequest::PdfCompress { level } => {
            return compress_pdf_one(path, *level, out_dir, fallback)
        }
        JobRequest::OfficeToPdf { engines, quick } => {
            return office_one(path, engines, *quick, out_dir, fallback)
        }
        _ => {}
    }
    let picture = img::load(path)?;
    let icc = picture.icc.as_deref();
    let density = picture.density;
    let lossy = |format: OutputFormat| (format != OutputFormat::Png).then_some(());
    match request {
        JobRequest::ImageConvert { format, quality } => {
            let name = output::converted_name(path, format.extension());
            let written = output::write_output(out_dir, fallback, &name, |w| {
                img::encode(&picture.image, icc, *format, *quality, w)
            })?;
            let detail = detail_of(&picture.image, lossy(*format).map(|()| *quality));
            Ok((written, Some(detail)))
        }
        JobRequest::ImageResize { mode } => {
            let resized = img::resize(&picture.image, *mode);
            write_same_format(path, out_dir, fallback, "resized", &resized, icc, density)
        }
        JobRequest::ImageCrop { rect, turn } => {
            let cropped = crop::crop(&crop::rotate(picture.image, *turn), *rect)?;
            write_same_format(path, out_dir, fallback, "cropped", &cropped, icc, density)
        }
        JobRequest::ImageRotate { turns } => {
            let turn = turns
                .get(path.to_string_lossy().as_ref())
                .copied()
                .unwrap_or_default();
            if turn == Turn::None {
                return Err(AppError::invalid("This image doesn't need turning."));
            }
            let rotated = crop::rotate(picture.image, turn);
            write_same_format(path, out_dir, fallback, "rotated", &rotated, icc, density)
        }
        JobRequest::ImageCompress { target } => {
            let plan = compress::plan_for_target(path, *target);
            let encoded = compress::compress(&picture.image, icc, &plan)?;
            let name = output::suffixed_name(path, "compressed", plan.format.extension());
            write_encoded(out_dir, fallback, &name, encoded)
        }
        JobRequest::ApplyPreset { preset } => {
            let (shaped, plan) = compress::apply_preset_geometry(&picture.image, preset)?;
            let encoded = compress::compress(&shaped, icc, &plan)?;
            let name = output::suffixed_name(path, &file_safe(&preset.id), plan.format.extension());
            write_encoded(out_dir, fallback, &name, encoded)
        }
        _ => Err(AppError::internal("not a per-file action")),
    }
}

/// Save `image` as `<name>-<suffix>` in the input's own format (HEIC: a JPG about as big as the
/// original, see `density`), at the quality used whenever the format is kept.
fn write_same_format(
    path: &Path,
    out_dir: &Path,
    fallback: Option<&Path>,
    suffix: &str,
    image: &image::DynamicImage,
    icc: Option<&[u8]>,
    density: Option<f64>,
) -> Result<(Written, Option<Detail>), AppError> {
    let (bytes, quality) = img::encode_same_format(image, icc, path, density)?;
    let name = output::suffixed_name(path, suffix, &img::same_format_extension(path));
    let written = output::write_output(out_dir, fallback, &name, |w| Ok(w.write_all(&bytes)?))?;
    Ok((written, Some(detail_of(image, quality))))
}

/// Split, pick pages or rotate pages of one PDF. Split writes a `<name> parts` folder (removed
/// again if nothing got written); the others write one PDF next to the original.
fn edit_pdf(
    request: &JobRequest,
    pdf: &Path,
    places: &Places<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
    emit: Emit<'_>,
    written: &mut Vec<Written>,
) -> Result<(), AppError> {
    let out_dir = places.dir_for(pdf);
    let write = |dir: &Path, fallback: Option<&Path>, name: &str, bytes: &[u8]| {
        output::write_output(dir, fallback, name, |w| Ok(w.write_all(bytes)?))
    };
    let mut doc = pdf_edit::open(pdf)?;
    match request {
        JobRequest::PdfExtract { pages } => {
            let bytes = pdf_edit::save(&mut pdf_edit::keep_pages(&doc, pages)?)?;
            let name = output::pages_pdf_name(pdf, pages);
            written.push(write(out_dir, places.fallback, &name, &bytes)?);
        }
        JobRequest::PdfRotate { turns } => {
            if turns.values().all(|t| *t == Turn::None) {
                return Err(AppError::invalid(
                    "Nothing to turn: every page is already the way you picked.",
                ));
            }
            let turns = turns
                .iter()
                .map(|(page, turn)| {
                    Ok((
                        page.parse::<u32>()
                            .map_err(|_| AppError::internal("bad page"))?,
                        *turn,
                    ))
                })
                .collect::<Result<HashMap<u32, Turn>, AppError>>()?;
            pdf_edit::rotate_pages(&mut doc, &turns)?;
            let bytes = pdf_edit::save(&mut doc)?;
            let name = output::suffixed_name(pdf, "rotated", "pdf");
            written.push(write(out_dir, places.fallback, &name, &bytes)?);
        }
        JobRequest::PdfSplit { parts } => {
            if parts.len() < 2 {
                return Err(AppError::invalid("Add at least one cut to split the PDF."));
            }
            // Build the parts a few at a time, then write them in order.
            let count = parts.len();
            let built = crate::pool::map(count, places.workers, |i| {
                if cancelled() {
                    return Err(AppError::Cancelled);
                }
                emit(i, count, &format!("Part {}", i + 1), percent(i, count));
                pdf_edit::save(&mut pdf_edit::keep_pages(&doc, &parts[i])?)
            });
            let (folder, fell_back) =
                output::create_folder(out_dir, places.fallback, &output::parts_folder_name(pdf))?;
            let result = (|| -> Result<(), AppError> {
                for (pages, bytes) in parts.iter().zip(built) {
                    let bytes =
                        bytes.unwrap_or_else(|| Err(AppError::internal("part worker stopped")))?;
                    let mut file =
                        write(&folder, None, &output::pages_pdf_name(pdf, pages), &bytes)?;
                    file.fell_back = fell_back;
                    written.push(file);
                }
                Ok(())
            })();
            if written.is_empty() {
                let _ = std::fs::remove_dir(&folder); // only if still empty
            }
            result?;
        }
        _ => return Err(AppError::internal("not a PDF edit")),
    }
    Ok(())
}

/// Make one PDF smaller into `<name>-compressed.pdf`, unless it would save under 5 %.
fn compress_pdf_one(
    path: &Path,
    level: Level,
    out_dir: &Path,
    fallback: Option<&Path>,
) -> Result<(Written, Option<Detail>), AppError> {
    // Pictures inside one PDF two at a time (files already run in parallel).
    let bytes = pdf_compress::compress_pdf(path, level, 2)?;
    let original = std::fs::metadata(path)?.len() as f64;
    if bytes.len() as f64 > original * (1.0 - pdf_compress::MIN_SAVING) {
        return Err(AppError::AlreadySmall);
    }
    let name = output::suffixed_name(path, "compressed", "pdf");
    let written = output::write_output(out_dir, fallback, &name, |w| Ok(w.write_all(&bytes)?))?;
    Ok((written, None))
}

/// One Office file → `<name>.pdf`, made the way picked for its type (or the best available).
fn office_one(
    path: &Path,
    engines: &HashMap<String, crate::office::Engine>,
    quick: crate::office::QuickOptions,
    out_dir: &Path,
    fallback: Option<&Path>,
) -> Result<(Written, Option<Detail>), AppError> {
    use crate::office::{self, DocType};
    let key = match DocType::of(path) {
        Some(DocType::Word) => "word",
        Some(DocType::Slides) => "slides",
        Some(DocType::Sheet) => "sheet",
        None => return Err(AppError::internal("not an Office file")),
    };
    let engine = office::choose(path, engines.get(key).copied(), &office::engines())?;
    let made = office::to_pdf(path, engine, quick)?;
    let name = output::converted_name(path, "pdf");
    let written = output::write_output(out_dir, fallback, &name, |w| Ok(w.write_all(&made.pdf)?))?;
    let detail = Detail {
        made_with: Some(engine),
        fonts_replaced: made.fonts,
        ..Detail::default()
    };
    Ok((written, Some(detail)))
}

fn detail_of(image: &image::DynamicImage, quality: Option<u8>) -> Detail {
    Detail {
        width: Some(image.width()),
        height: Some(image.height()),
        quality,
        ..Detail::default()
    }
}

/// Remove location, camera, date and other personal info into `<name>-clean.<ext>`. JPEG, PNG and
/// WebP keep their picture data byte-for-byte; TIFF, BMP and GIF are re-saved (lossless; GIF keeps
/// its first frame) and HEIC is re-saved as HEIC from its clean pixels — re-saving never writes
/// metadata.
fn strip_one(
    path: &Path,
    out_dir: &Path,
    fallback: Option<&Path>,
) -> Result<(Written, Option<Detail>), AppError> {
    let resave = |from: &Path| -> Result<Vec<u8>, AppError> {
        let picture = img::load(from)?;
        let mut bytes = Vec::new();
        img::encode_like(
            &picture.image,
            picture.icc.as_deref(),
            path,
            &mut std::io::Cursor::new(&mut bytes),
        )?;
        Ok(bytes)
    };
    let (bytes, found) = if crate::heic::is_heic(path) {
        // The readable copy is a TIFF that still carries the photo's info: read what it had.
        let readable = crate::heic::decode(path)?;
        let found = metadata::read_tiff(&std::fs::read(readable.path())?).0;
        let picture = img::load(readable.path())?;
        (
            crate::heic::encode(&picture.image, picture.icc.as_deref())?,
            found,
        )
    } else {
        let original = std::fs::read(path)?;
        match metadata::clean(&original) {
            Some(cleaned) => {
                let cleaned = cleaned?;
                (cleaned.bytes, cleaned.found)
            }
            None => {
                let found = if matches!(img::same_format_extension(path).as_str(), "tiff") {
                    metadata::read_tiff(&original).0
                } else {
                    Found::default()
                };
                (resave(path)?, found)
            }
        }
    };
    let extension = if crate::heic::is_heic(path) {
        output::extension(path) // stays HEIC (or HEIF)
    } else {
        img::same_format_extension(path)
    };
    let name = output::suffixed_name(path, "clean", &extension);
    let written = output::write_output(out_dir, fallback, &name, |w| Ok(w.write_all(&bytes)?))?;
    let detail = Detail {
        removed: Some(found),
        ..Detail::default()
    };
    Ok((written, Some(detail)))
}

fn write_encoded(
    out_dir: &Path,
    fallback: Option<&Path>,
    name: &str,
    encoded: compress::Encoded,
) -> Result<(Written, Option<Detail>), AppError> {
    let detail = Detail {
        width: Some(encoded.width),
        height: Some(encoded.height),
        quality: encoded.quality,
        ..Detail::default()
    };
    let written =
        output::write_output(
            out_dir,
            fallback,
            name,
            |w| Ok(w.write_all(&encoded.bytes)?),
        )?;
    Ok((written, Some(detail)))
}

/// Preset ids end up in file names: keep letters, digits, `-` and `_`.
fn file_safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn percent(index: usize, count: usize) -> f64 {
    index as f64 / count.max(1) as f64 * 100.0
}

fn parent(path: &Path) -> &Path {
    path.parent().unwrap_or(Path::new("."))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-jobs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixtures(dir: &Path) -> Vec<PathBuf> {
        let jpg = dir.join("photo.jpg");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 48, Rgb([200, 80, 20])))
            .save_with_format(&jpg, ImageFormat::Jpeg)
            .unwrap();
        let png = dir.join("logo.png");
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(32, 32, Rgba([0, 0, 255, 100])))
            .save(&png)
            .unwrap();
        let webp = dir.join("pic.webp");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(20, 30, Rgb([1, 2, 3])))
            .save(&webp)
            .unwrap();
        vec![jpg, png, webp]
    }

    fn snapshot(paths: &[PathBuf]) -> Vec<Vec<u8>> {
        paths.iter().map(|p| fs::read(p).unwrap()).collect()
    }

    fn places_for_tests() -> Vec<PathBuf> {
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium")]
    }

    fn run_quiet(request: JobRequest, paths: &[PathBuf]) -> JobDone {
        let dirs = places_for_tests();
        let places = Places {
            fallback: None,
            pdfium_dirs: &dirs,
            output_dir: None,
            workers: 3,
        };
        run(
            "t",
            &request,
            paths,
            &places,
            &AtomicBool::new(false),
            &|_, _, _, _| {},
        )
    }

    fn names(done: &JobDone) -> Vec<String> {
        done.outputs
            .iter()
            .map(|o| file_name(Path::new(&o.path)))
            .collect()
    }

    #[test]
    fn every_action_leaves_originals_byte_identical() {
        let dir = temp_dir("originals");
        let inputs = fixtures(&dir);
        let before = snapshot(&inputs);

        let convert = run_quiet(
            JobRequest::ImageConvert {
                format: OutputFormat::Jpg,
                quality: 85,
            },
            &inputs,
        );
        assert_eq!(names(&convert), ["photo (1).jpg", "logo.jpg", "pic.jpg"]);

        let resize = run_quiet(
            JobRequest::ImageResize {
                mode: ResizeMode::Percent { percent: 50.0 },
            },
            &inputs,
        );
        assert_eq!(
            names(&resize),
            ["photo-resized.jpg", "logo-resized.png", "pic-resized.webp"]
        );
        let resized = image::open(dir.join("photo-resized.jpg")).unwrap();
        assert_eq!((resized.width(), resized.height()), (32, 24));

        let pdf = run_quiet(
            JobRequest::ImagesToPdf {
                page_size: PageSize::A4,
                margin: Margin::Small,
                turns: HashMap::new(),
            },
            &inputs,
        );
        assert_eq!(names(&pdf), ["photo +2.pdf"]);

        let merged_inputs = vec![
            PathBuf::from(&pdf.outputs[0].path),
            PathBuf::from(&pdf.outputs[0].path),
        ];
        let merge = run_quiet(JobRequest::PdfMerge, &merged_inputs);
        assert_eq!(names(&merge), ["photo +2 merged.pdf"]);
        let merged = lopdf::Document::load(&merge.outputs[0].path).unwrap();
        assert_eq!(merged.get_pages().len(), 6);

        let pages = run_quiet(
            JobRequest::PdfToImages {
                format: OutputFormat::Png,
                dpi: 72,
                pages: PageSelection::All,
            },
            &merged_inputs[..1],
        );
        assert_eq!(
            names(&pages),
            ["photo +2-01.png", "photo +2-02.png", "photo +2-03.png"]
        );

        let compressed = run_quiet(
            JobRequest::ImageCompress {
                target: CompressTarget::Size { max_kb: 5 },
            },
            &inputs,
        );
        assert_eq!(
            names(&compressed),
            [
                "photo-compressed.jpg",
                "logo-compressed.png",
                "pic-compressed.webp"
            ]
        );
        assert!(compressed.outputs.iter().all(|o| o.bytes <= 5000));
        let jpg = &compressed.outputs[0];
        assert_eq!((jpg.width, jpg.height), (Some(64), Some(48)));
        assert!(jpg.quality.is_some());
        assert_eq!(compressed.outputs[1].quality, None, "PNG is lossless");

        let preset: Preset = serde_json::from_str(
            r#"{"id":"tiny thumb","label":"Tiny","format":"jpg","width":32,"height":32,"maxKB":5}"#,
        )
        .unwrap();
        let preset_done = run_quiet(JobRequest::ApplyPreset { preset }, &inputs[..1]);
        assert_eq!(names(&preset_done), ["photo-tiny-thumb.jpg"]);
        assert_eq!(preset_done.outputs[0].width, Some(32));

        // 64 × 48 photo: turn right (48 × 64), then keep a 20 × 30 box.
        let cropped = run_quiet(
            JobRequest::ImageCrop {
                rect: CropRect {
                    x: 10,
                    y: 20,
                    width: 20,
                    height: 30,
                },
                turn: Turn::Right,
            },
            &inputs[..1],
        );
        assert_eq!(names(&cropped), ["photo-cropped.jpg"]);
        assert_eq!(
            (cropped.outputs[0].width, cropped.outputs[0].height),
            (Some(20), Some(30))
        );

        // Remove info: same formats, -clean names; nothing to find in these plain files.
        let cleaned = run_quiet(JobRequest::StripMetadata, &inputs);
        assert_eq!(
            names(&cleaned),
            ["photo-clean.jpg", "logo-clean.png", "pic-clean.webp"]
        );
        assert!(cleaned
            .outputs
            .iter()
            .all(|o| o.removed == Some(Found::default())));

        // Each image its own turn.
        let key = |p: &PathBuf| p.to_string_lossy().into_owned();
        let turns = HashMap::from([
            (key(&inputs[0]), Turn::Half),
            (key(&inputs[1]), Turn::Right),
            (key(&inputs[2]), Turn::Left),
        ]);
        let rotated = run_quiet(JobRequest::ImageRotate { turns }, &inputs);
        assert_eq!(
            names(&rotated),
            ["photo-rotated.jpg", "logo-rotated.png", "pic-rotated.webp"]
        );
        let half = image::open(dir.join("photo-rotated.jpg")).unwrap();
        assert_eq!((half.width(), half.height()), (64, 48));
        let turned = image::open(dir.join("pic-rotated.webp")).unwrap();
        assert_eq!((turned.width(), turned.height()), (30, 20));

        for done in [
            &convert,
            &resize,
            &pdf,
            &merge,
            &pages,
            &compressed,
            &preset_done,
            &cropped,
            &rotated,
            &cleaned,
        ] {
            assert!(done.failures.is_empty(), "{:?}", done.failures);
            assert!(!done.cancelled);
        }
        assert_eq!(snapshot(&inputs), before, "originals must not change");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remove_info_reports_what_it_found_and_leaves_originals_alone() {
        let dir = temp_dir("strip");
        let phone = dir.join("IMG_1.jpg");
        fs::write(&phone, metadata::tests::phone_jpeg()).unwrap();
        let mut inputs = vec![phone];
        if cfg!(target_os = "macos") {
            let heic = dir.join("IMG_2.heic");
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sideways-gps.heic"),
                &heic,
            )
            .unwrap();
            inputs.push(heic);
        }
        let before = snapshot(&inputs);

        let done = run_quiet(JobRequest::StripMetadata, &inputs);
        assert!(done.failures.is_empty(), "{:?}", done.failures);
        assert_eq!(snapshot(&inputs), before, "originals must not change");
        let jpg = &done.outputs[0];
        assert_eq!(names(&done)[0], "IMG_1-clean.jpg");
        assert_eq!(
            jpg.removed,
            Some(Found {
                location: true,
                camera: true,
                date: true,
                other: true
            })
        );
        if cfg!(target_os = "macos") {
            assert_eq!(names(&done)[1], "IMG_2-clean.heic", "HEIC stays HEIC");
            let removed = done.outputs[1].removed.unwrap();
            assert!(removed.location && removed.camera);
            let out = Path::new(&done.outputs[1].path);
            let clean = img::load(out).unwrap();
            assert_eq!(
                (clean.image.width(), clean.image.height()),
                (200, 300),
                "upright"
            );
            let readable = crate::heic::decode(out).unwrap();
            let (left, _) = metadata::read_tiff(&fs::read(readable.path()).unwrap());
            assert!(
                !left.location && !left.camera,
                "nothing personal left: {left:?}"
            );

            // Other same-format actions turn HEIC into a JPG at quality 80–90.
            let resized = run_quiet(
                JobRequest::ImageResize {
                    mode: ResizeMode::Percent { percent: 50.0 },
                },
                &inputs[1..],
            );
            assert_eq!(names(&resized), ["IMG_2-resized.jpg"]);
            assert!(matches!(resized.outputs[0].quality, Some(80..=90)));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pdf_split_pick_rotate_and_compress() {
        let dir = temp_dir("pdf-tools");
        let pdf = crate::convert::pdf_edit::tests::numbered_pdf(&dir, 5);
        let before = snapshot(std::slice::from_ref(&pdf));

        let split = run_quiet(
            JobRequest::PdfSplit {
                parts: vec![vec![1, 2], vec![3], vec![4, 5]],
            },
            std::slice::from_ref(&pdf),
        );
        assert!(split.failures.is_empty(), "{:?}", split.failures);
        assert_eq!(
            names(&split),
            [
                "doc (pages 1-2).pdf",
                "doc (page 3).pdf",
                "doc (pages 4-5).pdf"
            ]
        );
        assert!(split.outputs.iter().all(|o| o.path.contains("/doc parts/")));

        let picked = run_quiet(
            JobRequest::PdfExtract { pages: vec![2, 5] },
            std::slice::from_ref(&pdf),
        );
        assert_eq!(names(&picked), ["doc (pages 2, 5).pdf"]);
        let kept = lopdf::Document::load(&picked.outputs[0].path).unwrap();
        assert_eq!(crate::convert::pdf_edit::tests::widths(&kept), [110, 140]);

        let rotated = run_quiet(
            JobRequest::PdfRotate {
                turns: HashMap::from([("1".to_string(), Turn::Right)]),
            },
            std::slice::from_ref(&pdf),
        );
        assert_eq!(names(&rotated), ["doc-rotated.pdf"]);
        let nothing = run_quiet(
            JobRequest::PdfRotate {
                turns: HashMap::from([("1".to_string(), Turn::None)]),
            },
            std::slice::from_ref(&pdf),
        );
        assert_eq!(nothing.failures[0].error.code(), "invalid_input");
        let one_part = run_quiet(
            JobRequest::PdfSplit {
                parts: vec![vec![1, 2, 3, 4, 5]],
            },
            std::slice::from_ref(&pdf),
        );
        assert_eq!(one_part.failures[0].error.code(), "invalid_input");
        assert!(
            !dir.join("doc parts (1)").exists(),
            "no empty folder left behind"
        );

        // Make smaller: the scan shrinks, the plain-text letter is reported as already small.
        let scan = crate::convert::pdf_compress::tests::scan_pdf(&dir, 1);
        let letter = crate::convert::pdf_compress::tests::text_pdf(&dir);
        let compressed = run_quiet(
            JobRequest::PdfCompress {
                level: Level::Balanced,
            },
            &[scan.clone(), letter.clone()],
        );
        assert!(compressed.failures.is_empty(), "{:?}", compressed.failures);
        assert_eq!(names(&compressed), ["scan-compressed.pdf"]);
        assert_eq!(compressed.unchanged, ["letter.pdf"]);
        assert!(compressed.outputs[0].bytes < fs::metadata(&scan).unwrap().len() / 3);

        assert_eq!(
            snapshot(std::slice::from_ref(&pdf)),
            before,
            "original untouched"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn office_files_become_pdfs_and_say_how() {
        let dir = temp_dir("office");
        let xlsx = dir.join("budget.xlsx");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/budget.xlsx"),
            &xlsx,
        )
        .unwrap();
        let csv = dir.join("scores.csv");
        fs::write(&csv, "Name,Score\nAsha,91\n").unwrap();
        let pptx = dir.join("deck.pptx");
        fs::write(&pptx, b"PK not really a deck").unwrap();
        let before = snapshot(&[xlsx.clone(), csv.clone(), pptx.clone()]);

        let request = JobRequest::OfficeToPdf {
            engines: HashMap::from([("sheet".to_string(), crate::office::Engine::Quick)]),
            quick: crate::office::QuickOptions::default(),
        };
        let done = run_quiet(request, &[xlsx.clone(), csv.clone()]);
        assert!(done.failures.is_empty(), "{:?}", done.failures);
        assert_eq!(names(&done), ["budget.pdf", "scores.pdf"]);
        assert!(done
            .outputs
            .iter()
            .all(|o| o.made_with == Some(crate::office::Engine::Quick)));

        // Without Keynote or LibreOffice, PowerPoint says what to install (skipped on a Mac that
        // has one, where it would really convert).
        let has_app = crate::office::engines().iter().any(|e| {
            e.installed && e.engine != crate::office::Engine::Quick && e.engine.handles(&pptx)
        });
        if !has_app {
            let done = run_quiet(
                JobRequest::OfficeToPdf {
                    engines: HashMap::new(),
                    quick: crate::office::QuickOptions::default(),
                },
                std::slice::from_ref(&pptx),
            );
            assert_eq!(done.failures[0].error.code(), "invalid_input");
            assert!(done.failures[0]
                .error
                .to_string()
                .contains("Keynote or LibreOffice"));
        }
        assert_eq!(snapshot(&[xlsx, csv, pptx]), before, "originals untouched");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn batches_continue_past_failures() {
        let dir = temp_dir("failures");
        let mut inputs = fixtures(&dir);
        let broken = dir.join("broken.png");
        fs::write(&broken, b"not a png").unwrap();
        inputs.insert(1, broken);

        let done = run_quiet(
            JobRequest::ImageConvert {
                format: OutputFormat::Png,
                quality: 85,
            },
            &inputs,
        );
        assert_eq!(done.outputs.len(), 3);
        assert_eq!(done.failures.len(), 1);
        assert_eq!(done.failures[0].name, "broken.png");
        assert_eq!(done.failures[0].error.code(), "corrupt");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn combine_failure_names_the_bad_file_and_writes_nothing() {
        let dir = temp_dir("combine-fail");
        let mut inputs = fixtures(&dir);
        let broken = dir.join("scan.png");
        fs::write(&broken, b"nope").unwrap();
        inputs.push(broken);

        let done = run_quiet(
            JobRequest::ImagesToPdf {
                page_size: PageSize::Fit,
                margin: Margin::None,
                turns: HashMap::new(),
            },
            &inputs,
        );
        assert!(done.outputs.is_empty());
        assert_eq!(done.failures[0].name, "scan.png");
        assert!(!dir.join("photo +3.pdf").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancel_stops_between_files_and_reports_progress() {
        let dir = temp_dir("cancel");
        let inputs = fixtures(&dir);
        let cancel = AtomicBool::new(false);
        let seen = Mutex::new(Vec::new());
        let emit = |i: usize, n: usize, _: &str, _: f64| {
            seen.lock().unwrap().push((i, n));
            if i == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        };
        let request = JobRequest::ImageResize {
            mode: ResizeMode::LongEdge { px: 10 },
        };
        let dirs = places_for_tests();
        let places = Places {
            fallback: None,
            pdfium_dirs: &dirs,
            output_dir: None,
            workers: 1,
        };
        let done = run("t", &request, &inputs, &places, &cancel, &emit);
        assert!(done.cancelled);
        assert_eq!(done.outputs.len(), 2, "files already started finish");
        assert_eq!(*seen.lock().unwrap(), [(0, 3), (1, 3)]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn requests_parse_from_ui_json() {
        let r: JobRequest =
            serde_json::from_str(r#"{"action":"images_to_pdf","pageSize":"a4","margin":"small"}"#)
                .unwrap();
        assert_eq!(
            r,
            JobRequest::ImagesToPdf {
                page_size: PageSize::A4,
                margin: Margin::Small,
                turns: HashMap::new()
            }
        );
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"image_resize","mode":{"kind":"long_edge","px":1600}}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::ImageResize {
                mode: ResizeMode::LongEdge { px: 1600 }
            }
        );
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"image_crop","rect":{"x":1,"y":2,"width":3,"height":4},"turn":90}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::ImageCrop {
                rect: CropRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4
                },
                turn: Turn::Right
            }
        );
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"image_rotate","turns":{"/a/b.jpg":180,"/a/c.png":90}}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::ImageRotate {
                turns: HashMap::from([
                    ("/a/b.jpg".to_string(), Turn::Half),
                    ("/a/c.png".to_string(), Turn::Right)
                ])
            }
        );
        let r: JobRequest =
            serde_json::from_str(r#"{"action":"pdf_rotate","turns":{"1":90,"3":270}}"#).unwrap();
        assert_eq!(
            r,
            JobRequest::PdfRotate {
                turns: HashMap::from([
                    ("1".to_string(), Turn::Right),
                    ("3".to_string(), Turn::Left)
                ])
            }
        );
        let r: JobRequest =
            serde_json::from_str(r#"{"action":"pdf_compress","level":"smallest"}"#).unwrap();
        assert_eq!(
            r,
            JobRequest::PdfCompress {
                level: Level::Smallest
            }
        );
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"office_to_pdf","engines":{"slides":"keynote","word":"quick"},"quick":{"a4":false,"landscape":true,"firstSheetOnly":false}}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::OfficeToPdf {
                engines: HashMap::from([
                    ("slides".to_string(), crate::office::Engine::Keynote),
                    ("word".to_string(), crate::office::Engine::Quick)
                ]),
                quick: crate::office::QuickOptions {
                    a4: false,
                    landscape: true,
                    first_sheet_only: false
                }
            }
        );
        let r: JobRequest = serde_json::from_str(r#"{"action":"strip_metadata"}"#).unwrap();
        assert_eq!(r, JobRequest::StripMetadata);
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"images_to_pdf","pageSize":"fit","margin":"none","turns":{"/a/b.jpg":90}}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::ImagesToPdf {
                page_size: PageSize::Fit,
                margin: Margin::None,
                turns: HashMap::from([("/a/b.jpg".to_string(), Turn::Right)])
            }
        );
        let r: JobRequest = serde_json::from_str(r#"{"action":"pdf_merge"}"#).unwrap();
        assert_eq!(r, JobRequest::PdfMerge);
        let r: JobRequest = serde_json::from_str(
            r#"{"action":"pdf_to_images","format":"jpg","dpi":150,"pages":{"kind":"range","spec":"1-3,5"}}"#,
        )
        .unwrap();
        assert_eq!(
            r,
            JobRequest::PdfToImages {
                format: OutputFormat::Jpg,
                dpi: 150,
                pages: PageSelection::Range {
                    spec: "1-3,5".into()
                }
            }
        );
    }

    // ---- SPECS §14 acceptance criteria and the 50-file batch (M7) ----

    /// A phone-sized JPEG (4032 × 3024) with photo-like detail, so file sizes are realistic.
    fn phone_photo(path: &Path, seed: u32) {
        let mut state = seed.wrapping_mul(2_654_435_761);
        let img = RgbImage::from_fn(4032, 3024, |x, y| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let grain = (state >> 27) as u8;
            Rgb([
                ((x / 16) as u8).wrapping_add(grain),
                ((y / 12) as u8).wrapping_add(grain),
                (((x + y) / 28) as u8).wrapping_add(grain),
            ])
        });
        DynamicImage::ImageRgb8(img)
            .save_with_format(path, ImageFormat::Jpeg)
            .unwrap();
    }

    #[test]
    fn acceptance_five_phone_photos_become_a_pdf_within_a_second() {
        let dir = temp_dir("accept-pdf");
        let photos: Vec<PathBuf> = (0..5)
            .map(|i| {
                let p = dir.join(format!("IMG_{i}.jpg"));
                phone_photo(&p, i);
                p
            })
            .collect();
        let started = std::time::Instant::now();
        let done = run_quiet(
            JobRequest::ImagesToPdf {
                page_size: PageSize::Fit,
                margin: Margin::None,
                turns: HashMap::new(),
            },
            &photos,
        );
        let took = started.elapsed();
        assert_eq!(names(&done), ["IMG_0 +4.pdf"]);
        let pdf = lopdf::Document::load(&done.outputs[0].path).unwrap();
        assert_eq!(pdf.get_pages().len(), 5);
        assert_eq!(
            done.outputs[0].path,
            dir.join("IMG_0 +4.pdf").to_string_lossy()
        );
        assert!(took.as_millis() < 1000, "took {took:?}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn acceptance_ten_page_pdf_becomes_ten_ordered_jpgs_at_150_dpi() {
        let dir = temp_dir("accept-pages");
        let pages: Vec<PathBuf> = (0..10)
            .map(|i| {
                let p = dir.join(format!("p{i}.png"));
                DynamicImage::ImageRgb8(RgbImage::from_pixel(
                    300,
                    400,
                    Rgb([i as u8 * 20, 50, 90]),
                ))
                .save(&p)
                .unwrap();
                p
            })
            .collect();
        let pdf = run_quiet(
            JobRequest::ImagesToPdf {
                page_size: PageSize::A4,
                margin: Margin::None,
                turns: HashMap::new(),
            },
            &pages,
        );
        let pdf_path = PathBuf::from(&pdf.outputs[0].path);
        let done = run_quiet(
            JobRequest::PdfToImages {
                format: OutputFormat::Jpg,
                dpi: 150,
                pages: PageSelection::All,
            },
            &[pdf_path],
        );
        let expected: Vec<String> = (1..=10).map(|n| format!("p0 +9-{n:02}.jpg")).collect();
        assert_eq!(names(&done), expected);
        assert!(done
            .outputs
            .iter()
            .all(|o| o.path.contains("/p0 +9 pages/")));
        // A4 at 150 DPI: 1240 × 1754 px.
        let first = image::open(&done.outputs[0].path).unwrap();
        assert_eq!((first.width(), first.height()), (1240, 1754));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn acceptance_four_mb_photo_compresses_under_50_kb_and_reports_it() {
        let dir = temp_dir("accept-compress");
        let photo = dir.join("big.jpg");
        let img = RgbImage::from_fn(4032, 3024, |x, y| {
            let n = (x.wrapping_mul(7919) ^ y.wrapping_mul(104_729)) % 64;
            Rgb([
                (x % 256) as u8 ^ n as u8,
                (y % 256) as u8,
                ((x * y) % 251) as u8,
            ])
        });
        let file = fs::File::create(&photo).unwrap();
        image::codecs::jpeg::JpegEncoder::new_with_quality(file, 95)
            .encode_image(&DynamicImage::ImageRgb8(img))
            .unwrap();
        let size = fs::metadata(&photo).unwrap().len();
        assert!(size > 3_000_000, "fixture should be phone-sized, is {size}");

        let done = run_quiet(
            JobRequest::ImageCompress {
                target: CompressTarget::Size { max_kb: 50 },
            },
            &[photo],
        );
        let out = &done.outputs[0];
        assert!(out.bytes <= 50_000, "{} bytes", out.bytes);
        assert!(out.width.is_some() && out.height.is_some() && out.quality.is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn batch_of_fifty_reports_progress_and_each_failure() {
        let dir = temp_dir("batch50");
        let mut inputs = Vec::new();
        for i in 0..50 {
            let p = dir.join(format!("photo_{i:02}.png"));
            if i == 7 || i == 31 {
                fs::write(&p, b"broken").unwrap();
            } else {
                DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([i as u8, 0, 0])))
                    .save(&p)
                    .unwrap();
            }
            inputs.push(p);
        }
        let progress = Mutex::new(Vec::new());
        let dirs = places_for_tests();
        let places = Places {
            fallback: None,
            pdfium_dirs: &dirs,
            output_dir: None,
            workers: 4,
        };
        let request = JobRequest::ImageConvert {
            format: OutputFormat::Jpg,
            quality: 80,
        };
        let done = run(
            "t",
            &request,
            &inputs,
            &places,
            &AtomicBool::new(false),
            &|i, n, name, pct| {
                progress.lock().unwrap().push((i, n, name.to_string(), pct));
            },
        );
        assert_eq!(done.outputs.len(), 48);
        let failed: Vec<_> = done.failures.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(failed, ["photo_07.png", "photo_31.png"]);
        // Four at a time: progress only moves forward, names match, and it ends at 100 %.
        let progress = progress.into_inner().unwrap();
        let (last, files) = progress.split_last().unwrap();
        assert_eq!(last.3, 100.0);
        assert!(
            files.len() >= 40,
            "nearly one report per file: {}",
            files.len()
        );
        assert!(files.windows(2).all(|w| w[0].0 < w[1].0), "never backwards");
        assert!(files
            .iter()
            .all(|(i, n, name, _)| *n == 50 && *name == format!("photo_{i:02}.png")));

        // Cancel half-way: files already started finish, the rest are skipped.
        let cancel = AtomicBool::new(false);
        let request = JobRequest::ImageResize {
            mode: ResizeMode::Percent { percent: 50.0 },
        };
        let done = run("t", &request, &inputs, &places, &cancel, &|i, _, _, _| {
            if i == 25 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(done.cancelled);
        let finished = done.outputs.len() + done.failures.len();
        assert!(
            (26..=29).contains(&finished),
            "files already started (up to 4 at once) finish: {finished}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
