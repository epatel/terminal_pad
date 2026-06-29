# help — the keybinding cheat-sheet overlay

A read-only centered panel listing every keybinding, toggled by **Ctrl+H**. Triggered when working on the help overlay, the bindings list, or onboarding hints.

## Behavior
- **Ctrl+H** sets `App::help = true` (handled in `main::handle_key`, before all other dispatch).
- While help is showing, **any key dismisses it** (`app.help = false`) — it's a read-only cheat sheet, no other input is acted on. Esc/Ctrl+Q still quit (checked before dispatch).
- Takes priority over the overview: the `app.help` branch in `render::draw` runs before the `ZoomMode::Overview` branch.

## Rendering (`help::rows`, pure)
`rows(width, height) -> Vec<String>` builds a centered box-drawing panel over a blank field:
1. Body = title, the `BINDINGS` table (right-aligned key column + description, empty pairs are spacer rows), then a footer.
2. Panel width = inner content + border + one-space padding each side; panel height = body + 2.
3. Centered within `width x height`; if the area is too small for the panel, returns a blank screen.

`render::draw` branches on `app.help`: paints `rows(...)` plus a `HELP …` status line and skips the text cursor.

## Invariants
- Pure and side-effect-free (testable without a terminal).
- Output is exactly `height` rows, each `width` chars.
- `BINDINGS` is the single source of truth for the list — add a binding here when adding a keybinding elsewhere.

## Ownership
Owns the `help` overlay construction and the `BINDINGS` list. `App` holds the `help: bool` flag. Reads nothing from the canvas/viewport.

## Status
Implemented with unit tests (exact dimensions, framed-panel content, too-small fallback).
