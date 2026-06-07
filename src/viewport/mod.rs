//! Viewport — the visible window over the infinite canvas (M3 coords; M4 scroll).
//!
//! M3 implements the coordinate mapping between screen offsets and absolute
//! canvas coordinates. Scrolling, the Shift = 1/3 jump, and cursor-follow land
//! in M4. See ./CLAUDE.md for the full contract.

use crate::canvas::Coord;

/// The rectangle of the canvas currently drawn. `origin` is the absolute canvas
/// coordinate shown at the top-left of the canvas drawing area; `width`/`height`
/// are that area's size in cells (the status line is excluded by the caller).
#[derive(Debug, Clone, Default)]
pub struct Viewport {
    pub origin: (Coord, Coord),
    pub width: u16,
    pub height: u16,
}

impl Viewport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Absolute canvas coordinate shown at screen offset `(sx, sy)` within the
    /// viewport's drawing area.
    pub fn canvas_at(&self, sx: u16, sy: u16) -> (Coord, Coord) {
        (self.origin.0 + sx as Coord, self.origin.1 + sy as Coord)
    }

    /// Screen offset of an absolute canvas coordinate, or `None` if it falls
    /// outside the visible rectangle.
    pub fn screen_of(&self, x: Coord, y: Coord) -> Option<(u16, u16)> {
        let dx = x - self.origin.0;
        let dy = y - self.origin.1;
        if dx >= 0 && dy >= 0 && dx < self.width as Coord && dy < self.height as Coord {
            Some((dx as u16, dy as u16))
        } else {
            None
        }
    }

    /// The one-third-of-a-screen jump distance, at least 1 cell each axis even on
    /// tiny terminals.
    pub fn step(&self) -> (Coord, Coord) {
        (
            (self.width as Coord / 3).max(1),
            (self.height as Coord / 3).max(1),
        )
    }

    /// Move the view by whole one-third steps (Shift+arrow). `dx`/`dy` are step
    /// counts (e.g. `-1`, `0`, `1`); the cursor is not touched, so it may scroll
    /// off-screen — that's the spec's "move canvas view" gesture.
    pub fn jump(&mut self, dx: i64, dy: i64) {
        let (sx, sy) = self.step();
        self.origin.0 += dx * sx;
        self.origin.1 += dy * sy;
    }

    /// Cursor-follow: scroll the minimum amount so `(cx, cy)` sits inside the
    /// visible rectangle. No-op if the viewport has no size yet.
    pub fn scroll_to_show(&mut self, (cx, cy): (Coord, Coord)) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (w, h) = (self.width as Coord, self.height as Coord);
        if cx < self.origin.0 {
            self.origin.0 = cx;
        } else if cx > self.origin.0 + w - 1 {
            self.origin.0 = cx - w + 1;
        }
        if cy < self.origin.1 {
            self.origin.1 = cy;
        } else if cy > self.origin.1 + h - 1 {
            self.origin.1 = cy - h + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_at_offsets_from_origin() {
        let vp = Viewport {
            origin: (10, -5),
            width: 80,
            height: 24,
        };
        assert_eq!(vp.canvas_at(0, 0), (10, -5));
        assert_eq!(vp.canvas_at(3, 2), (13, -3));
    }

    #[test]
    fn screen_of_is_inverse_within_bounds() {
        let vp = Viewport {
            origin: (5, 5),
            width: 10,
            height: 4,
        };
        assert_eq!(vp.screen_of(5, 5), Some((0, 0)));
        assert_eq!(vp.screen_of(14, 8), Some((9, 3))); // last visible cell
        assert_eq!(vp.screen_of(4, 5), None); // left of view
        assert_eq!(vp.screen_of(15, 5), None); // right of view
        assert_eq!(vp.screen_of(5, 9), None); // below view
    }

    #[test]
    fn step_is_a_third_and_at_least_one() {
        let big = Viewport {
            origin: (0, 0),
            width: 30,
            height: 12,
        };
        assert_eq!(big.step(), (10, 4));
        let tiny = Viewport {
            origin: (0, 0),
            width: 2,
            height: 1,
        };
        assert_eq!(tiny.step(), (1, 1));
    }

    #[test]
    fn jump_moves_origin_by_steps_only() {
        let mut vp = Viewport {
            origin: (0, 0),
            width: 30,
            height: 12,
        };
        vp.jump(1, 0);
        assert_eq!(vp.origin, (10, 0));
        vp.jump(0, -1);
        assert_eq!(vp.origin, (10, -4));
    }

    #[test]
    fn scroll_to_show_clamps_minimally() {
        let mut vp = Viewport {
            origin: (0, 0),
            width: 10,
            height: 5,
        };
        vp.scroll_to_show((12, 2)); // past right edge only
        assert_eq!(vp.origin, (3, 0)); // 12 - 10 + 1
        vp.scroll_to_show((-1, -1)); // past top-left
        assert_eq!(vp.origin, (-1, -1));
        let before = vp.origin;
        vp.scroll_to_show((vp.origin.0, vp.origin.1)); // already visible
        assert_eq!(vp.origin, before); // no-op
    }
}
