# ConfigDeck IPC contract and build lanes

Source of truth for requirements: `docs/spec/ConfigDeck_AI_Agent_Build_Spec.md`
(sections 0-21 plus the appended eng review R1-R18, corrections C1-C4 and
design decisions DD1-DD17; appended decisions win over sections 0-21).

DTOs live in `src-tauri/src/core/types.rs` and `core/error.rs`. TypeScript
mirrors are generated into `src/lib/types/generated/` by `cargo test` (ts-rs).
Never edit generated files; change the Rust type and re-run `cargo test`.

## Rules that apply to every lane

- No command takes a raw filesystem path from the webview (eng R6). Paths come
  from Rust-side pickers or Rust-issued candidate ids.
- `jsonc_parser` is imported only under `src-tauri/src/core/document/` and
  `src-tauri/src/core/security/redact.rs` (eng R7, R17). CI greps for this.
- Core (`src-tauri/src/core`) never depends on `tauri`. Tests run with
  `cargo test --lib` and use temp dirs; never touch real user config files.
- Numbers and values cross IPC as JSON text / exact lexemes (eng R3).
- Never log or put secret values in error messages.

## Commands (Tauri `invoke` names, camelCase args)

Errors reject with `CoreError` (`{ code, message, path, line, column, remediation }`).

| Command | Args | Returns |
|---|---|---|
| `list_managed_files` | – | `ManagedDocument[]` |
| `get_file_statuses` | – | `FileStatus[]` (`editable`: parses as editable; `null` when unreadable or too large) |
| `get_startup_notices` | – | `Notice[]` |
| `pick_and_add_managed_file` | `displayName?: string` | `ManagedDocument \| null` (null = dialog cancelled) |
| `pick_and_create_managed_file` | – | `ManagedDocument \| null` |
| `create_sample_file` | – | `ManagedDocument` |
| `remove_managed_file` | `fileId` | `void` |
| `update_managed_file_metadata` | `fileId, patch: MetadataPatch` | `ManagedDocument` |
| `relocate_managed_file` | `fileId` | `ManagedDocument \| null` |
| `confirm_link_write` | `fileId` | `ManagedDocument` |
| `list_discoverable_claude_files` | – | `DiscoveredCandidate[]` (user + managed) |
| `pick_project_directory` | – | `DiscoveredCandidate[] \| null` (shared + local for the picked dir) |
| `add_discovered_file` | `candidateId` | `ManagedDocument` (creates `{}` for a missing candidate) |
| `create_missing_file` | `fileId` | – (writes `{}` at the registered path; never overwrites) |
| `open_document` | `fileId` | `DocumentSnapshot` (current draft if already open) |
| `reload_document` | `fileId` | `DocumentSnapshot` (discards draft) |
| `get_subtree` | `fileId, pointer, depth` | `TreeNode` |
| `get_field_hints` | `fileId, pointers: string[]` | `FieldHint[]` (`valueType` = schema `type`, `required`; `protected` = required or preset `protectedPointers`: no remove/rename, enforced by `apply_draft_operation` and `remove_draft_items`) |
| `get_preset_info` | `fileId` | `PresetInfo \| null` |
| `apply_draft_operation` | `fileId, op: DraftOp` | `EditResult` |
| `remove_draft_items` | `fileId, pointers: string[]` | `EditResult` (removes all as ONE undo step; drops pointers under another selected pointer; array items highest index first; refuses protected ones) |
| `undo_draft_operation` | `fileId` | `EditResult` |
| `redo_draft_operation` | `fileId` | `EditResult` |
| `revert_change` | `fileId, opIndex` | `EditResult` |
| `discard_draft` | `fileId` | `DocumentSnapshot` |
| `preview_draft` | `fileId` | `ChangeSummary` |
| `save_draft` | `fileId, expectedFingerprint, acceptWarnings: boolean` | `SaveResult` |
| `get_conflict` | `fileId` | `ConflictInfo` |
| `reapply_draft` | `fileId` | `EditResult` |
| `save_draft_as_copy` | `fileId` | `ManagedDocument \| null` |
| `list_backups` | `fileId` | `BackupEntry[]` |
| `preview_restore` | `fileId, backupId` | `ChangeSummary` |
| `restore_backup` | `fileId, backupId, expectedFingerprint` | `SaveResult` |
| `pick_schema` | `fileId` | `ManagedDocument \| null` |
| `clear_schema` | `fileId` | `ManagedDocument` |
| `reveal_managed_file` | `fileId` | `void` |
| `open_in_external_editor` | `fileId` | `void` |
| `open_about_link` | `which`: `"site"` or `"releases"` | `void` (opens `https://altafino.com` or the GitHub releases page; any other id is `INVALID_OPERATION`) |
| `reveal_masked_value` | `fileId, maskId` | `string` |
| `list_leftover_temp_files` | `fileId` | `string[]` (file names) |
| `remove_leftover_temp_files` | `fileId` | `number` |
| `get_preferences` | – | `Preferences` |
| `set_preferences` | `prefs: Preferences` | `void` |

`save_draft` error codes the UI must handle: `EXTERNAL_CHANGE_CONFLICT`
(open conflict dialog), `PRESET_WARNINGS_REQUIRE_CONFIRMATION` (show
"Save with N warnings"), `SCHEMA_VALIDATION_FAILED`, `SYMLINK_REQUIRES_CONFIRMATION`,
`HARDLINK_REQUIRES_CONFIRMATION`, `BACKUP_FAILED`, `ATOMIC_WRITE_FAILED`, `READ_ONLY`.

## Events (`listen`)

| Event | Payload |
|---|---|
| `managed-file-external-change` | `{ fileId, fingerprint, hasUnsavedChanges }` |
| `managed-file-missing` | `{ fileId }` |
| `managed-file-access-changed` | `{ fileId, readable, writable }` |
| `draft-diagnostics` | `{ fileId, revision, diagnostics: Diagnostic[] }` (eng R14; drop stale revisions) |

## Lanes and ownership

| Lane | Owns | Must not edit |
|---|---|---|
| Document | `src-tauri/src/core/{document,patch,watcher}/`, `core/security/redact.rs`, `fixtures/` (additions) | other core modules, `src/` |
| Store | `src-tauri/src/core/{persistence,backup,registry}/`, `core/security/paths.rs` | other core modules, `src/` |
| Schema | `src-tauri/src/core/{schema,presets}/`, `presets/` | other core modules, `src/` |
| Frontend | `src/`, `tests/e2e/` (Playwright, mocked IPC), `DESIGN.md`, `public/` | `src-tauri/` |
| Integration | `src-tauri/src/commands/`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, capabilities, CI, WebdriverIO | — |

Changing a shared type in `core/types.rs` or `core/error.rs` is an
integration decision: do not change them in a lane; note the need in your
final report instead. Adding private helpers inside your own modules is fine.
