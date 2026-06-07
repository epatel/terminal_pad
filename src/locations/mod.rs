//! Locations — the nine saved bookmarks (M7).
//!
//! Ctrl+1..9 jump to a slot; Ctrl+Shift+1..9 save the current location into one.
//! A saved location records *both* the cursor and the viewport origin, so a jump
//! restores the exact screen state (plan decision: "save both"). See ./CLAUDE.md.

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::canvas::Coord;

/// A saved spot: where the cursor was and what the view was showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub cursor: (Coord, Coord),
    pub origin: (Coord, Coord),
}

/// Number of bookmark slots, bound to digits 1..9.
pub const SLOT_COUNT: usize = 9;

/// Map a typed digit char ('1'..'9') to a slot index (0..8), or `None`.
pub fn slot_for_digit(c: char) -> Option<usize> {
    c.to_digit(10)
        .filter(|&d| (1..=SLOT_COUNT as u32).contains(&d))
        .map(|d| d as usize - 1)
}

/// Save the current cursor + view into `slot` (Ctrl+Shift+digit).
pub fn save(app: &mut App, slot: usize) {
    if slot >= SLOT_COUNT {
        return;
    }
    app.locations[slot] = Some(Location {
        cursor: app.cursor,
        origin: app.viewport.origin,
    });
}

/// Jump to `slot` (Ctrl+digit): restore its cursor + view. No-op for an empty or
/// out-of-range slot.
pub fn jump(app: &mut App, slot: usize) {
    if slot >= SLOT_COUNT {
        return;
    }
    if let Some(loc) = app.locations[slot] {
        app.cursor = loc.cursor;
        app.viewport.origin = loc.origin;
        app.anchor_x = app.cursor.0; // a jump repositions, like navigation
        // Correct the view if the terminal shrank since the save.
        app.viewport.scroll_to_show(app.cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_maps_to_slot() {
        assert_eq!(slot_for_digit('1'), Some(0));
        assert_eq!(slot_for_digit('9'), Some(8));
        assert_eq!(slot_for_digit('0'), None);
        assert_eq!(slot_for_digit('a'), None);
    }

    #[test]
    fn save_then_jump_restores_cursor_and_view() {
        let mut app = App::new();
        app.viewport.width = 20;
        app.viewport.height = 10;
        app.cursor = (42, 17);
        app.viewport.origin = (40, 12);
        save(&mut app, 2);

        // Wander off, then jump back.
        app.cursor = (0, 0);
        app.viewport.origin = (0, 0);
        jump(&mut app, 2);
        assert_eq!(app.cursor, (42, 17));
        assert_eq!(app.viewport.origin, (40, 12));
    }

    #[test]
    fn jump_to_empty_slot_is_noop() {
        let mut app = App::new();
        app.cursor = (5, 5);
        jump(&mut app, 0);
        assert_eq!(app.cursor, (5, 5));
    }

    #[test]
    fn out_of_range_slot_is_ignored() {
        let mut app = App::new();
        save(&mut app, 99); // must not panic
        jump(&mut app, 99);
        assert_eq!(app.cursor, (0, 0));
    }
}
