//! Calc — the `[Calc]` tag calculator (M18).
//!
//! Enter on a line containing a `[Calc]` tag (case-insensitive) left of the
//! cursor treats the text between the tag and the cursor as calculator input:
//! a trailing `=` evaluates the expression in place, `name = expr` assigns a
//! session variable (result appended), and anything else chains — a normal
//! Enter whose new line starts with a fresh `[Calc] ` prefix. The "line" is
//! the layout segment under the cursor, so a tag past a ≥2-blank gap (a side
//! column) is not seen. See ./CLAUDE.md.

use evalexpr::{
    ContextWithMutableFunctions, ContextWithMutableVariables, EvalexprError, Function,
    HashMapContext, Value,
};

use crate::app::App;

/// The tag's char length (`[calc]`).
const TAG_LEN: usize = 6;

/// Session-only calculator state: the variables assigned on `[Calc]` lines.
/// Not persisted — reopening a pad starts with an empty table (the canvas text
/// documents the values; re-Enter an assignment line to recompute).
pub struct CalcState {
    ctx: HashMapContext,
}

impl Default for CalcState {
    fn default() -> Self {
        let mut ctx = HashMapContext::new();
        // Conversion helpers: hex(v) / bin(v) render a whole number as an
        // 0x… / 0b… string. Registration on a fresh context cannot fail.
        let _ = ctx.set_function(
            "hex".into(),
            Function::new(|arg| radix_str(arg, 16).map(Value::String)),
        );
        let _ = ctx.set_function(
            "bin".into(),
            Function::new(|arg| radix_str(arg, 2).map(Value::String)),
        );
        Self { ctx }
    }
}

/// Render a whole numeric value in `radix` (16 → `0x3E8`, 2 → `0b1010`),
/// uppercase hex digits, sign in front. Errors on fractional values and on
/// floats too large to hold an exact integer (> 2^53).
fn radix_str(v: &Value, radix: u32) -> Result<String, EvalexprError> {
    let n = match v {
        Value::Int(i) => *i as i128,
        Value::Float(f) if f.fract() == 0.0 && f.abs() <= 2f64.powi(53) => *f as i128,
        _ => {
            return Err(EvalexprError::CustomMessage(
                "hex()/bin() need a whole number".into(),
            ));
        }
    };
    let (sign, mag) = if n < 0 { ("-", -n) } else { ("", n) };
    Ok(match radix {
        16 => format!("{sign}0x{mag:X}"),
        _ => format!("{sign}0b{mag:b}"),
    })
}

/// What Enter should do with the text between the `[Calc]` tag and the cursor.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    /// Trailing `=`: evaluate the expression before it, insert the result.
    Eval(String),
    /// `name = expr`: assign the variable, append ` = result`, then chain.
    Assign(String, String),
    /// Anything else (already-evaluated line, plain text, empty): normal
    /// Enter + `[Calc] ` prefix on the new line.
    Chain,
}

/// The Enter hook: dispatch to the calculator when a `[Calc]` tag lies left of
/// the cursor on its line. Returns `false` (canvas untouched) when there is no
/// tag, so the caller can fall through to the next Enter handler.
pub fn enter(app: &mut App) -> bool {
    let Some((prefix, body)) = tagged_input(app) else {
        return false;
    };
    match classify(body.trim()) {
        Action::Eval(expr) => eval_in_place(app, &expr),
        Action::Assign(name, rhs) => assign(app, name, &rhs, &prefix),
        Action::Chain => chain_newline(app, &prefix),
    }
    true
}

/// When a `[Calc]` tag lies left of the cursor on its line: the chained-line
/// prefix (the tag exactly as typed on this line, case kept, plus a space) and
/// the body between the tag and the cursor.
fn tagged_input(app: &App) -> Option<(String, String)> {
    let (s, _) = app.line_under_cursor()?;
    let (cx, cy) = app.cursor;
    let left: Vec<char> = (s..cx)
        .map(|x| app.canvas.get(x, cy).unwrap_or(' '))
        .collect();
    let t = tag_end(&left)?;
    let prefix = left[t - TAG_LEN..t].iter().chain([&' ']).collect();
    Some((prefix, left[t..].iter().collect()))
}

/// Evaluate `expr` against the session variables.
fn eval(app: &App, expr: &str) -> Result<Value, EvalexprError> {
    evalexpr::eval_with_context(&prepare(expr), &app.calc.ctx)
}

/// `expr=` + Enter: insert the result after the `=`; errors go to the status line.
fn eval_in_place(app: &mut App, expr: &str) {
    if expr.is_empty() {
        app.status = "calc: empty expression".into();
        return;
    }
    match eval(app, expr) {
        Ok(v) => insert_str(app, &format_value(&v)),
        Err(e) => app.status = format!("calc: {e}"),
    }
}

/// `name = rhs` + Enter: store the variable, append ` = result`, confirm in the
/// status line, then chain a new tagged line. On an error the canvas is untouched.
fn assign(app: &mut App, name: String, rhs: &str, prefix: &str) {
    let stored = eval(app, rhs).and_then(|v| {
        let shown = format_value(&v);
        app.calc.ctx.set_value(name.clone(), v).map(|()| shown)
    });
    match stored {
        Ok(shown) => {
            insert_str(app, &format!(" = {shown}"));
            app.status = format!("calc: {name} = {shown}");
            chain_newline(app, prefix);
        }
        Err(e) => app.status = format!("calc: {e}"),
    }
}

/// Char offset just past the rightmost case-insensitive `[calc]` in `text`,
/// or `None` when the tag does not appear.
fn tag_end(text: &[char]) -> Option<usize> {
    const TAG: [char; TAG_LEN] = ['[', 'c', 'a', 'l', 'c', ']'];
    if text.len() < TAG.len() {
        return None;
    }
    (0..=text.len() - TAG.len())
        .rev()
        .find(|&i| (0..TAG.len()).all(|j| text[i + j].to_ascii_lowercase() == TAG[j]))
        .map(|i| i + TAG.len())
}

/// Classify the (trimmed) text between the tag and the cursor.
fn classify(body: &str) -> Action {
    if let Some(expr) = body.strip_suffix('=') {
        return Action::Eval(expr.trim().to_string());
    }
    match split_assignment(body) {
        Some((name, rhs)) => Action::Assign(name, rhs),
        None => Action::Chain,
    }
}

/// Split `name = expr` into its parts. `None` when the body is not a *fresh*
/// assignment: the left side must be a bare identifier, and the right side
/// must be non-empty and hold no further `=` — so an already-evaluated
/// `x = 5+3 = 8` (and a `==` comparison) falls through to `Chain`.
fn split_assignment(body: &str) -> Option<(String, String)> {
    let (lhs, rhs) = body.split_once('=')?;
    let name = lhs.trim();
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let rhs = rhs.trim();
    if rhs.is_empty() || rhs.contains('=') {
        return None;
    }
    Some((name.to_string(), rhs.to_string()))
}

/// Prepare an expression for `evalexpr`: decode `0x…`/`0b…` literals to
/// decimal, then rewrite bare integer literals as floats.
fn prepare(expr: &str) -> String {
    floatify(&decode_radix(expr))
}

/// Decode hexadecimal (`0xFF`) and binary (`0b1010`) integer literals to
/// decimal — `evalexpr` has no radix literals of its own. `0X`/`0B` prefixes
/// work too; the sign stays outside the literal (`-0xFF` is unary minus).
/// A malformed or overflowing literal (`0x`, `0xFG`, `0b12`) is left as-is
/// for `evalexpr` to reject, as is a digit run inside an identifier (`a0xF`).
fn decode_radix(expr: &str) -> String {
    Scanner::new(expr).rewrite(Scanner::radix_literal)
}

/// An expression scanned char by char, rewriting the literals a matcher
/// recognises and copying everything else through.
struct Scanner {
    chars: Vec<char>,
}

impl Scanner {
    fn new(expr: &str) -> Self {
        Self {
            chars: expr.chars().collect(),
        }
    }

    /// Copy the expression, replacing each literal `matcher` recognises — it
    /// returns the replacement text and the index just past the literal.
    fn rewrite(&self, matcher: impl Fn(&Self, usize) -> Option<(String, usize)>) -> String {
        let mut out = String::with_capacity(self.chars.len());
        let mut i = 0;
        while i < self.chars.len() {
            match matcher(self, i) {
                Some((text, end)) => {
                    out.push_str(&text);
                    i = end;
                }
                None => {
                    out.push(self.chars[i]);
                    i += 1;
                }
            }
        }
        out
    }

    /// A well-formed `0x…` / `0b…` literal at `i`, as decimal. `None` when no
    /// literal starts there: mid-identifier, no digits, glued to a following
    /// identifier char, or overflowing.
    fn radix_literal(&self, i: usize) -> Option<(String, usize)> {
        if !self.starts_token(i) || self.chars.get(i) != Some(&'0') {
            return None;
        }
        let radix = match self.chars.get(i + 1)? {
            'x' | 'X' => 16,
            'b' | 'B' => 2,
            _ => return None,
        };
        let (digits, end) = self.digit_run(i + 2, radix);
        if digits.is_empty() || !self.ends_token(end) {
            return None;
        }
        let value = i128::from_str_radix(&digits, radix).ok()?;
        Some((value.to_string(), end))
    }

    /// A bare integer literal at `i`, as a float (`5` → `5.0`). A digit run
    /// glued to an identifier or a `.` is copied through unchanged.
    fn int_literal(&self, i: usize) -> Option<(String, usize)> {
        if !self.chars[i].is_ascii_digit() || !self.starts_token(i) {
            return None;
        }
        let (mut digits, end) = self.digit_run(i, 10);
        if self.ends_token(end) {
            digits.push_str(".0");
        }
        Some((digits, end))
    }

    /// Whether a token can start at `i`: nothing before it joins onto it.
    fn starts_token(&self, i: usize) -> bool {
        i == 0 || !joins(self.chars[i - 1])
    }

    /// Whether a token ending just before `end` is cleanly delimited.
    fn ends_token(&self, end: usize) -> bool {
        self.chars.get(end).is_none_or(|&c| !joins(c))
    }

    /// The run of `radix` digits starting at `start`, and the index past it.
    fn digit_run(&self, start: usize, radix: u32) -> (String, usize) {
        let digits: String = self.chars[start..]
            .iter()
            .take_while(|c| c.is_digit(radix))
            .collect();
        let end = start + digits.len();
        (digits, end)
    }
}

/// Whether `c` glues onto an adjacent digit run (identifier chars and `.`).
fn joins(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// Rewrite bare integer literals as floats (`5` → `5.0`) so all arithmetic is
/// float arithmetic — without this, `evalexpr` gives `5/2 = 2` (integer
/// division), which reads wrong on a calculator. A digit run is left alone
/// when it is part of an identifier (`x2`), a float (`1.5`, `.5`), or an
/// exponent-style token — i.e. when the char before or after it is
/// alphanumeric, `_`, or `.`. `format_value` trims the `.0` back off whole
/// results, so `5+3=` still shows `8`.
fn floatify(expr: &str) -> String {
    Scanner::new(expr).rewrite(Scanner::int_literal)
}

/// Render an evaluation result for the canvas. Floats are rounded to 10
/// decimals with trailing zeros trimmed, so `0.1+0.2` reads `0.3`, not
/// `0.30000000000000004`; huge or non-finite floats fall back to their plain
/// display. Everything else (ints, booleans, strings, tuples) uses `Display`.
fn format_value(v: &Value) -> String {
    match v {
        Value::Float(f) if f.is_finite() && f.abs() < 1e15 => {
            let s = format!("{f:.10}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() || s == "-" {
                "0".into()
            } else {
                s.to_string()
            }
        }
        // Bare string, no quotes — hex()/bin() results land as typed text.
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Insert `s` at the cursor (shifting the rest of its line right, like
/// insert-mode typing regardless of the current edit mode) and advance past it.
fn insert_str(app: &mut App, s: &str) {
    crate::editing::insert_str(app, s);
}

/// A normal Enter, then the tag prefix (as typed on the source line, case
/// preserved, plus a space) at the new line's start so the next line keeps
/// calculating.
fn chain_newline(app: &mut App, prefix: &str) {
    crate::editing::newline(app);
    insert_str(app, prefix);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Coord;

    fn row(app: &App, y: Coord) -> String {
        app.canvas
            .cells()
            .into_iter()
            .filter(|&(_, cy, _)| cy == y)
            .map(|(_, _, c)| c)
            .collect()
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            crate::editing::type_char(app, c);
        }
    }

    #[test]
    fn tag_end_is_case_insensitive_and_rightmost() {
        let chars = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(tag_end(&chars("[Calc] 1+2")), Some(6));
        assert_eq!(tag_end(&chars("[CALC] x")), Some(6));
        assert_eq!(tag_end(&chars("[calc] [cAlC] y")), Some(13));
        assert_eq!(tag_end(&chars("no tag here")), None);
        assert_eq!(tag_end(&chars("[cal")), None);
    }

    #[test]
    fn classify_eval_assign_chain() {
        assert_eq!(classify("1+2="), Action::Eval("1+2".into()));
        assert_eq!(
            classify("x = 5+3"),
            Action::Assign("x".into(), "5+3".into())
        );
        assert_eq!(classify("x = 5+3 = 8"), Action::Chain); // already evaluated
        assert_eq!(classify("1+2=3"), Action::Chain); // already evaluated
        assert_eq!(classify("x == 3"), Action::Chain); // comparison, not assignment
        assert_eq!(classify("2*x == 3"), Action::Chain); // lhs not an identifier
        assert_eq!(classify("just notes"), Action::Chain);
        assert_eq!(classify(""), Action::Chain);
    }

    #[test]
    fn enter_evaluates_trailing_equals_in_place() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 1+2*3=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] 1+2*3=7");
        assert_eq!(app.cursor, (14, 0)); // after the result, no newline
    }

    #[test]
    fn floatify_targets_only_bare_integer_literals() {
        assert_eq!(floatify("5/2"), "5.0/2.0");
        assert_eq!(floatify("10+300"), "10.0+300.0");
        assert_eq!(floatify("1.5/3"), "1.5/3.0"); // float literal untouched
        assert_eq!(floatify("x2 + 5"), "x2 + 5.0"); // identifier untouched
        assert_eq!(floatify("math::sqrt(2)"), "math::sqrt(2.0)");
        assert_eq!(floatify(""), "");
    }

    #[test]
    fn decode_radix_handles_hex_bin_and_malformed() {
        assert_eq!(decode_radix("0xFF + 1"), "255 + 1");
        assert_eq!(decode_radix("0b1010 * 2"), "10 * 2");
        assert_eq!(decode_radix("0XfF"), "255"); // any prefix/digit case
        assert_eq!(decode_radix("-0xFF"), "-255"); // sign outside the literal
        assert_eq!(decode_radix("a0xF"), "a0xF"); // inside an identifier
        assert_eq!(decode_radix("0x"), "0x"); // malformed: no digits
        assert_eq!(decode_radix("0b12"), "0b12"); // malformed: bad digit
        assert_eq!(decode_radix("0xFG"), "0xFG"); // malformed: bad digit
    }

    #[test]
    fn hex_and_bin_literals_evaluate() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 0xFF + 1=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] 0xFF + 1=256");
    }

    #[test]
    fn hex_and_bin_functions_encode_uppercase() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] hex(1000)=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] hex(1000)=0x3E8");
        enter(&mut app); // chain to the next line
        type_str(&mut app, "bin(0xFF)=");
        enter(&mut app);
        assert_eq!(row(&app, 1), "[Calc] bin(0xFF)=0b11111111");
    }

    #[test]
    fn hex_of_a_variable_and_negative_values() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] x = 0b1010 * 2");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] x = 0b1010 * 2 = 20");
        type_str(&mut app, "hex(x)=");
        enter(&mut app);
        assert_eq!(row(&app, 1), "[Calc] hex(x)=0x14");
        enter(&mut app);
        type_str(&mut app, "hex(0 - 256)=");
        enter(&mut app);
        assert_eq!(row(&app, 2), "[Calc] hex(0 - 256)=-0x100");
    }

    #[test]
    fn hex_of_a_fraction_is_an_error() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] hex(1.5)=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] hex(1.5)="); // untouched
        assert!(app.status.starts_with("calc:"));
    }

    #[test]
    fn division_of_two_ints_is_float() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 5/2=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] 5/2=2.5");
    }

    #[test]
    fn division_through_a_variable_is_float() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] x = 5");
        enter(&mut app); // stores x (as a float) and chains
        type_str(&mut app, "x/2=");
        enter(&mut app);
        assert_eq!(row(&app, 1), "[Calc] x/2=2.5");
    }

    #[test]
    fn enter_formats_floats_cleanly() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 0.1+0.2=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] 0.1+0.2=0.3");
    }

    #[test]
    fn assignment_appends_result_and_chains_with_prefix() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] x = 5+3");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] x = 5+3 = 8");
        assert_eq!(row(&app, 1), "[Calc] "); // trailing space is a stored cell
        assert_eq!(app.cursor, (7, 1)); // after the prefix on the new line
        // The variable is usable on the chained line.
        type_str(&mut app, "x*2=");
        enter(&mut app);
        assert_eq!(row(&app, 1), "[Calc] x*2=16");
    }

    #[test]
    fn second_enter_after_evaluation_chains_with_prefix() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 1+2=");
        enter(&mut app); // evaluates in place
        enter(&mut app); // chains
        assert_eq!(row(&app, 0), "[Calc] 1+2=3");
        assert_eq!(row(&app, 1), "[Calc] ");
        assert_eq!(app.cursor, (7, 1));
    }

    #[test]
    fn chained_prefix_keeps_the_source_tag_case() {
        let mut app = App::new();
        type_str(&mut app, "[CALC] x = 2");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[CALC] x = 2 = 2");
        assert_eq!(row(&app, 1), "[CALC] "); // same case as the line above
        type_str(&mut app, "note");
        enter(&mut app); // chain again from a plain-text body
        assert_eq!(row(&app, 2), "[CALC] ");
    }

    #[test]
    fn eval_error_reports_status_and_leaves_canvas_untouched() {
        let mut app = App::new();
        type_str(&mut app, "[Calc] 1+*2=");
        enter(&mut app);
        assert_eq!(row(&app, 0), "[Calc] 1+*2="); // unchanged
        assert_eq!(app.cursor, (12, 0)); // Enter consumed, no newline
        assert!(app.status.starts_with("calc:"));
    }

    #[test]
    fn no_tag_falls_through_to_normal_newline() {
        let mut app = App::new();
        type_str(&mut app, "hello");
        assert!(!enter(&mut app)); // not handled — caller does the newline
        assert_eq!(app.cursor, (5, 0)); // canvas and cursor untouched
        assert_eq!(row(&app, 1), "");
    }

    #[test]
    fn tag_in_side_segment_past_gap_is_not_seen() {
        let mut app = App::new();
        type_str(&mut app, "[Calc]"); // cols 0..=5
        // ≥2-blank gap, then a separate segment the cursor sits in.
        app.cursor = (10, 0);
        app.anchor_x = 10;
        type_str(&mut app, "1+2=");
        assert!(!enter(&mut app)); // tag not seen — not handled
        assert_eq!(row(&app, 0), "[Calc]1+2="); // row text collapses the gap
        assert_eq!(app.cursor, (14, 0)); // untouched
    }
}
