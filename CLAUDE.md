## About this project

terminal_pad is a terminal (TUI) application offering an **infinite 2D canvas** you can paste and edit text on, navigate with arrow keys (Shift jumps the view by 1/3 of a screen, Option/Alt+Left/Right jumps by a word, Ctrl+A/Ctrl+E jump to line start/end, Ctrl+D/Ctrl+K delete a char / the rest of the line segment-aware), toggle insert/overwrite with **Ctrl+I**, and bookmark up to nine canvas locations on **Ctrl+1..9** (Ctrl+Shift+1..9 saves the current cursor+view), zoom out to a minimap overview with **Ctrl+Z**, view a keybinding cheat sheet with **Ctrl+H**, use the mouse (click to position the cursor, drag to select a rectangle, scroll wheel to pan; Ctrl+C/Ctrl+V copy/paste the selected block, Del/Bksp clears it), and calculate inline on **[Calc]**-tagged lines (Enter evaluates `expr=` in place, assigns `name = expr` as a session variable, and chains a new tagged line, reusing the tag as typed). Target stack: **Rust** with `ratatui` (render) + `crossterm` (input/terminal) + `serde` (persistence). Single self-contained binary.

The live build state, milestones, and open questions live in @project-plan.md — read it first.

## Cards

### Architecture
- [architecture](cards/architecture.md) — cross-feature work, the per-frame pipeline, data model, onboarding

### Decisions
- [decision-language-rust](cards/decision-language-rust.md) — choosing or revisiting the language/TUI stack
- [decision-sparse-grid](cards/decision-sparse-grid.md) — how the infinite canvas is stored in memory

### Features
> All features are documented in their co-located `src/<feature>/CLAUDE.md` (auto-discovered): **canvas**, **viewport**, **render**, **editing**, **locations**, **persistence**, **overview**, **help**, **selection**, **calc**. No feature cards live here.
