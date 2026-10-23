//! Which way is up? Finds the quarter turn that makes a photo or scan straight, from its text and
//! faces (Rotate → Straight). Like `selection/`, the OS-specific part lives in this module:
//! macOS asks the system's Vision framework through `osascript` (offline, nothing bundled);
//! elsewhere the answer is "can't tell" for now.

#[cfg(target_os = "macos")]
mod macos;

use std::path::Path;

use image::DynamicImage;
use serde::{Deserialize, Serialize};

use crate::convert::crop::Turn;
use crate::errors::AppError;

/// How convincing the image looks read as is, then as if it needs a quarter turn right, a half
/// turn, a quarter turn left (same order as `TURNS`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Scores {
    /// Letters of real words read.
    pub text: [f64; 4],
    /// Face confidence, weighted by how level the faces are.
    pub faces: [f64; 4],
}

const TURNS: [Turn; 4] = [Turn::None, Turn::Right, Turn::Half, Turn::Left];
/// Fewer real letters than this is too little text to judge.
const MIN_LETTERS: f64 = 12.0;
const MIN_FACE: f64 = 0.6;
/// The winning reading must beat the runner-up by this factor.
const MARGIN: f64 = 2.0;

/// The turn that makes the image straight; None when neither text nor faces make it clear
/// (landscapes, a few scattered words, mixed signals).
pub fn decide(scores: &Scores) -> Option<Turn> {
    winner(&scores.text, MIN_LETTERS)
        .or_else(|| winner(&scores.faces, MIN_FACE))
        .map(|i| TURNS[i])
}

fn winner(scores: &[f64; 4], min: f64) -> Option<usize> {
    let (best, &top) = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    let runner_up = scores
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != best)
        .map(|(_, s)| *s)
        .fold(0.0, f64::max);
    (top >= min && top >= runner_up * MARGIN).then_some(best)
}

/// The turn that makes `image` (upright per its EXIF tag already) straight, if it can tell.
pub fn detect(image: &DynamicImage) -> Option<Turn> {
    #[cfg(target_os = "macos")]
    {
        match macos::scores(image) {
            Ok(scores) => decide(&scores),
            Err(err) => {
                eprintln!("[upright] {err:?}");
                None
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = image;
        None
    }
}

/// What the Rotate view shows for one image.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateCheck {
    /// JPEG data URL, longest side `size` px, upright per the file's EXIF tag.
    pub url: String,
    /// Clockwise degrees that make it straight; None = can't tell.
    pub upright: Option<u16>,
}

pub fn check(path: &Path, size: u32) -> Result<RotateCheck, AppError> {
    // Detection reads a 1280 px copy anyway: HEIC is decoded at about that size straight away.
    let picture = crate::convert::image::load_small(path, 1280)?;
    let url =
        crate::files::jpeg_data_url(&picture.image.thumbnail(size, size), picture.icc.as_deref())?;
    Ok(RotateCheck {
        url,
        upright: detect(&picture.image).map(Turn::degrees),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores(text: [f64; 4], faces: [f64; 4]) -> Scores {
        Scores { text, faces }
    }

    #[test]
    fn clear_text_decides() {
        let s = scores([0.0, 30.0, 0.0, 82.0], [0.0; 4]);
        assert_eq!(decide(&s), Some(Turn::Left));
        let s = scores([82.0, 0.0, 30.0, 0.0], [0.0; 4]);
        assert_eq!(decide(&s), Some(Turn::None));
    }

    #[test]
    fn faces_decide_when_there_is_too_little_text() {
        let s = scores([5.0, 0.0, 0.0, 0.0], [0.1, 0.0, 0.95, 0.0]);
        assert_eq!(decide(&s), Some(Turn::Half));
    }

    #[test]
    fn unclear_or_empty_is_unknown() {
        assert_eq!(decide(&scores([0.0; 4], [0.0; 4])), None);
        // Text both ways up about equally (e.g. a table rotated in a page).
        assert_eq!(decide(&scores([40.0, 0.0, 30.0, 0.0], [0.0; 4])), None);
        // Faces too faint.
        assert_eq!(decide(&scores([0.0; 4], [0.0, 0.3, 0.0, 0.0])), None);
    }

    #[test]
    fn scores_parse_from_the_script_output() {
        let s: Scores =
            serde_json::from_str(r#"{"text":[82,0,30,0],"faces":[0,0,0.5,0]}"#).unwrap();
        assert_eq!(s.text, [82.0, 0.0, 30.0, 0.0]);
    }

    /// Real Vision run on a fixture with printed text, turned each way.
    #[cfg(target_os = "macos")]
    #[test]
    fn vision_finds_which_way_is_up() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upright-text.png");
        let upright = image::open(fixture).unwrap();
        assert_eq!(detect(&upright), Some(Turn::None));
        // Turned right (clockwise) by someone → needs a left turn back, and so on.
        assert_eq!(detect(&upright.rotate90()), Some(Turn::Left));
        assert_eq!(detect(&upright.rotate180()), Some(Turn::Half));
        assert_eq!(detect(&upright.rotate270()), Some(Turn::Right));
        // Nothing to read: can't tell.
        assert_eq!(detect(&DynamicImage::new_rgb8(300, 200)), None);
    }
}
