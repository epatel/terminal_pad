# editing — typing, modes, deletion, newline

Cursor writes, the Insert/Overwrite mode, and how typed text lands on the canvas. Triggered when working on typing, Ctrl+I, Backspace/Delete/Enter, or the line anchor.

## State (lives on `App`)
- `mode: EditMode` — `Insert` (default) or `Overwrite`, toggled by **Ctrl+I**.
- `cursor: (Coord, Coord)` — where the next char lands.
- `anchor_x: Coord` — column a later Enter returns to; reset by navigation (`App::move_cursor`), not by typing.

## Operations (`editing::*`, act on `&mut App`)
- `type_char(app, ch)` — Insert: `canvas.insert_shift` then `cursor.x += 1`; Overwrite: `canvas.set` then `cursor.x += 1`.
- `backspace(app)` — `cursor.x -= 1`, then delete at cursor (reflow in Insert, gap in Overwrite).
- `delete(app)` — delete the cell under the cursor (reflow in Insert, gap in Overwrite); cursor stays.
- `newline(app)` — `cursor = (anchor_x, cursor.y + 1)`.
- `toggle_mode(app)` — flip Insert/Overwrite (no canvas change).
- `paste(app, text)` — drop a rectangular block at the cursor (M6): line `i` at `cursor.y + i` from `cursor.x`; `\r\n` and bare `\r` line endings normalized to `\n` (terminals send `\r` in bracketed paste). **Block placement** — overwrites covered cells regardless of mode, never pushes content down; cursor ends at the end of the last pasted line.

Key dispatch lives in `main::handle_key`; printable input is ignored when Ctrl/Alt is held. Paste arrives as one `Event::Paste(String)` (bracketed paste, enabled in `main::setup_terminal`) and is routed to `paste`.

## Invariants
- Ctrl+I only flips `mode`; it never touches a cell.
- This feature decides *which* canvas op and *where*; the canvas model decides *how* a cell is stored. Erasing returns a cell to truly-blank (not a space).
- Insert-mode shifting affects only the cursor's row.

## Decisions / open items
- **Newline column** = the line anchor (`anchor_x`), set whenever navigation repositions the cursor. v1 choice; revisit if a remembered per-line start is wanted (plan open questions).
- **Paste = block placement** (overwrite, no push-down) — resolves plan open question #3. Revisit if push-down/insert-at-cursor is wanted.
- One `char` per cell; combining/wide characters and embedded control chars (other than stripped `\r`) are a v1 limitation.
- **Ctrl+I = Tab** historically (ASCII 0x09); it only toggles mode in terminals that distinguish them via the Kitty keyboard protocol (enabled in `main::setup_terminal`). In plain Terminal.app, Ctrl+I arrives as Tab and won't toggle.

## Ownership
Owns `mode`, `anchor_x`, and the typing/deletion logic. Writes canvas cells via the model; reads/writes `cursor`; calls `viewport.scroll_to_show` after edits.

## Status
Implemented (M5) plus bracketed paste (M6), with unit tests.
