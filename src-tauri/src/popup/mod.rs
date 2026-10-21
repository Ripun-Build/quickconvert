//! The popup window: created once at startup (tauri.conf.json), kept hidden, then moved and shown.

pub mod position;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition};

use crate::errors::AppError;
use crate::files::{self, FileSet};
use crate::selection::{self, Selection};
use position::{keep_inside, place_at_cursor, place_centered, Area, Origin, Rect};

pub const LABEL: &str = "popup";
/// Logical sizes. The list views are `WIDTH` wide and as tall as their content (up to `MAX_HEIGHT`);
/// editor views (resize, later crop) widen to `EDITOR_WIDTH` (SPECS §5).
const WIDTH: f64 = 360.0;
const EDITOR_WIDTH: f64 = 720.0;
const MIN_HEIGHT: f64 = 120.0;
const MAX_HEIGHT: f64 = 480.0;
const EDITOR_MAX_HEIGHT: f64 = 560.0;

#[derive(Clone, Copy)]
pub enum Anchor {
    Cursor,
    Center,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    generation: u64,
    origin: Origin,
    hotkey: String,
    /// True while the file selection is being read; a `popup://files` event follows.
    reading: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum FilesState {
    Files,
    Empty,
    Permission,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FilesPayload {
    generation: u64,
    state: FilesState,
    set: FileSet,
}

/// Where the popup was last placed, so it can be re-placed when its height changes.
#[derive(Clone, Copy)]
struct Layout {
    anchor: Anchor,
    cursor: (f64, f64),
    work_area: Rect,
    scale: f64,
    width: f64,
    height: f64,
}

/// Shared popup state, managed by Tauri.
pub struct PopupState {
    /// Whether native blur was applied; the UI uses a solid background otherwise.
    pub blur: bool,
    /// When the last show was requested, for the dev-only latency log.
    pub show_requested: Mutex<Option<Instant>>,
    /// Incremented on every show; stale selection results are dropped.
    generation: AtomicU64,
    /// The user dragged the popup since it was shown: size changes keep it where they left it.
    dragged: AtomicBool,
    /// Hidden only because Settings opened; it comes back, on the same screen, when Settings closes.
    set_aside: AtomicBool,
    layout: Mutex<Option<Layout>>,
}

impl PopupState {
    pub fn new(blur: bool) -> Self {
        Self {
            blur,
            show_requested: Mutex::new(None),
            generation: AtomicU64::new(0),
            dragged: AtomicBool::new(false),
            set_aside: AtomicBool::new(false),
            layout: Mutex::new(None),
        }
    }
}

/// Show the popup. With `read_selection`, the Finder/Explorer selection is read in the background
/// and delivered as a `popup://files` event, so a slow read never delays the popup.
pub fn show(app: &AppHandle, anchor: Anchor, read_selection: bool) -> Result<(), AppError> {
    let requested = Instant::now();
    let state = app.state::<PopupState>();
    let (cursor, monitor) = crate::cursor_and_monitor(app)?;
    let area = monitor.work_area();
    let height = state
        .layout
        .lock()
        .ok()
        .and_then(|l| l.map(|l| l.height))
        .unwrap_or(MIN_HEIGHT);
    let layout = Layout {
        anchor,
        cursor: (cursor.x, cursor.y),
        work_area: Rect {
            x: area.position.x,
            y: area.position.y,
            width: area.size.width,
            height: area.size.height,
        },
        scale: monitor.scale_factor(),
        width: WIDTH,
        height,
    };

    let generation = state.generation.fetch_add(1, Ordering::Relaxed) + 1;
    state.dragged.store(false, Ordering::Relaxed);
    state.set_aside.store(false, Ordering::Relaxed);
    if let Ok(mut slot) = state.show_requested.lock() {
        *slot = Some(requested);
    }

    let origin = apply_layout(app, layout)?;
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::internal("popup window missing"))?;
    window.show()?;
    window.set_focus()?;
    window.emit_to(
        LABEL,
        "popup://show",
        ShowPayload {
            generation,
            origin,
            hotkey: app.state::<crate::shortcut::HotkeyState>().label(),
            reading: read_selection,
        },
    )?;

    if read_selection {
        let app = app.clone();
        std::thread::spawn(move || {
            let (state, set) = match selection::get_selected_files() {
                Selection::Files(paths) => (FilesState::Files, files::describe(&paths)),
                Selection::Empty => (FilesState::Empty, FileSet::default()),
                Selection::PermissionDenied => (FilesState::Permission, FileSet::default()),
            };
            // Drop the result if the popup was shown again meanwhile.
            if app.state::<PopupState>().generation.load(Ordering::Relaxed) == generation {
                let payload = FilesPayload {
                    generation,
                    state,
                    set,
                };
                if let Err(err) = app.emit_to(LABEL, "popup://files", payload) {
                    eprintln!("[selection] {err:?}");
                }
            }
        });
    }
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<(), AppError> {
    app.state::<PopupState>()
        .set_aside
        .store(false, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(LABEL) {
        window.hide()?;
    }
    Ok(())
}

/// Step aside for the Settings window (the popup floats above other windows). The page isn't
/// touched, so `bring_back` shows it exactly as it was: same screen, same files, same place.
pub fn set_aside(app: &AppHandle) -> Result<(), AppError> {
    let Some(window) = app.get_webview_window(LABEL) else {
        return Ok(());
    };
    if window.is_visible()? {
        window.hide()?;
        app.state::<PopupState>()
            .set_aside
            .store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Settings closed: show the popup again if it was set aside for it.
pub fn bring_back(app: &AppHandle) -> Result<(), AppError> {
    if !app
        .state::<PopupState>()
        .set_aside
        .swap(false, Ordering::Relaxed)
    {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

/// Move the popup with the mouse (pressed on its header). Native drag: it follows until release.
pub fn start_drag(app: &AppHandle) -> Result<(), AppError> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::internal("popup window missing"))?;
    app.state::<PopupState>()
        .dragged
        .store(true, Ordering::Relaxed);
    window.start_dragging()?;
    Ok(())
}

/// Fit the window to the content (logical px), keeping the side of the cursor it opened on, or the
/// place the user dragged it to.
pub fn resize(app: &AppHandle, width: f64, height: f64) -> Result<(), AppError> {
    let width = width.clamp(WIDTH, EDITOR_WIDTH);
    let max_height = if width > WIDTH {
        EDITOR_MAX_HEIGHT
    } else {
        MAX_HEIGHT
    };
    let state = app.state::<PopupState>();
    let layout = state
        .layout
        .lock()
        .map_err(|_| AppError::internal("popup layout poisoned"))?
        .map(|l| Layout {
            width,
            height: height.clamp(MIN_HEIGHT, max_height),
            ..l
        });
    if let Some(layout) = layout {
        if state.dragged.load(Ordering::Relaxed) {
            resize_in_place(app, layout)?;
        } else {
            apply_layout(app, layout)?;
        }
    }
    Ok(())
}

/// Resize without snapping back to the cursor: the top-left stays where the user dragged it, nudged
/// only as far as needed to keep the popup on screen.
fn resize_in_place(app: &AppHandle, layout: Layout) -> Result<(), AppError> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::internal("popup window missing"))?;
    let scale = window.scale_factor()?;
    let at = window.outer_position()?;
    let area = match window.current_monitor()? {
        Some(monitor) => {
            let s = monitor.scale_factor();
            let wa = monitor.work_area();
            Area {
                x: f64::from(wa.position.x) / s,
                y: f64::from(wa.position.y) / s,
                width: f64::from(wa.size.width) / s,
                height: f64::from(wa.size.height) / s,
            }
        }
        None => {
            let (wa, s) = (layout.work_area, layout.scale);
            Area {
                x: f64::from(wa.x) / s,
                y: f64::from(wa.y) / s,
                width: f64::from(wa.width) / s,
                height: f64::from(wa.height) / s,
            }
        }
    };
    let (x, y) = keep_inside(
        (f64::from(at.x) / scale, f64::from(at.y) / scale),
        (layout.width, layout.height),
        area,
    );
    window.set_size(LogicalSize::new(layout.width, layout.height))?;
    window.set_position(LogicalPosition::new(x, y))?;
    if let Ok(mut slot) = app.state::<PopupState>().layout.lock() {
        *slot = Some(layout);
    }
    Ok(())
}

/// Size and position the window for `layout`, and remember it.
fn apply_layout(app: &AppHandle, layout: Layout) -> Result<Origin, AppError> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::internal("popup window missing"))?;
    let scale = layout.scale;
    let px = |v: f64| (v * scale).round() as u32;
    let size = (px(layout.width), px(layout.height));
    let placement = match layout.anchor {
        Anchor::Cursor => {
            place_at_cursor(layout.cursor, size, px(MAX_HEIGHT), layout.work_area, scale)
        }
        Anchor::Center => place_centered(size, layout.work_area),
    };

    window.set_size(LogicalSize::new(layout.width, layout.height))?;
    window.set_position(crate::window_position(
        PhysicalPosition::new(placement.x, placement.y),
        scale,
    ))?;
    if let Ok(mut slot) = app.state::<PopupState>().layout.lock() {
        *slot = Some(layout);
    }
    Ok(placement.origin)
}
