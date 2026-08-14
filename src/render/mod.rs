//! Render — paint the visible canvas window, cursor, and status line each frame.
//!
//! M3: draws the viewport window of the canvas + the terminal cursor + a status
//! line. The canvas drawing area is everything above a one-row status line; the
//! viewport's size is synced to that area here. See ./CLAUDE.md.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::canvas::Canvas;
use crate::help;
use crate::overview::{self, ZoomMode};
use crate::selection::Selection;
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

    if app.help {
        let panel = help::rows(canvas_area.width, canvas_area.height);
        frame.render_widget(Paragraph::new(panel.join("\n")), canvas_area);
        let status = format!(" HELP   ·   {}   ·   press any key to close ", app.status);
        frame.render_widget(Paragraph::new(Line::from(status).reversed()), status_area);
        return; // no text cursor while the cheat sheet is up
    }

    if app.zoom == ZoomMode::Overview {
        let map = overview::rows(
            &app.canvas,
            &app.viewport,
            canvas_area.width,
            canvas_area.height,
        );
        frame.render_widget(Paragraph::new(map.join("\n")), canvas_area);
        let status = format!(
            " OVERVIEW   {} cells   {}   ·   ^Z back · ^S save · Esc/^Q quit ",
            app.canvas.len(),
            app.status,
        );
        frame.render_widget(Paragraph::new(Line::from(status).reversed()), status_area);
        return; // no text cursor in the overview
    }

    let lines = canvas_lines(&app.canvas, &app.viewport, app.selection.as_ref());
    frame.render_widget(Paragraph::new(lines), canvas_area);

    let marks: String = app
        .locations
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.map(|_| char::from_digit(i as u32 + 1, 10).unwrap()))
        .collect();
    let sel = match app.selection {
        Some(s) => format!("   sel {}x{}", s.width(), s.height()),
        None => String::new(),
    };
    // The mode chip grows a BAN tag while banner (figlet) typing is on.
    let mode = if app.banner {
        format!("{}+BAN", app.mode.label())
    } else {
        app.mode.label().to_string()
    };
    let status = format!(
        " {}   cursor ({}, {})   cells {}   marks [{}]{}   {}   ·   ^H help · ^C/^V copy/paste · ^S save · Esc/^Q quit ",
        mode,
        app.cursor.0,
        app.cursor.1,
        app.canvas.len(),
        marks,
        sel,
        app.status,
    );
    frame.render_widget(Paragraph::new(Line::from(status).reversed()), status_area);

    // Place the real terminal cursor on the canvas cell, if it's visible.
    if let Some((sx, sy)) = app.viewport.screen_of(app.cursor.0, app.cursor.1) {
        frame.set_cursor_position(Position::new(canvas_area.x + sx, canvas_area.y + sy));
    }
}

/// Build the visible window as one `String` per screen row, each exactly `width`
/// *columns* wide (not chars — a double-width char is one char in two columns),
/// blanks for unwritten cells. Pure (no terminal I/O) so it can be tested.
///
/// A double-width char's glyph physically covers the next column, so the cell
/// there is skipped rather than drawn — that is what keeps every later column on
/// the row at its true screen position. Writers leave that cell blank anyway
/// (see `editing::type_char`); skipping it also keeps a stale char, left by an
/// edit around a wide glyph, from pushing the rest of the row sideways. A wide
/// char in the last visible column would spill past the area, so it draws as a
/// blank instead.
pub fn window_rows(canvas: &Canvas, vp: &Viewport) -> Vec<String> {
    (0..vp.height)
        .map(|sy| {
            let mut row = String::new();
            let mut sx = 0;
            while sx < vp.width {
                let (x, y) = vp.canvas_at(sx, sy);
                let ch = canvas.get(x, y).unwrap_or(' ');
                if crate::width::is_wide(ch) {
                    if sx + 1 < vp.width {
                        row.push(ch);
                        sx += 2; // the glyph covers the next column too
                        continue;
                    }
                    row.push(' '); // would spill past the right edge
                } else {
                    row.push(ch);
                }
                sx += 1;
            }
            row
        })
        .collect()
}

/// Build the visible window as styled `Line`s, reversing the cells inside the
/// selection rectangle. Without a selection each row is a single plain span.
fn canvas_lines(canvas: &Canvas, vp: &Viewport, sel: Option<&Selection>) -> Vec<Line<'static>> {
    let rows = window_rows(canvas, vp);
    rows.into_iter()
        .enumerate()
        .map(|(sy, row)| match sel {
            Some(sel) => style_row(&row, sy as u16, vp, sel),
            None => Line::from(row),
        })
        .collect()
}

/// Split one screen row into spans, reversing runs of cells that fall inside the
/// selection. Consecutive cells of the same state are merged into one span.
///
/// `row` comes from [`window_rows`], so a char is not always a column: the
/// column advances by the char's display width, and a double-width char takes
/// the selection state of the column it starts in.
fn style_row(row: &str, sy: u16, vp: &Viewport, sel: &Selection) -> Line<'static> {
    let (_, y) = vp.canvas_at(0, sy);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_selected = false;
    let mut sx: i64 = 0;
    for ch in row.chars() {
        let x = vp.origin.0 + sx;
        sx += crate::width::cols(ch);
        let selected = sel.contains(x, y);
        if selected != buf_selected && !buf.is_empty() {
            spans.push(make_span(std::mem::take(&mut buf), buf_selected));
        }
        buf_selected = selected;
        buf.push(ch);
    }
    if !buf.is_empty() {
        spans.push(make_span(buf, buf_selected));
    }
    Line::from(spans)
}

fn make_span(text: String, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(text, Style::new().reversed())
    } else {
        Span::raw(text)
    }
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
    fn selection_splits_row_into_styled_spans() {
        let vp = Viewport {
            origin: (0, 0),
            width: 4,
            height: 1,
        };
        let sel = Selection {
            anchor: (1, 0),
            head: (2, 0),
        }; // columns 1..=2 selected
        let line = style_row("abcd", 0, &vp, &sel);
        // "a" plain, "bc" reversed, "d" plain.
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "a");
        assert_eq!(line.spans[1].content, "bc");
        assert_eq!(line.spans[1].style, Style::new().reversed());
        assert_eq!(line.spans[2].content, "d");
    }

    /// Every row must be exactly `vp.width` columns wide, whatever it holds —
    /// that is the invariant wide chars threaten.
    fn assert_row_width(rows: &[String], width: u16) {
        for row in rows {
            assert_eq!(
                unicode_width::UnicodeWidthStr::width(row.as_str()),
                width as usize,
                "row {row:?} is not {width} columns"
            );
        }
    }

    #[test]
    fn wide_char_takes_two_columns_and_hides_its_shadow_cell() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, '日'); // covers columns 0 and 1
        canvas.set(2, 0, 'a');
        let vp = Viewport {
            origin: (0, 0),
            width: 4,
            height: 1,
        };
        // Three chars, four columns: 日 spans 0..=1, 'a' at 2, blank at 3.
        let rows = window_rows(&canvas, &vp);
        assert_eq!(rows, vec!["日a ".to_string()]);
        assert_row_width(&rows, 4);
    }

    #[test]
    fn stale_cell_under_a_wide_glyph_does_not_shift_the_row() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, '日');
        canvas.set(1, 0, 'X'); // hidden behind the glyph (e.g. left by an edit)
        canvas.set(2, 0, 'a');
        let vp = Viewport {
            origin: (0, 0),
            width: 4,
            height: 1,
        };
        // 'X' is skipped, so 'a' still lands on its true column 2.
        let rows = window_rows(&canvas, &vp);
        assert_eq!(rows, vec!["日a ".to_string()]);
        assert_row_width(&rows, 4);
    }

    #[test]
    fn wide_char_in_the_last_column_draws_blank_instead_of_spilling() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, 'a');
        canvas.set(1, 0, '日'); // would need columns 1 and 2; only 1 is left
        let vp = Viewport {
            origin: (0, 0),
            width: 2,
            height: 1,
        };
        let rows = window_rows(&canvas, &vp);
        assert_eq!(rows, vec!["a ".to_string()]);
        assert_row_width(&rows, 2);
    }

    #[test]
    fn selection_spans_track_columns_not_chars_across_a_wide_char() {
        let mut canvas = Canvas::new();
        canvas.set(0, 0, '日'); // columns 0..=1
        canvas.set(2, 0, 'a');
        canvas.set(3, 0, 'b');
        let vp = Viewport {
            origin: (0, 0),
            width: 4,
            height: 1,
        };
        let sel = Selection {
            anchor: (2, 0),
            head: (3, 0),
        }; // columns 2..=3 — 'a' and 'b', not the wide char
        let row = &window_rows(&canvas, &vp)[0];
        let line = style_row(row, 0, &vp, &sel);
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content, "日");
        assert_eq!(line.spans[1].content, "ab");
        assert_eq!(line.spans[1].style, Style::new().reversed());
    }

    /// End-to-end check that the rows we build actually land in the columns we
    /// mean once ratatui paints them — a wide glyph must occupy two buffer
    /// cells, leaving everything after it on its true column.
    #[test]
    fn wide_char_paints_across_two_buffer_cells() {
        use ratatui::{Terminal, backend::TestBackend};

        let mut canvas = Canvas::new();
        canvas.set(0, 0, '日');
        canvas.set(2, 0, 'a');
        let vp = Viewport {
            origin: (0, 0),
            width: 4,
            height: 1,
        };
        let rows = window_rows(&canvas, &vp);

        let mut terminal = Terminal::new(TestBackend::new(4, 1)).unwrap();
        terminal
            .draw(|f| f.render_widget(Paragraph::new(rows.join("\n")), f.area()))
            .unwrap();

        let buf = terminal.backend().buffer();
        assert_eq!(buf[(0, 0)].symbol(), "日");
        assert_eq!(buf[(1, 0)].symbol(), " "); // reserved by the glyph, not drawn
        assert_eq!(buf[(2, 0)].symbol(), "a"); // still on its true column
        assert_eq!(buf[(3, 0)].symbol(), " ");
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
