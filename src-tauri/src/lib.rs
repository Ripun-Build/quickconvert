pub mod commands;
pub mod convert;
pub mod errors;
pub mod files;
pub mod heic;
pub mod instance;
pub mod jobs;
pub mod office;
pub mod output;
pub mod pool;
pub mod popup;
pub mod presets;
pub mod selection;
pub mod settings;
pub mod settings_window;
pub mod shortcut;
pub mod temp;
pub mod upright;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Monitor, PhysicalPosition, Position, Theme};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::ShortcutState;

use errors::AppError;
use files::ThumbnailCache;
use jobs::Jobs;
use popup::{Anchor, PopupState};
use settings::{AppSettings, Appearance};
use shortcut::HotkeyState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if app.state::<Jobs>().is_running() {
                        // Don't reset the popup mid-job; just bring it forward.
                        log_err(commands::focus_popup(app.clone()));
                        return;
                    }
                    // Must run before the popup takes focus from Finder.
                    let read_selection = selection::source_is_frontmost();
                    log_err(popup::show(app, Anchor::Cursor, read_selection));
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::hide_popup,
            commands::popup_shown,
            commands::blur_enabled,
            commands::resize_popup,
            commands::drag_popup,
            commands::focus_popup,
            commands::describe_files,
            commands::thumbnail,
            commands::preview,
            commands::rotate_check,
            commands::pdf_compress_estimate,
            commands::tray_thumbnail,
            commands::office_engines,
            commands::office_open_store,
            commands::office_install_libreoffice,
            commands::office_install_status,
            commands::office_cancel_install,
            commands::office_libreoffice_package,
            commands::pdf_page_upright,
            commands::pdf_page_sizes,
            commands::pdf_thumbnails,
            commands::list_presets,
            commands::estimate_output,
            commands::start_job,
            commands::cancel_job,
            commands::open_output,
            commands::reveal_output,
            commands::get_settings,
            commands::save_settings,
            commands::set_shortcut,
            commands::suspend_shortcut,
            commands::set_launch_at_login,
            commands::open_settings,
            commands::presets_for_editor,
            commands::save_presets,
            commands::restore_presets,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let blur = apply_blur(app);
            let config_dir = app.path().app_config_dir()?;
            // Can't create the lock (unusual): run anyway rather than refuse to start.
            let lock = instance::acquire(&config_dir)
                .map_err(|err| eprintln!("[instance] {err:?}"))
                .ok();
            let already_running = matches!(lock, Some(None));
            let settings = AppSettings::load(config_dir.join("settings.json"));
            apply_appearance(app.handle(), settings.get().appearance);

            app.manage(HotkeyState::default());
            app.manage(PopupState::new(blur));
            app.manage(ThumbnailCache::default());
            app.manage(Jobs::default());
            app.manage(office::install::InstallState::default());
            if already_running {
                // This copy only explains itself and quits; the running one keeps the shortcut.
                app.manage(settings);
                tell_already_running(app.handle());
                return Ok(());
            }
            if let Some(Some(lock)) = lock {
                app.manage(lock);
            }
            let hotkey_ok =
                shortcut::register_initial(app.handle(), settings.get().shortcut.as_deref());
            app.manage(settings);
            build_tray(app)?;
            if !hotkey_ok {
                // SPECS §4: show why and let the user pick another shortcut right away.
                log_err(settings_window::open(
                    app.handle(),
                    Some("general"),
                    Some("shortcut-conflict"),
                ));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Second copy: say why nothing else happens, then quit when the message is closed.
fn tell_already_running(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .message(
            "Another copy of QuickConvert is already open. Use it from its menu bar icon or \
             its keyboard shortcut, or quit it from the menu bar first and then open this one again.",
        )
        .title("QuickConvert is already running")
        .kind(MessageDialogKind::Info)
        .show(move |_| handle.exit(0));
}

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open QuickConvert", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &settings, &separator, &quit])?;
    let hotkey = app.state::<HotkeyState>();
    if hotkey.conflict().is_some() {
        let note = format!("{} is in use by another app", hotkey.label());
        let conflict = MenuItem::with_id(app, "hotkey-conflict", note, false, None::<&str>)?;
        menu.insert(&conflict, 0)?;
        menu.insert(&PredefinedMenuItem::separator(app)?, 1)?;
    }
    Ok(menu)
}

/// Rebuild the tray menu (e.g. after the shortcut conflict is resolved).
pub fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// System / Light / Dark for every window, native blur included.
pub fn apply_appearance(app: &AppHandle, appearance: Appearance) {
    app.set_theme(match appearance {
        Appearance::System => None,
        Appearance::Light => Some(Theme::Light),
        Appearance::Dark => Some(Theme::Dark),
    });
}

fn build_tray(app: &App) -> tauri::Result<()> {
    let menu = tray_menu(app.handle())?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("QuickConvert")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => log_err(popup::show(app, Anchor::Center, false)),
            "settings" => log_err(settings_window::open(app, None, None)),
            "quit" => app.exit(0),
            _ => {}
        });
    // Black-on-transparent template image: macOS tints it for light and dark menu bars.
    match tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(icon) => tray = tray.icon(icon).icon_as_template(true),
        Err(_) => {
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
        }
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

/// Shortcut labels use Mac symbols (⌃⌥⇧⌘) on macOS.
pub const IS_MAC: bool = cfg!(target_os = "macos");

/// Default shortcut (SPECS §4). Never Alt+Shift on Windows (switches keyboard layout).
#[cfg(target_os = "macos")]
pub const DEFAULT_ACCELERATOR: &str = "Control+Shift+Space";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_ACCELERATOR: &str = "Control+Alt+Space";

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
