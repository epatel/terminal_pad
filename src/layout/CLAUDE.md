# layout — the "line" model + make-room push-down

The notion of a *line* (a run of words joined by single spaces) and the vertical
"make room" shift that Enter uses to split a line without clobbering what's below.
Triggered when working on Enter/`newline`, line detection, or block push-down.

## The model
- A column on a row is **filled** when it holds a non-whitespace char. A typed
  space is a *stored* cell, so a single space — and a single absent cell — both
  read as one blank.
- A **line** is a maximal run of filled columns where consecutive filled columns
  are ≤2 apart (≤1 blank between). A gap of **≥2 blank columns** ends the line, so
  separate blocks on the same row (e.g. a side `NOTES` column) are distinct lines.

## API (`layout::*`)
- `line_bounds(canvas, cx, cy) -> Option<(start, end)>` — the line on row `cy`
  with `start <= cx <= end + 1` (the typing position one past the end counts).
  `None` on a blank row or when `cx` is parked inside a ≥2-blank gap. Pure.
- `make_room(canvas: &mut Canvas, (lo, hi), at_row)` — ensure `at_row` is free
  within column band `[lo, hi]` by shifting the contiguous occupied band-rows from
  `at_row` down by one into the first fully-free row at/below it. No-op if `at_row`
  is already free. Moving the whole occupied stack into the first slack row **is**
  the cascade: stacked lines move together; a block already separated by a blank
  row below stays put. Private helpers: `filled_columns`, `first_free_row`.

## How Enter uses it (`editing::newline`)
`line_bounds(cursor)` → if the cursor is mid-line (`cx <= end`): capture the
trailing run `cx..=end`, `make_room` on `cy+1` for the band `(start, start+(end-cx))`,
clear the tail on `cy`, block-place the run on `cy+1` at `start` (set non-blank,
clear blank — preserving internal single spaces), cursor → `(start, cy+1)`. At/after
the line end it just drops the cursor to `(start, cy+1)`; on a blank row it falls
back to the saved `anchor_x`.

## Invariants
- `line_bounds` is pure; `make_room` only ever moves cells **down** and never
  overwrites (it shifts into free space), so no content is lost.
- Push-down is **band-scoped** — only the line's own columns shift; content in
  other columns (and the source row above) is untouched.

## Known limitation
Band-scoped push-down can **tear** a block on the row below that is *wider* than
the moved line's band (only its in-band columns shift). Fine for prose with
similar-width lines. A full "rectangle hierarchy" (move whole connected blocks as
units, with cascade) is a possible follow-up — see `project-plan.md`.

## Status
Implemented (M15) with unit tests (`line_bounds` single-space merge / ≥2-gap split
/ end+1 / blank row; `make_room` no-op / single shift / cascade / gap-stop /
other-columns-untouched).
