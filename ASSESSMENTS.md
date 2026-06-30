# Assessments

A running log of project health reviews. Newest first.

## 2026-06-30 — Reassessment after M10–M14

### Health: green

| Aspect | Status |
|--------|--------|
| Tests | **67 pass** (1 suite) |
| Lint | `cargo fmt --check` + `clippy -D warnings` **clean** |
| Size | ~2600 LOC across 11 modules |
| Docs | Every `src/*` feature dir has a co-located `CLAUDE.md` |
| Markers | No `TODO`/`FIXME`/`dead_code`/`unimplemented`/`todo!` |
| Milestones | M0–M14 done; open questions #1–#3 resolved |

Test distribution: editing 18, app 9, main 7, canvas 6, viewport 5, selection 5,
locations 4, persistence 4, render 3, overview 3, help 3.

### Stale docs found and fixed

The code was sound; documentation had drifted after the session's new features
(help, word-jump, mouse, selection, CLI options). Corrected:

1. **`canvas/CLAUDE.md`** — documented a `row_cells()` method that never existed
   in the code (×2 references), plus a dead-code allow that is gone. Replaced
   with the real `cells()`.
2. **`main.rs` header** — claimed persistence was "not built yet" and pointed at
   a nonexistent `cards/feature-persistence.md`; was missing
   overview/help/selection/clipboard. Now lists all modules with correct
   milestones, plus mouse capture + CLI parsing.
3. **`cards/architecture.md`** — data model, per-frame pipeline, and
   feature→dir map still described F11/F1–F10 keybindings and 10 bookmark slots,
   and omitted mouse, selection, help, and clipboard. All brought current.
4. **`render/CLAUDE.md`** — did not mention the help/overview branches or the
   selection-highlight rendering. Now documents `canvas_lines`/`style_row` and
   the real status-line contents.
5. **`README.md`** — Usage, keybindings, terminal note, and the architecture
   diagram were current, but the intro + **Features** list still omitted mouse,
   rectangle selection/clipboard, word jump, and the help overlay (and the
   `arboard` dependency). Brought current in a follow-up.

Committed as `730614e` (src/cards docs); README follow-up separately. The historical F-key references in
`project-plan.md` decisions, `locations/CLAUDE.md`, and
`decision-language-rust.md` were left as-is — they correctly record "changed
from the original F1–F10 idea," not staleness.

### Observations (not acted on)

- `clipboard.rs` is the only module without its own `CLAUDE.md` — intentional
  (single file, like `app.rs`/`main.rs`; documented inline + in
  `selection/CLAUDE.md`).
- Standing v1 limitations (single `char`/cell, no wide/Unicode-cell handling, no
  undo/redo) remain tracked in the plan's open items — nothing new.

No code changes were needed.
