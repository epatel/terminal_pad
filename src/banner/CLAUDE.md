# banner — figlet big-letter typing (Ctrl+B)

Stamps typed characters onto the canvas as multi-row FIGlet glyphs in the classic `smslant` font, matching `figlet -f smslant` output. Triggered when working on banner mode, the embedded font, or the kerning/smushing logic.

## Behavior
- **Ctrl+B** toggles `App::banner` (session-only, not persisted — like the calc state). The status line's mode chip shows `INS+BAN` / `OVR+BAN` while on.
- **Typing** a printable char (no Ctrl/Alt) stamps its glyph with its **top-left at the cursor** and advances the cursor to the glyph's right edge. A char with no glyph (non-ASCII) reports in the status line and does nothing.
- **Enter** drops the cursor one glyph height (5 rows) down, back at `anchor_x`. It deliberately bypasses `editing::newline` (and calc/todo) — a line split/push-down would shred the art.
- Everything else (arrows, Backspace/Delete, selection, bookmarks…) behaves exactly as in normal mode; banner mode only reroutes printable typing and Enter (`main::handle_key`).

## Font (`smslant.flf`, vendored)
The unmodified upstream FIGfont, embedded via `include_str!` and parsed once into a `OnceLock<Font>`: header (`flf2a$ 5 4 … 15 10 …` → hardblank `$`, height 5, old layout 15 = smush rules 1–4, 10 comment lines), then one glyph per ASCII 32..=126 (endmark-stripped, rows padded to uniform width). Latin-1 / code-tagged glyphs at the tail are ignored.

## Kerning & smushing (`fit_overlap` + `smush`)
figlet's horizontal kern/smush, re-derived against the sparse canvas instead of a line buffer:
- Per glyph row: gap back to the nearest existing cell left of the cursor (≤ 32-col lookback) + the row's leading blanks, **+1** when the boundary pair is smushable. The glyph shifts left by the **minimum over rows** (no left content → 0; capped at glyph width).
- Smush rules 1–4 (what smslant's layout declares): equal char, underscore replacement, class hierarchy (`| < /\ < [] < {} < () < <>`), opposite brackets → `|`.
- **Hardblanks** are solid during fitting (they block smushing, never match rule 1) and are written as real `' '` cells — invisible, but present so the next glyph can't kern through a typed space. A collision on write falls back to glyph-wins (hardblank never erases).

## Invariants
- Glyph blanks are transparent: stamping never clears existing canvas cells.
- Unit-tested for byte-for-byte fidelity against real `figlet -f smslant` output ("Hi", "AB", "Test", "a b" fixtures).

## Ownership
Owns the embedded font, its parser, and the stamp/kern logic. `App` holds the `banner: bool` flag; `main::handle_key` routes Ctrl+B, printable chars, and Enter; `render` shows the `+BAN` chip; `help` lists the binding.

## Status
Implemented (M21) with unit tests (font parse, figlet fixtures, smush rules, toggle, newline, missing glyph).
