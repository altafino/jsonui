//! Managed-file registry and preferences (store lane). App-owned JSON in the
//! per-user config dir, written atomically with `.bak` (eng R12). Registry
//! holds metadata only, never file contents (spec §8).

use std::path::{Path, PathBuf};

use crate::core::types::{DocKind, ManagedDocument, MetadataPatch, Notice, Preferences, Scope};
use crate::core::CoreResult;

pub struct Registry {
    _private: (),
}

impl Registry {
    /// Load `<config_dir>/registry.json`. An unreadable file is moved to
    /// `registry.corrupt-<timestamp>.json`, `.bak` is tried, else empty; a
    /// Notice describes what happened (eng R12). Never fails on bad content.
    pub fn load(config_dir: &Path) -> (Self, Vec<Notice>) {
        let _ = config_dir;
        todo!("store lane")
    }

    pub fn list(&self) -> Vec<ManagedDocument> {
        todo!("store lane")
    }

    pub fn get(&self, id: &str) -> CoreResult<ManagedDocument> {
        let _ = id;
        todo!("store lane")
    }

    /// Register an existing file. Kind from extension (`.jsonc` → Jsonc).
    /// Same canonical path twice returns the existing entry.
    pub fn add(&mut self, path: &Path, display_name: Option<String>, scope: Option<Scope>, preset_id: Option<String>) -> CoreResult<ManagedDocument> {
        let _ = (path, display_name, scope, preset_id);
        todo!("store lane")
    }

    /// Create a new file containing `{}\n` (refuses to overwrite) and register it.
    pub fn create_new(&mut self, path: &Path) -> CoreResult<ManagedDocument> {
        let _ = path;
        todo!("store lane")
    }

    /// Copy `sample_bytes` into `<data_dir>/samples/` and register it as
    /// `EntryKind::Sample` (design DD7).
    pub fn create_sample(&mut self, data_dir: &Path, sample_bytes: &[u8]) -> CoreResult<ManagedDocument> {
        let _ = (data_dir, sample_bytes);
        todo!("store lane")
    }

    /// Unregister. Deletes the file only for `EntryKind::Sample` whose
    /// canonical path is inside `data_dir` (correction C2); otherwise never.
    pub fn remove(&mut self, id: &str, data_dir: &Path) -> CoreResult<()> {
        let _ = (id, data_dir);
        todo!("store lane")
    }

    pub fn update_metadata(&mut self, id: &str, patch: MetadataPatch) -> CoreResult<ManagedDocument> {
        let _ = (id, patch);
        todo!("store lane")
    }

    pub fn relocate(&mut self, id: &str, new_path: &Path) -> CoreResult<ManagedDocument> {
        let _ = (id, new_path);
        todo!("store lane")
    }

    pub fn set_schema(&mut self, id: &str, schema_path: Option<PathBuf>) -> CoreResult<ManagedDocument> {
        let _ = (id, schema_path);
        todo!("store lane")
    }

    pub fn set_link_confirmed(&mut self, id: &str, symlink: bool, hardlink: bool) -> CoreResult<ManagedDocument> {
        let _ = (id, symlink, hardlink);
        todo!("store lane")
    }

    pub fn touch_opened(&mut self, id: &str) -> CoreResult<()> {
        let _ = id;
        todo!("store lane")
    }

    pub fn kind_for(path: &Path) -> DocKind {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("jsonc") => DocKind::Jsonc,
            _ => DocKind::Json,
        }
    }

    pub fn preferences(&self) -> Preferences {
        todo!("store lane")
    }

    pub fn set_preferences(&mut self, prefs: Preferences) -> CoreResult<()> {
        let _ = prefs;
        todo!("store lane")
    }
}
