# viewport — the visible window over the canvas

The rectangle of the infinite canvas currently drawn, plus the coordinate math and (from M4) all scrolling. Triggered when working on navigation, Shift-jump, cursor-follow, or screen↔canvas mapping.

## State (`Viewport`)
- `origin: (Coord, Coord)` — absolute canvas coordinate drawn at the top-left of the canvas area.
- `width: u16`, `height: u16` — the canvas drawing area's size in cells (the render layer syncs these each frame; the status line is excluded).

Visible rectangle: `[origin.x, origin.x+width)` × `[origin.y, origin.y+height)`.

## Coordinate mapping (M3)
- `canvas_at(sx, sy) -> (Coord, Coord)` — canvas coord at a screen offset.
- `screen_of(x, y) -> Option<(u16, u16)>` — screen offset of a canvas coord, or `None` if off-screen. Used to place the terminal cursor.

## Navigation (M4)
- `step() -> (Coord, Coord)` — the one-third-screen jump distance (`width/3`, `height/3`), floored at 1 each axis.
- `jump(dx, dy)` — move `origin` by whole `step`s (Shift+arrow). Cursor untouched, so it may scroll off-screen — the spec's "move canvas view" gesture.
- `scroll_to_show((cx, cy))` — cursor-follow: scroll the minimum to bring the cursor back inside the visible rect.
- Wiring: key dispatch lives in `main::handle_key`; `App::move_cursor` (arrows → move + `scroll_to_show`) and `App::jump_view` (Shift+arrow → `jump`) drive it. **Resize** is handled in the event loop: re-sync `width`/`height` (minus the status row) then `scroll_to_show`.

## Invariants
- The canvas is infinite, so `origin` may go negative; there is no clamping against canvas extent — only cursor-follow.
- Plain arrows move the cursor; Shift+arrows move the view. They must stay distinct.
- Off-by-one: the last visible column is `origin.x + width - 1`.

## Ownership
Owns `origin`, `width`, `height`, and the scroll/cursor-follow math. Reads the cursor (owned by `app`/editing). Does not read or write canvas cells.

## Status
M3 (coordinate mapping) and M4 (jump / cursor-follow / resize) done, with unit tests.
