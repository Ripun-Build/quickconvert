//! The popup window: created once at startup (tauri.conf.json), kept hidden, then moved and shown.

pub mod position;

use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::errors::AppError;
use position::{place_at_cursor, place_centered, Origin, Rect};

pub const LABEL: &str = "popup";
/// Logical size of the popup shell. Becomes content-driven in M2.
const SIZE: (f64, f64) = (360.0, 176.0);

pub enum Anchor {
    Cursor,
    Center,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    origin: Origin,
    hotkey: String,
}

/// Shared popup state, managed by Tauri.
pub struct PopupState {
    /// Label of the registered hotkey (e.g. "⌃⇧Space"), shown in the UI.
    pub hotkey_label: String,
    /// Whether native blur was applied; the UI uses a solid background otherwise.
    pub blur: bool,
    /// When the last show was requested, for the dev-only latency log.
    pub show_requested: Mutex<Option<Instant>>,
}

pub fn show(app: &AppHandle, anchor: Anchor) -> Result<(), AppError> {
    let requested = Instant::now();
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::internal("popup window missing"))?;

    let (cursor, monitor) = crate::cursor_and_monitor(app)?;
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let work_area = Rect {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    };
    let size = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let placement = match anchor {
        Anchor::Cursor => place_at_cursor((cursor.x, cursor.y), size, work_area, scale),
        Anchor::Center => place_centered(size, work_area),
    };

    let state = app.state::<PopupState>();
    if let Ok(mut slot) = state.show_requested.lock() {
        *slot = Some(requested);
    }

    window.set_position(crate::window_position(
        PhysicalPosition::new(placement.x, placement.y),
        scale,
    ))?;
    window.show()?;
    window.set_focus()?;
    window.emit_to(
        LABEL,
        "popup://show",
        ShowPayload {
            origin: placement.origin,
            hotkey: state.hotkey_label.clone(),
        },
    )?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.hide()?;
    }
    Ok(())
}
