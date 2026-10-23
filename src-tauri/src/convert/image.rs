//! Image format conversion and resize.

use std::io::{Cursor, Seek, Write};
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Rgb, RgbImage};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Quality used when an operation keeps the input format (resize).
pub const KEEP_FORMAT_QUALITY: u8 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Jpg,
    Png,
    Webp,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Jpg => "jpg",
            OutputFormat::Png => "png",
            OutputFormat::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ResizeMode {
    /// Longer side becomes `px`; smaller images are left as they are.
    LongEdge {
        px: u32,
    },
    /// Fit within `width`×`height` (keep_aspect) or stretch to exactly that size.
    Exact {
        width: u32,
        height: u32,
        keep_aspect: bool,
    },
    Percent {
        percent: f32,
    },
}

fn corrupt(e: image::ImageError) -> AppError {
    match e {
        image::ImageError::IoError(io) => AppError::Io(io),
        image::ImageError::Unsupported(u) => AppError::Unsupported {
            extension: u.to_string(),
        },
        other => AppError::Corrupt {
            detail: other.to_string(),
        },
    }
}

/// Decoded pixels plus the colour profile they're in. iPhone photos are Display P3: dropping the
/// profile makes viewers read them as sRGB and the colours look washed out.
pub struct Picture {
    pub image: DynamicImage,
    pub icc: Option<Vec<u8>>,
    /// HEIC originals: file bytes per pixel, so a JPG made from it can aim for a similar size.
    pub density: Option<f64>,
}

/// Like `load`, but HEIC is first scaled by the system decoder to about `max` px on its long side:
/// far quicker for thumbnails and checks. Other formats load in full (callers scale them).
pub fn load_small(path: &Path, max: u32) -> Result<Picture, AppError> {
    if crate::heic::is_heic(path) {
        let small = crate::heic::decode_scaled(path, Some(max))?;
        return load(small.path());
    }
    load(path)
}

/// Decode an image, keep its ICC profile, and apply its EXIF orientation (phone photos upright).
pub fn load(path: &Path) -> Result<Picture, AppError> {
    if crate::heic::is_heic(path) {
        let bytes = std::fs::metadata(path)?.len();
        let readable = crate::heic::decode(path)?;
        let mut picture = load(readable.path())?;
        let pixels = f64::from(picture.image.width()) * f64::from(picture.image.height());
        picture.density = (pixels > 0.0).then(|| bytes as f64 / pixels);
        return Ok(picture);
    }
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()
        .map_err(corrupt)?;
    let orientation = decoder.orientation().map_err(corrupt)?;
    let icc = decoder
        .icc_profile()
        .ok()
        .flatten()
        .filter(|p| !p.is_empty());
    let mut image = DynamicImage::from_decoder(decoder).map_err(corrupt)?;
    image.apply_orientation(orientation);
    Ok(Picture {
        image,
        icc,
        density: None,
    })
}

/// Encode to JPG (transparency on white), PNG or WebP, embedding `icc` when given.
/// `quality` is 1–100 (ignored for PNG).
pub fn encode<W: Write + Seek>(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    format: OutputFormat,
    quality: u8,
    out: &mut W,
) -> Result<(), AppError> {
    let quality = quality.clamp(1, 100);
    match format {
        OutputFormat::Jpg => {
            let rgb = flatten_on_white(img);
            let mut encoder = JpegEncoder::new_with_quality(out, quality);
            if let Some(icc) = icc {
                let _ = encoder.set_icc_profile(icc.to_vec());
            }
            DynamicImage::ImageRgb8(rgb)
                .write_with_encoder(encoder)
                .map_err(corrupt)
        }
        OutputFormat::Png => {
            let mut encoder = PngEncoder::new(out);
            if let Some(icc) = icc {
                let _ = encoder.set_icc_profile(icc.to_vec());
            }
            img.write_with_encoder(encoder).map_err(corrupt)
        }
        OutputFormat::Webp => {
            let (w, h) = (img.width(), img.height());
            let has_alpha = img.color().has_alpha();
            let encoded = if has_alpha {
                let rgba = img.to_rgba8();
                webp::Encoder::from_rgba(&rgba, w, h).encode(f32::from(quality))
            } else {
                let rgb = img.to_rgb8();
                webp::Encoder::from_rgb(&rgb, w, h).encode(f32::from(quality))
            };
            match icc {
                Some(icc) => out.write_all(&webp_with_icc(&encoded, icc, w, h, has_alpha))?,
                None => out.write_all(&encoded)?,
            }
            Ok(())
        }
    }
}

/// Write `img` in the same format as `input` (for operations that keep the format, like resize).
pub fn encode_like<W: Write + Seek>(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    input: &Path,
    out: &mut W,
) -> Result<(), AppError> {
    // QuickConvert doesn't write HEIC: same-format operations save it as JPG.
    if crate::heic::is_heic(input) {
        return encode(img, icc, OutputFormat::Jpg, KEEP_FORMAT_QUALITY, out);
    }
    let format = ImageFormat::from_path(input).map_err(corrupt)?;
    match format {
        ImageFormat::Jpeg => encode(img, icc, OutputFormat::Jpg, KEEP_FORMAT_QUALITY, out),
        ImageFormat::WebP => encode(img, icc, OutputFormat::Webp, KEEP_FORMAT_QUALITY, out),
        ImageFormat::Png => encode(img, icc, OutputFormat::Png, KEEP_FORMAT_QUALITY, out),
        other => img.write_to(out, other).map_err(corrupt),
    }
}

/// A JPG of `img` at quality 90, 85 or 80: the best of those no bigger than `budget` bytes (else
/// 80). HEIC packs a photo into about half the space of a JPG, so quality 90 alone would make iPhone
/// photos 1.5–2× bigger. Returns the bytes and the quality used.
///
/// One encode at 80 shows how much room there is: going up to 85 adds roughly 10–20 % and to 90
/// roughly 60 %, so higher qualities are only tried when they can plausibly fit. Most photos
/// need one or two encodes instead of three.
pub fn jpeg_near(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    budget: Option<f64>,
) -> Result<(Vec<u8>, u8), AppError> {
    let jpg = |quality: u8| -> Result<Vec<u8>, AppError> {
        let mut bytes = Vec::new();
        encode(
            img,
            icc,
            OutputFormat::Jpg,
            quality,
            &mut Cursor::new(&mut bytes),
        )?;
        Ok(bytes)
    };
    let Some(budget) = budget else {
        return Ok((jpg(90)?, 90));
    };
    let low = jpg(80)?;
    let size = low.len() as f64;
    for (quality, growth) in [(90, 1.6), (85, 1.05)] {
        if size * growth <= budget {
            let bytes = jpg(quality)?;
            if bytes.len() as f64 <= budget {
                return Ok((bytes, quality));
            }
        }
    }
    Ok((low, 80))
}

/// `img` in `input`'s own format (HEIC → JPG about as big as the original would be at this size,
/// via `density`), and the quality used for lossy formats.
pub fn encode_same_format(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    input: &Path,
    density: Option<f64>,
) -> Result<(Vec<u8>, Option<u8>), AppError> {
    if crate::heic::is_heic(input) {
        let pixels = f64::from(img.width()) * f64::from(img.height());
        let (bytes, quality) = jpeg_near(img, icc, density.map(|d| d * pixels))?;
        return Ok((bytes, Some(quality)));
    }
    let mut bytes = Vec::new();
    encode_like(img, icc, input, &mut Cursor::new(&mut bytes))?;
    let lossy = matches!(same_format_extension(input).as_str(), "jpg" | "webp");
    Ok((bytes, lossy.then_some(KEEP_FORMAT_QUALITY)))
}

/// Add an ICCP chunk to a simple-format WebP (libwebp's output), switching it to the extended
/// (VP8X) format as the WebP container spec requires.
pub(crate) fn webp_with_icc(
    webp: &[u8],
    icc: &[u8],
    width: u32,
    height: u32,
    has_alpha: bool,
) -> Vec<u8> {
    const ICC_FLAG: u8 = 0x20;
    const ALPHA_FLAG: u8 = 0x10;
    let Some(body) = webp
        .get(12..)
        .filter(|_| webp.starts_with(b"RIFF") && &webp[8..12] == b"WEBP")
    else {
        return webp.to_vec();
    };
    let chunk = |id: &[u8], data: &[u8]| {
        let mut c = Vec::with_capacity(8 + data.len() + 1);
        c.extend_from_slice(id);
        c.extend_from_slice(&(data.len() as u32).to_le_bytes());
        c.extend_from_slice(data);
        if data.len() % 2 == 1 {
            c.push(0); // chunks are padded to even sizes
        }
        c
    };

    let (mut vp8x, rest) = if body.starts_with(b"VP8X") && body.len() >= 18 {
        (body[8..18].to_vec(), &body[18..])
    } else {
        let mut v = vec![0u8; 10];
        if has_alpha {
            v[0] |= ALPHA_FLAG;
        }
        v[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
        v[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        (v, body)
    };
    vp8x[0] |= ICC_FLAG;

    let mut payload = b"WEBP".to_vec();
    payload.extend(chunk(b"VP8X", &vp8x));
    payload.extend(chunk(b"ICCP", icc)); // must come right after VP8X
    payload.extend_from_slice(rest);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

/// Output extension for `encode_like`: the input's, with `jpeg` normalised to `jpg`.
pub fn same_format_extension(input: &Path) -> String {
    match crate::output::extension(input).as_str() {
        "jpeg" | "heic" | "heif" => "jpg".into(),
        "tif" => "tiff".into(),
        other => other.into(),
    }
}

pub fn resize(img: &DynamicImage, mode: ResizeMode) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    match mode {
        ResizeMode::LongEdge { px } => {
            if w.max(h) <= px || px == 0 {
                img.clone()
            } else {
                img.resize(px, px, FilterType::Lanczos3)
            }
        }
        ResizeMode::Exact {
            width,
            height,
            keep_aspect,
        } => {
            let (width, height) = (width.max(1), height.max(1));
            if keep_aspect {
                img.resize(width, height, FilterType::Lanczos3)
            } else {
                img.resize_exact(width, height, FilterType::Lanczos3)
            }
        }
        ResizeMode::Percent { percent } => {
            let scale = f64::from(percent.max(0.0)) / 100.0;
            let nw = ((f64::from(w) * scale).round() as u32).max(1);
            let nh = ((f64::from(h) * scale).round() as u32).max(1);
            img.resize_exact(nw, nh, FilterType::Lanczos3)
        }
    }
}

/// RGB with any transparency composited onto white (JPEG has no alpha).
pub fn flatten_on_white(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = u32::from(p[3]);
        let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        Rgb([blend(p[0]), blend(p[1]), blend(p[2])])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_bytes(img: &DynamicImage) -> Vec<u8> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn transparent_pixels_become_white_in_jpg() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0])));
        assert_eq!(flatten_on_white(&img).get_pixel(0, 0).0, [255, 255, 255]);
        let half = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 128])));
        assert_eq!(flatten_on_white(&half).get_pixel(0, 0).0, [127, 127, 127]);
    }

    #[test]
    fn encodes_each_format_and_decodes_back() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([10, 120, 200])));
        for (format, magic) in [
            (OutputFormat::Jpg, &b"\xFF\xD8\xFF"[..]),
            (OutputFormat::Png, &b"\x89PNG"[..]),
            (OutputFormat::Webp, &b"RIFF"[..]),
        ] {
            let mut buf = Vec::new();
            encode(&img, None, format, 80, &mut Cursor::new(&mut buf)).unwrap();
            assert!(buf.starts_with(magic), "{format:?}");
            let back = image::load_from_memory(&buf).unwrap();
            assert_eq!(back.dimensions(), (40, 20), "{format:?}");
        }
    }

    #[test]
    fn lower_jpg_quality_gives_smaller_files() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_fn(200, 200, |x, y| {
            Rgb([
                (x * 7 % 255) as u8,
                (y * 13 % 255) as u8,
                ((x + y) % 255) as u8,
            ])
        }));
        let size = |q| {
            let mut buf = Vec::new();
            encode(&img, None, OutputFormat::Jpg, q, &mut Cursor::new(&mut buf)).unwrap();
            buf.len()
        };
        assert!(size(30) < size(95));
    }

    #[test]
    fn long_edge_never_upscales() {
        let img = DynamicImage::new_rgb8(4000, 3000);
        assert_eq!(
            resize(&img, ResizeMode::LongEdge { px: 1600 }).dimensions(),
            (1600, 1200)
        );
        let tall = DynamicImage::new_rgb8(3000, 4000);
        assert_eq!(
            resize(&tall, ResizeMode::LongEdge { px: 1600 }).dimensions(),
            (1200, 1600)
        );
        let small = DynamicImage::new_rgb8(800, 600);
        assert_eq!(
            resize(&small, ResizeMode::LongEdge { px: 1600 }).dimensions(),
            (800, 600)
        );
    }

    #[test]
    fn exact_and_percent_modes() {
        let img = DynamicImage::new_rgb8(400, 200);
        let fit = ResizeMode::Exact {
            width: 100,
            height: 100,
            keep_aspect: true,
        };
        assert_eq!(resize(&img, fit).dimensions(), (100, 50));
        let stretch = ResizeMode::Exact {
            width: 100,
            height: 100,
            keep_aspect: false,
        };
        assert_eq!(resize(&img, stretch).dimensions(), (100, 100));
        assert_eq!(
            resize(&img, ResizeMode::Percent { percent: 50.0 }).dimensions(),
            (200, 100)
        );
        assert_eq!(
            resize(&img, ResizeMode::Percent { percent: 0.01 }).dimensions(),
            (1, 1)
        );
    }

    #[test]
    fn resize_mode_parses_from_ui_json() {
        let mode: ResizeMode =
            serde_json::from_str(r#"{"kind":"exact","width":10,"height":20,"keepAspect":true}"#)
                .unwrap();
        assert_eq!(
            mode,
            ResizeMode::Exact {
                width: 10,
                height: 20,
                keep_aspect: true
            }
        );
    }

    #[test]
    fn png_round_trip_keeps_pixels() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 3, Rgb([1, 2, 3])));
        let back = image::load_from_memory(&png_bytes(&img)).unwrap();
        assert_eq!(back.to_rgb8().get_pixel(1, 1).0, [1, 2, 3]);
    }

    #[test]
    fn colour_profile_survives_every_output_format() {
        // Decoders return the ICC bytes as stored, so any recognisable payload works.
        let icc: Vec<u8> = (0..300u32).map(|i| (i * 7 % 251) as u8).collect();
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(30, 20, Rgb([200, 40, 40])));
        let rgba = DynamicImage::ImageRgba8(RgbaImage::from_pixel(30, 20, Rgba([0, 90, 200, 128])));
        for (source, format) in [
            (&img, OutputFormat::Jpg),
            (&img, OutputFormat::Png),
            (&img, OutputFormat::Webp),
            (&rgba, OutputFormat::Webp),
        ] {
            let mut buf = Vec::new();
            encode(source, Some(&icc), format, 90, &mut Cursor::new(&mut buf)).unwrap();
            let mut decoder = ImageReader::new(Cursor::new(&buf))
                .with_guessed_format()
                .unwrap()
                .into_decoder()
                .unwrap();
            assert_eq!(
                decoder.icc_profile().unwrap().as_deref(),
                Some(&icc[..]),
                "{format:?} alpha={}",
                source.color().has_alpha()
            );
            let back = DynamicImage::from_decoder(decoder).unwrap();
            assert_eq!(back.dimensions(), (30, 20), "{format:?}");
        }
    }

    #[test]
    fn load_returns_the_embedded_profile() {
        let icc = b"fake-display-p3-profile".to_vec();
        let mut buf = Vec::new();
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb([1, 2, 3])));
        encode(
            &img,
            Some(&icc),
            OutputFormat::Jpg,
            90,
            &mut Cursor::new(&mut buf),
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!("qc-icc-{}.jpg", std::process::id()));
        std::fs::write(&path, &buf).unwrap();
        assert_eq!(load(&path).unwrap().icc, Some(icc));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn heic_to_jpg_picks_the_best_quality_that_fits_the_budget() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_fn(160, 120, |x, y| {
            Rgb([
                (x * 37 % 251) as u8,
                (y * 91 % 241) as u8,
                ((x * y) % 239) as u8,
            ])
        }));
        let size = |q| {
            let mut buf = Vec::new();
            encode(&img, None, OutputFormat::Jpg, q, &mut Cursor::new(&mut buf)).unwrap();
            buf.len() as f64
        };
        assert_eq!(
            jpeg_near(&img, None, None).unwrap().1,
            90,
            "no budget: best quality"
        );
        let between = (size(90) + size(85)) / 2.0;
        let (bytes, quality) = jpeg_near(&img, None, Some(between)).unwrap();
        assert_eq!(quality, 85);
        assert!(bytes.len() as f64 <= between);
        assert_eq!(
            jpeg_near(&img, None, Some(10.0)).unwrap().1,
            80,
            "never below 80"
        );
    }
}
