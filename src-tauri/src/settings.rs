//! User settings (SPECS §10), persisted as `settings.json` in the app config dir.
//! Launch at login is owned by the autostart plugin, not stored here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputLocation {
    NextToOriginal,
    Folder { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Global shortcut, e.g. "Control+Shift+Space". None = the platform default.
    pub shortcut: Option<String>,
    pub output: OutputLocation,
    /// Default quality for Convert to JPG/WebP.
    pub jpeg_quality: u8,
    /// Default long edge in the Resize editor.
    pub resize_long_edge: u32,
    /// Default size in the PDF → images editor (72 / 150 / 300).
    pub pdf_dpi: u16,
    pub appearance: Appearance,
    /// Hide the popup 2.5 s after a successful conversion.
    pub auto_hide: bool,
    /// Convert to PDF: how each Office file type is made (None = the best way available).
    pub office: OfficeChoices,
}

/// Remembered per Office file type; an app that isn't installed any more falls back to the best
/// way available.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OfficeChoices {
    pub word: Option<crate::office::Engine>,
    pub slides: Option<crate::office::Engine>,
    pub sheet: Option<crate::office::Engine>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: None,
            output: OutputLocation::NextToOriginal,
            jpeg_quality: 85,
            resize_long_edge: 1600,
            pdf_dpi: 150,
            appearance: Appearance::System,
            auto_hide: true,
            office: OfficeChoices::default(),
        }
    }
}

impl Settings {
    /// Keep values in their valid ranges (hand-edited files can say anything).
    pub fn sanitized(mut self) -> Self {
        self.jpeg_quality = self.jpeg_quality.clamp(1, 100);
        self.resize_long_edge = self.resize_long_edge.clamp(16, 20_000);
        if ![72, 150, 300].contains(&self.pdf_dpi) {
            self.pdf_dpi = 150;
        }
        if let OutputLocation::Folder { path } = &self.output {
            if path.as_os_str().is_empty() {
                self.output = OutputLocation::NextToOriginal;
            }
        }
        self.shortcut = self.shortcut.filter(|s| !s.trim().is_empty());
        self
    }

    /// Custom output folder, if one is set.
    pub fn output_folder(&self) -> Option<&Path> {
        match &self.output {
            OutputLocation::Folder { path } => Some(path),
            OutputLocation::NextToOriginal => None,
        }
    }
}

/// Load settings; a missing or unreadable file gives the defaults.
pub fn load(path: &Path) -> Settings {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => settings.sanitized(),
        Err(err) => {
            eprintln!("[settings] ignoring {path:?}: {err}");
            Settings::default()
        }
    }
}

/// Save atomically (temp file, then rename), creating the config dir if needed.
pub fn save(path: &Path, settings: &Settings) -> Result<(), AppError> {
    write_json(path, settings)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text =
        serde_json::to_string_pretty(value).map_err(|e| AppError::internal(e.to_string()))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

/// Live settings, shared by the popup, jobs and the Settings window.
pub struct AppSettings {
    path: PathBuf,
    current: std::sync::Mutex<Settings>,
}

impl AppSettings {
    pub fn load(path: PathBuf) -> Self {
        let current = std::sync::Mutex::new(load(&path));
        Self { path, current }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Replace, save and return the stored (sanitized) settings.
    pub fn set(&self, settings: Settings) -> Result<Settings, AppError> {
        let settings = settings.sanitized();
        save(&self.path, &settings)?;
        if let Ok(mut current) = self.current.lock() {
            *current = settings.clone();
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-settings-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn settings_saved_by_v0_1_0_load_unchanged() {
        // Exactly what v0.1.0 wrote, with every value changed from its default. Updates must keep
        // reading it: add new fields with defaults, never rename or remove old ones.
        let dir = temp_dir("v010");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"shortcut":"Control+Alt+K","output":{"kind":"folder","path":"/Users/someone/Converted"},"jpegQuality":70,"resizeLongEdge":1200,"pdfDpi":300,"appearance":"dark","autoHide":false}"#,
        )
        .unwrap();
        assert_eq!(
            load(&path),
            Settings {
                shortcut: Some("Control+Alt+K".into()),
                output: OutputLocation::Folder {
                    path: "/Users/someone/Converted".into()
                },
                jpeg_quality: 70,
                resize_long_edge: 1200,
                pdf_dpi: 300,
                appearance: Appearance::Dark,
                auto_hide: false,
                office: OfficeChoices::default(),
            }
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_or_broken_file_gives_defaults() {
        let dir = temp_dir("broken");
        assert_eq!(load(&dir.join("settings.json")), Settings::default());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), "{ nope").unwrap();
        assert_eq!(load(&dir.join("settings.json")), Settings::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn round_trips_and_fills_missing_fields() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("nested/settings.json");
        let settings = Settings {
            shortcut: Some("Control+Alt+KeyK".into()),
            output: OutputLocation::Folder {
                path: PathBuf::from("/tmp/out"),
            },
            appearance: Appearance::Dark,
            auto_hide: false,
            ..Settings::default()
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings);

        std::fs::write(&path, r#"{"appearance":"light"}"#).unwrap();
        let partial = load(&path);
        assert_eq!(partial.appearance, Appearance::Light);
        assert_eq!(partial.jpeg_quality, 85);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn out_of_range_values_are_fixed() {
        let s = Settings {
            jpeg_quality: 0,
            resize_long_edge: 5,
            pdf_dpi: 999,
            shortcut: Some("  ".into()),
            output: OutputLocation::Folder {
                path: PathBuf::new(),
            },
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(
            (s.jpeg_quality, s.resize_long_edge, s.pdf_dpi),
            (1, 16, 150)
        );
        assert_eq!(s.shortcut, None);
        assert_eq!(s.output, OutputLocation::NextToOriginal);
    }

    #[test]
    fn json_shape_matches_the_ui() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["output"]["kind"], "next_to_original");
        assert_eq!(json["jpegQuality"], 85);
        assert_eq!(json["appearance"], "system");
        assert_eq!(json["autoHide"], true);
    }
}
