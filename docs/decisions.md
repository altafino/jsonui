# Decisions

Full records (question, options, evidence, answer) are in
`docs/spec/ConfigDeck_AI_Agent_Build_Spec.md`, sections "Decision ledger",
"Design review decisions" and "Eng re-check ledger". This page is the index.

## Scope
- **D2 Polling instead of `notify`:** 1 s stat poll of the open document, hash on change, other files checked on open/focus; pre-save fingerprint recheck unchanged. `notify` is P1.
- **D3 User schema association is P0** (A10).
- **D4 Font scaling + comfortable/compact density are P0.**
- **D5 Single-crate layout** per spec §15 (`src-tauri/src/core/*`).

## Engineering (R1-R18, C1-C4)
- R1 Claude local file stays `<dir>/.claude/settings.local.json` with a repo-root hint.
- R2 Preset schemas warn ("Save with N warnings"); user schemas block.
- R3 jsonc-parser CST is the only parse; numbers cross IPC as exact lexemes.
- R4 Rust owns the draft; undo/redo replay ops from original bytes.
- R5 Symlinks: confirm once, write through to the target; nlink>1 confirm; copy mode bits.
- R6 No raw paths from the webview; pickers run Rust-side.
- R7 Format-neutral contracts; jsonc-parser confined to `core/document` (+ redact); no Format trait yet.
- R8 Local AppImage/deb on Manjaro; portable build on ubuntu-22.04 CI. AUR recipes: source package `configdeck` and release package `configdeck-bin` (`docs/packaging.md`).
- R9 Rust is the only validator (no Ajv).
- R10 UTF-8 BOM stripped on read, restored on save.
- R11 Playwright (mocked IPC) + WebdriverIO (real app) + Vitest + Rust integration tests.
- R12 Registry written atomically with `.bak`; corrupt registry set aside and recovered.
- R13 In-place CST edits; changed-node responses; lazy `get_subtree`.
- R14 Schema validation debounced in the background; synchronous check at Save.
- R15 Temp files `.<basename>.configdeck-<random>.tmp`; leftovers offered for removal.
- R16 Reapply overlap = two-way prefix or shared parent array; disk must parse.
- R17 Invalid-file source view masks all string values.
- R18 One shared `pointers_overlap` helper for Revert and Reapply.
- C1 `rename-key` op. C2 sample deletion guard. C3 rebase after reapply. C4 disk-change diff for conflicts.

## Design (DD1-DD17)
Row anatomy (DD1), footer-only Save (DD2), depth ≤ 3 then drill-in (DD3), invalid source view (DD4), empty states (DD5), save-bar feedback (DD6), sample file (DD7), IBM Plex (DD8), token set (DD9, DD10, DD13), status semantics (DD11), width rules (DD12), keyboard/ARIA (DD14), Review dialog (DD15), Conflict dialog with reapply (DD16), array item summaries (DD17).

## Implementation notes
- Windows atomic replace uses `std::fs::rename` (MoveFileExW with replace) — see `core/persistence` docs.
- Real-app tests script pickers via `CONFIGDECK_TEST_PICKS` (file of paths, one per call).
