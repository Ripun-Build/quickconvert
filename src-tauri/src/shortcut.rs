//! Global shortcut strings: parsing, display labels and system shortcuts that can't be used.
//! Pure functions (the platform is a parameter) so they're testable everywhere.

use std::str::FromStr;

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

use crate::errors::AppError;

/// Parse "Control+Shift+Space" and check it's usable as a global shortcut.
pub fn parse(accelerator: &str, mac: bool) -> Result<Shortcut, AppError> {
    let shortcut = Shortcut::from_str(accelerator)
        .map_err(|_| AppError::invalid("That key can't be used for a shortcut."))?;
    let has_main_modifier = shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);
    if !has_main_modifier {
        let keys = if mac {
            "⌃, ⌥ or ⌘"
        } else {
            "Ctrl, Alt or Win"
        };
        return Err(AppError::invalid(format!(
            "Include {keys} in the shortcut."
        )));
    }
    if let Some(owner) = reserved_by(&shortcut, mac) {
        return Err(AppError::invalid(format!(
            "{} is used by {owner}. Pick a different shortcut.",
            label(&shortcut, mac)
        )));
    }
    Ok(shortcut)
}

/// Canonical form for settings.json, e.g. "Control+Shift+Space".
pub fn accelerator(shortcut: &Shortcut) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (flag, name) in [
        (Modifiers::CONTROL, "Control"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Super"),
    ] {
        if shortcut.mods.contains(flag) {
            parts.push(name.into());
        }
    }
    parts.push(format!("{:?}", shortcut.key));
    parts.join("+")
}

/// "⌃⇧Space" on macOS, "Ctrl+Alt+Space" elsewhere.
pub fn label(shortcut: &Shortcut, mac: bool) -> String {
    let key = key_label(shortcut.key);
    if mac {
        let mut out = String::new();
        for (flag, symbol) in [
            (Modifiers::CONTROL, "⌃"),
            (Modifiers::ALT, "⌥"),
            (Modifiers::SHIFT, "⇧"),
            (Modifiers::SUPER, "⌘"),
        ] {
            if shortcut.mods.contains(flag) {
                out.push_str(symbol);
            }
        }
        out + &key
    } else {
        let mut parts: Vec<String> = Vec::new();
        for (flag, name) in [
            (Modifiers::CONTROL, "Ctrl"),
            (Modifiers::ALT, "Alt"),
            (Modifiers::SHIFT, "Shift"),
            (Modifiers::SUPER, "Win"),
        ] {
            if shortcut.mods.contains(flag) {
                parts.push(name.into());
            }
        }
        parts.push(key);
        parts.join("+")
    }
}

fn key_label(code: Code) -> String {
    let name = format!("{code:?}");
    let named = match name.as_str() {
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Minus" => "-",
        "Equal" => "=",
        "Backquote" => "`",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Enter" => "↩",
        "Tab" => "⇥",
        "Backspace" => "⌫",
        "Escape" => "Esc",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    name.strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(&name)
        .to_string()
}

/// System shortcuts that would never reach us (registration may even "succeed" for them).
fn reserved_by(s: &Shortcut, mac: bool) -> Option<&'static str> {
    let m = s.mods;
    let only = |mods: Modifiers| m == mods;
    let key = s.key;
    if mac {
        let cmd = Modifiers::SUPER;
        let ctrl = Modifiers::CONTROL;
        match key {
            Code::Space if only(cmd) => Some("Spotlight"),
            Code::Space if only(cmd | Modifiers::ALT) => Some("Finder search"),
            Code::Space if only(ctrl) || only(ctrl | Modifiers::ALT) => {
                Some("macOS to switch keyboard input")
            }
            Code::Tab if m.contains(cmd) => Some("the app switcher"),
            Code::Digit3 | Code::Digit4 | Code::Digit5 if only(cmd | Modifiers::SHIFT) => {
                Some("macOS screenshots")
            }
            Code::KeyQ | Code::KeyW | Code::KeyH | Code::KeyM if only(cmd) => {
                Some("macOS for every app")
            }
            _ => None,
        }
    } else {
        let only_alt_shift = only(Modifiers::ALT | Modifiers::SHIFT);
        match key {
            _ if only_alt_shift => Some("Windows to switch keyboard layout"),
            Code::Tab if m.contains(Modifiers::ALT) => Some("the app switcher"),
            Code::Delete if only(Modifiers::CONTROL | Modifiers::ALT) => Some("Windows security"),
            _ if m.contains(Modifiers::SUPER) => Some("Windows"),
            _ => None,
        }
    }
}

// ---- Runtime: the registered shortcut ----

/// The registered global shortcut and, if registration failed, why.
#[derive(Default)]
pub struct HotkeyState {
    current: std::sync::Mutex<Option<Shortcut>>,
    conflict: std::sync::Mutex<Option<String>>,
}

impl HotkeyState {
    /// Display label of the registered shortcut (or of the one we tried).
    pub fn label(&self) -> String {
        let current = self.current.lock().ok().and_then(|c| *c);
        current
            .map(|s| label(&s, crate::IS_MAC))
            .unwrap_or_default()
    }

    /// Plain message when the shortcut couldn't be registered.
    pub fn conflict(&self) -> Option<String> {
        self.conflict.lock().ok().and_then(|c| c.clone())
    }
}

fn conflict_message(shortcut: &Shortcut) -> String {
    format!(
        "{} is already used by another app. Pick a different shortcut.",
        label(shortcut, crate::IS_MAC)
    )
}

/// Register the saved shortcut (or the default) at startup. Returns false on a conflict.
pub fn register_initial<R: tauri::Runtime>(app: &tauri::AppHandle<R>, saved: Option<&str>) -> bool {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcut = saved
        .and_then(|a| parse(a, crate::IS_MAC).ok())
        .unwrap_or_else(default_shortcut);
    let state = app.state::<HotkeyState>();
    let ok = app.global_shortcut().register(shortcut).is_ok();
    if let Ok(mut current) = state.current.lock() {
        *current = Some(shortcut);
    }
    if let Ok(mut conflict) = state.conflict.lock() {
        *conflict = (!ok).then(|| conflict_message(&shortcut));
    }
    ok
}

pub fn default_shortcut() -> Shortcut {
    // `DEFAULT_ACCELERATOR` is a valid constant per platform; the fallback is unreachable.
    Shortcut::from_str(crate::DEFAULT_ACCELERATOR)
        .unwrap_or_else(|_| Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space))
}

/// Switch to `accelerator` (None = default). The old shortcut stays if the new one can't be used.
pub fn change<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    accelerator: Option<&str>,
) -> Result<Shortcut, AppError> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let new = match accelerator {
        Some(a) => parse(a, crate::IS_MAC)?,
        None => default_shortcut(),
    };
    let state = app.state::<HotkeyState>();
    let old = state.current.lock().ok().and_then(|c| *c);
    let shortcuts = app.global_shortcut();
    let old_active = old.is_some_and(|o| shortcuts.is_registered(o));
    if old == Some(new) && old_active {
        return Ok(new);
    }
    if old_active {
        if let Some(o) = old {
            let _ = shortcuts.unregister(o);
        }
    }
    if shortcuts.register(new).is_err() {
        // Put the old one back.
        if let Some(o) = old.filter(|_| old_active) {
            let _ = shortcuts.register(o);
        }
        return Err(AppError::invalid(conflict_message(&new)));
    }
    if let Ok(mut current) = state.current.lock() {
        *current = Some(new);
    }
    if let Ok(mut conflict) = state.conflict.lock() {
        *conflict = None;
    }
    Ok(new)
}

/// While the Settings window records a new shortcut, the current one is paused so pressing it
/// doesn't open the popup.
pub fn suspend<R: tauri::Runtime>(app: &tauri::AppHandle<R>, paused: bool) {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let state = app.state::<HotkeyState>();
    let Some(current) = state.current.lock().ok().and_then(|c| *c) else {
        return;
    };
    let shortcuts = app.global_shortcut();
    if paused {
        let _ = shortcuts.unregister(current);
    } else if !shortcuts.is_registered(current) && state.conflict().is_none() {
        let _ = shortcuts.register(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_round_trip() {
        let s = parse("Control+Shift+Space", true).unwrap();
        assert_eq!(label(&s, true), "⌃⇧Space");
        assert_eq!(label(&s, false), "Ctrl+Shift+Space");
        assert_eq!(accelerator(&s), "Control+Shift+Space");
        let k = parse("super+alt+KeyK", true).unwrap();
        assert_eq!(label(&k, true), "⌥⌘K");
        assert_eq!(parse(&accelerator(&k), true).unwrap(), k);
        assert_eq!(
            label(&parse("Control+Alt+Comma", true).unwrap(), true),
            "⌃⌥,"
        );
        assert_eq!(
            label(&parse("Control+Alt+Digit1", true).unwrap(), true),
            "⌃⌥1"
        );
    }

    #[test]
    fn needs_a_real_modifier() {
        assert_eq!(
            parse("KeyK", true).unwrap_err().to_string(),
            "Include ⌃, ⌥ or ⌘ in the shortcut."
        );
        assert!(parse("Shift+KeyK", false).is_err());
        assert!(parse("Control+NotAKey", true).is_err());
    }

    #[test]
    fn system_shortcuts_are_refused() {
        assert_eq!(
            parse("Super+Space", true).unwrap_err().to_string(),
            "⌘Space is used by Spotlight. Pick a different shortcut."
        );
        assert!(parse("Control+Space", true).is_err());
        assert!(parse("Super+Shift+Digit4", true).is_err());
        assert!(parse("Alt+Shift+KeyA", false).is_err());
        // Fine elsewhere / with other modifiers.
        assert!(parse("Alt+Shift+KeyA", true).is_ok());
        assert!(parse("Control+Shift+Space", true).is_ok());
        assert!(parse("Control+Alt+Space", false).is_ok());
    }
}
