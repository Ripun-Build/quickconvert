//! Crop and quarter-turn rotation (CropEditor, Rotate).

use image::DynamicImage;
use serde::Deserialize;

use crate::errors::AppError;

/// Clockwise quarter turns. The UI sends degrees: 0, 90, 180 or 270.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "u16")]
pub enum Turn {
    #[default]
    None,
    Right,
    Half,
    Left,
}

impl TryFrom<u16> for Turn {
    type Error = String;

    fn try_from(degrees: u16) -> Result<Self, Self::Error> {
        match degrees {
            0 => Ok(Turn::None),
            90 => Ok(Turn::Right),
            180 => Ok(Turn::Half),
            270 => Ok(Turn::Left),
            other => Err(format!("unsupported rotation {other}°")),
        }
    }
}

impl Turn {
    /// Clockwise degrees, as the UI sends them.
    pub fn degrees(self) -> u16 {
        match self {
            Turn::None => 0,
            Turn::Right => 90,
            Turn::Half => 180,
            Turn::Left => 270,
        }
    }
}

/// Area to keep, in pixels of the upright image after any turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn rotate(img: DynamicImage, turn: Turn) -> DynamicImage {
    match turn {
        Turn::None => img,
        Turn::Right => img.rotate90(),
        Turn::Half => img.rotate180(),
        Turn::Left => img.rotate270(),
    }
}

/// The part of `img` inside `rect`, clamped to the image's edges.
pub fn crop(img: &DynamicImage, rect: CropRect) -> Result<DynamicImage, AppError> {
    let (w, h) = (img.width(), img.height());
    let (x, y) = (rect.x.min(w), rect.y.min(h));
    let width = rect.width.min(w - x);
    let height = rect.height.min(h - y);
    if width == 0 || height == 0 {
        return Err(AppError::invalid(
            "The crop area is empty. Make the box bigger and try again.",
        ));
    }
    Ok(img.crop_imm(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage};

    /// 4 × 2 image whose pixel at (x, y) is [x, y, 0], so positions can be checked after a turn.
    fn grid() -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(4, 2, |x, y| Rgb([x as u8, y as u8, 0])))
    }

    fn at(img: &DynamicImage, x: u32, y: u32) -> [u8; 2] {
        let p = img.get_pixel(x, y).0;
        [p[0], p[1]]
    }

    #[test]
    fn crop_keeps_exactly_the_box() {
        let rect = CropRect {
            x: 1,
            y: 1,
            width: 2,
            height: 1,
        };
        let out = crop(&grid(), rect).unwrap();
        assert_eq!(out.dimensions(), (2, 1));
        assert_eq!(at(&out, 0, 0), [1, 1]);
        assert_eq!(at(&out, 1, 0), [2, 1]);
    }

    #[test]
    fn crop_box_past_the_edge_is_clamped() {
        let rect = CropRect {
            x: 3,
            y: 0,
            width: 50,
            height: 50,
        };
        let out = crop(&grid(), rect).unwrap();
        assert_eq!(out.dimensions(), (1, 2));
        assert_eq!(at(&out, 0, 1), [3, 1]);
    }

    #[test]
    fn empty_crop_box_is_a_plain_language_error() {
        for rect in [
            CropRect {
                x: 0,
                y: 0,
                width: 0,
                height: 2,
            },
            CropRect {
                x: 4,
                y: 0,
                width: 2,
                height: 2,
            },
        ] {
            let err = crop(&grid(), rect).unwrap_err();
            assert_eq!(err.code(), "invalid_input");
            assert!(err.to_string().contains("crop area is empty"));
        }
    }

    #[test]
    fn quarter_turns_move_pixels_clockwise() {
        // Top-left pixel [0, 0] ends up top-right after a right turn, bottom-right after a half
        // turn, bottom-left after a left turn.
        let right = rotate(grid(), Turn::Right);
        assert_eq!(right.dimensions(), (2, 4));
        assert_eq!(at(&right, 1, 0), [0, 0]);
        let half = rotate(grid(), Turn::Half);
        assert_eq!(half.dimensions(), (4, 2));
        assert_eq!(at(&half, 3, 1), [0, 0]);
        let left = rotate(grid(), Turn::Left);
        assert_eq!(left.dimensions(), (2, 4));
        assert_eq!(at(&left, 0, 3), [0, 0]);
        assert_eq!(rotate(grid(), Turn::None).dimensions(), (4, 2));
    }

    #[test]
    fn turn_parses_degrees_and_rejects_others() {
        let turn: Turn = serde_json::from_str("270").unwrap();
        assert_eq!(turn, Turn::Left);
        assert!(serde_json::from_str::<Turn>("45").is_err());
    }
}
