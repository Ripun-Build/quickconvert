//! Job runner: conversions run on a blocking thread, report `job://progress`, finish with
//! `job://done`, and can be cancelled between files/pages.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::convert::image::{self as img, OutputFormat, ResizeMode};
use crate::convert::pdf::{self, Margin, PageSize};
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
    ImagesToPdf { page_size: PageSize, margin: Margin },
    PdfMerge,
    ImageConvert { format: OutputFormat, quality: u8 },
    ImageResize { mode: ResizeMode },
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDone {
    pub job_id: String,
    pub outputs: Vec<String>,
    pub failures: Vec<Failure>,
    /// Some output went to Downloads because the input's folder wasn't writable.
    pub saved_to_fallback: bool,
    pub cancelled: bool,
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
        let done = run(&id, &request, &paths, fallback.as_deref(), &cancel, &emit);

        let jobs = app.state::<Jobs>();
        if let Ok(mut outputs) = jobs.outputs.lock() {
            outputs.extend(done.outputs.iter().map(PathBuf::from));
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

type Emit<'a> = &'a dyn Fn(usize, usize, &str, f64);

/// Run a job to completion. Batches continue past individual failures.
pub fn run(
    job_id: &str,
    request: &JobRequest,
    paths: &[PathBuf],
    fallback: Option<&Path>,
    cancel: &AtomicBool,
    emit: Emit<'_>,
) -> JobDone {
    let mut done = JobDone {
        job_id: job_id.to_string(),
        outputs: Vec::new(),
        failures: Vec::new(),
        saved_to_fallback: false,
        cancelled: false,
    };
    let count = paths.len();
    let is_cancelled = || cancel.load(Ordering::Relaxed);
    let record = |done: &mut JobDone, path: &Path, result: Result<Written, AppError>| match result {
        Ok(written) => {
            done.saved_to_fallback |= written.fell_back;
            done.outputs
                .push(written.path.to_string_lossy().into_owned());
        }
        Err(AppError::Cancelled) => done.cancelled = true,
        Err(error) => done.failures.push(Failure {
            name: file_name(path),
            error,
        }),
    };

    match request {
        // Many inputs → one PDF.
        JobRequest::ImagesToPdf { .. } | JobRequest::PdfMerge => {
            let current = Cell::new(0);
            let mut step = |i: usize| {
                if is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                current.set(i);
                emit(i, count, &file_name(&paths[i]), percent(i, count));
                Ok(())
            };
            let first = &paths[0];
            let result = (|| {
                let (mut doc, name) = match request {
                    JobRequest::ImagesToPdf { page_size, margin } => (
                        pdf::images_to_pdf(paths, *page_size, *margin, &mut step)?,
                        output::combined_pdf_name(first, count),
                    ),
                    _ => (
                        pdf::merge_pdfs(paths, &mut step)?,
                        output::merged_pdf_name(first),
                    ),
                };
                output::write_output(parent(first), fallback, &name, |w| {
                    if is_cancelled() {
                        return Err(AppError::Cancelled);
                    }
                    Ok(doc.save_to(w)?)
                })
            })();
            record(&mut done, &paths[current.get()], result);
        }
        // One output per input.
        JobRequest::ImageConvert { .. } | JobRequest::ImageResize { .. } => {
            for (i, path) in paths.iter().enumerate() {
                if is_cancelled() {
                    done.cancelled = true;
                    break;
                }
                emit(i, count, &file_name(path), percent(i, count));
                let result = convert_one(request, path, fallback);
                record(&mut done, path, result);
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
    fallback: Option<&Path>,
) -> Result<Written, AppError> {
    let picture = img::load(path)?;
    let icc = picture.icc.as_deref();
    match request {
        JobRequest::ImageConvert { format, quality } => {
            let name = output::converted_name(path, format.extension());
            output::write_output(parent(path), fallback, &name, |w| {
                img::encode(&picture.image, icc, *format, *quality, w)
            })
        }
        JobRequest::ImageResize { mode } => {
            let resized = img::resize(&picture.image, *mode);
            let name = output::suffixed_name(path, "resized", &img::same_format_extension(path));
            output::write_output(parent(path), fallback, &name, |w| {
                img::encode_like(&resized, icc, path, w)
            })
        }
        _ => Err(AppError::internal("not a per-file action")),
    }
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

    fn run_quiet(request: JobRequest, paths: &[PathBuf]) -> JobDone {
        run(
            "t",
            &request,
            paths,
            None,
            &AtomicBool::new(false),
            &|_, _, _, _| {},
        )
    }

    fn names(done: &JobDone) -> Vec<String> {
        done.outputs
            .iter()
            .map(|p| file_name(Path::new(p)))
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
            },
            &inputs,
        );
        assert_eq!(names(&pdf), ["photo +2.pdf"]);

        let merged_inputs = vec![
            PathBuf::from(&pdf.outputs[0]),
            PathBuf::from(&pdf.outputs[0]),
        ];
        let merge = run_quiet(JobRequest::PdfMerge, &merged_inputs);
        assert_eq!(names(&merge), ["photo +2 merged.pdf"]);
        let merged = lopdf::Document::load(&merge.outputs[0]).unwrap();
        assert_eq!(merged.get_pages().len(), 6);

        for done in [&convert, &resize, &pdf, &merge] {
            assert!(done.failures.is_empty(), "{:?}", done.failures);
            assert!(!done.cancelled);
        }
        assert_eq!(snapshot(&inputs), before, "originals must not change");
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
        let done = run("t", &request, &inputs, None, &cancel, &emit);
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
                margin: Margin::Small
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
        let r: JobRequest = serde_json::from_str(r#"{"action":"pdf_merge"}"#).unwrap();
        assert_eq!(r, JobRequest::PdfMerge);
    }
}
