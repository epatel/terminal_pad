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
    let iw = w - 2; // inner columns
    let ih = h - 2; // inner rows

    let cells = canvas.cells();

    // View rectangle in canvas coords (always at least 1x1).
    let vx0 = vp.origin.0;
    let vy0 = vp.origin.1;
    let vx1 = vp.origin.0 + (vp.width.max(1) as Coord) - 1;
    let vy1 = vp.origin.1 + (vp.height.max(1) as Coord) - 1;

    // Bounding box = content bounds, unioned with the view rect so the view box
    // is always on screen even when it's outside any text.
    let (mut minx, mut miny, mut maxx, mut maxy) = (vx0, vy0, vx1, vy1);
    for &(x, y, _) in &cells {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }

    let span_w = (maxx - minx + 1).max(1);
    let span_h = (maxy - miny + 1).max(1);
    // Ceiling division (all operands positive); i64::div_ceil is still unstable.
    let tile_w = ((span_w + iw as Coord - 1) / iw as Coord).max(1);
    let tile_h = ((span_h + ih as Coord - 1) / ih as Coord).max(1);

    // Per-inner-cell content counts.
    let mut counts = vec![0u32; iw * ih];
    for &(x, y, _) in &cells {
        let ix = ((x - minx) / tile_w) as usize;
        let iy = ((y - miny) / tile_h) as usize;
        if ix < iw && iy < ih {
            counts[iy * iw + ix] += 1;
        }
    }
    let capacity = (tile_w * tile_h) as f32;

    // Density glyph grid.
    let mut grid: Vec<Vec<char>> = (0..ih)
        .map(|iy| {
            (0..iw)
                .map(|ix| {
                    let c = counts[iy * iw + ix];
                    if c == 0 {
                        ' '
                    } else {
                        match c as f32 / capacity {
                            r if r < SPARSE => '-',
                            r if r < MEDIUM => '=',
                            _ => '#',
                        }
                    }
                })
                .collect()
        })
        .collect();

    overlay_view_box(
        &mut grid,
        iw,
        ih,
        (minx, miny),
        (tile_w, tile_h),
        (vx0, vy0, vx1, vy1),
    );

    assemble(&grid, iw)
}

/// Draw the view rectangle onto the grid as a box, expanded to at least 2x2 (when
/// space allows) so it's always a visible rectangle, not a single cell.
fn overlay_view_box(
    grid: &mut [Vec<char>],
    iw: usize,
    ih: usize,
    (minx, miny): (Coord, Coord),
    (tile_w, tile_h): (Coord, Coord),
    (vx0, vy0, vx1, vy1): (Coord, Coord, Coord, Coord),
) {
    let mut x0 = (vx0 - minx) / tile_w;
    let mut y0 = (vy0 - miny) / tile_h;
    let mut x1 = (vx1 - minx) / tile_w;
    let mut y1 = (vy1 - miny) / tile_h;
    if x1 < x0 + 1 {
        x1 = x0 + 1;
    }
    if y1 < y0 + 1 {
        y1 = y0 + 1;
    }
    let clamp = |v: Coord, hi: usize| v.clamp(0, hi as Coord - 1) as usize;
    x0 = clamp(x0, iw) as Coord;
    y0 = clamp(y0, ih) as Coord;
    x1 = clamp(x1, iw) as Coord;
    y1 = clamp(y1, ih) as Coord;

    for y in y0..=y1 {
        for x in x0..=x1 {
            let (top, bot, left, right) = (y == y0, y == y1, x == x0, x == x1);
            let ch = match (top, bot, left, right) {
                (true, _, true, _) => '┌',
                (true, _, _, true) => '┐',
                (_, true, true, _) => '└',
                (_, true, _, true) => '┘',
                (true, _, _, _) | (_, true, _, _) => '─',
                (_, _, true, _) | (_, _, _, true) => '│',
                _ => continue, // interior: leave the content glyph visible
            };
            grid[y as usize][x as usize] = ch;
        }
    }
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
