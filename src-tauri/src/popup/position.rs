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

/// A rectangle in logical px (points on macOS), for a popup the user has dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Move a dragged popup's top-left just enough that a window of `size` fits inside `area` (with
/// the edge margin). Too big to fit: pinned to the top-left margin.
pub fn keep_inside(top_left: (f64, f64), size: (f64, f64), area: Area) -> (f64, f64) {
    let fit = |v: f64, len: f64, start: f64, extent: f64| {
        let lo = start + EDGE_MARGIN;
        let hi = start + extent - EDGE_MARGIN - len;
        if hi < lo {
            lo
        } else {
            v.clamp(lo, hi)
        }
    };
    (
        fit(top_left.0, size.0, area.x, area.width),
        fit(top_left.1, size.1, area.y, area.height),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub origin: Origin,
}

/// Top-left at cursor + offset; flip to the left/above when it would overflow; clamp inside the work area.
///
/// The vertical flip is decided with `flip_height` (the tallest the popup can grow) rather than the
/// current height, so a popup whose content grows later never jumps to the other side of the cursor.
pub fn place_at_cursor(
    cursor: (f64, f64),
    size: (u32, u32),
    flip_height: u32,
    work_area: Rect,
    scale: f64,
) -> Placement {
    let offset = (CURSOR_OFFSET * scale).round() as i32;
    let margin = (EDGE_MARGIN * scale).round() as i32;
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (cx, cy) = (cursor.0.round() as i32, cursor.1.round() as i32);

    let flip_x = cx + offset + w > work_area.right() - margin;
    let flip_y = cy + offset + (flip_height.max(size.1) as i32) > work_area.bottom() - margin;
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
        let p = place_at_cursor((500.0, 300.0), POPUP, POPUP.1, SCREEN, 1.0);
        assert_eq!((p.x, p.y, p.origin), (512, 312, Origin::TopLeft));
    }

    #[test]
    fn flips_left_near_right_edge() {
        let p = place_at_cursor((1300.0, 300.0), POPUP, POPUP.1, SCREEN, 1.0);
        assert_eq!(
            (p.x, p.y, p.origin),
            (1300 - 12 - 360, 312, Origin::TopRight)
        );
    }

    #[test]
    fn flips_up_near_bottom_edge() {
        let p = place_at_cursor((500.0, 850.0), POPUP, POPUP.1, SCREEN, 1.0);
        assert_eq!(
            (p.x, p.y, p.origin),
            (512, 850 - 12 - 176, Origin::BottomLeft)
        );
    }

    #[test]
    fn flips_both_in_bottom_right_corner() {
        let p = place_at_cursor((1435.0, 895.0), POPUP, POPUP.1, SCREEN, 1.0);
        assert_eq!(p.origin, Origin::BottomRight);
        assert_eq!((p.x, p.y), (1435 - 12 - 360, 895 - 12 - 176));
    }

    #[test]
    fn clamps_inside_work_area_near_top_left() {
        // Cursor in the menu bar area above the work area.
        let p = place_at_cursor((2.0, 5.0), POPUP, POPUP.1, SCREEN, 1.0);
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
        let p = place_at_cursor((100.0, 100.0), POPUP, POPUP.1, tiny, 1.0);
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
        let p = place_at_cursor((1000.0, 600.0), size, size.1, retina, 2.0);
        assert_eq!((p.x, p.y), (1024, 624));

        let p = place_at_cursor((2870.0, 1790.0), size, size.1, retina, 2.0);
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
        let p = place_at_cursor((-100.0, -150.0), POPUP, POPUP.1, left, 1.0);
        assert_eq!(p.origin, Origin::TopRight);
        assert_eq!((p.x, p.y), (-100 - 12 - 360, -138));
        assert!(p.x >= left.x + 8 && p.x + 360 <= left.right() - 8);
    }

    #[test]
    fn centers_in_work_area() {
        let p = place_centered(POPUP, SCREEN);
        assert_eq!((p.x, p.y, p.origin), (540, 25 + 349, Origin::Center));
    }

    #[test]
    fn flips_up_early_when_popup_may_grow() {
        // 176 px fits below, but the popup can grow to 480 px, so open above.
        let p = place_at_cursor((500.0, 600.0), POPUP, 480, SCREEN, 1.0);
        assert_eq!(p.origin, Origin::BottomLeft);
        assert_eq!(p.y, 600 - 12 - 176);
    }

    #[test]
    fn stays_below_when_max_height_fits() {
        let p = place_at_cursor((500.0, 300.0), POPUP, 480, SCREEN, 1.0);
        assert_eq!((p.y, p.origin), (312, Origin::TopLeft));
    }

    #[test]
    fn dragged_popup_stays_put_unless_it_would_leave_the_screen() {
        let area = Area {
            x: 0.0,
            y: 25.0,
            width: 1440.0,
            height: 875.0,
        };
        // Fits: unchanged.
        assert_eq!(
            keep_inside((300.0, 200.0), (360.0, 300.0), area),
            (300.0, 200.0)
        );
        // Widening to the editor near the right edge: slides left just enough.
        assert_eq!(
            keep_inside((1000.0, 200.0), (720.0, 540.0), area),
            (712.0, 200.0)
        );
        // Dragged above the menu bar and growing past the bottom: back inside both ways.
        assert_eq!(keep_inside((20.0, 0.0), (360.0, 480.0), area), (20.0, 33.0));
        assert_eq!(
            keep_inside((20.0, 600.0), (360.0, 480.0), area),
            (20.0, 412.0)
        );
        // Bigger than the screen: pinned to the top-left margin.
        assert_eq!(
            keep_inside((500.0, 500.0), (2000.0, 2000.0), area),
            (8.0, 33.0)
        );
    }
}
