# canvas — the infinite 2D grid model

The in-memory canvas: how cells are stored, addressed, and mutated. The single node of this feature; render/editing read and write through it.

## Coordinate system
Absolute integer cell coordinates `(x, y)` as `Coord` (= `i64`), unbounded in all four directions (negatives allowed). `x` grows right, `y` grows down. No origin is special.

## Storage (private)
`HashMap<(Coord, Coord), char>`. Present key → that char; absent key → blank. Memory is proportional to written cells only. The map is private — callers use the operations below (ADT: representation hidden, invariants enforced inside).

## Public surface (`Canvas`)
- `new()` / `Default`
- `get(x, y) -> Option<char>`
- `set(x, y, ch)` — overwrite a cell
- `clear(x, y)` — erase to blank (idempotent)
- `len()` — count of written cells
- `cells() -> Vec<(Coord, Coord, char)>` — all written cells, sorted by `(y, x)`; used by persistence, the overview/selection, and tests to reconstruct rows

## Invariants
- A cell is present-with-a-char or absent — never present-but-blank. Erase via `clear`, not `set(' ')`.
- The canvas stores and addresses cells; it has **no** notion of a line. Row-wide `insert_shift`/`delete_shift` primitives used to live here and were removed in M24 — they shifted *every* cell on the row, dragging a side column past a ≥2-blank gap. Shifting is the line model's job: see `layout::open_columns` (right) and `editing::delete_pull` (left).

## Gotchas
- `cells()` is O(all populated cells) and sorts; callers doing per-row work in a loop should hoist it.
- One `char` per cell — including a double-width char (CJK, emoji), which the *writers* in `editing` follow with a blank cell so it spans two canvas columns on screen (M23, see `crate::width`). The canvas itself stays width-agnostic: it stores and addresses cells, it does not know what a char looks like.

## Ownership
Writes: editing + paste handlers (M5/M6). Reads: render (visible window) (M3). This module owns the cell-storage contract only — cursor, scrolling, and bookmarks are separate features.

## Status
Implemented in `mod.rs` with unit tests (M2). Wired into render (M3), editing (M5), persistence (M8), the overview (M9), and selection (M13). No dead-code allows remain.
