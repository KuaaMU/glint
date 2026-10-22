//! HUD placement.
//!
//! Goal: open near the selection instead of the middle of the screen, and
//! never cover the text the user just selected.
//!
//! Wayland does not let a client position a normal toplevel window, so the
//! caller decides between:
//!   * `Native`: the compositor places it (current behaviour);
//!   * `XWayland`: GDK_BACKEND=x11, absolute coordinates work;
//!   * `LayerShell`: a margin-anchored overlay (handled by the window shell).
//!
//! This module holds the pure geometry so it is testable without a display.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Squared distance from `point` to this rect (0 when inside).
    pub fn squared_distance_to(&self, point: Point) -> i64 {
        let dx = (self.x - point.x).max(point.x - self.right()).max(0) as i64;
        let dy = (self.y - point.y).max(point.y - self.bottom()).max(0) as i64;
        dx * dx + dy * dy
    }
}

/// Bounding box of every monitor, in the same coordinate space as the input.
///
/// The Screenshot portal returns one image of the whole virtual desktop, so the
/// snip overlay has to cover this exact rectangle. Falls back to 1080p when the
/// backend reports no monitors yet.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn bounding_box(rects: &[Rect]) -> Rect {
    let Some(first) = rects.first() else {
        return Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
    };

    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.right();
    let mut max_y = first.bottom();
    for rect in &rects[1..] {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.right());
        max_y = max_y.max(rect.bottom());
    }

    Rect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1),
        height: (max_y - min_y).max(1),
    }
}

/// The monitor containing `point`, else the nearest one.
///
/// `cursor` is a physical desktop coordinate; monitors may have different sizes
/// and sit at negative offsets, so this cannot assume a 0,0 origin.
pub fn monitor_at_point(rects: &[Rect], point: Point) -> Option<Rect> {
    let mut nearest: Option<(i64, Rect)> = None;
    for rect in rects {
        if rect.contains(point) {
            return Some(*rect);
        }
        let distance = rect.squared_distance_to(point);
        if nearest.is_none_or(|(best, _)| distance < best) {
            nearest = Some((distance, *rect));
        }
    }
    nearest.map(|(_, rect)| rect)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionBounds {
    /// Cursor position at trigger time (top-left of the selection).
    pub cursor: Point,
    /// Estimated selection extent; unknown when the host app does not expose it.
    pub extent: Option<Rect>,
}

/// Gap between the selection and the card.
pub const GAP: i32 = 10;
/// Keep this much room from every monitor edge.
pub const EDGE_MARGIN: i32 = 12;

/// Choose a position for a `card_w` x `card_h` window.
///
/// Preference order:
///   1. below the selection, left-aligned with it;
///   2. above it, if the card would leave the monitor;
///   3. to the right of the cursor;
///   4. to the left of the cursor;
///   5. clamped to the monitor.
pub fn place_near_selection(
    selection: SelectionBounds,
    monitor: Rect,
    card_w: i32,
    card_h: i32,
) -> Point {
    let anchor = selection.extent.unwrap_or(Rect {
        x: selection.cursor.x,
        y: selection.cursor.y,
        width: 0,
        height: 0,
    });

    let min_x = monitor.x + EDGE_MARGIN;
    let max_x = monitor.right() - card_w - EDGE_MARGIN;
    let min_y = monitor.y + EDGE_MARGIN;
    let max_y = monitor.bottom() - card_h - EDGE_MARGIN;

    let candidates = [
        // below, aligned with the start of the selection
        Point {
            x: anchor.x,
            y: anchor.bottom() + GAP,
        },
        // above
        Point {
            x: anchor.x,
            y: anchor.y - card_h - GAP,
        },
        // right of the cursor
        Point {
            x: anchor.right() + GAP,
            y: anchor.y,
        },
        // left of the cursor
        Point {
            x: anchor.x - card_w - GAP,
            y: anchor.y,
        },
    ];

    for candidate in candidates {
        if candidate.x >= min_x
            && candidate.x <= max_x
            && candidate.y >= min_y
            && candidate.y <= max_y
        {
            return candidate;
        }
    }

    // Nothing fits cleanly: pick the side with the most room, then clamp.
    let space_below = monitor.bottom() - anchor.bottom();
    let space_above = anchor.y - monitor.y;
    let preferred_y = if space_below >= space_above {
        anchor.bottom() + GAP
    } else {
        anchor.y - card_h - GAP
    };

    Point {
        x: anchor.x.clamp(min_x, max_x.max(min_x)),
        y: preferred_y.clamp(min_y, max_y.max(min_y)),
    }
}

/// Overlap area between the card and the selection, used to assert the card
/// never covers the text it explains.
#[allow(dead_code)] // exercised by the placement tests, not by production code
pub fn overlap_area(card: Rect, selection: Rect) -> i32 {
    let w = (card.right().min(selection.right()) - card.x.max(selection.x)).max(0);
    let h = (card.bottom().min(selection.bottom()) - card.y.max(selection.y)).max(0);
    w * h
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    fn selection(x: i32, y: i32, w: i32, h: i32) -> SelectionBounds {
        SelectionBounds {
            cursor: Point { x, y },
            extent: Some(Rect {
                x,
                y,
                width: w,
                height: h,
            }),
        }
    }

    #[test]
    fn places_below_when_there_is_room() {
        let pos = place_near_selection(selection(400, 300, 200, 24), MONITOR, 440, 380);
        assert_eq!(pos.y, 300 + 24 + GAP);
        assert_eq!(pos.x, 400);
    }

    #[test]
    fn flips_above_near_the_bottom_edge() {
        let pos = place_near_selection(selection(400, 900, 200, 24), MONITOR, 440, 380);
        assert_eq!(pos.y, 900 - 380 - GAP);
    }

    #[test]
    fn never_covers_the_selection() {
        let sel = selection(700, 500, 300, 40);
        let pos = place_near_selection(sel, MONITOR, 440, 380);
        let card = Rect {
            x: pos.x,
            y: pos.y,
            width: 440,
            height: 380,
        };
        assert_eq!(overlap_area(card, sel.extent.unwrap()), 0);
    }

    #[test]
    fn never_covers_selection_on_a_second_monitor() {
        let monitor = Rect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let sel = selection(2000, 800, 200, 20);
        let pos = place_near_selection(sel, monitor, 440, 380);
        let card = Rect {
            x: pos.x,
            y: pos.y,
            width: 440,
            height: 380,
        };
        assert_eq!(overlap_area(card, sel.extent.unwrap()), 0);
        assert!(pos.x >= monitor.x);
    }

    #[test]
    fn clamps_inside_the_monitor() {
        let pos = place_near_selection(selection(1900, 1070, 20, 10), MONITOR, 440, 380);
        assert!(pos.x <= MONITOR.right() - 440);
        assert!(pos.y <= MONITOR.bottom() - 380);
    }

    #[test]
    fn falls_back_to_cursor_when_extent_is_unknown() {
        let bounds = SelectionBounds {
            cursor: Point { x: 500, y: 400 },
            extent: None,
        };
        let pos = place_near_selection(bounds, MONITOR, 440, 380);
        assert_eq!(pos.x, 500);
        assert_eq!(pos.y, 400 + GAP);
    }

    #[test]
    fn bounding_box_spans_both_monitors() {
        let left = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let right = Rect {
            x: 1920,
            y: 200,
            width: 2560,
            height: 1440,
        };
        assert_eq!(
            bounding_box(&[left, right]),
            Rect {
                x: 0,
                y: 0,
                width: 4480,
                height: 1640,
            }
        );
    }

    #[test]
    fn bounding_box_handles_negative_origins() {
        let left = Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let primary = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            bounding_box(&[left, primary]),
            Rect {
                x: -1920,
                y: 0,
                width: 3840,
                height: 1080,
            }
        );
    }

    #[test]
    fn empty_monitor_list_falls_back_to_1080p() {
        assert_eq!(
            bounding_box(&[]),
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        );
    }

    #[test]
    fn picks_the_monitor_under_the_cursor() {
        let left = Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let primary = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let monitors = [left, primary];
        assert_eq!(
            monitor_at_point(&monitors, Point { x: -400, y: 500 }),
            Some(left)
        );
        assert_eq!(
            monitor_at_point(&monitors, Point { x: 900, y: 500 }),
            Some(primary)
        );
    }

    #[test]
    fn picks_the_nearest_monitor_when_cursor_is_in_a_gap() {
        let left = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        // Right monitor begins 200px of vertical offset lower.
        let right = Rect {
            x: 1920,
            y: 200,
            width: 1920,
            height: 1080,
        };
        let monitors = [left, right];
        // In the dead corner above the right monitor's top edge, but further
        // from the left monitor's right edge than from the right monitor.
        assert_eq!(
            monitor_at_point(&monitors, Point { x: 2100, y: 100 }),
            Some(right)
        );
    }
}
