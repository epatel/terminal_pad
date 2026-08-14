//! Find — search the canvas for text (M26).
//!
//! Ctrl+F opens a live search in the status line: each typed character narrows
//! the query and jumps the cursor to the first place that text appears, Ctrl+F
//! again cycles through the rest, Enter keeps the landing spot and Esc returns
//! to where the search started.
//!
//! There is no separate tag feature: a `#name` tag is just text on the canvas,
//! so typing `#api` finds it like anything else. Nothing is indexed or
//! persisted. See ./CLAUDE.md.

use crate::app::App;
use crate::canvas::{Canvas, Coord};

/// A live search. Session-only — never persisted.
pub struct Search {
    /// Text typed so far.
    query: String,
    /// Cursor and viewport origin when the search opened; Esc restores them.
    start: ((Coord, Coord), (Coord, Coord)),
    /// Where `query` appears, in canvas order, as the position of its first char.
    matches: Vec<(Coord, Coord)>,
    /// Which match the cursor is parked on. Meaningless when `matches` is empty.
    index: usize,
}

impl Search {
    /// The query as typed, for the status line.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// `Some((position, total))` — 1-based — when anything matched.
    pub fn position(&self) -> Option<(usize, usize)> {
        (!self.matches.is_empty()).then(|| (self.index + 1, self.matches.len()))
    }
}

/// Every place `query` appears on the canvas, in canvas order (top-to-bottom,
/// then left-to-right), as the position of each match's first character.
///
/// Matching is a case-insensitive literal substring search **within a row** — a
/// match never wraps onto the next row. Unwritten cells read as blanks, so a
/// query containing a space matches whether that gap is a typed space or an
/// empty cell. An empty query matches nothing (there is nothing to jump to yet).
pub fn matches(canvas: &Canvas, query: &str) -> Vec<(Coord, Coord)> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    // cells() is sorted by (y, x), so rows come out in order and each row's
    // cells ascend — walk it once, grouping by row.
    let cells = canvas.cells();
    let mut i = 0;
    while i < cells.len() {
        let y = cells[i].1;
        let start = i;
        while i < cells.len() && cells[i].1 == y {
            i += 1;
        }
        let row = &cells[start..i];
        let (first, last) = (row[0].0, row[row.len() - 1].0);
        // Render the row's span as text, blanks for unwritten cells, so a
        // char offset into it maps to `first + offset`.
        let mut text = String::with_capacity((last - first + 1) as usize);
        let mut next = first;
        for &(x, _, c) in row {
            for _ in next..x {
                text.push(' ');
            }
            text.push(c);
            next = x + 1;
        }
        let hay = text.to_lowercase();
        // Overlapping matches count separately ("aa" twice in "aaa").
        let mut from = 0;
        while let Some(hit) = hay[from..].find(&needle) {
            let at = from + hit;
            // Char offset, not byte offset — a row may hold multi-byte chars.
            let col = hay[..at].chars().count() as Coord;
            out.push((first + col, y));
            from = at + hay[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Open a search (Ctrl+F). The view does not move until something is typed —
/// jumping before the user has said where would be startling.
pub fn open(app: &mut App) {
    app.search = Some(Search {
        query: String::new(),
        start: (app.cursor, app.viewport.origin),
        matches: Vec::new(),
        index: 0,
    });
    app.status = "find: type to search (# finds tags)".into();
}

/// Step to the next match, wrapping (Ctrl+F with a search already open).
pub fn advance(app: &mut App) {
    let Some(s) = app.search.as_mut() else {
        return;
    };
    if s.matches.is_empty() {
        return;
    }
    s.index = (s.index + 1) % s.matches.len();
    jump_to_current(app);
}

/// Extend the query by a typed character and re-search from the first match.
pub fn push(app: &mut App, c: char) {
    let Some(s) = app.search.as_mut() else {
        return;
    };
    s.query.push(c);
    research(app);
}

/// Drop the last query character and re-search. On an empty query this closes
/// the search, so Backspace walks all the way back out.
pub fn backspace(app: &mut App) {
    let Some(s) = app.search.as_mut() else {
        return;
    };
    if s.query.pop().is_none() {
        cancel(app);
        return;
    }
    research(app);
}

/// Recompute the matches for the current query and jump to the first.
fn research(app: &mut App) {
    let Some(s) = app.search.as_mut() else {
        return;
    };
    let query = s.query.clone();
    s.matches = matches(&app.canvas, &query);
    s.index = 0;
    if app.search.as_ref().is_some_and(|s| s.matches.is_empty()) {
        // Keep the query so Backspace can walk back to something that matches.
        app.status = format!("find: {query} — no match");
        return;
    }
    jump_to_current(app);
}

/// Move the cursor to the match at `index` and bring it on screen.
fn jump_to_current(app: &mut App) {
    let Some(s) = app.search.as_ref() else {
        return;
    };
    let Some(&pos) = s.matches.get(s.index) else {
        return;
    };
    app.cursor = pos;
    // Minimal scroll when the hit is already visible (so narrowing the query
    // doesn't shuffle the view); a quarter in from the top-left when jumping
    // somewhere new, so it lands with its text laid out below and right.
    if app.viewport.screen_of(pos.0, pos.1).is_some() {
        app.viewport.scroll_to_show(pos);
    } else {
        app.viewport.focus_on(pos);
    }
    app.anchor_x = pos.0;
}

/// Accept the search: close it, leaving the cursor on the match it found.
pub fn accept(app: &mut App) {
    let Some(s) = app.search.take() else {
        return;
    };
    app.status = match s.position() {
        Some((i, n)) => format!("find: {} {i}/{n}", s.query),
        None => format!("find: {} — no match", s.query),
    };
}

/// Cancel the search (Esc): close it and put the cursor and view back where the
/// search started.
pub fn cancel(app: &mut App) {
    let Some(s) = app.search.take() else {
        return;
    };
    (app.cursor, app.viewport.origin) = s.start;
    app.anchor_x = app.cursor.0;
    app.status = "find cancelled".into();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(app: &mut App, x: Coord, y: Coord, s: &str) {
        for (i, c) in s.chars().enumerate() {
            app.canvas.set(x + i as Coord, y, c);
        }
    }

    /// A viewport big enough that jumps stay visible, with text and two tags.
    fn app_with_text() -> App {
        let mut app = App::new();
        app.viewport.width = 80;
        app.viewport.height = 24;
        put(&mut app, 4, 1, "#arch is here");
        put(&mut app, 0, 5, "see #api-notes for more");
        put(&mut app, 10, 9, "rapid prototype");
        app
    }

    #[test]
    fn matches_finds_substrings_in_canvas_order() {
        let app = app_with_text();
        // "api" appears in the #api-notes tag and inside "rapid".
        assert_eq!(matches(&app.canvas, "api"), vec![(5, 5), (11, 9)]);
        assert_eq!(matches(&app.canvas, "arch"), vec![(5, 1)]);
        assert_eq!(matches(&app.canvas, "nope"), vec![]);
    }

    #[test]
    fn a_hash_query_finds_tags() {
        // The point of dropping the separate tag feature: '#' is just text.
        let app = app_with_text();
        assert_eq!(matches(&app.canvas, "#"), vec![(4, 1), (4, 5)]);
        assert_eq!(matches(&app.canvas, "#api"), vec![(4, 5)]);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let mut app = App::new();
        put(&mut app, 0, 0, "#API Notes");
        assert_eq!(matches(&app.canvas, "api"), vec![(1, 0)]);
        assert_eq!(matches(&app.canvas, "NOTES"), vec![(5, 0)]);
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        let app = app_with_text();
        assert_eq!(matches(&app.canvas, ""), vec![]);
    }

    #[test]
    fn a_query_may_span_blanks_and_typed_spaces() {
        let mut app = App::new();
        put(&mut app, 0, 0, "a b"); // typed space at column 1
        app.canvas.set(0, 1, 'a');
        app.canvas.set(2, 1, 'b'); // column 1 never written
        assert_eq!(matches(&app.canvas, "a b"), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn a_match_never_wraps_onto_the_next_row() {
        let mut app = App::new();
        put(&mut app, 0, 0, "ab");
        put(&mut app, 0, 1, "cd");
        assert_eq!(matches(&app.canvas, "bc"), vec![]);
    }

    #[test]
    fn overlapping_matches_are_all_found() {
        let mut app = App::new();
        put(&mut app, 0, 0, "aaa");
        assert_eq!(matches(&app.canvas, "aa"), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn matches_handle_multibyte_rows() {
        let mut app = App::new();
        put(&mut app, 0, 0, "日本x"); // wide chars occupy their own cells here
        assert_eq!(matches(&app.canvas, "x"), vec![(2, 0)]);
    }

    #[test]
    fn opening_does_not_move_the_cursor() {
        let mut app = app_with_text();
        app.cursor = (2, 2);
        open(&mut app);
        assert_eq!(app.cursor, (2, 2));
        assert_eq!(app.search.as_ref().unwrap().position(), None);
    }

    #[test]
    fn typing_narrows_and_jumps_to_the_first_match() {
        let mut app = app_with_text();
        open(&mut app);
        push(&mut app, 'a');
        assert_eq!(app.cursor, (5, 1)); // the 'a' of #arch, first in canvas order
        push(&mut app, 'p');
        assert_eq!(app.cursor, (5, 5)); // narrowed to "ap" in #api-notes
        assert_eq!(app.search.as_ref().unwrap().position(), Some((1, 2)));
    }

    #[test]
    fn ctrl_f_cycles_through_matches_and_wraps() {
        let mut app = app_with_text();
        open(&mut app);
        for c in "api".chars() {
            push(&mut app, c);
        }
        assert_eq!(app.cursor, (5, 5));
        advance(&mut app);
        assert_eq!(app.cursor, (11, 9)); // inside "rapid"
        assert_eq!(app.search.as_ref().unwrap().position(), Some((2, 2)));
        advance(&mut app); // wraps
        assert_eq!(app.cursor, (5, 5));
        assert_eq!(app.search.as_ref().unwrap().position(), Some((1, 2)));
    }

    #[test]
    fn a_query_with_no_match_leaves_the_cursor_put() {
        let mut app = app_with_text();
        open(&mut app);
        push(&mut app, 'a');
        let landed = app.cursor;
        push(&mut app, 'z'); // "az" matches nothing
        assert_eq!(app.cursor, landed);
        assert_eq!(app.search.as_ref().unwrap().position(), None);
        assert!(app.status.contains("no match"));
        backspace(&mut app); // back to "a", which matches again
        assert_eq!(app.cursor, landed);
        assert!(app.search.as_ref().unwrap().position().is_some());
    }

    #[test]
    fn accept_keeps_the_landing_spot_and_closes() {
        let mut app = app_with_text();
        open(&mut app);
        for c in "arch".chars() {
            push(&mut app, c);
        }
        accept(&mut app);
        assert!(app.search.is_none());
        assert_eq!(app.cursor, (5, 1));
        assert_eq!(app.status, "find: arch 1/1");
    }

    #[test]
    fn cancel_restores_the_starting_cursor_and_view() {
        let mut app = app_with_text();
        app.cursor = (2, 2);
        app.viewport.origin = (1, 1);
        open(&mut app);
        push(&mut app, 'a');
        assert_ne!(app.cursor, (2, 2));
        cancel(&mut app);
        assert!(app.search.is_none());
        assert_eq!(app.cursor, (2, 2));
        assert_eq!(app.viewport.origin, (1, 1));
    }

    #[test]
    fn backspace_on_an_empty_query_closes_the_search() {
        let mut app = app_with_text();
        app.cursor = (2, 2);
        open(&mut app);
        backspace(&mut app);
        assert!(app.search.is_none());
        assert_eq!(app.cursor, (2, 2));
    }

    #[test]
    fn a_far_off_hit_lands_a_quarter_in_not_shoved_to_the_edge() {
        let mut app = App::new();
        app.viewport.width = 80;
        app.viewport.height = 24;
        put(&mut app, 500, 500, "far");
        open(&mut app);
        push(&mut app, 'f');
        assert_eq!(app.cursor, (500, 500));
        // A quarter in from the top-left: 500 - 80/4 and 500 - 24/4.
        assert_eq!(app.viewport.origin, (480, 494));
    }
}
