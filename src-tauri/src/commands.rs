//! `#[tauri::command]` functions exposed to the UI. Each returns `Result<T, AppError>`
//! and gets a typed wrapper in `src/lib/ipc.ts`.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::errors::AppError;
use crate::files::{self, FileSet, ThumbnailCache};
use crate::jobs::{self, JobRequest, Jobs};
use crate::popup::{self, PopupState};

#[tauri::command]
pub fn hide_popup(app: AppHandle) -> Result<(), AppError> {
    popup::hide(&app)
}

/// Called by the UI once the popup has painted its first frame after a show.
#[tauri::command]
pub fn popup_shown(state: State<'_, PopupState>) -> Result<(), AppError> {
    let requested = state
        .show_requested
        .lock()
        .map_err(|_| AppError::internal("show timer poisoned"))?
        .take();
    if cfg!(debug_assertions) {
        if let Some(t) = requested {
            println!(
                "[popup] visible {} ms after show request",
                t.elapsed().as_millis()
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn blur_enabled(state: State<'_, PopupState>) -> bool {
    state.blur
}

/// Fit the popup to its content (logical px). Editor views pass a wider `width`.
#[tauri::command]
pub fn resize_popup(app: AppHandle, width: f64, height: f64) -> Result<(), AppError> {
    popup::resize(&app, width, height)
}

/// A larger preview (for editors) plus the image's upright pixel size.
#[tauri::command]
pub async fn preview(path: String) -> Result<files::Preview, AppError> {
    tauri::async_runtime::spawn_blocking(move || files::preview(&PathBuf::from(path)))
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
}

/// While pinned (drop zone), clicking elsewhere doesn't hide the popup.
#[tauri::command]
pub fn set_popup_pinned(state: State<'_, PopupState>, pinned: bool) {
    state.set_pinned(pinned);
}

/// Bring the popup back to the front, e.g. after files were dropped from Finder.
#[tauri::command]
pub fn focus_popup(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(popup::LABEL) {
        window.set_focus()?;
    }
    Ok(())
}

/// Classify dropped or chosen files.
#[tauri::command]
pub async fn describe_files(paths: Vec<String>) -> Result<FileSet, AppError> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || files::describe(&paths))
        .await
        .map_err(|e| AppError::internal(e.to_string()))
}

/// Small PNG data URL for an image (cached by path + modification time).
#[tauri::command]
pub async fn thumbnail(app: AppHandle, path: String) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<ThumbnailCache>()
            .get_or_create(&PathBuf::from(path))
    })
    .await
    .map_err(|e| AppError::internal(e.to_string()))?
}

/// Start a conversion; progress arrives as `job://progress`, the result as `job://done`.
#[tauri::command]
pub fn start_job(
    app: AppHandle,
    request: JobRequest,
    paths: Vec<String>,
) -> Result<String, AppError> {
    jobs::start(
        &app,
        request,
        paths.into_iter().map(PathBuf::from).collect(),
    )
}

#[tauri::command]
pub fn cancel_job(jobs: State<'_, Jobs>, job_id: String) {
    jobs.cancel(&job_id);
}

/// Open a file this session wrote, in its default app.
#[tauri::command]
pub fn open_output(app: AppHandle, path: String) -> Result<(), AppError> {
    let path = written_by_us(&app, path)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::internal(e.to_string()))
}

/// Show a file this session wrote in Finder/Explorer.
#[tauri::command]
pub fn reveal_output(app: AppHandle, path: String) -> Result<(), AppError> {
    let path = written_by_us(&app, path)?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::internal(e.to_string()))
}

fn written_by_us(app: &AppHandle, path: String) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(path);
    if app.state::<Jobs>().wrote(&path) {
        Ok(path)
    } else {
        Err(AppError::internal("not an output of this session"))
    }
}
