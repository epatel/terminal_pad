//! Overview — the zoomed-out minimap (M9), toggled by Ctrl+Z.
//!
//! Downsamples the whole canvas into a framed density map: each inner cell maps
//! to a tile of NxN canvas cells, drawn by fill density. The current normal-view
//! window is overlaid as a box so you can see where you are. See ./CLAUDE.md.

use crate::canvas::{Canvas, Coord};
use crate::viewport::Viewport;

/// Whether the normal editor or the zoomed-out overview is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZoomMode {
    #[default]
    Normal,
    Overview,
}

// Density thresholds (written cells / tile capacity) → glyph.
const SPARSE: f32 = 0.10;
const MEDIUM: f32 = 0.35;

/// Build the minimap as one `String` per screen row (each `width` chars), framed
/// with box-drawing corners, content shown by density (`#`/`=`/`-`/blank), and
/// the current view window overlaid as a box. Pure (no terminal I/O).
pub fn rows(canvas: &Canvas, vp: &Viewport, width: u16, height: u16) -> Vec<String> {
    let w = width as usize;
    let h = height as usize;
    if w < 3 || h < 3 {
        return vec![" ".repeat(w); h]; // too small for a frame
    }
    let cells = canvas.cells();
    let view = CellRect::of_view(vp);
    // Bounding box = content bounds, unioned with the view rect so the view box
    // is always on screen even when it's outside any text.
    let bounds = cells.iter().fold(view, |r, &(x, y, _)| r.including(x, y));
    let tiling = Tiling::new(bounds, w - 2, h - 2);

    let mut grid = density_grid(&cells, &tiling);
    overlay_view_box(&mut grid, &tiling, view);
    assemble(&grid, tiling.iw)
}

/// An inclusive rectangle of canvas cells.
#[derive(Clone, Copy)]
struct CellRect {
    x0: Coord,
    y0: Coord,
    x1: Coord,
    y1: Coord,
}

impl CellRect {
    /// The normal view's window (always at least 1x1).
    fn of_view(vp: &Viewport) -> Self {
        let (x0, y0) = vp.origin;
        Self {
            x0,
            y0,
            x1: x0 + vp.width.max(1) as Coord - 1,
            y1: y0 + vp.height.max(1) as Coord - 1,
        }
    }

    /// The smallest rectangle covering this one and the cell `(x, y)`.
    fn including(self, x: Coord, y: Coord) -> Self {
        Self {
            x0: self.x0.min(x),
            y0: self.y0.min(y),
            x1: self.x1.max(x),
            y1: self.y1.max(y),
        }
    }
}

/// How canvas cells map onto the `iw` x `ih` inner minimap grid: each grid cell
/// covers a `tile_w` x `tile_h` tile of canvas cells, counted from `origin`.
struct Tiling {
    origin: (Coord, Coord),
    tile_w: Coord,
    tile_h: Coord,
    iw: usize,
    ih: usize,
}

impl Tiling {
    /// Tiles just large enough to fit `bounds` into the grid.
    fn new(bounds: CellRect, iw: usize, ih: usize) -> Self {
        let span_w = (bounds.x1 - bounds.x0 + 1).max(1);
        let span_h = (bounds.y1 - bounds.y0 + 1).max(1);
        Self {
            origin: (bounds.x0, bounds.y0),
            tile_w: ceil_div(span_w, iw as Coord).max(1),
            tile_h: ceil_div(span_h, ih as Coord).max(1),
            iw,
            ih,
        }
    }

    /// The (unclamped) grid cell holding canvas cell `(x, y)`.
    fn cell_of(&self, x: Coord, y: Coord) -> (Coord, Coord) {
        (
            (x - self.origin.0) / self.tile_w,
            (y - self.origin.1) / self.tile_h,
        )
    }

    /// Canvas cells per grid cell.
    fn capacity(&self) -> f32 {
        (self.tile_w * self.tile_h) as f32
    }
}

/// Ceiling division of positive operands; `i64::div_ceil` is still unstable.
fn ceil_div(a: Coord, b: Coord) -> Coord {
    (a + b - 1) / b
}

/// The density glyph grid: each inner cell drawn by how full its tile is.
fn density_grid(cells: &[(Coord, Coord, char)], tiling: &Tiling) -> Vec<Vec<char>> {
    let (iw, ih) = (tiling.iw, tiling.ih);
    let mut counts = vec![0u32; iw * ih];
    for &(x, y, _) in cells {
        let (ix, iy) = tiling.cell_of(x, y);
        if (ix as usize) < iw && (iy as usize) < ih {
            counts[iy as usize * iw + ix as usize] += 1;
        }
    }
    counts
        .chunks(iw)
        .map(|row| {
            row.iter()
                .map(|&c| density_glyph(c, tiling.capacity()))
                .collect()
        })
        .collect()
}

/// The glyph for a tile holding `count` written cells out of `capacity`.
fn density_glyph(count: u32, capacity: f32) -> char {
    match count as f32 / capacity {
        _ if count == 0 => ' ',
        r if r < SPARSE => '-',
        r if r < MEDIUM => '=',
        _ => '#',
    }
}

/// Draw the view rectangle onto the grid as a box, expanded to at least 2x2 (when
/// space allows) so it's always a visible rectangle, not a single cell.
fn overlay_view_box(grid: &mut [Vec<char>], tiling: &Tiling, view: CellRect) {
    let (x0, y0) = tiling.cell_of(view.x0, view.y0);
    let (x1, y1) = tiling.cell_of(view.x1, view.y1);
    let clamp = |v: Coord, hi: usize| v.clamp(0, hi as Coord - 1) as usize;
    let (x1, y1) = (
        clamp(x1.max(x0 + 1), tiling.iw),
        clamp(y1.max(y0 + 1), tiling.ih),
    );
    let (x0, y0) = (clamp(x0, tiling.iw), clamp(y0, tiling.ih));

    for (y, row) in grid.iter_mut().enumerate().take(y1 + 1).skip(y0) {
        for (x, cell) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
            // The interior is left alone so the content glyph stays visible.
            if let Some(ch) = box_glyph(y == y0, y == y1, x == x0, x == x1) {
                *cell = ch;
            }
        }
    }
}

/// The box-drawing glyph for a cell on a box's edges, or `None` inside it.
fn box_glyph(top: bool, bot: bool, left: bool, right: bool) -> Option<char> {
    Some(match (top, bot, left, right) {
        (true, _, true, _) => '┌',
        (true, _, _, true) => '┐',
        (_, true, true, _) => '└',
        (_, true, _, true) => '┘',
        (true, _, _, _) | (_, true, _, _) => '─',
        (_, _, true, _) | (_, _, _, true) => '│',
        _ => return None,
    })
}

/// Wrap the glyph grid in a box-drawing frame.
fn assemble(grid: &[Vec<char>], iw: usize) -> Vec<String> {
    let border: String = "─".repeat(iw);
    let mut out = Vec::with_capacity(grid.len() + 2);
    out.push(format!("┌{border}┐"));
    for row in grid {
        let mid: String = row.iter().collect();
        out.push(format!("│{mid}│"));
    }
    out.push(format!("└{border}┘"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(rows: &[String], ch: char) -> usize {
        rows.iter()
            .flat_map(|r| r.chars())
            .filter(|&c| c == ch)
            .count()
    }

    fn vp(w: u16, h: u16) -> Viewport {
        Viewport {
            origin: (0, 0),
            width: w,
            height: h,
        }
    }

    #[test]
    fn dimensions_and_frame() {
        let canvas = Canvas::new();
        let rows = rows(&canvas, &vp(10, 5), 20, 8);
        assert_eq!(rows.len(), 8);
        assert!(rows.iter().all(|r| r.chars().count() == 20));
        assert!(rows[0].starts_with('┌') && rows[0].ends_with('┐'));
        assert!(rows[7].starts_with('└') && rows[7].ends_with('┘'));
    }

    #[test]
    fn dense_content_shows_hash() {
        let mut canvas = Canvas::new();
        // Fill a block solid so at least one tile is fully dense.
        for y in 0..40 {
            for x in 0..40 {
                canvas.set(x, y, '#');
            }
        }
        let rows = rows(&canvas, &vp(10, 5), 20, 8);
        assert!(count(&rows, '#') > 0);
    }

    #[test]
    fn view_box_is_drawn_inside() {
        // Content spans a wide area; the small view sits away from the corner,
        // so a second top-left corner (the view box) appears beyond the frame's.
        let mut canvas = Canvas::new();
        for x in 0..200 {
            canvas.set(x, 50, 'x');
        }
        let viewport = Viewport {
            origin: (90, 20),
            width: 10,
            height: 4,
        };
        let rows = rows(&canvas, &viewport, 30, 12);
        assert!(count(&rows, '┌') >= 2); // frame corner + view-box corner
    }
}
