## About this project

terminal_pad is a terminal (TUI) application offering an **infinite 2D canvas** you can paste and edit text on, navigate with arrow keys (Shift jumps the view by 1/3 of a screen, Option/Alt+Left/Right jumps by a word, Ctrl+A/Ctrl+E jump to line start/end, Ctrl+D/Ctrl+K delete a char / the rest of the line segment-aware), toggle insert/overwrite with **Ctrl+I**, and search the canvas with **Ctrl+F** (a live find: type to search, Ctrl+F cycles the hits, Enter keeps the spot, Esc goes back — `#api` finds a `#name` tag, since tags are just text), bookmark up to nine canvas locations on **Ctrl+1..9** (Ctrl+Shift+1..9 saves the current cursor+view), zoom out to a minimap overview with **Ctrl+Z**, view a keybinding cheat sheet with **Ctrl+H**, use the mouse (click to position the cursor, drag to select a rectangle, double-click to select the word under the pointer, triple-click for its whole line, scroll wheel to pan; Ctrl+C/Ctrl+V copy/paste the selected block, Del/Bksp clears it), calculate inline on **[Calc]**-tagged lines (Enter evaluates `expr=` in place, assigns `name = expr` as a session variable, and chains a new tagged line, reusing the tag as typed), keep checklists going (Enter on a line starting with `[ ]`/`[x]` chains a fresh `[ ] ` item; a bare `[ ] ` ends the list), and type figlet-style banners with **Ctrl+B** (banner mode stamps each char as a big `smslant` glyph with figlet-faithful kerning; Enter drops a glyph height). Target stack: **Rust** with `ratatui` (render) + `crossterm` (input/terminal) + `serde` (persistence). Single self-contained binary.

The live build state, milestones, and open questions live in @project-plan.md — read it first.

## Known stack — use what you already know

Standard crates used the standard way; no cards for them — read the code and apply the usual idioms.

- **ratatui 0.30** immediate-mode rendering: one `render::draw(frame, &mut app)` per loop turn (`src/render/`); widgets are plain `Paragraph`/spans, no stateful widgets.
- **crossterm 0.29** input + terminal: raw mode, alt screen, bracketed paste, mouse capture and the Kitty keyboard-enhancement flags are all set up and torn down (panic-safe) in `src/main.rs`; key dispatch is `main::handle_key` / `handle_mouse`.
- **serde + serde_json**: one versioned `Doc` in `src/persistence/`.
- **arboard** for the system clipboard, isolated in `src/clipboard.rs` (fails gracefully). **evalexpr 11** for `[Calc]` (`src/calc/`). **unicode-width** behind `src/width.rs`.
- Tests are inline `#[cfg(test)] mod tests` per module; no integration-test dir.

## Commands

- `make test` — all tests; one module or test: `cargo test find::` / `cargo test <name>` (cargo is in `~/.cargo/bin`; the Makefile sources `~/.cargo/env` for you)
- `make lint` — `cargo fmt --check` + clippy `-D warnings`; must be clean before a commit
- `make run` — debug run on `./canvas.tpad`; a specific pad: `cargo run -- <path>` or `cargo run -- --name <name>`
- `make build` / `make release`; releases are built by GitHub Actions on a pushed `vX.Y.Z` tag (README → Releasing)
- Edited `.rs` files are `rustfmt`-ed automatically by a PostToolUse hook (`.claude/hooks/rustfmt.sh`) — no need to run `make fmt` after edits.

## Code health
- Before committing, run `.venv/bin/aihealth delta` and fix new or worsened smells it reports in code you touched (exit code 1 = health degraded).
- `.venv/bin/aihealth scan .` gives an overview of the codebase.
- Install (pinned, per clone): `python3 -m venv .venv && .venv/bin/pip install "git+https://github.com/epatel/ai-health@v0.2.2"` — needs Python 3.11+.

## Cards

### Architecture
- [architecture](cards/architecture.md) — cross-feature work, the per-frame pipeline, data model, onboarding

### Decisions
- [decision-language-rust](cards/decision-language-rust.md) — choosing or revisiting the language/TUI stack
- [decision-sparse-grid](cards/decision-sparse-grid.md) — how the infinite canvas is stored in memory

### Defaults
- [standing-defaults](cards/standing-defaults.md) — before adding Python tooling or changing the `.tpad` on-disk format (venv, versioned migrations)

### Features
> All features are documented in their co-located `src/<feature>/CLAUDE.md` (auto-discovered): **canvas**, **viewport**, **render**, **editing**, **layout**, **locations**, **persistence**, **overview**, **help**, **selection**, **calc**, **todo**, **banner**, **find**. No feature cards live here.

When you commit a change that adds non-obvious logic, settles an architectural choice, or makes a card above wrong, update or add the card in that commit and keep this index in sync. Proposed diffs and answers need no card edits. Cards stay self-contained: no card links to another.
