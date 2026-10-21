//! Image compression, including compress-to-target-size (SPECS §7) and presets (SPECS §8).

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::{JpegEncoder, PixelDensity};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder};
use serde::{Deserialize, Serialize};

use super::image::{flatten_on_white, webp_with_icc, OutputFormat};
use crate::errors::AppError;

/// Quality search range for lossy formats (SPECS §7).
const MIN_QUALITY: u8 = 10;
const MAX_QUALITY: u8 = 95;
const MAX_SEARCH_STEPS: usize = 8;
/// Fine-tuning rounds after the first size estimate, each shrinking by `SHRINK`.
const MAX_SHRINK_ROUNDS: usize = 6;
const SHRINK: f64 = 0.85;
/// Never shrink below this long edge when chasing a size target.
const MIN_LONG_EDGE: u32 = 64;

/// "KB" means 1000 bytes to some portals and 1024 to others. Aiming under `max × 1000` and over
/// `min × 1024` passes under either reading.
pub fn max_bytes(kb: u32) -> u64 {
    u64::from(kb) * 1000
}
pub fn min_bytes(kb: u32) -> u64 {
    u64::from(kb) * 1024
}

/// What the user asked Compress for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CompressTarget {
    /// Largest quality whose file is at most this many KB.
    Size { max_kb: u32 },
    /// Fixed quality 1–100 (PNG: best lossless compression).
    Quality { quality: u8 },
}

/// A size/format recipe (SPECS §8). Shipped presets are examples, never official specs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub format: OutputFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// "35:45" — used when width/height are absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_edge: Option<u32>,
    #[serde(default, rename = "minKB", skip_serializing_if = "Option::is_none")]
    pub min_kb: Option<u32>,
    #[serde(default, rename = "maxKB", skip_serializing_if = "Option::is_none")]
    pub max_kb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// How to encode one image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub format: OutputFormat,
    pub goal: Goal,
    /// Written to JPEG metadata.
    pub dpi: Option<u16>,
    /// Size targets may shrink the image when quality alone isn't enough (not for fixed-size presets).
    pub may_shrink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    Quality(u8),
    Size { max: Option<u64>, min: Option<u64> },
}

/// Encoded result plus what it ended up as.
#[derive(Debug, Clone, PartialEq)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// None for lossless output (PNG).
    pub quality: Option<u8>,
}

/// Compress keeps the input's format where that can be tuned; BMP/TIFF/GIF become PNG.
pub fn format_for(input: &Path) -> OutputFormat {
    match crate::output::extension(input).as_str() {
        "jpg" | "jpeg" => OutputFormat::Jpg,
        "webp" => OutputFormat::Webp,
        _ => OutputFormat::Png,
    }
}

pub fn plan_for_target(input: &Path, target: CompressTarget) -> Plan {
    Plan {
        format: format_for(input),
        goal: match target {
            CompressTarget::Size { max_kb } => Goal::Size {
                max: Some(max_bytes(max_kb)),
                min: None,
            },
            CompressTarget::Quality { quality } => Goal::Quality(quality),
        },
        dpi: None,
        may_shrink: true,
    }
}

/// Apply a preset's geometry, then return the plan for its format and size window.
pub fn apply_preset_geometry(
    img: &DynamicImage,
    preset: &Preset,
) -> Result<(DynamicImage, Plan), AppError> {
    let shaped = match (preset.width, preset.height) {
        (Some(w), Some(h)) => img.resize_to_fill(w.max(1), h.max(1), FilterType::Lanczos3),
        _ => {
            let cropped = match preset.aspect.as_deref().map(parse_aspect) {
                Some(Some(ratio)) => crop_to_aspect(img, ratio),
                Some(None) => {
                    return Err(AppError::invalid("This preset's aspect ratio isn't valid."))
                }
                None => img.clone(),
            };
            match preset.long_edge {
                Some(px) if cropped.width().max(cropped.height()) > px => {
                    cropped.resize(px, px, FilterType::Lanczos3)
                }
                _ => cropped,
            }
        }
    };
    let fixed_size = preset.width.is_some() && preset.height.is_some();
    let goal = if preset.max_kb.is_some() || preset.min_kb.is_some() {
        Goal::Size {
            max: preset.max_kb.map(max_bytes),
            min: preset.min_kb.map(min_bytes),
        }
    } else {
        Goal::Quality(90)
    };
    Ok((
        shaped,
        Plan {
            format: preset.format,
            goal,
            dpi: preset.dpi,
            may_shrink: !fixed_size,
        },
    ))
}

/// "35:45" or "3.5:4.5" → width / height.
fn parse_aspect(text: &str) -> Option<f64> {
    let (w, h) = text.split_once(':')?;
    let (w, h): (f64, f64) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w > 0.0 && h > 0.0).then_some(w / h)
}

/// Centre crop to `ratio` (width / height).
fn crop_to_aspect(img: &DynamicImage, ratio: f64) -> DynamicImage {
    let (w, h) = (f64::from(img.width()), f64::from(img.height()));
    let (cw, ch) = if w / h > ratio {
        (h * ratio, h)
    } else {
        (w, w / ratio)
    };
    let (cw, ch) = ((cw.round() as u32).max(1), (ch.round() as u32).max(1));
    img.crop_imm((img.width() - cw) / 2, (img.height() - ch) / 2, cw, ch)
}

/// Encode `img` per `plan`. Size goals follow SPECS §7: binary-search quality, shrink if even
/// the lowest quality is too big, raise quality if a preset's minimum isn't met.
pub fn compress(img: &DynamicImage, icc: Option<&[u8]>, plan: &Plan) -> Result<Encoded, AppError> {
    match plan.goal {
        Goal::Quality(q) => encode(img, icc, plan, q),
        Goal::Size { max, min } => compress_to_size(img, icc, plan, max, min),
    }
}

fn compress_to_size(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    plan: &Plan,
    max: Option<u64>,
    min: Option<u64>,
) -> Result<Encoded, AppError> {
    let lossy = plan.format != OutputFormat::Png;
    let mut current = img.clone();
    let mut jumped = false;
    let mut rounds = 0;
    loop {
        let found = match max {
            None => Some(encode(&current, icc, plan, MAX_QUALITY)?),
            Some(max) if lossy => search_quality(&current, icc, plan, max)?,
            Some(max) => Some(encode(&current, icc, plan, MAX_QUALITY)?)
                .filter(|e| e.bytes.len() as u64 <= max),
        };
        if let Some(encoded) = found {
            if let Some(min) = min {
                if (encoded.bytes.len() as u64) < min {
                    return Err(AppError::invalid(format!(
                        "Couldn't get this above {} KB at {}×{}.",
                        min / 1024,
                        encoded.width,
                        encoded.height
                    )));
                }
            }
            return Ok(encoded);
        }

        // Too big even at the lowest quality: shrink (first jump close, then 15% steps).
        let max = max.unwrap_or(u64::MAX);
        let long_edge = current.width().max(current.height());
        if !plan.may_shrink || rounds >= MAX_SHRINK_ROUNDS || long_edge <= MIN_LONG_EDGE {
            return Err(too_big(max, min));
        }
        let scale = if jumped {
            rounds += 1;
            SHRINK
        } else {
            // First shrink: jump close to the target (file size grows roughly with pixel count).
            // This estimate doesn't use up one of the fine-tuning rounds.
            jumped = true;
            let smallest = encode(
                &current,
                icc,
                plan,
                if lossy { MIN_QUALITY } else { MAX_QUALITY },
            )?;
            ((max as f64 / smallest.bytes.len() as f64).sqrt() * 0.95).min(SHRINK)
        };
        let w = ((f64::from(current.width()) * scale).round() as u32).max(1);
        let h = ((f64::from(current.height()) * scale).round() as u32).max(1);
        if w.max(h) < MIN_LONG_EDGE {
            return Err(too_big(max, min));
        }
        current = current.resize_exact(w, h, FilterType::Lanczos3);
    }
}

fn too_big(max: u64, min: Option<u64>) -> AppError {
    let kb = max / 1000;
    match min {
        Some(_) => AppError::invalid(format!(
            "Couldn't get this below {kb} KB without going under the minimum size."
        )),
        None => AppError::invalid(format!(
            "Couldn't get this below {kb} KB without making it too small."
        )),
    }
}

/// Largest quality in 10–95 whose output fits in `max` bytes, or None if even 10 is too big.
fn search_quality(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    plan: &Plan,
    max: u64,
) -> Result<Option<Encoded>, AppError> {
    let fits = |e: &Encoded| e.bytes.len() as u64 <= max;
    let top = encode(img, icc, plan, MAX_QUALITY)?;
    if fits(&top) {
        return Ok(Some(top));
    }
    let bottom = encode(img, icc, plan, MIN_QUALITY)?;
    if !fits(&bottom) {
        return Ok(None);
    }
    let (mut lo, mut hi, mut best) = (MIN_QUALITY, MAX_QUALITY, bottom);
    for _ in 0..MAX_SEARCH_STEPS {
        if hi - lo <= 1 {
            break;
        }
        let mid = lo + (hi - lo) / 2;
        let attempt = encode(img, icc, plan, mid)?;
        if fits(&attempt) {
            lo = mid;
            best = attempt;
        } else {
            hi = mid;
        }
    }
    Ok(Some(best))
}

/// Encode once. PNG ignores quality and uses the strongest lossless compression.
fn encode(
    img: &DynamicImage,
    icc: Option<&[u8]>,
    plan: &Plan,
    quality: u8,
) -> Result<Encoded, AppError> {
    let quality = quality.clamp(1, 100);
    let mut bytes = Vec::new();
    let failed = |e: image::ImageError| AppError::internal(e.to_string());
    match plan.format {
        OutputFormat::Jpg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
            if let Some(dpi) = plan.dpi {
                encoder.set_pixel_density(PixelDensity::dpi(dpi));
            }
            if let Some(icc) = icc {
                let _ = encoder.set_icc_profile(icc.to_vec());
            }
            DynamicImage::ImageRgb8(flatten_on_white(img))
                .write_with_encoder(encoder)
                .map_err(failed)?;
        }
        OutputFormat::Png => {
            let mut encoder = PngEncoder::new_with_quality(
                Cursor::new(&mut bytes),
                CompressionType::Best,
                PngFilter::Adaptive,
            );
            if let Some(icc) = icc {
                let _ = encoder.set_icc_profile(icc.to_vec());
            }
            img.write_with_encoder(encoder).map_err(failed)?;
        }
        OutputFormat::Webp => {
            let (w, h) = (img.width(), img.height());
            let alpha = img.color().has_alpha();
            let encoded = if alpha {
                webp::Encoder::from_rgba(&img.to_rgba8(), w, h).encode(f32::from(quality))
            } else {
                webp::Encoder::from_rgb(&img.to_rgb8(), w, h).encode(f32::from(quality))
            };
            bytes = match icc {
                Some(icc) => webp_with_icc(&encoded, icc, w, h, alpha),
                None => encoded.to_vec(),
            };
        }
    }
    Ok(Encoded {
        bytes,
        width: img.width(),
        height: img.height(),
        quality: (plan.format != OutputFormat::Png).then_some(quality),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage};

    /// Photo-like test image: gradients plus pseudo-random grain, so size depends on quality.
    fn photo(w: u32, h: u32) -> DynamicImage {
        let mut seed: u32 = 12345;
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let n = (seed >> 24) as u8 / 4;
            Rgb([
                ((x * 255 / w) as u8).saturating_add(n),
                ((y * 255 / h) as u8).saturating_add(n),
                (((x + y) * 128 / (w + h)) as u8).saturating_add(n),
            ])
        }))
    }

    /// Busy texture (like skin, hair, foliage), so even small images need real bytes.
    fn grainy(w: u32, h: u32) -> DynamicImage {
        let mut seed: u32 = 99;
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |_, _| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let v = (seed >> 16) as u8;
            Rgb([v, v.wrapping_mul(3), v.wrapping_add(90)])
        }))
    }

    fn plan(format: OutputFormat, goal: Goal) -> Plan {
        Plan {
            format,
            goal,
            dpi: None,
            may_shrink: true,
        }
    }

    #[test]
    fn kb_limits_pass_under_both_readings() {
        assert_eq!(max_bytes(50), 50_000); // ≤ 51_200 too
        assert_eq!(min_bytes(20), 20_480); // ≥ 20_000 too
    }

    #[test]
    fn target_size_hits_the_target_with_the_best_quality() {
        let img = photo(800, 600);
        let target = max_bytes(60);
        let out = compress(
            &img,
            None,
            &plan(
                OutputFormat::Jpg,
                Goal::Size {
                    max: Some(target),
                    min: None,
                },
            ),
        )
        .unwrap();
        assert!(
            out.bytes.len() as u64 <= target,
            "{} bytes",
            out.bytes.len()
        );
        assert_eq!(
            (out.width, out.height),
            (800, 600),
            "quality alone was enough"
        );
        let q = out.quality.unwrap();
        assert!((MIN_QUALITY..=MAX_QUALITY).contains(&q));
        // One step up would no longer fit (or we're at the top).
        if q < MAX_QUALITY {
            let next = encode(
                &img,
                None,
                &plan(OutputFormat::Jpg, Goal::Quality(q + 1)),
                q + 1,
            )
            .unwrap();
            assert!(next.bytes.len() as u64 > target || q + 1 > MAX_QUALITY);
        }
    }

    #[test]
    fn large_photo_to_tiny_target_shrinks_until_it_fits() {
        let img = photo(3000, 2000);
        let target = max_bytes(20);
        for format in [OutputFormat::Jpg, OutputFormat::Webp] {
            let out = compress(
                &img,
                None,
                &plan(
                    format,
                    Goal::Size {
                        max: Some(target),
                        min: None,
                    },
                ),
            )
            .unwrap();
            assert!(
                out.bytes.len() as u64 <= target,
                "{format:?}: {} bytes",
                out.bytes.len()
            );
            assert!(out.width < 3000 && (out.width as f64 / out.height as f64 - 1.5).abs() < 0.01);
        }
    }

    #[test]
    fn png_target_shrinks_pixels_and_stays_png() {
        let img = photo(600, 400);
        let target = max_bytes(80);
        let out = compress(
            &img,
            None,
            &plan(
                OutputFormat::Png,
                Goal::Size {
                    max: Some(target),
                    min: None,
                },
            ),
        )
        .unwrap();
        assert!(out.bytes.starts_with(b"\x89PNG"));
        assert!(out.bytes.len() as u64 <= target);
        assert_eq!(out.quality, None);
    }

    #[test]
    fn exam_photo_preset_lands_in_its_window_at_exact_size() {
        let preset: Preset = serde_json::from_str(
            r#"{"id":"photo_small_example","label":"Exam photo","format":"jpg","width":200,"height":230,"minKB":20,"maxKB":50,"dpi":200}"#,
        )
        .unwrap();
        let (shaped, plan) = apply_preset_geometry(&grainy(240, 300), &preset).unwrap();
        assert!(!plan.may_shrink);
        let out = compress(&shaped, None, &plan).unwrap();
        assert_eq!((out.width, out.height), (200, 230));
        let len = out.bytes.len() as u64;
        assert!(
            (min_bytes(20)..=max_bytes(50)).contains(&len),
            "{len} bytes"
        );
        let decoded = image::load_from_memory(&out.bytes).unwrap();
        assert_eq!(decoded.dimensions(), (200, 230));
        // JFIF density = 200 dpi: units byte 1, then 0x00C8 twice.
        let jfif = out.bytes.windows(5).position(|w| w == b"JFIF\0").unwrap();
        assert_eq!(&out.bytes[jfif + 7..jfif + 12], &[1, 0, 200, 0, 200]);
    }

    #[test]
    fn unreachable_minimum_is_a_plain_message() {
        let flat = DynamicImage::ImageRgb8(RgbImage::from_pixel(140, 60, Rgb([255, 255, 255])));
        let preset: Preset = serde_json::from_str(
            r#"{"id":"sig","label":"Signature","format":"jpg","width":140,"height":60,"minKB":10,"maxKB":20}"#,
        )
        .unwrap();
        let (shaped, plan) = apply_preset_geometry(&flat, &preset).unwrap();
        let err = compress(&shaped, None, &plan).unwrap_err();
        assert_eq!(err.to_string(), "Couldn't get this above 10 KB at 140×60.");
    }

    #[test]
    fn unreachable_target_is_a_plain_message() {
        let img = photo(200, 200);
        let fixed = Plan {
            may_shrink: false,
            ..plan(
                OutputFormat::Jpg,
                Goal::Size {
                    max: Some(500),
                    min: None,
                },
            )
        };
        assert_eq!(
            compress(&img, None, &fixed).unwrap_err().to_string(),
            "Couldn't get this below 0 KB without making it too small."
        );
    }

    #[test]
    fn aspect_and_long_edge_presets() {
        let preset = |json: &str| serde_json::from_str::<Preset>(json).unwrap();
        let passport = preset(r#"{"id":"p","label":"P","format":"jpg","aspect":"35:45"}"#);
        let (shaped, _) = apply_preset_geometry(&photo(1000, 1000), &passport).unwrap();
        assert_eq!(shaped.dimensions(), (778, 1000));
        let email = preset(r#"{"id":"e","label":"E","format":"jpg","longEdge":1600,"maxKB":500}"#);
        let (shaped, plan) = apply_preset_geometry(&photo(4000, 3000), &email).unwrap();
        assert_eq!(shaped.dimensions(), (1600, 1200));
        assert!(plan.may_shrink);
        let (small, _) = apply_preset_geometry(&photo(800, 600), &email).unwrap();
        assert_eq!(small.dimensions(), (800, 600), "never upscales");
        assert!(apply_preset_geometry(
            &photo(10, 10),
            &preset(r#"{"id":"x","label":"X","format":"jpg","aspect":"0:1"}"#)
        )
        .is_err());
    }

    #[test]
    fn compress_keeps_tunable_formats() {
        assert_eq!(format_for(Path::new("a.JPEG")), OutputFormat::Jpg);
        assert_eq!(format_for(Path::new("a.webp")), OutputFormat::Webp);
        assert_eq!(format_for(Path::new("a.png")), OutputFormat::Png);
        assert_eq!(format_for(Path::new("a.bmp")), OutputFormat::Png);
    }

    #[test]
    fn shipped_presets_parse() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/presets.json"
        ))
        .unwrap();
        #[derive(Deserialize)]
        struct File {
            presets: Vec<Preset>,
        }
        let file: File = serde_json::from_str(&text).unwrap();
        assert_eq!(file.presets.len(), 6);
        assert!(file.presets.iter().all(|p| p.format == OutputFormat::Jpg));
    }
}
