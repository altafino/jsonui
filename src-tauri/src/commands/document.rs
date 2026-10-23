//! Document commands: open, edit, review, save, conflicts, backups.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime, State};

use super::background::schedule_validation;
use super::picker::{pick, PickKind};
use super::state::{AppState, Inner, OpenDoc};
use crate::core::document::DocumentSession;
use crate::core::persistence::{self, LinkConfirmations, SaveRequest};
use crate::core::schema::CompiledSchema;
use crate::core::types::{
    BackupEntry, ChangeSummary, ConflictInfo, DiagnosticSource, DocumentSnapshot, DraftOp, EditResult, FieldHint,
    ManagedDocument, PresetInfo, SaveResult, Severity, TreeNode,
};
use crate::core::watcher::FilePoller;
use crate::core::{CoreError, CoreResult, ErrorCode, MAX_EDITABLE_BYTES};

/// Current UTC time as RFC 3339 (`2026-10-23T14:02:05Z`).
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (H. Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Load (or reload) a document from disk into the open set.
fn load_document(inner: &mut Inner, file_id: &str) -> CoreResult<DocumentSnapshot> {
    let entry = inner.entry(file_id)?;
    let path = PathBuf::from(&entry.absolute_path);
    let bytes = persistence::read_checked(&path, MAX_EDITABLE_BYTES)?;
    let known = persistence::fingerprint(&bytes);
    let session = DocumentSession::load(file_id, bytes, entry.kind);
    let preset_id = entry.preset_id.clone().or_else(|| inner.presets.detect(&path, session.to_value().as_ref()));
    let user_schema = entry.local_schema_path.as_deref().and_then(|p| CompiledSchema::load_user(Path::new(p)).ok());
    let read_only = entry.read_only;
    inner.open.insert(
        file_id.to_string(),
        OpenDoc { session, poller: FilePoller::new(&path, known), external_change: None, user_schema, preset_id },
    );
    let _ = inner.registry.touch_opened(file_id);
    let doc = inner.doc(file_id)?;
    let mut snapshot = doc.session.snapshot(read_only);
    if entry.local_schema_path.is_some() && doc.user_schema.is_none() {
        snapshot.diagnostics.push(crate::core::types::Diagnostic {
            severity: Severity::Warning,
            source: DiagnosticSource::UserSchema,
            code: "SCHEMA_UNAVAILABLE".into(),
            message: "The associated schema could not be loaded; generic editing is still available.".into(),
            pointer: None,
            line: None,
            column: None,
        });
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn open_document<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String) -> CoreResult<DocumentSnapshot> {
    let snapshot = load_document(&mut state.lock(), &file_id)?;
    schedule_validation(&app, &file_id, snapshot.revision);
    Ok(snapshot)
}

#[tauri::command]
pub fn reload_document<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String) -> CoreResult<DocumentSnapshot> {
    open_document(app, state, file_id)
}

#[tauri::command]
pub fn get_subtree(state: State<AppState>, file_id: String, pointer: String, depth: u32) -> CoreResult<TreeNode> {
    state.lock().doc(&file_id)?.session.subtree(&pointer, depth.min(8))
}

#[tauri::command]
pub fn get_field_hints(state: State<AppState>, file_id: String, pointers: Vec<String>) -> CoreResult<Vec<FieldHint>> {
    let inner = state.lock();
    let doc = inner.doc(&file_id)?;
    let Some(value) = doc.session.to_value() else { return Ok(Vec::new()) };
    let mut hints: Vec<FieldHint> = match &doc.user_schema {
        Some(schema) => schema.hints(&value, &pointers),
        None => match doc.preset_id.as_deref().and_then(|id| inner.presets.schema(id)) {
            Some(schema) => schema.hints(&value, &pointers),
            None => pointers.iter().map(|p| FieldHint { pointer: p.clone(), ..Default::default() }).collect(),
        },
    };
    if let Some(id) = doc.preset_id.as_deref() {
        // Preset UI hints fill gaps the schema leaves (descriptions, itemSummary, sensitive, secret).
        for ui in inner.presets.ui_hints(id, &pointers) {
            if let Some(h) = hints.iter_mut().find(|h| h.pointer == ui.pointer) {
                h.description = h.description.take().or(ui.description);
                h.item_summary = h.item_summary.take().or(ui.item_summary);
                h.sensitive |= ui.sensitive;
                h.secret |= ui.secret;
                if h.title.is_none() {
                    h.title = ui.title;
                }
            }
        }
    }
    Ok(hints)
}

#[tauri::command]
pub fn get_preset_info(state: State<AppState>, file_id: String) -> CoreResult<Option<PresetInfo>> {
    let inner = state.lock();
    let preset = inner.doc(&file_id)?.preset_id.clone();
    Ok(preset.and_then(|id| inner.presets.info(&id)))
}

fn edit<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    file_id: &str,
    f: impl FnOnce(&mut DocumentSession) -> CoreResult<EditResult>,
) -> CoreResult<EditResult> {
    let mut inner = state.lock();
    if inner.entry(file_id)?.read_only {
        return Err(CoreError::new(ErrorCode::ReadOnly, "This file is read-only in ConfigDeck"));
    }
    let result = f(&mut inner.doc_mut(file_id)?.session)?;
    drop(inner);
    schedule_validation(app, file_id, result.revision);
    Ok(result)
}

#[tauri::command]
pub fn apply_draft_operation<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String, op: DraftOp) -> CoreResult<EditResult> {
    edit(&app, &state, &file_id, |s| s.apply(op))
}

#[tauri::command]
pub fn undo_draft_operation<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String) -> CoreResult<EditResult> {
    edit(&app, &state, &file_id, |s| s.undo())
}

#[tauri::command]
pub fn redo_draft_operation<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String) -> CoreResult<EditResult> {
    edit(&app, &state, &file_id, |s| s.redo())
}

#[tauri::command]
pub fn revert_change<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String, op_index: u32) -> CoreResult<EditResult> {
    edit(&app, &state, &file_id, |s| s.revert_op(op_index))
}

#[tauri::command]
pub fn discard_draft(state: State<AppState>, file_id: String) -> CoreResult<DocumentSnapshot> {
    let mut inner = state.lock();
    let read_only = inner.entry(&file_id)?.read_only;
    let doc = inner.doc_mut(&file_id)?;
    doc.session.discard();
    Ok(doc.session.snapshot(read_only))
}

/// Review payload with schema results split into warnings (preset) and errors.
fn summary_for(inner: &Inner, file_id: &str) -> CoreResult<ChangeSummary> {
    let doc = inner.doc(file_id)?;
    let is_secret = inner.secret_predicate(doc.preset_id.clone());
    let mut summary = doc.session.review(&is_secret);
    summary.errors.extend(doc.session.diagnostics().into_iter().filter(|d| d.severity == Severity::Error));
    if let Some(value) = doc.session.to_value() {
        for d in inner.schema_diagnostics(doc, &value) {
            match d.severity {
                Severity::Error => summary.errors.push(d),
                _ => summary.warnings.push(d),
            }
        }
    }
    Ok(summary)
}

#[tauri::command]
pub fn preview_draft(state: State<AppState>, file_id: String) -> CoreResult<ChangeSummary> {
    summary_for(&state.lock(), &file_id)
}

fn link_confirmations(entry: &ManagedDocument) -> LinkConfirmations {
    LinkConfirmations { symlink: entry.link_write_confirmed, hardlink: entry.hardlink_write_confirmed }
}

/// Validator for candidate bytes (spec §10 steps 6-7): reparse in the right
/// mode, then schema (user errors block; preset warnings need confirmation).
fn validator<'a>(inner: &'a Inner, file_id: &'a str, kind: crate::core::types::DocKind, accept_warnings: bool) -> impl Fn(&[u8]) -> CoreResult<()> + 'a {
    move |candidate: &[u8]| {
        let check = DocumentSession::load(file_id, candidate.to_vec(), kind);
        if !check.is_editable() {
            return Err(CoreError::new(ErrorCode::InvalidJson, "The edited document would not be valid; nothing was written"));
        }
        let doc = inner.doc(file_id)?;
        if let Some(value) = check.to_value() {
            let diags = inner.schema_diagnostics(doc, &value);
            let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
            let warnings = diags.len() - errors;
            if errors > 0 {
                return Err(CoreError::new(ErrorCode::SchemaValidationFailed, format!("{errors} schema error(s) must be fixed before saving")));
            }
            if warnings > 0 && !accept_warnings {
                return Err(CoreError::new(ErrorCode::PresetWarningsRequireConfirmation, format!("{warnings} preset warning(s)")));
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub fn save_draft(state: State<AppState>, file_id: String, expected_fingerprint: String, accept_warnings: bool) -> CoreResult<SaveResult> {
    let mut inner = state.lock();
    let entry = inner.entry(&file_id)?;
    if entry.read_only {
        return Err(CoreError::new(ErrorCode::ReadOnly, "This file is read-only in ConfigDeck"));
    }
    let doc = inner.doc(&file_id)?;
    if !doc.session.is_dirty() {
        return Err(CoreError::new(ErrorCode::InvalidOperation, "No unsaved changes"));
    }
    let candidate = doc.session.candidate_bytes();
    let change_count = doc.session.ops().len() as u32;
    let outcome = {
        let validate = validator(&inner, &file_id, entry.kind, accept_warnings);
        persistence::save(&SaveRequest {
            file_id: &file_id,
            path: Path::new(&entry.absolute_path),
            expected: &expected_fingerprint,
            candidate: &candidate,
            confirmations: link_confirmations(&entry),
            validate: &validate,
            backups: &inner.backups,
            change_count: Some(change_count),
        })?
    };
    let doc = inner.doc_mut(&file_id)?;
    doc.session.adopt_saved(candidate);
    doc.poller.set_known(outcome.fingerprint.clone());
    doc.external_change = None;
    Ok(SaveResult { fingerprint: outcome.fingerprint, backup_id: Some(outcome.backup_id), saved_at: now_rfc3339() })
}

fn disk_bytes(inner: &Inner, file_id: &str) -> CoreResult<Vec<u8>> {
    let entry = inner.entry(file_id)?;
    persistence::read_checked(Path::new(&entry.absolute_path), MAX_EDITABLE_BYTES)
}

#[tauri::command]
pub fn get_conflict(state: State<AppState>, file_id: String) -> CoreResult<ConflictInfo> {
    let inner = state.lock();
    let disk = disk_bytes(&inner, &file_id)?;
    let doc = inner.doc(&file_id)?;
    let is_secret = inner.secret_predicate(doc.preset_id.clone());
    Ok(doc.session.conflict(&disk, &is_secret))
}

#[tauri::command]
pub fn reapply_draft<R: Runtime>(app: AppHandle<R>, state: State<AppState>, file_id: String) -> CoreResult<EditResult> {
    let mut inner = state.lock();
    let disk = disk_bytes(&inner, &file_id)?;
    let fp = persistence::fingerprint(&disk);
    let doc = inner.doc_mut(&file_id)?;
    let result = doc.session.reapply_onto(disk)?;
    doc.poller.set_known(fp);
    doc.external_change = None;
    drop(inner);
    schedule_validation(&app, &file_id, result.revision);
    Ok(result)
}

#[tauri::command]
pub async fn save_draft_as_copy<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, file_id: String) -> CoreResult<Option<ManagedDocument>> {
    let (candidate, name) = {
        let inner = state.lock();
        let entry = inner.entry(&file_id)?;
        let name = Path::new(&entry.absolute_path)
            .file_name()
            .map(|n| format!("copy-of-{}", n.to_string_lossy()))
            .unwrap_or_else(|| "copy.json".into());
        (inner.doc(&file_id)?.session.candidate_bytes(), name)
    };
    let Some(path) = pick(&app, PickKind::SaveJson { suggested_name: name }) else { return Ok(None) };
    persistence::atomic_write(&path, &candidate, false)?;
    let mut inner = state.lock();
    let preset = inner.presets.detect(&path, None);
    inner.registry.add(&path, None, None, preset).map(Some)
}

#[tauri::command]
pub fn list_backups(state: State<AppState>, file_id: String) -> CoreResult<Vec<BackupEntry>> {
    state.lock().backups.list(&file_id)
}

#[tauri::command]
pub fn preview_restore(state: State<AppState>, file_id: String, backup_id: String) -> CoreResult<ChangeSummary> {
    let inner = state.lock();
    let backup = inner.backups.read(&file_id, &backup_id)?;
    let doc = inner.doc(&file_id)?;
    let is_secret = inner.secret_predicate(doc.preset_id.clone());
    // Current file vs backup, expressed as the "disk side" of a conflict diff.
    let current = DocumentSession::load(&file_id, doc.session.original_bytes().to_vec(), inner.entry(&file_id)?.kind);
    let diff = current.conflict(&backup, &is_secret);
    Ok(ChangeSummary { changes: diff.disk_changes, hunks: Vec::new(), warnings: Vec::new(), errors: Vec::new() })
}

#[tauri::command]
pub fn restore_backup(state: State<AppState>, file_id: String, backup_id: String, expected_fingerprint: String) -> CoreResult<SaveResult> {
    let mut inner = state.lock();
    let entry = inner.entry(&file_id)?;
    if entry.read_only {
        return Err(CoreError::new(ErrorCode::ReadOnly, "This file is read-only in ConfigDeck"));
    }
    if inner.doc(&file_id)?.session.is_dirty() {
        return Err(CoreError::new(ErrorCode::InvalidOperation, "Save or discard your unsaved changes before restoring"));
    }
    let backup = inner.backups.read(&file_id, &backup_id)?;
    let outcome = {
        // Restored bytes are a past version of this file: reparse only, no
        // schema gate (spec §10: same safety rules, not a new edit).
        let kind = entry.kind;
        let id = file_id.as_str();
        let validate = move |bytes: &[u8]| {
            if DocumentSession::load(id, bytes.to_vec(), kind).is_editable() {
                Ok(())
            } else {
                Err(CoreError::new(ErrorCode::InvalidJson, "The backup is not valid; nothing was written"))
            }
        };
        persistence::save(&SaveRequest {
            file_id: &file_id,
            path: Path::new(&entry.absolute_path),
            expected: &expected_fingerprint,
            candidate: &backup,
            confirmations: link_confirmations(&entry),
            validate: &validate,
            backups: &inner.backups,
            change_count: None,
        })?
    };
    let doc = inner.doc_mut(&file_id)?;
    doc.session.adopt_saved(backup);
    doc.poller.set_known(outcome.fingerprint.clone());
    doc.external_change = None;
    Ok(SaveResult { fingerprint: outcome.fingerprint, backup_id: Some(outcome.backup_id), saved_at: now_rfc3339() })
}

#[tauri::command]
pub fn reveal_masked_value(state: State<AppState>, file_id: String, mask_id: u32) -> CoreResult<String> {
    state.lock().doc(&file_id)?.session.reveal_masked(mask_id)
}
