# calc — the [Calc] tag calculator

Enter-triggered inline calculator (M18). Triggered when working on `[Calc]` lines, expression evaluation, calc variables, or the Enter dispatch.

## Behavior (all on Enter, routed via `calc::enter` in `main::handle_key`)
The *line* is the layout segment under the cursor (`layout::line_bounds`), so a
`[Calc]` past a ≥2-blank-column gap (a side column) is **not** seen — the tag
must lie left of the cursor within the cursor's own segment. The tag match is
**case-insensitive** (`[calc]`, `[CALC]`, …); the rightmost occurrence wins.
The *body* is the trimmed text between the tag and the cursor. Three actions:

1. **Eval** — body ends with `=`: evaluate what precedes it and insert the
   result right after the `=` (insert-shift, regardless of edit mode). The
   Enter is consumed — no newline. `[Calc] 1+2*3=` → `[Calc] 1+2*3=7`.
2. **Assign** — body is a *fresh* assignment `name = expr` (bare identifier
   lhs, rhs non-empty with no further `=`): evaluate the rhs, store the
   variable in the session context, append ` = result` to the line, then do a
   normal Enter with the tag prefix on the new line.
   `[Calc] x = 5+3` → `[Calc] x = 5+3 = 8` + a new `[Calc] ` line.
3. **Chain** — anything else (already-evaluated line like `x = 5+3 = 8` or
   `1+2=3`, a `==` comparison, plain text, empty body): normal Enter
   (`editing::newline`, M15 split/push-down) + the tag prefix typed at
   the new line's start — so Enter-Enter keeps a calc column going.

The chained prefix reuses the tag **exactly as typed on the source line** (case
preserved — `[CALC]` chains `[CALC] `), followed by one space.

No tag on the line (or a blank row / gap) → plain `editing::newline`.

## Evaluation
- `evalexpr` 11.x with a per-session `HashMapContext` (`CalcState` on `App`,
  **not persisted** — reopening a pad starts empty; the canvas text documents
  values, and re-Entering an assignment line recomputes). Gives `+ - * / % ^`,
  comparisons, booleans, strings, and builtins (`min`, `max`, `floor`,
  `math::sqrt`, `math::sin`, …).
- **All numbers are floats**: bare integer literals are rewritten `5` → `5.0`
  before evaluation (`floatify`), so division never truncates — `5/2=` → `2.5`,
  not evalexpr's integer `2`. Digit runs inside identifiers (`x2`) and float
  literals are left alone.
- **Hex/binary conversion**: `0xFF` / `0b1010` literals are decoded to decimal
  before evaluation (`decode_radix`, runs ahead of `floatify` — together
  `prepare`); malformed literals (`0x`, `0b12`) are left for evalexpr to
  reject. Encoding goes through custom context functions `hex(v)` / `bin(v)`
  (registered in `CalcState::default`, uppercase hex digits, sign in front)
  returning strings — `hex(1000)=` → `0x3E8`, `bin(0xFF)=` → `0b11111111`.
  Fractional or >2^53 values are an error. String results display unquoted.
- Floats are formatted to 10 decimals with trailing zeros trimmed
  (`0.1+0.2=` → `0.3`, `5+3=` → `8`); huge/non-finite fall back to plain
  display.
- **Errors never touch the canvas**: a failed eval/assign puts `calc: <err>` in
  the status line and consumes the Enter. A successful assign also confirms in
  the status line.

## API
- `CalcState` (on `App::calc`) — owns the `evalexpr` variable context.
- `enter(app)` — the Enter hook; falls back to `editing::newline`.
- Private, pure, unit-tested: `tag_end` (char-offset past the rightmost tag),
  `classify` → `Action::{Eval, Assign, Chain}`, `split_assignment`,
  `format_value`.

## Invariants
- Only Enter reaches this module; every non-calc path degrades to the exact
  pre-M18 `editing::newline` behavior.
- Result/prefix insertion always insert-shifts (never overwrites), whatever the
  Insert/Overwrite mode.
- `classify` is pure; canvas mutation happens only in `enter`'s action arms.

## Decisions
- Session-only variables; append `= result` on assignment; errors to the status
  line; case-insensitive tag (user decisions, 2026-07-07).
- `evalexpr` over a hand-rolled parser — the user wanted functions beyond
  arithmetic (sqrt/sin) without reinventing them.

## Status
Implemented (M18) with unit tests (tag matching, classification, eval-in-place,
float formatting, assign+chain, chain-after-eval, error handling, no-tag and
side-segment fallthrough).
