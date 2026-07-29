# selection — rectangular block selection

A rectangular selection of canvas cells made by left-click-drag, with copy / paste / clear. Triggered when working on selection, drag, the clip buffer, or Ctrl+C/V.

## Model (`Selection`, pure value)
Two corners in absolute canvas coords: `anchor` (drag start) and `head` (dragged corner); either may hold the min or max. Helpers: `bounds()` (inclusive `min/max`), `contains(x,y)`, `is_point()` (single cell = a click, not a drag), `width()`, `height()`.

## Pure canvas ops (`selection::*`)
- `extract(canvas, sel) -> Vec<Vec<char>>` — the rectangle as rows of chars, blanks → `' '` (always `width` wide).
- `to_text(block) -> String` — newline-joined, trailing blanks trimmed per row (the system-clipboard form).
- `clear(canvas, sel)` — erase every cell in the rectangle back to truly-blank.
- `word_span(canvas, x, y) -> Option<(Coord, Coord)>` — inclusive column range of the "word" under a cell, for double-click select. **Word** chars are alphanumeric + `_`; **joiners** (`. - : / ? # @ % & = + ~`) count only *between* word chars — the run is expanded over word+joiner cells then trimmed of edge joiners, so a whole URL / path / version (`https://example.com/a?x=1&y=2`, `src/main.rs`, `v1.2.3-rc`) selects as one unit but the period ending the sentence does not. Everything else is **punct**: a click there (or on a trimmed-off edge joiner, or a free-standing `--`) selects the maximal punctuation run, so `hello, world` gives `hello` or `,` and `->(` groups as one. Blank/whitespace → `None`.

## State (lives on `App`)
- `selection: Option<Selection>` — current rectangle (highlighted via reversed video in `render`).
- `dragging: bool` — a drag is in progress (between mouse-down and mouse-up).
- `last_click: Option<(Instant, (Coord, Coord), u8)>` — time, cell and run length of the last left-press, for double/triple-click detection (crossterm reports presses, not click counts).
- `clip: Option<Vec<Vec<char>>>` — internal copy buffer (Ctrl+C → Ctrl+V).

## Orchestration (`App::*`)
- `begin_drag(sx,sy)` — position the cursor (`click_to`) and start a zero-size selection.
- `update_drag(sx,sy)` — extend `head` to the cell, carrying the cursor (no-op if not dragging).
- `end_drag()` — finish; a selection that never grew past one cell was a click, so it's dropped. No-op unless `dragging` — the mouse-up after a double-click must not eat a one-char word selection.
- `register_click(now, cell) -> u8` — record a left-press and report the click count in the run (1/2/3). Clicks chain while they hit the same cell within `DOUBLE_CLICK` (400 ms); a fourth click, another cell, or a longer pause restarts at 1.
- `select_word()` / `select_line()` — select the `selection::word_span` / `layout::line_bounds` range under the cursor via the shared `select_span(s, e, y)`: a 1-row selection with the status set to `selected "…" (Ctrl+C to copy)`, returning the text. **Selection only** — `clip` and the system clipboard are untouched, so copying stays Ctrl+C's job (one copy path, no surprise clipboard writes from a stray click). `None` off a word / line (cursor just moves).
- `copy_selection() -> Option<String>` — fill `clip`, return text for the **caller** to write to the system clipboard (keeps OS I/O out of `App` so its tests don't touch the real clipboard).
- `delete_selection()` — `selection::clear` then drop the selection.
- `paste_clip()` — drop `clip` as a block at the cursor: a blank source cell **erases** the destination, any other char overwrites (so the rectangle lands cleanly); cursor ends at the block's end.

## Wiring (`main`)
- Mouse (`handle_mouse`, normal mode only): `Down(Left)` → `register_click`; a double/triple-click positions the cursor then `select_word` / `select_line`, otherwise `begin_drag`. `Drag(Left)` → `update_drag` (coords clamped to the canvas area), `Up(Left)` → `end_drag`.
- Keys (`handle_key`): **Ctrl+C** copy (then `clipboard::set_system`, best-effort), **Ctrl+V** paste, **Del/Bksp** clear the block when a selection exists, **Esc** cancels a selection before it reaches quit (in the event loop). Any other key dismisses a lingering selection.
- System clipboard write is isolated in `crate::clipboard` (text-only `arboard` wrapper, graceful failure). The internal `clip` always works even if the OS clipboard is unavailable.

## Invariants
- The `Selection` model and the `extract`/`to_text`/`clear` ops are pure (no `App`, no I/O) and unit-tested.
- Selection coords are absolute canvas coords, so the highlight tracks content when the view scrolls.
- One `char` per cell (same v1 limitation as the canvas); paste treats `' '` as "erase".

## Status
Implemented (M13; double/triple-click word & line select M22) with unit tests (model bounds/contains, extract/to_text/clear; `App` drag/copy/delete/paste; render span split). Possible later work: auto-scroll while dragging past the viewport edge; move (cut+paste) gesture; read the system clipboard for Ctrl+V.
