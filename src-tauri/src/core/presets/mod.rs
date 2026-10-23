//! Built-in presets and Claude Code discovery (schema lane).
//!
//! Presets are data only, bundled from `presets/<id>/` at the repo root
//! (manifest.json, settings.schema.json, ui.json) via `include_str!`.
//! Claude Code: vendored SchemaStore schema (draft-07), warnings only (R2).

use std::path::{Path, PathBuf};

use crate::core::schema::CompiledSchema;
use crate::core::types::{DiscoveredCandidate, FieldHint, PresetInfo};

pub struct PresetRegistry {
    _private: (),
}

impl PresetRegistry {
    pub fn bundled() -> Self {
        todo!("schema lane")
    }

    /// Detect a preset for a file path (+ optional parsed content).
    pub fn detect(&self, path: &Path, value: Option<&serde_json::Value>) -> Option<String> {
        let _ = (path, value);
        todo!("schema lane")
    }

    pub fn info(&self, preset_id: &str) -> Option<PresetInfo> {
        let _ = preset_id;
        todo!("schema lane")
    }

    pub fn schema(&self, preset_id: &str) -> Option<&CompiledSchema> {
        let _ = preset_id;
        todo!("schema lane")
    }

    /// Preset UI hints merged into schema hints (descriptions, itemSummary,
    /// secret path patterns, sensitive settings such as permissions).
    pub fn ui_hints(&self, preset_id: &str, pointers: &[String]) -> Vec<FieldHint> {
        let _ = (preset_id, pointers);
        todo!("schema lane")
    }

    /// Secret test from the preset's `secretPathPatterns` (glob on pointers).
    pub fn is_secret(&self, preset_id: &str, pointer: &str) -> bool {
        let _ = (preset_id, pointer);
        todo!("schema lane")
    }
}

/// Inputs for Claude discovery; Tauri layer supplies real dirs, tests use temp dirs.
pub struct ClaudeEnv {
    pub home: PathBuf,
    /// `CLAUDE_CONFIG_DIR` if set.
    pub config_dir: Option<PathBuf>,
    /// `/etc/claude-code` on Linux (overridable for tests).
    pub managed_dir: PathBuf,
}

/// Candidates: user settings, managed (read-only, incl. `managed-settings.d/*.json`),
/// and for `project_dir`: `.claude/settings.json` (shared) and
/// `.claude/settings.local.json` (local, note: Claude Code ≥ 2.1.211 may read
/// the local file from the git repo root — eng R1). Never lists `~/.claude.json`.
/// `candidate_id` is opaque; the integration layer maps it to the path.
pub fn discover_claude(env: &ClaudeEnv, project_dir: Option<&Path>) -> Vec<DiscoveredCandidate> {
    let _ = (env, project_dir);
    todo!("schema lane")
}
