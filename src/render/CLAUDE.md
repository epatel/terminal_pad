# render — paint the frame

Draws each frame: the visible canvas window, the terminal cursor, and a one-row status line. Triggered when working on what's on screen, layout, the status line, or cursor display.

## Layout
`Layout::vertical([Min(0), Length(1)])` splits the frame into the **canvas area** (top, flexible) and a **status line** (bottom, one row). The viewport's `width`/`height` are synced to the canvas area here every frame, so a terminal resize is absorbed without a separate handler (M4 adds the clamp).

## What it draws
- **Canvas window** — `window_rows(canvas, viewport)` builds one `String` per screen row (each `width` chars, blanks for unwritten cells) via `viewport.canvas_at` + `canvas.get`. Pure and unit-tested.
- **Status line** — reversed-video row showing cursor coords and written-cell count plus the quit hint. Mode (Insert/Overwrite) is added in M5.
- **Cursor** — `frame.set_cursor_position` at `viewport.screen_of(cursor)`, offset by the canvas area origin; skipped when the cursor is scrolled off-screen.

## Invariants
- Reads only — render never mutates the canvas. It does sync `viewport.{width,height}` from the frame area (the one write it owns).
- `window_rows` is side-effect-free so rendering logic is testable without a terminal.

## Ownership
Owns the on-screen layout and the status line's content. Reads `app.canvas`, `app.viewport`, `app.cursor`; writes only the viewport's size.

## Status
Implemented (M3) with unit tests for `window_rows` (cells/blanks and scrolled origin).
