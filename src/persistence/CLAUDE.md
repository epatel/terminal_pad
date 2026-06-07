# persistence — load & save

Loading and saving the canvas, bookmarks, and cursor as JSON. Triggered when working on the file format, save/load, the `.tpad` file, or the CLI path arg.

## File path
A single file given as the first CLI argument, defaulting to `./canvas.tpad` (resolved in `main`). The path is stored on `App::path`.

## What is persisted
- The canvas cells (flat list of `{x, y, c}`, sorted by `(y, x)`).
- The nine bookmark slots (`Option<Location>` each — cursor + viewport origin).
- The cursor position.

## Format
serde + serde_json, a versioned `Doc`:
```json
{
  "version": 1,
  "cells": [ { "x": 0, "y": 0, "c": "H" } ],
  "slots": [ null, { "cursor": [40,12], "origin": [38,10] }, null, ... ],
  "cursor": [0, 0]
}
```
`version` lets the format evolve. `char` serializes as a one-character JSON string.

## When
- **Load** — at startup in `main`, before the TUI. Missing file → fresh canvas (`Ok(false)`); a malformed/unreadable file **aborts the program** with a message rather than entering the app, so the auto-save-on-exit can't clobber it.
- **Save** — `Ctrl+S` (explicit, reports result in the status line) and automatically on clean exit (after the terminal is restored).
- On load, the viewport origin is anchored on the loaded cursor (no viewport size is known yet) so the cursor is visible on the first frame.

## Invariants
- A missing file at startup is normal, not an error.
- Saves are **atomic**: write a sibling `*.tpad.tmp`, then rename over the target.
- Erased cells are absent from `cells` (truly blank), keeping files small.

## Failure modes
- Malformed/older file → `load` returns an error; `main` prints it and exits non-zero (never silently discards the file).
- JSON is fine for v1; a binary/RLE format is a later option if size/latency hurts.

## Ownership
Owns the on-disk format and load/save. Reads the canvas (`cells()`), the bookmark slots, and the cursor; reconstructs them on load.

## Status
Implemented (M8) with unit tests, including a real temp-file save→load round-trip.
