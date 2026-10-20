//! Place the popup near the cursor and clamp it to the monitor work area (SPECS §5).
//!
//! Pure maths, no Tauri types, so it can be unit tested. All values are physical pixels in the
//! coordinate space of the monitor the cursor is on; `scale` is that monitor's scale factor.

use serde::Serialize;

/// Gap between the cursor and the popup, in logical px.
const CURSOR_OFFSET: f64 = 12.0;
/// Minimum distance from the work-area edges, in logical px.
const EDGE_MARGIN: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }
}

/// The popup corner nearest the cursor; the UI uses it as the animation `transform-origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub origin: Origin,
}

/// Top-left at cursor + offset; flip to the left/above when it would overflow; clamp inside the work area.
pub fn place_at_cursor(
    cursor: (f64, f64),
    size: (u32, u32),
    work_area: Rect,
    scale: f64,
) -> Placement {
    let offset = (CURSOR_OFFSET * scale).round() as i32;
    let margin = (EDGE_MARGIN * scale).round() as i32;
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (cx, cy) = (cursor.0.round() as i32, cursor.1.round() as i32);

    let flip_x = cx + offset + w > work_area.right() - margin;
    let flip_y = cy + offset + h > work_area.bottom() - margin;
    let x = if flip_x { cx - offset - w } else { cx + offset };
    let y = if flip_y { cy - offset - h } else { cy + offset };

    let origin = match (flip_x, flip_y) {
        (false, false) => Origin::TopLeft,
        (true, false) => Origin::TopRight,
        (false, true) => Origin::BottomLeft,
        (true, true) => Origin::BottomRight,
    };

    Placement {
        x: clamp_axis(x, w, work_area.x, work_area.right(), margin),
        y: clamp_axis(y, h, work_area.y, work_area.bottom(), margin),
        origin,
    }
}

/// Centred in the work area (tray "Open QuickConvert").
pub fn place_centered(size: (u32, u32), work_area: Rect) -> Placement {
    Placement {
        x: work_area.x + (work_area.width as i32 - size.0 as i32) / 2,
        y: work_area.y + (work_area.height as i32 - size.1 as i32) / 2,
        origin: Origin::Center,
    }
}

/// Keep `[pos, pos + len]` inside `[start + margin, end - margin]`. If it can't fit, pin to the start edge.
fn clamp_axis(pos: i32, len: i32, start: i32, end: i32, margin: i32) -> i32 {
    let min = start + margin;
    let max = end - margin - len;
    pos.min(max).max(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    const POPUP: (u32, u32) = (360, 176);
    const SCREEN: Rect = Rect {
        x: 0,
        y: 25,
        width: 1440,
        height: 875,
    };

    #[test]
    fn opens_below_right_of_cursor() {
        let p = place_at_cursor((500.0, 300.0), POPUP, SCREEN, 1.0);
        assert_eq!((p.x, p.y, p.origin), (512, 312, Origin::TopLeft));
    }

    #[test]
    fn flips_left_near_right_edge() {
        let p = place_at_cursor((1300.0, 300.0), POPUP, SCREEN, 1.0);
        assert_eq!(
            (p.x, p.y, p.origin),
            (1300 - 12 - 360, 312, Origin::TopRight)
        );
    }

    #[test]
    fn flips_up_near_bottom_edge() {
        let p = place_at_cursor((500.0, 850.0), POPUP, SCREEN, 1.0);
        assert_eq!(
            (p.x, p.y, p.origin),
            (512, 850 - 12 - 176, Origin::BottomLeft)
        );
    }

    #[test]
    fn flips_both_in_bottom_right_corner() {
        let p = place_at_cursor((1435.0, 895.0), POPUP, SCREEN, 1.0);
        assert_eq!(p.origin, Origin::BottomRight);
        assert_eq!((p.x, p.y), (1435 - 12 - 360, 895 - 12 - 176));
    }

    #[test]
    fn clamps_inside_work_area_near_top_left() {
        // Cursor in the menu bar area above the work area.
        let p = place_at_cursor((2.0, 5.0), POPUP, SCREEN, 1.0);
        assert_eq!((p.x, p.y), (14, 25 + 8));
    }

    #[test]
    fn popup_larger_than_screen_pins_to_top_left_margin() {
        let tiny = Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 150,
        };
        let p = place_at_cursor((100.0, 100.0), POPUP, tiny, 1.0);
        assert_eq!((p.x, p.y), (8, 8));
    }

    #[test]
    fn scales_offset_and_margin_on_hidpi() {
        let retina = Rect {
            x: 0,
            y: 50,
            width: 2880,
            height: 1750,
        };
        let size = (720, 352);
        let p = place_at_cursor((1000.0, 600.0), size, retina, 2.0);
        assert_eq!((p.x, p.y), (1024, 624));

        let p = place_at_cursor((2870.0, 1790.0), size, retina, 2.0);
        assert_eq!(p.origin, Origin::BottomRight);
        assert_eq!((p.x, p.y), (2870 - 24 - 720, 1790 - 24 - 352));
    }

    #[test]
    fn works_on_monitor_with_negative_origin() {
        let left = Rect {
            x: -1920,
            y: -200,
            width: 1920,
            height: 1040,
        };
        let p = place_at_cursor((-100.0, -150.0), POPUP, left, 1.0);
        assert_eq!(p.origin, Origin::TopRight);
        assert_eq!((p.x, p.y), (-100 - 12 - 360, -138));
        assert!(p.x >= left.x + 8 && p.x + 360 <= left.right() - 8);
    }

    #[test]
    fn centers_in_work_area() {
        let p = place_centered(POPUP, SCREEN);
        assert_eq!((p.x, p.y, p.origin), (540, 25 + 349, Origin::Center));
    }
}
