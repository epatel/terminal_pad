//! terminal_pad — infinite-canvas TUI text pad.
//!
//! Terminal lifecycle (raw mode, alt screen, bracketed paste, mouse capture,
//! panic-safe restore) + the blocking event loop, CLI parsing, and key/mouse
//! dispatch. Feature logic lives in the per-feature modules, each with a
//! co-located CLAUDE.md (except the single-file `clipboard`):
//!   - canvas      (M2)      — the sparse infinite grid
//!   - viewport    (M3, M4)  — visible window, scrolling, 1/3 jump, scroll-wheel pan
//!   - render      (M3)      — paint the frame (incl. selection highlight)
//!   - editing     (M5, M6)  — typing, Ctrl+I mode toggle, paste, Alt+word jump
//!   - locations   (M7)      — Ctrl+1..9 jump / Ctrl+Shift+1..9 save
//!   - persistence (M8)      — JSON load/save of cells + slots + cursor
//!   - overview    (M9)      — Ctrl+Z zoomed-out minimap
//!   - help        (M10)     — Ctrl+H keybinding cheat sheet
//!   - selection   (M13)     — click-drag rectangle + copy/paste/clear
//!   - clipboard   (M13)     — system-clipboard wrapper (arboard)
//!   - calc        (M18)     — [Calc] tag calculator on Enter
//!   - todo        (M20)     — [ ] checkbox list chaining on Enter
//!   - banner      (M21)     — Ctrl+B figlet-style big-letter typing
//!   - find        (M26)     — Ctrl+F live canvas search (finds #tags too)

mod app;
mod banner;
mod calc;
mod canvas;
mod clipboard;
mod editing;
mod find;
mod help;
mod layout;
mod locations;
mod overview;
mod persistence;
mod render;
mod selection;
mod todo;
mod viewport;
mod width;

use std::fs;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        MouseButton, MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use overview::ZoomMode;

type Tui = Terminal<CrosstermBackend<Stdout>>;

const USAGE: &str = "\
terminal_pad — infinite-canvas text pad

Usage:
  terminal_pad [PATH]
  terminal_pad --name <name> [--clear]

Options:
  --name <name>   Open the named pad under the data dir
                  ($XDG_DATA_HOME or ~/.local/share)/terminal_pad/<name>.tpad,
                  reachable from any directory.
  --clear         Start the pad empty; the cleared state is written on
                  exit / Ctrl+S.
  -h, --help      Show this help.
  -V, --version   Show the version.

Without --name, PATH is the pad file (default ./canvas.tpad).";

fn main() -> io::Result<()> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match parse(&raw) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("terminal_pad: {e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if parsed.help {
        println!("{USAGE}");
        return Ok(());
    }
    if parsed.version {
        println!("terminal_pad {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let path = resolve_path(&parsed).unwrap_or_else(|e| die(2, &e));
    if let Err(e) = ensure_parent_dir(&path) {
        die(1, &e);
    }

    let mut app = App::new();
    app.path = path;
    if let Err(e) = load_or_clear(&mut app, parsed.clear) {
        die(1, &e);
    }

    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, &mut app);
    restore_terminal()?;

    // Save on clean exit (after the terminal is restored so errors are visible).
    if let Err(e) = persistence::save(&app.path, &app) {
        eprintln!("terminal_pad: cannot save {}: {e}", app.path.display());
    }
    result
}

/// Print `msg` the way every startup error is reported and exit with `code`.
fn die(code: i32, msg: &str) -> ! {
    eprintln!("terminal_pad: {msg}");
    std::process::exit(code);
}

/// The pad file the command line names: the central pad for `--name`, else the
/// positional path, else `./canvas.tpad`.
fn resolve_path(parsed: &Parsed) -> Result<PathBuf, String> {
    match &parsed.name {
        Some(name) => data_dir().and_then(|base| pad_path_in(&base, name)),
        None => Ok(parsed
            .positional
            .clone()
            .unwrap_or_else(|| PathBuf::from("canvas.tpad"))),
    }
}

/// The central pads dir (and any nested path) may not exist yet; the atomic
/// save writes a sibling temp file, so the directory must be present first.
fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display())),
        _ => Ok(()),
    }
}

/// Fill `app` from its pad file, or start empty for `--clear`, and say which in
/// the status line.
///
/// `--clear` skips the load entirely — even a malformed file — and the cleared
/// state is persisted by the save-on-exit (or Ctrl+S). Otherwise the load runs
/// before the TUI starts, and a malformed file is an error so the caller aborts
/// and the save-at-exit never clobbers it.
fn load_or_clear(app: &mut App, clear: bool) -> Result<(), String> {
    let shown = app.path.display().to_string();
    if clear {
        app.status = format!("cleared {shown}");
        return Ok(());
    }
    let path = app.path.clone();
    app.status = match persistence::load(&path, app) {
        Ok(true) => format!("loaded {shown}"),
        Ok(false) => format!("new file {shown}"),
        Err(e) => return Err(format!("cannot read {shown}: {e}")),
    };
    Ok(())
}

/// Parsed command line. `name` selects a central pad, `positional` is a literal
/// pad path; they are mutually exclusive.
#[derive(Default)]
struct Parsed {
    name: Option<String>,
    positional: Option<PathBuf>,
    clear: bool,
    help: bool,
    version: bool,
}

impl Parsed {
    /// Record one argument. `value` yields the next argument, for the flags
    /// that take one.
    fn apply<'a>(
        &mut self,
        arg: &str,
        mut value: impl FnMut() -> Option<&'a String>,
    ) -> Result<(), String> {
        match arg {
            "--name" => {
                let v = value().ok_or("--name requires a value")?;
                if self.name.replace(v.clone()).is_some() {
                    return Err("--name given more than once".into());
                }
            }
            "--clear" => self.clear = true,
            "-h" | "--help" => self.help = true,
            "-V" | "--version" => self.version = true,
            // Any other dash-prefixed token (but not a lone "-") is an unknown flag.
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("unknown option: {arg}"));
            }
            _ => {
                if self.positional.replace(PathBuf::from(arg)).is_some() {
                    return Err("more than one path argument given".into());
                }
            }
        }
        Ok(())
    }
}

/// Parse the argument list (everything after the program name). Hand-rolled —
/// the surface is tiny and a clap dependency isn't worth it.
fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        parsed.apply(arg, || it.next())?;
    }
    if parsed.name.is_some() && parsed.positional.is_some() {
        return Err("--name and a path argument cannot be combined".into());
    }
    Ok(parsed)
}

/// Resolve a `--name` value to `<base>/terminal_pad/<name>.tpad`, rejecting names
/// that aren't a single bare path component (so a pad can't escape the dir).
fn pad_path_in(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() {
        return Err("--name must not be empty".into());
    }
    if !is_bare_name(name) {
        return Err(format!("--name must be a bare name, not a path: {name:?}"));
    }
    Ok(base.join("terminal_pad").join(format!("{name}.tpad")))
}

/// A single path component: no separators, and not `.` or `..`.
fn is_bare_name(name: &str) -> bool {
    !matches!(name, "." | "..") && !name.contains(['/', '\\'])
}

/// The user data directory: `$XDG_DATA_HOME` if set, else `~/.local/share`.
fn data_dir() -> Result<PathBuf, String> {
    if let Some(x) = std::env::var_os("XDG_DATA_HOME")
        && !x.is_empty()
    {
        return Ok(PathBuf::from(x));
    }
    match std::env::var_os("HOME") {
        Some(h) if !h.is_empty() => Ok(PathBuf::from(h).join(".local").join("share")),
        _ => Err("cannot locate a data directory (set HOME or XDG_DATA_HOME)".into()),
    }
}

/// Enter raw mode + alternate screen, enable bracketed paste and mouse capture,
/// and (when the terminal supports it) request key disambiguation so Shift+arrow,
/// Ctrl+I, and Ctrl/Alt+key report modifiers reliably — see
/// cards/decision-language-rust.md.
fn setup_terminal() -> io::Result<Tui> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;
    if supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    Terminal::new(CrosstermBackend::new(stdout))
}

/// Undo everything `setup_terminal` did. Safe to call more than once (errors are
/// swallowed) so it can run from both normal exit and the panic hook.
fn restore_terminal() -> io::Result<()> {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    let _ = execute!(
        stdout,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    );
    let _ = disable_raw_mode();
    Ok(())
}

/// Restore the terminal before the default panic handler prints, so a crash
/// never leaves the user's terminal in raw mode / the alternate screen.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        original(info);
    }));
}

/// Blocking event loop. M0 only draws a placeholder and quits on Esc / Ctrl-Q;
/// later milestones route events into the feature pipeline (see architecture).
fn run(terminal: &mut Tui, app: &mut App) -> io::Result<()> {
    // The view origin restored from disk was saved without a terminal size; sync
    // the size now and clamp so the cursor is on screen even if the terminal is
    // smaller than at save time. A same-size reopen is a no-op (origin preserved).
    let size = terminal.size()?;
    sync_size(app, size.width, size.height);

    loop {
        terminal.draw(|frame| render::draw(frame, app))?;
        if handle_event(app, event::read()?) == Flow::Quit {
            return Ok(());
        }
    }
}

/// Whether the event loop keeps going after an event.
#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Quit,
}

/// Size the viewport to a `width` x `height` terminal (status line excluded)
/// and keep the cursor on screen.
fn sync_size(app: &mut App, width: u16, height: u16) {
    app.viewport.width = width;
    app.viewport.height = height.saturating_sub(render::STATUS_HEIGHT);
    app.viewport.scroll_to_show(app.cursor);
}

/// Route one terminal event.
fn handle_event(app: &mut App, ev: Event) -> Flow {
    match ev {
        Event::Key(key) if key.kind == KeyEventKind::Press => return handle_press(app, &key),
        Event::Mouse(m) => handle_mouse(app, &m),
        Event::Paste(text) => editing::paste(app, &text),
        Event::Resize(w, h) => sync_size(app, w, h),
        _ => {}
    }
    Flow::Continue
}

/// A key press. Esc first cancels a live find, then clears a selection, and
/// only then reaches quit.
fn handle_press(app: &mut App, key: &KeyEvent) -> Flow {
    let esc = key.code == KeyCode::Esc;
    if esc && app.search.is_some() {
        find::cancel(app);
    } else if esc && app.selection.is_some() {
        app.selection = None;
    } else if should_quit(key) {
        return Flow::Quit;
    } else {
        handle_key(app, key);
    }
    Flow::Continue
}

/// Rows the scroll wheel pans per notch.
const WHEEL_STEP: i64 = 1;
/// Columns a horizontal scroll notch pans. Larger than `WHEEL_STEP` because a
/// terminal cell is ~twice as tall as it is wide, so equal cell counts feel
/// like much less horizontal travel.
const HWHEEL_STEP: i64 = 8;

/// Route a mouse event. Active only in the normal editor (the help and overview
/// overlays are read-only). Left-click-drag paints a rectangular selection (a
/// bare click positions the cursor); the scroll wheel pans the view vertically.
fn handle_mouse(app: &mut App, m: &MouseEvent) {
    if app.help || app.zoom == ZoomMode::Overview {
        return;
    }
    let (w, h) = (app.viewport.width, app.viewport.height);
    if w == 0 || h == 0 {
        return;
    }
    // Clamp to the canvas drawing area: a drag may run past the edges or onto the
    // status row; the press itself must start inside the canvas.
    let sx = m.column.min(w - 1);
    let sy = m.row.min(h - 1);
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) if m.row < h && m.column < w => {
            left_press(app, sx, sy)
        }
        MouseEventKind::Drag(MouseButton::Left) => app.update_drag(sx, sy),
        MouseEventKind::Up(MouseButton::Left) => app.end_drag(),
        MouseEventKind::ScrollDown => app.scroll_rows(WHEEL_STEP),
        MouseEventKind::ScrollUp => app.scroll_rows(-WHEEL_STEP),
        MouseEventKind::ScrollRight => app.scroll_cols(-HWHEEL_STEP),
        MouseEventKind::ScrollLeft => app.scroll_cols(HWHEEL_STEP),
        _ => {}
    }
}

/// A left press on screen cell `(sx, sy)`. Double-click selects the word under
/// the pointer, triple-click its whole line — selection only; Ctrl+C copies,
/// like any other block. A single click starts a drag.
fn left_press(app: &mut App, sx: u16, sy: u16) {
    let cell = app.viewport.canvas_at(sx, sy);
    let clicks = app.register_click(Instant::now(), cell);
    if !(2..=3).contains(&clicks) {
        app.begin_drag(sx, sy);
        return;
    }
    app.click_to(sx, sy);
    app.selection = None;
    if clicks == 2 {
        app.select_word();
    } else {
        app.select_line();
    }
}

/// Best-effort write of copied text to the system clipboard; a failure is
/// appended to whatever status the copy already set (the internal clip still works).
fn copy_to_system(app: &mut App, text: &str) {
    if let Err(e) = clipboard::set_system(text) {
        app.status = format!("{} (clipboard: {e})", app.status);
    }
}

fn should_quit(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
        || (key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Save to the app's path and report the result in the status line.
fn save_now(app: &mut App) {
    app.status = match persistence::save(&app.path, app) {
        Ok(()) => format!("saved {}", app.path.display()),
        Err(e) => format!("save failed: {e}"),
    };
}

/// The modifier keys held for a key press.
#[derive(Clone, Copy)]
struct Mods {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

impl Mods {
    fn of(key: &KeyEvent) -> Self {
        Self {
            ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
            shift: key.modifiers.contains(KeyModifiers::SHIFT),
            alt: key.modifiers.contains(KeyModifiers::ALT),
        }
    }
}

/// Route a key press. Each layer either consumes the key or passes it on, in
/// priority order: the help overlay, a live find, the overview (Ctrl+Z toggles
/// it), selection actions, and finally the editor — navigation (M4, plus
/// Alt+Left/Right word jump), editing (M5/M6), and bookmarks (M7).
fn handle_key(app: &mut App, key: &KeyEvent) {
    let m = Mods::of(key);
    let consumed = help_key(app, key.code, m)
        || find_key(app, key.code, m)
        || overview_key(app, key.code, m)
        || selection_key(app, key.code, m);
    if !consumed {
        editor_key(app, key.code, m);
    }
}

/// Help overlay: Ctrl+H opens it; while it's showing any key dismisses it (it's
/// a read-only cheat sheet). Checked before everything else.
fn help_key(app: &mut App, code: KeyCode, m: Mods) -> bool {
    if app.help {
        app.help = false;
        return true;
    }
    if m.ctrl && code == KeyCode::Char('h') {
        app.help = true;
        return true;
    }
    false
}

/// A live find (Ctrl+F) owns the keyboard while it runs: typing narrows it,
/// Ctrl+F steps to the next hit, Backspace edits it (and walks out of the
/// search on an empty query), Enter keeps the landing spot. Esc cancels,
/// handled in the event loop before it can reach quit. Any other key accepts
/// the search and is swallowed.
fn find_key(app: &mut App, code: KeyCode, m: Mods) -> bool {
    if app.search.is_none() {
        return false;
    }
    match code {
        KeyCode::Enter => find::accept(app),
        KeyCode::Backspace => find::backspace(app),
        KeyCode::Char('f') if m.ctrl => find::advance(app),
        KeyCode::Char(c) if !m.ctrl && !m.alt => find::push(app, c),
        _ => find::accept(app),
    }
    true
}

/// Ctrl+Z toggles the overview in either mode. In the overview, plain arrows pan
/// the view (and cursor) by a screenful for quick navigation; Shift+arrows pan by
/// 1/3 screen, the same gesture as in the normal editor. Saving is allowed;
/// editing is not, so every other key is swallowed.
fn overview_key(app: &mut App, code: KeyCode, m: Mods) -> bool {
    if m.ctrl && code == KeyCode::Char('z') {
        app.toggle_zoom();
        return true;
    }
    if app.zoom != ZoomMode::Overview {
        return false;
    }
    if let Some((dx, dy)) = arrow_delta(code) {
        if m.shift {
            app.jump_view(dx, dy);
        } else {
            app.pan_view(dx, dy);
        }
    } else if m.ctrl && code == KeyCode::Char('s') {
        save_now(app);
    }
    true
}

/// The unit direction of an arrow key, or `None` for any other key.
fn arrow_delta(code: KeyCode) -> Option<(i64, i64)> {
    match code {
        KeyCode::Left => Some((-1, 0)),
        KeyCode::Right => Some((1, 0)),
        KeyCode::Up => Some((0, -1)),
        KeyCode::Down => Some((0, 1)),
        _ => None,
    }
}

/// Selection actions. Ctrl+C copies (and keeps the selection); Ctrl+V pastes the
/// internal block; Delete/Backspace on a selection clears the block. Any other
/// key dismisses a lingering selection so the highlight doesn't stick, then
/// passes on to the editor.
fn selection_key(app: &mut App, code: KeyCode, m: Mods) -> bool {
    match code {
        KeyCode::Char('c') if m.ctrl => {
            if let Some(text) = app.copy_selection() {
                copy_to_system(app, &text);
            }
        }
        KeyCode::Char('v') if m.ctrl => app.paste_clip(),
        KeyCode::Delete | KeyCode::Backspace if app.selection.is_some() => app.delete_selection(),
        _ => {
            app.selection = None;
            return false;
        }
    }
    true
}

/// The normal editor: arrows, Ctrl commands, deletion, Enter and typing.
fn editor_key(app: &mut App, code: KeyCode, m: Mods) {
    if let Some((dx, dy)) = arrow_delta(code) {
        return arrow_key(app, dx, dy, m);
    }
    match code {
        KeyCode::Char(c) if m.ctrl => ctrl_key(app, c, m.shift),
        KeyCode::Backspace => editing::backspace(app),
        KeyCode::Delete => editing::delete(app),
        KeyCode::Enter => enter_key(app),
        // Printable input — ignore when Alt is held (those are commands).
        // Banner mode stamps a figlet glyph instead of a single cell.
        KeyCode::Char(c) if !m.alt => {
            if app.banner {
                banner::type_char(app, c)
            } else {
                editing::type_char(app, c)
            }
        }
        _ => {}
    }
}

/// An arrow key in the editor: Alt+Left/Right jump by a word, Shift jumps the
/// view by 1/3 screen, a plain arrow moves the cursor one cell.
fn arrow_key(app: &mut App, dx: i64, dy: i64, m: Mods) {
    if m.alt && dy == 0 {
        if dx < 0 {
            editing::word_left(app)
        } else {
            editing::word_right(app)
        }
    } else if m.shift {
        app.jump_view(dx, dy)
    } else {
        app.move_cursor(dx, dy)
    }
}

/// Ctrl+letter / Ctrl+digit commands in the editor. Most are plain ASCII
/// control codes that work in any terminal; Ctrl+I and the Ctrl+digit bindings
/// need one that reports them distinctly (Kitty keyboard protocol — see notes).
fn ctrl_key(app: &mut App, c: char, shift: bool) {
    match c {
        // Start / end of the line under the cursor.
        'a' => editing::line_start(app),
        'e' => editing::line_end(app),
        // Delete char (pull the line left) / delete to the line's end.
        'd' => editing::delete_pull(app),
        'k' => editing::kill_to_end(app),
        // Insert/Overwrite toggle (Ctrl+I is Tab without the Kitty protocol).
        'i' => editing::toggle_mode(app),
        // Banner (figlet big-letter) typing.
        'b' => banner::toggle(app),
        's' => save_now(app),
        // Open a live find. A `#name` tag is just text, so `#api` finds one.
        'f' => find::open(app),
        // Locations: Ctrl+1..9 jump, Ctrl+Shift+1..9 save.
        _ => match locations::slot_for_digit(c) {
            Some(slot) if shift => locations::save(app, slot),
            Some(slot) => locations::jump(app, slot),
            None => {}
        },
    }
}

/// Enter. In banner mode it just drops a glyph height (a split would shred the
/// art); otherwise it goes through the calculator first (a [Calc] tag
/// evaluates / assigns / chains, M18), then checkbox chaining (a leading
/// [ ] / [x] prefixes the new line with `[ ] `, M20), else a normal newline.
fn enter_key(app: &mut App) {
    if app.banner {
        banner::newline(app)
    } else if !calc::enter(app) && !todo::enter(app) {
        editing::newline(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_name_and_clear() {
        let p = parse(&args(&["--name", "notes", "--clear"])).unwrap();
        assert_eq!(p.name.as_deref(), Some("notes"));
        assert!(p.clear);
        assert!(p.positional.is_none());
    }

    #[test]
    fn parse_positional_path_default_back_compat() {
        let p = parse(&args(&["/tmp/foo.tpad"])).unwrap();
        assert_eq!(p.positional, Some(PathBuf::from("/tmp/foo.tpad")));
        assert!(p.name.is_none() && !p.clear);
    }

    #[test]
    fn parse_help_flag() {
        assert!(parse(&args(&["--help"])).unwrap().help);
        assert!(parse(&args(&["-h"])).unwrap().help);
    }

    #[test]
    fn parse_version_flag() {
        assert!(parse(&args(&["--version"])).unwrap().version);
        assert!(parse(&args(&["-V"])).unwrap().version);
    }

    #[test]
    fn parse_rejects_name_combined_with_path() {
        assert!(parse(&args(&["--name", "a", "b.tpad"])).is_err());
    }

    #[test]
    fn parse_rejects_unknown_flag_and_bad_arity() {
        assert!(parse(&args(&["--wat"])).is_err());
        assert!(parse(&args(&["--name"])).is_err());
        assert!(parse(&args(&["a.tpad", "b.tpad"])).is_err());
    }

    #[test]
    fn pad_path_joins_under_data_dir() {
        assert_eq!(
            pad_path_in(Path::new("/data"), "notes").unwrap(),
            PathBuf::from("/data/terminal_pad/notes.tpad")
        );
    }

    #[test]
    fn pad_path_rejects_non_bare_names() {
        assert!(pad_path_in(Path::new("/data"), "").is_err());
        assert!(pad_path_in(Path::new("/data"), "a/b").is_err());
        assert!(pad_path_in(Path::new("/data"), "..").is_err());
    }
}
