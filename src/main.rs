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
mod editing;
mod locations;
mod overview;
mod persistence;
mod render;
mod viewport;

use std::io::{self, Stdout};
use std::path::PathBuf;

use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
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

fn main() -> io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("canvas.tpad"));

    let mut app = App::new();
    app.path = path.clone();

    // Load before entering the TUI; a malformed file aborts so we never clobber
    // it on the save-at-exit below.
    match persistence::load(&path, &mut app) {
        Ok(true) => app.status = format!("loaded {}", path.display()),
        Ok(false) => app.status = format!("new file {}", path.display()),
        Err(e) => {
            eprintln!("terminal_pad: cannot read {}: {e}", path.display());
            std::process::exit(1);
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

/// Enter raw mode + alternate screen, enable bracketed paste, and (when the
/// terminal supports it) request key disambiguation so Shift+arrow and the
/// F-keys report modifiers reliably — see cards/decision-language-rust.md.
fn setup_terminal() -> io::Result<Tui> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
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
    let _ = execute!(stdout, DisableBracketedPaste, LeaveAlternateScreen);
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
                if should_quit(&key) {
                    break;
                }
                handle_key(app, &key);
            }
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

/// Route a key press. Ctrl+Z toggles the overview; while zoomed out only save
/// and toggle are accepted (it's a read-only view). Otherwise: navigation (M4),
/// editing (M5/M6), and bookmarks (M7).
fn handle_key(app: &mut App, key: &KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // Ctrl+Z toggles overview in either mode.
    if ctrl && key.code == KeyCode::Char('z') {
        app.toggle_zoom();
        return;
    }
    // In the overview, arrows pan the view (and cursor) by a screenful for quick
    // navigation; saving is allowed; editing is not.
    if app.zoom == ZoomMode::Overview {
        match key.code {
            KeyCode::Left => app.pan_view(-1, 0),
            KeyCode::Right => app.pan_view(1, 0),
            KeyCode::Up => app.pan_view(0, -1),
            KeyCode::Down => app.pan_view(0, 1),
            KeyCode::Char('s') if ctrl => save_now(app),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Left => {
            if shift {
                app.jump_view(-1, 0)
            } else {
                app.move_cursor(-1, 0)
            }
        }
        KeyCode::Right => {
            if shift {
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
