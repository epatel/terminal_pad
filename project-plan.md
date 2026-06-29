# Project Plan — terminal_pad

## Goal
Build a terminal (TUI) app that presents an infinite 2D canvas the user can paste and edit text on, navigate with arrow keys (Shift jumps the view by one-third of a screen), toggle insert/overwrite with Ctrl+I, and bookmark up to nine canvas locations on Ctrl+1..9 (Ctrl+Shift+1..9 saves the current cursor+view). Ship as a single self-contained binary. *(Keybindings revised from the original F-key idea — see Decisions.)*

## Non-goals
- Rich text / styling (colors, bold) of canvas content — plain characters only (v1).
- Multi-file / tabs / multiple canvases in one session.
- ~~Mouse support~~ (added in M12 — click to position, scroll-wheel pan), networking, collaboration.
- Syntax highlighting or language awareness.
- Undo/redo (deferred; tracked as an open question, not v1).

## Milestones
- [x] **M0 — Scaffold** — `cargo init` + deps (`ratatui` 0.30, `crossterm` 0.29, `serde` 1, `serde_json` 1); edition 2024; `.gitignore`. Builds clean. Done 2026-06-07.
- [x] **M1 — Terminal lifecycle** — raw mode + alternate screen, bracketed paste + key disambiguation enabled, panic-safe restore, blocking event loop quitting on Esc/Ctrl-Q. In `src/main.rs`. Done 2026-06-07.
- [x] **M2 — Canvas model** — sparse `(i64,i64)->char` grid + ops (get/set/clear/insert_shift/delete_shift/row_cells), 6 unit tests passing. In `src/canvas/` with co-located CLAUDE.md. Done 2026-06-07.
- [x] **M3 — Render** — `App` state (canvas+viewport+cursor); `render::draw` paints the visible window + terminal cursor + status line; viewport coord math (`canvas_at`/`screen_of`) with tests; 10 tests passing. In `src/app.rs`, `src/render/`, `src/viewport/`. Done 2026-06-07.
- [x] **M4 — Navigation** — arrow keys move the cursor with cursor-follow scroll; Shift+arrow jumps the view by 1/3 screen; resize re-syncs + re-clamps. `viewport::{step,jump,scroll_to_show}`, `App::{move_cursor,jump_view}`, `main::handle_key`; 15 tests passing. Done 2026-06-07.
- [x] **M5 — Editing** — typing writes at cursor (Insert shifts row / Overwrite replaces); Ctrl+I toggles mode (shown in status line); Backspace/Delete (reflow vs gap by mode); Enter → line anchor on next row. `src/editing/`, mode+anchor on `App`; 23 tests passing. Done 2026-06-07.
- [x] **M6 — Paste** — `Event::Paste` routed to `editing::paste`: block placement at cursor, `\r\n` normalized, cursor to end of block. 27 tests passing. Done 2026-06-07.
- [x] **M7 — Locations** — 9 slots: Ctrl+1..9 jump, Ctrl+Shift+1..9 save (cursor + viewport origin). Status line shows saved marks. `src/locations/`; 31 tests passing. Done 2026-06-07.
- [x] **M12 — Mouse** — mouse capture enabled; left-click positions the cursor at the canvas cell (`App::click_to`), scroll wheel pans the view vertically (`App::scroll_rows`); routed via `main::handle_mouse` (normal mode only). Reverses the original "no mouse" non-goal. `src/main.rs`, `src/app.rs`; 50 tests passing. Done 2026-06-30.
- [x] **M11 — Word jump** — Option/Alt+Left/Right move the cursor by a word on its row (non-whitespace runs; whitespace cells separate words). `editing::{word_left,word_right}`, `main::handle_key`; 48 tests passing. Done 2026-06-30.
- [x] **M10 — Help overlay** — Ctrl+H toggles a read-only centered cheat sheet listing every keybinding (any key dismisses it); takes priority over the overview branch. `src/help/`; 43 tests passing. Done 2026-06-30.
- [x] **M9 — Overview (zoom-out)** — Ctrl+Z toggles a read-only minimap: whole-canvas density map (`#`/`=`/`-`/blank) in a box-drawing frame, with the current view window overlaid as a box. `src/overview/`; 39 tests passing. Done 2026-06-07.
- [x] **M8 — Persistence** — JSON (serde) of cells + 9 slots + cursor; CLI path arg default `./canvas.tpad`; atomic save (temp+rename); load on startup (malformed → abort, never clobber); Ctrl+S + auto-save on exit. `src/persistence/`; 35 tests passing (incl. real-file round-trip). Done 2026-06-07.

## Decisions
- **2026-06-07 — Language/stack: Rust + ratatui + crossterm + serde.** *(Locked)* Reliable Shift+arrow / F-key modifier detection and first-class bracketed-paste come from `crossterm`; `ratatui` handles rendering; single static binary distribution. See [decision-language-rust](cards/decision-language-rust.md). Revisit signal: user prefers C++/Node, or a hard dependency forces a runtime.
- **2026-06-07 — Canvas storage: sparse map keyed by (i64, i64).** *(Locked, pending implementation)* Truly unbounded canvas, only written cells cost memory. See [decision-sparse-grid](cards/decision-sparse-grid.md).
- **2026-06-07 — Location keybindings: Ctrl+1..9 jump / Ctrl+Shift+1..9 save; saves cursor+view.** *(Locked)* Replaces the original F1–F10 idea (Fn keys are unreliable on macOS) → 9 slots. A slot stores both cursor and viewport origin so a jump restores the exact screen state. Caveat: Ctrl+digit modifier reporting needs the Kitty keyboard protocol (kitty/WezTerm/Ghostty/recent iTerm2); plain Terminal.app may not distinguish it. See src/locations/CLAUDE.md.
- **2026-06-07 — Insert/Overwrite toggle: Ctrl+I (was F11).** *(Locked)* Avoids the Fn-key problem on macOS. Caveat: Ctrl+I shares ASCII 0x09 with Tab, so it only toggles in terminals that distinguish them via the Kitty keyboard protocol.

## Current state / handoff
**All milestones M0–M12 complete.** Full feature set: infinite sparse canvas; arrow navigation with cursor-follow (Option/Alt+Left/Right jump by a word); mouse (left-click positions the cursor, scroll wheel pans); Shift+arrow 1/3-screen pan that carries the cursor (reversible); typing with Insert/Overwrite (Ctrl+I); Backspace/Delete/Enter; bracketed paste (block placement, CR/CRLF-normalized); 9 bookmarks (Ctrl+1..9 jump / Ctrl+Shift+1..9 save cursor+view); JSON persistence (Ctrl+S + auto-save on exit, atomic write, CLI path arg → `./canvas.tpad`); Ctrl+Z zoom-out minimap overview (arrows pan by a screenful while zoomed out); Ctrl+H keybinding cheat-sheet overlay (any key dismisses); Option/Alt+Left/Right word jump; mouse capture (click to position the cursor, scroll-wheel pan). Modules each have a co-located CLAUDE.md: canvas, viewport, render, editing, locations, persistence, overview, help; cards/ now holds only architecture + the two decisions. `Makefile` wraps run/build/test/check/fmt/clippy/lint/clean. **50 tests pass; `make lint` (fmt-check + clippy `-D warnings`) clean.** Toolchain via rustup (Rust 1.96.0); `~/.cargo/env` in `~/.zshrc`. Run with `make run [path]` (Esc/Ctrl-Q).

**Open items / next:** (1) Drive the real TUI to confirm interactivity — not done here (no interactive tty); the control scheme (Ctrl+I, Ctrl+1..9, Ctrl+Shift+1..9) assumes a Kitty-keyboard-protocol terminal on macOS (kitty/WezTerm/Ghostty/recent iTerm2), not plain Terminal.app. (2) Mouse capture (M12) takes over the terminal's native click-to-select/copy and scroll — users hold Shift/Option for the terminal's own selection; revisit if that proves annoying (e.g. a toggle to release capture). (3) Possible v2 work: undo/redo (open question #5), wide/Unicode-cell handling, dirty-flag to skip no-op exit saves.

## Open questions
1. ~~**Persistence location**~~ — RESOLVED (M8): first CLI arg, defaulting to `./canvas.tpad`.
2. ~~**What does the save key store?**~~ — RESOLVED (M7): rebound to Ctrl+1..9 / Ctrl+Shift+1..9; a save stores both cursor and viewport origin.
3. ~~**Paste placement**~~ — RESOLVED (M6): rectangular block at the cursor, overwriting covered cells (no push-down).
4. **Overwrite at end-of-content** — in overwrite mode, does typing past existing cells extend the canvas (assumed yes, infinite)?
5. **Undo/redo** — out of scope for v1; revisit after M8?
