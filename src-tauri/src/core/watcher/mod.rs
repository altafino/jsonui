//! External change detection by polling (design D2, document lane).
//!
//! The Tauri layer runs a 1 s timer for the open document and calls
//! [`FilePoller::check`]; cheap metadata first, hash only when it changed.
//! Own writes are suppressed by comparing against the committed fingerprint.

use std::fs::{File, Metadata, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::core::persistence::fingerprint;
use crate::core::types::Fingerprint;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub modified_nanos: i128,
    pub inode: u64,
}

impl FileStamp {
    fn of(meta: &Metadata) -> Self {
        let modified_nanos =
            meta.modified()
                .ok()
                .map_or(0, |t| match t.duration_since(UNIX_EPOCH) {
                    Ok(d) => i128::try_from(d.as_nanos()).unwrap_or(i128::MAX),
                    Err(e) => -i128::try_from(e.duration().as_nanos()).unwrap_or(i128::MAX),
                });
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(meta);
        #[cfg(not(unix))]
        let inode = 0;
        Self {
            len: meta.len(),
            modified_nanos,
            inode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    Unchanged,
    /// Bytes on disk differ from `known`; carries the new fingerprint.
    Changed(Fingerprint),
    Missing,
    /// Readability/writability changed.
    AccessChanged {
        readable: bool,
        writable: bool,
    },
}

pub struct FilePoller {
    path: PathBuf,
    known: Fingerprint,
    /// Stat at the last hash; `None` forces a rehash on the next check.
    stamp: Option<FileStamp>,
    access: (bool, bool),
    missing: bool,
}

impl FilePoller {
    /// Start tracking `path` whose committed content has fingerprint `known`.
    pub fn new(path: &Path, known: Fingerprint) -> Self {
        let stamp = std::fs::metadata(path).ok().map(|m| FileStamp::of(&m));
        let access = access_of(path);
        Self {
            path: path.to_path_buf(),
            known,
            stamp,
            access,
            missing: false,
        }
    }

    /// Re-baseline after our own save or a reload.
    ///
    /// The next check rehashes once, so a foreign write racing with this call
    /// is still reported while our own write (hash equals `known`) is not.
    pub fn set_known(&mut self, known: Fingerprint) {
        self.known = known;
        self.stamp = None;
    }

    pub fn check(&mut self) -> PollOutcome {
        let meta = match std::fs::metadata(&self.path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                self.stamp = None;
                return if std::mem::replace(&mut self.missing, true) {
                    PollOutcome::Unchanged // already reported
                } else {
                    PollOutcome::Missing
                };
            }
            Err(_) => return self.check_access(),
        };
        self.missing = false;
        let stamp = FileStamp::of(&meta);
        if self.stamp.as_ref() != Some(&stamp) {
            match std::fs::read(&self.path) {
                Ok(bytes) => {
                    self.stamp = Some(stamp);
                    let fp = fingerprint(&bytes);
                    if fp != self.known {
                        return PollOutcome::Changed(fp);
                    }
                }
                // Unreadable right now (e.g. permissions): retry next tick, report access.
                Err(_) => return self.check_access(),
            }
        }
        self.check_access()
    }

    fn check_access(&mut self) -> PollOutcome {
        let access = access_of(&self.path);
        if access == self.access {
            return PollOutcome::Unchanged;
        }
        self.access = access;
        PollOutcome::AccessChanged {
            readable: access.0,
            writable: access.1,
        }
    }
}

/// `(readable, writable)` by trying to open; the write probe neither creates
/// nor truncates.
fn access_of(path: &Path) -> (bool, bool) {
    (
        File::open(path).is_ok(),
        OpenOptions::new().write(true).open(path).is_ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn own_write_is_suppressed_and_foreign_write_detected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write(&path, "{\"a\":1}");
        let mut poller = FilePoller::new(&path, fingerprint(b"{\"a\":1}"));
        assert_eq!(poller.check(), PollOutcome::Unchanged);

        // Our own save: content changes, caller re-baselines.
        write(&path, "{\"a\":2}");
        poller.set_known(fingerprint(b"{\"a\":2}"));
        assert_eq!(poller.check(), PollOutcome::Unchanged);
        assert_eq!(poller.check(), PollOutcome::Unchanged);

        // Someone else.
        write(&path, "{\"a\":3}");
        assert_eq!(
            poller.check(),
            PollOutcome::Changed(fingerprint(b"{\"a\":3}"))
        );
        // Reported once; the stat has not moved since.
        assert_eq!(poller.check(), PollOutcome::Unchanged);
    }

    #[test]
    fn same_content_rewrite_is_not_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write(&path, "{}");
        let mut poller = FilePoller::new(&path, fingerprint(b"{}"));
        write(&path, "{}"); // new mtime, same bytes
        assert_eq!(poller.check(), PollOutcome::Unchanged);
    }

    #[test]
    fn foreign_write_before_rebaseline_race_is_still_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write(&path, "{\"a\":1}");
        let mut poller = FilePoller::new(&path, fingerprint(b"{\"a\":1}"));
        write(&path, "{\"a\":9}");
        poller.set_known(fingerprint(b"{\"a\":2}")); // we believed we wrote a:2
        assert!(matches!(poller.check(), PollOutcome::Changed(_)));
    }

    #[test]
    fn missing_is_reported_once_then_recovery_compares_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write(&path, "{}");
        let mut poller = FilePoller::new(&path, fingerprint(b"{}"));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(poller.check(), PollOutcome::Missing);
        assert_eq!(poller.check(), PollOutcome::Unchanged);
        write(&path, "{}");
        assert_eq!(poller.check(), PollOutcome::Unchanged);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(poller.check(), PollOutcome::Missing);
        write(&path, "[1]");
        assert_eq!(poller.check(), PollOutcome::Changed(fingerprint(b"[1]")));
    }

    #[cfg(unix)]
    #[test]
    fn access_change_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write(&path, "{}");
        let mut poller = FilePoller::new(&path, fingerprint(b"{}"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        // Root can still write; only assert when the probe actually sees the change.
        let writable_now = OpenOptions::new().write(true).open(&path).is_ok();
        let outcome = poller.check();
        if writable_now {
            assert_eq!(outcome, PollOutcome::Unchanged);
        } else {
            assert_eq!(
                outcome,
                PollOutcome::AccessChanged {
                    readable: true,
                    writable: false
                }
            );
            assert_eq!(poller.check(), PollOutcome::Unchanged);
        }
    }
}
