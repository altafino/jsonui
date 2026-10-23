//! Safe reads and the save pipeline (store lane). Spec §10 steps 2-13 with
//! eng R5 (symlink write-through, hardlink confirm, mode bits) and R15
//! (temp naming `.<basename>.configdeck-<random>.tmp`, leftover detection).
//!
//! # Residual race (spec §10 race note)
//!
//! The fingerprint check is not a compare-and-swap. The source is compared
//! to the expected fingerprint (step 3) and again immediately before the
//! rename (step 9), but a third-party process that writes between that last
//! read and the `rename` will still be overwritten (the earlier backup holds
//! the bytes we read, not that writer's bytes). Cooperating editors that
//! also write via temp+rename are no safer: whichever rename lands last
//! wins. Non-cooperating writers cannot be locked out portably; the window
//! is kept to a single read plus a rename.
//!
//! The same applies to link retargeting: the registered path is re-resolved
//! right before the rename and the save aborts with EXTERNAL_CHANGE_CONFLICT
//! if the resolved target changed, but a retarget inside that window is not
//! detectable.
//!
//! # Replace strategy
//!
//! The temp file is renamed onto the *resolved* target (R5), so a symlink
//! stays a symlink. On unix this is `rename(2)`. On Windows `std::fs::rename`
//! uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, which replaces the
//! destination without a window where it is missing; it fails (leaving the
//! source intact) if another process holds the file open without delete
//! sharing, and that failure is reported as ATOMIC_WRITE_FAILED.

mod timefmt;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::core::backup::BackupStore;
use crate::core::security::paths::{ResolvedTarget, resolve_target};
use crate::core::types::Fingerprint;
use crate::core::{CoreError, CoreResult, ErrorCode};

pub(crate) use timefmt::{Stamp, now_rfc3339};

/// SHA-256 of `bytes`, lowercase hex. Shared by every lane.
pub fn fingerprint(bytes: &[u8]) -> Fingerprint {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read a registered file: resolves links (security::paths), requires a
/// regular file, enforces `max_bytes` (FILE_TOO_LARGE). Does not decode UTF-8.
pub fn read_checked(path: &Path, max_bytes: u64) -> CoreResult<Vec<u8>> {
    let resolved = resolve_target(path)?;
    let target = &resolved.target_path;
    let too_large = || {
        CoreError::new(
            ErrorCode::FileTooLarge,
            format!("File is larger than the {max_bytes} byte editing limit."),
        )
        .with_path(path.display().to_string())
        .with_remediation(
            "Edit this file in another editor; ConfigDeck only opens small config files.",
        )
    };
    let file = File::open(target).map_err(|e| CoreError::from_io(&e, path))?;
    let len = file
        .metadata()
        .map_err(|e| CoreError::from_io(&e, path))?
        .len();
    if len > max_bytes {
        return Err(too_large());
    }
    // The file may grow between stat and read; never read more than the limit + 1.
    let mut buf = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| CoreError::from_io(&e, path))?;
    if buf.len() as u64 > max_bytes {
        return Err(too_large());
    }
    Ok(buf)
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
    /// Canonical target recorded when the draft was loaded (eng R5). When
    /// set, a registered symlink that now resolves elsewhere is a conflict,
    /// even if the new target has identical bytes.
    pub expected_target: Option<&'a Path>,
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

fn conflict(path: &Path) -> CoreError {
    CoreError::new(
        ErrorCode::ExternalChangeConflict,
        "The file changed on disk since it was loaded; nothing was written.",
    )
    .with_path(path.display().to_string())
    .with_remediation("Review the disk version, then reapply your changes or reload.")
}

fn atomic_failed(err: &std::io::Error, path: &Path) -> CoreError {
    CoreError::new(
        ErrorCode::AtomicWriteFailed,
        format!("Could not write the file safely: {err}"),
    )
    .with_path(path.display().to_string())
    .with_remediation(
        "The original file was not changed. Check permissions and free space, then try again.",
    )
}

/// Spec §10 steps 2-13. Never truncates or corrupts the source on failure;
/// no backup → no write; fingerprint recheck right before rename.
pub fn save(req: &SaveRequest) -> CoreResult<SaveOutcome> {
    let shown = req.path;
    // Step 2: resolve and validate the destination.
    let resolved = resolve_target(shown)?;
    if req.expected_target.is_some_and(|t| t != resolved.target_path) {
        return Err(conflict(shown));
    }
    // Steps 3-4: compare the bytes on disk with the loaded snapshot.
    let current = fs::read(&resolved.target_path).map_err(|e| CoreError::from_io(&e, shown))?;
    if fingerprint(&current) != *req.expected {
        return Err(conflict(shown));
    }
    if resolved.is_symlink && !req.confirmations.symlink {
        return Err(CoreError::new(
            ErrorCode::SymlinkRequiresConfirmation,
            "This path is a symbolic link; saving writes to the file it points to.",
        )
        .with_path(shown.display().to_string())
        .with_remediation(format!(
            "Confirm to write through the link to {}.",
            resolved.target_path.display()
        )));
    }
    if resolved.hardlink_count > 1 && !req.confirmations.hardlink {
        return Err(CoreError::new(
            ErrorCode::HardlinkRequiresConfirmation,
            "This file has other hard links; saving detaches them from this file.",
        )
        .with_path(shown.display().to_string())
        .with_remediation("Confirm to save anyway; the other hard links keep the old content."));
    }
    let meta = fs::metadata(&resolved.target_path).map_err(|e| CoreError::from_io(&e, shown))?;
    if meta.permissions().readonly() {
        // Never chmod: the user made this file read-only on purpose.
        return Err(
            CoreError::new(ErrorCode::ReadOnly, "The file is read-only.")
                .with_path(shown.display().to_string())
                .with_remediation(
                    "Make the file writable outside ConfigDeck if you want to edit it.",
                ),
        );
    }
    // Steps 6-7: the candidate must still be acceptable.
    (req.validate)(req.candidate)?;
    // Step 8: no backup, no write.
    let backup_id = req
        .backups
        .create(req.file_id, &current, req.change_count)?;
    // Steps 9-11.
    if let Err(e) = stage_and_replace(req, &resolved, &meta) {
        // The source is untouched, so this backup does not describe a save.
        let _ = req.backups.remove(req.file_id, &backup_id);
        return Err(e);
    }
    // Step 12: read back and verify.
    let want = fingerprint(req.candidate);
    let verified = fs::read(&resolved.target_path).is_ok_and(|b| fingerprint(&b) == want);
    if !verified {
        return Err(CoreError::new(
            ErrorCode::AtomicWriteFailed,
            "The saved file did not read back as expected.",
        )
        .with_path(shown.display().to_string())
        .with_remediation(format!(
            "Restore the previous version from backup {backup_id}."
        )));
    }
    // Retention is housekeeping; a failure must not fail a committed save.
    let _ = req.backups.prune(req.file_id);
    Ok(SaveOutcome {
        fingerprint: want,
        backup_id,
    })
}

/// Write the temp file, recheck the source, rename. On any error the temp
/// file is removed and the source is unchanged.
fn stage_and_replace(
    req: &SaveRequest,
    resolved: &ResolvedTarget,
    meta: &fs::Metadata,
) -> CoreResult<()> {
    let target = &resolved.target_path;
    let (dir, base) = split_target(target)?;
    let mut tmp = write_temp(dir, base, req.candidate, Some(&meta.permissions()))?;
    #[cfg(test)]
    test_hooks::run_after_temp();
    // Step 9: re-resolve (link retargeted?) and recheck the bytes.
    let again = resolve_target(req.path)?;
    if again.target_path != *target {
        return Err(conflict(req.path));
    }
    let now = fs::read(target).map_err(|e| CoreError::from_io(&e, req.path))?;
    if fingerprint(&now) != *req.expected {
        return Err(conflict(req.path));
    }
    // Step 10: atomic replace onto the resolved target (the link stays a link).
    replace(&tmp.path, target).map_err(|e| atomic_failed(&e, req.path))?;
    tmp.armed = false;
    // Step 11.
    sync_dir(dir);
    Ok(())
}

fn split_target(target: &Path) -> CoreResult<(&Path, &std::ffi::OsStr)> {
    match (target.parent(), target.file_name()) {
        (Some(dir), Some(base)) => Ok((dir, base)),
        _ => Err(CoreError::new(
            ErrorCode::Internal,
            "Path has no parent directory or file name.",
        )
        .with_path(target.display().to_string())),
    }
}

/// Removes the temp file on drop unless `armed` was cleared after the rename.
struct TempFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Create a new file readable and writable only by the owner (unix 0600).
pub(crate) fn create_new_private(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(path)
}

/// Write `bytes` to a fresh temp file in `dir`, fsync it, and apply `perms`
/// (the target's permissions; the temp starts at 0600, so never broader).
fn write_temp(
    dir: &Path,
    base: &std::ffi::OsStr,
    bytes: &[u8],
    perms: Option<&fs::Permissions>,
) -> CoreResult<TempFile> {
    let name = format!(
        ".{}.configdeck-{}.tmp",
        base.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    );
    let path = dir.join(name);
    let fail = |e: &std::io::Error| atomic_failed(e, &path);
    let mut file = create_new_private(&path).map_err(|e| fail(&e))?;
    let tmp = TempFile {
        path: path.clone(),
        armed: true,
    };
    file.write_all(bytes).map_err(|e| fail(&e))?;
    file.flush().map_err(|e| fail(&e))?;
    file.sync_all().map_err(|e| fail(&e))?;
    drop(file);
    if let Some(p) = perms {
        fs::set_permissions(&path, p.clone()).map_err(|e| fail(&e))?;
    }
    Ok(tmp)
}

fn replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if test_hooks::fail_replace() {
        return Err(std::io::Error::other("injected replace failure"));
    }
    fs::rename(temp, target)
}

/// Best effort: the rename has already happened, so a failure here must not
/// be reported as a failed save.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Atomic replace for app-owned files (registry, preferences): temp in the
/// same dir, fsync, rename, fsync dir. Optionally keeps the previous version
/// as `<name>.bak` (eng R12). Missing parent directories are created.
pub fn atomic_write(path: &Path, bytes: &[u8], keep_bak: bool) -> CoreResult<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|e| atomic_failed(&e, path))?;
    if keep_bak {
        match fs::read(path) {
            Ok(previous) => write_replace(&bak_path(path), &previous)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(atomic_failed(&e, path)),
        }
    }
    write_replace(path, bytes)
}

pub(crate) fn bak_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

fn write_replace(path: &Path, bytes: &[u8]) -> CoreResult<()> {
    let (dir, base) = match (
        path.parent().filter(|p| !p.as_os_str().is_empty()),
        path.file_name(),
    ) {
        (dir, Some(base)) => (dir.unwrap_or(Path::new(".")), base),
        _ => {
            return Err(
                CoreError::new(ErrorCode::Internal, "Path has no file name.")
                    .with_path(path.display().to_string()),
            );
        }
    };
    let mut tmp = write_temp(dir, base, bytes, None)?;
    replace(&tmp.path, path).map_err(|e| atomic_failed(&e, path))?;
    tmp.armed = false;
    sync_dir(dir);
    Ok(())
}

/// Directory and basename to scan for leftovers: the resolved target's when
/// the path resolves (temp files live beside the target), else the path's own.
fn leftover_scope(path: &Path) -> Option<(PathBuf, String)> {
    let target = resolve_target(path).map_or_else(|_| path.to_path_buf(), |r| r.target_path);
    let dir = target.parent()?.to_path_buf();
    let base = target.file_name()?.to_str()?.to_string();
    Some((dir, base))
}

/// Exactly `.<base>.configdeck-<alphanumeric>.tmp`.
fn is_leftover_name(name: &str, base: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|n| n.strip_prefix(base))
        .and_then(|n| n.strip_prefix(".configdeck-"))
        .and_then(|n| n.strip_suffix(".tmp"))
        .is_some_and(|random| {
            !random.is_empty() && random.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// Leftover temp files from a crash next to `path` (eng R15): only names
/// matching `.<basename>.configdeck-*.tmp` exactly.
pub fn find_leftovers(path: &Path) -> Vec<PathBuf> {
    let Some((dir, base)) = leftover_scope(path) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| is_leftover_name(n, &base))
        })
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

/// Remove leftovers returned by `find_leftovers` (re-validated by name).
pub fn remove_leftovers(path: &Path) -> CoreResult<u32> {
    let mut removed = 0;
    for leftover in find_leftovers(path) {
        match fs::remove_file(&leftover) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(CoreError::from_io(&e, &leftover)),
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod test_hooks {
    //! Per-thread failure injection (each test runs on its own thread).
    use std::cell::{Cell, RefCell};

    thread_local! {
        static FAIL_REPLACE: Cell<bool> = const { Cell::new(false) };
        static AFTER_TEMP: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    }

    pub fn fail_replace() -> bool {
        FAIL_REPLACE.with(Cell::get)
    }
    pub fn set_fail_replace(v: bool) {
        FAIL_REPLACE.with(|c| c.set(v));
    }
    pub fn set_after_temp(f: impl Fn() + 'static) {
        AFTER_TEMP.with(|c| *c.borrow_mut() = Some(Box::new(f)));
    }
    pub fn run_after_temp() {
        AFTER_TEMP.with(|c| {
            if let Some(f) = c.borrow().as_ref() {
                f();
            }
        });
    }
}

#[cfg(test)]
mod tests;
