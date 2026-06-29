//! System clipboard — a thin, text-only wrapper over `arboard`, isolated here so
//! the dependency and its failure modes live in one place. Callers treat it as
//! best-effort: a missing display / unavailable clipboard returns `Err`, never
//! panics, and the editor keeps working with its own internal clip buffer.

/// Copy `text` to the OS clipboard. Best-effort; returns a human-readable error
/// on failure (e.g. no display server).
///
/// Caveat: on X11 the selection is owned by the live `Clipboard` handle and is
/// lost when it drops — fine on macOS/Windows (the primary targets), where the
/// content is handed to the system and persists after this returns.
pub fn set_system(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}
