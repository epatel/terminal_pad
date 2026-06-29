//! Application state — the single struct the event loop and renderer share.
//!
//! Grows over the milestones: the canvas (M2), viewport (M3), cursor (M3, moved
//! in M4), edit mode (M5), and bookmark slots (M7) all hang off `App`.

use std::path::PathBuf;

use crate::canvas::{Canvas, Coord};
use crate::editing::EditMode;
use crate::locations::{Location, SLOT_COUNT};
use crate::overview::ZoomMode;
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
    /// Normal editor vs. zoomed-out overview (Ctrl+Z).
    pub zoom: ZoomMode,
    /// Whether the Ctrl+H keybinding cheat-sheet overlay is showing.
    pub help: bool,
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
            zoom: ZoomMode::Normal,
            help: false,
        }
    }

    /// Toggle between the normal editor and the zoomed-out overview (Ctrl+Z).
    pub fn toggle_zoom(&mut self) {
        self.zoom = match self.zoom {
            ZoomMode::Normal => ZoomMode::Overview,
            ZoomMode::Overview => ZoomMode::Normal,
        };
    }

    /// Move the cursor by `(dx, dy)` cells (arrow keys), then scroll the viewport
    /// just enough to keep the cursor visible. Navigation also resets the line
    /// anchor to the new column so a later Enter returns here.
    pub fn move_cursor(&mut self, dx: Coord, dy: Coord) {
        self.cursor = (self.cursor.0 + dx, self.cursor.1 + dy);
        self.anchor_x = self.cursor.0;
        self.viewport.scroll_to_show(self.cursor);
    }

    /// Jump the view by whole one-third steps (Shift+arrow), carrying the cursor
    /// the same distance so it keeps its screen position — reversing the jump
    /// lands the cursor back where it started.
    pub fn jump_view(&mut self, dx: i64, dy: i64) {
        let (sx, sy) = self.viewport.step();
        self.viewport.jump(dx, dy);
        self.cursor = (self.cursor.0 + dx * sx, self.cursor.1 + dy * sy);
        self.anchor_x = self.cursor.0;
    }

    /// Pan the view by whole screenfuls (overview arrows), carrying the cursor the
    /// same distance. Used for quick navigation while zoomed out.
    pub fn pan_view(&mut self, dx: i64, dy: i64) {
        let w = self.viewport.width.max(1) as Coord;
        let h = self.viewport.height.max(1) as Coord;
        let (ddx, ddy) = (dx * w, dy * h);
        self.viewport.origin = (self.viewport.origin.0 + ddx, self.viewport.origin.1 + ddy);
        self.cursor = (self.cursor.0 + ddx, self.cursor.1 + ddy);
        self.anchor_x = self.cursor.0;
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
    fn jump_view_carries_cursor_and_reverses() {
        let mut app = sized_app(30, 12); // step = (10, 4)
        app.jump_view(1, 0);
        assert_eq!(app.viewport.origin, (10, 0));
        assert_eq!(app.cursor, (10, 0)); // cursor jumped with the view
        app.jump_view(-1, 0);
        assert_eq!(app.viewport.origin, (0, 0));
        assert_eq!(app.cursor, (0, 0)); // reversing lands back at the start
    }

    #[test]
    fn pan_view_moves_view_and_cursor_by_screenfuls() {
        let mut app = sized_app(30, 12);
        app.pan_view(0, 1); // down one screen
        assert_eq!(app.viewport.origin, (0, 12));
        assert_eq!(app.cursor, (0, 12));
        app.pan_view(0, -1);
        assert_eq!(app.viewport.origin, (0, 0));
        assert_eq!(app.cursor, (0, 0));
    }
}
