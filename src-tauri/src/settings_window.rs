//! The Settings window (SPECS §10): a normal 480 × 560 window, created when first opened (never
//! on the hotkey path) and reused while it stays open.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::errors::AppError;
use crate::popup;

pub const LABEL: &str = "settings";

/// Open (or focus) Settings. `tab` is "general" | "conversions" | "presets"; `notice` is a reason
/// to show at the top, e.g. "shortcut-conflict".
pub fn open(app: &AppHandle, tab: Option<&str>, notice: Option<&str>) -> Result<(), AppError> {
    let _ = popup::set_aside(app);
    if let Some(window) = app.get_webview_window(LABEL) {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }
    let mut query = String::from("view=settings");
    if let Some(tab) = tab {
        query.push_str(&format!("&tab={tab}"));
    }
    if let Some(notice) = notice {
        query.push_str(&format!("&notice={notice}"));
    }
    let window = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App(format!("index.html?{query}").into()),
    )
    .title("QuickConvert Settings")
    .inner_size(480.0, 560.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(true)
    .center()
    .focused(true)
    .build()?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            if let Err(err) = popup::bring_back(&handle) {
                eprintln!("[settings] {err:?}");
            }
        }
    });
    window.set_focus()?;
    Ok(())
}
