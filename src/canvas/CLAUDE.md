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
- `insert_shift(x, y, ch)` — Insert-mode write; shifts the row's cells at `x' >= x` one right
- `delete_shift(x, y)` — delete `(x, y)`; shifts the row's cells at `x' > x` one left
- `row_cells(y) -> Vec<(Coord, char)>` — populated cells of a row, ascending by `x` (consumed in M8; carries a targeted `#[allow(dead_code)]` until then)

## Invariants
- A cell is present-with-a-char or absent — never present-but-blank. Erase via `clear`, not `set(' ')`.
- All edit ops act on a single row; cross-row reflow is not this feature's job (editing decides when a new row starts).

## Gotchas
- Row-wide shifts are O(populated cells in row) — fine normally; watch a single enormous row.
- One `char` per cell; combining/wide characters are a v1 limitation.

## Ownership
Writes: editing + paste handlers (M5/M6). Reads: render (visible window) (M3). This module owns the cell-storage contract only — cursor, scrolling, and bookmarks are separate features.

## Status
Implemented in `mod.rs` with unit tests (M2). Wired into render (M3) and editing (M5); the blanket dead-code allow is gone, only `row_cells` keeps a targeted one until M8.
