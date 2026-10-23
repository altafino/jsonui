# ConfigDeck — AI Coding Agent Build Specification

**Working product name:** ConfigDeck (renameable)  
**Document type:** Product requirements + architecture + implementation contract  
**Priority platform:** Manjaro Linux (Arch-based), initially GNOME; KDE and other Linux desktops supported  
**Secondary platforms:** Windows and macOS  
**Baseline:** October 2026  
**Status:** Implementation-ready MVP specification

---

## 0. Instructions to the AI coding agent

You are an autonomous senior Rust desktop engineer, frontend engineer, UX designer, and test engineer. **Build a functioning, polished desktop application**, not just a concept, mockup, API skeleton, or plan.

1. Read this entire specification before generating source code.
2. Inspect the current repository and environment. Preserve existing useful code if present.
3. Verify current versions and integration details against official documentation before adding dependencies. Do not hallucinate APIs or version numbers.
4. Implement incrementally in the order given in **Section 18**. Keep the application runnable after each milestone.
5. Prefer maintainable, composable modules and tests over one enormous component.
6. Treat safe, non-destructive manipulation of user configuration files as the highest priority. Never rewrite a real config file during automated tests.
7. Use temporary fixtures and synthetic data for all development, screenshots, demos, and automated tests.
8. Produce the actual project source tree, build scripts, tests, packaging configuration, screenshots, README, and a concise implementation/status report.
9. Run formatting, static analysis, unit tests, UI tests, and a production build. Fix issues before claiming completion.
10. If anything in the spec conflicts with a documented platform constraint, choose the safer behavior, record the reason in `docs/decisions.md`, and continue with the nearest workable implementation.
11. **Do not stop after planning or ask for confirmation about ordinary implementation details.** Make reasonable defaults consistent with this document.
12. Do not ship nonfunctional buttons, fake functionality, placeholder panels, or demo-only data disguised as real files. If a feature is deferred, remove its controls or mark it clearly as unavailable.

### Definition of success

A Manjaro user can install/run ConfigDeck, import an existing JSON file (including Claude Code settings), browse its structure as understandable controls, change booleans using checkboxes/switches, edit other common JSON types, preview the specific changes, safely save to the original file, and recover the previous version. The same core application architecture builds on other desktop operating systems.

---

## 1. Product vision

ConfigDeck is a **local-first, visually polished configuration editor for arbitrary JSON files**. It turns complicated nested JSON structures into accessible forms and controls, without forcing users to edit JSON source text.

It is intended for developers and technical users who manage several tools: Claude Code, AI coding agents, developer tools, JSON-based application settings, and custom internal configurations.

### Problems to solve

- Configuration files are scattered across home directories, project directories, and application-specific folders.
- Manually editing JSON is awkward and error-prone (quotes, commas, brackets, field types).
- Users may not know whether a value is a boolean, enum, integer, list, object, or nullable field.
- A GUI for one specific app quickly becomes outdated as config keys change.
- Several applications may modify the same file while the settings editor is open.
- A wrong save could break critical tools or silently erase undocumented keys.

### Product principles

1. **Any JSON first.** An app-specific preset is an enhancement, not a prerequisite.
2. **Form-first UX.** JSON source is an optional, read-only inspection/preview tool, not the main editor.
3. **Non-destructive.** Preserve everything the user did not explicitly change.
4. **No implicit save.** Changes are staged until the user presses Save.
5. **Discoverable and understandable.** Explain paths, types, scopes, defaults, and validation errors.
6. **No cloud dependency.** All parsing, editing, validation, and storage run locally.
7. **Good Linux citizen.** Work on Manjaro GNOME and common Linux desktops with keyboard, mouse, HiDPI, dark mode, and native file pickers.
8. **Cross-platform by design.** Avoid platform assumptions in the core domain logic.

### Non-goals for MVP

- AI assistant or AI-generated modification suggestions.
- Online synchronization and remote editing.
- Execution of hooks, shell commands, or arbitrary scripts contained inside configuration files.
- Full visual editing of YAML, TOML, XML, INI, or arbitrary programming-language configs.
- Changing Claude Code organization's managed settings or bypassing policy.
- A broad plugin marketplace or automatic downloading/execution of third-party code.
- Automatically computing every tool's actual runtime configuration precedence.
- Replacing full-featured text editors for advanced source editing.

---

## 2. Technology and architecture decisions

### Required stack

| Layer | Decision | Notes |
|---|---|---|
| Desktop shell | **Tauri 2** | Native application shell, packaging, OS integration, Rust commands |
| Core/backend | **Rust stable** | File management, parsing, patches, backups, conflicts, validation, watchers |
| UI | **React + TypeScript + Vite** | Strong ecosystem for advanced forms and polished desktop UX |
| Styling | **Tailwind CSS + shadcn/ui components** | Accessible, configurable component system; Radix-based primitives where appropriate |
| Icons | Lucide | Consistent line icon family |
| Frontend state | Zustand or equivalent small store | Separate app state from document draft state |
| Forms | A dedicated schema-aware recursive field renderer | Do not make an unmaintainable forest of one-off Claude-specific forms |
| Validation | JSON Schema where supplied + Rust authoritative checks | UI can use Ajv for fast inline feedback; Rust checks before commit |
| JSON document model | `serde_json` semantic model, plus a concrete syntax tree/token-aware editor | Do not serialize the whole document for a one-field update when a localized edit is possible |
| JSONC | `jsonc-parser` with CST feature (evaluate suitability) | Support `.jsonc` including comments; strict mode for `.json` |
| File change tracking | `notify` crate (or equivalent) | Handle Linux/Windows/macOS notification differences |
| Persistence of app metadata | Small application-owned JSON or SQLite storage | Never import and duplicate full user config content into a database by default |
| Frontend unit tests | Vitest + Testing Library | Rendering, keyboard interactions, draft behavior |
| End-to-end tests | Playwright for browser-level UI flows; Tauri/native integration where feasible | Use real Rust backend integration tests for file I/O |

**Why Tauri:** The application is genuinely Rust-based: its desktop shell and all privileged operations are in Rust. React is used only for the desktop user interface. A fully Rust-rendered UI framework such as Slint is an acceptable future alternative, but do not switch technology during this build. Prioritize a polished product and mature controls.

### Architecture boundary

```text
┌──────────────────────────────────────────────────────────────────┐
│ React / TypeScript UI                                            │
│ Sidebar · Search · Forms · Draft changes · Diff · Feedback       │
│                                                                  │
│ No direct unrestricted filesystem access                         │
└─────────────────────────────┬────────────────────────────────────┘
                              │ Typed Tauri commands/events
┌─────────────────────────────▼────────────────────────────────────┐
│ Rust application service                                         │
│ Registry · Document sessions · Schema registry · Validation     │
│ Conflict detection · Backups · Atomic persistence · File watcher │
└─────────────────────────────┬────────────────────────────────────┘
                              │
┌─────────────────────────────▼────────────────────────────────────┐
│ Rust document engine                                             │
│ Strict JSON / JSONC parser · Semantic tree · Minimal patch model  │
│ Serialized preview · Localized edits · Schema checks             │
└─────────────────────────────┬────────────────────────────────────┘
                              │
                         Local filesystem
```

Make the Rust document engine independent of Tauri wherever practical, allowing CLI/integration tests without a webview.

### Security boundary

- No JavaScript-side unrestricted file reads/writes. The Rust service owns all edits.
- File picker grants access to one chosen file; persist only explicitly managed paths.
- Rust still validates every command against its managed-file registry, not just a client-provided path.
- Use Tauri capabilities with least privilege. No shell execution plugin unless a narrowly defined future feature requires it.
- Bundle static frontend assets. Apply a restrictive content security policy; do not need a network connection.
- Do not execute file contents or treat them as instructions.

---

## 3. Users, scenarios, and primary workflows

### User personas

- **Developer:** manages global and per-project Claude Code settings and settings of other developer tools.
- **Technical power user:** imports miscellaneous JSON settings and expects clear nested forms without learning their structure.
- **Team member:** edits a shared project JSON config, previews changes, and avoids accidentally committing personal secrets.

### Core workflows

**A. First launch**

1. See a welcome screen with `Add JSON file`, `Discover Claude Code settings`, and `Try with sample`.
2. Select an existing JSON/JSONC file or create a new JSON file explicitly.
3. ConfigDeck identifies valid syntax, detects a matching application preset, and renders the form.
4. The imported file becomes a manageable sidebar item.

**B. Edit boolean and save**

1. Select a file.
2. Search/filter for a property.
3. Change an existing boolean using a toggle or checkbox.
4. A visible unsaved-change indicator appears; Save becomes enabled.
5. Open a change-preview dialog with the old and new value or a focused text diff.
6. Save. The file on disk is updated safely; the file remains valid JSON.
7. Show success and permit undo via history/backup restoration.

**C. Complex JSON structure**

1. Expand a nested object.
2. Edit a number, URL/string, enum, list item, or map entry.
3. Add an allowed property or array item where applicable.
4. Validate type/shape; highlight problems on the exact field.
5. Save changes without deleting unrelated fields.

**D. External change conflict**

1. Open a file and make unsaved edits.
2. Another editor modifies the file on disk.
3. ConfigDeck reports `This file changed outside ConfigDeck`.
4. Present `Compare`, `Discard my draft & reload`, or `Save a copy`.
5. Never overwrite the new external version silently. True field-level merge may be deferred; a conservative conflict dialog is mandatory.

**E. Claude Code settings**

1. Click `Discover Claude Code settings`.
2. Show known user-level candidates and let the user add one or more project directories.
3. Show separately managed user, project-shared, and project-local settings with scope badges in the file details (not decorative headings).
4. Apply a Claude-focused friendly schema where recognized; always retain generic editing for unrecognized keys.
5. Show the exact file being edited and never imply that changing one scope necessarily changes Claude's effective setting.

---

## 4. UX and visual design specification

### Design direction

Build a **modern, minimal desktop configuration workspace**. It should feel like a professional developer tool, not an HTML form embedded in a window. Think careful typography, restrained visual hierarchy, excellent spacing, strong keyboard support, and efficient density.

**Appearance:** default to system theme; both sophisticated dark and clean light themes are required. Use neutral/slate surfaces, one controlled accent color, subtle borders, modest corner radii, and tasteful hover/focus transitions. Avoid oversaturated gradients and huge cards.

### Desktop layout

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│ ConfigDeck                                    Search all settings     ⌘/Ctrl+K  ⚙ │
├───────────────────────┬──────────────────────────────────────────────────────────┤
│ CONFIGURATIONS        │  Claude Code · User settings             [Valid] [Save]   │
│ [+ Add file]          │  ~/.claude/settings.json · Last updated …                  │
│                       ├──────────────────────────────────────────────────────────┤
│ ★ Claude Code         │  [Search fields…] [Changed only] [Show advanced]           │
│   User settings       │                                                          │
│   Project A · Shared  │  General                                                 │
│   Project A · Local   │  Default model               [ text / suggested choices ]   │
│                       │  Example feature            [ ON  ●━━━━ ]                 │
│ ▾ Other JSON files    │                                                          │
│   App configuration   │  Permissions                    ▾                        │
│   Feature flags       │  Default permission mode     [ Choose…               ]   │
│                       │  Allow rules                 [editable list]              │
│                       │                                                          │
│                       │  Environment variables          ▾                        │
│                       │  [name]                         [value]       [+ Add]    │
│                       │                                                          │
├───────────────────────┴──────────────────────────────────────────────────────────┤
│ Ready · 2 pending changes                         [Review changes] [Save changes] │
└──────────────────────────────────────────────────────────────────────────────────┘
```

This is a **structural reference**, not literal final styling or factual Claude property inventory. Replace placeholders with supported settings; do not invent product-specific keys.

### Screen and component requirements

**Sidebar**

- Shows imported files grouped by manually assignable collection or detected application.
- Each row includes friendly name, filename/path tooltip, valid/invalid status, modified state, and unsaved dot.
- Context menu: Open, Rename display name, Reveal in file manager, Copy path, Remove from ConfigDeck (not delete file).
- Distinguish nonexistent, read-only, invalid, and externally modified files.
- Collapsible and resizable; remember width locally.

**File header**

- Friendly title; full resolved path accessible by copy/reveal action.
- Syntax type (`JSON`/`JSONC`), detected preset, scope when known, read-only status.
- Reload, review changes, save, and file history actions.
- Critical write operations should be impossible while syntax/validation errors exist.

**Main form**

- Stable groups derived from schema headings; otherwise follow source object nesting and property order.
- Progressive disclosure: nested objects fold/unfold; huge arrays are navigable and render efficiently.
- Every field shows readable label, field type when useful, a short explanation when available, and optional help tooltip.
- Advanced details show the actual JSON path such as `$.permissions.defaultMode`.
- Search matches friendly label, raw key, path, and schema description, with matching parent groups expanded.
- `Changed only` filter and global per-file search; a workspace-wide search is a stretch feature.
- Context action `Copy JSON path`.
- No constant save dialogs for every checkbox toggle.

**Footer / save bar**

- Persistent unsaved-change count and `Review changes` / `Save changes`.
- `Ctrl+S` / `Cmd+S` saves only if validation succeeds and no external conflict exists.
- `Ctrl+Z` / `Cmd+Z` and redo within focused form inputs where appropriate; support document-wide draft undo/redo without breaking normal text field behavior.

**Settings / onboarding**

- Theme (system/light/dark), font scaling, density (comfortable/compact).
- Default backup retention, automatic watcher behavior, optional sensitive-value hiding.
- Manage application presets/schema associations and registered files.
- Do not request root privileges or configure system-wide autostart.

### Accessibility and responsiveness

- Keyboard navigation through all inputs, lists, modal dialogs, and sidebar items.
- Visible keyboard focus rings; clear labels associated with controls.
- Prefer native button semantics and accessible component primitives.
- WCAG AA contrast target and appropriate ARIA live regions for save/conflict messages.
- No color-only error signalling.
- Support window widths from **1024 px** and 1366×768 layouts without horizontal app scrolling; support HiDPI/4K with normal OS scaling.
- No UX regressions under GNOME Wayland and X11.

---

## 5. Generic JSON → visual controls engine

The form renderer must work **with or without a JSON Schema**. The file's existing structure is authoritative for values; a schema adds labels, validation, enums, allowed extra fields, hints, and optional properties.

### Type mapping

| JSON / schema condition | Primary control | Additional behavior |
|---|---|---|
| `boolean` | Toggle or checkbox | Never conflate `false` with missing |
| `string` | Text field | Multiline if annotated; show character limits |
| `string` with `enum` | Dropdown / searchable select | Use labels/description from schema when available |
| `integer` | Numeric input | Enforce integer and min/max constraints |
| `number` | Numeric input | Preserve precision and valid JSON number representation |
| `string`, format `uri` | URL field | Validate via schema; do not automatically open URLs |
| `string`, format `date` / `date-time` | Date/time control if schema explicitly declares it | Keep exact expected serialization |
| `string`, format `password` or marked secret | Masked field | Reveal-on-demand, no unexpected clipboard exposure |
| `null` | Explicit null field | Provide type-selection only when schema permits |
| `object` | Collapsible section | Recursively render children |
| Map/dictionary object (`additionalProperties`) | Editable key/value table | Validate key uniqueness, rename safely |
| `array` of scalar values | Ordered list editor | Add, edit, delete, reorder |
| `array` of objects | Reorderable collapsible item cards | Show short item summary |
| Heterogeneous array / `oneOf` / `anyOf` | Typed list with explicit variant selector | Respect schema constraints |
| Unknown/unsupported schema keyword | Best-effort generic controls | Warn without dropping data |

### IMPORTANT: absent / null / false are different

For optional settings, visually distinguish:

- **Not set:** no property is present in that file (the underlying app may use a default or inherit elsewhere).
- **Set to false:** the property explicitly exists with `false`.
- **Set to null:** the property explicitly exists with `null` (only where valid).
- **Set to true:** the property explicitly exists with `true`.

For an **existing** boolean, a checkbox/switch provides normal `true/false` behavior. For an **optional missing** boolean, use a visible `Not set / On / Off` control or an `Add setting` action before switching. Do not materialize missing fields merely because the renderer displays a default value.

### Dynamic field features

- Inspect keys recursively from the actual document.
- For a schema-known optional property that is not present, show it only in an `Add setting` menu or schema-guided advanced view.
- When no schema is present, permit adding properties with a new unique key and chosen JSON type.
- Permit removing a field only through an explicit field action with Undo support.
- Permit editing array members and object map entries while retaining their order where possible.
- Display actual values, schema `default` hints, and inheritance hints **separately**; never write a default automatically.
- Treat `$schema` as metadata when present, but preserve it exactly like any other existing key.
- Surface type mismatches near controls; do not silently coerce strings such as `"false"` into booleans.
- A schema association must never cause unrecognized existing fields to vanish.
- Use stable tree node identities based on JSON Pointer (RFC 6901) plus item/session IDs as needed for list edits, avoiding array index key issues during reorder.

### Efficient rendering

- Render only expanded object/array branches.
- Debounce expensive validation/search as appropriate.
- Support typical 10 KB–1 MB config files comfortably; document a maximum file size and prevent blocking the UI on larger files. Default maximum editable size may be **5 MiB**, configurable later.

---

## 6. Schema support and application presets

### Schema sources, in precedence order

1. User-selected JSON Schema file associated with this managed file.
2. Built-in, versioned application preset (where a match is reliable).
3. A trusted local `$schema` URI/file reference after user consent; never fetch remote schemas automatically.
4. Structural inference from the current JSON value.

Schemas must be data only: **JSON Schema with optional supported UI metadata**, not code or script execution. Schema handling needs deterministic merge/precedence rules and a visible `Schema source` indicator.

### Supported schema subset in MVP

- `type`, `properties`, `required`, `additionalProperties`, `items`, `enum`, `const`, `default` (hint only), `title`, `description`, `minimum`, `maximum`, `minLength`, `maxLength`, `pattern`, `format`, `oneOf`/`anyOf` where feasible, `$ref` only with safe local resolution.
- Basic JSON Schema draft 2020-12 validation through an actively maintained library (verify support).
- Invalid or unresolvable schemas must not prevent **generic structural editing** when the JSON document itself is valid; show warning and disable schema-only affordances.
- Guard against cyclic references, remote network access, and pathological validation complexity.

### Built-in preset format

Keep bundled presets **independent from the Rust engine**, for example:

```text
presets/
  claude-code/
    manifest.json
    settings.schema.json
    ui.json
  generic/
    manifest.json
```

Example **illustrative** UI metadata (not a full schema):

```json
{
  "presetId": "claude-code-settings",
  "displayName": "Claude Code Settings",
  "groups": [
    { "id": "general", "title": "General", "paths": ["/model"] },
    { "id": "permissions", "title": "Permissions", "paths": ["/permissions"] },
    { "id": "environment", "title": "Environment", "paths": ["/env"] },
    { "id": "hooks", "title": "Hooks", "paths": ["/hooks"] }
  ],
  "secretPathPatterns": ["/env/*TOKEN*", "/env/*KEY*"]
}
```

The example secret patterns are advisory UI hints, **not** a complete or trustworthy secret detector. Support explicit user masking and cautious defaults for suspected credential fields.

### Preset stability

- Version and document each built-in preset.
- Unknown app settings must remain editable generically.
- Do not hardcode a fixed list of Claude models, hooks, or permission modes when the vendor may change them.
- App presets may suggest values but must not declare unknown values invalid unless an authoritative associated schema requires that constraint.
- No silent background downloads of new schemas or preset versions in MVP.

---

## 7. Claude Code integration: initial built-in preset

Claude Code is the first supported application, **not** the only supported application.

### Known configuration scopes and locations

| Scope | Candidate path | Behavior |
|---|---|---|
| User | `${CLAUDE_CONFIG_DIR}/settings.json` if environment variable is set; otherwise `~/.claude/settings.json` | Offer discovery/import; user-editable if filesystem permits |
| Shared project | `<project>/.claude/settings.json` | Discover only in directories explicitly selected by user |
| Local project | `<project>/.claude/settings.local.json` | Discover only in directories explicitly selected by user |
| Enterprise-managed file (Linux) | `/etc/claude-code/managed-settings.json` | Optional read-only discovery; never offer an elevated write workflow |
| Claude runtime/state file | `~/.claude.json` (or relocated as applicable) | **Do not auto-import or auto-edit**; may contain account/MCP/runtime state and sensitive information |

File precedence and content merge behavior can be nuanced. In the MVP, present source locations/scopes accurately and **edit one physical file at a time**. Do not falsely label a value as the effective final setting when other scopes or managed policies may override it.

### MVP-friendly settings categories

The preset should present these **only if they are present or documented by the currently reviewed authoritative schema**:

- General settings and default model (string with helpful description).
- Permissions, including `permissions.allow`, `permissions.ask`, `permissions.deny`, and supported permission-related keys.
- Environment variables, editable as name/value pairs with masking options.
- Hooks, rendered as nested arrays/maps in a structured editor (never executed by ConfigDeck).
- Status line and other nested configuration objects.
- Plugin-related configuration when applicable.
- Any unknown key through the generic form renderer.

### Specialized UX

- Display `User`, `Project shared`, and `Project local` scope labels near file titles.
- Warn before changing permissions that could loosen approvals or expand tool access; explain this is a potentially sensitive setting.
- For hooks, show command strings plainly, but **never** provide a `Run hook` button or execute scripts while previewing.
- Provide a visible prompt for project-local files reminding users to keep personal settings out of version control (do not alter Git settings automatically).
- Import/discovery is opt-in. Never crawl the entire home folder or all repositories silently.
- If Claude Code modifies a file while ConfigDeck is open, use the standard external-change conflict workflow.

### Authoritative references (verify again during implementation)

- Settings files and precedence: https://code.claude.com/docs/en/settings
- Settings reference index: https://code.claude.com/docs/en/settings#all-settings (if anchor changes, navigate from settings page)
- Claude Code configuration and policies: https://code.claude.com/docs/en/settings
- Claude Code example settings: https://github.com/anthropics/claude-code/tree/main/examples/settings

**Note:** This is a fast-changing external product. The agent must validate schemas and supported keys against current official documentation before finalizing the preset. Avoid using outdated community blog posts as a definitive schema.

---

## 8. File management requirements

### Registry

Each managed document stores application metadata, not a shadow copy of all config values:

```typescript
type ManagedDocument = {
  id: string;
  displayName: string;
  absolutePath: string;      // normalized and validated in Rust
  kind: 'json' | 'jsonc';
  presetId?: string;
  localSchemaPath?: string;
  collectionId?: string;
  discoveredScope?: 'user' | 'project-shared' | 'project-local' | 'managed' | 'generic';
  readOnly: boolean;
  lastOpenedAt?: string;
};
```

All type definitions above are conceptual; define canonical Rust DTOs and generate/maintain aligned TypeScript contracts where feasible.

### File operations

- Add existing file with native file picker or drag and drop (if platform behavior reliable).
- Add new file by choosing destination and initial `{}` document; warn before replacing an existing path.
- Rename a **display label** independently of underlying filename.
- Remove a file from ConfigDeck without deleting the source file.
- Choose a different file for an existing registry entry when path changed.
- Reveal source file in native file manager; copy absolute path.
- Reload after external changes; ask how to handle unsaved edits.
- Handle missing file, permission denied, invalid syntax, empty file, file too large, non-UTF-8 data, and dangling symlinks with explicit UI states.

### File watcher behavior

- Watch registered files (or nearest existing parent directories as needed) using a portable watcher.
- Debounce events and detect change by content fingerprint, not timestamp alone.
- Ignore ConfigDeck's own successful writes using the committed post-save fingerprint, without ignoring genuine subsequent external edits.
- After an external change with **no draft**, automatically reload and show a non-intrusive notice.
- After an external change **with draft**, preserve the draft, freeze Save, and show a conflict state.
- Restart file watching after an atomic replace/rename, as some platforms change file identity.

### Persistence location

- Store registry and app preferences in the OS-appropriate per-user application configuration directory via Tauri/path facilities (typically XDG config on Linux).
- Store backup data under the OS-appropriate per-user application data directory.
- Do not write ConfigDeck metadata into the user's imported configuration files.
- Respect Linux XDG conventions and avoid hardcoded `/home/<username>` assumptions.

---

## 9. JSON fidelity and editing contract (critical)

**Most important engineering requirement: changing a single field must not rewrite or destroy unrelated content.**

### Parsing modes

- For `.json`, accept **strict JSON only**; comments and trailing commas are invalid unless the user explicitly imports as JSONC and changes the extension/use accordingly.
- For `.jsonc`, allow comments and the standard expected JSONC extensions supported by the selected parser; do not silently enable unrelated JSON5 syntax.
- Preserve UTF-8 content, ordering, indentation, line endings (LF/CRLF), end-of-file newline, comments/trivia in JSONC, and existing formatting **outside the modified subtree** wherever technically possible.
- For files with duplicate keys: block editing/saving by default with a clear diagnostic; never resolve silently by retaining just one duplicate key.
- For malformed files, show errors with line/column and an option to open in an external editor. Do not attempt a destructive auto-fix.

### Document editing model

Maintain:

```text
originalRawBytes + originalFingerprint
parsedSyntaxTree + projectedSemanticTree
pendingOperations[]  // set / add / delete / array insert / remove / move
schemaAssociation + validationDiagnostics
```

Suggested typed operation interface:

```typescript
type JsonPatch =
  | { op: 'set'; path: string; value: unknown }
  | { op: 'add'; path: string; value: unknown }
  | { op: 'remove'; path: string }
  | { op: 'array-insert'; path: string; index: number; value: unknown }
  | { op: 'array-move'; path: string; from: number; to: number };
```

Paths are RFC 6901 JSON Pointers, **not** dot-separated pseudo paths. Validate operation types and bounds in Rust.

### Non-destructive editing requirements

- For `true → false`, change only the boolean token whenever possible.
- For scalar edits, preserve surrounding whitespace and member order.
- For array edits, preserve unchanged elements/comments and minimize textual churn.
- Unknown fields remain intact, regardless of schema coverage.
- If CST rewriting cannot safely preserve a document (particularly JSONC comments), **block save and explain** instead of silently serializing the entire document with lost comments.
- Round-trip an unmodified file byte-for-byte; opening and saving with no changes must be a no-op.
- Any formatting changes outside edited paths require an explicit accepted fallback workflow, never an invisible side effect.

### Editing preview

`Review changes` shows both:

1. **Semantic changes:** `permissions.allow[2] removed`, `feature.enabled false → true`.
2. **Text diff:** original file compared against exact candidate bytes, with sensitive values redacted by default.

For a simple boolean change, the diff should show only the relevant line/token rather than wholesale document reformatting.

---

## 10. Safe saving, conflicts, and backups (critical)

### Required save algorithm

1. The user deliberately activates Save.
2. Resolve and validate registered destination, file type, permissions, file size, and symlink policy.
3. Read source bytes and compare their content fingerprint with the snapshot loaded by this editing session.
4. If changed, **abort with conflict**. Do not overwrite. Filesystem notification is only an early-warning mechanism; this recheck is mandatory.
5. Apply pending operations to the original syntax tree, producing candidate bytes.
6. Parse candidate again in correct strict-JSON/JSONC mode and validate schema when one is active.
7. Reject candidate if any errors are introduced or existing protected invariants are broken.
8. Before replacement, create a timestamped backup of original bytes in the ConfigDeck backup store, with restrictive permissions and no plaintext secret values in log entries.
9. Recheck source fingerprint immediately before committing; detect obvious concurrent changes.
10. Write to a temporary file **in the same directory** as the source, flush and `fsync` where supported, preserve relevant existing file permissions, and atomically replace destination using a platform-appropriate strategy.
11. Ensure parent directory durability where supported; handle failures without corrupting original. On Windows, use a documented safe replacement mechanism rather than assuming POSIX rename semantics.
12. Read back and verify the committed candidate fingerprint.
13. Only then clear pending edits and report success; ensure watcher sessions adopt the new fingerprint.

**Race note:** A simple fingerprint check is not a fully atomic compare-and-swap. Implement locking/recheck carefully and document the residual race with cooperating/non-cooperating external editors. Do not claim impossible absolute transactional guarantees across arbitrary third-party processes.

### Security and path safety

- Do not follow symlinks to writable destinations silently. Prefer **read-only or explicit confirmation** for symlinked paths; support actual resolved target only after validation. Avoid file type/symlink TOCTOU flaws.
- Do not allow editing directories, device nodes, pipes, sockets, or files that are not normal files.
- Keep new backup files user-private (e.g. 0600 on Linux); secure backup directories (e.g. 0700).
- Never broaden original file permissions when saving.
- Do not request `sudo` access, bypass managed policies, or modify read-only system files.
- Handle filesystem errors without leaking credential values into logs/toasts.

### Backup and history UX

- Create **one backup for each successful save** (before modifying the source).
- Display backup timestamp, location, size, and a readable summary of change count if available.
- Allow restoring a selected snapshot, subject to the same conflict checks and safety rules as a normal save.
- Offer retention controls, default **20 backups per file** and optional age-based cleanup; never delete source files.
- If backup creation fails, **do not save** and show an actionable error.
- Include backup restore tests.

### Unsaved changes

- Warn on closing a window or switching documents with an unsaved draft; allow `Save / Discard / Cancel`.
- Save is disabled when there are validation errors, read-only status, or external changes.
- Save button tooltip must explain why it is disabled.

---

## 11. Domain model and Rust API contract

Define a domain library (or set of backend modules) with these responsibilities:

```text
core/
  registry              // registered files, groupings, file metadata
  document              // parse, snapshot, semantic tree
  patch                 // structured mutations and preview
  schema                // schema association and validation
  backup                // snapshot/history/restore
  persistence           // safe writes and file-specific metadata
  watcher               // external file change event handling
  presets               // app-specific detection and UX hints
  security              // path checks, secret masking, permission checks
```

### Minimum Tauri command surface

Names may vary; preserve semantics and narrow privilege:

```text
list_managed_files()
add_managed_file(selected_path, optional_display_name)
create_managed_file(selected_path)
remove_managed_file(file_id)
update_managed_file_metadata(file_id, metadata_patch)
list_discoverable_claude_files(optional_project_directory)
open_document(file_id)
reload_document(file_id)
get_document_tree(file_id)
get_schema_hints(file_id)
apply_draft_operation(file_id, operation)
undo_draft_operation(file_id)
redo_draft_operation(file_id)
validate_draft(file_id)
preview_draft(file_id)
save_draft(file_id, expected_fingerprint)
discard_draft(file_id)
list_backups(file_id)
preview_restore(file_id, backup_id)
restore_backup(file_id, backup_id, expected_fingerprint)
associate_schema(file_id, local_schema_path)
reveal_managed_file(file_id)
```

You may keep ephemeral draft edits entirely in React and send a complete sequence to Rust for preview/save, **provided** the Rust side validates and owns the authoritative base snapshot and all filesystem operations. Prefer sharing a well-tested patch implementation over duplicated semantics in Rust and TypeScript.

### Events

```text
managed-file-external-change: { fileId, fingerprint, hasUnsavedChanges }
managed-file-missing: { fileId }
managed-file-access-changed: { fileId, readable, writable }
```

Avoid streaming full config file contents into global events or logs.

### Error model

Return typed errors with stable codes and safe user-facing messages, for example:

- `NOT_FOUND`
- `INVALID_JSON`
- `DUPLICATE_KEYS`
- `READ_ONLY`
- `SYMLINK_REQUIRES_CONFIRMATION`
- `FILE_TOO_LARGE`
- `SCHEMA_VALIDATION_FAILED`
- `EXTERNAL_CHANGE_CONFLICT`
- `BACKUP_FAILED`
- `ATOMIC_WRITE_FAILED`
- `UNSUPPORTED_ENCODING`

Include actionable details (file path, line/column, permitted remediation) without exposing sensitive values.

---

## 12. Functional priority and MVP scope

### P0 — Must ship

- Tauri/Rust/React cross-platform foundation.
- Modern dark/light UI, sidebar, configuration workspace, native picker.
- Persistent list of managed JSON/JSONC files.
- Generic recursively generated form editor for booleans, strings, numbers, nulls, objects, arrays.
- Distinct handling of absent, null, false, and true.
- Adding/removing map entries and list items.
- Search in current file, field descriptions/JSON Pointer display.
- Draft edits, unsaved indicator, discard and review diff.
- Strict JSON and JSONC parsing, syntax validation, non-destructive CST edits.
- Safe persistence with external modification conflicts.
- Backup before successful write and working restoration.
- Claude Code initial preset with import/discovery of user/project settings.
- Fully offline operation; no unwanted external network use.
- Tests and a Manjaro-usable packaged artifact/build instructions.

### P1 — First follow-up release

- User-associated JSON Schema import and improved schema-driven controls.
- User-defined groups and favorite fields.
- Workspace-wide field search.
- Multi-file change review (but still explicit save per file initially).
- Better array editors, bulk edit/reorder, and copy/paste value support.
- More robust secret detection, masked diffs, more granular field notes.
- Automatic configuration preset update checks (opt-in, signed/verified only).
- Window layout restoration, deep keyboard shortcuts, compact density mode.
- Better cross-platform packaging and update channel.

### P2 — Later exploration, not a prerequisite

- Multiple selectable profiles/environments with careful source-to-destination semantics.
- Effective configuration viewer for specific app precedence rules.
- Version-control-aware previews and Git integration.
- Format support for YAML/TOML via separate document engines.
- Optional AI explanation or assisted configuration generation.
- Portable export/import of ConfigDeck's own registry without copying secrets.
- Preset marketplace, after defining a safe, data-only trust model.

**Implementation rule:** Complete P0, including tests, before working on P1/P2.

---

## 13. Data fixtures and representative use cases

Use fixtures that test both common and difficult structures. Do not copy anyone's real personal settings.

### Generic sample fixture: `fixtures/feature-flags.json`

```json
{
  "application": "example-service",
  "enabled": true,
  "debug": false,
  "retryCount": 3,
  "timeoutSeconds": 1.5,
  "displayName": "Local Development",
  "optionalValue": null,
  "logging": {
    "level": "info",
    "toFile": false
  },
  "features": ["search", "analytics"],
  "services": [
    { "name": "frontend", "enabled": true },
    { "name": "backend", "enabled": false }
  ],
  "extraUndocumentedKey": {
    "mustRemain": "untouched"
  }
}
```

### Small illustrative Claude Code fixture: `fixtures/claude-settings.json`

```json
{
  "model": "example-model-name",
  "permissions": {
    "allow": ["Bash(git status)"],
    "ask": [],
    "deny": ["Read(.env)"]
  },
  "env": {
    "EXAMPLE_NON_SECRET_FLAG": "1"
  },
  "unknownFutureVendorSetting": {
    "enabled": false
  }
}
```

The example model name and permission patterns are **test data**, not verified product defaults or recommended production permissions.

### JSONC fidelity fixture: `fixtures/commented.jsonc`

```jsonc
{
  // Keep this explanation next to the value.
  "enabled": true,
  "items": [
    "first", // Comment attached to first item.
    "second",
  ],
}
```

Required fixture assertion: editing `/enabled` from `true` to `false` changes only that literal token and preserves all other bytes, including comments and trailing comma.

### Edge-case fixture list

- Empty object `{}` and empty array `[]`.
- Deep nested objects and arrays (within guarded depth limits).
- A missing optional boolean (vs explicit `false`).
- An optional nullable scalar (missing, null, and value).
- Unicode text, escaped JSON Pointer segments (`~0`, `~1`), and Windows/Unix line endings.
- File with duplicated property names (must be rejected for editing).
- Invalid JSON (missing comma / unterminated string) with line/column diagnostics.
- JSONC comments before/after keys and list items.
- File modified externally while draft is pending.
- Read-only file and denied write permission.
- Missing source file, symlink, oversized file, invalid UTF-8.
- File containing a token-like secret (must be masked in preview and excluded from logs).

---

## 14. Acceptance tests (must be automated where feasible)

| ID | Scenario | Expected result |
|---|---|---|
| A01 | Import valid generic JSON | Form is generated without any preset |
| A02 | Toggle existing `true` boolean to false | Only that value changes in draft and saved JSON |
| A03 | Toggle false back to true | Reversible with undo and without coercing to string |
| A04 | Optional boolean not present | Remains absent until explicitly added |
| A05 | Edit integer and fractional number | Correct numeric JSON types preserved; invalid text blocked |
| A06 | Add/remove/reorder array entries | Output order/value matches action; unrelated elements intact |
| A07 | Add/rename/remove map property | Correct unique keys; duplicate key rejected |
| A08 | Edit nested value | Correct JSON Pointer path used |
| A09 | Import JSON without matching schema | Unknown fields remain editable |
| A10 | Associate restrictive local schema | Errors displayed by field and save blocked when invalid |
| A11 | Edit strict `.json` with a comment | Invalid syntax is reported; no rewrite |
| A12 | Edit JSONC scalar | Comments/trailing commas untouched outside modified value |
| A13 | Open then close unchanged file | Original bytes remain identical and no backup created |
| A14 | External modification with unsaved draft | Conflict appears; external bytes never overwritten |
| A15 | External modification without unsaved draft | Safe reload occurs and UI updates |
| A16 | Save successfully | Backup exists and candidate validates |
| A17 | Restore backup | Previous content restored using safe write procedure |
| A18 | Backup creation fails | Write aborted; original unchanged |
| A19 | Atomic replacement fails | Error shown; source not truncated/corrupted |
| A20 | Unauthorized/unregistered frontend path | Rust rejects privileged filesystem operation |
| A21 | Attempt to edit symlinked file | Explicit safe policy enforced |
| A22 | Open invalid JSON | Exact diagnostic shown; editing/saving unavailable |
| A23 | Open file with duplicate JSON keys | No silent dropping/merging; save unavailable |
| A24 | Import Claude user settings fixture | Claude grouping is selected and generic unknown key shown |
| A25 | Select Claude project directory | Existing shared/local settings are offered, not silently modified |
| A26 | Try to edit managed Claude policy | Default UI stays read-only; no elevation |
| A27 | Keyboard-only edits and save | All primary actions accessible, focus visible |
| A28 | Dark/light/system modes | Form controls remain readable and usable |
| A29 | Search by nested key/path | Correct matching fields displayed with parent context |
| A30 | Save with secret-like value changed | No secrets in application logs; diff masking defaults work |

### Additional quality tests

- Unit/property-based tests for patch operations and JSON Pointer escaping.
- Snapshot/fuzz tests for parse → edit → write → parse invariants.
- Format-fidelity tests that diff raw bytes and assert no unintended hunks.
- Temporary-directory tests for filesystem permission and conflict behavior on Linux.
- UI tests for empty states, complex forms, validation, undo/redo, conflict modals, and unsaved warnings.
- Smoke test packaging/launching on Manjaro using a non-root account.

### Nonfunctional targets

- Cold start should feel quick (aim for **under 2 seconds** on a typical modern desktop, measure instead of claiming it).
- Changing a simple toggle should visibly respond in **under 100 ms** for typical configs.
- Parsing/watching must not freeze the UI thread.
- Keep the default desktop bundle lightweight and free of bundled runtimes that Tauri does not need.
- Use stable Rust and avoid nightly-only requirements.
- Never transmit user file contents to an online service.

---

## 15. Repository structure to create

```text
configdeck/
├── README.md
├── LICENSE
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/
│   ├── app/
│   ├── components/
│   │   ├── chrome/
│   │   ├── file-list/
│   │   ├── document/
│   │   ├── fields/
│   │   ├── diff/
│   │   └── dialogs/
│   ├── hooks/
│   ├── lib/
│   │   ├── tauri-api/
│   │   ├── types/
│   │   ├── forms/
│   │   └── schemas/
│   ├── state/
│   ├── styles/
│   └── test/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       ├── core/
│       └── platform/
├── presets/
│   ├── claude-code/
│   └── generic/
├── fixtures/
├── tests/
│   ├── e2e/
│   └── integration/
├── docs/
│   ├── architecture.md
│   ├── decisions.md
│   ├── design-system.md
│   ├── file-safety.md
│   ├── packaging.md
│   └── known-limitations.md
└── .github/workflows/
```

Adjust directories as appropriate, but maintain clear ownership between frontend, Tauri adapter, and Rust document engine. Do not include sensitive user settings, live `.env` secrets, or machine-specific absolute paths in the repository.

---

## 16. Platform and distribution requirements

### Primary: Manjaro Linux

- Develop/test with an up-to-date stable Rust toolchain, Node LTS, and Tauri 2 prerequisites for Arch-based Linux.
- Verify exact Linux dependencies using official Tauri documentation (typically WebKitGTK 4.1 development/runtime dependencies and appropriate appindicator/tray libraries if used).
- Test on GNOME Wayland and X11; KDE smoke test when available.
- Honor OS window behavior, file dialog integrations, clipboard, HiDPI, and locale.
- Provide a clear first-party run/build path (`pnpm install`, `pnpm tauri dev`, `pnpm tauri build`, or the exact equivalent scripts configured in the repository).
- Produce a **working Linux distributable** suitable for Manjaro; evaluate AppImage portability and dynamic runtime library limitations on real target systems. Provide an Arch/AUR packaging recipe later if needed.

### Secondary: Windows and macOS

- Use portable Rust filesystem/path APIs; no Bash-only runtime assumptions.
- Enforce correct per-user config/data directories, paths, line endings, and safe atomic file replacement.
- Include build scripts/configuration for desktop distribution in later CI milestones.
- A developer may need native Windows/macOS CI runners for packaging verification; do not pretend Linux cross-compilation alone proves an installer works.

### Official project references

- Tauri v2 docs: https://v2.tauri.app/
- Linux development prerequisites: https://v2.tauri.app/start/prerequisites/
- Tauri file system/security model: https://v2.tauri.app/plugin/file-system/
- Tauri packaging: https://v2.tauri.app/distribute/
- Rust JSONC parser candidate: https://docs.rs/jsonc-parser/latest/jsonc_parser/
- JSONC draft format specification: https://jsonc.org/

---

## 17. Design quality bar

Implement at least these polished details:

- Elegant landing/empty state with a concise explanation and three useful actions.
- Thoughtful typography scale and harmonious spacing/density; consistent components and dialog styles.
- Professional visual treatment for nested property groups, tooltips, and complex list editors.
- Semantic field-type hints without overwhelming visual noise.
- Clear but restrained error/success/warning states.
- Excellent layout at 1366×768 and 4K HiDPI resolutions.
- Loading states that do not flash placeholders for trivial actions.
- Carefully designed conflicts and review modal, not browser `alert()`/`confirm()` popups.
- Accessible light/dark form controls, including hover, active, focus, disabled, invalid, and pending states.
- Keyboard shortcuts and a visible shortcut list in settings/help.

**Visual verification:** Create representative screenshots of the main workspace, nested array editing, change review, conflict dialog, onboarding, and dark/light themes. Inspect screenshots, correct clipping/spacing, and repeat until credible for a commercial-quality developer desktop app.

---

## 18. Build plan / execution order

### Milestone 1 — Foundation

Deliver: Tauri 2 project, React/Tailwind component system, typed command bridge, responsive app chrome, system/light/dark theme, first-launch empty state, cross-platform path abstraction, README.

**Gate:** `tauri dev` launches, build works, screenshots show credible basic UX.

### Milestone 2 — Real file registration and reading

Deliver: native picker, managed-file registry, persisted sidebar, Rust strict JSON/JSONC parser, readable error diagnostics, opening files, file metadata, non-destructive document snapshots.

**Gate:** user selects arbitrary fixture files and sees the actual parsed hierarchy.

### Milestone 3 — Generic interactive form engine

Deliver: recursive controls for all JSON types, add/delete/reorder, absent vs null vs false, search, nested groups, draft operations, undo/redo.

**Gate:** all fixture types can be edited without using JSON text.

### Milestone 4 — Validation, preview, and safe persistence

Deliver: JSON Schema association/basic validation, localized JSON/JSONC edits, visual change review, backup before save, atomic replace, conflict fingerprints, undo/restore history.

**Gate:** A02, A04, A12–A23, A30 tests pass; source files do not suffer formatting loss or unintended changes.

### Milestone 5 — Claude Code preset

Deliver: Claude location discovery, project-directory selection, user/shared/local scope labels, helpful descriptions for supported keys, nested permissions/hooks/env controls, unknown keys preserved.

**Gate:** Claude fixtures import, edit, validate, save, and back up like generic documents; managed paths stay protected.

### Milestone 6 — Hardening and packaging

Deliver: accessibility, keyboard support, GNOME/Wayland QA, performance pass, file watcher edge cases, packaging, screenshots, comprehensive documentation and tests.

**Gate:** all P0 acceptance tests pass; Linux distributable is produced and launched on a Manjaro target or a clearly documented equivalent supported test environment.

### Per-milestone engineering procedure

1. State the milestone goal and list affected modules.
2. Implement runnable code rather than pseudocode.
3. Add focused tests and synthetic fixtures.
4. Run formatter/linter/check/tests/build and report actual command results.
5. Record decisions/known limitations without hiding failing tests.
6. Continue to the next milestone unless genuinely blocked by an external dependency.

---

## 19. Release checklist

- [ ] App launches on Manjaro as a real desktop window.
- [ ] Working native file picker and persistent file registry.
- [ ] Generic JSON form editing, including boolean checkboxes/toggles.
- [ ] Correct editing of missing, null, false, and true states.
- [ ] Nested objects, arrays, and maps work.
- [ ] Claude settings discovery and useful form organization work.
- [ ] All files saved in their original location only on explicit user action.
- [ ] Preview clearly shows exact changes, with secret masking.
- [ ] JSON/JSONC syntax fidelity is tested.
- [ ] Unknown fields and unrelated formatting are preserved.
- [ ] Schema validation never silently deletes user data.
- [ ] External file edits cannot be overwritten without conflict handling.
- [ ] Backups are mandatory and restore works.
- [ ] Malformed/duplicate-key documents cannot be silently rewritten.
- [ ] No unmanaged path escalation or permission widening.
- [ ] No network requirement or automatic secret upload.
- [ ] Light/dark themes, keyboard navigation, and HiDPI tested.
- [ ] Unit/integration/UI tests pass and production build succeeds.
- [ ] README covers build, run, installation, screenshots, limitations.
- [ ] No placeholder-only controls or undocumented fake behavior.

---

## 20. Final response contract for the coding agent

When implementation is complete, report:

1. **Implemented features** (only those demonstrably working).
2. **Repository tree** and key module names.
3. **How to run on Manjaro** (verified commands).
4. **How to build/package** (actual verified artifacts and locations).
5. **Automated test results** (commands, pass/fail counts, known failures).
6. **Screenshots or previews** of polished UI states.
7. **Remaining issues** sorted by severity, especially any file-safety limitations.
8. **Next recommended milestone**, only after P0 has passed.

**Do not report the project as production-ready if save safety, conflict detection, backup recovery, or byte-fidelity tests are failing.**

---

## 21. Short agent kickoff prompt

Paste the following into an AI coding agent along with this file:

> Build the complete ConfigDeck desktop application defined in `ConfigDeck_AI_Agent_Build_Spec.md`. Use the specified Tauri 2 + Rust + React/TypeScript architecture. Prioritize Manjaro Linux, polished UX, generic schema-aware JSON forms, Claude Code settings as the first preset, safe format-preserving edits, conflict detection, backups, and real tests. Execute milestones in order and produce runnable source code and a Linux package. Do not stop at a plan, do not rely on real personal settings for testing, and do not claim success without running the build/tests and verifying file-safety invariants.

---

# Engineering review record (gstack /plan-eng-review, 2026-10-23)

Target: `/home/stefan/Downloads/ConfigDeck_AI_Agent_Build_Spec.md` (this file, sections 0-21 above, unchanged). Repo: altafino/jsonui, branch `dev`.
Everything below this line is review output. Where an approved amendment conflicts with sections 0-21, the amendment wins.

## Scope record

feature answers: D2 = "Poll open file" (replace `notify` watcher with 1s stat poll of the open document, hash on change, check other files on open/window focus, pre-save fingerprint recheck unchanged; `notify` deferred to P1); D3 = "Include in P0" (user-associated JSON Schema stays in Milestone 4 / A10; remove it from the §12 P1 list); D4 = "Ship both in P0" (font scaling + comfortable/compact density stay in P0 settings; remove "compact density mode" from the §12 P1 list); structure: A, "Original arrangement" (D5); accepted scope: spec §0-§21 as written, amended only by D2, D3, D4, keeping the §11/§15 single-crate layout (the `watcher` module implements polling per D2); pending remedies: R1, R2 (Scope Challenge), plus Section 1-4 findings.

Scope Challenge result: scope accepted as-is (D2 swaps mechanism, D3/D4 resolve contradictions toward inclusion).

## Decision ledger

### R1: Which `.claude/settings.local.json` counts as "Project local"
Finding: SC1, P1, confidence 8/10, spec line 404 (`| Local project | <project>/.claude/settings.local.json | Discover only in directories explicitly selected by user |`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal, spec §7 line 404 and acceptance test A25: look only in `<selected dir>/.claude/settings.local.json`.
Runtime evidence: Claude Code docs, "Settings files and precedence → Where Claude Code keeps the local file in a git repository" (fetched 2026-10-23): since v2.1.211 Claude Code reads and writes the local file at the git repository root, except outside a git repo, when the root is the home dir, on Windows, or when root/.git/.claude is not owned by the user. It still reads a legacy file left in the starting directory; where both set a key, the root value wins. The shared `.claude/settings.json` is read from the session's primary working directory.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R1 local-file resolution | selected dir only (spec:404) | follow Claude's rule: walk up to git root (look for `.git`, no git binary), offer `<root>/.claude/settings.local.json` as "Project local"; if `<selected>/.claude/settings.local.json` also exists, list it as "Project local (legacy location, lower precedence)"; apply the documented exceptions (no git, root = home, Windows, not owned by user → selected dir) | selected dir only, plus a static hint that Claude may use the repo root |
| Shared settings.json location | selected dir (spec:403) | unchanged | unchanged |
| R2 preset schema strictness | pending | pending | pending |
| D2-D5 | approved (see Scope record) | unchanged | unchanged |
Question D6:
D6 — Which settings.local.json should "Project local" point at?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §7 line 404 and acceptance test A25.
ELI10: The spec says the project-local file is `<folder you picked>/.claude/settings.local.json`. Since v2.1.211 Claude Code keeps that file at the git repository root instead (with a few exceptions such as no git repo or Windows), and still reads an older copy in the starting folder at lower priority. If someone picks a subfolder of a repo, ConfigDeck as specced edits a file Claude no longer uses, or misses the one it does.
Stakes if we pick wrong: users toggle a permission in ConfigDeck, save successfully, and Claude Code behaves exactly as before. That is a silent wrong-file edit, the failure the spec's §7 "show the exact file" rule is meant to prevent.
Recommendation: A because it matches Claude Code's documented behavior and shows both files when both exist, so nothing is hidden.
Completeness: A=9/10, B=5/10
Pros / cons:
A) Follow Claude's root rule, list legacy file too (recommended)
  ✅ The file labeled "Project local" is the one Claude Code actually reads and writes
  ✅ A legacy copy in the subfolder is visible with a lower-precedence label instead of silently ignored
  ❌ Adds git-root discovery, the documented exceptions and a nested-dir fixture (human: ~4h / CC: ~20min)
B) Keep selected-folder-only, add a hint
  ✅ No new discovery logic; matches spec text exactly
  ✅ Users who always pick the repo root see no difference
  ❌ Picking a subfolder edits a file Claude may ignore; the hint is easy to miss
Net: about 20 minutes of discovery logic versus a silent wrong-file edit for anyone who picks a subfolder.
Header: Local file
Options:
A) Follow Claude's rule (recommended)
Walk up to the git root, label `<root>/.claude/settings.local.json` "Project local", also list an existing subfolder copy as "legacy, lower precedence", apply documented exceptions. Add nested-dir fixture to A25. human: ~4h / CC: ~20min
B) Selected folder + hint
Keep spec line 404 as written and show a static note that Claude Code may use the repository root.

State: approved
Actual answer: B) "Selected folder + hint" (D6 answer, 2026-10-23). User note verbatim: "we can go with 2, but idea is, that this GUI can manage any kind of json stype settings. even it would be good that its later also open for eg yaml ect. and it must manage different files (user can add and remove)".
Accepted scope: keep spec §7 line 404 (`<selected dir>/.claude/settings.local.json`) unchanged; in the Claude discovery view, show a static note that Claude Code v2.1.211+ may read the local file from the git repository root. No git-root discovery. Product direction recorded from the note: ConfigDeck is a generic, user-managed multi-file editor; Claude Code is one preset; later formats (e.g. YAML) must remain possible (spec P2 "separate document engines").
History: none

### R2: Do schema violations from a built-in preset block Save?
Finding: SC2, P2, confidence 7/10, spec line 389 (`App presets may suggest values but must not declare unknown values invalid unless an authoritative associated schema requires that constraint.`) vs line 592 (`Save is disabled when there are validation errors, read-only status, or external changes.`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal; spec §10 line 592 disables Save on any validation error; §6 line 389 says presets must not invalidate unknown values. The two conflict when a bundled schema rejects a value the tool now accepts.
Runtime evidence: Claude Code docs, "Edit a settings file" (fetched 2026-10-23): "The schema can lag behind the newest CLI releases, so a validation warning on a recently documented key doesn't mean your configuration is invalid." SchemaStore `claude-code-settings.json` probe: draft-07, 142 properties, `additionalProperties: true`.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R2 severity of built-in preset schema violations | blocks Save (spec:592) | warning on the field + listed in Review changes; Save allowed after an explicit "Save with N warnings" confirmation | blocks Save (spec:592 literal) |
| User-associated schema violations (A10) | blocks Save | unchanged: blocks Save | unchanged: blocks Save |
| Syntax / duplicate-key / JSON type errors | block Save | unchanged | unchanged |
| R1 | approved B (D6) | unchanged | unchanged |
| D2-D5 | approved | unchanged | unchanged |
Question D7:
D7 — Should a built-in preset's schema be able to block Save?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §6 line 389 vs §10 line 592.
ELI10: Presets bundle someone else's schema (for Claude Code, the SchemaStore schema, which Claude's own docs say lags behind new releases). The spec says presets must not mark unknown values invalid, but also says any validation error disables Save. When the tool adds a new allowed value, the bundled schema calls it an error and the user cannot save a perfectly valid file. Schemas the USER attaches are a different case: the user chose them, so blocking is right (test A10).
Stakes if we pick wrong: with B, every bundled preset can lock users out of saving correct settings until ConfigDeck ships an update, and since remote schema fetching is banned, there is no workaround.
Recommendation: A because it keeps user-chosen schemas strict (A10) while a lagging third-party schema can only warn, which is exactly how Claude Code itself treats its schema.
Completeness: A=9/10, B=6/10
Pros / cons:
A) Preset schemas warn, user schemas block (recommended)
  ✅ Valid new values from a newer tool version can still be saved, with a visible warning
  ✅ A10 unchanged: an associated restrictive schema still blocks Save
  ❌ Two severities to render and test, plus a confirm step in the save flow (human: ~3h / CC: ~15min)
B) Every schema error blocks Save
  ✅ One rule, simplest mental model and UI
  ✅ Matches spec line 592 literally
  ❌ A lagging bundled schema blocks saving valid files with no workaround offline
Net: one extra severity level versus users locked out by someone else's stale schema.
Header: Schema block
Options:
A) Preset warns, user blocks (recommended)
Built-in preset schema violations show as field warnings and in Review changes; Save needs an explicit "Save with N warnings" confirm. User-associated schema errors, syntax, duplicate-key and type errors still block. Amend §10 line 592 accordingly. human: ~3h / CC: ~15min
B) All schema errors block
Keep §10 line 592 literally: any schema violation, preset or user, disables Save.

State: approved
Actual answer: A) "Preset warns, user blocks (recommended)" (D7 answer, 2026-10-23)
Accepted scope: amend §10 line 592: built-in preset schema violations are field warnings, listed in Review changes; Save is allowed after an explicit "Save with N warnings" confirmation. User-associated schema violations (A10), syntax errors, duplicate keys and JSON type errors still disable Save. Tests: a preset-schema warning fixture that saves after confirmation, and A10 unchanged.
History: none

### R3: One parse (CST) as the single source of truth
Finding: A1, P1, confidence 9/10, spec line 88 (`| JSON document model | serde_json semantic model, plus a concrete syntax tree/token-aware editor |`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal, spec §2 line 88 and §9 lines 510-514 (`parsedSyntaxTree + projectedSemanticTree`), two models from two parsers.
Runtime evidence: scratchpad probe with jsonc-parser 0.34.0 + serde_json 1 (2026-10-23): `{"a": 1, "a": 2}` parses in the CST without error, `get("a")` returns `1`, 2 properties kept; `serde_json::from_str` returns `{"a":2}`. serde_json cannot parse JSONC comments at all. CST toggle/CRLF/append/roundtrip were byte-exact. Number lexeme handling in the frontend: JS `number` loses integers above 2^53 (language fact).
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R3 document model | CST + separate serde_json parse (spec:88) | parse once into the jsonc-parser CST; walk it to build the UI tree DTO, detect duplicate keys, and derive a `serde_json::Value` only as input to schema validation; scalars cross IPC as raw lexeme text plus kind, so numbers keep exact text (`1.50`, `1e3`, big ints) | keep both parsers; add a separate duplicate-key pre-scan and send numbers as strings |
| R1, R2 | approved | unchanged | unchanged |
| D2-D5 | approved | unchanged | unchanged |
Question D8:
D8 — Should the CST be the only parse of each document?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §2 line 88 and §9 lines 510-514.
ELI10: The spec parses every file twice: once into an editable syntax tree (for byte-exact saves) and once with serde_json (for the form). I probed both: for `{"a":1,"a":2}` the syntax tree edits the first `a` while serde_json shows the second, so the form can show one value and Save change another. serde_json also cannot read JSONC comments, and JavaScript cannot represent big integers exactly. Option A parses once and builds everything from that one tree.
Stakes if we pick wrong: with two parsers, any disagreement means the form shows one thing while Save writes another place, which is the worst failure this product can have.
Recommendation: A because one parser removes the whole class of display/edit disagreement, and it is less code than keeping two in sync.
Completeness: A=10/10, B=7/10
Pros / cons:
A) CST only; derive tree, duplicates and schema input from it (recommended)
  ✅ The form and the save path read the same nodes, so they cannot disagree
  ✅ Exact number text survives (1.50 stays 1.50, 2^60 stays exact) and JSONC needs no second parser
  ❌ Must write a CST walker for the tree DTO and the serde_json bridge (human: ~1 day / CC: ~30min)
B) Keep two parsers, patch the known gaps
  ✅ serde_json gives the semantic tree for free
  ✅ Closest to the spec text
  ❌ Each newly found disagreement needs its own patch; JSONC still needs comment stripping for serde_json
Net: one tree walker versus an open-ended list of mismatches between two parsers.
Header: Parse model
Options:
A) CST only (recommended)
Parse once with jsonc-parser CST (strict options for .json). Build the UI tree DTO, the duplicate-key check and the serde_json::Value for schema validation from it. Scalars cross IPC as raw lexeme + kind. Amend §2 line 88 and §9 line 512. human: ~1 day / CC: ~30min
B) Two parsers + patches
Keep the serde_json semantic model; add a duplicate-key pre-scan and string-encoded numbers to IPC.

State: approved
Actual answer: A) "CST only (recommended)" (D8 answer, 2026-10-23)
Accepted scope: amend §2 line 88 and §9 lines 510-514: parse each document once into the jsonc-parser CST (all `ParseOptions` false for `.json`). Build the UI tree DTO, the duplicate-key diagnostic and the `serde_json::Value` used for schema validation from that CST. Scalars cross IPC as raw lexeme text plus kind. No independent serde_json parse of the file. Tests: duplicate-key fixture shows a diagnostic and blocks save; number fixture (`1.50`, `1e3`, `9007199254740993`) round-trips its exact text through tree DTO and an unrelated edit.
History: none

### R4: Who owns the draft: Rust or React?
Finding: A2, P1, confidence 8/10, spec line 643 (`You may keep ephemeral draft edits entirely in React and send a complete sequence to Rust for preview/save, **provided** the Rust side validates and owns the authoritative base snapshot`) vs lines 629-631 (`apply_draft_operation(file_id, operation)` / `undo_draft_operation(file_id)` / `redo_draft_operation(file_id)`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal leaves both options open; spec line 643 itself asks to "Prefer sharing a well-tested patch implementation over duplicated semantics in Rust and TypeScript."
Runtime evidence: none (no code yet). R3 approved: the CST lives in Rust, so op semantics against it can only run in Rust.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R4 draft owner | either React or Rust (spec:643) | Rust: per open document holds original bytes + fingerprint, ordered op list and redo stack; `apply_draft_operation` returns the updated tree DTO + diagnostics; undo = drop last op and replay the rest from the original bytes (no inverse ops); React holds only view state (expanded nodes, focus, in-progress text input) | React: holds op list and undo stack, mirrors patch semantics in TS for instant display; Rust replays the list at preview/save |
| R3 | approved A (D8) | unchanged | unchanged |
| R1, R2, D2-D5 | approved | unchanged | unchanged |
Question D9:
D9 — Should Rust own the draft and undo history?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §11 line 643 vs commands at lines 629-631.
ELI10: The spec allows keeping unsaved edits either in the React UI or in Rust. If React keeps them, the rules for "set / add / remove / move" must be written twice, in TypeScript for the screen and in Rust for the file, and any difference means the preview lies. With D8 the editable tree lives in Rust, so A keeps the edits there too: React sends "set /debug to false", Rust applies it and returns the updated tree. Undo simply throws away the last edit and replays the others from the original file, so no tricky "reverse operation" code is needed.
Stakes if we pick wrong: with B, the screen and the saved bytes come from two implementations; a mismatch shows up as "the diff said X but the file got Y".
Recommendation: A because one implementation of edit semantics is the spec's own stated preference, and replay-based undo is the simplest correct undo.
Completeness: A=9/10, B=7/10
Pros / cons:
A) Rust owns draft; replay-based undo (recommended)
  ✅ One implementation of edit rules; preview, diff and save cannot disagree
  ✅ Undo/redo is a list pop plus replay; no inverse-operation code for array moves and removals
  ❌ Each edit is one IPC round trip (local, typically ~1ms) and replay cost grows with op count on large files [estimated, measure against the 100 ms target]
B) React owns draft; Rust replays at save
  ✅ Zero IPC per keystroke-level edit; UI is instant
  ✅ Draft survives a Rust command error without extra state handling
  ❌ Patch semantics written twice (TS + Rust) with drift risk; spec line 643 itself discourages this
Net: one IPC hop per edit versus two implementations of the edit rules.
Header: Draft owner
Options:
A) Rust owns draft (recommended)
Rust keeps original bytes, fingerprint, op list and redo stack per open document; apply/undo/redo return the updated tree DTO + diagnostics; undo replays remaining ops from original bytes. React keeps only view state. Remove the React-draft allowance at §11 line 643. human: ~1 day / CC: ~30min
B) React owns draft
React holds ops and undo stack with a TS mirror of patch semantics; Rust validates and replays the full list at preview/save.

State: approved
Actual answer: A) "Rust owns draft (recommended)" (D9 answer, 2026-10-23)
Accepted scope: amend §11 line 643: remove the React-only draft allowance. Rust holds, per open document, original bytes + fingerprint, the ordered op list and a redo stack. `apply_draft_operation` / `undo_draft_operation` / `redo_draft_operation` return the updated tree DTO + diagnostics. Undo drops the last op and replays the rest from the original bytes (no inverse ops). React keeps only view state. Tests: undo/redo property test (random op sequences: apply N, undo k, redo k equals apply N), and a timing check of a toggle on the 1 MB fixture against the 100 ms target.
History: none

### R5: Saving through symlinks and hard links
Finding: A3, P1, confidence 8/10, spec line 564 (`Write to a temporary file **in the same directory** as the source, ... and atomically replace destination using a platform-appropriate strategy.`) and line 573 (`Prefer **read-only or explicit confirmation** for symlinked paths; support actual resolved target only after validation.`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: symlinks read-only or confirmed; behavior of the atomic replace on a link or hard link unspecified.
Runtime evidence: POSIX `rename(2)` onto a symlink path replaces the link itself with the new regular file, and atomic replace creates a new inode, detaching other hard links (OS semantics, not probed here). GNU stow and similar dotfile tools commonly symlink files such as `~/.claude/settings.json`.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R5 symlink save | unspecified (spec:564,573) | after a one-time confirmation per registry entry, resolve the link; show link and target paths; temp file in the target's directory; rename onto the target (link stays a link); at save re-resolve and abort with conflict if the target changed | symlinked files open read-only in MVP |
| R5 hard link (nlink > 1) | unspecified | one-time confirmation explaining that saving detaches the other hard links | read-only in MVP |
| Permission bits on save | "never broaden" (spec:576) | unchanged: copy mode bits from target to temp before rename | unchanged |
| R3, R4, R1, R2, D2-D5 | approved | unchanged | unchanged |
Question D10:
D10 — How should Save handle symlinked and hard-linked files?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §10 lines 564 and 573.
ELI10: Safe saving writes a new temp file and swaps it in with a rename. If the path is a symlink (common when dotfile tools like GNU stow manage `~/.claude/settings.json`), that rename silently replaces the link with a plain file, so the user's dotfiles repo stops receiving changes. Hard links get detached the same way. The spec says "read-only or confirm" but never says what Save then does. A: confirm once, then save into the real target so the link survives. B: make such files read-only for now.
Stakes if we pick wrong: with no rule, the default implementation breaks the user's dotfile setup without any error, and they find out weeks later when their other machine has stale settings.
Recommendation: A because developers, the primary persona, often symlink config files, and making them read-only would lock them out of the main use case.
Completeness: A=9/10, B=7/10
Pros / cons:
A) Confirm once, write through to the real target (recommended)
  ✅ Symlinks stay symlinks; dotfile repos keep receiving the change
  ✅ Re-resolving at save catches a link that was repointed while the draft was open
  ❌ More path handling and tests: link fixture, retarget-during-draft, nlink>1 warning (human: ~1 day / CC: ~30min)
B) Read-only in MVP
  ✅ Zero risk of breaking links; trivial to implement and test
  ✅ Satisfies A21 ("explicit safe policy enforced") as written
  ❌ Users with stow/symlinked configs cannot edit them at all in MVP
Net: about a day of careful path code versus locking out symlinked-config users.
Header: Symlinks
Options:
A) Write through (recommended)
One-time confirmation per registry entry; show link + target; temp file in target's dir; rename onto target; re-resolve at save and abort on change; nlink>1 gets a detach warning; mode bits copied. human: ~1 day / CC: ~30min
B) Read-only in MVP
Symlinked or hard-linked (nlink>1) files open read-only with an explanation; writing deferred.

State: approved
Actual answer: A) "Write through (recommended)" (D10 answer, 2026-10-23)
Accepted scope: amend §10 lines 564/573: symlinked paths need a one-time confirmation per registry entry, shown with link and resolved target. Save writes the temp file in the target's directory and renames it onto the target, so the link stays a link. At save, the link is re-resolved, and the save aborts with `EXTERNAL_CHANGE_CONFLICT` if the target changed. Files with nlink>1 need a one-time confirmation that saving detaches the other hard links. Mode bits are copied from target to temp before the rename. Tests (Linux tempdir): symlink save keeps the link and changes the target; link retargeted during draft aborts; nlink>1 shows the warning; mode 0600 is preserved.
History: none

### R6: File paths come only from Rust-side pickers
Finding: A4, P1, confidence 8/10, spec line 620 (`add_managed_file(selected_path, optional_display_name)`), line 621 (`create_managed_file(selected_path)`), line 639 (`associate_schema(file_id, local_schema_path)`), versus line 126 (`No JavaScript-side unrestricted file reads/writes. The Rust service owns all edits.`) and line 128 (`Rust still validates every command against its managed-file registry, not just a client-provided path.`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: the native dialog returns a path to the webview, which passes it to `add_managed_file`; Rust then trusts that path to enter the registry.
Runtime evidence: none (no code). Design reasoning: the registry check is circular when the webview can register any path first. A webview compromise (e.g. via rendered file or schema content) could register and read `~/.claude.json` or other JSON files.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R6 path provenance | JS supplies paths (spec:620,621,639) | Rust commands open the native dialogs (`pick_and_add_managed_file`, `pick_and_create_managed_file`, `pick_schema(file_id)`, `pick_project_directory`) via tauri-plugin-dialog's Rust API; discovery returns candidates as Rust-issued IDs; drag-and-drop handled from the Rust window event; the webview capability gets no dialog/fs permission; no command accepts a raw path from JS | keep JS-supplied paths; Rust validates file type, size and extension before registering |
| R3, R4, R5, R1, R2, D2-D5 | approved | unchanged | unchanged |
Question D11:
D11 — Should JavaScript ever hand Rust a file path?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §11 commands at lines 620, 621, 639 vs security boundary at lines 126-128.
ELI10: The spec says Rust guards the filesystem and only touches registered files. But the way files get registered is that the UI (JavaScript) passes a path it got from the file dialog. So if the UI were ever tricked, for example by malicious text in a config file or schema, it could register `~/.claude.json` or any other JSON and read it back. A: Rust opens the dialog itself, so a path can only come from the user's actual click. B: keep the spec and validate the path's type and size.
Stakes if we pick wrong: with B, the "Rust owns the filesystem" boundary has a front door that any webview bug can walk through, and A20 ("Unauthorized/unregistered frontend path rejected") passes while the real hole stays open.
Recommendation: A because it makes the registry check meaningful at almost no cost; the dialog plugin has a Rust API.
Completeness: A=10/10, B=6/10
Pros / cons:
A) Rust-side pickers; no raw paths from JS (recommended)
  ✅ A path can only enter the registry through a real user dialog action or a Rust-discovered candidate
  ✅ Webview capability shrinks to plain IPC: no dialog or fs plugin permissions
  ❌ Commands block on a modal dialog from Rust, and tests need an injectable picker in the core (human: ~4h / CC: ~20min)
B) JS paths + Rust validation
  ✅ Matches the spec's command list exactly
  ✅ Simplest wiring; standard Tauri dialog usage from JS
  ❌ A webview compromise can register and read any JSON file the user can read
Net: a few hours of command rewiring versus a bypassable security boundary.
Header: Path source
Options:
A) Rust-side pickers (recommended)
Replace path-taking commands with pick_and_add_managed_file / pick_and_create_managed_file / pick_schema(file_id) / pick_project_directory; discovery returns candidate IDs; drag-and-drop via Rust window event; no dialog/fs permission in the webview capability. Add an A20 case: forged path string is impossible to submit. human: ~4h / CC: ~20min
B) JS paths + validation
Keep §11 commands; Rust validates regular file, size limit and .json/.jsonc extension before registering.

State: approved
Actual answer: A) "Rust-side pickers (recommended)" (D11 answer, 2026-10-23)
Accepted scope: amend §11 command surface. `add_managed_file(selected_path, …)`, `create_managed_file(selected_path)` and `associate_schema(file_id, local_schema_path)` become `pick_and_add_managed_file(optional_display_name)`, `pick_and_create_managed_file()` and `pick_schema(file_id)`. Add `pick_project_directory()` for discovery. All of these open the native dialog from Rust via tauri-plugin-dialog's Rust API, behind an injectable picker so tests can supply paths. Discovery returns Rust-issued candidate IDs (`add_discovered_file(candidate_id)`), and drag-and-drop is handled from the Rust window event. The webview capability gets no dialog or fs permission, and no command accepts a raw filesystem path from JS. A20 gains a case confirming that no command takes a path argument (command-signature test).
History: none

### R7: Keep IPC contracts format-neutral for future YAML/TOML
Finding: A5, P2, confidence 7/10, spec line 88 (`JSON document model`) and §12 P2 line 711 (`Format support for YAML/TOML via separate document engines.`), plus user direction recorded in R1 (D6 note: "later also open for eg yaml"), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: nothing constrains where jsonc-parser types appear; P2 promises separate engines later.
Runtime evidence: none (no code). R3 (CST only) and R4 (Rust-owned draft) approved; both already put a tree DTO and pointer-based ops at the IPC boundary.
Comparison grid:
| Choice | Current | A | B | C |
|---|---|---|---|---|
| R7 format boundary | unconstrained | format-neutral contracts, no abstraction: tree DTO (pointer, node kind, raw scalar text, children), RFC 6901 pointer ops, line/col diagnostics, fingerprints; jsonc-parser imported only under `core/document` (CI grep check); no `Format` trait until a second format is built; decision recorded in docs/decisions.md | introduce a `DocumentFormat` trait now with a JSON/JSONC implementation | no rule (spec as written) |
| R3, R4, R5, R6, R1, R2, D2-D5 | approved | unchanged | unchanged | unchanged |
Question D12:
D12 — How much should we prepare for YAML now?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §2 line 88 and P2 line 711; your D6 note that YAML etc. should be possible later.
ELI10: You want ConfigDeck to open other formats later. The cheapest way to keep that door open is to make sure the "language" between the UI and Rust (a tree of nodes, edits addressed by path, errors with line/column) never mentions JSON-parser types, and to keep the JSON parser inside one module. A does exactly that, with a CI check, and writes no extra code. B builds a plug-in interface for formats now, with only JSON behind it. C does nothing and risks parser types leaking everywhere.
Stakes if we pick wrong: with C, adding YAML later means rewriting commands and UI types; with B, we pay for an interface designed before we know what YAML's comment and anchor handling needs, and will likely redesign it anyway.
Recommendation: A because it keeps YAML cheap later at zero code cost now; an interface with one implementation is guesswork.
Completeness: A=9/10, B=9/10, C=5/10
Pros / cons:
A) Neutral contracts + one-module rule, no trait (recommended)
  ✅ UI and commands work unchanged when a YAML engine arrives; only `core/document` grows
  ✅ No speculative abstraction; one CI grep line enforces the boundary
  ❌ The YAML engine will still need its own CST-preserving editor and an interface designed then (human: ~1h / CC: ~5min now)
B) DocumentFormat trait now
  ✅ Plug-in point exists from day one
  ✅ Forces clean separation in code, not just by convention
  ❌ Designed with one implementation; YAML anchors, multi-docs and comments will likely force a redesign (human: ~1 day / CC: ~30min)
C) No rule
  ✅ Zero effort
  ✅ Spec unchanged
  ❌ jsonc-parser types can leak into commands and TS contracts, making YAML a rewrite
Net: one boundary rule now versus a guessed interface or a later rewrite.
Header: Formats
Options:
A) Neutral contracts (recommended)
Tree DTO, pointer ops, diagnostics and fingerprints stay format-neutral; jsonc-parser only under core/document with a CI grep; no Format trait until a second format; record in docs/decisions.md. human: ~1h / CC: ~5min
B) Format trait now
Define a DocumentFormat trait (parse, tree, apply op, serialize) with a JSON/JSONC implementation behind it. human: ~1 day / CC: ~30min
C) No rule
Leave spec as written; decide format boundaries when YAML is scheduled.

State: approved
Actual answer: A) "Neutral contracts (recommended)" (D12 answer, 2026-10-23)
Accepted scope: IPC and TS contracts stay format-neutral: tree DTO (pointer, node kind, raw scalar text, children), RFC 6901 pointer ops, line/col diagnostics and fingerprints. `jsonc_parser` may be imported only under `src-tauri/src/core/document/`, enforced by a CI grep step. No `Format` trait until a second format is implemented. Record this in docs/decisions.md.
History: none

### R8: Linux artifact and CI build for portability
Finding: A6, P2, confidence 7/10, spec line 928 (`Produce a **working Linux distributable** suitable for Manjaro; evaluate AppImage portability and dynamic runtime library limitations on real target systems.`) and line 912 (`.github/workflows/` with no stated contents), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: a Manjaro-usable artifact; portability evaluated; CI contents unspecified.
Runtime evidence: Tauri v2 docs, "AppImage → Limitations" (fetched 2026-10-23): "you must build your Tauri application using the oldest base system you intend to support … Ubuntu 22.04 and Debian 12 are suitable baseline examples … Building on a newer base system can raise the minimum glibc version required by your app … We recommend using a Docker container or GitHub Actions to build your Tauri application for Linux." Arch prerequisites (same docs): `webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool`.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R8 artifacts + CI | Manjaro-usable artifact; CI unspecified | P0: `pnpm tauri build` on Manjaro produces AppImage + .deb (documented as Manjaro/newer-glibc only). Milestone 6 adds a GitHub Actions workflow on ubuntu-22.04 that runs fmt/clippy/tests/vitest and builds the portable AppImage + .deb as artifacts. PKGBUILD stays later | P0: local Manjaro build only, documented; no CI workflow until a later milestone |
| R1-R7, D2-D5 | approved | unchanged | unchanged |
Question D13:
D13 — Add an ubuntu-22.04 CI build in Milestone 6?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §16 line 928 and the empty `.github/workflows/` in §15.
ELI10: Building on Manjaro produces an AppImage that works on Manjaro, but Tauri's own docs warn it needs Manjaro's very new glibc, so it fails on Ubuntu LTS, Debian and older Fedora. Tauri recommends building Linux releases on an old base like Ubuntu 22.04 in GitHub Actions. That same CI job can also run all the tests on every push, which the spec's "do not claim success without tests" rule benefits from.
Stakes if we pick wrong: with B, anyone outside Manjaro who downloads the AppImage gets `GLIBC_2.xx not found`, and tests only run when someone remembers to run them locally.
Recommendation: A because it is one workflow file that both proves portability and runs the safety tests on every push.
Completeness: A=9/10, B=6/10
Pros / cons:
A) Local Manjaro build + ubuntu-22.04 CI (recommended)
  ✅ Portable AppImage/.deb built where Tauri recommends; Manjaro build still works locally
  ✅ fmt, clippy, cargo test and vitest run on every push to dev, not just when remembered
  ❌ One more workflow to maintain; CI minutes on a private repo (human: ~4h / CC: ~20min)
B) Local Manjaro build only
  ✅ Nothing extra to maintain in P0
  ✅ Satisfies "Manjaro-usable artifact" literally
  ❌ AppImage is not portable, and tests have no automatic gate
Net: one workflow file versus a non-portable artifact and no automatic test gate.
Header: CI build
Options:
A) Add ubuntu-22.04 CI (recommended)
P0 local build on Manjaro (AppImage + .deb, glibc caveat in docs/packaging.md). Milestone 6: GitHub Actions on ubuntu-22.04 runs fmt/clippy/cargo test/vitest and uploads portable AppImage + .deb. PKGBUILD later. human: ~4h / CC: ~20min
B) Local build only
Manjaro build documented with the glibc caveat; no CI workflow in P0.

State: approved
Actual answer: A) "Add ubuntu-22.04 CI (recommended)" (D13 answer, 2026-10-23)
Accepted scope: P0 local `pnpm tauri build` on Manjaro produces AppImage + .deb; docs/packaging.md states the glibc caveat and the Arch prerequisite package list. Milestone 6 adds `.github/workflows/ci.yml` on ubuntu-22.04 that runs `cargo fmt --check`, `cargo clippy`, `cargo test`, `pnpm vitest run` and the R7 grep check, and uploads the portable AppImage + .deb as artifacts. PKGBUILD stays later.
History: none

### Correction C1 (no decision needed): add a key-rename operation
Finding: Q3, P3, confidence 9/10, spec lines 520-525 (`type JsonPatch = | { op: 'set' … } | { op: 'add' … } | { op: 'remove' … } | { op: 'array-insert' … } | { op: 'array-move' … }`) vs acceptance test A07 (`Add/rename/remove map property`). The op union cannot express A07's rename. This is necessary implementation of an approved acceptance test, so it is not a new choice.
Runtime evidence: scratchpad probe with jsonc-parser 0.34.0: replacing a property's `ObjectPropName::String` literal renames `"a"` to `"z"` in place, keeping member order, and keeps a comment inside an object-valued member.
Correction: add `{ op: 'rename-key'; path: string; newKey: string }`, implemented by replacing the name literal. It rejects a `newKey` that already exists in the object (`DUPLICATE_KEYS`). `set` on a missing member and `add` on an existing member return typed errors instead of silently switching behavior.

### R9: Drop Ajv; Rust is the only validator
Finding: Q1, P2, confidence 8/10, spec line 91 (`| Validation | JSON Schema where supplied + Rust authoritative checks | UI can use Ajv for fast inline feedback; Rust checks before commit |`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: optional Ajv in the UI plus Rust validation before commit.
Runtime evidence: none (no code). R4 approved: every draft op is applied in Rust and returns the updated tree + diagnostics, so Rust already produces inline feedback per edit. R2 approved: preset-schema results are warnings, user-schema results are errors, a severity split Ajv would have to replicate.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R9 UI validator | optional Ajv in UI (spec:91) | no Ajv; field diagnostics come only from Rust in the `apply_draft_operation` response (text inputs debounced ~150 ms before sending) | Ajv in UI for instant feedback; Rust result remains authoritative and wins on disagreement |
| R2, R3, R4 and other approved rows | approved | unchanged | unchanged |
Question D14:
D14 — Keep Ajv in the UI, or use only Rust's validation?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §2 line 91.
ELI10: The spec allows a second JSON Schema validator (Ajv) in the UI for instant red underlines. Since D9, every edit already goes to Rust and comes back with the updated tree and its errors, in about a millisecond locally. Two validators means two sets of rules: different JSON Schema draft support, different error wording, and the preset-warning versus user-error split from D7 would have to be built twice. When they disagree, the user sees an error under the field but Save is enabled, or the reverse.
Stakes if we pick wrong: with B, validation messages in the form can contradict what Save does, which undermines trust in the one screen that is supposed to say "this is safe".
Recommendation: A because Rust already returns diagnostics on every edit, so Ajv adds a second source of truth and a dependency without a visible speed gain.
Completeness: A=9/10, B=8/10
Pros / cons:
A) Rust-only validation (recommended)
  ✅ One validator, one set of messages; the form and Save always agree
  ✅ Drops the Ajv dependency and its draft/format config from the bundle
  ❌ Text fields show errors after a short debounce instead of on every keystroke (human: ~0 / CC: ~0 extra)
B) Ajv in UI, Rust authoritative
  ✅ Keystroke-instant feedback without IPC
  ✅ Matches spec line 91
  ❌ Two engines can disagree; D7's severity split must be implemented twice (human: ~1 day / CC: ~30min)
Net: a ~150 ms debounce on text fields versus two validators that can contradict each other.
Header: Validator
Options:
A) Rust only (recommended)
No Ajv. Field diagnostics come from Rust in each apply_draft_operation response; text inputs debounce ~150 ms before sending. Amend §2 line 91.
B) Ajv + Rust
Ajv in the UI mirrors schema checks for instant feedback (including D7 severities); Rust remains authoritative at preview/save.

State: approved
Actual answer: A) "Rust only (recommended)" (D14 answer, 2026-10-23)
Accepted scope: amend §2 line 91: no Ajv. Field diagnostics come only from Rust in each `apply_draft_operation` response, and text inputs debounce ~150 ms before sending. Rust validates with the `jsonschema` crate (draft-07 through 2020-12).
History: none

### R10: UTF-8 byte order mark (BOM)
Finding: Q2, P2, confidence 9/10, spec line 502 (`Preserve UTF-8 content, ordering, indentation, line endings (LF/CRLF), end-of-file newline, comments/trivia in JSONC, and existing formatting outside the modified subtree`), with no BOM rule anywhere in the spec, reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: no BOM handling specified.
Runtime evidence: scratchpad probe with jsonc-parser 0.34.0: `CstRootNode::parse("\u{FEFF}{\n  \"a\": true\n}\n", …)` returns `Unexpected token on line 1 column 1`. RFC 8259 §8.1: implementations must not add a BOM and may ignore one when parsing (standard text, not probed).
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R10 BOM policy | unspecified; parser error "Unexpected token line 1 col 1" | strip a leading UTF-8 BOM before parsing, record `hasBom`, re-prepend it to candidate bytes on save so round-trip stays byte-exact; show a small "UTF-8 with BOM" note in file details | treat a BOM as `UNSUPPORTED_ENCODING` with a clear message; file opens read-only |
| R3 (CST only), byte-fidelity A13 | approved | unchanged | unchanged |
Question D15:
D15 — What should happen with a file that starts with a UTF-8 BOM?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §9 line 502.
ELI10: Some Windows editors put an invisible marker (BOM) at the very start of a text file. I tested the parser we chose: it rejects such files with "Unexpected token on line 1 column 1", which the user cannot see or fix. A: quietly remove the marker before parsing, remember it was there, and put it back when saving, so the file stays byte-identical. B: refuse the file with a clear "this file has a BOM" message.
Stakes if we pick wrong: with no rule, valid-looking files from Windows tools show a baffling syntax error at line 1, and once Windows support arrives this becomes a common support issue.
Recommendation: A because it keeps the "open and save unchanged = no-op" promise (A13) and costs a few lines.
Completeness: A=10/10, B=7/10
Pros / cons:
A) Strip on read, restore on save (recommended)
  ✅ BOM files open and save normally; bytes stay identical, including the marker
  ✅ A few lines plus one fixture; no user action needed
  ❌ ConfigDeck tolerates input that strict RFC 8259 tools may reject (they may ignore it, per the RFC) (human: ~1h / CC: ~5min)
B) Reject with a clear message
  ✅ Strict and simple; nothing silently tolerated
  ✅ Message is actionable ("re-save as UTF-8 without BOM")
  ❌ Users cannot edit those files in ConfigDeck at all
Net: a few lines of handling versus locking out files from common Windows editors.
Header: BOM
Options:
A) Strip and restore (recommended)
Strip a leading EF BB BF before parsing, keep a hasBom flag, prepend it to candidate bytes on save, show "UTF-8 with BOM" in file details. Add a BOM fixture to the byte-fidelity tests. human: ~1h / CC: ~5min
B) Reject as unsupported
Report UNSUPPORTED_ENCODING with "file starts with a UTF-8 BOM" and open read-only.

State: approved
Actual answer: A) "Strip and restore (recommended)" (D15 answer, 2026-10-23)
Accepted scope: amend §9 line 502: strip a leading EF BB BF before parsing and keep a `hasBom` flag. Prepend the BOM to candidate bytes on save, and show "UTF-8 with BOM" in file details. Add a BOM fixture to the byte-fidelity tests: open + save unchanged is a no-op, and a toggle changes only the token while keeping the BOM.
History: none

### R11: End-to-end test stack
Finding: T1, P1, confidence 8/10, spec line 93 (`| End-to-end tests | Playwright for browser-level UI flows; Tauri/native integration where feasible | Use real Rust backend integration tests for file I/O |`), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: Playwright for UI flows; native integration "where feasible"; Rust integration tests for file I/O.
Runtime evidence: Tauri v2 docs "WebDriver" (fetched 2026-10-23): "The recommended way to use it with Tauri is WebdriverIO and the `@wdio/tauri-service`, which works on Windows, Linux, and macOS … By default the service runs an embedded WebDriver server inside your app." Tauri "Mock Tauri APIs" docs: `mockIPC` from `@tauri-apps/api/mocks` for Vitest. Playwright has no Tauri/WebKitGTK webview driver, so Playwright here can only target the Vite dev server with mocked IPC.
Comparison grid:
| Choice | Current | A | B | C |
|---|---|---|---|---|
| R11 E2E stack | Playwright (mocked IPC only, in practice) | drop Playwright. Real-app E2E with WebdriverIO + `@wdio/tauri-service` for 4 journeys (A01 add+form, toggle→review→save→backup→restore, A14 conflict, A27 keyboard-only), using the R6 injectable picker and tempdir fixtures, run in CI under xvfb; Vitest + Testing Library + `mockIPC` for component states | keep Playwright against Vite + mockIPC; no real-app E2E; manual smoke on Manjaro | Playwright (mocked) and WebdriverIO (real app) both |
| Rust integration tests for file safety | approved (spec:93) | unchanged | unchanged | unchanged |
| R8 CI on ubuntu-22.04 | approved | runs the WDIO suite too | unchanged | runs both |
| Other approved rows | approved | unchanged | unchanged | unchanged |
Question D16:
D16 — Which tool runs the end-to-end tests?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §2 line 93.
ELI10: The spec names Playwright for end-to-end tests, but Playwright cannot drive a Tauri window. It can only test the UI in a normal browser with the Rust side faked. So no test would ever click through the real app and check the real file on disk. Tauri's own docs recommend WebdriverIO with its Tauri service, which drives the actual app on Linux. A: use that for a handful of key journeys and drop Playwright. B: keep Playwright with faked Rust and smoke-test by hand. C: run both.
Stakes if we pick wrong: with B, a bug in the wiring between UI and Rust (wrong command, wrong pointer, stale fingerprint) passes every automated test and only shows up on the user's real settings file.
Recommendation: A because the safety story ends at real bytes on disk, and only a real-app test proves the UI drives that path correctly; one E2E framework is enough.
Completeness: A=9/10, B=6/10, C=9/10
Pros / cons:
A) WebdriverIO real-app E2E, drop Playwright (recommended)
  ✅ Four journeys exercise UI → IPC → Rust → disk exactly as a user would
  ✅ One E2E framework instead of two; Tauri's documented path
  ❌ Needs a built binary and xvfb in CI; slower than browser tests (human: ~1-2 days / CC: ~1h)
B) Playwright with mocked IPC only
  ✅ Fast, no binary needed
  ✅ Matches spec line 93 as written
  ❌ No automated test ever runs the real app; wiring bugs reach users
C) Both
  ✅ Broad, fast mocked UI coverage plus real-app journeys
  ✅ Playwright's tooling for visual screenshots stays available
  ❌ Two E2E frameworks to maintain, and Vitest + mockIPC already covers mocked UI states (human: ~2-3 days / CC: ~1.5h)
Net: one real-app framework versus fast tests that never touch the real file path.
Header: E2E stack
Options:
A) WebdriverIO real app (recommended)
Drop Playwright. WebdriverIO + @wdio/tauri-service drives the built app for 4 journeys (A01, save/backup/restore, A14 conflict, A27 keyboard) with injected picker + tempdir fixtures, in CI under xvfb. Vitest + Testing Library + mockIPC for component states. human: ~1-2 days / CC: ~1h
B) Playwright mocked only
Keep spec line 93: Playwright vs Vite dev server with mockIPC; real app only smoke-tested by hand on Manjaro.
C) Both frameworks
Playwright (mocked IPC) for broad UI flows plus WebdriverIO real-app journeys. human: ~2-3 days / CC: ~1.5h

State: approved
Actual answer: C) "Both frameworks" (D16 answer, 2026-10-23)
Accepted scope: amend §2 line 93. Playwright runs against the Vite dev server with `mockIPC` for broad UI flows and screenshots. WebdriverIO + `@wdio/tauri-service` drives the built app for 4 real-app journeys: A01 add+form; toggle→review→save→backup→restore; A14 conflict; A27 keyboard-only. These use the R6 injectable picker and tempdir fixtures. Vitest + Testing Library + `mockIPC` cover component states. The R8 CI workflow runs Rust tests, Vitest, Playwright and WebdriverIO (under xvfb).
History: none

### R12: ConfigDeck's own registry file is unreadable at startup
Finding: T2, P2, confidence 7/10, spec lines 487-489 (`Store registry and app preferences in the OS-appropriate per-user application configuration directory … Store backup data under the OS-appropriate per-user application data directory.`), with no rule for a corrupt or unreadable registry, reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: registry persisted as app-owned JSON; failure behavior unspecified.
Runtime evidence: none (no code). Failure mode: a crash or full disk while writing the registry leaves it truncated; a naive `serde_json::from_reader(...)?` at startup then fails app launch, and the user loses every registration (the user's config files themselves stay untouched).
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R12 registry write | unspecified | registry and preferences written with the same temp + fsync + rename path as documents, keeping the previous version as `registry.json.bak` | unspecified write; no .bak |
| R12 unreadable registry at startup | unspecified (likely launch failure) | move the bad file aside as `registry.corrupt-<timestamp>.json`, load `.bak` if valid, otherwise start empty; show a non-blocking notice naming the saved-aside file | show a blocking error screen with "Reset registry" (moves the file aside, starts empty) |
| User config files | never touched (spec:489) | unchanged | unchanged |
| Other approved rows | approved | unchanged | unchanged |
Question D17:
D17 — What happens if ConfigDeck's own file list is corrupt at startup?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §8 lines 487-489.
ELI10: ConfigDeck stores its list of managed files in its own small JSON file. If that file is cut short (crash, power loss, full disk while saving it), the spec does not say what happens, and the default code would simply fail to start. A: save it the same safe way as user files and keep the previous copy; if it is broken at startup, set it aside, fall back to the previous copy, and tell the user. B: show an error screen with a "Reset" button.
Stakes if we pick wrong: with no rule, a single bad shutdown turns into "the app won't open" and the user has to find and delete a hidden file by hand.
Recommendation: A because it recovers automatically from the most likely cause with the previous copy, loses nothing (bad file kept), and reuses the save path that is already being built.
Completeness: A=9/10, B=7/10
Pros / cons:
A) Atomic write + .bak + automatic fallback (recommended)
  ✅ The app always starts; the most recent good file list is restored automatically
  ✅ The broken file is kept aside for inspection; nothing is deleted
  ❌ One more startup branch and two tests: truncated registry and missing .bak (human: ~3h / CC: ~15min)
B) Blocking error screen + Reset
  ✅ User sees exactly what happened and decides
  ✅ Simpler startup logic
  ❌ Reset loses the whole file list even when a good previous copy could have been used
Net: automatic recovery from the previous copy versus a manual reset that throws the list away.
Header: Registry
Options:
A) Atomic + .bak fallback (recommended)
Write registry/preferences via the temp+fsync+rename path keeping registry.json.bak; on unreadable registry move it to registry.corrupt-<ts>.json, load .bak or start empty, show a notice. Tests: truncated registry, missing .bak. human: ~3h / CC: ~15min
B) Error screen + Reset
On unreadable registry, show a blocking screen explaining it; "Reset registry" moves the file aside and starts empty.

State: approved
Actual answer: A) "Atomic + .bak fallback (recommended)" (D17 answer, 2026-10-23)
Accepted scope: registry and preferences are written via the same temp + fsync + rename path as documents, keeping the previous version as `registry.json.bak`. On an unreadable registry at startup, move it to `registry.corrupt-<timestamp>.json`, load `.bak` if valid, otherwise start empty, and show a non-blocking notice naming the saved-aside file. Tests: truncated registry recovers from .bak; truncated registry with no .bak starts empty with notice; user config files untouched.
History: none

### R13: Per-edit response size and lazy tree loading
Finding: Perf1, P1, confidence 8/10, spec line 627 (`get_document_tree(file_id)`) and line 330 (`Support typical 10 KB–1 MB config files comfortably; … Default maximum editable size may be **5 MiB**`), with R4's accepted scope "`apply_draft_operation` … return the updated tree DTO + diagnostics", reviewer: Claude (plan-eng-review)
Plan baseline: R4 approved (D9): Rust owns the draft and apply/undo/redo return the updated tree DTO + diagnostics. Its response granularity (whole tree vs changed part) was not specified. This record refines that response shape; R4's ownership decision is unchanged.
Runtime evidence: scratchpad benchmark, jsonc-parser 0.34.0, release build, this machine (2026-10-23): 0.15 MiB parse 5.7 ms; 1.56 MiB parse 56.3 ms, to_string 9.0 ms, serde_value 11.3 ms; 3.94 MiB parse 119.0 ms, in-place edit 5.4 µs, to_string 22.8 ms, serde_value 27.8 ms, value JSON 3298 KiB [measured]. IPC and React reconcile cost for a 3.3 MB payload: unknown.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R13 tree delivery + edit response | whole tree per open and per edit (spec:627 + R4 wording) | ops apply in place to the live CST (no re-parse); `apply/undo/redo` return only the changed node(s) by pointer + changed-count + syntax/type diagnostics; the tree loads lazily via `get_subtree(file_id, pointer, depth=1)` for expanded branches only; undo still replays from original bytes (~36 ms/MiB [measured]) | keep whole-tree responses; lower the default editable cap from 5 MiB to 1 MiB |
| R14 schema validation timing | pending | pending | pending |
| R4 Rust-owned draft | approved (D9) | unchanged ownership; response shape refined | unchanged |
| Other approved rows | approved | unchanged | unchanged |
Question D18:
D18 — Send only what changed per edit, or the whole tree?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §11 line 627 and §5 line 330; refines the response shape of D9.
ELI10: I measured the parser on this machine. Changing a value inside the already-parsed tree takes about 5 microseconds. But turning a 4 MB file into a tree for the UI takes about 28 ms, and that tree is 3.3 MB of data that then has to cross into the UI and be re-rendered. If every checkbox click re-sends the whole tree, big files feel sluggish. A: send only the changed piece, and load parts of the tree only when the user expands them (the spec already says "render only expanded branches"). B: keep it simple and shrink the maximum file size to 1 MB.
Stakes if we pick wrong: with B, files between 1 and 5 MB that the spec promises to support become uneditable; with no rule, toggles on large files miss the 100 ms target.
Recommendation: A because edits are already microseconds, so the only real cost is shipping data the user is not looking at, and lazy loading is what the spec's "render only expanded branches" asks for.
Completeness: A=9/10, B=6/10
Pros / cons:
A) Changed-node responses + lazy subtree loading (recommended)
  ✅ Toggle cost stays flat regardless of file size; the 5 MiB cap stays
  ✅ Matches spec §5 "render only expanded object/array branches"
  ❌ Frontend keeps a pointer-keyed node cache and must invalidate on undo/reload (human: ~1-2 days / CC: ~1h)
B) Whole tree per edit, cap at 1 MiB
  ✅ Simplest frontend state: every response replaces the tree
  ✅ Fine for typical config files (tens of KB)
  ❌ Files from 1 to 5 MiB, which the spec targets, are refused; large arrays still re-render fully
Net: a node cache in the UI versus a 5x smaller file limit.
Header: Edit payload
Options:
A) Delta + lazy tree (recommended)
Ops apply in place to the live CST; apply/undo/redo return changed node(s) by pointer + diagnostics; tree loads lazily with get_subtree(file_id, pointer, depth=1) for expanded branches; undo replays from original bytes (~36 ms/MiB). Test: 4 MiB fixture toggle under 100 ms end to end. human: ~1-2 days / CC: ~1h
B) Whole tree, 1 MiB cap
Every response returns the whole tree DTO; default editable size limit lowered from 5 MiB to 1 MiB (amend §5 line 330).

State: approved
Actual answer: A) "Delta + lazy tree (recommended)" (D18 answer, 2026-10-23)
Accepted scope: ops apply in place to the live CST without re-parsing. `apply_draft_operation` / `undo_draft_operation` / `redo_draft_operation` return only the changed node(s) by pointer, the changed-count and syntax/type diagnostics. The tree loads lazily via `get_subtree(file_id, pointer, depth=1)` for expanded branches, replacing a whole-tree `get_document_tree`. The frontend keeps a pointer-keyed node cache that is invalidated on undo/redo/reload. Undo still replays from original bytes (~36 ms/MiB [measured]; documented in known-limitations.md). The 5 MiB cap stays. Test: toggle on a 4 MiB fixture completes end to end in under 100 ms.
History: none

### R14: When schema validation runs
Finding: Perf2, P2, confidence 6/10, spec line 629 (`apply_draft_operation(file_id, operation)`) and §10 step 6 line 560 (`Parse candidate again in correct strict-JSON/JSONC mode and validate schema when one is active.`), with R9's accepted scope "field diagnostics come only from Rust in each `apply_draft_operation` response", reviewer: Claude (plan-eng-review)
Plan baseline: R9 approved (D14): Rust is the only validator and diagnostics arrive with each apply response. Timing of schema validation within that response was not specified. This refines R9's delivery; R9's single-validator decision is unchanged.
Runtime evidence: scratchpad benchmark (2026-10-23): building `serde_json::Value` from a 3.94 MiB document takes 27.8 ms [measured]; `jsonschema` validation time on such a document: unknown.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R14 schema validation timing | in each apply response (R9 wording) | syntax/type/duplicate diagnostics stay in the apply response; schema validation runs on a background thread after a 150 ms debounce and arrives via a `draft-diagnostics` event tagged with the draft revision (stale results dropped); Save step 6 always runs a final synchronous validation | validate schema synchronously inside every apply response |
| R9 Rust-only validator | approved (D14) | unchanged; delivery refined | unchanged |
| R13 delta responses | approved (D18) | unchanged | unchanged |
| Other approved rows | approved | unchanged | unchanged |
Question D19:
D19 — Should schema validation run in the background after edits?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §11 line 629 and §10 line 560; refines how D14's Rust-only validation is delivered.
ELI10: After D18 an edit costs microseconds, but checking the whole document against a JSON Schema means first converting it (about 28 ms for a 4 MB file, measured) and then validating (time unknown). Doing that inside every edit response puts it on the click path. A: return the edit immediately with basic errors, validate against the schema shortly after in the background, and push the results to the UI; Save always re-validates synchronously, so nothing unsafe gets through. B: validate inside every edit for simplicity.
Stakes if we pick wrong: with B, large files with a schema lose the fast toggle that D18 just bought; with A, schema warnings appear ~150 ms after the click instead of instantly.
Recommendation: A because it keeps edits fast for every file size and Save still has the final, authoritative check.
Completeness: A=9/10, B=8/10
Pros / cons:
A) Debounced background validation + final check at Save (recommended)
  ✅ Edit latency independent of schema size and document size
  ✅ Save step 6 still validates synchronously, so safety is unchanged
  ❌ Needs a revision counter so late results for an older draft are ignored (human: ~4h / CC: ~20min)
B) Synchronous validation per edit
  ✅ Errors are always exactly in sync with the response
  ✅ No event plumbing or revision tracking
  ❌ Schema-backed large files pay conversion + validation on every click
Net: a revision counter and one event versus validation cost on every click.
Header: Validation
Options:
A) Background + final check (recommended)
Apply responses carry syntax/type/duplicate diagnostics only; schema validation runs on a worker after 150 ms debounce and emits draft-diagnostics {fileId, revision, diagnostics}; stale revisions dropped; Save step 6 validates synchronously. Test: rapid edits never show diagnostics from an older revision. human: ~4h / CC: ~20min
B) Synchronous per edit
Every apply response includes full schema validation results.

State: approved
Actual answer: A) "Background + final check (recommended)" (D19 answer, 2026-10-23)
Accepted scope: apply/undo/redo responses carry only syntax, type and duplicate-key diagnostics. Schema validation runs on a worker thread after a 150 ms debounce and emits `draft-diagnostics { fileId, revision, diagnostics }`, and results for stale revisions are dropped. Save step 6 always validates synchronously before writing. Test: rapid edits never display diagnostics from an older revision; save with a pending background validation still runs the synchronous check.
History: none

### R15 (TODO choice): Leftover temp files after a crash mid-save
Finding: FM1, P2, confidence 8/10, spec line 564 (`Write to a temporary file **in the same directory** as the source`) and A19 (`Atomic replacement fails | Error shown; source not truncated/corrupted`). A19 covers the error path only; a crash or power loss between temp write and rename leaves a temp file beside the user's config with no test, no handling and no message (critical gap per Failure modes), reviewer: Claude (plan-eng-review)
Plan baseline: original proposal: no crash-leftover handling.
Runtime evidence: none (no code). OS fact: a process killed after `write+fsync` of the temp file and before `rename` leaves the temp file on disk.
TODO details: What: name temp files `.<basename>.configdeck-<random>.tmp`. When a managed file is opened, list matching leftovers in its directory and offer "Remove leftover temp file" (never deleted without the user's click). Why: prevents silent clutter in `~/.claude/` and project `.claude/` dirs. Pros: closes the only silent failure mode; ~15 min CC. Cons: touching files in user dirs, limited to our exact name pattern. Context: persistence module, save step 10. Depends on: T3 save pipeline.
Comparison grid:
| Choice | Current | A | B | C |
|---|---|---|---|---|
| R15 crash leftover handling | none | add to TODOS.md for P1 | skip (accept clutter) | build now in P0: unique temp-name pattern; on open, detect leftovers matching that exact pattern in the file's directory and offer one-click removal in the file details; test with a planted leftover |
| Other approved rows | approved | unchanged | unchanged | unchanged |
Question D20:
D20 — Handle temp files left behind by a crash during save?
Project/branch/task: altafino/jsonui `dev`, ConfigDeck spec §10 line 564 and A19.
ELI10: Safe saving writes a temporary file next to the real one, then swaps it in. If ConfigDeck crashes or the power goes out between those two steps, the user's real file is fine, but a stray temp file stays in their folder (for example `~/.claude/`) forever, and nothing tells them. C: give temp files a recognizable name and, next time that file is opened, offer to remove the leftover. A: write it down as a TODO for later. B: ignore it.
Stakes if we pick wrong: no data is lost either way; the cost is unexplained junk files in config folders, which is exactly where users notice and worry.
Recommendation: C because it is about 15 minutes of work and closes the only silent failure mode found in this review.
Completeness: A=7/10, B=5/10, C=10/10
Pros / cons:
A) Add to TODOS.md (P1)
  ✅ Keeps P0 scope unchanged
  ✅ Problem is documented so it is not forgotten
  ❌ Leftovers from P0 users accumulate until P1 ships
B) Skip
  ✅ Zero effort
  ✅ No code touches files in user directories
  ❌ Silent clutter in config folders, with no explanation
C) Build now in P0 (recommended)
  ✅ Closes the only silent failure mode; users see and control cleanup
  ✅ Only files matching ConfigDeck's exact temp-name pattern are ever offered for removal
  ❌ One more UI affordance and test in Milestone 4 (human: ~2h / CC: ~15min)
Net: 15 minutes now versus silent leftovers in users' config folders.
Header: Temp files
Options:
A) Add to TODOS.md
Record the TODO (What/Why/Context above) for P1; no P0 change.
B) Skip
Accept possible leftover temp files after crashes; no TODO.
C) Build now (recommended)
P0: temp files named .<basename>.configdeck-<random>.tmp; on open, detect exact-pattern leftovers in that directory and offer one-click removal in file details; test with a planted leftover. human: ~2h / CC: ~15min

State: approved
Actual answer: C) "Build now (recommended)" (D20 answer, 2026-10-23)
Accepted scope: in P0 (Milestone 4), temp files are named `.<basename>.configdeck-<random>.tmp`. When a managed file is opened, leftovers matching that exact pattern in its directory are listed in file details with one-click removal; nothing is deleted without the user's click. Test: a planted leftover is detected and offered, and unrelated dotfiles in the directory are never listed.
History: none

Approval readiness: PASS. Checked R1 (D6 B), R2 (D7 A), R3 (D8 A), R4 (D9 A), R5 (D10 A), R6 (D11 A), R7 (D12 A), R8 (D13 A), R9 (D14 A), R10 (D15 A), R11 (D16 C), R12 (D17 A), R13 (D18 A), R14 (D19 A), R15 (D20 C); scope answers D2 A, D3 A, D4 B, D5 A; correction C1 is necessary implementation of spec A07. No pending choices.

## Approved amendments to sections 0-21 (working plan for the coding agent)

Apply these on top of the spec. Each line cites its ledger record.

1. **File change detection (D2):** no `notify` watcher in P0. The `watcher` module polls the open document every 1 s (size, mtime, inode; hash only when those change) and checks other files on open or window focus. The pre-save fingerprint recheck (§10 step 3/9) is unchanged. `notify` moves to P1.
2. **Scope fixes (D3, D4):** user-associated schema stays in P0 (Milestone 4, A10); remove it from the §12 P1 list. Font scaling + comfortable/compact density stay in P0 settings; remove "compact density mode" from §12 P1.
3. **Layout (D5):** keep the §11/§15 single-crate layout.
4. **Claude local file (R1):** keep `<selected dir>/.claude/settings.local.json`; show a static hint that Claude Code v2.1.211+ may use the git repo root.
5. **Preset schemas warn, user schemas block (R2):** amend §10 line 592 as recorded.
6. **One parse (R3):** jsonc-parser CST is the only parse; tree DTO, duplicate-key check and the schema `serde_json::Value` all derive from it; scalars cross IPC as raw lexeme + kind.
7. **Rust owns the draft (R4):** op list + redo stack in Rust; undo = replay from original bytes.
8. **Symlinks/hard links (R5):** one-time confirmation, write through to the resolved target, re-resolve at save, nlink>1 warning, copy mode bits.
9. **No paths from JS (R6):** Rust-side pickers (`pick_and_add_managed_file`, `pick_and_create_managed_file`, `pick_schema`, `pick_project_directory`), Rust-issued discovery candidate IDs, no dialog/fs permission in the webview.
10. **Format-neutral contracts (R7):** jsonc-parser only under `core/document` (CI grep); no Format trait yet.
11. **Distribution + CI (R8):** local Manjaro AppImage/.deb in P0; ubuntu-22.04 GitHub Actions in Milestone 6.
12. **Rust-only validation (R9), background schema validation with revision tags and synchronous check at Save (R14).**
13. **Op set (C1):** add `rename-key`; `set` on missing / `add` on existing are typed errors.
14. **BOM (R10):** strip on read, restore on save.
15. **Tests (R11):** Playwright (mocked IPC) + WebdriverIO real-app journeys + Vitest/mockIPC + Rust integration tests.
16. **Registry durability (R12):** atomic write + `.bak`; corrupt registry is set aside and recovered.
17. **Edit payload (R13):** in-place CST edits, changed-node responses, lazy `get_subtree(pointer, depth=1)`, 5 MiB cap stays.
18. **Crash leftovers (R15):** `.<basename>.configdeck-<random>.tmp` naming, offered for removal on open.

## NOT in scope
- `notify`-based file watching: deferred to P1 by D2; polling covers A14/A15.
- Git-root discovery for Claude's local settings file: declined in D6 (hint only).
- `DocumentFormat` trait / YAML engine: deferred by D12 until a second format is built (P2).
- PKGBUILD / AUR recipe: spec §16 says "later"; R8 keeps it later.
- Field-level merge on external-change conflicts: spec §3 D allows deferral. This is the most likely friction point, because Claude Code writes `settings.local.json` mid-session.
- `managed-settings.d/` drop-in files: the docs confirm they exist; the spec only lists `managed-settings.json`. Listing them read-only would be a small follow-up and was not asked.
- Separate Tauri-free core crate: declined in D5.

## What already exists
- Repository: only `README.md` at commit 45819f3 on `main` and `dev`; nothing to reuse.
- Libraries verified on 2026-10-23: jsonc-parser 0.34.0 (CST edit API: `set_value`, `append`, `insert`, `remove`, name-literal `replace_with`, byte-exact in probes); jsonschema 0.50 (draft-07 … 2020-12); @tauri-apps/cli 2.12.4; tauri-plugin-dialog Rust API (R6); `@tauri-apps/api/mocks` `mockIPC`; WebdriverIO `@wdio/tauri-service`.
- Claude Code's official schema: SchemaStore `claude-code-settings.json` (draft-07, 142 properties, `additionalProperties: true`), vendored as the preset schema with R2 warning semantics. Per spec §6, it is never fetched at runtime.
- Shared code: the atomic write path is reused by document save, backup restore (spec §10) and registry/preferences save (R12). No other extraction proposed.

## Diagrams

Save pipeline (spec §10 with R5, R14, R15):
```
save_draft(file_id, expected_fp)
  │
  ├─ registry lookup (R6: id only) ──✗ unknown id ──► NOT_FOUND
  ├─ path checks: regular file? size ≤ 5 MiB? symlink confirmed? (R5) ──✗──► typed error
  ├─ read bytes → fp == expected_fp ? ──✗──► EXTERNAL_CHANGE_CONFLICT (draft kept)
  ├─ candidate = live CST.to_string() (+BOM if hasBom, R10)
  ├─ re-parse candidate (strict|jsonc) + dup-key walk ──✗──► reject, nothing written
  ├─ schema validate (sync, R14): user schema error ──✗──► SCHEMA_VALIDATION_FAILED
  │                                preset warnings ──► require "Save with N warnings" (R2)
  ├─ backup original bytes (0600 in 0700 dir) ──✗──► BACKUP_FAILED, nothing written
  ├─ re-resolve symlink + recheck fp ──✗──► EXTERNAL_CHANGE_CONFLICT
  ├─ write .<name>.configdeck-<rand>.tmp in target dir, copy mode bits, fsync (R15)
  ├─ rename tmp → target (link stays a link) ──✗──► remove tmp, ATOMIC_WRITE_FAILED
  ├─ fsync parent dir; read back; fp == candidate fp ? ──✗──► report, keep backup
  └─ adopt new fp (poller ignores own write, D2); clear op list; prune backups to 20
```

Edit flow (R3, R4, R13, R14):
```
React field ──apply_draft_operation(file_id, op)──► Rust session
                                                   ├─ validate pointer/op (C1 typed errors)
                                                   ├─ apply in place to live CST (~5 µs measured)
                                                   ├─ dup-key/type diagnostics (sync)
                                                   └─ return {changed nodes by pointer, count, diagnostics}
React node cache (pointer-keyed) ◄─────────────────┘
        ▲                                   worker: debounce 150 ms → CST → serde Value → jsonschema
        └────── event draft-diagnostics {fileId, revision, diagnostics} (stale revisions dropped)
undo/redo: pop op → replay remaining ops from original bytes (~36 ms/MiB measured) → changed nodes
```

## Failure modes
| Path | Realistic failure | Test | Handling | User sees |
|---|---|---|---|---|
| Save rename | disk full / EXDEV | A19 + temp cleanup | typed error, tmp removed | clear error |
| Crash mid-save | power loss after tmp write | R15 planted-leftover test | leftover offered for removal on open | notice in file details |
| Symlink | link retargeted during draft | R5 test | abort with conflict | conflict dialog |
| Poll (D2) | change lands < 1 s before Save | pre-save recheck test | conflict | conflict dialog |
| Background validation (R14) | late result for older draft | revision test | dropped; sync check at Save | correct diagnostics |
| Registry (R12) | truncated registry.json | 2 tests | .bak fallback, file set aside | notice |
| Claude Code writes settings.local.json mid-draft | conflict | A14 | conflict dialog, no merge | must discard or save copy (friction, not silent) |
| Webview compromise | forged path | A20 signature test | no path-taking command exists (R6) | n/a |
| BOM file | parser rejects | R10 fixture | strip/restore | opens normally |
Critical gaps (no test + no handling + silent): 1 flagged (crash leftovers), resolved by R15 (D20 C).

## Worktree parallelization strategy
| Step | Modules touched | Depends on |
|---|---|---|
| S1 Foundation (M1) | repo root, src/app, src-tauri shell | — |
| S2 Document engine (R3, R10, C1, R4, R13) | src-tauri/src/core/{document,patch} | S1 |
| S3 Save pipeline + backups + registry (R5, R12, R15) | src-tauri/src/core/{persistence,backup,security,registry} | S2 (fingerprint + candidate bytes) |
| S4 Schema + presets (R2, R9, R14, R1) | src-tauri/src/core/{schema,presets}, presets/ | S2 |
| S5 Commands + pickers (R6) | src-tauri/src/commands, capabilities | S2, S3 |
| S6 Frontend form engine (R13 cache, D4) | src/components, src/state, src/lib | S1 (DTO contract from S2) |
| S7 Tests + CI (R8, R11, R7 grep) | tests/, .github/workflows | S5, S6 |

Parallel lanes: Lane A: S2 → S3 → S5 (shared core/). Lane B: S4 (core/schema, presets/) after S2. Lane C: S6 against a mocked IPC contract once S2 fixes the DTO.
Execution order: S1. Then S2. Then launch A (S3→S5), B (S4) and C (S6) together; merge; then S7.
Conflict flags: A and B both touch `src-tauri/src/core/mod.rs` and the shared error enum; serialize edits to those two files. The spec's milestone order (§18) still sets the release gates.

## Implementation Tasks
Synthesized from this review's findings. Each task derives from a specific finding above. Run with Claude Code or Codex; checkbox as you ship.

- [ ] **T1 (P1, human: ~2d / CC: ~1h)** — core/document — CST-only parse, tree DTO with raw lexemes, nested duplicate-key walk, BOM strip/restore
  - Surfaced by: Architecture A1 (R3), Code Quality Q2 (R10)
  - Files: src-tauri/src/core/document/
  - Verify: `cargo test document::` incl. duplicate-key, number-lexeme and BOM fixtures
- [ ] **T2 (P1, human: ~2d / CC: ~1h)** — core/patch — op set incl. rename-key with typed errors, in-place apply, replay undo/redo, changed-node responses
  - Surfaced by: A2 (R4), Q3 (C1), Perf1 (R13)
  - Files: src-tauri/src/core/patch/
  - Verify: property test (apply N, undo k, redo k); 4 MiB toggle < 100 ms
- [ ] **T3 (P1, human: ~2d / CC: ~1h)** — core/persistence+backup+security — save pipeline with symlink write-through, nlink warning, mode bits, restore conflict, retention, temp naming + leftover detection
  - Surfaced by: A3 (R5), FM1 (R15)
  - Files: src-tauri/src/core/{persistence,backup,security}/
  - Verify: Linux tempdir integration tests A13-A19, A21, R5, R15 cases
- [ ] **T4 (P1, human: ~4h / CC: ~20min)** — commands — Rust-side pickers behind an injectable picker; no path arguments; webview capability without dialog/fs
  - Surfaced by: A4 (R6)
  - Files: src-tauri/src/commands/, src-tauri/capabilities/
  - Verify: A20 command-signature test
- [ ] **T5 (P2, human: ~1d / CC: ~30min)** — core/schema — preset warnings vs user errors, debounced worker validation with revisions, sync check at save
  - Surfaced by: SC2 (R2), Q1 (R9), Perf2 (R14)
  - Files: src-tauri/src/core/schema/, presets/claude-code/
  - Verify: A10, preset-warning save-with-confirm test, stale-revision test
- [ ] **T6 (P2, human: ~3h / CC: ~15min)** — core/watcher — 1 s poll of open document, own-write suppression
  - Surfaced by: Scope D2
  - Files: src-tauri/src/core/watcher/
  - Verify: A14, A15, own-save-no-banner test
- [ ] **T7 (P2, human: ~4h / CC: ~20min)** — core/registry — atomic write + .bak + corrupt recovery; schema association field (D3)
  - Surfaced by: T2 (R12), Scope D3
  - Files: src-tauri/src/core/registry/
  - Verify: truncated-registry tests
- [ ] **T8 (P2, human: ~2d / CC: ~1h)** — frontend — pointer-keyed node cache + lazy subtree, warnings confirm, density + font scaling, Claude repo-root hint
  - Surfaced by: Perf1 (R13), SC2 (R2), D4, SC1 (R1)
  - Files: src/components/, src/state/, src/lib/tauri-api/
  - Verify: Vitest + mockIPC; Playwright screenshots in both themes and densities
- [ ] **T9 (P2, human: ~2-3d / CC: ~1.5h)** — tests — WebdriverIO 4 real-app journeys + Playwright mocked flows
  - Surfaced by: Test T1 (R11)
  - Files: tests/e2e/
  - Verify: `pnpm wdio` and `pnpm playwright test` green locally and in CI
- [ ] **T10 (P2, human: ~4h / CC: ~20min)** — CI — ubuntu-22.04 workflow: fmt, clippy, cargo test, vitest, playwright, wdio (xvfb), R7 grep, AppImage/.deb artifacts
  - Surfaced by: A6 (R8), A5 (R7)
  - Files: .github/workflows/ci.yml, docs/packaging.md
  - Verify: green run on `dev` with artifacts attached
- [ ] **T11 (P3, human: ~2h / CC: ~10min)** — docs — decisions.md entries for D2, D4, R3-R15; known-limitations (undo replay ~36 ms/MiB, residual fingerprint race, no conflict merge)
  - Surfaced by: all sections
  - Files: docs/decisions.md, docs/known-limitations.md
  - Verify: each ledger ID referenced once

## Reality rows
Premise table:
| premise | claim | receipt | verdict |
|---|---|---|---|
| jsonc-parser CST edits are byte-faithful | spec:89 | scratchpad probe (toggle, CRLF, append, round-trip) | TRUE |
| strict mode via ParseOptions rejects comments | spec:500 | probe case 5 | TRUE |
| serde_json model agrees with CST | spec:88 | probe: dup keys first vs last | FALSE |
| Arch deps = webkit2gtk-4.1 et al. | spec:924 | v2.tauri.app/start/prerequisites | TRUE |
| maintained 2020-12 validator exists | spec:348 | docs.rs jsonschema 0.50.0 | TRUE |
| user settings at ~/.claude/settings.json or CLAUDE_CONFIG_DIR | spec:402 | code.claude.com settings docs | TRUE |
| local settings at `<project>/.claude/settings.local.json` | spec:404 | code.claude.com settings docs (repo root since v2.1.211) | PARTLY FALSE |
| managed file at /etc/claude-code/managed-settings.json | spec:405 | code.claude.com managed-settings docs | TRUE |
| ~/.claude.json holds account/MCP state | spec:406 | code.claude.com settings docs | TRUE |
| Playwright covers Tauri UI flows | spec:93 | v2.tauri.app/develop/tests/webdriver | FALSE (mocked only) |
| Manjaro-built AppImage is portable | spec:928 | v2.tauri.app/distribute/appimage | FALSE |
| UTF-8 handling complete | spec:502 | probe: BOM rejected | FALSE |
| cold start < 2 s, toggle < 100 ms achievable | spec:849-850 | no code; edit 5.4 µs measured | unverified |
unverified=1

REALITY: premise-table finding 13 premises, 5 false/partly false, unverified=1 spec:88
REALITY: already-done pass repo holds only README (45819f3), nothing pre-built git:origin/main
REALITY: surface-check pass `pnpm tauri dev|build` via @tauri-apps/cli 2.12.4 (npm view); `--help` not run (not installed) spec:927
REALITY: binding-decisions pass no prior gstack decisions; this review's D2-D20 recorded in ledger spec:1063
REALITY: numbers finding spec targets 5 MiB/20 backups/2 s/100 ms are [estimated] spec:330; parse 36 ms/MiB [measured: /tmp/claude-1000/-home-stefan-go-src-github-com-altafino-jsonui/9b3a30de-c634-455e-a75d-2ba62f0e3fb8/scratchpad/cstprobe/src/main.rs:9]; edit 5.4 µs [measured: /tmp/claude-1000/-home-stefan-go-src-github-com-altafino-jsonui/9b3a30de-c634-455e-a75d-2ba62f0e3fb8/scratchpad/cstprobe/src/main.rs:10]
REALITY: deferred-asks pass no linked issue spec:1
REALITY: pr-count pass no ship-policy.json, default one PR per wave; 6 milestones = 6 waves into dev spec:967

## Unresolved decisions
None.

## Completion summary
- Step 0: Scope Challenge — scope accepted as-is
- Reality rows: 7/7 applicable rows emitted, 1 unverified premise
- Architecture Review: 6 issues found
- Code Quality Review: 3 issues found
- Test Review: diagram produced, 40 proposed paths all GAP (greenfield), 2 gap decisions (R11, R12) + required-proof tests added to Test Plan
- Performance Review: 2 issues found
- NOT in scope: written
- What already exists: written
- TODOS.md updates: 1 item proposed to user (R15, built now in P0)
- Failure modes: 1 critical gap flagged (resolved by R15)
- Unresolved decisions: 0 in this review
- Outside voice: codex, disabled (codex_reviews=disabled), recorded as skipped
- Parallelization: 3 lanes, 3 parallel / 2 sequential (S1, S7)
- Lake Score: 6/18

Test Plan artifact: `~/.gstack/projects/altafino-jsonui/stefan-dev-eng-review-test-plan-20261023-104321.md`

## Suppressed findings
- (confidence 4/10) Full-document text diff for Review changes on 5 MiB files may be slow; not measured. Re-measure during Milestone 4.
- (confidence 4/10) Backup storage worst case 20 × 5 MiB = 100 MiB per file [estimated]; age-based cleanup is already optional in spec §10.

## Design review decisions (gstack /plan-design-review, 2026-10-23)

Target: this file. Initial design completeness: 5/10 (direction present; tokens, field-row anatomy, tri-state control, dialogs, state coverage and overflow rules missing). No DESIGN.md existed. Scope: all 7 passes (D1). Outside design voices: skipped by user (D4).
Approved visual reference (D2 board submit, confirmed D3): `/home/stefan/.gstack/projects/altafino-jsonui/designs/workspace-20261023/variant-A.png`. Calm dense developer tool: dark slate surfaces, one indigo accent, flat divided rows (no cards), mono for keys/paths/values. The light theme mirrors its structure.
Each decision below was approved individually; where it conflicts with §4/§5/§17, the decision wins.

### Pass 1: Information architecture
- **DD1 Row anatomy (D5 → 1A):** a form row is `label` + `control`. The raw key appears in small mono under the label only when it differs from the label (schema `title`) or when "Show advanced" is on. A type hint appears only when it carries information: `null`, `not in file`, type mismatch, or a mixed array. Hover or focus on the row, or "Show advanced", reveals key, type and JSON Pointer (`$.logging.level` display, RFC 6901 underneath) with a "Copy JSON path" action. Schema-less labels are humanized from the key (`retryCount` → "Retry count").
- **DD2 One primary action (D6 → 2A):** Review changes (secondary) and Save changes (primary) live only in the footer save bar, next to the pending-change count. Disabled Save shows its reason as a tooltip and as visible text in the bar (e.g. "Fix 1 error to save"). The header holds identity and file-level actions: title, path (copy/reveal), JSON/JSONC, preset, scope, status chip, Reload, History, overflow menu. Remove "review changes, save" from the §4 header list.
- **DD3 Depth rule (D7 → 3A):** objects and arrays expand inline with a 1px guide line per level up to depth 3. Deeper containers render as a single "Open ›" row showing a summary (`{4 keys}`, `[3 items]`). Opening one shows a drill-in view of that container, with a breadcrumb in the form header (`$ › hooks › PreToolUse › 0`, each segment clickable). Back, Esc or Alt+Left returns, and search results deep-link into the right drill-in view with parents expanded.
Pass 1 rating: 5/10 → 9/10 (remaining: visual weight of header vs form depends on Pass 5 tokens).

### Pass 2: Interaction states
- **DD4 Invalid file view (D8 → 4A):** for `INVALID_JSON` and `DUPLICATE_KEYS` the form is replaced by a read-only, line-numbered source view scrolled to the first error. The error line gets an error-tinted background, a caret marks the column, and a message panel above shows a plain-language explanation ("Comments are not allowed in .json files", "Key `model` appears twice: lines 3 and 18") plus the code. Duplicate keys highlight every occurrence. Actions: Open in editor (system default), Reload. Secret-like values stay masked. The sidebar row shows an error icon with the "Invalid" label (not color alone).
- **DD5 Empty states (D9 → 5A):** each has context plus one obvious next step, in utility language:
  | Where | Copy | Actions |
  |---|---|---|
  | Welcome (no files) | "Edit JSON settings as forms. Every save makes a backup first." | Add JSON file (primary); Discover Claude Code settings; Try with a sample |
  | Empty object `{}` | "This file is an empty object." | Add setting |
  | Empty array row | muted "Empty list" | Add item |
  | Field search, no match | "No fields match "xyz"" | Clear search |
  | Changed only, nothing changed | "No unsaved changes" | Show all fields |
  | History empty | "No backups yet. One is created every time you save." | none |
  | Claude discovery, nothing found | "No user settings at ~/.claude/settings.json" | Create file; Choose project folder |
  | Missing file | "This file no longer exists at <path>" | Locate file (Rust picker); Create empty file; Remove from ConfigDeck |
- **DD6 Save feedback in the save bar (D10 → 6A):** Save shows a spinner only after 150 ms. Success: the bar reads "Saved 14:02 · backup created · Undo save" for 8 s, then "All changes saved". "Undo save" restores that backup through the normal restore flow (conflict checks included). Failure: a persistent inline error strip above the bar with plain message, error code and remedy ("Backup folder is not writable. Choose another in Settings."); the draft is kept. Announcements go through aria-live: polite for success, assertive for failure. No toasts for save results.

State table (records the spec and DD4-DD6; no new choices):
| Feature | Loading | Empty | Error | Success | Partial |
|---|---|---|---|---|---|
| App start / registry | no placeholder flash (spec §17) | Welcome (DD5) | registry recovered notice (eng R12) | sidebar list | missing files flagged per row |
| Open document | no flash for fast opens (§17) | `{}` state (DD5) | invalid/duplicate source view (DD4); FILE_TOO_LARGE, UNSUPPORTED_ENCODING messages | form | preset-schema warnings on fields (eng R2) |
| Field search | debounced, no spinner | "No fields match" (DD5) | n/a | matches with parents expanded (§4) | n/a |
| Save | spinner after 150 ms (DD6) | Save disabled, "No unsaved changes" | inline error strip (DD6); conflict dialog (§3D) | "Saved · backup created · Undo save" (DD6) | "Save with N warnings" confirm (eng R2) |
| History / restore | list loads with document | "No backups yet" (DD5) | conflict on restore (eng plan) | restored, same save-bar feedback | n/a |
| Claude discovery | per-candidate rows | "No user settings at …" (DD5) | permission denied row state | candidates with scope chips | managed file listed read-only |
| Registered file gone | n/a | Missing file state (DD5) | permission denied: read-only chip + reason | n/a | n/a |
Pass 2 rating: 3/10 → 8/10 (remaining: large-file loading visuals are spec-level only).

### Pass 3: User journey
- **DD7 Sample file (D11 → 7A):** "Try with a sample" copies `fixtures/feature-flags.json` into ConfigDeck's app data folder and registers it under a sidebar group "Sample" with a "Sample" chip. The header note reads "Safe to edit. This is a copy in ConfigDeck's data folder." Save, backup, restore and conflict all work for real. "Remove from ConfigDeck" on the sample deletes that copy; for every other file it never deletes anything.

Journey storyboard (records spec §3 flows with approved decisions):
| Step | User does | User feels | Plan specifies |
|---|---|---|---|
| 1 | Opens app first time | curious, cautious | Welcome copy + 3 actions, backup promise up front (DD5) |
| 2 | Tries the sample | safe to experiment | real labeled copy (DD7) |
| 3 | Adds real Claude settings | wary ("will it break my config?") | exact path, scope chip, managed = read-only (§7) |
| 4 | Finds a setting | oriented | field search, labels over keys (DD1), breadcrumb (DD3) |
| 5 | Toggles, reviews | in control | changed markers + Review changes dialog (§9, Pass 7) |
| 6 | Saves | relieved | "Saved · backup created · Undo save" (DD6) |
| 7 | Claude Code edits the file mid-draft | interrupted, worried | conflict dialog (§3D; layout in Pass 7) |
| 8 | Months later restores a backup | reassured | History + restore with conflict checks (§10) |
Time horizons: 5 s = calm workspace and one primary action; 5 min = save/undo loop proves safety; long term = every save is recoverable and nothing is ever rewritten silently.
Pass 3 rating: 4/10 → 8/10 (remaining: step 7 depends on the conflict dialog decision in Pass 7).

### Pass 4: AI slop risk
Mode: OPERATE (app UI). Mockup A: no hard rejections (flat divided rows, no card mosaic, no hero, no gradients). Litmus: product clear in first screen YES (title bar + file identity); one visual anchor YES (value column); scannable by headings YES; one job per section YES; cards necessary: none used YES; motion: not specified (settled in Pass 5); premium without shadows YES. Slop signal found: Inter as the UI face (overused default).
- **DD8 Typefaces (D12 → 8A):** bundle IBM Plex Sans (400/500/600) for UI and IBM Plex Mono (400/500) for keys, paths, values and the source view, both OFL and offline. Numbers use tabular numerals. Tune row padding so mockup A's density holds at 1024 px with the wider face.
Pass 4 rating: 6/10 → 9/10 (remaining: browser surfaces such as scrollbars and selection, settled with tokens in Pass 5).

### Pass 5: Design system
- **DD9 Token set (D13 → 9A).** Milestone 1 writes these into `DESIGN.md` and CSS variables on `:root` / `[data-theme]`:
  - Color, dark: bg `#0B1020`, sidebar `#10172A`, row-hover `#141C33`, border `#1E293B`, text `#E2E8F0`, muted `#94A3B8`, accent `#6366F1` (hover/text/focus `#818CF8`). Light: bg `#FFFFFF`, sidebar `#F8FAFC`, border `#E2E8F0`, text `#0F172A`, muted `#475569`, accent `#4F46E5`. Semantic (light/dark): error `#DC2626`/`#F87171`, warning `#B45309`/`#F59E0B`, success `#15803D`/`#4ADE80`. Requirement: every text pair is at least 4.5:1.
    Measured (WCAG formula, 2026-10-23): text 15.4/17.9, muted 6.6-7.6, semantic 4.8-10.9, accent-as-text 6.3. **Open: white on dark accent `#6366F1` = 4.47:1, below the requirement (see D14).**
  - Type (IBM Plex, DD8): 20/600 file title; 16/400 labels and values (comfortable); 14 in compact density (explicit opt-in, D4); 13 mono meta (keys, paths); 12/500 caps section labels.
  - Space: 4px base (4/8/12/16/24/32); row height 44 comfortable, 32 compact; radius 6 controls, 8 dialogs; 1px borders; shadow only on dialogs/popovers.
  - Motion: 120 ms ease-out on expand/collapse only; none with `prefers-reduced-motion`.
  - Browser surfaces: thin themed scrollbars, `::selection` accent at 30%, caret accent, 2px focus ring with 2px offset.
- **DD10 Primary fill (D14 → 10A):** the primary button fill is `#4F46E5` in both themes, with white text at 6.29:1. Dark accent `#818CF8` serves focus ring, links and accent text (6.35:1). Correction applied with this decision: the D14 option named `#6366F1` as the dark hover fill, but white on `#6366F1` is 4.47:1, the failure D14 fixes. The hover fill is therefore `#4338CA` (7.90:1). `#6366F1` is used only for non-text accents such as selection tint and the active toggle track. This closes the open item in DD9.
- **DD11 Status semantics (D15 → 11A):**
  - Changed: an accent dot before the label (`#818CF8` dark / `#4F46E5` light), muted "was `<old>`" text, and the same accent dot on the sidebar row.
  - Warning: amber ▲ icon + text. Error: red ●! icon + text. Success: green ✓ + text.
  - Status is never conveyed by color alone (spec §4), and amber is reserved for warnings.
Pass 5 rating: 2/10 → 9/10 (remaining: icon set limited to Lucide per spec; no brand mark designed).

### Pass 6: Responsive and accessibility
- **DD12 Width rules (D16 → 12A):**
  - Below 1180 px the sidebar collapses to a 56 px rail: file icons with status dots, names in tooltips. Ctrl+B or the rail button expands it as an overlay; the expanded width is remembered locally.
  - Columns: label at least 200 px, control at least 280 px. Labels wrap to 2 lines, then ellipsis with the full text in a tooltip. Paths middle-truncate (`~/projects/…/feature-flags.json`).
  - Dialogs are `min(720px, 90vw)` wide.
  - Above 1600 px the form column is capped at 960 px. No horizontal scrolling at any width from 1024 px up.
- **DD13 Control border (D17 → 13A):** new token `control-border` `#64748B` in both themes (3.98:1 on `#0B1020`, 4.76:1 on `#FFFFFF`, measured) for text inputs, number inputs, selects, segmented controls and off-state switch tracks. Row and section dividers keep the subtle `border` token. The focus ring is unchanged (DD10).
- **DD14 Keyboard and screen-reader pattern (D18 → 14A):**
  - Landmarks: header, navigation (sidebar), main (form), and a status region (save bar, aria-live). F6 cycles between them.
  - Rows are native labeled controls; Tab moves control to control. Sections are disclosure buttons with `aria-expanded`.
  - The Not set / On / Off control is a radiogroup with arrow keys, announced as e.g. "Verbose logging, not set, not in file". Changed state goes in the accessible description ("changed, was false").
  - Shortcuts: Ctrl+K command search, Ctrl+F field search, Ctrl+S save, Ctrl+Z / Ctrl+Shift+Z document undo/redo when focus is not in a text field, Ctrl+B sidebar, Alt+Left back from drill-in, Shift+F10 context menu, Esc closes dialogs. Cmd replaces Ctrl on macOS. All shortcuts are listed in Settings › Shortcuts (spec §17).
Pass 6 rating: 6/10 → 9/10 (remaining: Wayland/X11 and Orca checks happen in Milestone 6 QA).

### Pass 7: Unresolved decisions
- **DD15 Review changes dialog (D19 → 15A):**
  - Two tabs. "Changes" is the default: one line per change with friendly path, old → new, and a per-change Revert. "Text diff" shows changed hunks only, with 3 lines of context, secret-like values masked and a per-value Reveal.
  - Preset-schema warnings (eng R2) sit in a block above the buttons, and the primary button reads "Save with N warnings". Buttons: Cancel, Save changes.
  - Implementation note, consistent with eng R4 replay: Revert removes that op and replays the rest. If a later op depends on its path, Revert is disabled with the tooltip "Revert later changes under <path> first".
- **DD16 Conflict dialog (D20 → 16A):**
  - Two short change lists, "Changed on disk" (original → disk) and "Your unsaved changes" (original → draft). Overlapping paths are marked "both changed"; an op counts as overlapping when its path, an ancestor, or its parent array changed on disk.
  - Actions:
    - "Reapply my changes to the new version": enabled only with no overlap. It replays the draft ops (eng R4) onto the new disk bytes into a new draft and writes nothing; the user reviews and saves normally.
    - "Discard my draft & reload".
    - "Save my version as a copy".
  - Save stays frozen until the conflict is resolved.
  - This moves a limited, non-overlapping reapply into P0, which the eng review's NOT-in-scope list had deferred as "field-level merge". True merge of overlapping changes stays deferred.
- **DD17 Arrays of objects (D21 → 17A):**
  - Items are collapsible flat rows, not cards: drag handle, index, summary, overflow menu (Duplicate, Move up, Move down, Remove), and Alt+Up/Down to move with the keyboard.
  - Summary: the first present of `name`, `title`, `id`, `label`, `key`, else the first short string field, plus up to two scalars ("frontend · enabled"). Fallback: "Item 3 · 4 keys".
  - A preset `ui.json` may set `itemSummary` per array path (Claude hooks: `matcher`).
Pass 7: 3 decisions resolved (DD15-DD17), 0 deferred.
<!-- design-decisions-end -->

Note for engineering: DD16 adds a non-overlapping "reapply my changes" path to P0, and DD7 adds a sample-file registry kind. Neither existed when the eng review ran; re-check them in the next eng pass.

## Design: NOT in scope
- Logo / brand mark and app icon art: not reviewed; spec §15 lists `icons/` only.
- True merge of overlapping conflict changes: DD16 covers only non-overlapping reapply.
- Motion beyond expand/collapse: DD9 limits motion on purpose.
- Light-theme mockup: not generated; light tokens are specified in DD9/DD10/DD13 and the structure mirrors the approved dark mockup.
- Phone/tablet layouts: not applicable (desktop app; minimum width 1024 px per spec §4).

## Design: What already exists
- No DESIGN.md or UI code in the repo (only README at 45819f3).
- Spec-mandated building blocks reused: shadcn/ui on Radix primitives (dialogs, tabs, radiogroup, switch, select, tooltip, context menu), Lucide icons, Tailwind with CSS variables from DD9.
- Fonts: IBM Plex Sans/Mono (OFL), bundled per DD8.

## Approved Mockups

| Screen/Section | Mockup Path | Direction | Notes |
|----------------|-------------|-----------|-------|
| Main workspace (dark) | /home/stefan/.gstack/projects/altafino-jsonui/designs/workspace-20261023/variant-A-reviewed.png | Calm dense dev tool: slate surfaces, one indigo accent, flat divided rows, IBM Plex | Reflects DD1, DD2, DD8, DD10, DD11, DD13. Deviation: the `features` row shows an "array" hint that DD1 omits; DD1 wins. Original pick (pre-review): variant-A.png in the same folder. |

## Design Implementation Tasks
Synthesized from this review's findings. Each task derives from a specific finding above. Run with Claude Code or Codex; checkbox as you ship.

- [ ] **D-T1 (P1, human: ~1d / CC: ~30min)**: design system. Write DESIGN.md + CSS variables (both themes), bundle IBM Plex, theme browser surfaces.
  - Surfaced by: Pass 5 (DD9, DD10, DD13), Pass 4 (DD8), DD11
  - Files: DESIGN.md, src/styles/, src/assets/fonts/
  - Verify: unit test asserting every text token pair is at least 4.5:1 and control-border is at least 3:1 in both themes; screenshot both themes
- [ ] **D-T2 (P1, human: ~2d / CC: ~1h)**: form engine UI. Field row anatomy, Not set / On / Off radiogroup, changed markers, array-of-object rows with summary rule.
  - Surfaced by: Pass 1 (DD1), Pass 6 (DD14), Pass 7 (DD17), DD11
  - Files: src/components/fields/, presets/claude-code/ui.json
  - Verify: Vitest + Testing Library (labels, radiogroup arrow keys, summary rule table); Playwright screenshot vs approved mockup
- [ ] **D-T3 (P1, human: ~2d / CC: ~1h)**: save bar states, Review changes dialog, conflict dialog with safe reapply.
  - Surfaced by: Pass 1 (DD2), Pass 2 (DD6), Pass 7 (DD15, DD16)
  - Files: src/components/chrome/, src/components/diff/, src/components/dialogs/, src-tauri/src/core/patch/
  - Verify: Vitest for save-bar state machine; WebdriverIO A14 journey extended with reapply (no-overlap enabled, overlap disabled)
- [ ] **D-T4 (P2, human: ~4h / CC: ~20min)**: invalid-file source view.
  - Surfaced by: Pass 2 (DD4)
  - Files: src/components/document/
  - Verify: fixtures (comment in .json, missing comma, duplicate keys) render the highlight, caret and message
- [ ] **D-T5 (P2, human: ~4h / CC: ~20min)**: empty states and sample file.
  - Surfaced by: Pass 2 (DD5), Pass 3 (DD7)
  - Files: src/components/document/, src/components/file-list/, src-tauri/src/core/registry/
  - Verify: Vitest per empty state; sample copy lands in app data dir and removal deletes only the copy
- [ ] **D-T6 (P2, human: ~1d / CC: ~30min)**: depth drill-in with breadcrumb, width rules and sidebar rail.
  - Surfaced by: Pass 1 (DD3), Pass 6 (DD12)
  - Files: src/components/document/, src/components/chrome/
  - Verify: Playwright at 1024, 1366, 1920 and 3840 px: no horizontal scroll; deep fixture (depth 6) uses drill-in

## Design Review Completion Summary

```
  +====================================================================+
  |         DESIGN PLAN REVIEW — COMPLETION SUMMARY                    |
  +====================================================================+
  | System Audit         | no DESIGN.md; large UI scope (14 surfaces)  |
  | Step 0               | 5/10 initial; all 7 dimensions (D1)         |
  | Pass 1  (Info Arch)  | 5/10 → 9/10 after fixes                     |
  | Pass 2  (States)     | 3/10 → 8/10 after fixes                     |
  | Pass 3  (Journey)    | 4/10 → 8/10 after fixes                     |
  | Pass 4  (AI Slop)    | 6/10 → 9/10 after fixes                     |
  | Pass 5  (Design Sys) | 2/10 → 9/10 after fixes                     |
  | Pass 6  (Responsive) | 6/10 → 9/10 after fixes                     |
  | Pass 7  (Decisions)  | 3 resolved, 0 deferred                      |
  +--------------------------------------------------------------------+
  | NOT in scope         | written (5 items)                           |
  | What already exists  | written                                     |
  | TODOS.md updates     | 0 items proposed (no deferrals)             |
  | Approved Mockups     | 4 generated, 1 approved (+1 updated)        |
  | Decisions made       | 17 added to plan (DD1-DD17)                 |
  | Decisions deferred   | 0                                           |
  | Overall design score | 2/10 → 8/10                                 |
  +====================================================================+
```
All rated passes are 8+. Outside design voices were skipped by user choice (D4).

Unresolved Decisions: none.

## Eng re-check ledger (gstack /plan-eng-review, 2026-10-23, after design review)

Target: this file. Scope: delta since the first eng review, mainly DD7 (sample file) and DD16 (conflict reapply), plus the engineering side of DD3, DD4, DD6, DD15. Complexity gate not tripped (< 8 files, no new service).

### Correction C2 (no decision needed): guard sample-file deletion (DD7)
DD7 approved "Remove from ConfigDeck on the sample deletes that copy; for every other file it never deletes anything." Necessary implementation: the delete path runs in Rust only for registry entries with `kind: sample` whose canonicalized path is inside the app data directory. Any other entry, or a sample entry whose stored path resolves outside that directory, is only unregistered. Tests: a non-sample removal leaves the file; a sample removal deletes only the copy; a tampered sample path outside the data dir is not deleted.

### Correction C3 (no decision needed): rebase the draft after reapply (DD16)
DD16 approved "replays the draft ops onto the new disk bytes into a new draft and writes nothing". Necessary implementation: after reapply, the session's original bytes and fingerprint become the disk version's, and the op list and redo stack are the replayed ops (eng R4). Line endings and BOM follow the disk version (R10). Tests: property test that reapply output equals applying the ops to the disk bytes and leaves every disk-changed path byte-identical; reapply disabled whenever overlap exists.

### R16: DD16 overlap rule must include descendants
Finding: RC1, P1, confidence 8/10, DD16 text ("an op counts as overlapping when its path, an ancestor, or its parent array changed on disk"), reviewer: Claude (plan-eng-review re-check)
Plan baseline: DD16 approved (D20 → 16A), with the overlap definition quoted above.
Runtime evidence: none (no code). Reasoning: a draft op that sets `/permissions` to a new object while the disk changed `/permissions/allow` matches none of path-equal, ancestor or parent-array, so reapply would overwrite the disk change. DD16's stated contract is that reapply never overwrites external changes.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R16 overlap definition | path, ancestor, or parent array changed on disk (DD16) | overlap when a draft op's target path and any disk-changed path are equal, or one is a prefix (ancestor) of the other in either direction, or they share a parent array; `remove`/`rename-key` of a key whose subtree changed counts as overlap; reapply also disabled if the disk version fails to parse | keep DD16 wording as approved |
| C2, C3 | necessary implementation | unchanged | unchanged |
| DD16 rest (lists, actions, writes nothing) | approved | unchanged | unchanged |
Question D1:
D1 — Close the hole in the conflict "Reapply" rule?
Project/branch/task: altafino/jsonui `dev`, design decision DD16 (conflict dialog) in ConfigDeck_AI_Agent_Build_Spec.md.
ELI10: DD16 lets you reapply your unsaved edits onto a file that changed on disk, but only if your edits and the disk changes don't touch the same thing. The written rule checks "same path, a parent of it, or the same list". It forgets the case where your edit replaces a whole section and the disk changed something inside that section. Example: you replace `permissions`, Claude Code meanwhile added a rule inside `permissions.allow`; reapply would quietly wipe Claude's rule. A: treat any parent/child relationship in either direction as overlap. B: keep the rule as written.
Stakes if we pick wrong: with B, the one button designed to protect Claude's concurrent writes can silently delete them.
Recommendation: A because it is the rule DD16 actually promised ("never overwrites external changes"), and it costs a two-way prefix check.
Completeness: A=10/10, B=5/10
Pros / cons:
A) Two-way prefix overlap (recommended)
  ✅ Reapply can never overwrite a disk change inside a section you replaced
  ✅ Same check covers remove and rename of a key whose contents changed
  ❌ Reapply is disabled a bit more often (whole-section edits) (human: ~2h / CC: ~10min)
B) Keep DD16 wording
  ✅ Reapply enabled in slightly more cases
  ✅ No change to the approved text
  ❌ Whole-section edits can silently erase disk changes
Net: a stricter but safe reapply versus one that can lose Claude's writes.
Header: Overlap rule
Options:
A) Two-way prefix (recommended)
Overlap = equal paths, either path a prefix of the other, or shared parent array; remove/rename of a changed subtree counts; disk version must parse. Tests: section-replace vs child-change is overlap; sibling changes are not. human: ~2h / CC: ~10min
B) Keep DD16 wording
Path, ancestor, or parent array only.

State: approved
Actual answer: A) "Two-way prefix (recommended)" (D1 answer, 2026-10-23, eng re-check)
Accepted scope: replace DD16's overlap definition. An op overlaps when its target path and any disk-changed path are equal, either is a prefix of the other, or they share a parent array. `remove`/`rename-key` of a key whose subtree changed counts as overlap, and reapply is disabled if the disk version fails to parse. Tests: a section replace vs a child change on disk is overlap (reapply disabled); sibling-key changes are not; remove of a changed subtree is overlap.
History: none

### R17: Masking in the invalid-file source view (DD4)
Finding: RC2, P2, confidence 7/10, DD4 text ("Secret-like values stay masked"), reviewer: Claude (plan-eng-review re-check)
Plan baseline: DD4 approved (D8 → 4A), with masking unspecified for files that do not parse.
Runtime evidence: scratchpad probe, jsonc-parser 0.34 `Scanner` (2026-10-23): on a missing-comma file it yields every token with byte offsets (`S(sk-secret-123)@26..41`); on an unterminated string it yields tokens up to `ERR Unterminated string literal on line 3 column 8` and stops. Path-based redaction (eng plan) needs a CST, which invalid files lack.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R17 masking rule in invalid view | "secret-like values stay masked" (unspecified for unparseable files) | tokenize with the jsonc Scanner; mask every string value (a string not followed by `:`); keys, punctuation, numbers, booleans stay visible; after a lexical error, mask every quoted run on the remaining lines; per-value Reveal plus "Reveal all" | tokenize; mask only secret-looking values: key matches preset/user patterns (default `*TOKEN*`, `*KEY*`, `*SECRET*`, `*PASSWORD*`) or value matches credential shapes; per-value Reveal |
| DD4 rest (source view, highlight, caret, actions) | approved | unchanged | unchanged |
Question D2:
D2 — How should secrets be hidden when a file can't be parsed?
Project/branch/task: altafino/jsonui `dev`, design decision DD4 (invalid-file source view).
ELI10: When a file has a syntax error, ConfigDeck shows its raw text so the user can see the mistake, and DD4 promised secrets stay hidden. Normally we find secrets by their place in the file (like `env.API_TOKEN`), but a broken file has no reliable structure. I tested the tokenizer: it still splits a broken file into keys, values and punctuation up to the error. A: hide every text value; keys and punctuation stay visible, which is what you need to fix a syntax error; click to reveal. B: hide only values that look like secrets (by key name or shape), so everything else is readable.
Stakes if we pick wrong: with B, a token under an unusual key name (e.g. `"auth": "ghx_..."`) shows in plain text on screen and in screenshots; with A, users click Reveal when the error is inside a value.
Recommendation: A because the view exists to fix syntax, which never needs value text, and hiding all values cannot miss a secret.
Completeness: A=10/10, B=7/10
Pros / cons:
A) Mask all string values (recommended)
  ✅ No heuristic can miss a secret; nothing leaks into screenshots
  ✅ Keys and punctuation stay visible, which is what syntax fixes need
  ❌ Errors inside a value need one Reveal click (human: ~3h / CC: ~15min)
B) Mask only secret-looking values
  ✅ Most of the file reads normally
  ✅ Reuses the preset secret patterns
  ❌ Unusual key names or value shapes slip through unmasked
Net: one extra click for value errors versus heuristics that can leak a token.
Header: Mask rule
Options:
A) Mask all values (recommended)
Scanner-based: mask every string value; keys/punctuation/numbers/booleans visible; after a lexical error mask quoted runs on remaining lines; per-value Reveal + Reveal all. Tests: missing-comma and unterminated-string fixtures never render the secret value. human: ~3h / CC: ~15min
B) Mask secret-looking only
Scanner-based: mask values whose key matches preset/user patterns or whose text matches credential shapes; per-value Reveal.

State: approved
Actual answer: A) "Mask all values (recommended)" (D2 answer, 2026-10-23, eng re-check)
Accepted scope: the DD4 invalid-file view tokenizes with the jsonc `Scanner` and masks every string value (a string not followed by `:`). Keys, punctuation, numbers and booleans stay visible. After a lexical error, every quoted run on the remaining lines is masked. Per-value Reveal plus "Reveal all". Tests: missing-comma and unterminated-string fixtures containing a token never render the token text until Reveal.
History: none

### Correction C4 (no decision needed): disk-change diff for DD16
Necessary implementation of DD16's "Changed on disk" list and the R16 overlap check. A structural diff of the original CST vs the disk CST yields the changed paths. Arrays are compared positionally, and any change inside an array marks the array path itself as changed (conservative, consistent with R16's shared-parent-array rule). If the disk version does not parse, the dialog says "File on disk is not valid JSON" and offers only Discard my draft & reload and Save my version as a copy. Cost: two parses at ~36 ms/MiB [measured, eng R13 bench], only when a conflict occurs.

### R18: One shared path-overlap helper for DD15 and DD16
Finding: RQ1, P3, confidence 7/10, DD15 ("If a later op depends on its path, Revert is disabled") and R16 ("overlap when … equal, either is a prefix of the other, or they share a parent array"), reviewer: Claude (plan-eng-review re-check)
Plan baseline: none; both behaviors approved separately (DD15 via D19, R16 via D1 re-check), with no shared contract named.
Runtime evidence: none (no code). Proposed callers (plan-review assumption): DD15 revert-dependency check in `core/patch` and DD16/R16 reapply overlap check in `core/patch`. Both need "do two RFC 6901 pointers touch the same data", with identical semantics.
Comparison grid:
| Choice | Current | A | B |
|---|---|---|---|
| R18 overlap logic | unspecified (two features) | one helper `pointers_overlap(a, b) -> bool` in `core/patch` (equal, prefix either way, shared parent array; segment-aware so `/env/A` is not a prefix of `/env/AB`); DD15 and DD16 both call it; one table-driven test | each feature implements its own check |
| DD15, R16 behavior | approved | unchanged | unchanged |
Question D3:
D3 — Share one "do these paths touch?" helper between Revert and Reapply?
Project/branch/task: altafino/jsonui `dev`, DD15 (per-change Revert) and DD16/R16 (conflict Reapply).
ELI10: Two features need the same answer: "does this edit touch the same data as that one?" Revert needs it to know if a later edit depends on the one being reverted; Reapply needs it to know if your edit collides with a disk change. If each writes its own check, they will drift, and one of them will make the classic mistake of treating `/env/A` as a parent of `/env/AB` (plain text prefix instead of path segments). A: one small helper with one test table. B: each feature writes its own.
Stakes if we pick wrong: with B, a subtle prefix bug in one copy either blocks valid reverts or, worse, lets Reapply overwrite a disk change.
Recommendation: A because both callers need identical semantics and the safety case rests on getting this one function right once.
Completeness: A=10/10, B=6/10
Pros / cons:
A) One shared helper (recommended)
  ✅ One definition of overlap, one test table, both features stay consistent
  ✅ Segment-aware comparison fixes the `/env/A` vs `/env/AB` trap in one place
  ❌ A bug in it affects both features at once (mitigated by the shared test table) (human: ~1h / CC: ~5min)
B) Separate checks per feature
  ✅ Each feature can tune its rule independently
  ✅ No coupling between Revert and Reapply
  ❌ Two copies of safety-critical logic that can drift apart
Net: one tested function versus two drifting copies of the same rule.
Header: Overlap helper
Options:
A) Shared helper (recommended)
`pointers_overlap(a, b)` in core/patch, segment-aware (equal, prefix either way, shared parent array); used by DD15 and DD16; table-driven test incl. `/env/A` vs `/env/AB`, `~0`/`~1` escapes, array parents. human: ~1h / CC: ~5min
B) Separate checks
DD15 and DD16 each implement their own overlap logic and tests.

State: approved
Actual answer: A) "Shared helper (recommended)" (D3 answer, 2026-10-23, eng re-check)
Accepted scope: one helper `pointers_overlap(a, b) -> bool` in `core/patch`. It compares decoded RFC 6901 segments: equal, prefix either way, or shared parent array. DD15's revert-dependency check and DD16/R16's reapply check both call it. One table-driven test covers `/env/A` vs `/env/AB` (no overlap), `~0`/`~1` escapes, array parents and the root pointer.
History: none

Approval readiness (re-check): PASS. Checked R16 (D1 A), R17 (D2 A), R18 (D3 A); corrections C2, C3, C4 are necessary implementation of DD7/DD16. Earlier R1-R15 and DD1-DD17 unchanged except DD16's overlap definition, now replaced by R16. No pending choices.

Re-check sections: Architecture: C4 recorded, no further issues. Code quality: R18. Tests: delta cases added to the updated Test Plan artifact (`~/.gstack/projects/altafino-jsonui/stefan-dev-eng-review-test-plan-20261023-recheck.md`). Performance: no issues; conflict diff costs two parses (~36 ms/MiB measured) only on conflict. Outside voice: disabled (`codex_reviews=disabled`). TODOs: none proposed.

Failure modes (delta): reapply over a disk change inside a replaced section is blocked by R16 (test). Unparseable disk version is handled by C4 (test). A secret in an invalid file is masked by R17 (test). Sample deletion hitting a user file is blocked by C2 (test). Critical gaps: 0.

Reality rows (re-check):
REALITY: premise-table pass 2 premises: Scanner tokenizes broken JSON with offsets TRUE (scratchpad probe, this run); DD16 overlap rule covers all collisions FALSE → R16; unverified=0 DD16
REALITY: already-done pass repo unchanged at 45819f3, no code yet git:origin/main
REALITY: surface-check pass jsonc_parser::Scanner::new/scan/token_start/token_end verified in source jsonc-parser-0.34.0/src/scanner.rs:56
REALITY: binding-decisions pass R1-R15 and DD1-DD17 honored; only DD16's overlap definition replaced, by user answer D1 (R16)
REALITY: numbers pass no new numbers; parse 36 ms/MiB [measured: /tmp/claude-1000/-home-stefan-go-src-github-com-altafino-jsonui/9b3a30de-c634-455e-a75d-2ba62f0e3fb8/scratchpad/cstprobe/src/main.rs:9 (first eng run)]
REALITY: deferred-asks pass no linked issue
REALITY: pr-count pass default one PR per wave; no new waves

### Re-check Implementation Tasks
- [ ] **RT1 (P1, human: ~1d / CC: ~45min)**: core/patch. Disk-change diff (C4), `pointers_overlap` helper (R18), reapply with rebase (C3, R16).
  - Surfaced by: re-check Scope Challenge RC1, Section 1 C4, Section 2 R18
  - Files: src-tauri/src/core/patch/, src-tauri/src/core/document/
  - Verify: overlap table test, reapply property test, WebdriverIO reapply journey
- [ ] **RT2 (P2, human: ~3h / CC: ~15min)**: core/security. Scanner-based masking for the invalid-file view (R17).
  - Surfaced by: re-check RC2
  - Files: src-tauri/src/core/security/, src/components/document/
  - Verify: masked fixtures never render token text until Reveal
- [ ] **RT3 (P2, human: ~1h / CC: ~5min)**: core/registry. Guard sample deletion by kind + data-dir containment (C2).
  - Surfaced by: re-check C2
  - Files: src-tauri/src/core/registry/
  - Verify: tampered-path test keeps the file

Re-check completion summary: Scope Challenge: scope accepted as-is (2 findings → R16, R17; corrections C2, C3). Reality rows 7/7, 0 unverified. Architecture 0 issues (C4 correction). Code quality 1 issue (R18). Tests: 8 delta gaps added to the Test Plan. Performance 0. Critical gaps 0. Unresolved 0. Outside voice: codex disabled. Lake Score 3/3.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | — |
| Outside Review | codex via `/plan-eng-review` outside voice | Independent 2nd opinion | 2 | disabled | outside_status: disabled (codex_reviews=disabled), no findings |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 2 | ISSUES OPEN | 9 issues, 0 critical gaps (re-check of DD7/DD16; 3 decisions approved, 0 unresolved) |
| Design Review | `/plan-design-review` | UI/UX gaps | 1 | CLEAR | score: 2/10 → 8/10, 17 decisions |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | — |

- **OUTSIDE COVERAGE:** both eng plan-review runs: codex disabled by config (`codex_reviews=disabled`). Design phase: outside design voices skipped by user. No outside findings.
- **VERDICT:** DESIGN CLEARED. Eng Review is ISSUES OPEN (9 findings in the re-check, all mapped to approved decisions R16-R18 and corrections C2-C4; 0 unresolved). eng review required
NO UNRESOLVED DECISIONS


