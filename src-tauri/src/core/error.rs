//! Typed errors with stable codes (spec §11). Messages never contain secret
//! values; include paths and positions instead.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    NotFound,
    InvalidJson,
    DuplicateKeys,
    ReadOnly,
    SymlinkRequiresConfirmation,
    HardlinkRequiresConfirmation,
    FileTooLarge,
    SchemaValidationFailed,
    PresetWarningsRequireConfirmation,
    ExternalChangeConflict,
    BackupFailed,
    AtomicWriteFailed,
    UnsupportedEncoding,
    NotARegularFile,
    PermissionDenied,
    InvalidOperation,
    NotEditable,
    Cancelled,
    Io,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{code:?}: {message}")]
pub struct CoreError {
    pub code: ErrorCode,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// Actionable next step for the user.
    pub remediation: Option<String>,
}

impl CoreError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), path: None, line: None, column: None, remediation: None }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_position(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    /// Map an io::Error for `path` to a typed error without leaking contents.
    pub fn from_io(err: &std::io::Error, path: &std::path::Path) -> Self {
        use std::io::ErrorKind;
        let code = match err.kind() {
            ErrorKind::NotFound => ErrorCode::NotFound,
            ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            _ => ErrorCode::Io,
        };
        Self::new(code, err.to_string()).with_path(path.display().to_string())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
