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
- `jump(dx, dy)` — move `origin` by whole `step`s. The viewport primitive; the cursor is moved separately by the caller.
- `scroll_to_show((cx, cy))` — cursor-follow: scroll the minimum to bring the cursor back inside the visible rect.
- Wiring: key dispatch lives in `main::handle_key`. `App::move_cursor` (arrows → move + `scroll_to_show`); `App::jump_view` (Shift+arrow → `jump` **and** carry the cursor the same delta, so it keeps its screen position and reversing returns to start); `App::pan_view` (overview arrows → move view+cursor by a screenful). **Resize** is handled in the event loop: re-sync `width`/`height` (minus the status row) then `scroll_to_show`.

## Mouse
Mouse capture is enabled in `main::setup_terminal` (disabled in `restore_terminal`) and routed via `main::handle_mouse` (normal mode only; ignored under the help/overview overlays). A click's `(column, row)` is already in canvas-area space (the area is top-left, status line at the bottom), so `App::click_to(col, row)` maps it through `canvas_at` and sets the cursor (clicks on the status row are ignored). The scroll wheel calls `App::scroll_rows(±WHEEL_STEP)`, panning `origin.y` without moving the cursor. **Caveat:** capturing the mouse takes over the terminal's native click-to-select/copy; users typically hold Shift (or Option) to get the terminal's own selection back.

## Invariants
- The canvas is infinite, so `origin` may go negative; there is no clamping against canvas extent — only cursor-follow.
- Plain arrows move the cursor one cell (view follows); Shift+arrows pan the view by 1/3 screen and carry the cursor with it. The cursor therefore stays on screen after either.
- Off-by-one: the last visible column is `origin.x + width - 1`.

## Ownership
Owns `origin`, `width`, `height`, and the scroll/cursor-follow math. Reads the cursor (owned by `app`/editing). Does not read or write canvas cells.

## Status
M3 (coordinate mapping) and M4 (jump / cursor-follow / resize) done, with unit tests.
