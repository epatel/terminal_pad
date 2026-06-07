# overview — the zoomed-out minimap

A whole-canvas density map, toggled by **Ctrl+Z**. Triggered when working on zoom-out, the minimap, density glyphs, or the view-box overlay.

## Behavior
- **Ctrl+Z** toggles `App::zoom` between `Normal` and `Overview` (handled in `main::handle_key`, works in either mode).
- **Arrows** pan the underlying view (and cursor) by a screenful (`App::pan_view`) for quick navigation — zooming back in lands you at the new spot.
- Otherwise read-only: editing keys are ignored; Ctrl+S still saves; Esc/Ctrl+Q still quit (checked before dispatch).

## Rendering (`overview::rows`, pure)
`rows(canvas, viewport, width, height) -> Vec<String>` builds the minimap for the canvas drawing area:
1. Bounding box = content bounds **unioned with the current view rect** (so the view box is always visible).
2. Each inner cell maps to a tile of `tile_w x tile_h` canvas cells (ceil division to fit `width-2 x height-2`).
3. Glyph by fill density (written cells / tile capacity): blank = empty, `-` < 0.10, `=` < 0.35, `#` otherwise.
4. The normal-view window is overlaid as a box-drawing rectangle (`┌┐└┘─│`), expanded to ≥2x2 so it's always a visible box; interior content shows through.
5. Everything is wrapped in a box-drawing frame (= the unioned extent).

`render::draw` branches on `app.zoom`: in `Overview` it paints `rows(...)` plus an `OVERVIEW …` status line and skips the text cursor.

## Invariants
- Pure and side-effect-free (testable without a terminal).
- Output is exactly `height` rows, each `width` chars; box-drawing glyphs are single display-width.
- Density thresholds (`SPARSE`, `MEDIUM`) are heuristic constants — tune freely.

## Ownership
Owns `ZoomMode` and the minimap construction. Reads the canvas (`cells()`) and the viewport; writes nothing. `App` holds the `zoom` flag and `toggle_zoom`.

## Status
Implemented (M9) with unit tests (frame/dimensions, dense→`#`, view-box overlay). Possible later work: pan the view box with arrows while zoomed; per-block labels.
