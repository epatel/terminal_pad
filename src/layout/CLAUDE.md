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
- `line_bounds(canvas, cx, cy) -> Option<(start, end)>` — the segment on row `cy`
  with `start <= cx <= end + 1` (the typing position one past the end counts).
  `None` on a blank row or when `cx` is parked inside a ≥2-blank gap. Pure.
- `make_room(canvas: &mut Canvas, (lo, hi), at_row)` — open `at_row` across band
  `[lo, hi]` by pushing **whole lines** down. Every segment on `at_row` overlapping
  the band moves down one row; any segment it would land on moves too, cascading
  down until the shift reaches free space. Lines move **as units** (a wide line
  below a narrow band is *not* torn), while a segment that never overlaps — a side
  column past a ≥2-blank gap — stays put. No-op if the band is already free.
  Implemented as a downward overlap-flood over segments. Private helpers:
  `filled_columns`, `segments_on_row`, `overlaps`.

## How Enter uses it (`editing::newline`)
`line_bounds(cursor)` → if the cursor is mid-line (`cx <= end`): capture the
trailing run `cx..=end`, `make_room` on `cy+1` for the band `(start, start+(end-cx))`,
clear the tail on `cy`, block-place the run on `cy+1` at `start` (set non-blank,
clear blank — preserving internal single spaces), cursor → `(start, cy+1)`. At/after
the line end it just drops the cursor to `(start, cy+1)`; on a blank row it falls
back to the saved `anchor_x`.

## Invariants
- `line_bounds` is pure; `make_room` only ever moves cells **down** and never
  overwrites (it shifts whole lines into free space), so no content is lost.
- Lines move as whole units; only segments that overlap the band (directly or via
  the downward cascade) move — a side column past a ≥2-blank gap is untouched, as
  is the source row above.

## Status
Implemented (M15) with unit tests (`line_bounds` single-space merge / ≥2-gap split
/ end+1 / blank row; `make_room` no-op / single shift / cascade / gap-stop /
whole-wide-line / cascaded-wide-lines / side-column-untouched).
