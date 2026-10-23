//! Backup store (store lane): one backup per successful save, 0600 files in
//! 0700 dirs, per-file retention (default 20), never deletes source files.

use std::path::{Path, PathBuf};

use crate::core::types::BackupEntry;
use crate::core::CoreResult;

pub struct BackupStore {
    pub root: PathBuf,
    pub retention: u32,
}

impl BackupStore {
    pub fn new(root: PathBuf, retention: u32) -> Self {
        Self { root, retention }
    }

    /// Store `original` for `file_id`; returns the backup id. Errors map to BACKUP_FAILED.
    pub fn create(&self, file_id: &str, original: &[u8], change_count: Option<u32>) -> CoreResult<String> {
        let _ = (file_id, original, change_count);
        todo!("store lane")
    }

    /// Newest first.
    pub fn list(&self, file_id: &str) -> CoreResult<Vec<BackupEntry>> {
        let _ = file_id;
        todo!("store lane")
    }

    pub fn read(&self, file_id: &str, backup_id: &str) -> CoreResult<Vec<u8>> {
        let _ = (file_id, backup_id);
        todo!("store lane")
    }

    /// Keep the newest `retention` backups for `file_id`.
    pub fn prune(&self, file_id: &str) -> CoreResult<u32> {
        let _ = file_id;
        todo!("store lane")
    }

    pub fn dir_for(&self, file_id: &str) -> PathBuf {
        let _ = file_id;
        todo!("store lane")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
