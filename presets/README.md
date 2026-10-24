# Built-in presets

Presets are data only. The app bundles them at compile time (`include_str!`), never downloads them, and never executes anything from them.

```
presets/<dir>/
  manifest.json          required
  settings.schema.json   optional JSON Schema (named by manifest.schema.file)
  ui.json                optional UI metadata
```

## manifest.json

| key | meaning |
|---|---|
| `presetId` | stable id stored in the registry; never rename |
| `displayName`, `description` | shown in the UI |
| `version` | preset version (semver); bump on any change to the files |
| `schema.file/source/fetched/draft` | provenance of the vendored schema; `fetched` is the date it was downloaded by hand |

## ui.json

| key | meaning |
|---|---|
| `groups[]` | `{id, title, paths[]}`: sidebar groups; `paths` are RFC 6901 pointers (only keys that exist in the schema) |
| `secretPathPatterns[]` | globs over decoded pointer segments (`*` and `?`, case-insensitive); matching values are masked by default. Advisory, not a secret detector |
| `sensitivePointers[]` | same glob syntax; fields that get a "sensitive setting" warning when changed |
| `protectedPointers[]` | same glob syntax; fields that can be changed but not removed or renamed (schema `required` fields are protected too) |
| `openMapPointers[]` | same glob syntax; objects edited as free-form name/value maps even though the schema also lists known keys (e.g. `/env`) |
| `itemSummary` | `{pointerGlob: memberName}`; label for items of an array of objects (e.g. hooks use `matcher`) |
| `descriptions` | `{pointer: text}`; overrides the schema description |

## Stability and versioning

- Preset schemas produce warnings only (they can lag behind the vendor); user-associated schemas produce errors.
- Do not hardcode vendor value lists (models, permission modes) in `ui.json`; they come from the vendored schema.
- To update `claude-code`: download the new schema by hand, replace `settings.schema.json`, update `schema.fetched` and bump `version`. `cargo test` checks that every pointer in `ui.json` exists in the schema.
