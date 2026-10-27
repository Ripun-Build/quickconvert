//! Word, PowerPoint and Excel files → PDF (SPECS §6 `office_to_pdf`).
//!
//! Each file type can be made one of several ways (an `Engine`): built in ("Quick": macOS's own
//! text engine, Word and Excel only, close to the original but not identical), or as an exact copy
//! by an installed app (Pages / Keynote / Numbers, LibreOffice). QuickConvert can open the App
//! Store for Apple's apps and download LibreOffice itself (`install`), only when asked.
//! Like `selection/`, `heic/` and `upright/`, the OS-specific parts live in this module.

pub mod fonts;
pub mod install;
#[cfg(target_os = "macos")]
mod macos;
pub mod sheet;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// What kind of Office file, by extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocType {
    Word,
    Slides,
    Sheet,
}

impl DocType {
    pub fn of(path: &Path) -> Option<DocType> {
        match crate::output::extension(path).as_str() {
            "docx" | "doc" | "rtf" | "odt" | "pages" => Some(DocType::Word),
            "pptx" | "ppt" | "key" | "odp" => Some(DocType::Slides),
            "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" | "csv" | "numbers" => Some(DocType::Sheet),
            _ => None,
        }
    }

    /// The name people know these files by.
    pub fn label(self) -> &'static str {
        match self {
            DocType::Word => "Word",
            DocType::Slides => "PowerPoint",
            DocType::Sheet => "Excel",
        }
    }
}

/// A way to make the PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Built in: macOS's text engine (Word) or QuickConvert's own table (Excel).
    Quick,
    Pages,
    Keynote,
    Numbers,
    Libreoffice,
}

/// Apps QuickConvert can use, in the order it prefers them when nothing was picked: Apple's own
/// app for the type first (made for Macs, no extra install for most people), then LibreOffice.
pub const APPS: [Engine; 4] = [
    Engine::Pages,
    Engine::Keynote,
    Engine::Numbers,
    Engine::Libreoffice,
];

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Quick => "Quick (built in)",
            Engine::Pages => "Pages",
            Engine::Keynote => "Keynote",
            Engine::Numbers => "Numbers",
            Engine::Libreoffice => "LibreOffice",
        }
    }

    /// The app's bundle file name in Applications.
    pub fn bundle(self) -> Option<&'static str> {
        match self {
            Engine::Quick => None,
            Engine::Pages => Some("Pages.app"),
            Engine::Keynote => Some("Keynote.app"),
            Engine::Numbers => Some("Numbers.app"),
            Engine::Libreoffice => Some("LibreOffice.app"),
        }
    }

    /// Whether this engine can turn `path` into a PDF.
    pub fn handles(self, path: &Path) -> bool {
        let ext = crate::output::extension(path);
        let ext = ext.as_str();
        match self {
            Engine::Quick => matches!(
                ext,
                "docx" | "doc" | "rtf" | "odt" | "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" | "csv"
            ),
            Engine::Pages => matches!(ext, "docx" | "doc" | "rtf" | "pages"),
            Engine::Keynote => matches!(ext, "pptx" | "ppt" | "key"),
            Engine::Numbers => matches!(ext, "xlsx" | "xls" | "csv" | "numbers"),
            Engine::Libreoffice => DocType::of(path).is_some(),
        }
    }
}

/// One way to make the PDF, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub engine: Engine,
    pub installed: bool,
    /// e.g. "14.3" (apps only).
    pub version: Option<String>,
    pub path: Option<String>,
}

/// Every engine and whether it's ready on this Mac. Quick is always ready.
pub fn engines() -> Vec<EngineInfo> {
    let mut out = vec![EngineInfo {
        engine: Engine::Quick,
        installed: true,
        version: None,
        path: None,
    }];
    out.extend(APPS.iter().map(|&engine| find_app(engine, &app_dirs())));
    out
}

/// Where apps are looked for: Applications, then the user's own Applications folder.
pub fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

/// `engine`'s app in the first of `dirs` that has it.
pub fn find_app(engine: Engine, dirs: &[PathBuf]) -> EngineInfo {
    let found = engine
        .bundle()
        .and_then(|bundle| dirs.iter().map(|d| d.join(bundle)).find(|p| p.is_dir()));
    EngineInfo {
        engine,
        installed: found.is_some(),
        version: found.as_deref().and_then(app_version),
        path: found.map(|p| p.to_string_lossy().into_owned()),
    }
}

/// CFBundleShortVersionString from the app's Info.plist.
fn app_version(app: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::bundle_version(app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        None
    }
}

/// The engine to use for `path`: the one picked for its type if it can be used, otherwise the
/// best installed app that handles it, otherwise Quick, otherwise a plain-language error naming
/// the apps to install.
pub fn choose(
    path: &Path,
    picked: Option<Engine>,
    available: &[EngineInfo],
) -> Result<Engine, AppError> {
    let ready =
        |e: Engine| e.handles(path) && available.iter().any(|i| i.engine == e && i.installed);
    if let Some(engine) = picked.filter(|&e| ready(e)) {
        return Ok(engine);
    }
    if let Some(&engine) = APPS.iter().find(|&&e| ready(e)) {
        return Ok(engine);
    }
    if ready(Engine::Quick) {
        return Ok(Engine::Quick);
    }
    let apps: Vec<&str> = APPS
        .iter()
        .filter(|e| e.handles(path))
        .map(|e| e.name())
        .collect();
    let kind = DocType::of(path).map_or("These", DocType::label);
    Err(AppError::invalid(format!(
        "{kind} files like this need {}. Install one from the Convert to PDF screen.",
        apps.join(" or ")
    )))
}

/// Built-in conversion settings (the apps keep each document's own page setup).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickOptions {
    /// A4 (true) or Letter (false).
    pub a4: bool,
    /// Spreadsheets: landscape pages.
    pub landscape: bool,
    /// Spreadsheets: only the first sheet.
    pub first_sheet_only: bool,
}

impl Default for QuickOptions {
    fn default() -> Self {
        Self {
            a4: true,
            landscape: false,
            first_sheet_only: false,
        }
    }
}

/// A PDF, and the fonts swapped to make it.
#[derive(Debug)]
pub struct Made {
    pub pdf: Vec<u8>,
    pub fonts: Vec<fonts::FontSwap>,
}

/// Make the PDF for `input` with `engine`. Fonts the file uses that `engine` can't draw are first
/// swapped for close ones in a temporary copy (same file name, so titles and errors read the same).
pub fn to_pdf(input: &Path, engine: Engine, quick: QuickOptions) -> Result<Made, AppError> {
    if !engine.handles(input) {
        return Err(AppError::internal("engine can't handle this file"));
    }
    let swaps = font_swaps(input, engine);
    if !swaps.is_empty() {
        if let Ok(dir) = crate::temp::TempDir::new("fonts") {
            let copy = dir.path().join(input.file_name().unwrap_or_default());
            if fonts::rewrite(input, &swaps, &copy).is_ok() {
                let pdf = convert(&copy, engine, quick)?;
                return Ok(Made { pdf, fonts: swaps });
            }
        }
    }
    let pdf = convert(input, engine, quick)?;
    Ok(Made {
        pdf,
        fonts: Vec::new(),
    })
}

/// Fonts to swap before converting. Quick prints spreadsheets in its own font, so none there.
fn font_swaps(input: &Path, engine: Engine) -> Vec<fonts::FontSwap> {
    let quick_sheet = engine == Engine::Quick && DocType::of(input) == Some(DocType::Sheet);
    if quick_sheet || !fonts::supports(input) {
        return Vec::new();
    }
    let uses = fonts::used(input).unwrap_or_default();
    fonts::plan(&uses, &fonts::available_for(engine))
}

fn convert(input: &Path, engine: Engine, quick: QuickOptions) -> Result<Vec<u8>, AppError> {
    #[cfg(target_os = "macos")]
    {
        macos::to_pdf(input, engine, quick)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (engine, quick);
        Err(AppError::Unsupported {
            extension: crate::output::extension(input),
        })
    }
}

/// First-page thumbnail (PNG bytes) from the system's quick preview, which reads Office files
/// with nothing else installed.
pub fn thumbnail(path: &Path, size: u32) -> Result<Vec<u8>, AppError> {
    #[cfg(target_os = "macos")]
    {
        macos::thumbnail(path, size)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = size;
        Err(AppError::Unsupported {
            extension: crate::output::extension(path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(engine: Engine, installed: bool) -> EngineInfo {
        EngineInfo {
            engine,
            installed,
            version: None,
            path: None,
        }
    }

    fn with(installed: &[Engine]) -> Vec<EngineInfo> {
        let mut all = vec![info(Engine::Quick, true)];
        all.extend(APPS.iter().map(|&e| info(e, installed.contains(&e))));
        all
    }

    #[test]
    fn file_types() {
        assert_eq!(DocType::of(Path::new("a.DOCX")), Some(DocType::Word));
        assert_eq!(DocType::of(Path::new("a.key")), Some(DocType::Slides));
        assert_eq!(DocType::of(Path::new("a.csv")), Some(DocType::Sheet));
        assert_eq!(DocType::of(Path::new("a.pdf")), None);
    }

    #[test]
    fn nothing_installed_word_and_excel_go_quick_powerpoint_needs_an_app() {
        let none = with(&[]);
        assert_eq!(
            choose(Path::new("r.docx"), None, &none).unwrap(),
            Engine::Quick
        );
        assert_eq!(
            choose(Path::new("r.xlsx"), None, &none).unwrap(),
            Engine::Quick
        );
        let err = choose(Path::new("deck.pptx"), None, &none).unwrap_err();
        assert_eq!(
            err.to_string(),
            "PowerPoint files like this need Keynote or LibreOffice. Install one from the Convert to PDF screen."
        );
        let numbers = choose(Path::new("budget.numbers"), None, &none).unwrap_err();
        assert!(numbers.to_string().contains("Numbers or LibreOffice"));
    }

    #[test]
    fn installed_apps_are_preferred_and_a_pick_is_respected() {
        let both = with(&[Engine::Keynote, Engine::Libreoffice]);
        assert_eq!(
            choose(Path::new("deck.pptx"), None, &both).unwrap(),
            Engine::Keynote
        );
        assert_eq!(
            choose(Path::new("deck.pptx"), Some(Engine::Libreoffice), &both).unwrap(),
            Engine::Libreoffice
        );
        // Picked Quick for Word: used even though LibreOffice is there.
        assert_eq!(
            choose(Path::new("r.docx"), Some(Engine::Quick), &both).unwrap(),
            Engine::Quick
        );
        // Picked an app that isn't installed: falls back to the best one that is.
        assert_eq!(
            choose(Path::new("r.docx"), Some(Engine::Pages), &both).unwrap(),
            Engine::Libreoffice
        );
    }

    #[test]
    fn apps_are_found_in_either_applications_folder() {
        let root = std::env::temp_dir().join(format!("qc-apps-{}", std::process::id()));
        let (system, user) = (root.join("Applications"), root.join("home/Applications"));
        std::fs::create_dir_all(system.join("Keynote.app/Contents")).unwrap();
        std::fs::create_dir_all(user.join("LibreOffice.app/Contents")).unwrap();
        let dirs = [system, user];
        assert!(find_app(Engine::Keynote, &dirs).installed);
        let lo = find_app(Engine::Libreoffice, &dirs);
        assert!(lo.installed && lo.path.unwrap().contains("home/Applications"));
        assert!(!find_app(Engine::Pages, &dirs).installed);
        std::fs::remove_dir_all(root).unwrap();
    }

    /// A Word file whose text is set in a font no Mac has.
    #[cfg(target_os = "macos")]
    fn docx_in_missing_font(path: &Path) {
        fonts::tests::zip_file(
            path,
            &[
                (
                    "[Content_Types].xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
                ),
                (
                    "_rels/.rels",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
                ),
                (
                    "word/document.xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:rFonts w:ascii="Avenir Next LT Pro" w:hAnsi="Avenir Next LT Pro"/></w:rPr><w:t>Hello fonts</w:t></w:r></w:p></w:body></w:document>"#,
                ),
            ],
        );
    }

    #[cfg(target_os = "macos")]
    fn uses_font(pdf: &[u8], name: &str) -> bool {
        pdf.windows(name.len()).any(|w| w == name.as_bytes())
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn missing_fonts_are_swapped_in_a_copy_and_reported() {
        let dir = crate::temp::TempDir::new("fonts-test").unwrap();
        let docx = dir.path().join("Missing font.docx");
        docx_in_missing_font(&docx);
        let before = std::fs::read(&docx).unwrap();
        let swap = fonts::FontSwap {
            from: "Avenir Next LT Pro".into(),
            to: "Avenir Next".into(),
        };
        let made = to_pdf(&docx, Engine::Quick, QuickOptions::default()).unwrap();
        assert_eq!(made.fonts, vec![swap.clone()]);
        assert!(
            uses_font(&made.pdf, "AvenirNext"),
            "Quick draws the lookalike"
        );
        assert_eq!(std::fs::read(&docx).unwrap(), before, "original untouched");

        let lo = find_app(Engine::Libreoffice, &app_dirs());
        if lo.installed {
            let made = to_pdf(&docx, Engine::Libreoffice, QuickOptions::default()).unwrap();
            assert_eq!(made.fonts, vec![swap]);
            assert!(
                uses_font(&made.pdf, "AvenirNext"),
                "LibreOffice draws the lookalike"
            );
        }
    }
}
