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

/// The segments (lines) on row `y`, left to right, as inclusive `(start, end)`
/// column spans. A segment is a single-space-joined run; a gap of ≥2 blank
/// columns separates two segments.
fn segments_on_row(canvas: &Canvas, y: Coord) -> Vec<(Coord, Coord)> {
    let cols = filled_columns(canvas, y);
    let mut segs = Vec::new();
    let Some(&first) = cols.first() else {
        return segs;
    };
    let (mut start, mut prev) = (first, first);
    for &x in &cols[1..] {
        if x - prev >= 3 {
            segs.push((start, prev));
            start = x;
        }
        prev = x;
    }
    segs.push((start, prev));
    segs
}

/// Two inclusive column spans overlap.
fn overlaps((a, b): (Coord, Coord), (c, d): (Coord, Coord)) -> bool {
    a <= d && c <= b
}

/// Bounds `(start, end)` of the single-space-joined line on row `cy` containing
/// the cursor column `cx`. The cursor counts as "on" the line for
/// `start <= cx <= end + 1` (so the typing position just past the end still
/// belongs to it). Returns `None` when `cx` is not on a line — a blank row, or
/// parked in a gap of ≥2 blank columns.
pub fn line_bounds(canvas: &Canvas, cx: Coord, cy: Coord) -> Option<(Coord, Coord)> {
    segments_on_row(canvas, cy)
        .into_iter()
        .find(|&(s, e)| (s..=e + 1).contains(&cx))
}

/// Segments displaced when content spanning `span` lands on row `landing`:
/// the overlapping segments on `landing` itself, or — when `landing` holds none
/// but the row just below does — those on `landing + 1`, so a single blank
/// separator row travels down with its block instead of being consumed. Two
/// consecutive blank rows absorb the shift (the vertical analogue of the
/// ≥2-blank-column gap that ends a line).
fn displaced(canvas: &Canvas, span: (Coord, Coord), landing: Coord) -> Vec<(Coord, Coord, Coord)> {
    for row in [landing, landing + 1] {
        let hits: Vec<(Coord, Coord, Coord)> = segments_on_row(canvas, row)
            .into_iter()
            .filter(|&seg| overlaps(seg, span))
            .map(|(s, e)| (s, e, row))
            .collect();
        if !hits.is_empty() {
            return hits;
        }
    }
    Vec::new()
}

/// Open row `at_row` across the band `[lo, hi]` by pushing **whole lines** down.
/// Every segment on `at_row` (or, when `at_row` is free, on `at_row + 1` — a
/// single blank separator moves with its block) that overlaps the band moves
/// down one row; any segment it would land on or trail by one blank row moves
/// too, cascading down until the shift is absorbed by a gap of ≥2 blank rows.
/// Lines move as units — a wide line below a narrow band is not torn — while
/// segments that never overlap (a side column past a ≥2-blank gap) stay put.
/// No-op when the band is free on `at_row` and the row below it.
pub fn make_room(canvas: &mut Canvas, band: (Coord, Coord), at_row: Coord) {
    // Flood downward: collect every segment that must shift down by one row.
    let mut to_move: Vec<(Coord, Coord, Coord)> = Vec::new(); // (start, end, row)
    let mut queue: Vec<(Coord, Coord, Coord)> = displaced(canvas, band, at_row);
    while let Some(seg @ (s, e, row)) = queue.pop() {
        if to_move.contains(&seg) {
            continue;
        }
        to_move.push(seg);
        queue.extend(displaced(canvas, (s, e), row + 1));
    }
    if to_move.is_empty() {
        return;
    }
    // Collect every populated cell of the moving segments, then re-place it one row
    // lower — collect-then-write so the shift can't clobber itself. Segments on a
    // row never share columns, so targets never collide.
    let mut cells: Vec<(Coord, Coord, char)> = Vec::new();
    for &(s, e, row) in &to_move {
        for x in s..=e {
            if let Some(c) = canvas.get(x, row) {
                cells.push((x, row, c));
            }
        }
    }
    for &(x, y, _) in &cells {
        canvas.clear(x, y);
    }
    for (x, y, c) in cells {
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
    fn make_room_carries_single_blank_separator_down() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaa");
        put(&mut c, 0, 3, "ccc"); // row 2 is a single-blank separator
        make_room(&mut c, (0, 2), 1);
        assert_eq!(row(&c, 2), "aaa");
        assert_eq!(c.get(0, 3), None); // separator preserved, one row lower
        assert_eq!(row(&c, 4), "ccc");
    }

    #[test]
    fn make_room_stops_at_two_blank_rows() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaa");
        put(&mut c, 0, 4, "ccc"); // rows 2 and 3 blank: a ≥2-row gap
        make_room(&mut c, (0, 2), 1);
        assert_eq!(row(&c, 2), "aaa"); // shift absorbed by the gap
        assert_eq!(row(&c, 4), "ccc"); // untouched, still ≥1 blank row apart
    }

    #[test]
    fn make_room_seeds_past_blank_target_row() {
        let mut c = Canvas::new();
        put(&mut c, 0, 2, "bbb"); // at_row 1 itself is blank
        make_room(&mut c, (0, 2), 1);
        assert_eq!(c.get(0, 2), None); // row 2 freed so row 1's blank survives
        assert_eq!(row(&c, 3), "bbb");
    }

    #[test]
    fn make_room_pushes_whole_wide_line_not_just_band() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "the quick brown fox"); // one line, cols 0..=18
        make_room(&mut c, (0, 4), 1); // band narrower than the line
        assert_eq!(c.get(0, 1), None); // band freed on row 1
        assert_eq!(c.get(18, 1), None); // and so is the far end — moved as a unit
        assert_eq!(row(&c, 2), "the quick brown fox"); // whole line on row 2, not torn
    }

    #[test]
    fn make_room_cascades_whole_stacked_lines() {
        let mut c = Canvas::new();
        put(&mut c, 0, 1, "aaaaaaaaaa"); // wide lines, stacked, no gap
        put(&mut c, 0, 2, "bbbbbbbbbb");
        make_room(&mut c, (0, 2), 1); // narrow band
        assert_eq!(c.get(0, 1), None);
        assert_eq!(row(&c, 2), "aaaaaaaaaa"); // both whole lines cascaded down
        assert_eq!(row(&c, 3), "bbbbbbbbbb");
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
