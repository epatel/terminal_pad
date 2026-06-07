# locations — the nine bookmarks

Saved canvas spots: jump to one, or save the current one. Triggered when working on Ctrl+digit bindings, bookmarks, or restoring a view.

## Bindings
- **Ctrl+1 .. Ctrl+9** — jump to slot N (1-based).
- **Ctrl+Shift+1 .. Ctrl+Shift+9** — save the current location into slot N.

Nine slots (digits 1–9), indexed 0–8 internally. (Changed from the original F1–F10 idea — Fn keys are unreliable on macOS.)

## State (lives on `App`)
`locations: [Option<Location>; SLOT_COUNT]` where `Location { cursor, origin }` records **both** the cursor and the viewport origin — so a jump restores the exact screen state, not just where the cursor was (plan decision: "save both", open question #2).

## Operations (`locations::*`, act on `&mut App`)
- `slot_for_digit(c) -> Option<usize>` — map '1'..'9' to slot 0..8.
- `save(app, slot)` — store `{cursor, viewport.origin}` into the slot.
- `jump(app, slot)` — restore the slot's cursor + origin, reset the line anchor, then `scroll_to_show` (corrects the view if the terminal shrank since the save). No-op for an empty or out-of-range slot.

Dispatch lives in `main::handle_key`.

## Invariants
- Exactly nine slots; saving is non-destructive to the canvas (records coordinates only).
- Jump to an empty slot does nothing.

## Gotchas
- **Terminal support**: Ctrl+digit only reports a modifier under the Kitty keyboard protocol (enabled in `main::setup_terminal` when supported). In terminals without it (e.g. macOS Terminal.app), Ctrl+digit may not be distinguishable — prefer kitty / WezTerm / Ghostty / recent iTerm2.
- Slots are in-memory only until persistence (M8) saves/loads them.

## Ownership
Owns `App::locations` and the save/jump logic. Reads/writes `cursor`, `viewport.origin`, `anchor_x`. Hands its slot array to persistence (M8) for save/load.

## Status
Implemented (M7) with unit tests. Persistence of slots lands in M8.
