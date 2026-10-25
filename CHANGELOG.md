# Changelog

All notable changes to ConfigDeck are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.1] - 2026-10-25

### Changed

- **Go to setting** (Ctrl+K) jumps to a field and leaves the rest of the form visible. **Filter this form** hides the others and shows how many fields match.
- The pending-change count in the footer jumps to the changed field. Previous and next appear when more than one field changed. Clicking a path in Review changes does the same.
- Remove is a trash icon, shown when the row is hovered or focused, so it is not mistaken for clearing the value.
- The save bar shrinks to one line when there is nothing to save. Review and Save return as soon as there is a pending change, and stay up after a successful save so Undo save is available.

## [0.1.0] - 2026-10-24

### Added

- **Apache License 2.0.**
- **Arch and Manjaro packages.** `make aur-install` builds and installs from a checkout. After a release is published, AUR users install `configdeck` (build from source) or `configdeck-bin` (the release build).
- **Remove buttons on every setting**, not only in the row's hover menu.
- **Remove several items at once:** **Select** in the toolbar puts checkboxes on
  settings, sections, list items and map entries; **Remove selected** removes them as
  one step that a single Undo restores.
- **Protected settings:** settings a schema marks `required`, or a preset lists in
  `protectedPointers`, show "Required" and can't be removed or renamed. ConfigDeck's
  backend refuses it too.
- **YAML (`.yaml`, `.yml`) and TOML (`.toml`) files**, edited as forms with the same
  guarantees as JSON: only the touched value's bytes change, and comments, quoting,
  ordering and line endings stay as they were. Undo, review, backups, conflict
  detection and reapply work the same. Every YAML/TOML edit is re-parsed and checked
  against the expected result before it is accepted. Limits (anchors, multi-document
  YAML, TOML null and layout cases) are listed in `docs/known-limitations.md`.

### Fixed

- **Revert no longer offered for edits that a later change depends on through a
  shifted list index.** Removing or inserting a whole list item shifts the indices
  of the items after it. The review list could show Revert on an earlier edit
  inside one of those items even though a later edit relied on it, and clicking
  Revert then failed with "The path does not exist in the document". Paths that
  meet at an index of the same list now count as overlapping when either one
  addresses a whole item, so Revert stays disabled ("Revert later changes under
  <path> first"). Edits deeper inside two different items stay independent. Reapplying a
  draft after an outside change was not affected.
