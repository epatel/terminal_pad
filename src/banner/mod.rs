//! Banner — figlet-style big-letter typing, toggled by Ctrl+B.
//!
//! While banner mode is on, each typed character is stamped onto the canvas as
//! a multi-row FIGlet glyph (the classic `smslant` font, vendored and embedded
//! at compile time) with figlet's kerning/smushing so letters pack together the
//! way `figlet -f smslant` renders them. Enter drops the cursor a full glyph
//! height down to the line anchor. See ./CLAUDE.md.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::app::App;
use crate::canvas::{Canvas, Coord};

/// The vendored FIGfont (unmodified upstream `smslant.flf`).
const SMSLANT: &str = include_str!("smslant.flf");

/// A parsed FIGfont: fixed glyph height plus per-char cell grids. Glyph rows are
/// padded to a uniform width; the hardblank char is kept distinct until stamping
/// (it blocks smushing but prints as a blank).
struct Font {
    height: usize,
    hardblank: char,
    glyphs: HashMap<char, Vec<Vec<char>>>,
}

fn font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| parse(SMSLANT))
}

/// Parse a .flf file: header (`flf2a<hardblank> height baseline maxlen
/// oldlayout commentlines …`), the comment block, then one glyph per printable
/// ASCII char 32..=126, each `height` lines ending in the endmark char (doubled
/// on the glyph's last line). Trailing glyphs (Latin-1 etc.) are ignored.
fn parse(src: &str) -> Font {
    let mut lines = src.lines();
    let header = lines.next().expect("flf header");
    let hardblank = header.chars().nth(5).expect("flf hardblank");
    let fields: Vec<usize> = header
        .split_whitespace()
        .skip(1)
        .filter_map(|t| t.parse().ok())
        .collect();
    let (height, comments) = (fields[0], fields[4]);
    for _ in 0..comments {
        lines.next();
    }

    let mut glyphs = HashMap::new();
    'chars: for code in 32u32..=126 {
        let mut rows: Vec<Vec<char>> = Vec::with_capacity(height);
        for _ in 0..height {
            let Some(line) = lines.next() else {
                break 'chars;
            };
            let end = line.chars().last().unwrap_or('@');
            let mut s = line;
            for _ in 0..2 {
                if let Some(stripped) = s.strip_suffix(end) {
                    s = stripped;
                }
            }
            rows.push(s.chars().collect());
        }
        let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        for r in &mut rows {
            r.resize(w, ' ');
        }
        glyphs.insert(char::from_u32(code).unwrap(), rows);
    }
    Font {
        height,
        hardblank,
        glyphs,
    }
}

/// Toggle banner mode (Ctrl+B). Session-only, like the calculator state.
pub fn toggle(app: &mut App) {
    app.banner = !app.banner;
    app.status = if app.banner {
        "banner mode on (smslant) — Ctrl+B to leave".into()
    } else {
        "banner mode off".into()
    };
}

/// Enter in banner mode: no canvas mutation (a newline split would shred the
/// art) — just drop the cursor one glyph height down, back at the line anchor.
pub fn newline(app: &mut App) {
    app.cursor = (app.anchor_x, app.cursor.1 + font().height as Coord);
    app.viewport.scroll_to_show(app.cursor);
}

/// Stamp `c`'s glyph with its top-left at the cursor, kerned/smushed left
/// against whatever the canvas already holds (figlet's controlled smushing,
/// rules 1–4 — what smslant declares). The cursor advances to the glyph's
/// right edge; a char with no glyph reports in the status line and does nothing.
pub fn type_char(app: &mut App, c: char) {
    let font = font();
    let Some(glyph) = font.glyphs.get(&c) else {
        app.status = format!("banner: no glyph for {c:?}");
        return;
    };
    let (cx, cy) = app.cursor;
    let width = glyph[0].len() as Coord;
    let start = cx - fit_overlap(&app.canvas, glyph, font.hardblank, cx, cy);

    for (r, row) in glyph.iter().enumerate() {
        let y = cy + r as Coord;
        for (j, &g) in row.iter().enumerate() {
            if g == ' ' {
                continue; // transparent — never clobber the canvas
            }
            let x = start + j as Coord;
            // A hardblank prints as a real space cell: invisible, but present so
            // the next glyph can't kern through it (that's the width of ' ').
            let ch = if g == font.hardblank { ' ' } else { g };
            let merged = match app.canvas.get(x, y) {
                Some(existing) if ch == ' ' => existing, // hardblank never erases
                Some(existing) => smush(existing, ch).unwrap_or(ch),
                None => ch,
            };
            app.canvas.set(x, y, merged);
        }
    }

    app.cursor.0 = start + width;
    app.viewport
        .scroll_to_show((app.cursor.0, cy + font.height as Coord - 1));
    app.viewport.scroll_to_show(app.cursor);
}

/// How far left of the cursor the glyph may shift: figlet's kern/smush amount,
/// re-derived against the canvas. Per row, the gap back to the nearest existing
/// cell plus the glyph row's leading blanks — plus one when the boundary pair
/// smushes — and the whole glyph moves by the minimum over its rows. Rows with
/// nothing to their left within reach impose no bound; a fully empty
/// neighborhood means no shift at all.
fn fit_overlap(
    canvas: &Canvas,
    glyph: &[Vec<char>],
    hardblank: char,
    cx: Coord,
    cy: Coord,
) -> Coord {
    let width = glyph[0].len() as Coord;
    const LOOKBACK: Coord = 32;
    let mut overlap = Coord::MAX;
    for (r, row) in glyph.iter().enumerate() {
        let y = cy + r as Coord;
        let Some(edge) = (1..=LOOKBACK)
            .map(|d| cx - d)
            .find(|&x| canvas.get(x, y).is_some())
        else {
            continue;
        };
        let lead = row.iter().take_while(|&&g| g == ' ').count();
        let mut k = (cx - 1 - edge) + lead as Coord;
        if lead < row.len() {
            let g = row[lead];
            let g = if g == hardblank { ' ' } else { g };
            if smush(canvas.get(edge, y).unwrap(), g).is_some() {
                k += 1;
            }
        }
        overlap = overlap.min(k);
    }
    if overlap == Coord::MAX {
        0
    } else {
        overlap.clamp(0, width)
    }
}

/// FIGlet controlled smushing, rules 1–4 (smslant's layout): equal char,
/// underscore replacement, class hierarchy, and opposite brackets → `|`.
/// Spaces (which stand in for hardblanks on the canvas) never smush.
fn smush(l: char, r: char) -> Option<char> {
    if l == ' ' || r == ' ' {
        return None;
    }
    if l == r {
        return Some(l); // rule 1: equal character
    }
    const UNDER: &str = "|/\\[]{}()<>";
    if l == '_' && UNDER.contains(r) {
        return Some(r); // rule 2: underscore gives way
    }
    if r == '_' && UNDER.contains(l) {
        return Some(l);
    }
    let class = |c: char| match c {
        '|' => Some(1),
        '/' | '\\' => Some(2),
        '[' | ']' => Some(3),
        '{' | '}' => Some(4),
        '(' | ')' => Some(5),
        '<' | '>' => Some(6),
        _ => None,
    };
    if let (Some(a), Some(b)) = (class(l), class(r))
        && a != b
    {
        return Some(if a > b { l } else { r }); // rule 3: hierarchy
    }
    match (l, r) {
        ('[', ']') | (']', '[') | ('{', '}') | ('}', '{') | ('(', ')') | (')', '(') => Some('|'), // rule 4
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(app: &mut App, s: &str) {
        for c in s.chars() {
            type_char(app, c);
        }
    }

    /// Canvas rows 0..height as right-trimmed strings, like figlet output.
    fn rows_text(app: &App) -> Vec<String> {
        (0..font().height as Coord)
            .map(|y| {
                (0..40)
                    .map(|x| app.canvas.get(x, y).unwrap_or(' '))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn parses_the_embedded_font() {
        let f = font();
        assert_eq!(f.height, 5);
        assert_eq!(f.hardblank, '$');
        assert_eq!(f.glyphs.len(), 95); // ASCII 32..=126
        assert!(!f.glyphs[&'A'][0].is_empty());
    }

    #[test]
    fn matches_figlet_hi() {
        let mut app = App::new();
        stamp(&mut app, "Hi");
        assert_eq!(
            rows_text(&app),
            ["   __ ___", "  / // (_)", " / _  / /", "/_//_/_/", ""]
        );
    }

    #[test]
    fn matches_figlet_ab() {
        let mut app = App::new();
        stamp(&mut app, "AB");
        assert_eq!(
            rows_text(&app),
            [
                "   ___   ___",
                "  / _ | / _ )",
                " / __ |/ _  |",
                "/_/ |_/____/",
                ""
            ]
        );
    }

    #[test]
    fn matches_figlet_test_with_smushing() {
        let mut app = App::new();
        stamp(&mut app, "Test");
        assert_eq!(
            rows_text(&app),
            [
                " ______        __",
                "/_  __/__ ___ / /_",
                " / / / -_|_-</ __/",
                "/_/  \\__/___/\\__/",
                ""
            ]
        );
    }

    #[test]
    fn hardblank_space_keeps_words_apart() {
        let mut app = App::new();
        stamp(&mut app, "a b");
        assert_eq!(
            rows_text(&app),
            [
                "         __",
                " ___ _  / /",
                "/ _ `/ / _ \\",
                "\\_,_/ /_.__/",
                ""
            ]
        );
    }

    #[test]
    fn cursor_advances_and_first_char_does_not_shift() {
        let mut app = App::new();
        app.cursor = (10, 3);
        type_char(&mut app, 'H');
        let w = font().glyphs[&'H'][0].len() as Coord;
        assert_eq!(app.cursor, (10 + w, 3)); // no left content → no overlap
    }

    #[test]
    fn missing_glyph_reports_and_leaves_canvas_untouched() {
        let mut app = App::new();
        type_char(&mut app, 'å');
        assert_eq!(app.canvas.len(), 0);
        assert!(app.status.contains("no glyph"));
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn toggle_flips_flag_and_reports() {
        let mut app = App::new();
        toggle(&mut app);
        assert!(app.banner);
        assert!(app.status.contains("banner mode on"));
        toggle(&mut app);
        assert!(!app.banner);
    }

    #[test]
    fn newline_drops_a_glyph_height_to_the_anchor() {
        let mut app = App::new();
        app.anchor_x = 4;
        app.cursor = (20, 2);
        newline(&mut app);
        assert_eq!(app.cursor, (4, 2 + font().height as Coord));
    }

    #[test]
    fn smush_rules() {
        assert_eq!(smush('/', '/'), Some('/')); // rule 1
        assert_eq!(smush('_', '/'), Some('/')); // rule 2
        assert_eq!(smush('|', '/'), Some('/')); // rule 3: / outranks |
        assert_eq!(smush('(', ')'), Some('|')); // rule 4
        assert_eq!(smush(' ', '/'), None); // spaces/hardblanks never smush
        assert_eq!(smush('a', 'b'), None);
    }
}
