//! External change detection by polling (design D2, document lane).
//!
//! The Tauri layer runs a 1 s timer for the open document and calls
//! [`FilePoller::check`]; cheap metadata first, hash only when it changed.
//! Own writes are suppressed by comparing against the committed fingerprint.

use std::path::Path;

use crate::core::types::Fingerprint;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub modified_nanos: i128,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    Unchanged,
    /// Bytes on disk differ from `known`; carries the new fingerprint.
    Changed(Fingerprint),
    Missing,
    /// Readability/writability changed.
    AccessChanged { readable: bool, writable: bool },
}

pub struct FilePoller {
    _private: (),
}

impl FilePoller {
    /// Start tracking `path` whose committed content has fingerprint `known`.
    pub fn new(path: &Path, known: Fingerprint) -> Self {
        let _ = (path, known);
        todo!("document lane")
    }

    /// Re-baseline after our own save or a reload.
    pub fn set_known(&mut self, known: Fingerprint) {
        let _ = known;
        todo!("document lane")
    }

    pub fn check(&mut self) -> PollOutcome {
        todo!("document lane")
    }
}
