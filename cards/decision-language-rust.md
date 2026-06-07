# decision-language-rust

Why terminal_pad is built in Rust with ratatui + crossterm + serde, and what would make us reconsider.

## Choice
Rust, with `ratatui` (rendering/layout), `crossterm` (terminal backend + input events), and `serde` + `serde_json` (persistence).

## Deciding constraints
- **Modifier-key detection.** The spec needs Shift+arrow and Shift+F-key distinguished from plain arrow/F-key. `crossterm` exposes `KeyEvent { code, modifiers }` and supports the Kitty keyboard protocol for reliable modifier reporting. Raw `ncurses` frequently cannot tell Shift+arrow apart without terminfo hacks.
- **Bracketed paste.** "Paste text into the canvas" is a core feature. `crossterm` delivers paste as a single `Event::Paste(String)` once bracketed paste is enabled — avoiding the keystroke-flood problem you get with naive terminal input in C++/Node.
- **Distribution.** `cargo build --release` yields one static binary, no runtime to install.
- **Memory model.** A sparse infinite canvas (a map of coordinates → char) is clean and fast in Rust without GC pauses.

## Alternatives considered
- **C++ / ncurses** — viable but more boilerplate, manual memory, weaker modifier-key story (terminfo quirks), manual bracketed-paste handling.
- **Node.js (`ink`/`blessed`)** — fastest prototype, but `blessed` is stale, raw 2D canvas + precise F-key/Shift handling is awkward, and it ships a Node runtime dependency.

## Revisit signals
- User explicitly prefers C++ or Node.
- A required capability exists only in a non-Rust ecosystem.
- The build/distribution constraints change (e.g. must embed in an existing Node/C++ app).

## Status
Locked (recorded 2026-06-07).
