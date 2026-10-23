//! Path policy (store lane): regular files only, symlink/hardlink detection,
//! containment checks. Never follows links silently (spec §10, eng R5).

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{CoreError, CoreResult, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    /// Path as registered.
    pub link_path: PathBuf,
    /// Canonical path of the regular file that is actually read/written.
    pub target_path: PathBuf,
    pub is_symlink: bool,
    pub hardlink_count: u64,
}

/// Resolve and validate a registered path: must end at a regular file
/// (not dir/device/pipe/socket). Does not require write permission.
///
/// The registered path is `lstat`ed first so a symlink is reported as such;
/// only then is it resolved with `canonicalize`.
pub fn resolve_target(path: &Path) -> CoreResult<ResolvedTarget> {
    let io_err = |e: &std::io::Error| CoreError::from_io(e, path);
    let link_meta = fs::symlink_metadata(path).map_err(|e| io_err(&e))?;
    let is_symlink = link_meta.file_type().is_symlink();
    if !is_symlink && !link_meta.is_file() {
        return Err(not_a_regular_file(path));
    }
    // For a dangling link this fails with NotFound, which is the right error.
    let target_path = fs::canonicalize(path).map_err(|e| io_err(&e))?;
    let target_meta = fs::metadata(&target_path).map_err(|e| io_err(&e))?;
    if !target_meta.is_file() {
        return Err(not_a_regular_file(path));
    }
    Ok(ResolvedTarget {
        link_path: path.to_path_buf(),
        target_path,
        is_symlink,
        hardlink_count: nlink(&target_meta),
    })
}

fn not_a_regular_file(path: &Path) -> CoreError {
    CoreError::new(
        ErrorCode::NotARegularFile,
        "Only regular files can be managed (not directories, devices, pipes or sockets).",
    )
    .with_path(path.display().to_string())
}

#[cfg(unix)]
fn nlink(meta: &fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::nlink(meta)
}

/// Windows: best effort, reported as 1 (no stable std API for link counts).
#[cfg(not(unix))]
fn nlink(_meta: &fs::Metadata) -> u64 {
    1
}

/// True when `path` (canonicalized) lies inside `dir` (canonicalized).
/// Any canonicalization failure (e.g. a missing path) yields `false`.
pub fn is_within(path: &Path, dir: &Path) -> bool {
    match (fs::canonicalize(path), fs::canonicalize(dir)) {
        (Ok(p), Ok(d)) => p.starts_with(d),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_file_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.json");
        fs::write(&f, "{}").unwrap();
        let r = resolve_target(&f).unwrap();
        assert!(!r.is_symlink);
        assert_eq!(r.hardlink_count, 1);
        assert_eq!(r.target_path, fs::canonicalize(&f).unwrap());
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let e = resolve_target(&dir.path().join("nope.json")).unwrap_err();
        assert_eq!(e.code, ErrorCode::NotFound);
    }

    #[test]
    fn directory_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let e = resolve_target(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::NotARegularFile);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_directory_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&sub, &link).unwrap();
        assert_eq!(
            resolve_target(&link).unwrap_err().code,
            ErrorCode::NotARegularFile
        );
    }

    #[cfg(unix)]
    #[test]
    fn fifo_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe.json");
        let status = std::process::Command::new("mkfifo").arg(&fifo).status();
        if !matches!(status, Ok(s) if s.success()) {
            return; // mkfifo unavailable; nothing to test
        }
        assert_eq!(
            resolve_target(&fifo).unwrap_err().code,
            ErrorCode::NotARegularFile
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_reported_and_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        fs::write(&real, "{}").unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let r = resolve_target(&link).unwrap();
        assert!(r.is_symlink);
        assert_eq!(r.target_path, fs::canonicalize(&real).unwrap());
        assert_eq!(r.link_path, link);
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(dir.path().join("gone.json"), &link).unwrap();
        assert_eq!(resolve_target(&link).unwrap_err().code, ErrorCode::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn hardlink_count_reported() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.json");
        fs::write(&a, "{}").unwrap();
        fs::hard_link(&a, dir.path().join("b.json")).unwrap();
        assert_eq!(resolve_target(&a).unwrap().hardlink_count, 2);
    }

    #[test]
    fn is_within_uses_canonical_paths() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("a").join("f.json");
        fs::create_dir_all(inner.parent().unwrap()).unwrap();
        fs::write(&inner, "{}").unwrap();
        let sneaky = dir.path().join("a").join("..").join("a").join("f.json");
        assert!(is_within(&sneaky, dir.path()));
        let other = tempfile::tempdir().unwrap();
        assert!(!is_within(&inner, other.path()));
        assert!(!is_within(&dir.path().join("missing"), dir.path()));
    }

    #[cfg(unix)]
    #[test]
    fn is_within_rejects_symlink_escape() {
        let data = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let real = outside.path().join("real.json");
        fs::write(&real, "{}").unwrap();
        let link = data.path().join("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(!is_within(&link, data.path()));
    }
}
