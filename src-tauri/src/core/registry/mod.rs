//! Managed-file registry and preferences (store lane). App-owned JSON in the
//! per-user config dir, written atomically with `.bak` (eng R12). Registry
//! holds metadata only, never file contents (spec §8).
//!
//! Storage: one file, `<config_dir>/registry.json`, holding
//! `{ "version": 1, "entries": [...], "preferences": {...} }`. Preferences
//! share the file (and so its atomic write, `.bak` and recovery) instead of
//! a separate `preferences.json`.
//!
//! Every mutation is transactional: it is applied to a copy, persisted, and
//! only then adopted in memory, so a failed write never leaves the in-memory
//! registry ahead of the disk.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::persistence::{Stamp, atomic_write, bak_path, now_rfc3339};
use crate::core::security::paths::{is_within, resolve_target};
use crate::core::types::{
    DocKind, EntryKind, ManagedDocument, MetadataPatch, Notice, Preferences, Scope,
};
use crate::core::{CoreError, CoreResult, ErrorCode};

/// Preset id of the built-in Claude Code preset; its files default to the
/// `claude-code` sidebar collection.
const CLAUDE_PRESET_ID: &str = "claude-code-settings";
const CLAUDE_COLLECTION_ID: &str = "claude-code";
const SAMPLE_COLLECTION_ID: &str = "sample";
const SAMPLE_FILE_NAME: &str = "feature-flags.json";

#[derive(Clone, Serialize, Deserialize)]
struct State {
    version: u32,
    entries: Vec<ManagedDocument>,
    #[serde(default)]
    preferences: Preferences,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
            preferences: Preferences::default(),
        }
    }
}

pub struct Registry {
    file: PathBuf,
    state: State,
}

fn parse(bytes: &[u8]) -> Option<State> {
    serde_json::from_slice(bytes).ok()
}

fn not_found(id: &str) -> CoreError {
    CoreError::new(
        ErrorCode::NotFound,
        format!("No managed file with id {id}."),
    )
}

fn invalid(message: &str) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, message)
}

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

impl Registry {
    /// Load `<config_dir>/registry.json`. An unreadable file is moved to
    /// `registry.corrupt-<timestamp>.json`, `.bak` is tried, else empty; a
    /// Notice describes what happened (eng R12). Never fails on bad content.
    pub fn load(config_dir: &Path) -> (Self, Vec<Notice>) {
        let file = config_dir.join("registry.json");
        let mut notices = Vec::new();
        let bak = bak_path(&file);
        let state = match fs::read(&file) {
            Ok(bytes) => match parse(&bytes) {
                Some(state) => state,
                None => Self::recover(&file, &bak, true, &mut notices),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // A missing registry with a surviving .bak is a recovery, not a fresh start.
                if bak.exists() {
                    Self::recover(&file, &bak, false, &mut notices)
                } else {
                    State::default()
                }
            }
            Err(_) => Self::recover(&file, &bak, true, &mut notices),
        };
        (Self { file, state }, notices)
    }

    fn recover(file: &Path, bak: &Path, set_aside: bool, notices: &mut Vec<Notice>) -> State {
        let mut aside_note = String::new();
        if set_aside {
            let aside =
                file.with_file_name(format!("registry.corrupt-{}.json", Stamp::now().compact()));
            match fs::rename(file, &aside) {
                Ok(()) => {
                    aside_note = format!(" The unreadable file was kept as {}.", aside.display())
                }
                Err(e) => {
                    aside_note = format!(" The unreadable file could not be moved aside ({e}).")
                }
            }
        }
        let restored = fs::read(bak).ok().and_then(|b| parse(&b).map(|s| (s, b)));
        let (code, message, state) = if let Some((state, bytes)) = restored {
            // Put a valid main file back so a crash before the next save cannot lose the list.
            let _ = atomic_write(file, &bytes, false);
            (
                "registry_recovered",
                "ConfigDeck's file list was damaged; the previous version was restored.",
                state,
            )
        } else {
            (
                "registry_reset",
                "ConfigDeck's file list was damaged and no previous version was usable; starting with an empty list. Your config files were not touched.",
                State::default(),
            )
        };
        notices.push(Notice {
            code: code.to_string(),
            message: format!("{message}{aside_note}"),
            path: Some(lossy(file)),
        });
        state
    }

    /// Apply `f` to a copy of the state, persist it, then adopt it.
    fn commit<T>(&mut self, f: impl FnOnce(&mut State) -> CoreResult<T>) -> CoreResult<T> {
        let mut next = self.state.clone();
        let out = f(&mut next)?;
        let json = serde_json::to_vec_pretty(&next)
            .map_err(|e| CoreError::new(ErrorCode::Internal, e.to_string()))?;
        atomic_write(&self.file, &json, true)?;
        self.state = next;
        Ok(out)
    }

    fn entry_mut<'a>(state: &'a mut State, id: &str) -> CoreResult<&'a mut ManagedDocument> {
        state
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| not_found(id))
    }

    pub fn list(&self) -> Vec<ManagedDocument> {
        self.state.entries.clone()
    }

    pub fn get(&self, id: &str) -> CoreResult<ManagedDocument> {
        self.state
            .entries
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| not_found(id))
    }

    /// Canonical path of an entry for duplicate detection; the stored path if it cannot be resolved.
    fn canonical(path: &str) -> PathBuf {
        fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path))
    }

    fn new_entry(path: &Path, display_name: String, entry_kind: EntryKind) -> ManagedDocument {
        ManagedDocument {
            id: uuid::Uuid::new_v4().to_string(),
            display_name,
            absolute_path: lossy(&absolute(path)),
            kind: Self::kind_for(path),
            entry_kind,
            preset_id: None,
            local_schema_path: None,
            collection_id: None,
            discovered_scope: None,
            read_only: false,
            link_write_confirmed: false,
            hardlink_write_confirmed: false,
            last_opened_at: None,
        }
    }

    /// Register an existing file. Kind from extension (`.jsonc` → Jsonc).
    /// Same canonical path twice returns the existing entry.
    ///
    /// `Scope::Managed` entries are policy read-only (spec §7, A26). A
    /// `claude-code-settings` preset defaults the collection to `claude-code`.
    pub fn add(
        &mut self,
        path: &Path,
        display_name: Option<String>,
        scope: Option<Scope>,
        preset_id: Option<String>,
    ) -> CoreResult<ManagedDocument> {
        let canonical = resolve_target(path)?.target_path;
        if let Some(existing) = self
            .state
            .entries
            .iter()
            .find(|e| Self::canonical(&e.absolute_path) == canonical)
        {
            return Ok(existing.clone());
        }
        let name = display_name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Untitled".to_string());
        let mut entry = Self::new_entry(path, name, EntryKind::File);
        entry.read_only = scope == Some(Scope::Managed);
        entry.discovered_scope = scope;
        if preset_id.as_deref() == Some(CLAUDE_PRESET_ID) {
            entry.collection_id = Some(CLAUDE_COLLECTION_ID.to_string());
        }
        entry.preset_id = preset_id;
        self.commit(|s| {
            s.entries.push(entry.clone());
            Ok(entry)
        })
    }

    /// Create a new file containing `{}\n` (refuses to overwrite) and register it.
    pub fn create_new(&mut self, path: &Path) -> CoreResult<ManagedDocument> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    invalid("A file already exists at that path; ConfigDeck will not overwrite it.")
                        .with_path(lossy(path))
                } else {
                    CoreError::from_io(&e, path)
                }
            })?;
        file.write_all(b"{}\n")
            .and_then(|()| file.sync_all())
            .map_err(|e| CoreError::from_io(&e, path))?;
        drop(file);
        self.add(path, None, None, None)
    }

    /// Copy `sample_bytes` into `<data_dir>/samples/` and register it as
    /// `EntryKind::Sample` (design DD7).
    pub fn create_sample(
        &mut self,
        data_dir: &Path,
        sample_bytes: &[u8],
    ) -> CoreResult<ManagedDocument> {
        use std::io::Write;
        let dir = data_dir.join("samples");
        fs::create_dir_all(&dir).map_err(|e| CoreError::from_io(&e, &dir))?;
        let (stem, ext) = SAMPLE_FILE_NAME
            .rsplit_once('.')
            .unwrap_or((SAMPLE_FILE_NAME, "json"));
        let mut n = 1u32;
        let (path, mut file) = loop {
            let name = if n == 1 {
                SAMPLE_FILE_NAME.to_string()
            } else {
                format!("{stem}-{n}.{ext}")
            };
            let path = dir.join(name);
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(f) => break (path, f),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(CoreError::from_io(&e, &path)),
            }
        };
        if let Err(e) = file.write_all(sample_bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&path);
            return Err(CoreError::from_io(&e, &path));
        }
        drop(file);
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut entry = Self::new_entry(&path, format!("Sample: {file_name}"), EntryKind::Sample);
        entry.collection_id = Some(SAMPLE_COLLECTION_ID.to_string());
        let committed = self.commit(|s| {
            s.entries.push(entry.clone());
            Ok(entry)
        });
        if committed.is_err() {
            let _ = fs::remove_file(&path); // our own fresh copy, never registered
        }
        committed
    }

    /// Unregister. Deletes the file only for `EntryKind::Sample` whose
    /// canonical path is inside `data_dir` (correction C2); otherwise never.
    pub fn remove(&mut self, id: &str, data_dir: &Path) -> CoreResult<()> {
        let entry = self.get(id)?;
        if entry.entry_kind == EntryKind::Sample
            && is_within(Path::new(&entry.absolute_path), data_dir)
        {
            match fs::remove_file(&entry.absolute_path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(CoreError::from_io(&e, Path::new(&entry.absolute_path))),
            }
        }
        self.commit(|s| {
            s.entries.retain(|e| e.id != id);
            Ok(())
        })
    }

    pub fn update_metadata(
        &mut self,
        id: &str,
        patch: MetadataPatch,
    ) -> CoreResult<ManagedDocument> {
        let display_name = match patch.display_name.map(|n| n.trim().to_string()) {
            Some(n) if n.is_empty() => return Err(invalid("The display name cannot be empty.")),
            other => other,
        };
        self.commit(|s| {
            let e = Self::entry_mut(s, id)?;
            if let Some(n) = display_name {
                e.display_name = n;
            }
            if let Some(c) = patch.collection_id {
                e.collection_id = Some(c).filter(|c| !c.is_empty());
            }
            Ok(e.clone())
        })
    }

    /// Point an entry at a different file. The new file becomes a plain user
    /// file (never deletable as a sample) and needs fresh link confirmations.
    pub fn relocate(&mut self, id: &str, new_path: &Path) -> CoreResult<ManagedDocument> {
        let canonical = resolve_target(new_path)?.target_path;
        if self
            .state
            .entries
            .iter()
            .any(|e| e.id != id && Self::canonical(&e.absolute_path) == canonical)
        {
            return Err(invalid("That file is already managed by another entry.")
                .with_path(lossy(new_path)));
        }
        self.commit(|s| {
            let e = Self::entry_mut(s, id)?;
            e.absolute_path = lossy(&absolute(new_path));
            e.kind = Self::kind_for(new_path);
            e.entry_kind = EntryKind::File;
            e.link_write_confirmed = false;
            e.hardlink_write_confirmed = false;
            Ok(e.clone())
        })
    }

    pub fn set_schema(
        &mut self,
        id: &str,
        schema_path: Option<PathBuf>,
    ) -> CoreResult<ManagedDocument> {
        self.commit(|s| {
            let e = Self::entry_mut(s, id)?;
            e.local_schema_path = schema_path.map(|p| lossy(&p));
            Ok(e.clone())
        })
    }

    pub fn set_link_confirmed(
        &mut self,
        id: &str,
        symlink: bool,
        hardlink: bool,
    ) -> CoreResult<ManagedDocument> {
        self.commit(|s| {
            let e = Self::entry_mut(s, id)?;
            e.link_write_confirmed = symlink;
            e.hardlink_write_confirmed = hardlink;
            Ok(e.clone())
        })
    }

    pub fn touch_opened(&mut self, id: &str) -> CoreResult<()> {
        self.commit(|s| {
            Self::entry_mut(s, id)?.last_opened_at = Some(now_rfc3339());
            Ok(())
        })
    }

    pub fn kind_for(path: &Path) -> DocKind {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("jsonc") => DocKind::Jsonc,
            _ => DocKind::Json,
        }
    }

    pub fn preferences(&self) -> Preferences {
        self.state.preferences.clone()
    }

    /// Rejects values that would make the app unusable (zero retention would
    /// delete every backup; font scale outside the UI's supported range).
    pub fn set_preferences(&mut self, prefs: Preferences) -> CoreResult<()> {
        if prefs.backup_retention == 0 {
            return Err(invalid("Backup retention must be at least 1."));
        }
        if !(0.875..=1.5).contains(&prefs.font_scale) {
            return Err(invalid("Font scale must be between 0.875 and 1.5."));
        }
        self.commit(|s| {
            s.preferences = prefs;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
