//! Registry, discovery, pickers, preferences and OS actions.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;

use super::picker::{PickKind, pick};
use super::state::AppState;
use crate::core::persistence;
use crate::core::presets::{ClaudeEnv, discover_claude};
use crate::core::security::paths::resolve_target;
use crate::core::types::{
    DiscoveredCandidate, FileStatus, ManagedDocument, MetadataPatch, Notice, Preferences,
};
use crate::core::{CoreError, CoreResult, ErrorCode};

const CLAUDE_PRESET: &str = "claude-code-settings";
const SAMPLE_BYTES: &[u8] = include_bytes!("../../../fixtures/feature-flags.json");

#[tauri::command]
pub fn list_managed_files(state: State<AppState>) -> Vec<ManagedDocument> {
    state.lock().registry.list()
}

#[tauri::command]
pub fn get_startup_notices(state: State<AppState>) -> Vec<Notice> {
    std::mem::take(&mut state.lock().notices)
}

#[tauri::command]
pub fn get_file_statuses(state: State<AppState>) -> Vec<FileStatus> {
    state
        .lock()
        .registry
        .list()
        .iter()
        .map(file_status)
        .collect()
}

fn file_status(doc: &ManagedDocument) -> FileStatus {
    let path = Path::new(&doc.absolute_path);
    let resolved = resolve_target(path).ok();
    let meta = resolved
        .as_ref()
        .and_then(|r| std::fs::metadata(&r.target_path).ok());
    FileStatus {
        file_id: doc.id.clone(),
        exists: meta.is_some(),
        readable: resolved
            .as_ref()
            .map(|r| std::fs::File::open(&r.target_path).is_ok())
            .unwrap_or(false),
        // ponytail: permission bits only; ownership/ACLs are caught by the save path.
        writable: meta
            .as_ref()
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false),
        is_symlink: resolved.as_ref().map(|r| r.is_symlink).unwrap_or(false),
        link_target: resolved
            .as_ref()
            .filter(|r| r.is_symlink)
            .map(|r| r.target_path.display().to_string()),
        hardlink_count: resolved
            .as_ref()
            .map(|r| r.hardlink_count as u32)
            .unwrap_or(1),
        size_bytes: meta.map(|m| m.len()),
    }
}

#[tauri::command]
pub async fn pick_and_add_managed_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    display_name: Option<String>,
) -> CoreResult<Option<ManagedDocument>> {
    let Some(path) = pick(&app, PickKind::OpenJson) else {
        return Ok(None);
    };
    let mut inner = state.lock();
    let preset = inner.presets.detect(&path, None);
    inner
        .registry
        .add(&path, display_name, None, preset)
        .map(Some)
}

#[tauri::command]
pub async fn pick_and_create_managed_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CoreResult<Option<ManagedDocument>> {
    let Some(path) = pick(
        &app,
        PickKind::SaveJson {
            suggested_name: "settings.json".into(),
        },
    ) else {
        return Ok(None);
    };
    state.lock().registry.create_new(&path).map(Some)
}

#[tauri::command]
pub fn create_sample_file(state: State<AppState>) -> CoreResult<ManagedDocument> {
    let mut inner = state.lock();
    let data_dir = inner.data_dir.clone();
    inner.registry.create_sample(&data_dir, SAMPLE_BYTES)
}

#[tauri::command]
pub fn remove_managed_file(state: State<AppState>, file_id: String) -> CoreResult<()> {
    let mut inner = state.lock();
    inner.open.remove(&file_id);
    let data_dir = inner.data_dir.clone();
    inner.registry.remove(&file_id, &data_dir)
}

#[tauri::command]
pub fn update_managed_file_metadata(
    state: State<AppState>,
    file_id: String,
    patch: MetadataPatch,
) -> CoreResult<ManagedDocument> {
    state.lock().registry.update_metadata(&file_id, patch)
}

#[tauri::command]
pub async fn relocate_managed_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    file_id: String,
) -> CoreResult<Option<ManagedDocument>> {
    state.lock().entry(&file_id)?;
    let Some(path) = pick(&app, PickKind::OpenJson) else {
        return Ok(None);
    };
    let mut inner = state.lock();
    inner.open.remove(&file_id);
    inner.registry.relocate(&file_id, &path).map(Some)
}

/// One-time confirmation for writing through symlinks / detaching hard links (eng R5).
#[tauri::command]
pub fn confirm_link_write(state: State<AppState>, file_id: String) -> CoreResult<ManagedDocument> {
    state
        .lock()
        .registry
        .set_link_confirmed(&file_id, true, true)
}

fn claude_env() -> ClaudeEnv {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    ClaudeEnv {
        home,
        config_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        managed_dir: PathBuf::from(if cfg!(target_os = "macos") {
            "/Library/Application Support/ClaudeCode"
        } else if cfg!(windows) {
            r"C:\Program Files\ClaudeCode"
        } else {
            "/etc/claude-code"
        }),
    }
}

fn remember_candidates(
    state: &AppState,
    mut found: Vec<DiscoveredCandidate>,
) -> Vec<DiscoveredCandidate> {
    let mut inner = state.lock();
    let registered: Vec<String> = inner
        .registry
        .list()
        .into_iter()
        .map(|d| d.absolute_path)
        .collect();
    for c in &mut found {
        c.already_registered = registered.iter().any(|p| p == &c.path);
        inner
            .candidates
            .insert(c.candidate_id.clone(), (PathBuf::from(&c.path), c.scope));
    }
    found
}

#[tauri::command]
pub fn list_discoverable_claude_files(state: State<AppState>) -> Vec<DiscoveredCandidate> {
    remember_candidates(&state, discover_claude(&claude_env(), None))
}

#[tauri::command]
pub async fn pick_project_directory<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CoreResult<Option<Vec<DiscoveredCandidate>>> {
    let Some(dir) = pick(&app, PickKind::Folder) else {
        return Ok(None);
    };
    let project_only = discover_claude(&claude_env(), Some(&dir))
        .into_iter()
        .filter(|c| {
            matches!(
                c.scope,
                crate::core::types::Scope::ProjectShared | crate::core::types::Scope::ProjectLocal
            )
        })
        .collect();
    Ok(Some(remember_candidates(&state, project_only)))
}

#[tauri::command]
pub fn add_discovered_file(
    state: State<AppState>,
    candidate_id: String,
) -> CoreResult<ManagedDocument> {
    let mut inner = state.lock();
    let (path, scope) = inner
        .candidates
        .get(&candidate_id)
        .cloned()
        .ok_or_else(|| {
            CoreError::new(ErrorCode::NotFound, "Unknown discovery candidate")
                .with_remediation("Run discovery again.")
        })?;
    if !path.exists() {
        // Explicit "Create file" for a missing candidate (design DD5); never overwrites.
        let create = || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            std::io::Write::write_all(&mut f, b"{}\n")?;
            f.sync_all()
        };
        create().map_err(|e| CoreError::from_io(&e, &path))?;
    }
    // Every discovery candidate is a Claude Code settings file, including a
    // relocated CLAUDE_CONFIG_DIR that path-based detection cannot recognize.
    // Registry::add marks Scope::Managed entries read-only (spec §7, A26).
    inner
        .registry
        .add(&path, None, Some(scope), Some(CLAUDE_PRESET.into()))
}

#[tauri::command]
pub async fn pick_schema<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    file_id: String,
) -> CoreResult<Option<ManagedDocument>> {
    state.lock().entry(&file_id)?;
    let Some(path) = pick(&app, PickKind::Schema) else {
        return Ok(None);
    };
    // Validate it compiles before associating; an invalid schema is reported, not stored.
    crate::core::schema::CompiledSchema::load_user(&path)?;
    let mut inner = state.lock();
    inner.open.remove(&file_id);
    inner.registry.set_schema(&file_id, Some(path)).map(Some)
}

#[tauri::command]
pub fn clear_schema(state: State<AppState>, file_id: String) -> CoreResult<ManagedDocument> {
    let mut inner = state.lock();
    inner.open.remove(&file_id);
    inner.registry.set_schema(&file_id, None)
}

#[tauri::command]
pub fn reveal_managed_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<AppState>,
    file_id: String,
) -> CoreResult<()> {
    let path = state.lock().entry(&file_id)?.absolute_path;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| CoreError::new(ErrorCode::Io, e.to_string()).with_path(path))
}

#[tauri::command]
pub fn open_in_external_editor<R: Runtime>(
    app: AppHandle<R>,
    state: State<AppState>,
    file_id: String,
) -> CoreResult<()> {
    let path = state.lock().entry(&file_id)?.absolute_path;
    app.opener()
        .open_path(path.clone(), None::<&str>)
        .map_err(|e| CoreError::new(ErrorCode::Io, e.to_string()).with_path(path))
}

#[tauri::command]
pub fn list_leftover_temp_files(
    state: State<AppState>,
    file_id: String,
) -> CoreResult<Vec<String>> {
    let path = state.lock().entry(&file_id)?.absolute_path;
    Ok(persistence::find_leftovers(Path::new(&path))
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect())
}

#[tauri::command]
pub fn remove_leftover_temp_files(state: State<AppState>, file_id: String) -> CoreResult<u32> {
    let path = state.lock().entry(&file_id)?.absolute_path;
    persistence::remove_leftovers(Path::new(&path))
}

#[tauri::command]
pub fn get_preferences(state: State<AppState>) -> Preferences {
    state.lock().registry.preferences()
}

#[tauri::command]
pub fn set_preferences(state: State<AppState>, prefs: Preferences) -> CoreResult<()> {
    let mut inner = state.lock();
    inner.backups.retention = prefs.backup_retention.max(1);
    inner.registry.set_preferences(prefs)
}

/// App-owned directories (spec §8 persistence location).
pub fn app_dirs<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    Ok((app.path().app_config_dir()?, app.path().app_data_dir()?))
}
