# decision-sparse-grid

How the infinite canvas is represented in memory, and the trade-offs.

## Choice
Store the canvas as a sparse map: `HashMap<(i64, i64), char>`, keyed by absolute `(x, y)` cell coordinates. Absent keys render as blank. No fixed width, height, or origin — coordinates extend in all four directions, including negative.

## Deciding constraints
- **Truly infinite.** A dense 2D array (`Vec<Vec<char>>`) forces a fixed or repeatedly-reallocated bound and wastes memory on blank regions; the spec calls for an *infinite* canvas.
- **Memory proportional to content.** Only written cells cost memory, so a mostly-empty canvas with a few pasted blocks stays tiny.
- **Simple edits.** Set/get/clear a cell are O(1) average. "Insert shifts the row's trailing cells right" is a bounded scan over the populated cells of that row.

## Alternatives considered
- **Dense 2D array** — simplest indexing, but bounded and memory-hungry; rejected for an infinite canvas.
- **Per-row rope / gap buffer** — great for long single lines, more machinery than needed for a sparse 2D pad; revisit only if huge single-row edits become a bottleneck.

## Trade-offs / watch-outs
- Iterating "the visible window" means querying each visible `(x, y)` (cheap for a terminal-sized rectangle) rather than scanning the whole map.
- Row-wise operations (insert-shift, line length) need either a scan or an auxiliary per-row index; add the index only if profiling demands it.
- Serialization: persist as a list of `{x, y, ch}` entries (or run-length per row) rather than a giant grid.

## Status
Locked pending implementation (recorded 2026-06-07).
