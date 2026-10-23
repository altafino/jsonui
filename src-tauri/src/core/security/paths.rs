//! Path policy (store lane): regular files only, symlink/hardlink detection,
//! containment checks. Never follows links silently (spec §10, eng R5).

use std::path::{Path, PathBuf};

use crate::core::CoreResult;

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
pub fn resolve_target(path: &Path) -> CoreResult<ResolvedTarget> {
    let _ = path;
    todo!("store lane")
}

/// True when `path` (canonicalized) lies inside `dir` (canonicalized).
pub fn is_within(path: &Path, dir: &Path) -> bool {
    let _ = (path, dir);
    todo!("store lane")
}
