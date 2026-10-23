# Known limitations

## File safety
- **Residual race:** the pre-save fingerprint recheck is not a compare-and-swap. A
  third-party write landing between the final read and the atomic rename is
  overwritten (the backup holds the bytes ConfigDeck read). No cross-process locking
  exists for arbitrary editors. Details: `src-tauri/src/core/persistence/mod.rs`.
- **Windows replace** uses `MoveFileExW` with replace-existing; it fails (source intact)
  if another process holds the file open without delete sharing. Windows and macOS are
  untested; hard-link counts report 1 on Windows.
- **Directory fsync** failure after a committed rename is ignored.
- **Conflicts:** only non-overlapping edits can be reapplied onto a changed file
  (eng R16). Overlapping changes need Discard or Save a copy; no field-level merge.

## Editing
- **Undo** replays the op list from the original bytes: ~36 ms/MiB parse plus replay
  (≈230 ms for a 4 MiB file in release). Toggles stay in microseconds.
- **Single-line arrays** may get small whitespace quirks inside the edited array
  (`[ 2, 3]`), coming from jsonc-parser; multi-line structures format cleanly.
- **Big integers** keep their exact text in the file and UI, but schema validation sees
  them as f64.
- `dirty_count` counts ops; setting a value back to its original still counts until undone.

## Schema and presets
- `patternProperties` is ignored for hints; `oneOf`/`anyOf` hints merge all branches.
- User schemas must be self-contained: external `$ref` (including relative files) is
  rejected; the file stays editable without the schema.
- The bundled Claude Code schema (SchemaStore, fetched 2026-10-23) lags new CLI
  releases; violations are warnings only.
- Claude Code 2.1.211+ may read `.claude/settings.local.json` from the git repository
  root; ConfigDeck edits the file in the folder you pick and shows a hint (eng R1).

## Platform
- Linux (Manjaro, GNOME) is the primary target. AppImage built on Manjaro needs a
  recent glibc; use the CI build for portability.
