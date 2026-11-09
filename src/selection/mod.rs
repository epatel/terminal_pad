//! Selection — a rectangular block of canvas cells, made by left-click-drag.
//!
//! The model is a pure value (two corners in absolute canvas coords) plus pure
//! canvas operations (extract / to-text / clear). Drag tracking, the internal
//! clip buffer, and the copy/paste/delete orchestration live on `App`; the
//! system-clipboard side effect lives in `crate::clipboard`. See ./CLAUDE.md.

use crate::canvas::{Canvas, Coord};

/// A rectangular selection, defined by the drag `anchor` and the current `head`
/// (the dragged corner). Either corner may hold the min or max coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (Coord, Coord),
    pub head: (Coord, Coord),
}

impl Selection {
    /// A zero-size selection at a single cell (a click before any drag).
    pub fn new(p: (Coord, Coord)) -> Self {
        Self { anchor: p, head: p }
    }

    /// Inclusive normalized bounds `(min_x, min_y, max_x, max_y)`.
    pub fn bounds(&self) -> (Coord, Coord, Coord, Coord) {
        (
            self.anchor.0.min(self.head.0),
            self.anchor.1.min(self.head.1),
            self.anchor.0.max(self.head.0),
            self.anchor.1.max(self.head.1),
        )
    }

    /// Whether `(x, y)` lies inside the rectangle (inclusive).
    pub fn contains(&self, x: Coord, y: Coord) -> bool {
        let (x0, y0, x1, y1) = self.bounds();
        x >= x0 && x <= x1 && y >= y0 && y <= y1
    }

    /// True when the selection covers a single cell — i.e. a click, not a drag.
    pub fn is_point(&self) -> bool {
        self.anchor == self.head
    }

    /// Width in cells (inclusive).
    pub fn width(&self) -> Coord {
        let (x0, _, x1, _) = self.bounds();
        x1 - x0 + 1
    }

    /// Height in cells (inclusive).
    pub fn height(&self) -> Coord {
        let (_, y0, _, y1) = self.bounds();
        y1 - y0 + 1
    }
}

/// The selected rectangle as a grid of chars, top→bottom then left→right, with a
/// space for each blank cell. Rectangular: every row is `selection.width()` wide.
pub fn extract(canvas: &Canvas, sel: &Selection) -> Vec<Vec<char>> {
    let (x0, y0, x1, y1) = sel.bounds();
    (y0..=y1)
        .map(|y| (x0..=x1).map(|x| canvas.get(x, y).unwrap_or(' ')).collect())
        .collect()
}

/// Render a block as newline-joined text with trailing blanks trimmed per row —
/// the friendly form for the system clipboard / other apps.
pub fn to_text(block: &[Vec<char>]) -> String {
    block
        .iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Character classes used for word-boundary detection (double-click select).
/// Blank cells and whitespace are `None` — they belong to no word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Letters, digits and `_` — the "word" run.
    Word,
    /// Any other visible char (`,`, `.`, `(`, `-`, …) — punctuation runs group
    /// with each other, so a double-click on `,` selects just the punctuation.
    Punct,
}

/// Punctuation that joins word chars into one word *when it sits between them* —
/// everything a URL, path, host, or version string is built from
/// (`https://example.com/a_b?x=1&y=2`, `src/main.rs`, `v1.2.3-rc`). At an edge
/// it is not part of the word, so a sentence's trailing `.` stays out.
const JOINERS: &[char] = &['.', '-', ':', '/', '?', '#', '@', '%', '&', '=', '+', '~'];

fn class_of(ch: char) -> Option<Class> {
    if ch.is_whitespace() {
        None
    } else if ch.is_alphanumeric() || ch == '_' {
        Some(Class::Word)
    } else {
        Some(Class::Punct)
    }
}

/// Whether `ch` can sit inside a word (a word char or a joiner).
fn inner(ch: char) -> bool {
    class_of(ch) == Some(Class::Word) || JOINERS.contains(&ch)
}

/// Inclusive column range of the "word" under `(x, y)`, or `None` on a blank /
/// whitespace cell.
///
/// A word is the maximal run of word chars (alphanumeric + `_`) and joiners
/// (`. - : / ? # @ % & = + ~`), with joiners trimmed off both edges — so a whole
/// URL selects as one unit while the period ending the sentence after it does
/// not. Anything else is punctuation, and a click there selects the maximal
/// punctuation run instead (so `hello, world` gives `hello` or `,`, and `->(`
/// selects as one run). Clicking a joiner that trimming would drop — the final
/// `.` of `example.com.` — falls back to that punctuation run too.
pub fn word_span(canvas: &Canvas, x: Coord, y: Coord) -> Option<(Coord, Coord)> {
    let ch = canvas.get(x, y)?;
    class_of(ch)?;
    let class_at = |c: Coord| canvas.get(c, y).and_then(class_of);
    // A run of joiners only (`--`), or a click on a trimmed-off edge joiner, is
    // really punctuation — fall through to the punctuation run.
    let word = inner(ch)
        .then(|| {
            let run = expand(x, |c| canvas.get(c, y).is_some_and(inner));
            trim_to(run, |c| class_at(c) == Some(Class::Word))
        })
        .flatten()
        .filter(|&(s, e)| (s..=e).contains(&x));
    word.or_else(|| Some(expand(x, |c| class_at(c) == Some(Class::Punct))))
}

/// The maximal column run around `x` whose neighbours satisfy `keep` (`x`
/// itself is always included).
fn expand(x: Coord, keep: impl Fn(Coord) -> bool) -> (Coord, Coord) {
    let (mut s, mut e) = (x, x);
    while keep(s - 1) {
        s -= 1;
    }
    while keep(e + 1) {
        e += 1;
    }
    (s, e)
}

/// `run` shrunk from both ends until each end satisfies `keep` — used to trim
/// joiners off a word's edges, since they only count between word chars.
/// `None` when nothing in the run does.
fn trim_to((mut s, mut e): (Coord, Coord), keep: impl Fn(Coord) -> bool) -> Option<(Coord, Coord)> {
    while s <= e && !keep(s) {
        s += 1;
    }
    while s <= e && !keep(e) {
        e -= 1;
    }
    (s <= e).then_some((s, e))
}

/// Erase every cell in the selection back to truly-blank.
pub fn clear(canvas: &mut Canvas, sel: &Selection) {
    let (x0, y0, x1, y1) = sel.bounds();
    for y in y0..=y1 {
        for x in x0..=x1 {
            canvas.clear(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_canvas() -> Canvas {
        // A 3x2 region with a gap:  "ab "
        //                           " cd"
        let mut c = Canvas::new();
        c.set(0, 0, 'a');
        c.set(1, 0, 'b');
        c.set(1, 1, 'c');
        c.set(2, 1, 'd');
        c
    }

    #[test]
    fn bounds_normalize_regardless_of_drag_direction() {
        let down_right = Selection {
            anchor: (1, 1),
            head: (4, 3),
        };
        let up_left = Selection {
            anchor: (4, 3),
            head: (1, 1),
        };
        assert_eq!(down_right.bounds(), (1, 1, 4, 3));
        assert_eq!(up_left.bounds(), (1, 1, 4, 3));
        assert_eq!(down_right.width(), 4);
        assert_eq!(down_right.height(), 3);
    }

    #[test]
    fn contains_and_is_point() {
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        assert!(sel.contains(1, 1));
        assert!(sel.contains(0, 0));
        assert!(!sel.contains(3, 1));
        assert!(!sel.contains(1, 2));
        assert!(!sel.is_point());
        assert!(Selection::new((5, 5)).is_point());
    }

    fn row_canvas(s: &str) -> Canvas {
        let mut c = Canvas::new();
        for (i, ch) in s.chars().enumerate() {
            if ch != ' ' {
                c.set(i as Coord, 0, ch);
            }
        }
        c
    }

    #[test]
    fn word_span_stops_at_punctuation_and_blanks() {
        let c = row_canvas("hello, world_9 x");
        assert_eq!(word_span(&c, 0, 0), Some((0, 4))); // "hello"
        assert_eq!(word_span(&c, 4, 0), Some((0, 4))); // last char of the word
        assert_eq!(word_span(&c, 5, 0), Some((5, 5))); // the comma alone
        assert_eq!(word_span(&c, 10, 0), Some((7, 13))); // "world_9" (digits + _)
        assert_eq!(word_span(&c, 6, 0), None); // blank between words
        assert_eq!(word_span(&c, 15, 0), Some((15, 15))); // one-char word
        assert_eq!(word_span(&c, 40, 0), None); // empty canvas
    }

    #[test]
    fn word_span_takes_a_whole_url_but_not_the_sentence_period() {
        //                0123456789...
        let c = row_canvas("see https://example.com/a_b?x=1&y=2. ok");
        let url = (4, 34); // "https://…&y=2" — the sentence period at 35 stays out
        assert_eq!(word_span(&c, 4, 0), Some(url)); // on the scheme
        assert_eq!(word_span(&c, 11, 0), Some(url)); // on the "//"
        assert_eq!(word_span(&c, 15, 0), Some(url)); // mid-host
        assert_eq!(word_span(&c, 34, 0), Some(url)); // last char of the query
        assert_eq!(word_span(&c, 35, 0), Some((35, 35))); // the trailing period alone
    }

    #[test]
    fn word_span_keeps_paths_and_versions_together() {
        let c = row_canvas("src/main.rs v1.2.3-rc a--b -- x");
        assert_eq!(word_span(&c, 5, 0), Some((0, 10))); // "src/main.rs"
        assert_eq!(word_span(&c, 14, 0), Some((12, 20))); // "v1.2.3-rc"
        assert_eq!(word_span(&c, 23, 0), Some((22, 25))); // joiners inside: "a--b"
        assert_eq!(word_span(&c, 27, 0), Some((27, 28))); // free-standing "--" is punctuation
    }

    #[test]
    fn word_span_groups_punctuation_runs() {
        let c = row_canvas("a->(b)");
        assert_eq!(word_span(&c, 1, 0), Some((1, 3))); // "->(" as one run
        assert_eq!(word_span(&c, 4, 0), Some((4, 4))); // "b"
    }

    #[test]
    fn extract_fills_blanks_with_spaces() {
        let c = block_canvas();
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        assert_eq!(
            extract(&c, &sel),
            vec![vec!['a', 'b', ' '], vec![' ', 'c', 'd']]
        );
    }

    #[test]
    fn to_text_trims_trailing_blanks_per_row() {
        let block = vec![vec!['a', 'b', ' '], vec![' ', 'c', 'd']];
        assert_eq!(to_text(&block), "ab\n cd");
    }

    #[test]
    fn clear_erases_only_the_rectangle() {
        let mut c = block_canvas();
        c.set(5, 5, 'z'); // outside the selection
        let sel = Selection {
            anchor: (0, 0),
            head: (2, 1),
        };
        clear(&mut c, &sel);
        assert_eq!(c.get(0, 0), None);
        assert_eq!(c.get(1, 1), None);
        assert_eq!(c.get(5, 5), Some('z')); // untouched
    }
}
