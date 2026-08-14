//! Display width of a canvas char, in terminal columns (M23).
//!
//! The canvas stores one `char` per cell and the app's core invariant is that a
//! canvas column *is* a screen column — cursor math, `App::click_to`, and
//! selection rectangles all rely on it. A double-width char (CJK, most emoji)
//! covers two terminal columns, so writers leave its second column blank and the
//! renderer skips the cell hiding behind the glyph. Both sides ask here.

use crate::canvas::Coord;
use unicode_width::UnicodeWidthChar;

/// Columns `ch` occupies on screen: 2 for double-width chars, else 1.
///
/// Zero-width (combining marks) and control chars report 1: they still get a
/// cell of their own, which keeps the column invariant intact even though such
/// a char renders oddly. Filtering them out is the input layer's job, not this
/// one's.
pub fn cols(ch: char) -> Coord {
    match ch.width() {
        Some(2) => 2,
        _ => 1,
    }
}

/// Whether `ch` covers two columns (i.e. writers must leave a blank behind it).
pub fn is_wide(ch: char) -> bool {
    cols(ch) == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_latin_are_one_column() {
        for ch in ['a', 'Z', '0', ' ', '~', 'é', 'ß'] {
            assert_eq!(cols(ch), 1, "{ch:?}");
            assert!(!is_wide(ch));
        }
    }

    #[test]
    fn cjk_and_emoji_are_two_columns() {
        for ch in ['日', '本', '語', '中', '한', 'あ', '🦀', '😀'] {
            assert_eq!(cols(ch), 2, "{ch:?}");
            assert!(is_wide(ch));
        }
    }

    #[test]
    fn zero_width_and_control_fall_back_to_one() {
        // A combining acute reports width 0 and a control char reports None;
        // both take a cell so the canvas column stays the screen column.
        assert_eq!(cols('\u{301}'), 1);
        assert_eq!(cols('\u{7}'), 1);
    }
}
