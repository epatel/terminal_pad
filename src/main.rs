//! terminal_pad — infinite-canvas TUI text pad.
//!
//! Terminal lifecycle (raw mode, alt screen, bracketed paste, panic-safe
//! restore) + the blocking event loop and key dispatch. Feature logic lives in
//! the per-feature modules, each with a co-located CLAUDE.md:
//!   - canvas    (M2)      — the sparse infinite grid
//!   - viewport  (M3, M4)  — visible window, scrolling, 1/3 jump
//!   - render    (M3)      — paint the frame
//!   - editing   (M5, M6)  — typing, Ctrl+I mode toggle, paste
//!   - locations (M7)      — Ctrl+1..9 jump / Ctrl+Shift+1..9 save
//!   - persistence (M8)    — see cards/feature-persistence.md (not built yet)

mod app;
mod canvas;
mod clipboard;
mod editing;
mod help;
mod locations;
mod overview;
mod persistence;
mod render;
mod selection;
mod viewport;

use std::fs;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};

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

    let path = match &parsed.name {
        Some(name) => match data_dir().and_then(|base| pad_path_in(&base, name)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("terminal_pad: {e}");
                std::process::exit(2);
            }
        },
        None => parsed
            .positional
            .clone()
            .unwrap_or_else(|| PathBuf::from("canvas.tpad")),
    };

    // The central pads dir (and any nested path) may not exist yet; the atomic
    // save writes a sibling temp file, so the directory must be present first.
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("terminal_pad: cannot create {}: {e}", parent.display());
        std::process::exit(1);
    }

    let mut app = App::new();
    app.path = path.clone();

    if parsed.clear {
        // Start from an empty canvas; the cleared state is persisted by the
        // save-on-exit (or Ctrl+S). Existing contents — even a malformed file —
        // are simply not loaded.
        app.status = format!("cleared {}", path.display());
    } else {
        // Load before entering the TUI; a malformed file aborts so we never
        // clobber it on the save-at-exit below.
        match persistence::load(&path, &mut app) {
            Ok(true) => app.status = format!("loaded {}", path.display()),
            Ok(false) => app.status = format!("new file {}", path.display()),
            Err(e) => {
                eprintln!("terminal_pad: cannot read {}: {e}", path.display());
                std::process::exit(1);
            }
        }
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

/// Parsed command line. `name` selects a central pad, `positional` is a literal
/// pad path; they are mutually exclusive.
struct Parsed {
    name: Option<String>,
    positional: Option<PathBuf>,
    clear: bool,
    help: bool,
}

/// Parse the argument list (everything after the program name). Hand-rolled —
/// the surface is tiny and a clap dependency isn't worth it.
fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut name = None;
    let mut positional: Option<PathBuf> = None;
    let mut clear = false;
    let mut help = false;

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--name" => {
                i += 1;
                let v = args.get(i).ok_or("--name requires a value")?;
                if name.is_some() {
                    return Err("--name given more than once".into());
                }
                name = Some(v.clone());
            }
            "--clear" => clear = true,
            "-h" | "--help" => help = true,
            // Any other dash-prefixed token (but not a lone "-") is an unknown flag.
            _ if a.starts_with('-') && a != "-" => {
                return Err(format!("unknown option: {a}"));
            }
            _ => {
                if positional.is_some() {
                    return Err("more than one path argument given".into());
                }
                positional = Some(PathBuf::from(a));
            }
        }
        i += 1;
    }

    if name.is_some() && positional.is_some() {
        return Err("--name and a path argument cannot be combined".into());
    }
    Ok(Parsed {
        name,
        positional,
        clear,
        help,
    })
}

/// Resolve a `--name` value to `<base>/terminal_pad/<name>.tpad`, rejecting names
/// that aren't a single bare path component (so a pad can't escape the dir).
fn pad_path_in(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() {
        return Err("--name must not be empty".into());
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(format!("--name must be a bare name, not a path: {name:?}"));
    }
    Ok(base.join("terminal_pad").join(format!("{name}.tpad")))
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

/// Enter raw mode + alternate screen, enable bracketed paste, and (when the
/// terminal supports it) request key disambiguation so Shift+arrow and the
/// F-keys report modifiers reliably — see cards/decision-language-rust.md.
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
    loop {
        terminal.draw(|frame| render::draw(frame, app))?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // Esc clears an active selection before it falls through to quit.
                if key.code == KeyCode::Esc && app.selection.is_some() {
                    app.selection = None;
                } else if should_quit(&key) {
                    break;
                } else {
                    handle_key(app, &key);
                }
            }
            Event::Mouse(m) => handle_mouse(app, &m),
            Event::Paste(text) => editing::paste(app, &text),
            Event::Resize(w, h) => {
                // Re-sync the viewport size and keep the cursor on screen.
                app.viewport.width = w;
                app.viewport.height = h.saturating_sub(render::STATUS_HEIGHT);
                app.viewport.scroll_to_show(app.cursor);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Rows the scroll wheel pans per notch.
const WHEEL_STEP: i64 = 3;

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
            app.begin_drag(sx, sy)
        }
        MouseEventKind::Drag(MouseButton::Left) => app.update_drag(sx, sy),
        MouseEventKind::Up(MouseButton::Left) => app.end_drag(),
        MouseEventKind::ScrollDown => app.scroll_rows(WHEEL_STEP),
        MouseEventKind::ScrollUp => app.scroll_rows(-WHEEL_STEP),
        _ => {}
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

/// Route a key press. Ctrl+H toggles the help overlay (any key dismisses it);
/// Ctrl+Z toggles the overview; while zoomed out only save and toggle are
/// accepted (it's a read-only view). Otherwise: navigation (M4, plus Alt+Left/
/// Right word jump), editing (M5/M6), and bookmarks (M7).
fn handle_key(app: &mut App, key: &KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // Help overlay: Ctrl+H toggles it; while it's showing any key dismisses it
    // (it's a read-only cheat sheet). Checked before everything else.
    if app.help {
        app.help = false;
        return;
    }
    if ctrl && key.code == KeyCode::Char('h') {
        app.help = true;
        return;
    }

    // Ctrl+Z toggles overview in either mode.
    if ctrl && key.code == KeyCode::Char('z') {
        app.toggle_zoom();
        return;
    }
    // In the overview, plain arrows pan the view (and cursor) by a screenful for
    // quick navigation; Shift+arrows pan by 1/3 screen, the same gesture as in the
    // normal editor. Saving is allowed; editing is not.
    if app.zoom == ZoomMode::Overview {
        match key.code {
            KeyCode::Left if shift => app.jump_view(-1, 0),
            KeyCode::Right if shift => app.jump_view(1, 0),
            KeyCode::Up if shift => app.jump_view(0, -1),
            KeyCode::Down if shift => app.jump_view(0, 1),
            KeyCode::Left => app.pan_view(-1, 0),
            KeyCode::Right => app.pan_view(1, 0),
            KeyCode::Up => app.pan_view(0, -1),
            KeyCode::Down => app.pan_view(0, 1),
            KeyCode::Char('s') if ctrl => save_now(app),
            _ => {}
        }
        return;
    }

    // Selection actions. Ctrl+C copies (and keeps the selection); Ctrl+V pastes
    // the internal block; Delete/Backspace on a selection clears the block. Any
    // other key dismisses a lingering selection so the highlight doesn't stick.
    if ctrl && key.code == KeyCode::Char('c') {
        if let Some(text) = app.copy_selection()
            && let Err(e) = clipboard::set_system(&text)
        {
            app.status = format!("{} (clipboard: {e})", app.status);
        }
        return;
    }
    if ctrl && key.code == KeyCode::Char('v') {
        app.paste_clip();
        return;
    }
    if app.selection.is_some() && matches!(key.code, KeyCode::Delete | KeyCode::Backspace) {
        app.delete_selection();
        return;
    }
    app.selection = None;

    match key.code {
        KeyCode::Left => {
            if alt {
                editing::word_left(app)
            } else if shift {
                app.jump_view(-1, 0)
            } else {
                app.move_cursor(-1, 0)
            }
        }
        KeyCode::Right => {
            if alt {
                editing::word_right(app)
            } else if shift {
                app.jump_view(1, 0)
            } else {
                app.move_cursor(1, 0)
            }
        }
        KeyCode::Up => {
            if shift {
                app.jump_view(0, -1)
            } else {
                app.move_cursor(0, -1)
            }
        }
        KeyCode::Down => {
            if shift {
                app.jump_view(0, 1)
            } else {
                app.move_cursor(0, 1)
            }
        }
        // Insert/Overwrite toggle. Ctrl+I needs a terminal that distinguishes it
        // from Tab (Kitty keyboard protocol — see notes).
        KeyCode::Char('i') if ctrl => editing::toggle_mode(app),
        // Ctrl+S — save now.
        KeyCode::Char('s') if ctrl => save_now(app),
        KeyCode::Backspace => editing::backspace(app),
        KeyCode::Delete => editing::delete(app),
        KeyCode::Enter => editing::newline(app),
        // Locations: Ctrl+1..9 jump, Ctrl+Shift+1..9 save. Needs a terminal that
        // reports Ctrl+digit modifiers (Kitty keyboard protocol — see notes).
        KeyCode::Char(c) if ctrl && locations::slot_for_digit(c).is_some() => {
            let slot = locations::slot_for_digit(c).unwrap();
            if shift {
                locations::save(app, slot)
            } else {
                locations::jump(app, slot)
            }
        }
        // Printable input — ignore when Ctrl/Alt is held (those are commands).
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            editing::type_char(app, c)
        }
        _ => {}
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
