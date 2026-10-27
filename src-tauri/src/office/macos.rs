//! macOS: printing with the system text engine, Pages / Keynote / Numbers via AppleScript,
//! LibreOffice in headless mode, and Quick Look thumbnails. Every outside tool runs with a time
//! limit, so a stuck app or a dialog can't hang a conversion.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{sheet, Engine, QuickOptions};
use crate::errors::AppError;
use crate::temp::TempFile;

const PRINT_SCRIPT: &str = include_str!("print_to_pdf.js");
const A4: (f64, f64) = (595.276, 841.89);
const LETTER: (f64, f64) = (612.0, 792.0);
/// The built-in path is quick; apps can take a while to open big files.
const QUICK_LIMIT: Duration = Duration::from_secs(120);
const APP_LIMIT: Duration = Duration::from_secs(600);

/// One document at a time per app: they're single windows driven by AppleScript or one profile.
static PAGES: Mutex<()> = Mutex::new(());
static KEYNOTE: Mutex<()> = Mutex::new(());
static NUMBERS: Mutex<()> = Mutex::new(());
static LIBREOFFICE: Mutex<()> = Mutex::new(());

pub fn bundle_version(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(&plist)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Run `cmd`, killing it after `limit`. Returns (success, stdout + stderr).
fn run_limited(mut cmd: Command, limit: Duration, what: &str) -> Result<(bool, String), AppError> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let mut text = String::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut text);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut text);
            }
            return Ok((status.success(), text));
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::invalid(format!(
                "{what} took too long and was stopped. Try again, or pick another way to convert it."
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn to_pdf(input: &Path, engine: Engine, quick: QuickOptions) -> Result<Vec<u8>, AppError> {
    match engine {
        Engine::Quick => quick_pdf(input, quick),
        Engine::Pages => iwork_pdf(input, "com.apple.iWork.Pages", "Pages", &PAGES),
        Engine::Keynote => iwork_pdf(input, "com.apple.iWork.Keynote", "Keynote", &KEYNOTE),
        Engine::Numbers => iwork_pdf(input, "com.apple.iWork.Numbers", "Numbers", &NUMBERS),
        Engine::Libreoffice => libreoffice_pdf(input),
    }
}

// ---- Quick (built in) ----

fn quick_pdf(input: &Path, quick: QuickOptions) -> Result<Vec<u8>, AppError> {
    let paper = if quick.a4 { A4 } else { LETTER };
    if super::DocType::of(input) != Some(super::DocType::Sheet) {
        // macOS's text engine reads unknown bytes as plain text, so check it's really a document.
        if !looks_like_document(input)? {
            return Err(AppError::Corrupt {
                detail: "not a Word / RTF / OpenDocument file".into(),
            });
        }
        return print_pdf(input, paper);
    }
    // Spreadsheets: each sheet printed from an HTML table on its own pages, then joined (the
    // text engine ignores page breaks inside one document).
    let size = if quick.landscape {
        (paper.1, paper.0)
    } else {
        paper
    };
    let sheets = sheet::read(input, quick.first_sheet_only)?;
    let mut parts = Vec::new();
    for one in sheets.chunks(1) {
        let html = TempFile::new("sheet", "html");
        std::fs::write(html.path(), sheet::to_html(one, sheets.len() > 1))?;
        let pdf = TempFile::new("sheet", "pdf");
        std::fs::write(pdf.path(), print_pdf(html.path(), size)?)?;
        parts.push(pdf);
    }
    if parts.len() == 1 {
        return Ok(std::fs::read(parts[0].path())?);
    }
    let paths: Vec<PathBuf> = parts.iter().map(|p| p.path().to_path_buf()).collect();
    let mut doc = crate::convert::pdf::merge_pdfs(&paths, &mut |_| Ok(()))?;
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)?;
    Ok(bytes)
}

/// The first bytes of a .docx / .odt (zip), .doc (OLE) or .rtf file.
fn looks_like_document(path: &Path) -> Result<bool, AppError> {
    let mut head = [0u8; 8];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    let head = &head[..n];
    Ok(match crate::output::extension(path).as_str() {
        "docx" | "odt" => head.starts_with(b"PK"),
        "doc" => head.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) || head.starts_with(b"{\\rtf"),
        "rtf" => head.starts_with(b"{\\rtf"),
        _ => true,
    })
}

/// Print a document (or HTML) to a PDF on paper of `size` points.
fn print_pdf(source: &Path, size: (f64, f64)) -> Result<Vec<u8>, AppError> {
    let pdf = TempFile::new("office", "pdf");
    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.args(["-l", "JavaScript", "-e", PRINT_SCRIPT])
        .arg(source)
        .arg(pdf.path())
        .arg(size.0.to_string())
        .arg(size.1.to_string());
    let (ok, log) = run_limited(cmd, QUICK_LIMIT, "Making the PDF")?;
    if !ok || !pdf.path().exists() {
        return Err(if log.contains("unreadable") {
            AppError::Corrupt { detail: log }
        } else {
            AppError::internal(log)
        });
    }
    Ok(std::fs::read(pdf.path())?)
}

// ---- Pages / Keynote / Numbers ----

/// Where a sandboxed iWork app may write: its own container's tmp folder (QuickConvert, which isn't
/// sandboxed, reads the PDF from there), or the system temp folder if it has no container.
fn iwork_out_dir(bundle_id: &str) -> PathBuf {
    let container = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(format!("Library/Containers/{bundle_id}/Data/tmp")))
        .filter(|p| p.is_dir());
    container.unwrap_or_else(std::env::temp_dir)
}

fn iwork_pdf(
    input: &Path,
    bundle_id: &str,
    name: &str,
    lock: &Mutex<()>,
) -> Result<Vec<u8>, AppError> {
    let _one_at_a_time = lock.lock().unwrap_or_else(|p| p.into_inner());
    let out = iwork_out_dir(bundle_id).join(format!(
        "quickconvert-{}-{}.pdf",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let cleanup = OutFile(out.clone());
    // The app's id is written into the script so its terms (export … as PDF) are known.
    let script = format!(
        r#"on run argv
  set wasRunning to application id "{bundle_id}" is running
  set inFile to POSIX file (item 1 of argv)
  set outFile to POSIX file (item 2 of argv)
  with timeout of 600 seconds
    tell application id "{bundle_id}"
      set theDoc to open inFile
      export theDoc to outFile as PDF
      close theDoc saving no
      if not wasRunning then quit
    end tell
  end timeout
end run"#
    );
    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.arg("-e").arg(&script).arg(input).arg(&out);
    let (ok, log) = run_limited(cmd, APP_LIMIT, name)?;
    if !ok || !out.exists() {
        // -1743: the user (or macOS) hasn't allowed QuickConvert to control the app.
        if log.contains("-1743") || log.contains("Not authorized") {
            return Err(AppError::invalid(format!(
                "QuickConvert isn't allowed to use {name}. Allow it in System Settings → Privacy & Security → Automation."
            )));
        }
        return Err(AppError::invalid(format!(
            "{name} couldn't make a PDF of this file.{}",
            if log.trim().is_empty() {
                String::new()
            } else {
                format!(" ({})", log.trim())
            }
        )));
    }
    let bytes = std::fs::read(&out)?;
    drop(cleanup);
    Ok(bytes)
}

/// Deletes the exported file whatever happens.
struct OutFile(PathBuf);

impl Drop for OutFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ---- LibreOffice ----

/// LibreOffice's settings for QuickConvert, kept apart from the user's own LibreOffice settings
/// (and from a LibreOffice window they may have open).
fn libreoffice_profile() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/com.ripun.quickconvert/libreoffice-profile")
}

fn libreoffice_pdf(input: &Path) -> Result<Vec<u8>, AppError> {
    let app = super::find_app(Engine::Libreoffice, &super::app_dirs());
    let soffice = app
        .path
        .map(|p| PathBuf::from(p).join("Contents/MacOS/soffice"))
        .filter(|p| p.exists())
        .ok_or_else(|| AppError::invalid("LibreOffice isn't installed any more."))?;
    let _one_at_a_time = LIBREOFFICE.lock().unwrap_or_else(|p| p.into_inner());
    libreoffice_convert(&soffice, &libreoffice_profile(), input)
}

/// Run in the background, LibreOffice finds fonts through its own font list, which on a Mac only
/// holds the fonts it brings with it (so Arial printed as Liberation Sans). This list adds the
/// Mac's font folders, including fonts downloaded through Font Book; it's kept next to the
/// settings folder and returned for `FONTCONFIG_FILE`.
fn libreoffice_font_config(profile: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let mut dirs = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        home.join("Library/Fonts"),
    ];
    if let Ok(assets) = std::fs::read_dir("/System/Library/AssetsV2") {
        let mut downloaded: Vec<PathBuf> = assets
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("com_apple_MobileAsset_Font")
            })
            .map(|e| e.path())
            .collect();
        downloaded.sort();
        dirs.extend(downloaded);
    }
    let folder = profile.with_file_name("libreoffice-fonts");
    let escape = |p: &Path| {
        p.to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n<fontconfig>\n",
    );
    for dir in &dirs {
        xml.push_str(&format!("  <dir>{}</dir>\n", escape(dir)));
    }
    xml.push_str(&format!(
        "  <cachedir>{}</cachedir>\n</fontconfig>\n",
        escape(&folder.join("cache"))
    ));
    let conf = folder.join("fonts.conf");
    if std::fs::read_to_string(&conf).ok().as_deref() != Some(xml.as_str()) {
        std::fs::create_dir_all(&folder).ok()?;
        std::fs::write(&conf, xml).ok()?;
    }
    Some(conf)
}

/// `file://` URL for a path, every byte outside letters, digits and `-._~/` percent-encoded.
/// LibreOffice crashes on start-up when its settings path isn't a proper URL (e.g. the space in
/// "Application Support").
fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut url = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            url.push(char::from(b));
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Headless `soffice --convert-to pdf` with QuickConvert's own settings folder. If LibreOffice dies
/// while starting (a half-made or damaged settings folder), the folder is reset and it's tried once
/// more.
fn libreoffice_convert(soffice: &Path, profile: &Path, input: &Path) -> Result<Vec<u8>, AppError> {
    let out_dir = std::env::temp_dir().join(format!(
        "quickconvert-lo-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir)?;
    let cleanup = OutDir(out_dir.clone());
    // LibreOffice names the PDF after the whole file name minus its last extension.
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let pdf = out_dir.join(format!("{stem}.pdf"));
    let fonts = libreoffice_font_config(profile);
    for attempt in 0..2 {
        let mut cmd = Command::new(soffice);
        if let Some(conf) = &fonts {
            cmd.env("FONTCONFIG_FILE", conf);
        }
        cmd.arg(format!("-env:UserInstallation={}", file_url(profile)))
            .args([
                "--headless",
                "--norestore",
                "--nolockcheck",
                "--nologo",
                "--nodefault",
                "--convert-to",
                "pdf",
                "--outdir",
            ])
            .arg(&out_dir)
            .arg(input);
        let (ok, log) = run_limited(cmd, APP_LIMIT, "LibreOffice")?;
        if ok && pdf.exists() {
            let bytes = std::fs::read(&pdf)?;
            drop(cleanup);
            return Ok(bytes);
        }
        eprintln!("[office] LibreOffice attempt {attempt}: {}", log.trim());
        let crashed_starting =
            log.contains("uncaught exception") || log.contains("RuntimeException");
        if attempt == 0 && crashed_starting {
            let _ = std::fs::remove_dir_all(profile);
            continue;
        }
        break;
    }
    Err(AppError::invalid(
        "LibreOffice couldn't make a PDF of this file. Try again, or pick another way to convert it.",
    ))
}

struct OutDir(PathBuf);

impl Drop for OutDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---- Thumbnails ----

pub fn thumbnail(path: &Path, size: u32) -> Result<Vec<u8>, AppError> {
    let dir = std::env::temp_dir().join(format!(
        "quickconvert-ql-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&dir)?;
    let cleanup = OutDir(dir.clone());
    let mut cmd = Command::new("/usr/bin/qlmanage");
    cmd.args(["-t", "-s", &size.to_string(), "-o"])
        .arg(&dir)
        .arg(path);
    run_limited(cmd, Duration::from_secs(20), "The preview")?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let png = dir.join(format!("{name}.png"));
    let bytes = std::fs::read(&png).map_err(|_| AppError::Unsupported {
        extension: crate::output::extension(path),
    })?;
    drop(cleanup);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::pdf_render;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-office-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A Word document made by macOS's own tools: a heading, a long paragraph, a tab-aligned list.
    pub(crate) fn word_file(dir: &Path, ext: &str) -> PathBuf {
        let rtf = dir.join("report.rtf");
        let body =
            "This paragraph is long enough to wrap onto several lines of the page. ".repeat(30);
        std::fs::write(
            &rtf,
            format!(
                r"{{\rtf1\ansi{{\fonttbl\f0\fswiss Helvetica;}}\f0\fs40 \b Quarterly report\b0\par\fs24 {body}\par Rent\tab 1,200\par Food\tab 450\par}}"
            ),
        )
        .unwrap();
        if ext == "rtf" {
            return rtf;
        }
        let out = dir.join(format!("report.{ext}"));
        let ok = Command::new("/usr/bin/textutil")
            .args(["-convert", ext, "-output"])
            .arg(&out)
            .arg(&rtf)
            .status()
            .unwrap()
            .success();
        assert!(ok, "textutil made {ext}");
        out
    }

    fn pdfium() -> &'static pdfium_render::prelude::Pdfium {
        pdf_render::pdfium(&[PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium")])
            .unwrap()
    }

    /// Text of every page, and the first page rendered small.
    fn inspect(pdf: &[u8]) -> (Vec<String>, image::GrayImage) {
        // Tests run in parallel: every call gets its own folder.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = temp_dir(&format!("inspect-{n}"));
        let path = dir.join("out.pdf");
        std::fs::write(&path, pdf).unwrap();
        let lib = pdfium();
        let doc = lib.load_pdf_from_file(&path, None).unwrap();
        let texts = doc
            .pages()
            .iter()
            .map(|p| p.text().map(|t| t.all()).unwrap_or_default())
            .collect();
        let first = pdf_render::page_image(lib, &path, 1, 400)
            .unwrap()
            .to_luma8();
        drop(doc);
        std::fs::remove_dir_all(dir).unwrap();
        (texts, first)
    }

    /// The top fifth of the page has ink: text starts at the top, it isn't centred.
    fn starts_at_top(page: &image::GrayImage) -> bool {
        let band = page.height() / 5;
        (0..band).any(|y| (0..page.width()).any(|x| page.get_pixel(x, y).0[0] < 128))
    }

    #[test]
    fn quick_word_formats_become_paginated_pdfs_starting_at_the_top() {
        let dir = temp_dir("word");
        for ext in ["docx", "doc", "rtf", "odt"] {
            let input = word_file(&dir, ext);
            let pdf = to_pdf(&input, Engine::Quick, QuickOptions::default()).unwrap();
            let (pages, first) = inspect(&pdf);
            assert!(!pages.is_empty(), "{ext}");
            assert!(
                pages[0].contains("Quarterly report"),
                "{ext}: {:?}",
                &pages[0][..40.min(pages[0].len())]
            );
            assert!(starts_at_top(&first), "{ext} centred on the page");
            // A4 portrait: 400 px on the long side → about 283 wide.
            assert!(first.width() < first.height(), "{ext}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn quick_spreadsheet_keeps_every_sheet_and_any_language() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/budget.xlsx");
        let pdf = to_pdf(&path, Engine::Quick, QuickOptions::default()).unwrap();
        let (pages, first) = inspect(&pdf);
        assert_eq!(pages.len(), 2, "one page per sheet");
        assert!(pages[0].contains("Rent") && pages[0].contains("1200"));
        // The page shows "किराया" correctly; PDF text extraction can't map its reordered vowel sign
        // (it reads "क# राया"), so check the rest of the word sits on one line with its amount.
        assert!(
            pages[0].contains("राया 300"),
            "Hindi on one line: {:?}",
            pages[0]
        );
        assert!(pages[1].contains("Notes from the café"));
        assert!(starts_at_top(&first));

        let landscape = QuickOptions {
            landscape: true,
            first_sheet_only: true,
            ..QuickOptions::default()
        };
        let (pages, first) = inspect(&to_pdf(&path, Engine::Quick, landscape).unwrap());
        assert_eq!(pages.len(), 1, "first sheet only");
        assert!(first.width() > first.height(), "landscape");
    }

    #[test]
    fn quick_csv_and_unreadable_files() {
        let dir = temp_dir("csv");
        let csv = dir.join("list.csv");
        std::fs::write(&csv, "Name,Score\nAsha,91\nRavi,88\n").unwrap();
        let (pages, _) = inspect(&to_pdf(&csv, Engine::Quick, QuickOptions::default()).unwrap());
        assert!(pages[0].contains("Asha") && pages[0].contains("91"));

        let broken = dir.join("broken.docx");
        std::fs::write(&broken, b"not a word file").unwrap();
        let err = to_pdf(&broken, Engine::Quick, QuickOptions::default()).unwrap_err();
        assert_eq!(err.code(), "corrupt");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn office_files_get_thumbnails_with_nothing_installed() {
        let dir = temp_dir("thumb");
        let png = thumbnail(&word_file(&dir, "docx"), 128).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_urls_encode_spaces_and_accents() {
        assert_eq!(
            file_url(Path::new("/Users/me/Library/Application Support/x")),
            "file:///Users/me/Library/Application%20Support/x"
        );
        assert_eq!(file_url(Path::new("/tmp/Café")), "file:///tmp/Caf%C3%A9");
    }

    /// The real LibreOffice, when installed (skipped otherwise): a settings folder with a space in
    /// its path, a half-made settings folder like a crash leaves behind, and a dotted file name.
    #[test]
    fn libreoffice_converts_with_awkward_paths_when_installed() {
        let Some(app) = super::super::find_app(Engine::Libreoffice, &super::super::app_dirs()).path
        else {
            eprintln!("LibreOffice not installed: skipped");
            return;
        };
        let soffice = PathBuf::from(app).join("Contents/MacOS/soffice");
        let dir = temp_dir("libreoffice");
        let input = dir.join("report.v2.docx");
        std::fs::rename(word_file(&dir, "docx"), &input).unwrap();
        let profile = dir.join("Application Support/libreoffice-profile");
        // What a crashed start leaves: the folder begun, its settings missing.
        std::fs::create_dir_all(profile.join("user/extensions")).unwrap();
        std::fs::write(profile.join("user/extensions/buildid"), "").unwrap();

        let pdf = libreoffice_convert(&soffice, &profile, &input).unwrap();
        let (pages, _) = inspect(&pdf);
        assert!(pages[0].contains("Quarterly report"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stuck_tools_are_stopped() {
        let mut cmd = Command::new("/bin/sleep");
        cmd.arg("5");
        let err = run_limited(cmd, Duration::from_millis(200), "Keynote").unwrap_err();
        assert!(err.to_string().starts_with("Keynote took too long"));
    }
}
