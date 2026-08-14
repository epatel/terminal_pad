# find — search the canvas for text

Live canvas search opened by **Ctrl+F** (M26). Triggered when working on find,
the search prompt, jump-to-match navigation, or `#tag` lookup.

## What it matches
A **case-insensitive literal substring, within a row**. A match never wraps onto
the next row. Unwritten cells read as blanks, so a query containing a space
matches whether that gap is a typed space or an empty cell. Overlapping matches
each count (`aa` twice in `aaa`). An empty query matches nothing — there is
nothing to jump to yet.

There is deliberately **no separate tag feature**: a `#name` tag is just text on
the canvas, so typing `#api` finds it like any other string, and a bare `#`
walks every tag. That replaced an earlier tag-only search bound to Ctrl+#, which
was a bad binding — on most layouts `#` *is* Shift+3, so it collided with
`Ctrl+Shift+3` = save bookmark slot 3 (`src/locations/`).

## Behavior (keys routed in `main::handle_key`)
**Ctrl+F** opens a search: the status line becomes a prompt and `App::search`
holds a `Search`. The view does **not** move on open — jumping before the user
has said where would be startling. While the search runs it owns the keyboard:

| Key | Action |
|-----|--------|
| a printable char | append to the query, re-search, jump to the **first** match |
| **Ctrl+F** | advance to the next match, wrapping |
| **Backspace** | drop a char and re-search; on an *empty* query, close the search |
| **Enter** | accept — close, leaving the cursor where the search landed |
| **Esc** | cancel — close and restore the cursor **and** viewport origin |
| anything else | accept, and the key is swallowed |

A query with no match leaves the cursor put and reports `no match`; the query is
kept so Backspace can walk back to something that matches.

Esc is intercepted in the **event loop** (`main::run`), before `should_quit` —
otherwise cancelling a search would quit the app. Same guard the selection uses.

Ctrl+F is a plain ASCII control code (0x06), so unlike the `Ctrl+digit`
bindings it works in **any** terminal — no Kitty keyboard protocol needed.

## API (`find::*`)
- `Search` — on `App::search`, session-only. `query()` and `position()`
  (1-based `(i, n)`, `None` when nothing matched) are what `render` shows.
- `matches(canvas, query) -> Vec<(Coord, Coord)>` — every hit's first-char
  position, in canvas order (top-to-bottom, left-to-right). Pure. Walks
  `canvas.cells()` once, grouping by row (it is sorted by `(y, x)`), rendering
  each row's span as a string so a char offset maps back to a column — offsets
  are counted in **chars**, not bytes, so a multi-byte row still maps correctly.
- `open` / `advance` / `push` / `backspace` / `accept` / `cancel` — the key
  handlers, acting on `&mut App`.
- Private: `research`, `jump_to_current`.

## Jumping
`jump_to_current` sets the cursor to the match and brings it on screen:
`viewport.scroll_to_show` when it is **already visible** (so narrowing the query
doesn't shuffle the view under you), `viewport.focus_on` when it isn't — which
parks the target a **quarter** in from the top-left rather than centering it,
since the text you want to read after landing runs below and to the right.
`anchor_x` is reset like other navigation.

## Rendering
`render::draw` has a branch for `app.search`: the canvas is still painted (the
jump has to be visible while typing) but the status line becomes
` FIND  query_   i/n   · … `, and the text cursor is placed as usual.

## Invariants
- `matches` is pure; only the key handlers mutate `App`.
- A search never modifies the canvas — it is navigation only.
- Nothing here is persisted; a search does not outlive the keystrokes driving it.

## Status
Implemented (M26) with unit tests: substring matching and canvas ordering,
`#` and `#api` finding tags, case-insensitivity, empty query, a query spanning
a typed space *and* an unwritten cell, no wrap across rows, overlapping matches,
multi-byte rows, open-does-not-move, typing narrows and jumps, Ctrl+F cycles and
wraps, no-match holds position, accept keeps the spot, Esc restores cursor+view,
Backspace-out, quarter-in focus for an off-screen hit.
