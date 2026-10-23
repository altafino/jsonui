//! App state shared by commands and the background poller.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::core::backup::BackupStore;
use crate::core::document::DocumentSession;
use crate::core::presets::PresetRegistry;
use crate::core::registry::Registry;
use crate::core::schema::{CompiledSchema, SchemaOrigin};
use crate::core::types::{Fingerprint, ManagedDocument, Notice, Scope};
use crate::core::watcher::FilePoller;
use crate::core::{CoreError, CoreResult, ErrorCode};

pub struct OpenDoc {
    pub session: DocumentSession,
    pub poller: FilePoller,
    /// Fingerprint of an external change seen while a draft existed.
    pub external_change: Option<Fingerprint>,
    /// User schema compiled for this document, if associated.
    pub user_schema: Option<CompiledSchema>,
    pub preset_id: Option<String>,
    /// Canonical file the registered path resolved to at load (eng R5).
    pub target: Option<std::path::PathBuf>,
}

pub struct Inner {
    pub registry: Registry,
    pub notices: Vec<Notice>,
    pub open: HashMap<String, OpenDoc>,
    pub presets: PresetRegistry,
    /// Rust-issued discovery candidate ids → (path, scope) (eng R6).
    pub candidates: HashMap<String, (PathBuf, Scope)>,
    pub backups: BackupStore,
    pub data_dir: PathBuf,
}

pub struct AppState(pub Mutex<Inner>);

impl AppState {
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned lock means a panic mid-command; the data is still the
        // last consistent registry/session state, so keep serving.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Inner {
    pub fn entry(&self, file_id: &str) -> CoreResult<ManagedDocument> {
        self.registry.get(file_id)
    }

    pub fn doc_mut(&mut self, file_id: &str) -> CoreResult<&mut OpenDoc> {
        self.open
            .get_mut(file_id)
            .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "Document is not open").with_remediation("Open the file first."))
    }

    pub fn doc(&self, file_id: &str) -> CoreResult<&OpenDoc> {
        self.open
            .get(file_id)
            .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "Document is not open").with_remediation("Open the file first."))
    }

    /// Secret test for diffs: preset patterns plus the default key patterns
    /// (applied inside the document engine).
    pub fn secret_predicate(&self, preset_id: Option<String>) -> impl Fn(&str) -> bool + '_ {
        move |pointer: &str| match &preset_id {
            Some(id) => self.presets.is_secret(id, pointer),
            None => false,
        }
    }

    /// Schema diagnostics for a value: user schema (errors) and preset (warnings).
    pub fn schema_diagnostics(&self, doc: &OpenDoc, value: &serde_json::Value) -> Vec<crate::core::types::Diagnostic> {
        let mut out = Vec::new();
        if let Some(schema) = &doc.user_schema {
            debug_assert_eq!(schema.origin(), SchemaOrigin::User);
            out.extend(schema.validate(value));
        }
        if let Some(schema) = doc.preset_id.as_deref().and_then(|id| self.presets.schema(id)) {
            out.extend(schema.validate(value));
        }
        out
    }
}
