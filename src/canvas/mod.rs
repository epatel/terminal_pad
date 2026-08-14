//! Canvas model — the infinite 2D grid of characters (M2).
//!
//! Representation (a sparse map) is private; callers go through the operations
//! below. See ./CLAUDE.md for the contract and cards/decision-sparse-grid.md
//! for why it's stored this way.

use std::collections::HashMap;

/// Absolute cell coordinate component. Unbounded in all four directions.
pub type Coord = i64;

/// An infinite 2D grid of characters, stored sparsely: only written cells cost
/// memory, and an absent cell renders as blank. A cell is either present with a
/// char or absent — never present-but-blank (erase with [`Canvas::clear`], not a
/// space), which keeps the map and any future serialization small.
#[derive(Debug, Default, Clone)]
pub struct Canvas {
    cells: HashMap<(Coord, Coord), char>,
}

impl Canvas {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the char at `(x, y)`, or `None` if the cell is blank.
    pub fn get(&self, x: Coord, y: Coord) -> Option<char> {
        self.cells.get(&(x, y)).copied()
    }

    /// Write `ch` at `(x, y)`, replacing whatever was there (overwrite semantics).
    pub fn set(&mut self, x: Coord, y: Coord, ch: char) {
        self.cells.insert((x, y), ch);
    }

    /// Erase `(x, y)` back to truly-blank. No-op if already blank.
    pub fn clear(&mut self, x: Coord, y: Coord) {
        self.cells.remove(&(x, y));
    }

    /// Number of written (non-blank) cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// All written cells, sorted by `(y, x)` for deterministic output. Used by
    /// persistence and by tests reconstructing rows.
    pub fn cells(&self) -> Vec<(Coord, Coord, char)> {
        let mut v: Vec<(Coord, Coord, char)> =
            self.cells.iter().map(|(&(x, y), &c)| (x, y, c)).collect();
        v.sort_unstable_by_key(|&(x, y, _)| (y, x));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_string(c: &Canvas, y: Coord) -> String {
        // cells() is sorted by (y, x), so filtering a row yields ascending x.
        c.cells()
            .into_iter()
            .filter(|&(_, cy, _)| cy == y)
            .map(|(_, _, ch)| ch)
            .collect()
    }

    #[test]
    fn set_get_clear_roundtrip() {
        let mut c = Canvas::new();
        assert_eq!(c.len(), 0);
        c.set(3, -2, 'A');
        assert_eq!(c.get(3, -2), Some('A'));
        assert_eq!(c.get(0, 0), None);
        assert_eq!(c.len(), 1);

        c.set(3, -2, 'B'); // overwrite
        assert_eq!(c.get(3, -2), Some('B'));
        assert_eq!(c.len(), 1);

        c.clear(3, -2);
        assert_eq!(c.get(3, -2), None);
        c.clear(3, -2); // idempotent
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn cells_sorted_by_yx_and_rows_isolated() {
        let mut c = Canvas::new();
        c.set(2, 0, 'c');
        c.set(0, 0, 'a');
        c.set(1, 0, 'b');
        c.set(0, 1, 'z'); // different row
        assert_eq!(row_string(&c, 0), "abc");
        assert_eq!(row_string(&c, 1), "z");
        assert_eq!(row_string(&c, 9), "");
        assert_eq!(
            c.cells(),
            vec![(0, 0, 'a'), (1, 0, 'b'), (2, 0, 'c'), (0, 1, 'z')]
        );
    }
}
