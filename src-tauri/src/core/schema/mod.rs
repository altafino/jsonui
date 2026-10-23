//! JSON Schema association and validation (schema lane).
//!
//! Precedence (spec §6): user schema > built-in preset > structural inference.
//! Never fetches remote schemas; `$ref` resolves locally only. An invalid or
//! unresolvable schema never blocks generic editing (warning diagnostic).
//! Severity (eng R2): preset → Warning/PresetSchema; user → Error/UserSchema.

use std::path::Path;

use crate::core::types::{Diagnostic, FieldHint};
use crate::core::CoreResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaOrigin {
    Preset,
    User,
}

pub struct CompiledSchema {
    _private: (),
}

impl CompiledSchema {
    /// Compile a schema document (draft-07 … 2020-12). Remote `$ref` → error.
    pub fn compile(schema: &serde_json::Value, origin: SchemaOrigin) -> CoreResult<Self> {
        let _ = (schema, origin);
        todo!("schema lane")
    }

    /// Load and compile a local schema file chosen by the user.
    pub fn load_user(path: &Path) -> CoreResult<Self> {
        let _ = path;
        todo!("schema lane")
    }

    pub fn origin(&self) -> SchemaOrigin {
        todo!("schema lane")
    }

    /// Validate; each diagnostic carries the instance pointer.
    pub fn validate(&self, instance: &serde_json::Value) -> Vec<Diagnostic> {
        let _ = instance;
        todo!("schema lane")
    }

    /// Hints for the given instance pointers (labels, enums, defaults, addable
    /// optional properties, open maps, secret/format flags).
    pub fn hints(&self, instance: &serde_json::Value, pointers: &[String]) -> Vec<FieldHint> {
        let _ = (instance, pointers);
        todo!("schema lane")
    }
}
