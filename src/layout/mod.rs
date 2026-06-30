//! Layout — the "line" model over the canvas, used by Enter to split a line and
//! push blocks below it down to make room.
//!
//! A *line* is a run of words joined by single blanks: consecutive filled columns
//! at most one blank apart. A gap of ≥2 blank columns ends it. A column is
//! "filled" when it holds a non-whitespace char — a typed space is a stored cell,
//! so a single space reads as one blank, the same as an absent cell. See
//! ./CLAUDE.md.

use crate::canvas::{Canvas, Coord};

/// Filled columns on row `y`, ascending: populated cells whose char is not
/// whitespace. `canvas.cells()` is sorted by `(y, x)`.
fn filled_columns(canvas: &Canvas, y: Coord) -> Vec<Coord> {
    canvas
        .cells()
        .into_iter()
        .filter(|&(_, cy, c)| cy == y && !c.is_whitespace())
        .map(|(x, _, _)| x)
        .collect()
}

/// Bounds `(start, end)` of the single-space-joined line on row `cy` containing
/// the cursor column `cx`. The cursor counts as "on" the line for
/// `start <= cx <= end + 1` (so the typing position just past the end still
/// belongs to it). Returns `None` when `cx` is not on a line — a blank row, or
/// parked in a gap of ≥2 blank columns.
pub fn line_bounds(canvas: &Canvas, cx: Coord, cy: Coord) -> Option<(Coord, Coord)> {
    let cols = filled_columns(canvas, cy);
    if cols.is_empty() {
        return None;
    }
    // Walk the filled columns, breaking a line wherever the gap to the next is
    // ≥2 blanks (i.e. the columns are ≥3 apart). Return the line covering `cx`.
    let mut start = cols[0];
    let mut prev = cols[0];
    for &x in &cols[1..] {
        if x - prev >= 3 {
            if (start..=prev + 1).contains(&cx) {
                return Some((start, prev));
            }
            start = x;
        }
        prev = x;
    }
    if (start..=prev + 1).contains(&cx) {
        Some((start, prev))
    } else {
        None
    }
}

/// Smallest row `r >= from` with no populated cell in columns `[lo, hi]`.
fn first_free_row(canvas: &Canvas, (lo, hi): (Coord, Coord), from: Coord) -> Coord {
    let mut r = from;
    while (lo..=hi).any(|x| canvas.get(x, r).is_some()) {
        r += 1;
    }
    r
}

/// Ensure row `at_row` is free within band `[lo, hi]` by shifting the contiguous
/// occupied band-rows starting at `at_row` down by one, into the first fully-free
/// row at/below it. No-op when `at_row` is already free. Moving the whole occupied
/// stack into the first slack row is the cascade: stacked lines move together,
/// while a block already separated by a blank row below is left untouched.
pub fn make_room(canvas: &mut Canvas, (lo, hi): (Coord, Coord), at_row: Coord) {
    let free = first_free_row(canvas, (lo, hi), at_row);
    if free == at_row {
        return;
    }
    // Collect every populated band cell in rows [at_row, free-1], then re-place it
    // one row lower — collect-then-write so the shift can't clobber itself.
    let mut moved: Vec<(Coord, Coord, char)> = Vec::new();
    for y in at_row..free {
        for x in lo..=hi {
            if let Some(c) = canvas.get(x, y) {
                moved.push((x, y, c));
            }
        }
    }
    for &(x, y, _) in &moved {
        canvas.clear(x, y);
    }
    for (x, y, c) in moved {
        canvas.set(x, y + 1, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(c: &mut Canvas, x: Coord, y: Coord, s: &str) {
        for (i, ch) in s.chars().enumerate() {
            c.set(x + i as Coord, y, ch);
        }
    }

    /// Chars on row `y`, left-to-right (gaps collapsed — fine for contiguous rows).
    fn row(c: &Canvas, y: Coord) -> String {
        c.cells()
            .into_iter()
            .filter(|&(_, cy, _)| cy == y)
            .map(|(_, _, ch)| ch)
            .collect()
    }

    #[test]
    fn line_bounds_merges_single_spaces() {
        let mut c = Canvas::new();
        put(&mut c, 2, 0, "the cat"); // t2 h3 e4 _5 c6 a7 t8
        assert_eq!(line_bounds(&c, 4, 0), Some((2, 8)));
        assert_eq!(line_bounds(&c, 2, 0), Some((2, 8)));
        assert_eq!(line_bounds(&c, 5, 0), Some((2, 8))); // on the internal space
        assert_eq!(line_bounds(&c, 9, 0), Some((2, 8))); // end+1 typing position
        assert_eq!(line_bounds(&c, 10, 0), None); // past end+1
    }

    #[test]
    fn line_bounds_splits_on_double_gap() {
        let mut c = Canvas::new();
        put(&mut c, 0, 0, "the"); // 0..2
        put(&mut c, 5, 0, "cat"); // 5..7, cols 3 & 4 blank (2 gap)
        assert_eq!(line_bounds(&c, 1, 0), Some((0, 2)));
        assert_eq!(line_bounds(&c, 3, 0), Some((0, 2))); // end+1 of "the"
        assert_eq!(line_bounds(&c, 4, 0), None); // inside the 2-blank gap
        assert_eq!(line_bounds(&c, 6, 0), Some((5, 7)));
    }

    #[test]
    fn blank_row_has_no_line() {
        let c = Canvas::new();
        assert_eq!(line_bounds(&c, 5, 9), None);
    }

    #[test]
    fn make_room_noop_when_target_free() {
        let mut c = Canvas::new();
        put(&mut c, 0, 5, "abc");
        make_room(&mut c, (0, 2), 1); // row 1 already free
        assert_eq!(row(&c, 5), "abc");
        assert_eq!(c.get(0, 1), None);
    }

    #[test]
    fn make_room_shifts_one_occupied_row() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "abc");
        make_room(&mut c, (0, 2), 1);
        assert_eq!(c.get(0, 1), None);
        assert_eq!(row(&c, 2), "abc");
    }

    #[test]
    fn make_room_cascades_stacked_rows() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaa");
        put(&mut c, 0, 2, "bbb"); // no gap between rows 1 and 2; row 3 free
        make_room(&mut c, (0, 2), 1);
        assert_eq!(c.get(0, 1), None);
        assert_eq!(row(&c, 2), "aaa");
        assert_eq!(row(&c, 3), "bbb");
    }

    #[test]
    fn make_room_ignores_block_below_a_gap() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaa");
        put(&mut c, 0, 3, "ccc"); // row 2 is a blank gap
        make_room(&mut c, (0, 2), 1);
        assert_eq!(row(&c, 2), "aaa"); // shifted into the gap
        assert_eq!(row(&c, 3), "ccc"); // untouched
    }

    #[test]
    fn make_room_leaves_other_columns_untouched() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaa");
        put(&mut c, 10, 1, "NOTES"); // outside band (0..2)
        make_room(&mut c, (0, 2), 1);
        assert_eq!(c.get(10, 1), Some('N')); // side column stays on row 1
        assert_eq!(c.get(0, 1), None);
        assert_eq!(row(&c, 2), "aaa");
    }
}
