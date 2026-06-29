//! Selection — a rectangular block of canvas cells, made by left-click-drag.
//!
//! The model is a pure value (two corners in absolute canvas coords) plus pure
//! canvas operations (extract / to-text / clear). Drag tracking, the internal
//! clip buffer, and the copy/paste/delete orchestration live on `App`; the
//! system-clipboard side effect lives in `crate::clipboard`. See ./CLAUDE.md.

use crate::canvas::{Canvas, Coord};

/// A rectangular selection, defined by the drag `anchor` and the current `head`
/// (the dragged corner). Either corner may hold the min or max coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (Coord, Coord),
    pub head: (Coord, Coord),
}

impl Selection {
    /// A zero-size selection at a single cell (a click before any drag).
    pub fn new(p: (Coord, Coord)) -> Self {
        Self { anchor: p, head: p }
    }

    /// Inclusive normalized bounds `(min_x, min_y, max_x, max_y)`.
    pub fn bounds(&self) -> (Coord, Coord, Coord, Coord) {
        (
            self.anchor.0.min(self.head.0),
            self.anchor.1.min(self.head.1),
            self.anchor.0.max(self.head.0),
            self.anchor.1.max(self.head.1),
        )
    }

    /// Whether `(x, y)` lies inside the rectangle (inclusive).
    pub fn contains(&self, x: Coord, y: Coord) -> bool {
        let (x0, y0, x1, y1) = self.bounds();
        x >= x0 && x <= x1 && y >= y0 && y <= y1
    }

    /// True when the selection covers a single cell — i.e. a click, not a drag.
    pub fn is_point(&self) -> bool {
        self.anchor == self.head
    }

    /// Width in cells (inclusive).
    pub fn width(&self) -> Coord {
        let (x0, _, x1, _) = self.bounds();
        x1 - x0 + 1
    }

    /// Height in cells (inclusive).
    pub fn height(&self) -> Coord {
        let (_, y0, _, y1) = self.bounds();
        y1 - y0 + 1
    }
}

/// The selected rectangle as a grid of chars, top→bottom then left→right, with a
/// space for each blank cell. Rectangular: every row is `selection.width()` wide.
pub fn extract(canvas: &Canvas, sel: &Selection) -> Vec<Vec<char>> {
    let (x0, y0, x1, y1) = sel.bounds();
    (y0..=y1)
        .map(|y| (x0..=x1).map(|x| canvas.get(x, y).unwrap_or(' ')).collect())
        .collect()
}

/// Render a block as newline-joined text with trailing blanks trimmed per row —
/// the friendly form for the system clipboard / other apps.
pub fn to_text(block: &[Vec<char>]) -> String {
    block
        .iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Erase every cell in the selection back to truly-blank.
pub fn clear(canvas: &mut Canvas, sel: &Selection) {
    let (x0, y0, x1, y1) = sel.bounds();
    for y in y0..=y1 {
        for x in x0..=x1 {
            canvas.clear(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_canvas() -> Canvas {
        // A 3x2 region with a gap:  "ab "
        //                           " cd"
        let mut c = Canvas::new();
        c.set(0, 0, 'a');
        c.set(1, 0, 'b');
        c.set(1, 1, 'c');
        c.set(2, 1, 'd');
        c
    }

    #[test]
    fn bounds_normalize_regardless_of_drag_direction() {
        let down_right = Selection {
            anchor: (1, 1),
            head: (4, 3),
        };
        let up_left = Selection {
            anchor: (4, 3),
            head: (1, 1),
        };
        assert_eq!(down_right.bounds(), (1, 1, 4, 3));
        assert_eq!(up_left.bounds(), (1, 1, 4, 3));
        assert_eq!(down_right.width(), 4);
        assert_eq!(down_right.height(), 3);
    }

    #[test]
    fn contains_and_is_point() {
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        assert!(sel.contains(1, 1));
        assert!(sel.contains(0, 0));
        assert!(!sel.contains(3, 1));
        assert!(!sel.contains(1, 2));
        assert!(!sel.is_point());
        assert!(Selection::new((5, 5)).is_point());
    }

    #[test]
    fn extract_fills_blanks_with_spaces() {
        let c = block_canvas();
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        assert_eq!(
            extract(&c, &sel),
            vec![vec!['a', 'b', ' '], vec![' ', 'c', 'd']]
        );
    }

    #[test]
    fn to_text_trims_trailing_blanks_per_row() {
        let block = vec![vec!['a', 'b', ' '], vec![' ', 'c', 'd']];
        assert_eq!(to_text(&block), "ab\n cd");
    }

    #[test]
    fn clear_erases_only_the_rectangle() {
        let mut c = block_canvas();
        c.set(5, 5, 'z'); // outside the selection
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        clear(&mut c, &sel);
        assert_eq!(c.get(0, 0), None);
        assert_eq!(c.get(1, 1), None);
        assert_eq!(c.get(5, 5), Some('z')); // untouched
    }
}
