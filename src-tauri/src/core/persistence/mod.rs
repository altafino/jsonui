//! Safe reads and the save pipeline (store lane). Spec §10 steps 2-13 with
//! eng R5 (symlink write-through, hardlink confirm, mode bits) and R15
//! (temp naming `.<basename>.configdeck-<random>.tmp`, leftover detection).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::core::backup::BackupStore;
use crate::core::types::Fingerprint;
use crate::core::CoreResult;

/// SHA-256 of `bytes`, lowercase hex. Shared by every lane.
pub fn fingerprint(bytes: &[u8]) -> Fingerprint {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read a registered file: resolves links (security::paths), requires a
/// regular file, enforces `max_bytes` (FILE_TOO_LARGE). Does not decode UTF-8.
pub fn read_checked(path: &Path, max_bytes: u64) -> CoreResult<Vec<u8>> {
    let _ = (path, max_bytes);
    todo!("store lane")
}

/// What the caller confirmed for this registry entry (eng R5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkConfirmations {
    pub symlink: bool,
    pub hardlink: bool,
}

pub struct SaveRequest<'a> {
    pub file_id: &'a str,
    /// Registered path (may be a symlink).
    pub path: &'a Path,
    /// Fingerprint of the bytes the draft is based on.
    pub expected: &'a Fingerprint,
    pub candidate: &'a [u8],
    pub confirmations: LinkConfirmations,
    /// Re-validates candidate bytes (reparse + schema). Err aborts the save.
    pub validate: &'a dyn Fn(&[u8]) -> CoreResult<()>,
    pub backups: &'a BackupStore,
    /// Optional summary stored with the backup.
    pub change_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub fingerprint: Fingerprint,
    pub backup_id: String,
}

/// Spec §10 steps 2-13. Never truncates or corrupts the source on failure;
/// no backup → no write; fingerprint recheck right before rename.
pub fn save(req: &SaveRequest) -> CoreResult<SaveOutcome> {
    let _ = req;
    todo!("store lane")
}

/// Atomic replace for app-owned files (registry, preferences): temp in the
/// same dir, fsync, rename, fsync dir. Optionally keeps the previous version
/// as `<name>.bak` (eng R12).
pub fn atomic_write(path: &Path, bytes: &[u8], keep_bak: bool) -> CoreResult<()> {
    let _ = (path, bytes, keep_bak);
    todo!("store lane")
}

/// Leftover temp files from a crash next to `path` (eng R15): only names
/// matching `.<basename>.configdeck-*.tmp` exactly.
pub fn find_leftovers(path: &Path) -> Vec<PathBuf> {
    let _ = path;
    todo!("store lane")
}

/// Remove leftovers returned by `find_leftovers` (re-validated by name).
pub fn remove_leftovers(path: &Path) -> CoreResult<u32> {
    let _ = path;
    todo!("store lane")
}
