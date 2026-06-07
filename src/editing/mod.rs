//! Editing — cursor writes, insert/overwrite mode, deletion, and newline (M5).
//!
//! Operations act on the shared `App` state: they decide *which* canvas op and
//! *where*, while the canvas model decides *how* a cell is stored. See
//! ./CLAUDE.md for the contract.

use crate::app::App;

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

/// Move to the start column of the next row. The "start column" is the line
/// anchor — the column where text entry on the current line began (set whenever
/// the cursor is repositioned by navigation).
pub fn newline(app: &mut App) {
    app.cursor = (app.anchor_x, app.cursor.1 + 1);
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
/// last pasted line. `\r\n` line endings are normalized to `\n`.
pub fn paste(app: &mut App, text: &str) {
    let (cx, cy) = app.cursor;
    let mut last_line_len = 0;
    let mut line_count = 0;
    for (i, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
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
        let mut app = App::new();
        app.move_cursor(5, 2); // navigate -> sets anchor_x = 5
        type_str(&mut app, "hi"); // cursor (7, 2), anchor still 5
        newline(&mut app);
        assert_eq!(app.cursor, (5, 3));
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
}
