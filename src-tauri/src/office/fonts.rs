//! Fonts a document uses that aren't on this Mac are swapped for the closest ones that are, in a
//! temporary copy that's then converted (the original is never touched). Nobody has to download
//! fonts: the match comes from a list of well-known fonts, then the font's kind recorded in the file
//! (PANOSE codes in Word / PowerPoint, the family class in Excel and OpenDocument), then words in
//! its name, and finally a plain sans-serif. Works for .docx .pptx .xlsx .odt .odp .ods (zip + XML).

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::Engine;
use crate::errors::AppError;

/// One replacement, for the Result note ("Univers → Helvetica Neue").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FontSwap {
    pub from: String,
    pub to: String,
}

/// What kind of font a document says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sans,
    Serif,
    Mono,
    Script,
    Display,
}

/// A font the document uses, and its kind if the file records one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontUse {
    pub name: String,
    pub kind: Option<Kind>,
}

/// Font families that can be used, matched without regard to case.
#[derive(Debug, Clone, Default)]
pub struct Families(HashMap<String, String>);

impl Families {
    pub fn from_names<I: IntoIterator<Item = S>, S: Into<String>>(names: I) -> Self {
        Families(
            names
                .into_iter()
                .map(|n| {
                    let n = n.into();
                    (n.to_lowercase(), n)
                })
                .collect(),
        )
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(&name.to_lowercase()).map(String::as_str)
    }

    fn extend(&mut self, names: &[&str]) {
        for n in names {
            self.0.insert(n.to_lowercase(), (*n).to_string());
        }
    }
}

/// Formats whose fonts can be listed and swapped.
pub fn supports(path: &Path) -> bool {
    matches!(
        crate::output::extension(path).as_str(),
        "docx" | "docm" | "pptx" | "xlsx" | "xlsm" | "odt" | "odp" | "ods"
    )
}

// ---- What the Mac has ----

/// Every font family on this Mac (asked of macOS, remembered for a minute).
pub fn installed() -> Families {
    static CACHE: Mutex<Option<(Instant, Families)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, families)) = cache.as_ref() {
        if at.elapsed() < Duration::from_secs(60) {
            return families.clone();
        }
    }
    let families = Families::from_names(system_families());
    *cache = Some((Instant::now(), families.clone()));
    families
}

#[cfg(target_os = "macos")]
fn system_families() -> Vec<String> {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-l",
            "JavaScript",
            "-e",
            "ObjC.import('AppKit'); JSON.stringify(ObjC.deepUnwrap($.NSFontManager.sharedFontManager.availableFontFamilies))",
        ])
        .output();
    out.ok()
        .filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice(&o.stdout).ok())
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn system_families() -> Vec<String> {
    Vec::new()
}

/// Fonts LibreOffice brings with it (usable only when LibreOffice converts).
const LIBREOFFICE_FONTS: &[&str] = &[
    "Carlito",
    "Caladea",
    "Liberation Sans",
    "Liberation Serif",
    "Liberation Mono",
    "DejaVu Sans",
    "DejaVu Serif",
    "DejaVu Sans Mono",
    "Noto Sans",
    "Noto Serif",
];

/// What `engine` can draw with: the Mac's fonts, plus LibreOffice's own when it converts.
pub fn available_for(engine: Engine) -> Families {
    let mut families = installed();
    if engine == Engine::Libreoffice {
        families.extend(LIBREOFFICE_FONTS);
    }
    families
}

// ---- Choosing ----

/// Well-known fonts that often aren't on a Mac, and what to use instead (first one available).
/// Name prefixes, most specific first.
const KNOWN: &[(&[&str], &[&str])] = &[
    (&["segoe ui emoji"], &["Apple Color Emoji"]),
    (
        &["segoe ui symbol", "segoe mdl2", "segoe fluent"],
        &["Apple Symbols"],
    ),
    (
        &[
            "segoe print",
            "segoe script",
            "ink free",
            "lucida handwriting",
        ],
        &["Bradley Hand", "Noteworthy", "Chalkboard SE"],
    ),
    (
        &["avenir next lt", "avenir lt", "avenir"],
        &["Avenir Next", "Avenir"],
    ),
    (&["gill sans"], &["Gill Sans"]),
    (&["calibri"], &["Carlito", "Helvetica Neue"]),
    (&["cambria math"], &["STIX Two Math", "Cambria Math"]),
    (&["cambria"], &["Caladea", "Georgia"]),
    (
        &["century gothic", "tw cen", "futura"],
        &["Futura", "Avenir Next"],
    ),
    (
        &["franklin gothic", "news gothic", "trade gothic"],
        &["Avenir Next Condensed", "Helvetica Neue"],
    ),
    (
        &["frutiger", "myriad", "candara", "corbel"],
        &["Avenir Next", "Optima", "Helvetica Neue"],
    ),
    (
        &[
            "aptos",
            "segoe ui",
            "univers",
            "helvetica now",
            "neue haas",
            "akzidenz",
            "arial nova",
            "nimbus sans",
            "roboto",
            "open sans",
            "inter",
            "lato",
            "source sans",
            "selawik",
            "tahoma",
        ],
        &["Helvetica Neue", "Helvetica", "Arial"],
    ),
    (
        &["bahnschrift", "din"],
        &["DIN Alternate", "DIN Condensed", "Helvetica Neue"],
    ),
    (
        &["constantia", "georgia pro", "calisto", "charter"],
        &["Georgia", "Charter"],
    ),
    (&["book antiqua", "palatino"], &["Palatino"]),
    (
        &["garamond", "eb garamond", "sabon", "bembo"],
        &["EB Garamond", "Hoefler Text", "Baskerville"],
    ),
    (
        &["bookman", "century schoolbook", "new century"],
        &["Baskerville", "Georgia"],
    ),
    (
        &[
            "felix titling",
            "trajan",
            "copperplate gothic",
            "perpetua titling",
            "castellar",
            "engravers",
        ],
        &["Copperplate", "Didot", "Baskerville"],
    ),
    (
        &[
            "consolas",
            "cascadia",
            "lucida console",
            "source code",
            "fira code",
            "jetbrains mono",
            "roboto mono",
            "ubuntu mono",
        ],
        &["Menlo", "Courier New", "Courier"],
    ),
    (
        &[
            "nirmala ui",
            "mangal",
            "kokila",
            "aparajita",
            "utsaah",
            "sanskrit text",
        ],
        &[
            "Kohinoor Devanagari",
            "Devanagari Sangam MN",
            "Devanagari MT",
        ],
    ),
    (
        &["ms gothic", "ms pgothic", "yu gothic", "meiryo"],
        &["Hiragino Sans", "Hiragino Kaku Gothic ProN"],
    ),
    (
        &["ms mincho", "ms pmincho", "yu mincho"],
        &["Hiragino Mincho ProN"],
    ),
    (&["microsoft yahei", "dengxian", "simhei"], &["PingFang SC"]),
    (
        &["simsun", "nsimsun", "fangsong", "kaiti"],
        &["Songti SC", "STSong"],
    ),
    (
        &["microsoft jhenghei", "mingliu", "pmingliu"],
        &["PingFang TC"],
    ),
    (
        &["malgun gothic", "batang", "gulim", "dotum"],
        &["Apple SD Gothic Neo"],
    ),
    (&["leelawadee"], &["Thonburi"]),
    (
        &["estrangelo edessa"],
        &["Noto Sans Syriac", "Helvetica Neue"],
    ),
    (
        &[
            "dubai",
            "sakkal",
            "traditional arabic",
            "arabic typesetting",
            "simplified arabic",
        ],
        &["Geeza Pro", "Al Nile"],
    ),
    (
        &["david", "miriam", "narkisim", "frank ruehl"],
        &["Arial Hebrew", "Raanana"],
    ),
    (&["ebrima", "gadugi", "nyala"], &["Helvetica Neue"]),
];

/// Plain fonts for a kind, first available.
fn generic(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Sans => &["Helvetica Neue", "Helvetica", "Arial"],
        Kind::Serif => &["Georgia", "Times New Roman", "Times"],
        Kind::Mono => &["Menlo", "Courier New", "Courier"],
        Kind::Script => &["Snell Roundhand", "Apple Chancery", "Bradley Hand"],
        Kind::Display => &["Copperplate", "Didot", "Helvetica Neue"],
    }
}

/// The kind from a 10-byte PANOSE code (hex): family kind, then serif style and proportion.
pub fn kind_from_panose(hex: &str) -> Option<Kind> {
    let bytes: Vec<u8> = (0..hex.len().min(20))
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    let [family, serif, _, proportion, ..] = bytes[..] else {
        return None;
    };
    match family {
        2 if proportion == 9 => Some(Kind::Mono),
        2 if (11..=15).contains(&serif) => Some(Kind::Sans),
        2 if (2..=10).contains(&serif) => Some(Kind::Serif),
        3 => Some(Kind::Script),
        4 => Some(Kind::Display),
        _ => None,
    }
}

/// The kind from a family class: Word's "swiss" / "roman" / …, ODF's generic family, or Excel's
/// number (1 Roman, 2 Swiss, 3 Modern, 4 Script, 5 Decorative).
pub fn kind_from_class(class: &str) -> Option<Kind> {
    match class.to_ascii_lowercase().as_str() {
        "swiss" | "2" => Some(Kind::Sans),
        "roman" | "1" => Some(Kind::Serif),
        "modern" | "3" => Some(Kind::Mono),
        "script" | "4" => Some(Kind::Script),
        "decorative" | "5" => Some(Kind::Display),
        _ => None,
    }
}

/// The kind from words in the name ("… Mono", "… Script", "… Serif").
fn kind_from_name(name: &str) -> Option<Kind> {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["mono", "code", "courier", "console"]) {
        Some(Kind::Mono)
    } else if has(&["script", "hand", "brush", "callig"]) {
        Some(Kind::Script)
    } else if has(&["titling", "display", "poster", "stencil"]) {
        Some(Kind::Display)
    } else if has(&["sans", "gothic", "grotesk", "grotesque", " ui"]) {
        Some(Kind::Sans)
    } else if has(&[
        "serif", "roman", "times", "book", "antiqua", "garamond", "mincho", "song",
    ]) {
        Some(Kind::Serif)
    } else {
        None
    }
}

/// Picture fonts (bullets, arrows, ticks): a letter font would turn them into letters, so these
/// stay as they are and the converter uses its own fallback.
fn is_symbols(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "wingdings",
        "webdings",
        "symbol",
        "marlett",
        "mt extra",
        "zapf dingbats",
        "segoe mdl2",
    ]
    .iter()
    .any(|s| n.starts_with(s))
}

/// The font to use instead of `font`, or None when it's already available.
pub fn replacement(font: &FontUse, available: &Families) -> Option<String> {
    if available.get(&font.name).is_some() || is_symbols(&font.name) {
        return None;
    }
    let first = |candidates: &[&str]| {
        candidates
            .iter()
            .find_map(|c| available.get(c).map(str::to_string))
    };
    let name = font.name.to_lowercase();
    if let Some(found) = KNOWN
        .iter()
        .filter(|(prefixes, _)| prefixes.iter().any(|p| name.starts_with(p)))
        .find_map(|(_, candidates)| first(candidates))
    {
        return Some(found);
    }
    let kind = font
        .kind
        .or_else(|| kind_from_name(&font.name))
        .unwrap_or(Kind::Sans);
    first(generic(kind)).or_else(|| first(generic(Kind::Sans)))
}

/// Swaps for every font in `uses` that isn't available (each name once).
pub fn plan(uses: &[FontUse], available: &Families) -> Vec<FontSwap> {
    let mut seen = HashSet::new();
    uses.iter()
        .filter(|u| seen.insert(u.name.to_lowercase()))
        .filter_map(|u| {
            replacement(u, available).map(|to| FontSwap {
                from: u.name.clone(),
                to,
            })
        })
        .collect()
}

// ---- Reading a document's fonts ----

fn unescape(s: &str) -> String {
    s.replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The value of `attr="…"` inside `tag` (one element's opening tag or block).
fn attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let key = format!("{attr}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

/// Every element (opening tag through its end, or the self-closing tag) named `name`.
fn elements<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = xml[at..].find(&open) {
        let start = at + found;
        let after = xml[start + open.len()..].chars().next();
        if !matches!(after, Some(' ' | '>' | '/' | '\n' | '\t' | '\r')) {
            at = start + open.len();
            continue;
        }
        let Some(tag_end) = xml[start..].find('>').map(|e| start + e + 1) else {
            break;
        };
        let end = if xml[..tag_end].ends_with("/>") {
            tag_end
        } else {
            xml[tag_end..]
                .find(&close)
                .map_or(tag_end, |e| tag_end + e + close.len())
        };
        out.push(&xml[start..end]);
        at = end;
    }
    out
}

/// The fonts an XML part names, with their kinds where the part records them.
fn fonts_in(xml: &str, part: &str) -> Vec<FontUse> {
    let mut out = Vec::new();
    let mut add = |name: Option<&str>, kind: Option<Kind>| {
        if let Some(name) = name
            .map(unescape)
            .map(|n| n.trim_matches('\'').trim().to_string())
        {
            // "+mj-lt" style names point at the theme, not at a font.
            if !name.is_empty() && !name.starts_with('+') {
                out.push(FontUse { name, kind });
            }
        }
    };
    // Word font table: name, PANOSE and family class.
    for font in elements(xml, "w:font") {
        let panose = elements(font, "w:panose1")
            .first()
            .and_then(|p| attr(p, "w:val"))
            .and_then(kind_from_panose);
        let class = elements(font, "w:family")
            .first()
            .and_then(|f| attr(f, "w:val"))
            .and_then(kind_from_class);
        add(attr(font, "w:name"), panose.or(class));
    }
    // Word runs.
    for fonts in elements(xml, "w:rFonts") {
        for a in ["w:ascii", "w:hAnsi", "w:eastAsia", "w:cs"] {
            add(attr(fonts, a), None);
        }
    }
    // DrawingML (PowerPoint, and themes everywhere): typeface plus PANOSE.
    for tag in ["a:latin", "a:ea", "a:cs", "a:sym", "a:buFont"] {
        for el in elements(xml, tag) {
            add(
                attr(el, "typeface"),
                attr(el, "panose").and_then(kind_from_panose),
            );
        }
    }
    // Excel: cell fonts and rich text, with the family class.
    if part.starts_with("xl/") {
        for font in elements(xml, "font")
            .into_iter()
            .chain(elements(xml, "rPr"))
        {
            let class = elements(font, "family")
                .first()
                .and_then(|f| attr(f, "val"))
                .and_then(kind_from_class);
            let name = elements(font, "name")
                .first()
                .and_then(|n| attr(n, "val"))
                .or_else(|| elements(font, "rFont").first().and_then(|n| attr(n, "val")));
            add(name, class);
        }
    }
    // OpenDocument font faces.
    for face in elements(xml, "style:font-face") {
        let class = attr(face, "style:font-family-generic").and_then(kind_from_class);
        add(
            attr(face, "svg:font-family").or_else(|| attr(face, "style:name")),
            class,
        );
    }
    out
}

/// Whether a part of the zip can name fonts.
fn font_part(name: &str) -> bool {
    name.ends_with(".xml")
        && (name.starts_with("word/")
            || name.starts_with("ppt/")
            || name.starts_with("xl/")
            || name == "content.xml"
            || name == "styles.xml")
}

/// Every font the document uses, first mention (with a kind, if any mention has one) first.
pub fn used(path: &Path) -> Result<Vec<FontUse>, AppError> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| AppError::Corrupt {
        detail: e.to_string(),
    })?;
    let mut uses: Vec<FontUse> = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| AppError::Corrupt {
            detail: e.to_string(),
        })?;
        let Ok(name) = entry.name().map(|n| n.to_string()) else {
            continue;
        };
        if !font_part(&name) {
            continue;
        }
        let mut xml = String::new();
        if entry.read_to_string(&mut xml).is_err() {
            continue;
        }
        for font in fonts_in(&xml, &name) {
            match uses
                .iter_mut()
                .find(|u| u.name.eq_ignore_ascii_case(&font.name))
            {
                Some(known) => known.kind = known.kind.or(font.kind),
                None => uses.push(font),
            }
        }
    }
    Ok(uses)
}

// ---- Writing the copy ----

/// A copy of `input` at `output` with each swap's font name replaced in the document's XML.
/// Other parts are copied byte for byte (in order, so an OpenDocument `mimetype` stays first).
pub fn rewrite(input: &Path, swaps: &[FontSwap], output: &Path) -> Result<(), AppError> {
    let corrupt = |e: zip::result::ZipError| AppError::Corrupt {
        detail: e.to_string(),
    };
    let mut zip = zip::ZipArchive::new(std::fs::File::open(input)?).map_err(corrupt)?;
    let mut out = zip::ZipWriter::new(std::fs::File::create(output)?);
    for i in 0..zip.len() {
        let name = zip
            .by_index_raw(i)
            .map_err(corrupt)?
            .name()
            .map_err(corrupt)?
            .to_string();
        if !font_part(&name) {
            out.raw_copy_file(zip.by_index_raw(i).map_err(corrupt)?)
                .map_err(corrupt)?;
            continue;
        }
        let mut xml = String::new();
        zip.by_index(i).map_err(corrupt)?.read_to_string(&mut xml)?;
        let changed = swap_names(&xml, swaps);
        if changed == xml {
            out.raw_copy_file(zip.by_index_raw(i).map_err(corrupt)?)
                .map_err(corrupt)?;
            continue;
        }
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        out.start_file(name, options).map_err(corrupt)?;
        out.write_all(changed.as_bytes())?;
    }
    out.finish().map_err(corrupt)?;
    Ok(())
}

/// Attributes that hold a font family (`<name val=` and `<rFont val=` are Excel's). OpenDocument
/// styles point at a font face by its own name, so only the face's family changes there.
const FONT_ATTRS: &[&str] = &[
    " w:name=",
    " w:ascii=",
    " w:hAnsi=",
    " w:eastAsia=",
    " w:cs=",
    " typeface=",
    "<name val=",
    "<rFont val=",
    " svg:font-family=",
    " fo:font-family=",
];

/// Replace font attributes whose value is exactly a swapped font's name (also ODF's quoted form).
fn swap_names(xml: &str, swaps: &[FontSwap]) -> String {
    let mut text = xml.to_string();
    for swap in swaps {
        let (from, to) = (escape(&swap.from), escape(&swap.to));
        for attr in FONT_ATTRS {
            for (open, close) in [("\"", "\""), ("\"&apos;", "&apos;\""), ("\"'", "'\"")] {
                text = text.replace(
                    &format!("{attr}{open}{from}{close}"),
                    &format!("{attr}{open}{to}{close}"),
                );
            }
        }
    }
    text
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-fonts-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A zip with these parts, in order (the first stored, like an OpenDocument mimetype).
    pub(crate) fn zip_file(path: &Path, parts: &[(&str, &str)]) {
        let mut out = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (i, (name, body)) in parts.iter().enumerate() {
            let method = if i == 0 {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };
            out.start_file(
                *name,
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
            out.write_all(body.as_bytes()).unwrap();
        }
        out.finish().unwrap();
    }

    fn part(path: &Path, name: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut s = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    fn mac() -> Families {
        Families::from_names([
            "Avenir Next",
            "Gill Sans",
            "Helvetica Neue",
            "Helvetica",
            "Arial",
            "Georgia",
            "Times New Roman",
            "Menlo",
            "Copperplate",
            "Didot",
            "Snell Roundhand",
            "Kohinoor Devanagari",
            "Futura",
        ])
    }

    fn font(name: &str, kind: Option<Kind>) -> FontUse {
        FontUse {
            name: name.into(),
            kind,
        }
    }

    #[test]
    fn known_fonts_get_their_mac_lookalikes() {
        let mac = mac();
        let r = |n: &str| replacement(&font(n, None), &mac);
        assert_eq!(r("Avenir Next LT Pro"), Some("Avenir Next".into()));
        assert_eq!(r("Gill Sans Nova Light"), Some("Gill Sans".into()));
        assert_eq!(r("Univers"), Some("Helvetica Neue".into()));
        assert_eq!(r("Felix Titling"), Some("Copperplate".into()));
        assert_eq!(r("Nirmala UI"), Some("Kohinoor Devanagari".into()));
        assert_eq!(r("Consolas"), Some("Menlo".into()));
        assert_eq!(r("Arial"), None, "installed: left alone");
        assert_eq!(r("arial"), None, "case doesn't matter");
        assert_eq!(r("Wingdings 3"), None, "symbol fonts stay");
    }

    #[test]
    fn calibri_and_cambria_use_libreoffices_same_width_fonts_only_there() {
        let mut lo = mac();
        lo.extend(LIBREOFFICE_FONTS);
        assert_eq!(
            replacement(&font("Calibri", None), &lo),
            Some("Carlito".into())
        );
        assert_eq!(
            replacement(&font("Cambria", None), &lo),
            Some("Caladea".into())
        );
        assert_eq!(
            replacement(&font("Calibri", None), &mac()),
            Some("Helvetica Neue".into())
        );
        assert_eq!(
            replacement(&font("Cambria", None), &mac()),
            Some("Georgia".into())
        );
    }

    #[test]
    fn unknown_fonts_follow_their_kind_then_their_name() {
        let mac = mac();
        assert_eq!(
            replacement(&font("Zapfino Pro", Some(Kind::Script)), &mac),
            Some("Snell Roundhand".into())
        );
        assert_eq!(
            replacement(&font("Mystery Serif", None), &mac),
            Some("Georgia".into())
        );
        assert_eq!(
            replacement(&font("Acme Mono", None), &mac),
            Some("Menlo".into())
        );
        assert_eq!(
            replacement(&font("Brandname", Some(Kind::Serif)), &mac),
            Some("Georgia".into())
        );
        assert_eq!(
            replacement(&font("Brandname", None), &mac),
            Some("Helvetica Neue".into()),
            "default: sans"
        );
    }

    #[test]
    fn panose_and_class_codes() {
        assert_eq!(kind_from_panose("020B0504020202020204"), Some(Kind::Sans));
        assert_eq!(kind_from_panose("02020603050405020304"), Some(Kind::Serif)); // Times New Roman
        assert_eq!(kind_from_panose("020B0609020204030204"), Some(Kind::Mono)); // Consolas
        assert_eq!(kind_from_panose("03010101010101010101"), Some(Kind::Script));
        assert_eq!(kind_from_panose("zz"), None);
        assert_eq!(kind_from_class("swiss"), Some(Kind::Sans));
        assert_eq!(kind_from_class("1"), Some(Kind::Serif));
    }

    #[test]
    fn reads_fonts_from_word_powerpoint_excel_and_opendocument() {
        let dir = temp_dir("read");
        let docx = dir.join("a.docx");
        zip_file(
            &docx,
            &[
                ("[Content_Types].xml", "<Types/>"),
                (
                    "word/fontTable.xml",
                    r#"<w:fonts><w:font w:name="Avenir Next LT Pro"><w:panose1 w:val="020B0504020202020204"/><w:family w:val="swiss"/></w:font><w:font w:name="Univers"><w:family w:val="roman"/></w:font></w:fonts>"#,
                ),
                (
                    "word/document.xml",
                    r#"<w:document><w:r><w:rPr><w:rFonts w:ascii="Avenir Next LT Pro" w:hAnsi="Avenir Next LT Pro"/></w:rPr></w:r><w:r><w:rPr><w:rFonts w:asciiTheme="minorHAnsi"/></w:rPr></w:r></w:document>"#,
                ),
            ],
        );
        let uses = used(&docx).unwrap();
        assert_eq!(
            uses,
            vec![
                font("Avenir Next LT Pro", Some(Kind::Sans)),
                font("Univers", Some(Kind::Serif))
            ]
        );

        let pptx = dir.join("a.pptx");
        zip_file(
            &pptx,
            &[
                ("[Content_Types].xml", "<Types/>"),
                (
                    "ppt/theme/theme1.xml",
                    r#"<a:theme><a:latin typeface="Felix Titling" panose="020F0502020204030204"/><a:ea typeface=""/><a:latin typeface="+mj-lt"/></a:theme>"#,
                ),
                (
                    "ppt/slides/slide1.xml",
                    r#"<p:sld><a:latin typeface="Gill Sans Nova Light"/></p:sld>"#,
                ),
            ],
        );
        let names: Vec<String> = used(&pptx).unwrap().into_iter().map(|u| u.name).collect();
        assert_eq!(names, ["Felix Titling", "Gill Sans Nova Light"]);

        let xlsx = dir.join("a.xlsx");
        zip_file(
            &xlsx,
            &[
                ("[Content_Types].xml", "<Types/>"),
                (
                    "xl/styles.xml",
                    r#"<styleSheet><fonts><font><sz val="11"/><name val="Univers"/><family val="2"/></font></fonts></styleSheet>"#,
                ),
            ],
        );
        assert_eq!(
            used(&xlsx).unwrap(),
            vec![font("Univers", Some(Kind::Sans))]
        );

        let ods = dir.join("a.ods");
        zip_file(
            &ods,
            &[
                ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
                (
                    "content.xml",
                    r#"<office:document-content><style:font-face style:name="Univers" svg:font-family="&apos;Univers&apos;" style:font-family-generic="swiss"/></office:document-content>"#,
                ),
            ],
        );
        assert_eq!(used(&ods).unwrap(), vec![font("Univers", Some(Kind::Sans))]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_copy_swaps_names_and_keeps_everything_else() {
        let dir = temp_dir("rewrite");
        let ods = dir.join("a.ods");
        let picture = "PNG-BYTES-UNCHANGED";
        zip_file(
            &ods,
            &[
                ("mimetype", "application/vnd.oasis.opendocument.text"),
                (
                    "content.xml",
                    r#"<c><style:font-face style:name="Univers" svg:font-family="&apos;Univers&apos;"/><style:style style:name="Univers"/><text:p style:font-name="Univers" fo:font-family="Univers">Univers is also a word in the text</text:p></c>"#,
                ),
                ("Pictures/logo.png", picture),
            ],
        );
        let copy = dir.join("copy.ods");
        let swaps = [FontSwap {
            from: "Univers".into(),
            to: "Helvetica Neue".into(),
        }];
        rewrite(&ods, &swaps, &copy).unwrap();
        let content = part(&copy, "content.xml");
        assert!(
            content.contains(r#"<style:style style:name="Univers"/>"#),
            "style names stay"
        );
        assert!(content.contains(r#"svg:font-family="&apos;Helvetica Neue&apos;""#));
        assert!(
            content.contains(r#"style:font-name="Univers""#),
            "points at the face"
        );
        assert!(content.contains(r#"fo:font-family="Helvetica Neue""#));
        assert!(
            content.contains("Univers is also a word in the text"),
            "text itself untouched"
        );
        assert_eq!(part(&copy, "Pictures/logo.png"), picture);
        let mut zip = zip::ZipArchive::new(Cursor::new(std::fs::read(&copy).unwrap())).unwrap();
        let first = zip.by_index(0).unwrap();
        assert_eq!(first.name().unwrap(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
