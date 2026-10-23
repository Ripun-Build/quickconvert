//! Remove location, camera, date and other personal data without touching the picture. JPEG, PNG
//! and WebP are cleaned by dropping their metadata blocks, so the image data stays byte-for-byte the
//! same. A "turn to view" orientation tag survives as a minimal EXIF block holding nothing else, so
//! sideways phone photos still show upright.

use serde::Serialize;

use crate::errors::AppError;

/// What was found (and removed), for the Result view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Found {
    pub location: bool,
    pub camera: bool,
    pub date: bool,
    /// Editing software, descriptions, XMP/IPTC blocks, comments…
    pub other: bool,
}

impl Found {
    fn add(&mut self, more: Found) {
        self.location |= more.location;
        self.camera |= more.camera;
        self.date |= more.date;
        self.other |= more.other;
    }
}

pub struct Cleaned {
    pub bytes: Vec<u8>,
    pub found: Found,
}

/// Clean a JPEG, PNG or WebP file's bytes; None for other formats (the caller re-saves those).
pub fn clean(bytes: &[u8]) -> Option<Result<Cleaned, AppError>> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        Some(clean_jpeg(bytes, true))
    } else if bytes.starts_with(PNG_SIGNATURE) {
        Some(clean_png(bytes))
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(clean_webp(bytes))
    } else {
        None
    }
}

fn damaged(detail: &str) -> AppError {
    AppError::Corrupt {
        detail: detail.to_string(),
    }
}

// ---- JPEG ----

/// Keep the image data, quantisation/Huffman tables, JFIF header, colour profile and Adobe colour
/// marker; drop EXIF, XMP, IPTC, comments and anything after the end of the main image (phones put
/// depth and HDR maps there, each with its own copy of the metadata). With `keep_orientation`, a
/// sideways photo gets a minimal EXIF block with only its orientation.
pub fn clean_jpeg(data: &[u8], keep_orientation: bool) -> Result<Cleaned, AppError> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Err(damaged("not a JPEG"));
    }
    let mut found = Found::default();
    let mut orientation = 1;
    let mut head: Vec<&[u8]> = Vec::new(); // JFIF APP0, written before the orientation block
    let mut body: Vec<&[u8]> = Vec::new();
    let mut i = 2;
    loop {
        if i + 1 >= data.len() || data[i] != 0xFF {
            return Err(damaged("JPEG ended early"));
        }
        let marker = data[i + 1];
        match marker {
            0xFF => {
                i += 1; // fill byte
                continue;
            }
            0xD9 => {
                body.push(&data[i..i + 2]);
                break;
            }
            0x01 | 0xD0..=0xD7 => {
                body.push(&data[i..i + 2]);
                i += 2;
                continue;
            }
            _ => {}
        }
        let len = data
            .get(i + 2..i + 4)
            .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])))
            .filter(|&l| l >= 2)
            .ok_or_else(|| damaged("bad JPEG segment"))?;
        let end = i + 2 + len;
        let segment = data
            .get(i..end)
            .ok_or_else(|| damaged("JPEG segment too long"))?;
        let payload = &segment[4..];
        match marker {
            0xDA => {
                // Start of scan: copy the entropy-coded data up to the next real marker.
                let mut j = end;
                while j + 1 < data.len() {
                    if data[j] == 0xFF
                        && data[j + 1] != 0x00
                        && !(0xD0..=0xD7).contains(&data[j + 1])
                    {
                        break;
                    }
                    j += 1;
                }
                body.push(&data[i..j]);
                i = j;
                continue;
            }
            0xE0 if payload.starts_with(b"JFIF\0") && head.is_empty() && body.is_empty() => {
                head.push(segment);
            }
            0xE1 if payload.starts_with(b"Exif\0\0") => {
                let (f, o) = read_tiff(&payload[6..]);
                found.add(f);
                if o != 1 {
                    orientation = o;
                }
            }
            0xE2 if payload.starts_with(b"ICC_PROFILE\0") => body.push(segment),
            // Multi-picture index (points at the extra images dropped after EOI): not personal.
            0xE2 if payload.starts_with(b"MPF\0") => {}
            0xEE if payload.starts_with(b"Adobe") => body.push(segment),
            0xE0..=0xEF | 0xFE => found.other = true,
            _ => body.push(segment),
        }
        i = end;
    }

    let mut bytes = Vec::with_capacity(data.len());
    bytes.extend_from_slice(&[0xFF, 0xD8]);
    for part in head {
        bytes.extend_from_slice(part);
    }
    if keep_orientation && orientation != 1 {
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend(orientation_tiff(orientation));
        bytes.extend_from_slice(&[0xFF, 0xE1]);
        bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        bytes.extend(exif);
    }
    for part in body {
        bytes.extend_from_slice(part);
    }
    Ok(Cleaned { bytes, found })
}

// ---- EXIF / TIFF ----

/// A big-endian TIFF block holding only an orientation tag (EXIF orientation 2–8).
fn orientation_tiff(orientation: u16) -> Vec<u8> {
    let mut t = b"MM\0\x2A\0\0\0\x08".to_vec(); // header, first IFD at offset 8
    t.extend(1u16.to_be_bytes()); // one entry
    t.extend(0x0112u16.to_be_bytes()); // Orientation
    t.extend(3u16.to_be_bytes()); // SHORT
    t.extend(1u32.to_be_bytes()); // count
    t.extend(orientation.to_be_bytes());
    t.extend([0, 0]); // value padding
    t.extend(0u32.to_be_bytes()); // no next IFD
    t
}

struct Tiff<'a> {
    data: &'a [u8],
    little: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.data.get(at..at + 2)?;
        Some(if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.data.get(at..at + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }

    /// (tag, offset of the entry) for every entry of the IFD at `at`.
    fn entries(&self, at: usize) -> Vec<(u16, usize)> {
        let count = self.u16(at).unwrap_or(0) as usize;
        (0..count.min(512))
            .map(|n| at + 2 + n * 12)
            .filter_map(|e| Some((self.u16(e)?, e)))
            .collect()
    }
}

/// What a TIFF block (EXIF payload, or a whole TIFF file) contains, and its orientation (1 = none).
/// Unreadable data counts as "other": it is dropped either way.
pub fn read_tiff(data: &[u8]) -> (Found, u16) {
    let mut found = Found::default();
    let little = match data.get(0..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => {
            found.other = !data.is_empty();
            return (found, 1);
        }
    };
    let tiff = Tiff { data, little };
    let Some(ifd0) = tiff.u32(4) else {
        found.other = true;
        return (found, 1);
    };
    let mut orientation = 1;
    for (tag, entry) in tiff.entries(ifd0 as usize) {
        match tag {
            0x0112 => {
                orientation = tiff
                    .u16(entry + 8)
                    .filter(|o| (1..=8).contains(o))
                    .unwrap_or(1)
            }
            0x010F | 0x0110 => found.camera = true, // Make, Model
            0x0132 => found.date = true,            // DateTime
            0x010E | 0x0131 | 0x013B | 0x8298 | 0x83BB | 0x02BC => found.other = true, // description, software, artist, copyright, IPTC, XMP
            0x8825 => {
                // GPS block: location only if it holds anything.
                let gps = tiff.u32(entry + 8).unwrap_or(0) as usize;
                found.location |= !tiff.entries(gps).is_empty();
            }
            0x8769 => {
                for (tag, _) in tiff.entries(tiff.u32(entry + 8).unwrap_or(0) as usize) {
                    match tag {
                        0x9003 | 0x9004 | 0x9010 | 0x9011 | 0x9012 | 0x9290..=0x9292 => {
                            found.date = true // when it was taken / digitised, and time zones
                        }
                        0x9286 | 0xA420 | 0xA430 => found.other = true, // comment, unique ID, owner
                        // Technical: versions, colour space, pixel size, interoperability.
                        0x9000 | 0x9101 | 0xA000..=0xA003 | 0xA005 => {}
                        _ => found.camera = true, // exposure, lens, serial number, maker notes…
                    }
                }
            }
            _ => {}
        }
    }
    (found, orientation)
}

// ---- PNG ----

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Ancillary chunks that describe how to show the picture, not who/where/when.
const PNG_KEEP: [&[u8; 4]; 15] = [
    b"tRNS", b"cHRM", b"gAMA", b"iCCP", b"sBIT", b"sRGB", b"cICP", b"mDCV", b"cLLI", b"pHYs",
    b"bKGD", b"hIST", b"acTL", b"fcTL", b"fdAT",
];

pub fn clean_png(data: &[u8]) -> Result<Cleaned, AppError> {
    let mut found = Found::default();
    let mut orientation = 1;
    let mut chunks: Vec<(&[u8], &[u8])> = Vec::new(); // (type, data)
    let mut i = PNG_SIGNATURE.len();
    while i + 8 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        let kind = &data[i + 4..i + 8];
        let body = data
            .get(i + 8..i + 8 + len)
            .ok_or_else(|| damaged("PNG chunk too long"))?;
        i += 12 + len;
        let critical = kind[0].is_ascii_uppercase();
        if critical || PNG_KEEP.iter().any(|k| &k[..] == kind) {
            chunks.push((kind, body));
            if kind == b"IEND" {
                break;
            }
            continue;
        }
        match kind {
            b"eXIf" => {
                let (f, o) = read_tiff(body);
                found.add(f);
                orientation = o;
            }
            b"tIME" => found.date = true,
            b"tEXt" | b"zTXt" | b"iTXt" => {
                if body.starts_with(b"Creation Time\0") {
                    found.date = true;
                } else {
                    found.other = true; // XMP, author, software, comments…
                }
            }
            _ => found.other = true,
        }
    }

    let mut bytes = PNG_SIGNATURE.to_vec();
    let orientation_chunk = (orientation != 1).then(|| orientation_tiff(orientation));
    let mut wrote_orientation = false;
    for (kind, body) in chunks {
        if kind == b"IDAT" && !wrote_orientation {
            if let Some(exif) = &orientation_chunk {
                write_png_chunk(&mut bytes, b"eXIf", exif); // must come before the image data
            }
            wrote_orientation = true;
        }
        write_png_chunk(&mut bytes, kind, body);
    }
    Ok(Cleaned { bytes, found })
}

fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8], body: &[u8]) {
    out.extend((body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ---- WebP ----

const WEBP_EXIF_FLAG: u8 = 0x08;
const WEBP_XMP_FLAG: u8 = 0x04;

pub fn clean_webp(data: &[u8]) -> Result<Cleaned, AppError> {
    let mut found = Found::default();
    let mut orientation = 1;
    let mut chunks: Vec<(&[u8], &[u8])> = Vec::new();
    let mut i = 12;
    while i + 8 <= data.len() {
        let kind = &data[i..i + 4];
        let len = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
        let body = data
            .get(i + 8..i + 8 + len)
            .ok_or_else(|| damaged("WebP chunk too long"))?;
        i += 8 + len + len % 2;
        match kind {
            b"EXIF" => {
                let tiff = body.strip_prefix(b"Exif\0\0").unwrap_or(body);
                let (f, o) = read_tiff(tiff);
                found.add(f);
                orientation = o;
            }
            b"XMP " => found.other = true,
            b"VP8X" | b"VP8 " | b"VP8L" | b"ALPH" | b"ANIM" | b"ANMF" | b"ICCP" => {
                chunks.push((kind, body))
            }
            _ => found.other = true,
        }
    }
    // Simple WebP (no VP8X header) can't carry metadata at all.
    if chunks.first().map(|c| c.0) != Some(&b"VP8X"[..]) {
        return Ok(Cleaned {
            bytes: data.to_vec(),
            found,
        });
    }

    let exif = (orientation != 1).then(|| orientation_tiff(orientation));
    let mut payload = b"WEBP".to_vec();
    for (kind, body) in chunks {
        if kind == b"VP8X" {
            let mut header = body.to_vec();
            if let Some(flags) = header.first_mut() {
                *flags &= !(WEBP_EXIF_FLAG | WEBP_XMP_FLAG);
                if exif.is_some() {
                    *flags |= WEBP_EXIF_FLAG;
                }
            }
            write_riff_chunk(&mut payload, kind, &header);
        } else {
            write_riff_chunk(&mut payload, kind, body);
        }
    }
    if let Some(exif) = exif {
        write_riff_chunk(&mut payload, b"EXIF", &exif); // after the image data, as the spec asks
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    Ok(Cleaned { bytes, found })
}

fn write_riff_chunk(out: &mut Vec<u8>, kind: &[u8], body: &[u8]) {
    out.extend_from_slice(kind);
    out.extend((body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::convert::image::{encode, OutputFormat};
    use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage};
    use std::io::Cursor;

    /// Big-endian EXIF: Make "Apple", orientation 6, Exif block with the date taken, GPS latitude.
    pub(crate) fn exif_tiff() -> Vec<u8> {
        let mut t = b"MM\0\x2A\0\0\0\x08".to_vec();
        let entry = |t: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: u32| {
            t.extend(tag.to_be_bytes());
            t.extend(kind.to_be_bytes());
            t.extend(count.to_be_bytes());
            t.extend(value.to_be_bytes());
        };
        t.extend(4u16.to_be_bytes());
        entry(&mut t, 0x010F, 2, 6, 98); // Make
        entry(&mut t, 0x0112, 3, 1, 6 << 16); // Orientation 6
        entry(&mut t, 0x8769, 4, 1, 62); // Exif IFD
        entry(&mut t, 0x8825, 4, 1, 80); // GPS IFD
        t.extend(0u32.to_be_bytes());
        assert_eq!(t.len(), 62);
        t.extend(1u16.to_be_bytes());
        entry(&mut t, 0x9003, 2, 20, 104); // DateTimeOriginal
        t.extend(0u32.to_be_bytes());
        t.extend(1u16.to_be_bytes());
        entry(&mut t, 0x0001, 2, 2, u32::from_be_bytes(*b"N\0\0\0")); // GPSLatitudeRef
        t.extend(0u32.to_be_bytes());
        assert_eq!(t.len(), 98);
        t.extend(b"Apple\0");
        t.extend(b"2026:10:22 10:00:00\0");
        t
    }

    fn picture() -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(48, 32, |x, y| {
            Rgb([(x * 5) as u8, (y * 7) as u8, ((x + y) * 3) as u8])
        }))
    }

    const ICC: &[u8] = b"a-colour-profile-payload";

    fn encoded(format: OutputFormat) -> Vec<u8> {
        let mut buf = Vec::new();
        encode(
            &picture(),
            Some(ICC),
            format,
            90,
            &mut Cursor::new(&mut buf),
        )
        .unwrap();
        buf
    }

    fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut s = vec![0xFF, marker];
        s.extend(((payload.len() + 2) as u16).to_be_bytes());
        s.extend_from_slice(payload);
        s
    }

    /// A phone-like JPEG: EXIF with GPS, XMP, a comment, and a second image after the end marker.
    pub(crate) fn phone_jpeg() -> Vec<u8> {
        let plain = encoded(OutputFormat::Jpg);
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend(exif_tiff());
        let mut out = plain[..2].to_vec();
        out.extend(segment(0xE1, &exif));
        out.extend(segment(
            0xE1,
            b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>Sam's phone</x:xmpmeta>",
        ));
        out.extend(segment(0xFE, b"taken at home"));
        out.extend_from_slice(&plain[2..]);
        out.extend_from_slice(b"\xFF\xD8\xFF\xE1 depth map with its own iPhone metadata");
        out
    }

    fn raw_pixels(bytes: &[u8], format: ImageFormat) -> Vec<u8> {
        image::load_from_memory_with_format(bytes, format)
            .unwrap()
            .to_rgb8()
            .into_raw()
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    fn all_found() -> Found {
        Found {
            location: true,
            camera: true,
            date: true,
            other: true,
        }
    }

    #[test]
    fn jpeg_loses_personal_data_but_not_a_single_pixel() {
        let original = phone_jpeg();
        let cleaned = clean(&original).unwrap().unwrap();
        assert_eq!(cleaned.found, all_found());
        for personal in [
            &b"Apple"[..],
            b"2026:10:22",
            b"Sam",
            b"taken at home",
            b"depth map",
        ] {
            assert!(
                !contains(&cleaned.bytes, personal),
                "{}",
                String::from_utf8_lossy(personal)
            );
        }
        assert_eq!(
            raw_pixels(&cleaned.bytes, ImageFormat::Jpeg),
            raw_pixels(&original, ImageFormat::Jpeg)
        );
        let mut decoder = ImageReader::new(Cursor::new(&cleaned.bytes))
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        assert_eq!(decoder.icc_profile().unwrap().as_deref(), Some(ICC));
        assert_eq!(
            decoder.orientation().unwrap().to_exif(),
            6,
            "still shows upright"
        );
    }

    #[test]
    fn jpeg_for_pdfs_drops_orientation_too() {
        let cleaned = clean_jpeg(&phone_jpeg(), false).unwrap();
        assert!(!contains(&cleaned.bytes, b"Exif\0\0"));
        assert!(cleaned.bytes.ends_with(&[0xFF, 0xD9]));
    }

    #[test]
    fn clean_jpeg_reports_nothing_found() {
        let cleaned = clean(&encoded(OutputFormat::Jpg)).unwrap().unwrap();
        assert_eq!(cleaned.found, Found::default());
    }

    fn png_chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        write_png_chunk(&mut out, kind, body);
        out
    }

    #[test]
    fn png_keeps_colour_and_pixels_and_drops_text_time_and_exif() {
        let plain = encoded(OutputFormat::Png);
        // Insert after IHDR (8 signature + 25 IHDR bytes).
        let mut original = plain[..33].to_vec();
        original.extend(png_chunk(b"eXIf", &exif_tiff()));
        original.extend(png_chunk(b"tEXt", b"Software\0PhotoEditor 9"));
        original.extend(png_chunk(b"tIME", &[7, 234, 10, 22, 10, 0, 0]));
        original.extend_from_slice(&plain[33..]);

        let cleaned = clean(&original).unwrap().unwrap();
        assert_eq!(cleaned.found, all_found());
        assert!(!contains(&cleaned.bytes, b"Apple"));
        assert!(!contains(&cleaned.bytes, b"PhotoEditor"));
        assert!(!contains(&cleaned.bytes, b"tIME"));
        assert_eq!(
            raw_pixels(&cleaned.bytes, ImageFormat::Png),
            raw_pixels(&plain, ImageFormat::Png)
        );
        let at = cleaned.bytes.windows(4).position(|w| w == b"eXIf").unwrap();
        let idat = cleaned.bytes.windows(4).position(|w| w == b"IDAT").unwrap();
        assert!(at < idat, "orientation comes before the image data");
        let len = u32::from_be_bytes(cleaned.bytes[at - 4..at].try_into().unwrap()) as usize;
        assert_eq!(
            read_tiff(&cleaned.bytes[at + 4..at + 4 + len]),
            (Found::default(), 6)
        );
        let mut decoder = ImageReader::new(Cursor::new(&cleaned.bytes))
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        assert_eq!(decoder.icc_profile().unwrap().as_deref(), Some(ICC));
    }

    #[test]
    fn webp_drops_exif_and_xmp_and_fixes_its_header() {
        let plain = encoded(OutputFormat::Webp); // extended format: VP8X + ICCP + image
        let mut payload = plain[12..].to_vec();
        payload[8] |= WEBP_EXIF_FLAG | WEBP_XMP_FLAG;
        write_riff_chunk(&mut payload, b"EXIF", &exif_tiff());
        write_riff_chunk(&mut payload, b"XMP ", b"<x:xmpmeta>Sam</x:xmpmeta>");
        let mut original = b"RIFF".to_vec();
        original.extend(((payload.len() + 4) as u32).to_le_bytes());
        original.extend(b"WEBP");
        original.extend(payload);

        let cleaned = clean(&original).unwrap().unwrap();
        assert_eq!(cleaned.found, all_found());
        assert!(!contains(&cleaned.bytes, b"Apple"));
        assert!(!contains(&cleaned.bytes, b"Sam"));
        assert_eq!(cleaned.bytes[20] & WEBP_XMP_FLAG, 0);
        assert_ne!(cleaned.bytes[20] & WEBP_EXIF_FLAG, 0, "orientation kept");
        let riff = u32::from_le_bytes(cleaned.bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, cleaned.bytes.len());
        assert_eq!(
            raw_pixels(&cleaned.bytes, ImageFormat::WebP),
            raw_pixels(&plain, ImageFormat::WebP)
        );
    }

    #[test]
    fn exif_reader_survives_garbage() {
        assert_eq!(read_tiff(b"nonsense").1, 1);
        assert!(read_tiff(b"nonsense").0.other);
        assert_eq!(read_tiff(b"MM\0*\0\0\xFF\xFF").1, 1);
        let (found, orientation) = read_tiff(&exif_tiff());
        assert_eq!(
            found,
            Found {
                other: false,
                ..all_found()
            }
        );
        assert_eq!(orientation, 6);
    }

    #[test]
    fn technical_exif_tags_are_not_personal() {
        // IFD0 → Exif IFD holding only colour space and pixel size (what encoders write).
        let mut t = b"MM\0\x2A\0\0\0\x08".to_vec();
        t.extend(1u16.to_be_bytes());
        t.extend([0x87, 0x69, 0, 4, 0, 0, 0, 1, 0, 0, 0, 26]);
        t.extend(0u32.to_be_bytes());
        t.extend(2u16.to_be_bytes());
        t.extend([0xA0, 0x01, 0, 3, 0, 0, 0, 1, 0, 1, 0, 0]);
        t.extend([0xA0, 0x02, 0, 4, 0, 0, 0, 1, 0, 0, 0, 200]);
        t.extend(0u32.to_be_bytes());
        assert_eq!(read_tiff(&t), (Found::default(), 1));
    }

    #[test]
    fn crc_matches_the_png_reference() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }
}
