//! Todo — checkbox list chaining (M20).
//!
//! Enter on a line whose segment starts with a `[ ]` / `[x]` / `[X]` checkbox
//! followed by a blank (the cursor at or past the box) does a normal Enter and
//! prefixes the new line with a fresh unchecked `[ ] ` — so Enter keeps a
//! checklist going. The "line" is the layout segment under the cursor, so a
//! checkbox past a ≥2-blank gap (a side column) is not seen. See ./CLAUDE.md.

use crate::app::App;

/// The chained prefix: a new item always starts unchecked.
const PREFIX: &str = "[ ] ";

/// The Enter hook: chain a checkbox line. Returns `false` (canvas untouched)
/// when the cursor's line does not start with a checkbox, so the caller can
/// fall through to the normal newline.
pub fn enter(app: &mut App) -> bool {
    let (cx, cy) = app.cursor;
    let Some((s, _)) = crate::layout::line_bounds(&app.canvas, cx, cy) else {
        return false;
    };
    let at = |x| app.canvas.get(x, cy).unwrap_or(' ');
    let boxed = at(s) == '[' && matches!(at(s + 1), ' ' | 'x' | 'X') && at(s + 2) == ']';
    // The box must read `[ ] ` (blank after the bracket) and lie fully left of
    // the cursor — Enter inside the box itself is a plain newline.
    if !boxed || at(s + 3) != ' ' || cx < s + 3 {
        return false;
    }
    crate::editing::newline(app);
    for ch in PREFIX.chars() {
        app.canvas.insert_shift(app.cursor.0, app.cursor.1, ch);
        app.cursor.0 += 1;
    }
    app.viewport.scroll_to_show(app.cursor);
    true
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
            crate::editing::type_char(app, c);
        }
    }

    #[test]
    fn enter_on_unchecked_item_chains_a_fresh_box() {
        let mut app = App::new();
        type_str(&mut app, "[ ] buy milk");
        assert!(enter(&mut app));
        assert_eq!(row(&app, 0), "[ ] buy milk");
        assert_eq!(row(&app, 1), "[ ] "); // trailing space is a stored cell
        assert_eq!(app.cursor, (4, 1)); // after the prefix, ready to type
    }

    #[test]
    fn checked_items_chain_an_unchecked_box() {
        for boxed in ["[x] done", "[X] done"] {
            let mut app = App::new();
            type_str(&mut app, boxed);
            assert!(enter(&mut app));
            assert_eq!(row(&app, 1), "[ ] ");
            assert_eq!(app.cursor, (4, 1));
        }
    }

    #[test]
    fn split_mid_item_moves_the_tail_after_the_new_box() {
        let mut app = App::new();
        type_str(&mut app, "[ ] buy milk");
        app.cursor.0 = 8; // before "milk"
        assert!(enter(&mut app));
        assert_eq!(row(&app, 0), "[ ] buy "); // the typed space cell stays
        assert_eq!(row(&app, 1), "[ ] milk");
    }

    #[test]
    fn box_needs_a_blank_after_the_bracket() {
        let mut app = App::new();
        type_str(&mut app, "[x]!alarm");
        assert!(!enter(&mut app));
        assert_eq!(row(&app, 1), "");
    }

    #[test]
    fn cursor_inside_the_box_is_not_a_chain() {
        let mut app = App::new();
        type_str(&mut app, "[ ] item");
        app.cursor.0 = 2; // on the ']'
        assert!(!enter(&mut app));
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        // After the bare prefix the cursor sits past `end + 1` of the segment
        // (the trailing space is blank), so per `layout::line_bounds` it is off
        // the line — Enter falls through to a plain newline, ending the list.
        let mut app = App::new();
        type_str(&mut app, "[ ] ");
        assert!(!enter(&mut app));
        // Right after the `]` (still on the line) it chains as usual.
        app.cursor.0 = 3;
        assert!(enter(&mut app));
        assert_eq!(row(&app, 1), "[ ] ");
    }

    #[test]
    fn plain_text_and_empty_space_fall_through() {
        let mut app = App::new();
        type_str(&mut app, "notes [ ] later");
        assert!(!enter(&mut app)); // box not at the line's start
        let mut empty = App::new();
        assert!(!enter(&mut empty)); // no line under the cursor
    }

    #[test]
    fn box_in_side_segment_past_gap_is_not_seen() {
        let mut app = App::new();
        type_str(&mut app, "[ ] a"); // cols 0..=4
        // ≥2-blank gap, then a separate segment the cursor sits in.
        app.cursor = (10, 0);
        app.anchor_x = 10;
        type_str(&mut app, "notes");
        assert!(!enter(&mut app));
    }
}
