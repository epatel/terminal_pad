//! Application state — the single struct the event loop and renderer share.
//!
//! Grows over the milestones: the canvas (M2), viewport (M3), cursor (M3, moved
//! in M4), edit mode (M5), and bookmark slots (M7) all hang off `App`.

use std::path::PathBuf;

use crate::canvas::{Canvas, Coord};
use crate::editing::EditMode;
use crate::locations::{Location, SLOT_COUNT};
use crate::viewport::Viewport;

pub struct App {
    pub canvas: Canvas,
    pub viewport: Viewport,
    /// Absolute canvas position where the next edit lands and the cursor draws.
    pub cursor: (Coord, Coord),
    /// Insert vs Overwrite (Ctrl+I).
    pub mode: EditMode,
    /// Column the next Enter returns to — the current line's start, set whenever
    /// navigation repositions the cursor.
    pub anchor_x: Coord,
    /// Bookmark slots, bound to Ctrl+1..9 (jump) / Ctrl+Shift+1..9 (save).
    pub locations: [Option<Location>; SLOT_COUNT],
    /// File the canvas is loaded from / saved to.
    pub path: PathBuf,
    /// Transient message shown in the status line (e.g. save result).
    pub status: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            canvas: Canvas::new(),
            viewport: Viewport::new(),
            cursor: (0, 0),
            mode: EditMode::Insert,
            anchor_x: 0,
            locations: [None; SLOT_COUNT],
            path: PathBuf::from("canvas.tpad"),
            status: String::new(),
        }
    }

    /// Move the cursor by `(dx, dy)` cells (arrow keys), then scroll the viewport
    /// just enough to keep the cursor visible. Navigation also resets the line
    /// anchor to the new column so a later Enter returns here.
    pub fn move_cursor(&mut self, dx: Coord, dy: Coord) {
        self.cursor = (self.cursor.0 + dx, self.cursor.1 + dy);
        self.anchor_x = self.cursor.0;
        self.viewport.scroll_to_show(self.cursor);
    }

    /// Jump the view by whole one-third steps (Shift+arrow). The cursor stays put.
    pub fn jump_view(&mut self, dx: i64, dy: i64) {
        self.viewport.jump(dx, dy);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized_app(w: u16, h: u16) -> App {
        let mut app = App::new();
        app.viewport.width = w;
        app.viewport.height = h;
        app
    }

    #[test]
    fn move_cursor_scrolls_view_to_follow() {
        let mut app = sized_app(10, 5);
        for _ in 0..12 {
            app.move_cursor(1, 0);
        }
        assert_eq!(app.cursor, (12, 0));
        // Cursor-follow keeps it visible.
        assert!(app.viewport.screen_of(12, 0).is_some());
    }

    #[test]
    fn jump_view_moves_origin_not_cursor() {
        let mut app = sized_app(30, 12);
        app.jump_view(1, 0);
        assert_eq!(app.viewport.origin, (10, 0));
        assert_eq!(app.cursor, (0, 0)); // cursor untouched by a view jump
    }
}
