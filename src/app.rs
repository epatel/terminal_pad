//! Application state — the single struct the event loop and renderer share.
//!
//! Grows over the milestones: the canvas (M2), viewport (M3), cursor (M3, moved
//! in M4), edit mode (M5), and bookmark slots (M7) all hang off `App`.

use std::path::PathBuf;

use crate::calc::CalcState;
use crate::canvas::{Canvas, Coord};
use crate::editing::EditMode;
use crate::locations::{Location, SLOT_COUNT};
use crate::overview::ZoomMode;
use crate::selection::{self, Selection};
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
    /// Current rectangular selection (left-click-drag), if any.
    pub selection: Option<Selection>,
    /// Whether a selection drag is in progress (between mouse-down and mouse-up).
    pub dragging: bool,
    /// Internal copy buffer: the last copied block as rows of chars (Ctrl+C → Ctrl+V).
    pub clip: Option<Vec<Vec<char>>>,
    /// Session-only calculator variables for `[Calc]` lines (not persisted).
    pub calc: CalcState,
    /// Whether banner (figlet big-letter) typing is on (Ctrl+B, not persisted).
    pub banner: bool,
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
            selection: None,
            dragging: false,
            clip: None,
            calc: CalcState::default(),
            banner: false,
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

    /// Place the cursor at the canvas cell under a click at screen offset
    /// `(sx, sy)` within the canvas drawing area. Resets the line anchor like the
    /// arrow keys; `scroll_to_show` is a no-op here (the cell is already visible).
    pub fn click_to(&mut self, sx: u16, sy: u16) {
        let (x, y) = self.viewport.canvas_at(sx, sy);
        self.cursor = (x, y);
        self.anchor_x = x;
        self.viewport.scroll_to_show(self.cursor);
    }

    /// Pan the view vertically by `d` rows (scroll wheel) without moving the
    /// cursor — the content scrolls under it, like a terminal's own scrollback.
    pub fn scroll_rows(&mut self, d: Coord) {
        self.viewport.origin.1 += d;
    }

    /// Pan the view horizontally by `d` columns (horizontal scroll wheel /
    /// trackpad swipe) without moving the cursor.
    pub fn scroll_cols(&mut self, d: Coord) {
        self.viewport.origin.0 += d;
    }

    /// Begin a selection drag: position the cursor at the click cell and start a
    /// zero-size selection anchored there.
    pub fn begin_drag(&mut self, sx: u16, sy: u16) {
        self.click_to(sx, sy);
        self.selection = Some(Selection::new(self.cursor));
        self.dragging = true;
    }

    /// Extend the active drag's far corner to the cell at `(sx, sy)`, carrying the
    /// cursor with it. No-op when no drag is in progress.
    pub fn update_drag(&mut self, sx: u16, sy: u16) {
        if !self.dragging {
            return;
        }
        let p = self.viewport.canvas_at(sx, sy);
        self.cursor = p;
        self.anchor_x = p.0;
        if let Some(sel) = &mut self.selection {
            sel.head = p;
        }
    }

    /// End the drag. A selection that never grew past one cell was really a click,
    /// so it's dropped (leaving just the positioned cursor).
    pub fn end_drag(&mut self) {
        self.dragging = false;
        if matches!(self.selection, Some(sel) if sel.is_point()) {
            self.selection = None;
        }
    }

    /// Copy the selection into the internal clip buffer and return its text form
    /// for the system clipboard (caller does the actual OS write). `None` when
    /// there's no selection.
    pub fn copy_selection(&mut self) -> Option<String> {
        let sel = self.selection?;
        let block = selection::extract(&self.canvas, &sel);
        let text = selection::to_text(&block);
        self.clip = Some(block);
        self.status = format!("copied {}x{} block", sel.width(), sel.height());
        Some(text)
    }

    /// Erase the selected rectangle and drop the selection. No-op when nothing is
    /// selected.
    pub fn delete_selection(&mut self) {
        if let Some(sel) = self.selection {
            selection::clear(&mut self.canvas, &sel);
            self.status = format!("cleared {}x{} block", sel.width(), sel.height());
            self.selection = None;
        }
    }

    /// Paste the internal clip buffer as a block anchored at the cursor: a blank
    /// source cell erases the destination, any other char overwrites it (so the
    /// rectangle lands cleanly). The cursor ends at the block's end, like paste.
    pub fn paste_clip(&mut self) {
        let Some(block) = self.clip.clone() else {
            return;
        };
        let (cx, cy) = self.cursor;
        let mut last_w = 0;
        for (i, rowcells) in block.iter().enumerate() {
            let y = cy + i as Coord;
            for (j, &ch) in rowcells.iter().enumerate() {
                let x = cx + j as Coord;
                if ch == ' ' {
                    self.canvas.clear(x, y);
                } else {
                    self.canvas.set(x, y, ch);
                }
            }
            last_w = rowcells.len() as Coord;
        }
        if !block.is_empty() {
            self.cursor = (cx + last_w, cy + block.len() as Coord - 1);
        }
        self.selection = None;
        self.viewport.scroll_to_show(self.cursor);
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
    fn click_positions_cursor_at_canvas_cell() {
        let mut app = sized_app(80, 24);
        app.viewport.origin = (10, 5);
        app.click_to(3, 2);
        assert_eq!(app.cursor, (13, 7)); // origin + screen offset
        assert_eq!(app.anchor_x, 13); // anchor follows, like arrow nav
    }

    #[test]
    fn scroll_rows_pans_view_without_moving_cursor() {
        let mut app = sized_app(80, 24);
        app.cursor = (4, 4);
        app.scroll_rows(3);
        assert_eq!(app.viewport.origin, (0, 3));
        assert_eq!(app.cursor, (4, 4)); // cursor stays put
        app.scroll_rows(-3);
        assert_eq!(app.viewport.origin, (0, 0));
    }

    #[test]
    fn drag_makes_a_rectangle_then_click_clears_it() {
        let mut app = sized_app(80, 24);
        app.begin_drag(2, 1); // anchor at canvas (2,1)
        app.update_drag(5, 3); // head at (5,3)
        let sel = app.selection.unwrap();
        assert_eq!(sel.bounds(), (2, 1, 5, 3));
        assert_eq!(app.cursor, (5, 3)); // cursor follows the drag head
        app.end_drag();
        assert!(app.selection.is_some()); // a real rectangle survives

        // A click (down then up, no drag) leaves no selection.
        app.begin_drag(7, 7);
        app.end_drag();
        assert!(app.selection.is_none());
        assert_eq!(app.cursor, (7, 7));
    }

    #[test]
    fn copy_fills_clip_and_returns_text() {
        let mut app = sized_app(80, 24);
        app.canvas.set(0, 0, 'h');
        app.canvas.set(1, 0, 'i');
        app.selection = Some(Selection {
            anchor: (0, 0),
            head: (1, 0),
        });
        let text = app.copy_selection();
        assert_eq!(text.as_deref(), Some("hi"));
        assert_eq!(app.clip, Some(vec![vec!['h', 'i']]));
    }

    #[test]
    fn delete_selection_clears_block_and_drops_selection() {
        let mut app = sized_app(80, 24);
        app.canvas.set(0, 0, 'x');
        app.canvas.set(1, 0, 'y');
        app.selection = Some(Selection {
            anchor: (0, 0),
            head: (1, 0),
        });
        app.delete_selection();
        assert_eq!(app.canvas.get(0, 0), None);
        assert_eq!(app.canvas.get(1, 0), None);
        assert!(app.selection.is_none());
    }

    #[test]
    fn paste_clip_writes_block_and_erases_blanks() {
        let mut app = sized_app(80, 24);
        app.clip = Some(vec![vec!['a', 'b'], vec![' ', 'c']]); // blank at lower-left
        app.canvas.set(3, 6, 'Z'); // will be under the blank source cell
        app.cursor = (3, 5);
        app.paste_clip();
        assert_eq!(app.canvas.get(3, 5), Some('a'));
        assert_eq!(app.canvas.get(4, 5), Some('b'));
        assert_eq!(app.canvas.get(3, 6), None); // blank source erased the 'Z'
        assert_eq!(app.canvas.get(4, 6), Some('c'));
        assert_eq!(app.cursor, (5, 6)); // end of the block
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
