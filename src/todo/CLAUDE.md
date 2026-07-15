# todo — checkbox list chaining

Enter-triggered checklist continuation (M20). Triggered when working on `[ ]` /
`[x]` checkbox lines or the Enter dispatch.

## Behavior (on Enter, routed via `main::handle_key`)
The *line* is the layout segment under the cursor (`layout::line_bounds`), so a
checkbox past a ≥2-blank-column gap (a side column) is **not** seen. When the
segment **starts** with a checkbox — `[` + (blank | `x` | `X`) + `]` followed by
a blank — and the cursor sits at or past the box (`cx >= start + 3`), Enter does
a normal `editing::newline` (M15 split/push-down) and types a fresh unchecked
`[ ] ` at the new line's start. The chained prefix is always unchecked,
whatever the source box held.

- Splitting mid-item moves the tail after the new box: `[ ] buy| milk` →
  `[ ] buy` / `[ ] milk`.
- **Empty item ends the list**: on a bare `[ ] ` the cursor is past the
  segment's `end + 1` (the trailing space is blank), so `line_bounds` reports
  the cursor off the line and Enter falls through to a plain newline — no
  prefix. Same mechanics as `[Calc]`.
- Not a chain (falls through): box not at the segment start (`notes [ ] later`),
  no blank after the `]` (`[x]!alarm`), cursor inside the box, blank row / gap.

## Enter dispatch order (in `main::handle_key`)
`calc::enter` (a `[Calc]` tag anywhere left of the cursor wins) →
`todo::enter` → `editing::newline`. Both hooks return `bool` — `false` means
the canvas is untouched and the next handler runs.

## API
- `enter(app) -> bool` — the whole surface. Prefix insertion insert-shifts
  (never overwrites), whatever the Insert/Overwrite mode, matching calc.

## Status
Implemented (M20) with unit tests (chain from unchecked/checked, mid-item
split, malformed box, cursor-in-box, empty-item list end, side-segment
fallthrough).
