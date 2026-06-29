# selection — rectangular block selection

A rectangular selection of canvas cells made by left-click-drag, with copy / paste / clear. Triggered when working on selection, drag, the clip buffer, or Ctrl+C/V.

## Model (`Selection`, pure value)
Two corners in absolute canvas coords: `anchor` (drag start) and `head` (dragged corner); either may hold the min or max. Helpers: `bounds()` (inclusive `min/max`), `contains(x,y)`, `is_point()` (single cell = a click, not a drag), `width()`, `height()`.

## Pure canvas ops (`selection::*`)
- `extract(canvas, sel) -> Vec<Vec<char>>` — the rectangle as rows of chars, blanks → `' '` (always `width` wide).
- `to_text(block) -> String` — newline-joined, trailing blanks trimmed per row (the system-clipboard form).
- `clear(canvas, sel)` — erase every cell in the rectangle back to truly-blank.

## State (lives on `App`)
- `selection: Option<Selection>` — current rectangle (highlighted via reversed video in `render`).
- `dragging: bool` — a drag is in progress (between mouse-down and mouse-up).
- `clip: Option<Vec<Vec<char>>>` — internal copy buffer (Ctrl+C → Ctrl+V).

## Orchestration (`App::*`)
- `begin_drag(sx,sy)` — position the cursor (`click_to`) and start a zero-size selection.
- `update_drag(sx,sy)` — extend `head` to the cell, carrying the cursor (no-op if not dragging).
- `end_drag()` — finish; a selection that never grew past one cell was a click, so it's dropped.
- `copy_selection() -> Option<String>` — fill `clip`, return text for the **caller** to write to the system clipboard (keeps OS I/O out of `App` so its tests don't touch the real clipboard).
- `delete_selection()` — `selection::clear` then drop the selection.
- `paste_clip()` — drop `clip` as a block at the cursor: a blank source cell **erases** the destination, any other char overwrites (so the rectangle lands cleanly); cursor ends at the block's end.

## Wiring (`main`)
- Mouse (`handle_mouse`, normal mode only): `Down(Left)` → `begin_drag`, `Drag(Left)` → `update_drag` (coords clamped to the canvas area), `Up(Left)` → `end_drag`.
- Keys (`handle_key`): **Ctrl+C** copy (then `clipboard::set_system`, best-effort), **Ctrl+V** paste, **Del/Bksp** clear the block when a selection exists, **Esc** cancels a selection before it reaches quit (in the event loop). Any other key dismisses a lingering selection.
- System clipboard write is isolated in `crate::clipboard` (text-only `arboard` wrapper, graceful failure). The internal `clip` always works even if the OS clipboard is unavailable.

## Invariants
- The `Selection` model and the `extract`/`to_text`/`clear` ops are pure (no `App`, no I/O) and unit-tested.
- Selection coords are absolute canvas coords, so the highlight tracks content when the view scrolls.
- One `char` per cell (same v1 limitation as the canvas); paste treats `' '` as "erase".

## Status
Implemented (M13) with unit tests (model bounds/contains, extract/to_text/clear; `App` drag/copy/delete/paste; render span split). Possible later work: auto-scroll while dragging past the viewport edge; move (cut+paste) gesture; read the system clipboard for Ctrl+V.
