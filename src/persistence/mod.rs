//! Persistence — load/save the canvas, bookmarks, and cursor as JSON (M8).
//!
//! On-disk format is a versioned document (cells as a flat list, the nine slots,
//! and the cursor). Saves are atomic (temp file + rename). See ./CLAUDE.md.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::canvas::{Canvas, Coord};
use crate::locations::{Location, SLOT_COUNT};

const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Doc {
    version: u32,
    cells: Vec<Cell>,
    /// Bookmark slots; expected length `SLOT_COUNT` but tolerated otherwise.
    slots: Vec<Option<Location>>,
    cursor: [Coord; 2],
}

#[derive(Serialize, Deserialize)]
struct Cell {
    x: Coord,
    y: Coord,
    c: char,
}

/// Serialize the current state to a pretty JSON string.
pub fn to_json(app: &App) -> serde_json::Result<String> {
    let doc = Doc {
        version: FORMAT_VERSION,
        cells: app
            .canvas
            .cells()
            .into_iter()
            .map(|(x, y, c)| Cell { x, y, c })
            .collect(),
        slots: app.locations.to_vec(),
        cursor: [app.cursor.0, app.cursor.1],
    };
    serde_json::to_string_pretty(&doc)
}

/// Replace the app's canvas/bookmarks/cursor from a JSON string.
pub fn from_json(s: &str, app: &mut App) -> serde_json::Result<()> {
    let doc: Doc = serde_json::from_str(s)?;

    let mut canvas = Canvas::new();
    for cell in &doc.cells {
        canvas.set(cell.x, cell.y, cell.c);
    }
    app.canvas = canvas;

    let mut slots: [Option<Location>; SLOT_COUNT] = [None; SLOT_COUNT];
    for (dst, src) in slots.iter_mut().zip(doc.slots) {
        *dst = src;
    }
    app.locations = slots;

    app.cursor = (doc.cursor[0], doc.cursor[1]);
    app.anchor_x = app.cursor.0;
    // No viewport size yet at load time; anchor the view on the cursor so it's
    // visible on the first frame.
    app.viewport.origin = app.cursor;
    Ok(())
}

/// Load from `path` into `app`. Returns `Ok(false)` if the file doesn't exist
/// (a fresh canvas is normal), `Ok(true)` if loaded, or an error for an
/// unreadable / malformed file (caller should not overwrite it).
pub fn load(path: &Path, app: &mut App) -> io::Result<bool> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    from_json(&text, app).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(true)
}

/// Save `app` to `path` atomically: write a sibling temp file, then rename over
/// the target so a crash mid-write can't corrupt the existing file.
pub fn save(path: &Path, app: &App) -> io::Result<()> {
    let json = to_json(app).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = temp_path(path);
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("canvas.tpad");
    path.with_file_name(format!("{name}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing;

    #[test]
    fn json_round_trip_restores_state() {
        let mut app = App::new();
        editing::type_char(&mut app, 'h');
        editing::type_char(&mut app, 'i');
        editing::newline(&mut app); // cursor to (0, 1)
        editing::type_char(&mut app, '!');
        app.cursor = (40, 12);
        app.locations[3] = Some(Location {
            cursor: (5, 6),
            origin: (1, 2),
        });

        let json = to_json(&app).unwrap();

        let mut restored = App::new();
        from_json(&json, &mut restored).unwrap();

        assert_eq!(restored.canvas.cells(), app.canvas.cells());
        assert_eq!(restored.cursor, (40, 12));
        assert_eq!(
            restored.locations[3],
            Some(Location {
                cursor: (5, 6),
                origin: (1, 2),
            })
        );
        // View is anchored on the loaded cursor.
        assert_eq!(restored.viewport.origin, (40, 12));
    }

    #[test]
    fn malformed_json_is_an_error() {
        let mut app = App::new();
        assert!(from_json("not json", &mut app).is_err());
    }

    #[test]
    fn temp_path_is_a_sibling() {
        assert_eq!(
            temp_path(Path::new("/tmp/canvas.tpad")),
            PathBuf::from("/tmp/canvas.tpad.tmp")
        );
    }

    #[test]
    fn save_then_load_via_real_file() {
        let mut path = std::env::temp_dir();
        path.push(format!("terminal_pad_test_{}.tpad", std::process::id()));
        let _ = fs::remove_file(&path);

        // Missing file is normal, not an error.
        let mut fresh = App::new();
        assert!(!load(&path, &mut fresh).unwrap());

        let mut app = App::new();
        editing::type_char(&mut app, 'Z');
        app.cursor = (7, 3);
        save(&path, &app).unwrap();

        let mut loaded = App::new();
        assert!(load(&path, &mut loaded).unwrap());
        assert_eq!(loaded.canvas.cells(), app.canvas.cells());
        assert_eq!(loaded.cursor, (7, 3));

        fs::remove_file(&path).unwrap();
    }
}
