//! Background work: 1 s poll of open documents (design D2) and debounced
//! schema validation delivered as `draft-diagnostics` events (eng R14).

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::state::AppState;
use crate::core::MAX_EDITABLE_BYTES;
use crate::core::persistence;
use crate::core::types::Diagnostic;
use crate::core::watcher::PollOutcome;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const VALIDATION_DEBOUNCE: Duration = Duration::from_millis(150);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExternalChange {
    file_id: String,
    fingerprint: String,
    has_unsaved_changes: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FileRef {
    file_id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AccessChanged {
    file_id: String,
    readable: bool,
    writable: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DraftDiagnostics {
    file_id: String,
    revision: u64,
    diagnostics: Vec<Diagnostic>,
}

pub fn start_poller<R: Runtime>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(POLL_INTERVAL);
            poll_once(&app);
        }
    });
}

fn poll_once<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let mut inner = state.lock();
    let ids: Vec<String> = inner.open.keys().cloned().collect();
    for file_id in ids {
        let Some(doc) = inner.open.get_mut(&file_id) else {
            continue;
        };
        match doc.poller.check() {
            PollOutcome::Unchanged => {}
            PollOutcome::Missing => {
                let _ = app.emit(
                    "managed-file-missing",
                    FileRef {
                        file_id: file_id.clone(),
                    },
                );
            }
            PollOutcome::AccessChanged { readable, writable } => {
                let _ = app.emit(
                    "managed-file-access-changed",
                    AccessChanged {
                        file_id: file_id.clone(),
                        readable,
                        writable,
                    },
                );
            }
            PollOutcome::Changed(fingerprint) => {
                let dirty = doc.session.is_dirty();
                if dirty {
                    // Keep the draft; Save is frozen until the conflict is resolved (spec §8).
                    doc.external_change = Some(fingerprint.clone());
                } else {
                    // No draft: reload in place (A15); UI refetches the snapshot on the event.
                    let Ok(entry) = inner.entry(&file_id) else {
                        continue;
                    };
                    if let Ok(bytes) = persistence::read_checked(
                        std::path::Path::new(&entry.absolute_path),
                        MAX_EDITABLE_BYTES,
                    ) && let Some(doc) = inner.open.get_mut(&file_id)
                    {
                        doc.session = crate::core::document::DocumentSession::load(
                            &file_id, bytes, entry.kind,
                        );
                        doc.poller.set_known(fingerprint.clone());
                    }
                }
                let _ = app.emit(
                    "managed-file-external-change",
                    ExternalChange {
                        file_id: file_id.clone(),
                        fingerprint,
                        has_unsaved_changes: dirty,
                    },
                );
            }
        }
    }
}

/// Validate the draft of `file_id` after a short debounce; results for
/// superseded revisions are dropped here and again in the UI (R14).
pub fn schedule_validation<R: Runtime>(app: &AppHandle<R>, file_id: &str, revision: u64) {
    let app = app.clone();
    let file_id = file_id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(VALIDATION_DEBOUNCE);
        let state = app.state::<AppState>();
        let inner = state.lock();
        let Ok(doc) = inner.doc(&file_id) else { return };
        if doc.session.revision() != revision {
            return;
        }
        let Some(value) = doc.session.to_value() else {
            return;
        };
        let diagnostics = inner.schema_diagnostics(doc, &value);
        drop(inner);
        let _ = app.emit(
            "draft-diagnostics",
            DraftDiagnostics {
                file_id,
                revision,
                diagnostics,
            },
        );
    });
}
