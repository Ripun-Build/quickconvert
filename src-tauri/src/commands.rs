//! `#[tauri::command]` functions exposed to the UI. Each returns `Result<T, AppError>`
//! and gets a typed wrapper in `src/lib/ipc.ts`.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::errors::AppError;
use crate::files::{self, FileSet, ThumbnailCache};
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

/// Fit the popup to its content height (logical px).
#[tauri::command]
pub fn resize_popup(app: AppHandle, height: f64) -> Result<(), AppError> {
    popup::resize(&app, height)
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
