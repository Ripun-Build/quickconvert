//! Images → PDF and PDF merge (lopdf). PDF → images arrives in M4.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use image::{ImageDecoder, ImageReader};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;

use super::image::{flatten_on_white, jpeg_near, load};
use super::metadata;
use crate::errors::AppError;

/// Called before each input with its index; returns `Err(Cancelled)` to stop.
pub type Step<'a> = &'a mut dyn FnMut(usize) -> Result<(), AppError>;

/// Progress/cancel callback for `images_to_pdf`, called from its worker threads.
pub type SharedStep<'a> = &'a (dyn Fn(usize) -> Result<(), AppError> + Sync);

/// The input that stopped `images_to_pdf` (position in `paths`), and why.
#[derive(Debug)]
pub struct ImageFailed {
    pub index: usize,
    pub error: AppError,
}

/// Assumed resolution when an image doesn't say (decided in M3).
const DEFAULT_DPI: f64 = 150.0;
const PT_PER_INCH: f64 = 72.0;
const SMALL_MARGIN_PT: f64 = 10.0 * PT_PER_INCH / 25.4;
const A4_PT: (f64, f64) = (595.276, 841.89);
const LETTER_PT: (f64, f64) = (612.0, 792.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    Fit,
    A4,
    Letter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Margin {
    None,
    Small,
}

pub fn pdf_error(err: lopdf::Error) -> AppError {
    match err {
        lopdf::Error::IO(io) => AppError::Io(io),
        lopdf::Error::Decryption(_)
        | lopdf::Error::InvalidPassword
        | lopdf::Error::UnsupportedSecurityHandler(_) => AppError::PasswordProtected,
        other => AppError::Corrupt {
            detail: other.to_string(),
        },
    }
}

// ---- Images → PDF ----

/// One page per image, in the given order. JPEGs are embedded byte-for-byte (no re-encoding).
/// The heavy part (reading, cleaning or encoding, compressing) runs `workers` images at a time;
/// `step` is called from those threads. A failure names the input it came from.
pub fn images_to_pdf(
    paths: &[PathBuf],
    page_size: PageSize,
    margin: Margin,
    workers: usize,
    step: SharedStep<'_>,
) -> Result<Document, ImageFailed> {
    // After the first failure, images not started yet are skipped (inputs start in order, so the
    // first failure by position is the one to report).
    let stop = AtomicBool::new(false);
    let prepared = crate::pool::map(paths.len(), workers, |i| {
        if stop.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let result = step(i).and_then(|()| prepare_image(&paths[i]));
        if result.is_err() {
            stop.store(true, Ordering::Relaxed);
        }
        result
    });

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::with_capacity(paths.len());

    for (index, item) in prepared.into_iter().enumerate() {
        let prepared = item
            .unwrap_or_else(|| Err(AppError::internal("image worker stopped")))
            .map_err(|error| ImageFailed { index, error })?;
        let image = add_image(&mut doc, prepared);
        let swaps = image.orientation >= 5;
        let display = if swaps {
            (image.height, image.width)
        } else {
            (image.width, image.height)
        };
        let dpi = match (image.dpi, swaps) {
            (Some((x, y)), true) => (y, x),
            (Some(d), false) => d,
            (None, _) => (DEFAULT_DPI, DEFAULT_DPI),
        };
        let layout = page_layout(display, dpi, page_size, margin);
        let m = placement_matrix(image.orientation, layout.x, layout.y, layout.w, layout.h);
        let content = format!(
            "q {} {} {} {} {} {} cm /Im0 Do Q",
            num(m[0]),
            num(m[1]),
            num(m[2]),
            num(m[3]),
            num(m[4]),
            num(m[5])
        );
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(layout.page_w as f32), Object::Real(layout.page_h as f32)],
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image.id } },
            "Contents" => content_id,
        });
        kids.push(Object::Reference(page_id));
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    Ok(doc)
}

struct EmbeddedImage {
    id: ObjectId,
    /// Stored (not displayed) pixel size.
    width: u32,
    height: u32,
    /// EXIF orientation 1–8 still to apply when drawing.
    orientation: u8,
    dpi: Option<(f64, f64)>,
}

/// An image ready to go into the document: everything but its colour space, which needs the
/// document (an ICC profile is an object of its own).
struct PreparedImage {
    stream: Stream,
    icc: Option<Vec<u8>>,
    components: i64,
    device: &'static str,
    width: u32,
    height: u32,
    orientation: u8,
    dpi: Option<(f64, f64)>,
}

fn prepare_image(path: &Path) -> Result<PreparedImage, AppError> {
    let bytes = if crate::heic::is_heic(path) {
        // HEIC: store as a JPEG about as big as the original (not raw pixels), so the PDF stays small.
        let picture = load(path)?;
        let pixels = f64::from(picture.image.width()) * f64::from(picture.image.height());
        let budget = picture.density.map(|d| d * pixels);
        jpeg_near(&picture.image, picture.icc.as_deref(), budget)?.0
    } else {
        std::fs::read(path)?
    };
    let Some(info) = jpeg_info(&bytes).filter(|i| i.precision == 8) else {
        return prepare_decoded(path);
    };
    // Orientation and colour profile from the headers; the bytes themselves are embedded as-is.
    let (orientation, icc) = {
        let mut decoder = ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_decoder().ok());
        let orientation = decoder
            .as_mut()
            .and_then(|d| d.orientation().ok())
            .map(|o| o.to_exif())
            .unwrap_or(1);
        let icc = decoder
            .as_mut()
            .and_then(|d| d.icc_profile().ok().flatten());
        (orientation, icc)
    };
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => i64::from(info.width),
        "Height" => i64::from(info.height),
        "BitsPerComponent" => 8,
        "Filter" => "DCTDecode",
    };
    let (components, device) = match info.components {
        1 => (1, "DeviceGray"),
        3 => (3, "DeviceRGB"),
        4 => {
            if info.adobe {
                // Adobe CMYK JPEGs store inverted values.
                let decode: Vec<Object> = [1, 0, 1, 0, 1, 0, 1, 0].map(Object::from).to_vec();
                dict.set("Decode", decode);
            }
            (4, "DeviceCMYK")
        }
        _ => return prepare_decoded(path),
    };
    // Location, camera and date info stays out of the PDF; the picture data is unchanged.
    let bytes = metadata::clean_jpeg(&bytes, false)
        .map(|c| c.bytes)
        .unwrap_or(bytes);
    let mut stream = Stream::new(dict, bytes);
    stream.allows_compression = false;
    Ok(PreparedImage {
        stream,
        icc,
        components,
        device,
        width: info.width,
        height: info.height,
        orientation,
        dpi: info.dpi,
    })
}

/// Non-JPEG input: decode (orientation applied), flatten alpha on white, store losslessly (Flate).
fn prepare_decoded(path: &Path) -> Result<PreparedImage, AppError> {
    let picture = load(path)?;
    let rgb = flatten_on_white(&picture.image);
    let (width, height) = rgb.dimensions();
    let mut stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(width),
            "Height" => i64::from(height),
            "BitsPerComponent" => 8,
        },
        rgb.into_raw(),
    );
    stream.compress().map_err(pdf_error)?;
    Ok(PreparedImage {
        stream,
        icc: picture.icc,
        components: 3,
        device: "DeviceRGB",
        width,
        height,
        orientation: 1,
        dpi: None,
    })
}

fn add_image(doc: &mut Document, prepared: PreparedImage) -> EmbeddedImage {
    let mut stream = prepared.stream;
    let space = color_space(
        doc,
        prepared.icc.as_deref(),
        prepared.components,
        prepared.device,
    );
    stream.dict.set("ColorSpace", space);
    EmbeddedImage {
        id: doc.add_object(stream),
        width: prepared.width,
        height: prepared.height,
        orientation: prepared.orientation,
        dpi: prepared.dpi,
    }
}

/// `[/ICCBased <profile>]` when the image has a colour profile (PDF viewers ignore profiles inside
/// JPEG data, so wide-gamut phone photos would look washed out), else the plain device space.
fn color_space(doc: &mut Document, icc: Option<&[u8]>, components: i64, device: &str) -> Object {
    let Some(icc) = icc.filter(|p| !p.is_empty()) else {
        return Object::Name(device.as_bytes().to_vec());
    };
    let mut stream = Stream::new(
        dictionary! { "N" => components, "Alternate" => device },
        icc.to_vec(),
    );
    let _ = stream.compress();
    let id = doc.add_object(stream);
    Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        Object::Reference(id),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PageLayout {
    page_w: f64,
    page_h: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Page size and image box (points) for an image of `px` displayed pixels at `dpi`.
fn page_layout(px: (u32, u32), dpi: (f64, f64), page_size: PageSize, margin: Margin) -> PageLayout {
    let m = match margin {
        Margin::None => 0.0,
        Margin::Small => SMALL_MARGIN_PT,
    };
    let natural = (
        f64::from(px.0) * PT_PER_INCH / dpi.0.max(1.0),
        f64::from(px.1) * PT_PER_INCH / dpi.1.max(1.0),
    );
    let paper = match page_size {
        PageSize::Fit => {
            return PageLayout {
                page_w: natural.0 + 2.0 * m,
                page_h: natural.1 + 2.0 * m,
                x: m,
                y: m,
                w: natural.0,
                h: natural.1,
            }
        }
        PageSize::A4 => A4_PT,
        PageSize::Letter => LETTER_PT,
    };
    // Landscape paper for landscape images.
    let (page_w, page_h) = if px.0 > px.1 {
        (paper.1, paper.0)
    } else {
        paper
    };
    let scale = ((page_w - 2.0 * m) / natural.0).min((page_h - 2.0 * m) / natural.1);
    let (w, h) = (natural.0 * scale, natural.1 * scale);
    PageLayout {
        page_w,
        page_h,
        x: (page_w - w) / 2.0,
        y: (page_h - h) / 2.0,
        w,
        h,
    }
}

/// `cm` matrix drawing the stored image upright into the box (x, y, w, h) for EXIF orientation 1–8.
fn placement_matrix(orientation: u8, x: f64, y: f64, w: f64, h: f64) -> [f64; 6] {
    match orientation {
        2 => [-w, 0.0, 0.0, h, x + w, y],
        3 => [-w, 0.0, 0.0, -h, x + w, y + h],
        4 => [w, 0.0, 0.0, -h, x, y + h],
        5 => [0.0, -h, -w, 0.0, x + w, y + h],
        6 => [0.0, -h, w, 0.0, x, y + h],
        7 => [0.0, h, w, 0.0, x, y],
        8 => [0.0, h, -w, 0.0, x + w, y],
        _ => [w, 0.0, 0.0, h, x, y],
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct JpegInfo {
    width: u32,
    height: u32,
    components: u8,
    precision: u8,
    dpi: Option<(f64, f64)>,
    adobe: bool,
}

/// Size, components and JFIF density from JPEG headers, without decoding pixels.
fn jpeg_info(bytes: &[u8]) -> Option<JpegInfo> {
    if bytes.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut dpi = None;
    let mut adobe = false;
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1; // fill byte
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            return None; // reached image data without a frame header
        }
        let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
        let seg = bytes.get(i + 4..i + 2 + len)?;
        match marker {
            0xE0 if dpi.is_none() && seg.starts_with(b"JFIF\0") && seg.len() >= 12 => {
                let (x, y) = (
                    f64::from(u16::from_be_bytes([seg[8], seg[9]])),
                    f64::from(u16::from_be_bytes([seg[10], seg[11]])),
                );
                dpi = match seg[7] {
                    1 if x > 0.0 && y > 0.0 => Some((x, y)),
                    2 if x > 0.0 && y > 0.0 => Some((x * 2.54, y * 2.54)),
                    _ => None,
                };
            }
            0xEE if seg.starts_with(b"Adobe") => adobe = true,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) && seg.len() >= 6 => {
                return Some(JpegInfo {
                    precision: seg[0],
                    height: u32::from(u16::from_be_bytes([seg[1], seg[2]])),
                    width: u32::from(u16::from_be_bytes([seg[3], seg[4]])),
                    components: seg[5],
                    dpi,
                    adobe,
                });
            }
            _ => {}
        }
        i += 2 + len;
    }
    None
}

// ---- PDF merge ----

const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

pub fn load_pdf(path: &Path) -> Result<Document, AppError> {
    Document::load(path).map_err(pdf_error)
}

/// All pages of all inputs, in order, in one new document.
pub fn merge_pdfs(paths: &[PathBuf], step: Step<'_>) -> Result<Document, AppError> {
    let mut next_id = 1;
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut pages: Vec<(ObjectId, Dictionary)> = Vec::new();

    for (i, path) in paths.iter().enumerate() {
        step(i)?;
        let mut doc = load_pdf(path)?;
        inherit_page_attributes(&mut doc)?;
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;

        for page_id in doc.get_pages().into_values() {
            let page = doc.get_dictionary(page_id).map_err(pdf_error)?.clone();
            pages.push((page_id, page));
        }
        for (id, object) in doc.objects {
            let kind = object
                .as_dict()
                .ok()
                .and_then(|d| d.get(b"Type").ok())
                .and_then(|t| t.as_name().ok());
            if !matches!(kind, Some(b"Catalog") | Some(b"Pages")) {
                objects.insert(id, object);
            }
        }
    }
    if pages.is_empty() {
        return Err(AppError::Corrupt {
            detail: "no pages".into(),
        });
    }

    let mut out = Document::with_version("1.5");
    out.objects = objects;
    out.max_id = next_id;
    let pages_id = out.new_object_id();
    let mut kids = Vec::with_capacity(pages.len());
    for (id, mut page) in pages {
        page.set("Parent", pages_id);
        out.objects.insert(id, Object::Dictionary(page));
        kids.push(Object::Reference(id));
    }
    let count = kids.len() as i64;
    out.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog_id = out.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    out.trailer.set("Root", catalog_id);
    out.renumber_objects();
    Ok(out)
}

/// Copy attributes pages inherit from their page-tree parents onto the pages themselves, since the
/// original page trees are dropped when merging.
fn inherit_page_attributes(doc: &mut Document) -> Result<(), AppError> {
    for page_id in doc.get_pages().into_values() {
        let mut page = doc.get_dictionary(page_id).map_err(pdf_error)?.clone();
        for key in INHERITABLE {
            if page.has(key) {
                continue;
            }
            let mut parent = page.get(b"Parent").and_then(Object::as_reference).ok();
            for _ in 0..32 {
                let Some(id) = parent else { break };
                let Ok(node) = doc.get_dictionary(id) else {
                    break;
                };
                if let Ok(value) = node.get(key) {
                    page.set(key, value.clone());
                    break;
                }
                parent = node.get(b"Parent").and_then(Object::as_reference).ok();
            }
        }
        doc.objects.insert(page_id, Object::Dictionary(page));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-pdf-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([200, 100, 50])))
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    /// Insert an APP segment right after SOI.
    fn with_segment(jpeg: &[u8], marker: u8, payload: &[u8]) -> Vec<u8> {
        let len = (payload.len() + 2) as u16;
        let mut out = vec![0xFF, 0xD8, 0xFF, marker];
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn exif_orientation(value: u8) -> Vec<u8> {
        let mut p = b"Exif\0\0MM\0\x2A\0\0\0\x08".to_vec(); // big-endian TIFF, IFD at 8
        p.extend_from_slice(&[0, 1]); // one entry
        p.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, value, 0, 0]); // Orientation, SHORT
        p.extend_from_slice(&[0, 0, 0, 0]); // no next IFD
        p
    }

    fn jfif_dpi(dpi: u16) -> Vec<u8> {
        let mut p = b"JFIF\0\x01\x02\x01".to_vec(); // version 1.2, units = dpi
        p.extend_from_slice(&dpi.to_be_bytes());
        p.extend_from_slice(&dpi.to_be_bytes());
        p.extend_from_slice(&[0, 0]);
        p
    }

    fn ok_step(_: usize) -> Result<(), AppError> {
        Ok(())
    }

    fn no_step() -> impl FnMut(usize) -> Result<(), AppError> {
        |_| Ok(())
    }

    fn reload(doc: &mut Document) -> Document {
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        Document::load_mem(&buf).unwrap()
    }

    fn media_box(doc: &Document, page: ObjectId) -> (f32, f32) {
        let mb = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let n = |o: &Object| o.as_float().unwrap();
        (n(&mb[2]) - n(&mb[0]), n(&mb[3]) - n(&mb[1]))
    }

    fn page_image(doc: &Document, page: ObjectId) -> Stream {
        let res = doc.get_dictionary(page).unwrap().get(b"Resources").unwrap();
        let res = match res {
            Object::Reference(id) => doc.get_dictionary(*id).unwrap(),
            other => other.as_dict().unwrap(),
        };
        let id = res
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Im0")
            .unwrap()
            .as_reference()
            .unwrap();
        doc.get_object(id).unwrap().as_stream().unwrap().clone()
    }

    #[test]
    fn reads_jpeg_headers() {
        let plain = jpeg(320, 200);
        let info = jpeg_info(&plain).unwrap();
        assert_eq!(
            (info.width, info.height, info.components, info.precision),
            (320, 200, 3, 8)
        );

        let dense = with_segment(&plain, 0xE0, &jfif_dpi(300));
        assert_eq!(jpeg_info(&dense).unwrap().dpi, Some((300.0, 300.0)));
        assert_eq!(jpeg_info(b"\x89PNG...."), None);
    }

    #[test]
    fn jpegs_are_embedded_without_reencoding_and_rotated_by_exif() {
        let dir = temp_dir("embed");
        let upright = dir.join("a.jpg");
        let rotated = dir.join("b.jpg");
        let png = dir.join("c.png");
        let upright_bytes = with_segment(&jpeg(300, 150), 0xE0, &jfif_dpi(300));
        let rotated_bytes = with_segment(&jpeg(300, 150), 0xE1, &exif_orientation(6));
        std::fs::write(&upright, &upright_bytes).unwrap();
        std::fs::write(&rotated, &rotated_bytes).unwrap();
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(150, 150, Rgba([0, 0, 0, 0])))
            .save(&png)
            .unwrap();

        let mut doc = images_to_pdf(
            &[upright.clone(), rotated.clone(), png],
            PageSize::Fit,
            Margin::None,
            3,
            &ok_step,
        )
        .unwrap();
        let doc = reload(&mut doc);
        let pages: Vec<_> = doc.get_pages().into_values().collect();
        assert_eq!(pages.len(), 3);

        // 300×150 px at 300 dpi → 72×36 pt.
        assert_eq!(media_box(&doc, pages[0]), (72.0, 36.0));
        // Orientation 6 swaps sides; no dpi → 150 dpi: 150×300 px → 72×144 pt.
        assert_eq!(media_box(&doc, pages[1]), (72.0, 144.0));

        let first = page_image(&doc, pages[0]);
        assert_eq!(
            first.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        // Not re-encoded: the same JPEG with only its metadata blocks removed.
        let cleaned = |b: &[u8]| metadata::clean_jpeg(b, false).unwrap().bytes;
        assert_eq!(
            first.content,
            cleaned(&upright_bytes),
            "JPEG must not be re-encoded"
        );
        let second = page_image(&doc, pages[1]).content;
        assert_eq!(second, cleaned(&rotated_bytes));
        assert!(!second.windows(6).any(|w| w == b"Exif\0\0"));
        let third = page_image(&doc, pages[2]);
        assert_eq!(
            third.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"FlateDecode"
        );

        // Originals untouched.
        assert_eq!(std::fs::read(&upright).unwrap(), upright_bytes);
        assert_eq!(std::fs::read(&rotated).unwrap(), rotated_bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a4_pages_turn_landscape_and_keep_margins() {
        let wide = page_layout((4000, 3000), (150.0, 150.0), PageSize::A4, Margin::Small);
        assert_eq!((wide.page_w, wide.page_h), (A4_PT.1, A4_PT.0));
        assert!(wide.x >= SMALL_MARGIN_PT - 1e-6 && wide.y >= SMALL_MARGIN_PT - 1e-6);
        assert!((wide.w / wide.h - 4.0 / 3.0).abs() < 1e-6);

        let tall = page_layout((1000, 2000), (150.0, 150.0), PageSize::Letter, Margin::None);
        assert_eq!((tall.page_w, tall.page_h), LETTER_PT);
        assert!((tall.h - LETTER_PT.1).abs() < 1e-6, "fills the height");
    }

    #[test]
    fn orientation_matrices_put_stored_top_left_in_the_right_corner() {
        // Stored top-left is (0, 1) in PDF image space.
        let corner = |o| {
            let m = placement_matrix(o, 0.0, 0.0, 10.0, 20.0);
            (m[2] + m[4], m[3] + m[5])
        };
        assert_eq!(corner(1), (0.0, 20.0)); // top-left
        assert_eq!(corner(3), (10.0, 0.0)); // rotated 180: bottom-right
        assert_eq!(corner(6), (10.0, 20.0)); // rotated 90° CW: top-right
        assert_eq!(corner(8), (0.0, 0.0)); // rotated 90° CCW: bottom-left
    }

    #[test]
    fn merge_keeps_page_order_and_inherited_media_box() {
        let dir = temp_dir("merge");
        let img = dir.join("p.jpg");
        std::fs::write(&img, jpeg(150, 300)).unwrap();
        let mut step = no_step();
        let one = dir.join("one.pdf");
        images_to_pdf(
            std::slice::from_ref(&img),
            PageSize::Fit,
            Margin::None,
            1,
            &ok_step,
        )
        .unwrap()
        .save(&one)
        .unwrap();

        // A PDF whose pages inherit MediaBox from the page tree.
        let two = dir.join("two.pdf");
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = doc.add_object(Stream::new(dictionary! {}, b"".to_vec()));
        let kids: Vec<Object> = (0..2)
            .map(|_| {
                doc.add_object(
                    dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content },
                )
                .into()
            })
            .collect();
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => 2,
                "MediaBox" => vec![0.into(), 0.into(), 500.into(), 700.into()],
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        doc.save(&two).unwrap();
        let before = (std::fs::read(&one).unwrap(), std::fs::read(&two).unwrap());

        let mut merged = merge_pdfs(&[one.clone(), two.clone()], &mut step).unwrap();
        let merged = reload(&mut merged);
        let pages: Vec<_> = merged.get_pages().into_values().collect();
        assert_eq!(pages.len(), 3);
        assert_eq!(media_box(&merged, pages[0]), (72.0, 144.0));
        assert_eq!(media_box(&merged, pages[1]), (500.0, 700.0));
        assert_eq!(media_box(&merged, pages[2]), (500.0, 700.0));

        assert_eq!(before.0, std::fs::read(&one).unwrap());
        assert_eq!(before.1, std::fs::read(&two).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn damaged_pdf_is_a_friendly_error() {
        let dir = temp_dir("bad");
        let bad = dir.join("bad.pdf");
        std::fs::write(&bad, b"%PDF-1.4 nonsense").unwrap();
        let mut step = no_step();
        assert!(matches!(
            merge_pdfs(&[bad], &mut step),
            Err(AppError::Corrupt { .. })
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancelling_stops_before_the_next_image() {
        let dir = temp_dir("cancel");
        let a = dir.join("a.jpg");
        std::fs::write(&a, jpeg(10, 10)).unwrap();
        let step = |i: usize| {
            if i == 1 {
                Err(AppError::Cancelled)
            } else {
                Ok(())
            }
        };
        let result = images_to_pdf(&[a.clone(), a], PageSize::Fit, Margin::None, 2, &step);
        assert!(matches!(
            result,
            Err(ImageFailed {
                index: 1,
                error: AppError::Cancelled
            })
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn jpeg_colour_profile_becomes_an_icc_colour_space() {
        let dir = temp_dir("icc");
        let path = dir.join("p3.jpg");
        let icc = b"fake-display-p3-profile".to_vec();
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(20, 10, Rgb([250, 30, 30])));
        let mut buf = Vec::new();
        super::super::image::encode(
            &img,
            Some(&icc),
            super::super::image::OutputFormat::Jpg,
            90,
            &mut Cursor::new(&mut buf),
        )
        .unwrap();
        std::fs::write(&path, &buf).unwrap();

        let mut doc = images_to_pdf(&[path], PageSize::Fit, Margin::None, 1, &ok_step).unwrap();
        let doc = reload(&mut doc);
        let page = doc.get_pages().into_values().next().unwrap();
        let image = page_image(&doc, page);
        let space = image.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
        assert_eq!(space[0].as_name().unwrap(), b"ICCBased");
        let mut profile = doc
            .get_object(space[1].as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap()
            .clone();
        let _ = profile.decompress();
        assert_eq!(profile.content, icc);
        assert_eq!(image.content, buf, "still embedded without re-encoding");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn many_images_keep_their_order_and_a_bad_one_is_named() {
        let dir = temp_dir("parallel");
        let paths: Vec<PathBuf> = (0..12)
            .map(|i| {
                let p = dir.join(format!("{i:02}.jpg"));
                std::fs::write(&p, jpeg(100 + i * 10, 50)).unwrap();
                p
            })
            .collect();
        let mut doc = images_to_pdf(&paths, PageSize::Fit, Margin::None, 4, &ok_step).unwrap();
        let doc = reload(&mut doc);
        let widths: Vec<i64> = doc
            .get_pages()
            .into_values()
            .map(|page| {
                page_image(&doc, page)
                    .dict
                    .get(b"Width")
                    .unwrap()
                    .as_i64()
                    .unwrap()
            })
            .collect();
        assert_eq!(widths, (0..12).map(|i| 100 + i * 10).collect::<Vec<_>>());

        let mut with_bad = paths.clone();
        let bad = dir.join("bad.png");
        std::fs::write(&bad, b"not a png").unwrap();
        with_bad.insert(5, bad);
        let failed =
            images_to_pdf(&with_bad, PageSize::Fit, Margin::None, 4, &ok_step).unwrap_err();
        assert_eq!(failed.index, 5);
        assert_eq!(failed.error.code(), "corrupt");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn location_and_camera_info_stay_out_of_the_pdf() {
        let dir = temp_dir("private");
        let photo = dir.join("phone.jpg");
        std::fs::write(&photo, metadata::tests::phone_jpeg()).unwrap();
        let mut doc = images_to_pdf(&[photo], PageSize::Fit, Margin::None, 1, &ok_step).unwrap();
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        for personal in [
            &b"Apple"[..],
            b"2026:10:22",
            b"Sam",
            b"taken at home",
            b"depth map",
        ] {
            assert!(!bytes.windows(personal.len()).any(|w| w == personal));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn heic_pages_are_stored_as_jpeg() {
        let heic = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sideways-gps.heic");
        let mut doc = images_to_pdf(&[heic], PageSize::Fit, Margin::None, 1, &ok_step).unwrap();
        let doc = reload(&mut doc);
        let page = doc.get_pages().into_values().next().unwrap();
        let image = page_image(&doc, page);
        assert_eq!(
            image.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        // Upright: stored 300 × 200, tagged sideways → a 200 × 300 page image.
        assert_eq!(image.dict.get(b"Width").unwrap().as_i64().unwrap(), 200);
    }
}
