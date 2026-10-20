//! `#[tauri::command]` functions exposed to the UI. Each returns `Result<T, AppError>`
//! and gets a typed wrapper in `src/lib/ipc.ts`.

use tauri::{AppHandle, State};

use crate::errors::AppError;
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
