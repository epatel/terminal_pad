//! Render — paint the visible canvas window, cursor, and status line each frame.
//!
//! M3: draws the viewport window of the canvas + the terminal cursor + a status
//! line. The canvas drawing area is everything above a one-row status line; the
//! viewport's size is synced to that area here. See ./CLAUDE.md.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::Stylize,
    text::Line,
    widgets::Paragraph,
};

use crate::app::App;
use crate::canvas::Canvas;
use crate::viewport::Viewport;

/// Rows reserved at the bottom for the status line.
pub const STATUS_HEIGHT: u16 = 1;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [canvas_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(STATUS_HEIGHT)])
            .areas(frame.area());

    // The viewport tracks the canvas drawing area (status line excluded), so a
    // resize is absorbed here without a separate handler until M4 adds clamping.
    app.viewport.width = canvas_area.width;
    app.viewport.height = canvas_area.height;

    let rows = window_rows(&app.canvas, &app.viewport);
    frame.render_widget(Paragraph::new(rows.join("\n")), canvas_area);

    let marks: String = app
        .locations
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.map(|_| char::from_digit(i as u32 + 1, 10).unwrap()))
        .collect();
    let status = format!(
        " {}   cursor ({}, {})   cells {}   marks [{}]   {}   ·   ^S save · ^1-9 jump / ^⇧1-9 save · ^I mode · Esc/^Q quit ",
        app.mode.label(),
        app.cursor.0,
        app.cursor.1,
        app.canvas.len(),
        marks,
        app.status,
    );
    frame.render_widget(Paragraph::new(Line::from(status).reversed()), status_area);

    // Place the real terminal cursor on the canvas cell, if it's visible.
    if let Some((sx, sy)) = app.viewport.screen_of(app.cursor.0, app.cursor.1) {
        frame.set_cursor_position(Position::new(canvas_area.x + sx, canvas_area.y + sy));
    }
}

/// Build the visible window as one `String` per screen row, each `width` chars
/// wide, blanks for unwritten cells. Pure (no terminal I/O) so it can be tested.
pub fn window_rows(canvas: &Canvas, vp: &Viewport) -> Vec<String> {
    (0..vp.height)
        .map(|sy| {
            (0..vp.width)
                .map(|sx| {
                    let (x, y) = vp.canvas_at(sx, sy);
                    canvas.get(x, y).unwrap_or(' ')
                })
                .collect::<String>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_renders_cells_and_blanks() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, 'a');
        canvas.set(2, 1, 'b');
        let vp = Viewport {
            origin: (0, 0),
            width: 3,
            height: 2,
        };
        assert_eq!(
            window_rows(&canvas, &vp),
            vec!["a  ".to_string(), "  b".to_string()]
        );
    }

    #[test]
    fn window_respects_scrolled_origin() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, 'a'); // scrolled out of view
        canvas.set(3, 1, 'b');
        let vp = Viewport {
            origin: (1, 0),
            width: 3,
            height: 2,
        };
        // Columns shown are x=1,2,3. 'a' at x=0 is gone; 'b' at x=3 is rightmost.
        assert_eq!(
            window_rows(&canvas, &vp),
            vec!["   ".to_string(), "  b".to_string()]
        );
    }
}
