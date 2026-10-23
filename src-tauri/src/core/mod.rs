//! ConfigDeck core: Tauri-free document engine and file services.
//!
//! Ownership (parallel build lanes, see docs/contract.md):
//! - document, patch, watcher, security::redact  → document lane
//! - persistence, backup, registry, security::paths → store lane
//! - schema, presets                             → schema lane
//! - `crate::commands` wiring                     → integration
//!
//! Rule (eng R7): `jsonc_parser` may only be imported under `core/document`
//! and `core/security/redact.rs` (masking of unparseable files, eng R17).

pub mod backup;
pub mod document;
pub mod error;
pub mod patch;
pub mod persistence;
pub mod presets;
pub mod registry;
pub mod schema;
pub mod security;
pub mod types;
pub mod watcher;

pub use error::{CoreError, CoreResult, ErrorCode};

/// Default maximum editable file size (spec §5): 5 MiB.
pub const MAX_EDITABLE_BYTES: u64 = 5 * 1024 * 1024;
