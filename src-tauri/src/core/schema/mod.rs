//! JSON Schema association and validation (schema lane).
//!
//! Precedence (spec §6): user schema > built-in preset > structural inference.
//! Never fetches remote schemas; `$ref` resolves locally only. An invalid or
//! unresolvable schema never blocks generic editing (warning diagnostic).
//! Severity (eng R2): preset → Warning/PresetSchema; user → Error/UserSchema.

mod hints;
#[cfg(test)]
mod tests;

use std::error::Error;
use std::path::Path;

use jsonschema::error::ValidationErrorKind;
use jsonschema::{Uri, Validator};
use serde_json::Value;

use crate::core::types::{Diagnostic, DiagnosticSource, FieldHint, Severity};
use crate::core::{CoreError, CoreResult, ErrorCode, MAX_EDITABLE_BYTES};

/// Upper bound on diagnostics per validation run (pathological instances).
const MAX_DIAGNOSTICS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaOrigin {
    Preset,
    User,
}

/// Refuses every external `$ref` (http, file, relative): schemas must be self-contained.
struct DenyRetriever;

impl jsonschema::Retrieve for DenyRetriever {
    fn retrieve(&self, uri: &Uri<String>) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Err(format!("external $ref is not allowed: {uri}").into())
    }
}

pub struct CompiledSchema {
    origin: SchemaOrigin,
    validator: Validator,
    root: Value,
}

impl CompiledSchema {
    /// Compile a schema document (draft-07 … 2020-12). Remote `$ref` → error.
    pub fn compile(schema: &serde_json::Value, origin: SchemaOrigin) -> CoreResult<Self> {
        let validator = jsonschema::options()
            .with_retriever(DenyRetriever)
            .build(schema)
            .map_err(|e| {
                CoreError::new(
                    ErrorCode::SchemaValidationFailed,
                    format!("The schema could not be compiled: {e}"),
                )
                .with_remediation(
                    "Fix or remove the schema association; the file stays editable without it.",
                )
            })?;
        Ok(Self {
            origin,
            validator,
            root: schema.clone(),
        })
    }

    /// Load and compile a local schema file chosen by the user.
    pub fn load_user(path: &Path) -> CoreResult<Self> {
        let bytes = std::fs::read(path).map_err(|e| CoreError::from_io(&e, path))?;
        if bytes.len() as u64 > MAX_EDITABLE_BYTES {
            return Err(
                CoreError::new(ErrorCode::FileTooLarge, "The schema file is too large")
                    .with_path(path.display().to_string()),
            );
        }
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        let schema: Value = serde_json::from_slice(bytes).map_err(|e| {
            CoreError::new(
                ErrorCode::SchemaValidationFailed,
                format!("The schema file is not valid JSON: {e}"),
            )
            .with_path(path.display().to_string())
            .with_position(
                u32::try_from(e.line()).unwrap_or(0),
                u32::try_from(e.column()).unwrap_or(0),
            )
        })?;
        Self::compile(&schema, SchemaOrigin::User)
            .map_err(|e| e.with_path(path.display().to_string()))
    }

    pub fn origin(&self) -> SchemaOrigin {
        self.origin
    }

    /// Validate; each diagnostic carries the instance pointer.
    pub fn validate(&self, instance: &serde_json::Value) -> Vec<Diagnostic> {
        let (severity, source) = match self.origin {
            SchemaOrigin::Preset => (Severity::Warning, DiagnosticSource::PresetSchema),
            SchemaOrigin::User => (Severity::Error, DiagnosticSource::UserSchema),
        };
        self.validator
            .iter_errors(instance)
            .take(MAX_DIAGNOSTICS)
            .map(|e| Diagnostic {
                severity,
                source,
                code: "SCHEMA_VIOLATION".to_owned(),
                message: message(&e),
                pointer: Some(e.instance_path().to_string()),
                line: None,
                column: None,
            })
            .collect()
    }

    /// Hints for the given instance pointers (labels, enums, defaults, addable
    /// optional properties, open maps, secret/format flags). Returns one hint
    /// per pointer, in order; pointers the schema does not describe get an
    /// empty hint.
    pub fn hints(&self, instance: &serde_json::Value, pointers: &[String]) -> Vec<FieldHint> {
        pointers
            .iter()
            .map(|p| hints::hint_for(&self.root, instance, p))
            .collect()
    }
}

/// Plain-language message that never echoes the offending value (it may be a secret).
fn message(e: &jsonschema::ValidationError<'_>) -> String {
    let list = |v: &Value| {
        v.as_array().map_or_else(
            || v.to_string(),
            |a| {
                a.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        )
    };
    match e.kind() {
        ValidationErrorKind::Enum { options } => format!("must be one of: {}", list(options)),
        ValidationErrorKind::Constant { expected_value } => format!("must be {expected_value}"),
        _ => e.masked().to_string(),
    }
}

/// Decode an RFC 6901 pointer into segments (`""` is the root → no segments).
pub(crate) fn decode_pointer(pointer: &str) -> Vec<String> {
    if pointer.is_empty() {
        return Vec::new();
    }
    pointer
        .split('/')
        .skip(1)
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}
