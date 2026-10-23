//! Make a PDF smaller: scale down and re-save its pictures (scans, photos) at the chosen level,
//! drop parts nobody sees (unused objects, page thumbnails, editor data, XMP metadata) and pack the
//! rest tightly. Text, vector drawings and fonts are not touched.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage};
use lopdf::{Dictionary, Document, Object, ObjectId, SaveOptions, Stream};
use serde::{Deserialize, Serialize};

use super::pdf_edit;
use crate::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Email, uploads.
    Smallest,
    /// Most uses.
    Balanced,
    /// Printing.
    Print,
}

impl Level {
    /// Pixels per inch a picture keeps at most, measured against the page it's on.
    fn ppi(self) -> f64 {
        match self {
            Level::Smallest => 100.0,
            Level::Balanced => 150.0,
            Level::Print => 220.0,
        }
    }

    fn quality(self) -> u8 {
        match self {
            Level::Smallest => 60,
            Level::Balanced => 75,
            Level::Print => 85,
        }
    }
}

/// A smaller copy is only worth saving when it saves at least this share of the size.
pub const MIN_SAVING: f64 = 0.05;
/// Pictures with more pixels than this are left alone rather than decoded.
const MAX_PIXELS: u64 = 120_000_000;
/// A re-saved picture replaces the original only if it's at least this much smaller.
const PICTURE_MIN_SAVING: f64 = 0.10;

/// The smaller PDF's bytes (not yet compared with the original; see `MIN_SAVING`).
pub fn compress_pdf(path: &Path, level: Level, workers: usize) -> Result<Vec<u8>, AppError> {
    let mut doc = pdf_edit::open(path)?;
    let limits: Vec<(ObjectId, (f64, f64))> =
        picture_limits(&doc, level.ppi()).into_iter().collect();
    let replaced = crate::pool::map(limits.len(), workers, |i| {
        let (id, limit) = limits[i];
        let stream = doc.get_object(id).ok()?.as_stream().ok()?;
        smaller_picture(&doc, stream, limit, level.quality())
    });
    for ((id, _), stream) in limits.iter().zip(replaced) {
        if let Some(Some(stream)) = stream {
            doc.objects.insert(*id, Object::Stream(stream));
        }
    }
    strip_extras(&mut doc);
    doc.prune_objects();
    doc.compress();
    let mut bytes = Vec::new();
    let options = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .build();
    doc.save_with_options(&mut bytes, options)?;
    Ok(bytes)
}

/// For every picture drawn on a page: the most pixels (long side, short side) it may keep, from the
/// largest page it appears on at `ppi`. A picture can't usefully hold more than a full page's worth.
fn picture_limits(doc: &Document, ppi: f64) -> HashMap<ObjectId, (f64, f64)> {
    let mut limits = HashMap::new();
    for page_id in doc.get_pages().into_values() {
        let Ok(page) = doc.get_dictionary(page_id) else {
            continue;
        };
        let (w, h) = page_size(doc, page).unwrap_or((612.0, 792.0)); // Letter if unreadable
        let limit = (w.max(h) / 72.0 * ppi, w.min(h) / 72.0 * ppi);
        let mut seen = HashSet::new();
        collect_pictures(
            doc,
            page.get(b"Resources").ok(),
            limit,
            &mut limits,
            &mut seen,
            0,
        );
    }
    limits
}

fn collect_pictures(
    doc: &Document,
    resources: Option<&Object>,
    limit: (f64, f64),
    out: &mut HashMap<ObjectId, (f64, f64)>,
    seen: &mut HashSet<ObjectId>,
    depth: u32,
) {
    if depth > 8 {
        return;
    }
    let Some(xobjects) = resources
        .and_then(|r| resolve_dict(doc, r))
        .and_then(|r| r.get(b"XObject").ok())
        .and_then(|x| resolve_dict(doc, x))
    else {
        return;
    };
    for (_, value) in xobjects.iter() {
        let Ok(id) = value.as_reference() else {
            continue;
        };
        let Ok(stream) = doc.get_object(id).and_then(Object::as_stream) else {
            continue;
        };
        match stream.dict.get(b"Subtype").and_then(Object::as_name) {
            Ok(b"Image") => {
                let entry = out.entry(id).or_insert((0.0, 0.0));
                entry.0 = entry.0.max(limit.0);
                entry.1 = entry.1.max(limit.1);
            }
            Ok(b"Form") if seen.insert(id) => {
                let inner = stream.dict.get(b"Resources").ok();
                collect_pictures(doc, inner, limit, out, seen, depth + 1);
            }
            _ => {}
        }
    }
}

fn resolve<'a>(doc: &'a Document, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(object),
        other => other,
    }
}

fn resolve_dict<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Dictionary> {
    resolve(doc, object).as_dict().ok()
}

fn number(doc: &Document, object: &Object) -> Option<f64> {
    match resolve(doc, object) {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

/// Page size in points from its MediaBox.
fn page_size(doc: &Document, page: &Dictionary) -> Option<(f64, f64)> {
    let mb = resolve(doc, page.get(b"MediaBox").ok()?).as_array().ok()?;
    let v: Vec<f64> = mb.iter().filter_map(|o| number(doc, o)).collect();
    let [x0, y0, x1, y1] = v[..] else { return None };
    Some(((x1 - x0).abs(), (y1 - y0).abs())).filter(|(w, h)| *w > 0.0 && *h > 0.0)
}

/// Components of an 8-bit picture's colour space we can re-save as JPEG: gray (1) or RGB (3).
fn components(doc: &Document, space: &Object) -> Option<u8> {
    match resolve(doc, space) {
        Object::Name(n) if n == b"DeviceGray" => Some(1),
        Object::Name(n) if n == b"DeviceRGB" => Some(3),
        Object::Array(parts)
            if parts.first().and_then(|p| p.as_name().ok()) == Some(b"ICCBased") =>
        {
            let profile = resolve(doc, parts.get(1)?).as_stream().ok()?;
            match profile.dict.get(b"N").and_then(Object::as_i64).ok()? {
                1 => Some(1),
                3 => Some(3),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The picture re-saved as a JPEG of at most `limit` pixels, or None when it's better left as it
/// is: masks, inverted or unusual colour, 16-bit, JPEG 2000 / fax / JBIG2 encodings, or no saving.
fn smaller_picture(
    doc: &Document,
    stream: &Stream,
    limit: (f64, f64),
    quality: u8,
) -> Option<Stream> {
    let dict = &stream.dict;
    if dict
        .get(b"ImageMask")
        .and_then(Object::as_bool)
        .unwrap_or(false)
        || dict.has(b"Mask")
        || dict.has(b"Decode")
        || dict
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .ok()?
            != 8
    {
        return None;
    }
    let width = u32::try_from(dict.get(b"Width").and_then(Object::as_i64).ok()?).ok()?;
    let height = u32::try_from(dict.get(b"Height").and_then(Object::as_i64).ok()?).ok()?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }
    let n = components(doc, dict.get(b"ColorSpace").ok()?)?;
    let filters: Vec<&[u8]> = match dict.get(b"Filter") {
        Err(_) => Vec::new(),
        Ok(Object::Name(name)) => vec![name.as_slice()],
        Ok(Object::Array(items)) => items.iter().filter_map(|i| i.as_name().ok()).collect(),
        Ok(_) => return None,
    };

    let image = match filters.as_slice() {
        [b"DCTDecode"] => {
            let decoded =
                image::load_from_memory_with_format(&stream.content, ImageFormat::Jpeg).ok()?;
            match (n, decoded) {
                (1, DynamicImage::ImageLuma8(img)) => DynamicImage::ImageLuma8(img),
                (3, DynamicImage::ImageRgb8(img)) => DynamicImage::ImageRgb8(img),
                _ => return None,
            }
        }
        [] | [b"FlateDecode"] | [b"LZWDecode"] => {
            let expected = width as usize * height as usize * usize::from(n);
            // Bounded: a "zip bomb" picture can't make us unpack more than it claims to hold.
            let raw = if filters.is_empty() {
                stream.content.clone()
            } else {
                stream
                    .decompressed_content_with_limit(expected + 4096)
                    .ok()?
            };
            if raw.len() < expected {
                return None;
            }
            let raw = raw[..expected].to_vec();
            match n {
                1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw)?),
                _ => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, raw)?),
            }
        }
        _ => return None,
    };
    if image.width() != width || image.height() != height {
        return None;
    }

    let (long, short) = (width.max(height) as f64, width.min(height) as f64);
    let scale = (limit.0 / long).min(limit.1 / short).min(1.0);
    let image = if scale < 1.0 {
        let w = ((width as f64 * scale).round() as u32).max(1);
        let h = ((height as f64 * scale).round() as u32).max(1);
        image.resize_exact(w, h, FilterType::Lanczos3)
    } else {
        image
    };
    let mut bytes = Vec::new();
    // Through `write_with_encoder` so a gray picture stays a one-channel JPEG (matching its
    // DeviceGray colour space); `encode_image` would see every DynamicImage as RGBA.
    image
        .write_with_encoder(JpegEncoder::new_with_quality(
            Cursor::new(&mut bytes),
            quality,
        ))
        .ok()?;
    if bytes.len() as f64 > stream.content.len() as f64 * (1.0 - PICTURE_MIN_SAVING) {
        return None;
    }

    let mut new_dict = dict.clone();
    new_dict.set("Width", i64::from(image.width()));
    new_dict.set("Height", i64::from(image.height()));
    new_dict.set("Filter", "DCTDecode");
    new_dict.remove(b"DecodeParms");
    let mut smaller = Stream::new(new_dict, bytes);
    smaller.allows_compression = false;
    Some(smaller)
}

/// Remove what only editing apps or file browsers use: XMP metadata, app-private data, embedded
/// page thumbnails.
fn strip_extras(doc: &mut Document) {
    if let Ok(root) = doc.trailer.get(b"Root").and_then(Object::as_reference) {
        if let Ok(catalog) = doc.get_dictionary_mut(root) {
            catalog.remove(b"Metadata");
            catalog.remove(b"PieceInfo");
        }
    }
    for id in doc.get_pages().into_values() {
        if let Ok(page) = doc.get_dictionary_mut(id) {
            page.remove(b"Thumb");
            page.remove(b"PieceInfo");
            page.remove(b"Metadata");
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::convert::pdf::{images_to_pdf, Margin, PageSize};
    use crate::convert::pdf_edit::tests::temp_dir;
    use image::Rgb;
    use lopdf::dictionary;
    use std::path::PathBuf;

    /// A detailed photo-like JPEG, so re-saving it has something to save.
    fn photo(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| {
            Rgb([
                ((x * 7 + y * 3) % 256) as u8,
                ((x * y / 97) % 256) as u8,
                ((x ^ y) % 256) as u8,
            ])
        });
        let mut buf = Vec::new();
        JpegEncoder::new_with_quality(Cursor::new(&mut buf), 95)
            .encode_image(&img)
            .unwrap();
        buf
    }

    /// A4 pages each holding a 300-DPI photo-like scan.
    pub(crate) fn scan_pdf(dir: &Path, pages: u32) -> PathBuf {
        let images: Vec<PathBuf> = (0..pages)
            .map(|i| {
                let p = dir.join(format!("scan{i}.jpg"));
                std::fs::write(&p, photo(2480, 3508)).unwrap(); // A4 at 300 DPI
                p
            })
            .collect();
        let ok = |_: usize| Ok(());
        let mut doc = images_to_pdf(&images, PageSize::A4, Margin::None, 2, &ok, &[]).unwrap();
        let path = dir.join("scan.pdf");
        doc.save(&path).unwrap();
        path
    }

    fn picture_sizes(bytes: &[u8]) -> Vec<(i64, i64)> {
        let doc = Document::load_mem(bytes).unwrap();
        let mut sizes: Vec<(i64, i64)> = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
            .map(|s| {
                (
                    s.dict.get(b"Width").unwrap().as_i64().unwrap(),
                    s.dict.get(b"Height").unwrap().as_i64().unwrap(),
                )
            })
            .collect();
        sizes.sort();
        sizes
    }

    #[test]
    fn scans_shrink_to_the_level_and_pages_stay() {
        let dir = temp_dir("compress");
        let path = scan_pdf(&dir, 2);
        let original = std::fs::read(&path).unwrap();
        let before = original.len();

        let balanced = compress_pdf(&path, Level::Balanced, 2).unwrap();
        assert!(
            balanced.len() * 3 < before,
            "{} → {}",
            before,
            balanced.len()
        );
        // A4 at 150 per inch: 1240 × 1754.
        assert_eq!(picture_sizes(&balanced), [(1240, 1754), (1240, 1754)]);
        assert_eq!(Document::load_mem(&balanced).unwrap().get_pages().len(), 2);

        let smallest = compress_pdf(&path, Level::Smallest, 2).unwrap();
        let print = compress_pdf(&path, Level::Print, 2).unwrap();
        assert!(smallest.len() < balanced.len() && balanced.len() < print.len());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "original untouched"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// One Letter page of plain text: nothing for Make smaller to shrink.
    pub(crate) fn text_pdf(dir: &Path) -> PathBuf {
        let mut doc = Document::with_version("1.5");
        let font = doc.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        let content = doc.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 72 720 Td (Hello from QuickConvert) Tj ET".to_vec(),
        ));
        let pages = doc.new_object_id();
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages, "Contents" => content,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(
                dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
            ),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let path = dir.join("letter.pdf");
        doc.save(&path).unwrap();
        path
    }

    #[test]
    fn text_only_pdf_gains_little_and_keeps_its_text() {
        let dir = temp_dir("text");
        let path = text_pdf(&dir);
        let out = compress_pdf(&path, Level::Smallest, 2).unwrap();
        let back = Document::load_mem(&out).unwrap();
        let page = back.get_pages()[&1];
        let text = back.get_page_content(page);
        assert!(String::from_utf8_lossy(&text).contains("Hello from QuickConvert"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_picture_that_would_unpack_huge_is_left_alone() {
        let dir = temp_dir("bomb");
        let path = scan_pdf(&dir, 1);
        let mut doc = Document::load(&path).unwrap();
        // Claims 100 × 100 gray but unpacks to 20 MB.
        let mut bomb = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 100, "Height" => 100,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            vec![0u8; 20_000_000],
        );
        bomb.compress().unwrap();
        let bomb_id = doc.add_object(bomb);
        let page = doc.get_pages()[&1];
        let (resources, _) = doc.get_page_resources(page).unwrap();
        let mut resources = resources.unwrap().clone();
        let mut xobjects = resources
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .clone();
        xobjects.set("Bomb", bomb_id);
        resources.set("XObject", xobjects);
        doc.get_dictionary_mut(page)
            .unwrap()
            .set("Resources", resources);
        doc.save(&path).unwrap();

        let out = compress_pdf(&path, Level::Balanced, 2).unwrap();
        assert!(
            picture_sizes(&out).contains(&(100, 100)),
            "bomb kept as it was"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gray_scans_stay_gray() {
        let dir = temp_dir("gray");
        // Scan-like: paper grain everywhere, which lossless encoding can't squeeze.
        let gray = GrayImage::from_fn(2480, 3508, |x, y| {
            let mut h = x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663);
            h ^= h >> 13;
            h = h.wrapping_mul(0x5bd1_e995);
            h ^= h >> 15;
            image::Luma([180 + (h % 60) as u8])
        });
        let p = dir.join("gray.png");
        DynamicImage::ImageLuma8(gray).save(&p).unwrap();
        let ok = |_: usize| Ok(());
        // Combine stores PNGs losslessly as RGB; rewrite it as a gray Flate picture.
        let mut doc = images_to_pdf(&[p], PageSize::A4, Margin::None, 1, &ok, &[]).unwrap();
        for object in doc.objects.values_mut() {
            if let Ok(stream) = object.as_stream_mut() {
                if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") {
                    let rgb = stream.decompressed_content().unwrap();
                    let gray: Vec<u8> = rgb.chunks(3).map(|c| c[0]).collect();
                    stream.dict.set("ColorSpace", "DeviceGray");
                    stream.dict.remove(b"Filter");
                    stream.set_content(gray);
                    stream.compress().unwrap();
                }
            }
        }
        let path = dir.join("gray.pdf");
        doc.save(&path).unwrap();

        let out = compress_pdf(&path, Level::Balanced, 1).unwrap();
        let back = Document::load_mem(&out).unwrap();
        let picture = back
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .find(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
            .unwrap();
        assert_eq!(
            picture.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
            b"DeviceGray"
        );
        assert_eq!(
            picture.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        let decoded =
            image::load_from_memory_with_format(&picture.content, ImageFormat::Jpeg).unwrap();
        assert!(matches!(decoded, DynamicImage::ImageLuma8(_)));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
