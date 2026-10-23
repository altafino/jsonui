//! Built-in presets and Claude Code discovery (schema lane).
//!
//! Presets are data only, bundled from `presets/<id>/` at the repo root
//! (manifest.json, settings.schema.json, ui.json) via `include_str!`.
//! Claude Code: vendored SchemaStore schema (draft-07), warnings only (R2).

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::schema::{CompiledSchema, SchemaOrigin, decode_pointer};
use crate::core::types::{DiscoveredCandidate, FieldHint, PresetGroup, PresetInfo, Scope};

const CLAUDE_ID: &str = "claude-code-settings";
const CLAUDE_SCHEMA_URLS: [&str; 2] = [
    "https://json.schemastore.org/claude-code-settings.json",
    "https://www.schemastore.org/claude-code-settings.json",
];
const LOCAL_NOTE: &str = "Claude Code 2.1.211+ may read this file from the git repository root instead. Keep personal settings out of version control.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    preset_id: String,
    display_name: String,
    version: String,
}

#[derive(Deserialize)]
struct UiGroup {
    id: String,
    title: String,
    paths: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Ui {
    groups: Vec<UiGroup>,
    secret_path_patterns: Vec<String>,
    sensitive_pointers: Vec<String>,
    protected_pointers: Vec<String>,
    open_map_pointers: Vec<String>,
    item_summary: BTreeMap<String, String>,
    descriptions: BTreeMap<String, String>,
}

struct Preset {
    info: PresetInfo,
    ui: Ui,
    schema: Option<CompiledSchema>,
}

impl Preset {
    /// Bundled data is validated by tests, so a parse failure is a build-time bug.
    fn load(manifest: &str, schema: Option<&str>, ui: Option<&str>) -> Self {
        let m: Manifest = serde_json::from_str(manifest).expect("bundled manifest.json is valid");
        let ui: Ui = ui
            .map(|u| serde_json::from_str(u).expect("bundled ui.json is valid"))
            .unwrap_or_default();
        // A schema that fails to compile degrades to structural editing instead of crashing.
        let schema = schema
            .and_then(|s| serde_json::from_str(s).ok())
            .and_then(|s| CompiledSchema::compile(&s, SchemaOrigin::Preset).ok());
        let groups = ui
            .groups
            .iter()
            .map(|g| PresetGroup {
                id: g.id.clone(),
                title: g.title.clone(),
                pointers: g.paths.clone(),
            })
            .collect();
        let info = PresetInfo {
            preset_id: m.preset_id,
            display_name: m.display_name,
            version: m.version,
            groups,
        };
        Self { info, ui, schema }
    }
}

pub struct PresetRegistry {
    presets: Vec<Preset>,
}

impl PresetRegistry {
    pub fn bundled() -> Self {
        Self {
            presets: vec![
                Preset::load(
                    include_str!("../../../../presets/claude-code/manifest.json"),
                    Some(include_str!(
                        "../../../../presets/claude-code/settings.schema.json"
                    )),
                    Some(include_str!("../../../../presets/claude-code/ui.json")),
                ),
                Preset::load(
                    include_str!("../../../../presets/generic/manifest.json"),
                    None,
                    None,
                ),
            ],
        }
    }

    fn get(&self, preset_id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.info.preset_id == preset_id)
    }

    /// Detect a preset for a file path (+ optional parsed content).
    pub fn detect(&self, path: &Path, value: Option<&serde_json::Value>) -> Option<String> {
        let name = path.file_name()?.to_str()?;
        let parent = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|p| p.to_str());
        let by_path = match (parent, name) {
            (Some(".claude"), "settings.json" | "settings.local.json") => true,
            (Some("managed-settings.d"), n) => n.ends_with(".json"),
            (_, "managed-settings.json") => true,
            _ => false,
        };
        let by_schema = value
            .and_then(|v| v.get("$schema"))
            .and_then(|s| s.as_str())
            .is_some_and(|s| CLAUDE_SCHEMA_URLS.contains(&s));
        (by_path || by_schema).then(|| CLAUDE_ID.to_owned())
    }

    pub fn info(&self, preset_id: &str) -> Option<PresetInfo> {
        self.get(preset_id).map(|p| p.info.clone())
    }

    pub fn schema(&self, preset_id: &str) -> Option<&CompiledSchema> {
        self.get(preset_id)?.schema.as_ref()
    }

    /// Preset UI hints merged into schema hints (descriptions, itemSummary,
    /// secret path patterns, sensitive settings such as permissions).
    ///
    /// Returns only the preset layer, one hint per pointer in order: the
    /// caller overlays it on `CompiledSchema::hints` (`Some` fields replace,
    /// booleans are OR-ed).
    pub fn ui_hints(&self, preset_id: &str, pointers: &[String]) -> Vec<FieldHint> {
        let Some(p) = self.get(preset_id) else {
            return pointers
                .iter()
                .map(|ptr| FieldHint {
                    pointer: ptr.clone(),
                    ..FieldHint::default()
                })
                .collect();
        };
        pointers
            .iter()
            .map(|ptr| FieldHint {
                pointer: ptr.clone(),
                description: p.ui.descriptions.get(ptr).cloned(),
                item_summary: p
                    .ui
                    .item_summary
                    .iter()
                    .find(|(pat, _)| pointer_glob(pat, ptr))
                    .map(|(_, field)| field.clone()),
                secret: p
                    .ui
                    .secret_path_patterns
                    .iter()
                    .any(|pat| pointer_glob(pat, ptr)),
                open_map: p
                    .ui
                    .open_map_pointers
                    .iter()
                    .any(|pat| pointer_glob(pat, ptr)),
                sensitive: p
                    .ui
                    .sensitive_pointers
                    .iter()
                    .any(|pat| pointer_glob(pat, ptr)),
                protected: p
                    .ui
                    .protected_pointers
                    .iter()
                    .any(|pat| pointer_glob(pat, ptr)),
                ..FieldHint::default()
            })
            .collect()
    }

    /// Secret test from the preset's `secretPathPatterns` (glob on pointers).
    pub fn is_secret(&self, preset_id: &str, pointer: &str) -> bool {
        self.get(preset_id).is_some_and(|p| {
            p.ui.secret_path_patterns
                .iter()
                .any(|pat| pointer_glob(pat, pointer))
        })
    }
}

/// Glob over decoded pointer segments: same segment count, `*`/`?` within a
/// segment, case-insensitive.
fn pointer_glob(pattern: &str, pointer: &str) -> bool {
    let (pat, ptr) = (decode_pointer(pattern), decode_pointer(pointer));
    pat.len() == ptr.len()
        && pat
            .iter()
            .zip(&ptr)
            .all(|(p, s)| wildcard(&lower(p), &lower(s)))
}

fn lower(s: &str) -> Vec<char> {
    s.to_lowercase().chars().collect()
}

/// `*` = any run of chars, `?` = one char; iterative with single-star backtracking.
fn wildcard(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) && p[pi] != '*' {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Inputs for Claude discovery; Tauri layer supplies real dirs, tests use temp dirs.
pub struct ClaudeEnv {
    pub home: PathBuf,
    /// `CLAUDE_CONFIG_DIR` if set.
    pub config_dir: Option<PathBuf>,
    /// `/etc/claude-code` on Linux (overridable for tests).
    pub managed_dir: PathBuf,
}

fn candidate(
    path: PathBuf,
    scope: Scope,
    read_only: bool,
    note: Option<&str>,
) -> DiscoveredCandidate {
    let digest = Sha256::digest(format!("{scope:?}\0{}", path.display()).as_bytes());
    let candidate_id = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    DiscoveredCandidate {
        candidate_id,
        exists: path.exists(),
        path: path.display().to_string(),
        scope,
        read_only,
        already_registered: false,
        note: note.map(str::to_owned),
    }
}

/// Candidates: user settings, managed (read-only, incl. `managed-settings.d/*.json`),
/// and for `project_dir`: `.claude/settings.json` (shared) and
/// `.claude/settings.local.json` (local, note: Claude Code ≥ 2.1.211 may read
/// the local file from the git repo root — eng R1). Never lists `~/.claude.json`.
/// `candidate_id` is opaque; the integration layer maps it to the path.
///
/// Missing user/project files are listed with `exists: false` (the UI offers
/// "Create file"). Managed files are listed only when present: they are
/// read-only and never created by the app.
pub fn discover_claude(env: &ClaudeEnv, project_dir: Option<&Path>) -> Vec<DiscoveredCandidate> {
    let user_dir = env
        .config_dir
        .clone()
        .unwrap_or_else(|| env.home.join(".claude"));
    let mut out = vec![candidate(
        user_dir.join("settings.json"),
        Scope::User,
        false,
        None,
    )];

    let managed = |path: PathBuf| {
        candidate(
            path,
            Scope::Managed,
            true,
            Some("Managed policy: read-only."),
        )
    };
    let main = env.managed_dir.join("managed-settings.json");
    if main.is_file() {
        out.push(managed(main));
    }
    let mut drop_ins: Vec<PathBuf> = std::fs::read_dir(env.managed_dir.join("managed-settings.d"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
        .collect();
    drop_ins.sort();
    out.extend(drop_ins.into_iter().map(managed));

    if let Some(dir) = project_dir {
        let claude = dir.join(".claude");
        out.push(candidate(
            claude.join("settings.json"),
            Scope::ProjectShared,
            false,
            None,
        ));
        out.push(candidate(
            claude.join("settings.local.json"),
            Scope::ProjectLocal,
            false,
            Some(LOCAL_NOTE),
        ));
    }
    out
}
