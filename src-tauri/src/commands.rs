//! `#[tauri::command]` functions exposed to the UI. Each returns `Result<T, AppError>`
//! and gets a typed wrapper in `src/lib/ipc.ts`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::convert::compress::{self, Preset};
use crate::convert::image::{self as img, Picture};
use crate::convert::pdf_render::{self, PageSize};
use crate::errors::AppError;
use crate::files::{self, FileSet, ThumbnailCache};
use crate::jobs::{self, JobRequest, Jobs};
use crate::popup::{self, PopupState};
use crate::presets;
use crate::settings::{AppSettings, Settings};
use crate::settings_window;
use crate::shortcut::{self, HotkeyState};

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

/// Page sizes of a PDF (points), for the pages editor.
#[tauri::command]
pub async fn pdf_page_sizes(app: AppHandle, path: String) -> Result<Vec<PageSize>, AppError> {
    blocking(move || {
        let lib = pdf_render::pdfium(&jobs::pdfium_dirs(&app))?;
        pdf_render::page_sizes(lib, &PathBuf::from(path))
    })
    .await
}

/// Thumbnails for the given 1-based pages.
#[tauri::command]
pub async fn pdf_thumbnails(
    app: AppHandle,
    path: String,
    pages: Vec<u32>,
) -> Result<Vec<String>, AppError> {
    blocking(move || {
        let lib = pdf_render::pdfium(&jobs::pdfium_dirs(&app))?;
        pdf_render::page_thumbnails(lib, &PathBuf::from(path), &pages, 160)
    })
    .await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
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

/// Shipped preset files (bundled first; the source tree in dev builds) and the user's file.
fn preset_paths(app: &AppHandle) -> (Vec<PathBuf>, Option<PathBuf>) {
    let mut shipped = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        shipped.push(dir.join("presets.json"));
    }
    if cfg!(debug_assertions) {
        shipped.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/presets.json"));
    }
    let user = app
        .path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("presets.json"));
    (shipped, user)
}

/// Shipped presets merged with the user's (app config dir).
#[tauri::command]
pub fn list_presets(app: AppHandle) -> Vec<Preset> {
    let (shipped, user) = preset_paths(&app);
    presets::load(&shipped, user.as_deref())
}

#[tauri::command]
pub fn presets_for_editor(app: AppHandle) -> Vec<presets::EditorPreset> {
    let (shipped, user) = preset_paths(&app);
    presets::for_editor(&shipped, user.as_deref())
}

/// Save the editor's full preset list (only differences from the built-ins are stored).
#[tauri::command]
pub fn save_presets(
    app: AppHandle,
    presets: Vec<Preset>,
) -> Result<Vec<presets::EditorPreset>, AppError> {
    let (shipped, user) = preset_paths(&app);
    let user = user.ok_or_else(|| AppError::internal("no config dir"))?;
    presets::save(&shipped, &user, &presets)?;
    Ok(presets::for_editor(&shipped, Some(&user)))
}

/// Bring back the built-in presets as shipped; the user's own presets stay.
#[tauri::command]
pub fn restore_presets(app: AppHandle) -> Result<Vec<presets::EditorPreset>, AppError> {
    let (shipped, user) = preset_paths(&app);
    let user = user.ok_or_else(|| AppError::internal("no config dir"))?;
    presets::restore(&shipped, &user)?;
    Ok(presets::for_editor(&shipped, Some(&user)))
}

/// What Compress / Apply preset would produce for one image, without writing anything.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub bytes: u64,
    pub original_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub quality: Option<u8>,
}

/// Last decoded image, so moving a slider doesn't decode a 12 MP photo every time.
static LAST_PICTURE: Mutex<Option<(PathBuf, std::time::SystemTime, Arc<Picture>)>> =
    Mutex::new(None);

fn cached_picture(path: &PathBuf) -> Result<Arc<Picture>, AppError> {
    let modified = std::fs::metadata(path)?.modified()?;
    if let Ok(slot) = LAST_PICTURE.lock() {
        if let Some((p, m, picture)) = slot.as_ref() {
            if p == path && *m == modified {
                return Ok(picture.clone());
            }
        }
    }
    let picture = Arc::new(img::load(path)?);
    if let Ok(mut slot) = LAST_PICTURE.lock() {
        *slot = Some((path.clone(), modified, picture.clone()));
    }
    Ok(picture)
}

#[tauri::command]
pub async fn estimate_output(path: String, request: JobRequest) -> Result<Estimate, AppError> {
    blocking(move || {
        let path = PathBuf::from(path);
        let picture = cached_picture(&path)?;
        let icc = picture.icc.as_deref();
        let encoded = match &request {
            JobRequest::ImageCompress { target } => compress::compress(
                &picture.image,
                icc,
                &compress::plan_for_target(&path, *target),
            )?,
            JobRequest::ApplyPreset { preset } => {
                let (shaped, plan) = compress::apply_preset_geometry(&picture.image, preset)?;
                compress::compress(&shaped, icc, &plan)?
            }
            _ => return Err(AppError::internal("no estimate for this action")),
        };
        Ok(Estimate {
            bytes: encoded.bytes.len() as u64,
            original_bytes: std::fs::metadata(&path)?.len(),
            width: encoded.width,
            height: encoded.height,
            quality: encoded.quality,
        })
    })
    .await
}

// ---- Settings ----

/// Settings plus what the Settings window needs to show alongside them.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: Settings,
    pub shortcut_label: String,
    pub default_shortcut_label: String,
    /// Why the shortcut couldn't be registered, if it couldn't.
    pub shortcut_conflict: Option<String>,
    pub launch_at_login: bool,
}

fn settings_view(app: &AppHandle) -> SettingsView {
    let hotkey = app.state::<HotkeyState>();
    SettingsView {
        settings: app.state::<AppSettings>().get(),
        shortcut_label: hotkey.label(),
        default_shortcut_label: shortcut::label(&shortcut::default_shortcut(), crate::IS_MAC),
        shortcut_conflict: hotkey.conflict(),
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

/// Tell every window (popup and Settings) about new settings.
fn broadcast(app: &AppHandle) -> SettingsView {
    let view = settings_view(app);
    let _ = app.emit("settings://changed", view.clone());
    view
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsView {
    settings_view(&app)
}

/// Save everything except the shortcut (that goes through `set_shortcut`) and apply it.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<SettingsView, AppError> {
    let store = app.state::<AppSettings>();
    let settings = Settings {
        shortcut: store.get().shortcut,
        ..settings
    };
    let saved = store.set(settings)?;
    crate::apply_appearance(&app, saved.appearance);
    Ok(broadcast(&app))
}

/// Change the global shortcut (None = default). Fails with a plain message if it can't be used;
/// the old shortcut then stays.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, accelerator: Option<String>) -> Result<SettingsView, AppError> {
    let new = shortcut::change(&app, accelerator.as_deref())?;
    let store = app.state::<AppSettings>();
    store.set(Settings {
        shortcut: accelerator.map(|_| shortcut::accelerator(&new)),
        ..store.get()
    })?;
    crate::refresh_tray(&app);
    Ok(broadcast(&app))
}

/// Pause the shortcut while the recorder listens for a new one.
#[tauri::command]
pub fn suspend_shortcut(app: AppHandle, paused: bool) {
    shortcut::suspend(&app, paused);
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<SettingsView, AppError> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| AppError::internal(e.to_string()))?;
    Ok(broadcast(&app))
}

#[tauri::command]
pub fn open_settings(app: AppHandle, tab: Option<String>) -> Result<(), AppError> {
    settings_window::open(&app, tab.as_deref(), None)
}
