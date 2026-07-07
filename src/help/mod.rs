//! Help — the keybinding cheat-sheet overlay, toggled by Ctrl+H.
//!
//! A read-only centered panel listing every keybinding. Toggled on Ctrl+H and
//! dismissed by any key (handled in `main::handle_key`). Pure construction so it
//! can be unit-tested without a terminal. See ./CLAUDE.md.

/// The keybinding lines shown in the panel. `("keys", "description")`; an empty
/// pair renders as a blank spacer row.
const BINDINGS: &[(&str, &str)] = &[
    ("Arrows", "move cursor"),
    ("Shift+Arrows", "pan view by 1/3 screen"),
    ("Opt+Left/Right", "jump word back / forward"),
    ("Ctrl+A / Ctrl+E", "line start / end, hop parts"),
    ("Click / drag", "position cursor / select rect"),
    ("Scroll wheel", "pan view up / down"),
    ("", ""),
    ("Ctrl+C / Ctrl+V", "copy / paste block"),
    ("Del / Bksp", "clear selection or a cell"),
    ("Ctrl+D / Ctrl+K", "delete char / rest of line"),
    ("Esc", "cancel selection"),
    ("[Calc] + Enter", "calc: eval `1+2=` / assign"),
    ("Ctrl+I", "toggle Insert / Overwrite"),
    ("Ctrl+1..9 (+Shift)", "jump / save bookmark"),
    ("Ctrl+Z", "overview / minimap"),
    ("Ctrl+S", "save to file"),
    ("Ctrl+H", "this help"),
    ("Esc / Ctrl+Q", "quit"),
];

const TITLE: &str = "terminal_pad — keys";
const FOOTER: &str = "press any key to close";

/// Build the help overlay as one `String` per screen row (each `width` chars,
/// exactly `height` rows), with a centered box-drawing panel over blanks. Pure.
pub fn rows(width: u16, height: u16) -> Vec<String> {
    let w = width as usize;
    let h = height as usize;

    // Body lines: title, blank, each binding, blank, footer.
    let key_col = BINDINGS.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let mut body: Vec<String> = Vec::new();
    body.push(center_in(TITLE, content_width(key_col)));
    body.push(String::new());
    for &(k, d) in BINDINGS {
        if k.is_empty() && d.is_empty() {
            body.push(String::new());
        } else {
            body.push(format!("{k:>key_col$}  {d}"));
        }
    }
    body.push(String::new());
    body.push(center_in(FOOTER, content_width(key_col)));

    // Panel = body wrapped in a 1-char border, padded one space each side.
    let inner = content_width(key_col);
    let panel_w = inner + 4; // border + one-space padding each side
    let panel_h = body.len() + 2;

    // Too small to draw a framed panel — fall back to a blank screen.
    if w < panel_w || h < panel_h {
        return vec![" ".repeat(w); h];
    }

    let pad_left = (w - panel_w) / 2;
    let pad_top = (h - panel_h) / 2;
    let border: String = "─".repeat(panel_w - 2);

    let mut out: Vec<String> = Vec::with_capacity(h);
    for _ in 0..pad_top {
        out.push(" ".repeat(w));
    }
    out.push(framed(&format!("┌{border}┐"), pad_left, w));
    for line in &body {
        let mid = format!("│ {line:<inner$} │");
        out.push(framed(&mid, pad_left, w));
    }
    out.push(framed(&format!("└{border}┘"), pad_left, w));
    while out.len() < h {
        out.push(" ".repeat(w));
    }
    out
}

/// Inner content width (key column + two-space gap + a description allowance).
fn content_width(key_col: usize) -> usize {
    let widest = BINDINGS
        .iter()
        .map(|(_, d)| d.len())
        .max()
        .unwrap_or(0)
        .max(TITLE.len())
        .max(FOOTER.len());
    (key_col + 2 + widest).max(TITLE.len()).max(FOOTER.len())
}

/// Center `s` within `width` columns (left-padded, right-filled to `width`).
fn center_in(s: &str, width: usize) -> String {
    let total = width.saturating_sub(s.len());
    let left = total / 2;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(total - left))
}

/// Left-pad a panel line by `pad_left` and right-fill the row to `width`.
fn framed(line: &str, pad_left: usize, width: usize) -> String {
    let mut row = String::with_capacity(width);
    row.push_str(&" ".repeat(pad_left));
    row.push_str(line);
    let drawn = pad_left + line.chars().count();
    if drawn < width {
        row.push_str(&" ".repeat(width - drawn));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_dimensions() {
        let rows = rows(80, 24);
        assert_eq!(rows.len(), 24);
        assert!(rows.iter().all(|r| r.chars().count() == 80));
    }

    #[test]
    fn draws_a_framed_panel() {
        let rows = rows(80, 24);
        let joined = rows.join("\n");
        assert!(joined.contains('┌') && joined.contains('┐'));
        assert!(joined.contains('└') && joined.contains('┘'));
        // A representative binding is present.
        assert!(joined.contains("Ctrl+Z"));
        assert!(joined.contains("this help"));
    }

    #[test]
    fn too_small_falls_back_to_blank() {
        let rows = rows(10, 3);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.chars().count() == 10));
        assert!(rows.iter().all(|r| r.trim().is_empty()));
    }
}
