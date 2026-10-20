pub mod commands;
pub mod convert;
pub mod errors;
pub mod jobs;
pub mod output;
pub mod popup;
pub mod selection;
pub mod settings;

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Monitor, PhysicalPosition, Position, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use errors::AppError;
use popup::{Anchor, PopupState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        log_err(popup::show(app, Anchor::Cursor));
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::hide_popup,
            commands::popup_shown,
            commands::blur_enabled,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let blur = apply_blur(app);
            let (hotkey, hotkey_label) = default_hotkey();
            let hotkey_ok = app.global_shortcut().register(hotkey).is_ok();
            if !hotkey_ok {
                eprintln!("[hotkey] {hotkey_label} is already in use; couldn't register it");
            }

            app.manage(PopupState {
                hotkey_label: hotkey_label.to_string(),
                blur,
                show_requested: Mutex::new(None),
            });
            build_tray(app, (!hotkey_ok).then_some(hotkey_label))?;

            if let Some(window) = app.get_webview_window(popup::LABEL) {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        log_err(popup::hide(&handle));
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn build_tray(app: &App, hotkey_conflict: Option<&str>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open QuickConvert", true, None::<&str>)?;
    // Settings window arrives in M6.
    let settings = MenuItem::with_id(app, "settings", "Settings…", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &settings, &separator, &quit])?;
    if let Some(label) = hotkey_conflict {
        let note = format!("{label} is in use by another app");
        let conflict = MenuItem::with_id(app, "hotkey-conflict", note, false, None::<&str>)?;
        menu.insert(&conflict, 0)?;
        menu.insert(&PredefinedMenuItem::separator(app)?, 1)?;
    }

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("QuickConvert")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => log_err(popup::show(app, Anchor::Center)),
            "quit" => app.exit(0),
            _ => {}
        });
    // Placeholder: the app icon. Template tray icons arrive in M7.
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn log_err(result: Result<(), AppError>) {
    if let Err(err) = result {
        eprintln!("[popup] {err:?}");
    }
}

// ---- Platform specifics: OS-specific code stays in small cfg blocks here and in selection/ ----

/// Default hotkey and its display label (SPECS §4). Never Alt+Shift on Windows (switches layout).
fn default_hotkey() -> (Shortcut, &'static str) {
    #[cfg(target_os = "macos")]
    return (
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space),
        "⌃⇧Space",
    );
    #[cfg(not(target_os = "macos"))]
    return (
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space),
        "Ctrl+Alt+Space",
    );
}

/// Native blur behind the popup. Returns false if unavailable (UI falls back to `--bg-solid`).
fn apply_blur(app: &App) -> bool {
    let Some(window) = app.get_webview_window(popup::LABEL) else {
        return false;
    };
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        apply_vibrancy(
            &window,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            Some(18.0),
        )
        .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::{apply_acrylic, apply_mica};
        apply_mica(&window, None).is_ok() || apply_acrylic(&window, Some((0, 0, 0, 0))).is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window;
        false
    }
}

/// Cursor position and the monitor under it, in that monitor's physical pixel space.
///
/// macOS: tao scales the cursor by the *primary* monitor's factor but each monitor's rect by its
/// own, so convert through points to find the right monitor on mixed-DPI setups.
#[cfg(target_os = "macos")]
fn cursor_and_monitor(app: &AppHandle) -> Result<(PhysicalPosition<f64>, Monitor), AppError> {
    let primary = app
        .primary_monitor()?
        .ok_or_else(|| AppError::internal("no primary monitor"))?;
    let cursor = app.cursor_position()?;
    let (px, py) = (
        cursor.x / primary.scale_factor(),
        cursor.y / primary.scale_factor(),
    );
    for monitor in app.available_monitors()? {
        let s = monitor.scale_factor();
        let (x, y) = (
            monitor.position().x as f64 / s,
            monitor.position().y as f64 / s,
        );
        let (w, h) = (
            monitor.size().width as f64 / s,
            monitor.size().height as f64 / s,
        );
        if px >= x && px < x + w && py >= y && py < y + h {
            return Ok((PhysicalPosition::new(px * s, py * s), monitor));
        }
    }
    let s = primary.scale_factor();
    Ok((PhysicalPosition::new(px * s, py * s), primary))
}

#[cfg(not(target_os = "macos"))]
fn cursor_and_monitor(app: &AppHandle) -> Result<(PhysicalPosition<f64>, Monitor), AppError> {
    let cursor = app.cursor_position()?;
    let monitor = match app.monitor_from_point(cursor.x, cursor.y)? {
        Some(m) => m,
        None => app
            .primary_monitor()?
            .ok_or_else(|| AppError::internal("no primary monitor"))?,
    };
    Ok((cursor, monitor))
}

/// Convert a position in the target monitor's physical space to what `set_position` expects.
/// macOS positions windows in points, so go through the target monitor's scale factor.
#[cfg(target_os = "macos")]
fn window_position(p: PhysicalPosition<i32>, scale: f64) -> Position {
    Position::Logical(tauri::LogicalPosition::new(
        p.x as f64 / scale,
        p.y as f64 / scale,
    ))
}

#[cfg(not(target_os = "macos"))]
fn window_position(p: PhysicalPosition<i32>, _scale: f64) -> Position {
    Position::Physical(p)
}
