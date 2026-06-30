//! Editing — cursor writes, insert/overwrite mode, deletion, and newline (M5).
//!
//! Operations act on the shared `App` state: they decide *which* canvas op and
//! *where*, while the canvas model decides *how* a cell is stored. See
//! ./CLAUDE.md for the contract.

use crate::app::App;
use crate::canvas::{Canvas, Coord};

/// Whether typing inserts (shifting the row right) or overwrites in place.
/// Toggled by Ctrl+I.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Insert,
    Overwrite,
}

impl EditMode {
    pub fn toggled(self) -> Self {
        match self {
            EditMode::Insert => EditMode::Overwrite,
            EditMode::Overwrite => EditMode::Insert,
        }
    }

    /// Short label for the status line.
    pub fn label(self) -> &'static str {
        match self {
            EditMode::Insert => "INS",
            EditMode::Overwrite => "OVR",
        }
    }
}

/// Write a printable char at the cursor and advance one column. Insert shifts the
/// row's trailing cells right; Overwrite replaces in place.
pub fn type_char(app: &mut App, ch: char) {
    match app.mode {
        EditMode::Insert => app.canvas.insert_shift(app.cursor.0, app.cursor.1, ch),
        EditMode::Overwrite => app.canvas.set(app.cursor.0, app.cursor.1, ch),
    }
    app.cursor.0 += 1;
    app.viewport.scroll_to_show(app.cursor);
}

/// Delete the cell before the cursor and move left.
pub fn backspace(app: &mut App) {
    app.cursor.0 -= 1;
    delete_at_cursor(app);
    app.viewport.scroll_to_show(app.cursor);
}

/// Delete the cell under the cursor; the cursor stays put.
pub fn delete(app: &mut App) {
    delete_at_cursor(app);
    app.viewport.scroll_to_show(app.cursor);
}

/// Split the current "line" at the cursor (Enter). The trailing run — words
/// joined by single spaces, up to the first ≥2-blank gap — moves down one row,
/// left-aligned to the line's start column, with the block(s) below pushed down
/// to make room (`layout::make_room`). When the cursor is at/after the line's end
/// the cursor just drops to the line's start on the next row; on a blank row it
/// falls back to the saved anchor column. See `src/layout/`.
pub fn newline(app: &mut App) {
    let (cx, cy) = app.cursor;
    match crate::layout::line_bounds(&app.canvas, cx, cy) {
        Some((s, e)) if cx <= e => {
            // Capture the trailing run before mutating (make_room only touches
            // cy+1 and below, so the source row cy is safe to read here).
            let run: Vec<Option<char>> = (cx..=e).map(|x| app.canvas.get(x, cy)).collect();
            let band = (s, s + (e - cx));
            crate::layout::make_room(&mut app.canvas, band, cy + 1);
            for x in cx..=e {
                app.canvas.clear(x, cy);
            }
            for (i, ch) in run.into_iter().enumerate() {
                let x = s + i as Coord;
                match ch {
                    Some(c) => app.canvas.set(x, cy + 1, c),
                    None => app.canvas.clear(x, cy + 1),
                }
            }
            app.cursor = (s, cy + 1);
            app.anchor_x = s;
        }
        Some((s, _)) => {
            app.cursor = (s, cy + 1);
            app.anchor_x = s;
        }
        None => {
            app.cursor = (app.anchor_x, cy + 1);
        }
    }
    app.viewport.scroll_to_show(app.cursor);
}

/// Jump the cursor to the start of the next word on its row (Option/Alt+Right).
/// A word is a maximal run of non-whitespace cells; blanks and whitespace cells
/// separate them. From inside a word this lands on the next word's first cell;
/// past the last word it lands one column after the final word char
/// (end-of-content). No movement on a row with no words.
pub fn word_right(app: &mut App) {
    let (x, y) = app.cursor;
    let starts = word_starts(&app.canvas, y);
    let target = starts.iter().copied().find(|&s| s > x).or_else(|| {
        last_populated(&app.canvas, y)
            .map(|m| m + 1)
            .filter(|&t| t > x)
    });
    if let Some(tx) = target {
        move_cursor_x(app, tx);
    }
}

/// Jump the cursor to the start of the previous word on its row (Option/Alt+Left).
/// From inside a word this lands on that word's first cell; from a word start it
/// lands on the previous word's start. No movement when no word lies to the left.
pub fn word_left(app: &mut App) {
    let (x, y) = app.cursor;
    let starts = word_starts(&app.canvas, y);
    if let Some(tx) = starts.iter().copied().rev().find(|&s| s < x) {
        move_cursor_x(app, tx);
    }
}

/// Word-character columns on row `y`, ascending: populated cells whose char is
/// not whitespace. A typed space *is* a stored cell, so whitespace — not just an
/// absent cell — separates words. `canvas.cells()` is sorted by `(y, x)`.
fn word_positions(canvas: &Canvas, y: Coord) -> Vec<Coord> {
    canvas
        .cells()
        .into_iter()
        .filter(|&(_, cy, c)| cy == y && !c.is_whitespace())
        .map(|(x, _, _)| x)
        .collect()
}

/// Columns that begin a word on row `y`: a word column whose left neighbor is not
/// a word column (a gap, a blank, or whitespace marks the boundary).
fn word_starts(canvas: &Canvas, y: Coord) -> Vec<Coord> {
    let mut starts = Vec::new();
    let mut prev: Option<Coord> = None;
    for x in word_positions(canvas, y) {
        if prev != Some(x - 1) {
            starts.push(x);
        }
        prev = Some(x);
    }
    starts
}

/// Rightmost word column on row `y`, or `None` if the row has no word chars.
fn last_populated(canvas: &Canvas, y: Coord) -> Option<Coord> {
    word_positions(canvas, y).into_iter().next_back()
}

/// Reposition the cursor to column `tx` (same row), resetting the line anchor and
/// scrolling to keep it visible — matching `App::move_cursor`'s navigation rules.
fn move_cursor_x(app: &mut App, tx: Coord) {
    app.cursor.0 = tx;
    app.anchor_x = tx;
    app.viewport.scroll_to_show(app.cursor);
}

/// Flip Insert/Overwrite (Ctrl+I). Touches no canvas cells.
pub fn toggle_mode(app: &mut App) {
    app.mode = app.mode.toggled();
}

/// Drop pasted text as a rectangular block anchored at the cursor: line `i` of
/// the paste lands at `cursor.y + i`, starting at `cursor.x`. v1 = block
/// placement — it overwrites the cells it covers (regardless of Insert/Overwrite)
/// and does not push existing content down. The cursor ends at the end of the
/// last pasted line. `\r\n` and bare `\r` line endings are normalized to `\n`.
pub fn paste(app: &mut App, text: &str) {
    let (cx, cy) = app.cursor;
    // Normalize line endings before splitting. Terminals commonly send bracketed
    // paste with bare `\r` (or `\r\n`) as the line break, not `\n` — without this
    // a multi-line paste collapses onto a single row.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut last_line_len = 0;
    let mut line_count = 0;
    for (i, line) in normalized.split('\n').enumerate() {
        let y = cy + i as crate::canvas::Coord;
        let mut x = cx;
        for ch in line.chars() {
            app.canvas.set(x, y, ch);
            x += 1;
        }
        last_line_len = x - cx;
        line_count = i as crate::canvas::Coord + 1;
    }
    if line_count > 0 {
        app.cursor = (cx + last_line_len, cy + line_count - 1);
    }
    app.viewport.scroll_to_show(app.cursor);
}

/// Insert mode reflows the row (pull trailing cells left); Overwrite just erases
/// the cell, leaving a gap.
fn delete_at_cursor(app: &mut App) {
    match app.mode {
        EditMode::Insert => app.canvas.delete_shift(app.cursor.0, app.cursor.1),
        EditMode::Overwrite => app.canvas.clear(app.cursor.0, app.cursor.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Coord;

    fn row(app: &App, y: Coord) -> String {
        app.canvas
            .cells()
            .into_iter()
            .filter(|&(_, cy, _)| cy == y)
            .map(|(_, _, c)| c)
            .collect()
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            type_char(app, c);
        }
    }

    #[test]
    fn insert_mode_writes_and_advances() {
        let mut app = App::new();
        type_str(&mut app, "hi");
        assert_eq!(row(&app, 0), "hi");
        assert_eq!(app.cursor, (2, 0));
    }

    #[test]
    fn insert_mode_pushes_existing_right() {
        let mut app = App::new();
        type_str(&mut app, "ac");
        app.cursor = (1, 0); // between a and c
        type_char(&mut app, 'b');
        assert_eq!(row(&app, 0), "abc");
        assert_eq!(app.cursor, (2, 0));
    }

    #[test]
    fn overwrite_mode_replaces_in_place() {
        let mut app = App::new();
        type_str(&mut app, "cat");
        app.cursor = (0, 0);
        app.mode = EditMode::Overwrite;
        type_str(&mut app, "b");
        assert_eq!(row(&app, 0), "bat");
    }

    #[test]
    fn backspace_in_insert_reflows() {
        let mut app = App::new();
        type_str(&mut app, "abc"); // cursor at (3,0)
        backspace(&mut app); // remove 'c'
        assert_eq!(row(&app, 0), "ab");
        assert_eq!(app.cursor, (2, 0));
    }

    #[test]
    fn delete_under_cursor_pulls_left_in_insert() {
        let mut app = App::new();
        type_str(&mut app, "abc");
        app.cursor = (1, 0); // on 'b'
        delete(&mut app); // remove 'b'
        assert_eq!(row(&app, 0), "ac");
        assert_eq!(app.cursor, (1, 0)); // unchanged
    }

    #[test]
    fn delete_in_overwrite_leaves_gap() {
        let mut app = App::new();
        type_str(&mut app, "abc");
        app.mode = EditMode::Overwrite;
        app.cursor = (1, 0);
        delete(&mut app);
        assert_eq!(app.canvas.get(0, 0), Some('a'));
        assert_eq!(app.canvas.get(1, 0), None); // gap, not pulled left
        assert_eq!(app.canvas.get(2, 0), Some('c'));
    }

    #[test]
    fn newline_returns_to_line_anchor() {
        // Cursor at the line's end+1: nothing to split, drops to the start column.
        let mut app = App::new();
        app.move_cursor(5, 2); // navigate -> sets anchor_x = 5
        type_str(&mut app, "hi"); // cursor (7, 2)
        newline(&mut app);
        assert_eq!(app.cursor, (5, 3));
    }

    #[test]
    fn newline_splits_line_and_moves_tail_to_start() {
        let mut app = App::new();
        app.move_cursor(2, 0); // anchor 2
        type_str(&mut app, "the cat sat"); // cols 2..=12
        app.cursor = (6, 0); // on 'c' of "cat"
        newline(&mut app);
        assert_eq!(row(&app, 0), "the "); // cols 2..=5 stay (incl. trailing space)
        assert_eq!(app.canvas.get(6, 0), None); // tail cleared
        assert_eq!(row(&app, 1), "cat sat"); // moved to the line start (col 2)
        assert_eq!(app.cursor, (2, 1));
    }

    #[test]
    fn newline_at_line_start_moves_whole_line_down() {
        let mut app = App::new();
        app.move_cursor(0, 0);
        type_str(&mut app, "hello");
        app.cursor = (0, 0);
        newline(&mut app);
        assert_eq!(app.canvas.get(0, 0), None); // row 0 emptied
        assert_eq!(row(&app, 1), "hello");
        assert_eq!(app.cursor, (0, 1));
    }

    #[test]
    fn newline_pushes_block_below_and_keeps_side_column() {
        let mut app = App::new();
        app.move_cursor(0, 0);
        type_str(&mut app, "the cat"); // row 0, cols 0..=6
        for (i, ch) in "dog".chars().enumerate() {
            app.canvas.set(i as Coord, 1, ch); // block directly below, in-band
        }
        for (i, ch) in "NOTES".chars().enumerate() {
            app.canvas.set(10 + i as Coord, 1, ch); // side column past a 2-blank gap
        }
        app.cursor = (4, 0); // on 'c' of "cat"
        newline(&mut app);
        assert_eq!(app.canvas.get(4, 0), None); // tail cleared on row 0
        // Tail lands at the line start on row 1...
        assert_eq!(app.canvas.get(0, 1), Some('c'));
        assert_eq!(app.canvas.get(2, 1), Some('t'));
        // ...the "dog" block is pushed down to row 2...
        assert_eq!(app.canvas.get(0, 2), Some('d'));
        assert_eq!(app.canvas.get(2, 2), Some('g'));
        // ...and the NOTES side column is untouched on row 1.
        assert_eq!(app.canvas.get(10, 1), Some('N'));
        assert_eq!(app.canvas.get(14, 1), Some('S'));
    }

    #[test]
    fn toggle_mode_flips() {
        let mut app = App::new();
        assert_eq!(app.mode, EditMode::Insert);
        toggle_mode(&mut app);
        assert_eq!(app.mode, EditMode::Overwrite);
        toggle_mode(&mut app);
        assert_eq!(app.mode, EditMode::Insert);
    }

    #[test]
    fn word_right_steps_word_to_word_then_to_end() {
        let mut app = App::new();
        type_str(&mut app, "foo bar baz"); // words at 0, 4, 8; ends at 11
        app.cursor = (0, 0);
        word_right(&mut app);
        assert_eq!(app.cursor.0, 4); // start of "bar"
        word_right(&mut app);
        assert_eq!(app.cursor.0, 8); // start of "baz"
        word_right(&mut app);
        assert_eq!(app.cursor.0, 11); // one past the last cell (end-of-content)
        word_right(&mut app);
        assert_eq!(app.cursor.0, 11); // nothing further → no movement
    }

    #[test]
    fn word_right_from_mid_word_jumps_to_next_word() {
        let mut app = App::new();
        type_str(&mut app, "foo bar");
        app.cursor = (1, 0); // inside "foo"
        word_right(&mut app);
        assert_eq!(app.cursor.0, 4); // start of "bar", not end of "foo"
    }

    #[test]
    fn word_left_lands_on_word_starts() {
        let mut app = App::new();
        type_str(&mut app, "foo bar baz");
        app.cursor = (11, 0); // end-of-content
        word_left(&mut app);
        assert_eq!(app.cursor.0, 8); // "baz"
        word_left(&mut app);
        assert_eq!(app.cursor.0, 4); // "bar"
        word_left(&mut app);
        assert_eq!(app.cursor.0, 0); // "foo"
        word_left(&mut app);
        assert_eq!(app.cursor.0, 0); // nothing to the left → no movement
    }

    #[test]
    fn word_left_from_mid_word_goes_to_its_start() {
        let mut app = App::new();
        type_str(&mut app, "foo bar");
        app.cursor = (5, 0); // inside "bar"
        word_left(&mut app);
        assert_eq!(app.cursor.0, 4); // start of "bar"
    }

    #[test]
    fn word_jump_handles_gaps_and_empty_rows() {
        let mut app = App::new();
        // Words separated by a multi-cell gap: "ab" at 0-1, "cd" at 5-6.
        app.canvas.set(0, 0, 'a');
        app.canvas.set(1, 0, 'b');
        app.canvas.set(5, 0, 'c');
        app.canvas.set(6, 0, 'd');
        app.cursor = (0, 0);
        word_right(&mut app);
        assert_eq!(app.cursor.0, 5); // skips the gap to "cd"
        // Empty row: no movement either way.
        app.cursor = (3, 9);
        word_right(&mut app);
        assert_eq!(app.cursor, (3, 9));
        word_left(&mut app);
        assert_eq!(app.cursor, (3, 9));
    }

    #[test]
    fn paste_single_line_at_cursor() {
        let mut app = App::new();
        app.move_cursor(2, 1); // cursor (2, 1)
        paste(&mut app, "abc");
        assert_eq!(row(&app, 1), "abc"); // at x = 2,3,4
        assert_eq!(app.cursor, (5, 1)); // end of the line
    }

    #[test]
    fn paste_multiline_block_lands_cursor_at_end() {
        let mut app = App::new();
        paste(&mut app, "ab\ncde");
        assert_eq!(row(&app, 0), "ab");
        assert_eq!(row(&app, 1), "cde");
        assert_eq!(app.cursor, (3, 1)); // end of last line
    }

    #[test]
    fn paste_overwrites_covered_cells_only() {
        let mut app = App::new();
        type_str(&mut app, "XXXXX"); // row 0 full of X
        app.cursor = (1, 0);
        paste(&mut app, "ab"); // overwrite cols 1,2
        assert_eq!(row(&app, 0), "XabXX");
    }

    #[test]
    fn paste_normalizes_crlf() {
        let mut app = App::new();
        paste(&mut app, "a\r\nb");
        assert_eq!(row(&app, 0), "a");
        assert_eq!(row(&app, 1), "b");
    }

    #[test]
    fn paste_splits_on_bare_cr() {
        let mut app = App::new();
        paste(&mut app, "a\rb\rc"); // terminal-style line breaks
        assert_eq!(row(&app, 0), "a");
        assert_eq!(row(&app, 1), "b");
        assert_eq!(row(&app, 2), "c");
        assert_eq!(app.cursor, (1, 2));
    }
}
