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
- `newline(app)` — **split the line at the cursor** (Enter). The trailing run (words joined by single spaces, up to the first ≥2-blank gap) moves down one row, left-aligned to the line's start column, pushing the block(s) below down to make room (`layout::make_room`, see `src/layout/`). Cursor at/after the line end just drops to the line start on the next row; a blank row falls back to `anchor_x`.
- `line_start(app)` / `line_end(app)` — **Ctrl+A / Ctrl+E** jump the cursor to the start / one-past-the-end (typing position) of the line under it, per `layout::line_bounds` — so in a side column past a ≥2-blank gap they stay within that segment. Already at that edge, or off any line (empty space, blank stretch, past end+1): they **hop to the neighboring part** on the row — Ctrl+E to the next part's start on the right, Ctrl+A to the previous part's end on the left (via `layout::segments_on_row`) — so alternating them walks the row's parts. No-op only when nothing lies in that direction (or the row is blank). Plain ASCII control codes (0x01/0x05) — work in any terminal, no Kitty protocol needed. No canvas change; resets `anchor_x` like other navigation.
- `word_left(app)` / `word_right(app)` — **Option/Alt+Left/Right** jump the cursor by a word on its row. A word is a maximal run of non-whitespace cells (a typed space is a stored cell, so whitespace — not just an absent cell — separates words). `word_right` from inside a word lands on the next word's start, and past the last word lands one column after the final word char (end-of-content); `word_left` lands on the current word's start, then earlier word starts. No canvas change; resets `anchor_x` like other navigation.
- `toggle_mode(app)` — flip Insert/Overwrite (no canvas change).
- `paste(app, text)` — drop a rectangular block at the cursor (M6): line `i` at `cursor.y + i` from `cursor.x`; `\r\n` and bare `\r` line endings normalized to `\n` (terminals send `\r` in bracketed paste). **Block placement** — overwrites covered cells regardless of mode, never pushes content down; cursor ends at the end of the last pasted line.

Key dispatch lives in `main::handle_key`; printable input is ignored when Ctrl/Alt is held. Paste arrives as one `Event::Paste(String)` (bracketed paste, enabled in `main::setup_terminal`) and is routed to `paste`.

## Invariants
- Ctrl+I only flips `mode`; it never touches a cell.
- This feature decides *which* canvas op and *where*; the canvas model decides *how* a cell is stored. Erasing returns a cell to truly-blank (not a space).
- Insert-mode shifting affects only the cursor's row.

## Decisions / open items
- **Enter splits the line** (M15) — the trailing single-space-joined run moves down to the line's *content-derived* start column, pushing blocks below down (`layout::make_room`). Supersedes the earlier anchor-only newline. `anchor_x` is still the fallback for a blank row / end-of-line drop, because navigation resets it and can't be trusted as the line start.
- **Paste = block placement** (overwrite, no push-down) — resolves plan open question #3. Revisit if push-down/insert-at-cursor is wanted.
- One `char` per cell; combining/wide characters and embedded control chars (other than stripped `\r`) are a v1 limitation.
- **Ctrl+I = Tab** historically (ASCII 0x09); it only toggles mode in terminals that distinguish them via the Kitty keyboard protocol (enabled in `main::setup_terminal`). In plain Terminal.app, Ctrl+I arrives as Tab and won't toggle.
- **Option/Alt+Left/Right word jump** needs the terminal to report the Alt modifier on arrow keys (Kitty keyboard protocol). Some macOS terminals instead send `ESC b`/`ESC f` for Option+arrow unless "Use Option as Meta" / kitty protocol is on; there it won't word-jump. Same caveat family as Ctrl+I / Ctrl+digit.

## Ownership
Owns `mode`, `anchor_x`, and the typing/deletion logic. Writes canvas cells via the model; reads/writes `cursor`; calls `viewport.scroll_to_show` after edits.

## Status
Implemented (M5) plus bracketed paste (M6), with unit tests.
