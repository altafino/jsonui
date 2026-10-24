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

## YAML
- Files with several YAML documents (`---`) are read-only, shown as a masked source view.
- Values behind an alias, nodes defining a referenced anchor, tagged values and complex
  keys (`? [a, b]`) can't be edited here; the message points to a text editor. Merge
  keys (`<<`) show as a normal member.
- Typing follows YAML 1.2 core: `0x1F` and `.inf` are numbers, `yes`/`on` are strings.
  New strings that YAML 1.1 readers would take as booleans or dates are quoted.
- String-for-string edits keep plain/single/double style; a block scalar replaced by text
  that can't stay a literal block becomes a double-quoted string.
- Replacing or removing a block collection drops comments inside it; comment lines
  directly above a removed entry stay.
- Every edit re-parses and validates the whole file, so very large YAML files edit more
  slowly than JSON.
- The masked view of a broken file is a line scanner: after an unclosed flow collection
  or quote, later lines are masked wholesale.

## TOML
- Files with dotted keys interleaved among other keys, or with mixed line endings, are
  read-only: `toml_edit` cannot print them back byte for byte.
- TOML has no null: setting or adding a value containing null is refused.
- Integers outside 64 bits are refused. Dates and times show as strings and stay dates
  only when the new text parses as one.
- Removing a key or table also removes the comment lines directly above it.
- New tables get a blank line before them; moves inside inline arrays move values only,
  while array-of-tables moves carry comments and sub-tables along.
- Multi-line string styles are kept only while the new text fits them.

## UI
- No date/time pickers and no variant selector for heterogeneous (`oneOf`) arrays;
  those fields fall back to text or raw JSON value editing.
- Numeric `minimum`/`maximum` and integer-ness are enforced while typing, so an
  out-of-range number never reaches the save-time warnings.
- The "was <old value>" hint costs one debounced `preview_draft` call after edits.
- No preference for the watcher's behavior: a file changed on disk reloads silently
  when there is no draft and raises the conflict flow when there is one.

## Platform
- Linux (Manjaro, GNOME) is the primary target. AppImage built on Manjaro needs a
  recent glibc; use the CI build for portability.
