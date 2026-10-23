//! Backup store (store lane): one backup per successful save, 0600 files in
//! 0700 dirs, per-file retention (default 20), never deletes source files.
//!
//! Layout: `<root>/<file_id>/<id>.bak` plus a small `<id>.json` sidecar
//! (`created_at`, `size_bytes`, `change_count`). The id is
//! `<compact UTC timestamp with nanoseconds>-<8 hex>`, so ids sort
//! chronologically. A missing or unreadable sidecar degrades gracefully
//! (timestamp from the file's mtime, no change count).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::core::persistence::{Stamp, create_new_private};
use crate::core::types::BackupEntry;
use crate::core::{CoreError, CoreResult, ErrorCode};

pub struct BackupStore {
    pub root: PathBuf,
    pub retention: u32,
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    created_at: String,
    size_bytes: u64,
    change_count: Option<u32>,
}

/// Ids become path components: allow only a conservative character set.
fn valid_component(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn bad_id(what: &str) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, format!("Invalid {what}."))
}

fn failed(err: &std::io::Error, path: &Path) -> CoreError {
    CoreError::new(ErrorCode::BackupFailed, format!("Could not create a backup: {err}"))
        .with_path(path.display().to_string())
        .with_remediation("Check that the backup folder is writable and the disk has free space, then save again. The file was not changed.")
}

fn create_private_dir_all(path: &Path) -> std::io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(path)
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = create_new_private(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

impl BackupStore {
    pub fn new(root: PathBuf, retention: u32) -> Self {
        Self { root, retention }
    }

    /// Store `original` for `file_id`; returns the backup id. Errors map to BACKUP_FAILED.
    pub fn create(
        &self,
        file_id: &str,
        original: &[u8],
        change_count: Option<u32>,
    ) -> CoreResult<String> {
        if !valid_component(file_id) {
            return Err(failed(
                &std::io::Error::other("invalid file id"),
                &self.root,
            ));
        }
        let dir = self.dir_for(file_id);
        create_private_dir_all(&dir).map_err(|e| failed(&e, &dir))?;
        let now = Stamp::now();
        let id = format!(
            "{}-{}",
            now.compact(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let bak = dir.join(format!("{id}.bak"));
        write_private(&bak, original).map_err(|e| {
            let _ = fs::remove_file(&bak);
            failed(&e, &bak)
        })?;
        let sidecar = Sidecar {
            created_at: now.rfc3339(),
            size_bytes: original.len() as u64,
            change_count,
        };
        let json =
            serde_json::to_vec(&sidecar).map_err(|e| failed(&std::io::Error::other(e), &bak))?;
        let side = dir.join(format!("{id}.json"));
        write_private(&side, &json).map_err(|e| {
            let _ = fs::remove_file(&bak);
            let _ = fs::remove_file(&side);
            failed(&e, &side)
        })?;
        Ok(id)
    }

    /// Newest first.
    pub fn list(&self, file_id: &str) -> CoreResult<Vec<BackupEntry>> {
        if !valid_component(file_id) {
            return Err(bad_id("file id"));
        }
        let dir = self.dir_for(file_id);
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(CoreError::from_io(&e, &dir)),
        };
        let mut entries: Vec<BackupEntry> = read
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let id = path
                    .file_name()?
                    .to_str()?
                    .strip_suffix(".bak")?
                    .to_string();
                if !valid_component(&id) {
                    return None;
                }
                let meta = e.metadata().ok().filter(fs::Metadata::is_file)?;
                let side: Option<Sidecar> = fs::read(dir.join(format!("{id}.json")))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok());
                let created_at = side.as_ref().map_or_else(
                    || {
                        let secs = meta
                            .modified()
                            .ok()
                            .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                            .map_or(0, |d| d.as_secs());
                        Stamp::from_unix(i64::try_from(secs).unwrap_or(0), 0).rfc3339()
                    },
                    |s| s.created_at.clone(),
                );
                Some(BackupEntry {
                    id,
                    file_id: file_id.to_string(),
                    created_at,
                    size_bytes: meta.len(),
                    change_count: side.and_then(|s| s.change_count),
                    location: path.display().to_string(),
                })
            })
            .collect();
        entries.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(entries)
    }

    pub fn read(&self, file_id: &str, backup_id: &str) -> CoreResult<Vec<u8>> {
        if !valid_component(file_id) || !valid_component(backup_id) {
            return Err(bad_id("backup id"));
        }
        let path = self.dir_for(file_id).join(format!("{backup_id}.bak"));
        fs::read(&path).map_err(|e| CoreError::from_io(&e, &path))
    }

    /// Keep the newest `retention` backups for `file_id` (at least one).
    /// Returns how many were removed.
    pub fn prune(&self, file_id: &str) -> CoreResult<u32> {
        let keep = self.retention.max(1) as usize;
        let mut removed = 0;
        for entry in self.list(file_id)?.into_iter().skip(keep) {
            self.remove(file_id, &entry.id)?;
            removed += 1;
        }
        Ok(removed)
    }

    /// Delete one backup (never a source file). Used to prune and to drop a
    /// backup whose save did not happen.
    pub fn remove(&self, file_id: &str, backup_id: &str) -> CoreResult<()> {
        if !valid_component(file_id) || !valid_component(backup_id) {
            return Err(bad_id("backup id"));
        }
        let dir = self.dir_for(file_id);
        for ext in ["bak", "json"] {
            let path = dir.join(format!("{backup_id}.{ext}"));
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(CoreError::from_io(&e, &path)),
            }
        }
        Ok(())
    }

    /// Directory holding `file_id`'s backups. Callers that accept ids from
    /// outside must validate them; every method of this store does.
    pub fn dir_for(&self, file_id: &str) -> PathBuf {
        self.root.join(file_id)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_read_newest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let s = BackupStore::new(tmp.path().join("b"), 20);
        let a = s.create("f1", b"one", Some(2)).unwrap();
        let b = s.create("f1", b"two", None).unwrap();
        let list = s.list("f1").unwrap();
        assert_eq!(
            list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            [b.as_str(), a.as_str()]
        );
        assert_eq!(list[1].change_count, Some(2));
        assert_eq!(list[1].size_bytes, 3);
        assert!(list[0].created_at.ends_with('Z'));
        assert_eq!(s.read("f1", &a).unwrap(), b"one");
        assert!(s.list("other").unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let s = BackupStore::new(tmp.path().join("b"), 20);
        let id = s.create("f1", b"x", None).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&s.dir_for("f1")), 0o700);
        assert_eq!(mode(&s.dir_for("f1").join(format!("{id}.bak"))), 0o600);
        assert_eq!(mode(&s.dir_for("f1").join(format!("{id}.json"))), 0o600);
    }

    #[test]
    fn prune_keeps_newest_per_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = BackupStore::new(tmp.path().join("b"), 2);
        let ids: Vec<_> = (0..4)
            .map(|i| s.create("f1", format!("v{i}").as_bytes(), None).unwrap())
            .collect();
        s.create("f2", b"other", None).unwrap();
        assert_eq!(s.prune("f1").unwrap(), 2);
        let left: Vec<_> = s.list("f1").unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(left, [ids[3].clone(), ids[2].clone()]);
        assert_eq!(s.list("f2").unwrap().len(), 1);
        assert!(!s.dir_for("f1").join(format!("{}.json", ids[0])).exists());
    }

    #[test]
    fn unwritable_root_is_backup_failed() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("file");
        fs::write(&blocker, "x").unwrap();
        let s = BackupStore::new(blocker.join("sub"), 20);
        let e = s.create("f1", b"x", None).unwrap_err();
        assert_eq!(e.code, ErrorCode::BackupFailed);
        assert!(e.remediation.is_some());
    }

    #[test]
    fn path_traversal_ids_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let s = BackupStore::new(tmp.path().join("b"), 20);
        assert_eq!(
            s.create("../x", b"x", None).unwrap_err().code,
            ErrorCode::BackupFailed
        );
        assert_eq!(s.list("..").unwrap_err().code, ErrorCode::InvalidOperation);
        let id = s.create("f1", b"x", None).unwrap();
        assert!(s.read("f1", "../../etc/passwd").is_err());
        assert!(s.read("f1", &id).is_ok());
    }
}
