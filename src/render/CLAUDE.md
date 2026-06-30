# render — paint the frame

Draws each frame: the visible canvas window, the terminal cursor, and a one-row status line. Triggered when working on what's on screen, layout, the status line, or cursor display.

## Layout
`Layout::vertical([Min(0), Length(1)])` splits the frame into the **canvas area** (top, flexible) and a **status line** (bottom, one row). The viewport's `width`/`height` are synced to the canvas area here every frame, so a terminal resize is absorbed without a separate handler (M4 adds the clamp).

`draw` branches on app state: the **help** overlay (`app.help`) and the **overview** (`app.zoom == Overview`) each paint their own full-screen content and return early; otherwise the normal editor below.

## What it draws (normal editor)
- **Canvas window** — `window_rows(canvas, viewport)` builds one `String` per screen row (each `width` chars, blanks for unwritten cells) via `viewport.canvas_at` + `canvas.get`. Pure and unit-tested.
- **Selection highlight** — `canvas_lines` wraps `window_rows` into styled `Line`s; `style_row` splits each row into spans, reversing the runs of cells inside `app.selection`. No selection → one plain span per row.
- **Status line** — reversed-video row: mode (INS/OVR), cursor coords, written-cell count, saved bookmark marks, a `sel WxH` indicator when a selection is active, the transient `app.status`, and a compact key hint.
- **Cursor** — `frame.set_cursor_position` at `viewport.screen_of(cursor)`, offset by the canvas area origin; skipped when the cursor is scrolled off-screen (and in the help/overview branches).

## Invariants
- Reads only — render never mutates the canvas. It does sync `viewport.{width,height}` from the frame area (the one write it owns).
- `window_rows` / `style_row` are side-effect-free so rendering logic is testable without a terminal.

## Ownership
Owns the on-screen layout and the status line's content. Reads `app.canvas`, `app.viewport`, `app.cursor`, `app.selection`, `app.help`, `app.zoom`; writes only the viewport's size. Delegates the overview minimap to `overview::rows` and the help panel to `help::rows`.

## Status
Implemented (M3); selection highlight added (M13). Unit tests for `window_rows` (cells/blanks, scrolled origin) and `style_row` (span split).
