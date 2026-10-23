//! Parsing a file's bytes into a CST, with BOM/encoding/duplicate-key checks.

use std::collections::HashMap;

use jsonc_parser::cst::{CstContainerNode, CstNode, CstRootNode};
use jsonc_parser::errors::ParseErrorKind;
use jsonc_parser::tokens::Token;
use jsonc_parser::{ParseOptions, Scanner, ScannerOptions};

use crate::core::patch::escape_segment;
use crate::core::security::redact::mask_unparseable;
use crate::core::types::{Diagnostic, DiagnosticSource, DocKind, LineEnding, Severity, SourceView};
use crate::core::{CoreError, ErrorCode};

use super::cst::kind_of;
use super::syntax::Syntax;

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Strict for `.json`; `.jsonc` adds comments and trailing commas, nothing JSON5-ish (spec §9).
pub fn parse_options(kind: DocKind) -> ParseOptions {
    let jsonc = kind == DocKind::Jsonc;
    ParseOptions {
        allow_comments: jsonc,
        allow_trailing_commas: jsonc,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

/// A syntax error with a plain-language message (no document content) and 1-based position.
pub struct SyntaxError {
    pub message: String,
    pub line: u32,
    pub column: u32,
}

/// Parse into the CST with this kind's rules.
///
/// jsonc-parser's CST mode collects comments as tokens and then never applies
/// `allow_comments`, so strict `.json` rejects comments here instead.
pub fn parse_cst(text: &str, kind: DocKind) -> Result<CstRootNode, SyntaxError> {
    let root = CstRootNode::parse(text, &parse_options(kind)).map_err(|e| {
        let message = match (e.kind(), kind) {
            (ParseErrorKind::TrailingCommasNotAllowed, DocKind::Json) => {
                "Trailing commas are not allowed in .json files".to_string()
            }
            (other, _) => other.to_string(),
        };
        SyntaxError {
            message,
            line: e.line_display() as u32,
            column: e.column_display() as u32,
        }
    })?;
    if kind == DocKind::Json
        && let Some(at) = first_comment(text)
    {
        let (line, column) = line_col(text, at);
        return Err(SyntaxError {
            message: "Comments are not allowed in .json files".to_string(),
            line,
            column,
        });
    }
    Ok(root)
}

/// Byte offset of the first comment token, if any.
fn first_comment(text: &str) -> Option<usize> {
    // Strings may contain `//`, so a hit is only a reason to tokenize.
    if !text.contains("//") && !text.contains("/*") {
        return None;
    }
    let mut scanner = Scanner::new(text, &ScannerOptions::default());
    while let Ok(Some(token)) = scanner.scan() {
        if matches!(token, Token::CommentLine(_) | Token::CommentBlock(_)) {
            return Some(scanner.token_start());
        }
    }
    None
}

/// Why a file cannot be edited, with what the UI needs to explain it.
pub struct Blocked {
    pub diagnostics: Vec<Diagnostic>,
    /// Masked source and the original text of each mask id.
    pub source: Option<(SourceView, Vec<String>)>,
}

impl Blocked {
    /// The first diagnostic as a typed error (for APIs that return `CoreResult`).
    pub fn to_error(&self) -> CoreError {
        let d = &self.diagnostics[0];
        let code = match d.code.as_str() {
            "DUPLICATE_KEYS" => ErrorCode::DuplicateKeys,
            "UNSUPPORTED_ENCODING" => ErrorCode::UnsupportedEncoding,
            _ => ErrorCode::InvalidJson,
        };
        let mut err = CoreError::new(code, d.message.clone());
        if let (Some(line), Some(column)) = (d.line, d.column) {
            err = err.with_position(line, column);
        }
        err
    }
}

pub struct Loaded {
    pub has_bom: bool,
    pub line_ending: LineEnding,
    pub result: Result<Box<dyn Syntax>, Box<Blocked>>,
}

/// Parse file bytes. Never fails; problems become [`Blocked`].
pub fn load(bytes: &[u8], kind: DocKind) -> Loaded {
    let has_bom = bytes.starts_with(&BOM);
    let body = if has_bom { &bytes[BOM.len()..] } else { bytes };
    let line_ending = detect_line_ending(body);
    let result = parse_body(body, kind);
    Loaded {
        has_bom,
        line_ending,
        result,
    }
}

pub fn file_diag(code: &str, message: &str) -> Box<Blocked> {
    Box::new(Blocked {
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::File,
            code: code.to_string(),
            message: message.to_string(),
            pointer: None,
            line: None,
            column: None,
        }],
        source: None,
    })
}

fn parse_body(body: &[u8], kind: DocKind) -> Result<Box<dyn Syntax>, Box<Blocked>> {
    let Ok(text) = std::str::from_utf8(body) else {
        return Err(file_diag(
            "UNSUPPORTED_ENCODING",
            "The file is not valid UTF-8 text. Re-save it as UTF-8 in another editor.",
        ));
    };
    // An empty TOML/YAML file is a valid empty document; empty JSON is not.
    if matches!(kind, DocKind::Json | DocKind::Jsonc) && text.trim().is_empty() {
        return Err(file_diag(
            "EMPTY_FILE",
            "The file is empty. Start from an empty object `{}` to edit it here.",
        ));
    }
    super::syntax::parse(text, kind)
}

/// Parse JSON/JSONC text into the CST, blocking on syntax errors, a missing
/// value, and duplicate keys.
pub fn parse_json(text: &str, kind: DocKind) -> Result<CstRootNode, Box<Blocked>> {
    let root = match parse_cst(text, kind) {
        Ok(root) => root,
        Err(e) => {
            return Err(Box::new(Blocked {
                diagnostics: vec![Diagnostic {
                    severity: Severity::Error,
                    source: DiagnosticSource::Syntax,
                    code: "INVALID_JSON".to_string(),
                    message: e.message,
                    pointer: None,
                    line: Some(e.line),
                    column: Some(e.column),
                }],
                source: Some(mask_unparseable(text, Some(e.line), Some(e.column))),
            }));
        }
    };
    if root.value().is_none() {
        return Err(file_diag(
            "EMPTY_FILE",
            "The file has comments but no JSON value.",
        ));
    }
    let dups = find_duplicates(&root);
    if dups.is_empty() {
        return Ok(root);
    }
    let diagnostics: Vec<Diagnostic> = dups
        .iter()
        .map(|d| {
            let positions: Vec<(u32, u32)> = d.offsets.iter().map(|o| line_col(text, *o)).collect();
            let lines: Vec<String> = positions.iter().map(|(l, _)| l.to_string()).collect();
            let listed = match lines.split_last() {
                Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                _ => lines.join(""),
            };
            Diagnostic {
                severity: Severity::Error,
                source: DiagnosticSource::DuplicateKey,
                code: "DUPLICATE_KEYS".to_string(),
                message: format!(
                    "Key `{}` appears {} times: lines {listed}",
                    d.key,
                    d.offsets.len()
                ),
                pointer: Some(d.pointer.clone()),
                line: Some(positions[0].0),
                column: Some(positions[0].1),
            }
        })
        .collect();
    let (first_line, first_col) = (diagnostics[0].line, diagnostics[0].column);
    let (mut view, originals) = mask_unparseable(text, first_line, first_col);
    let mut lines: Vec<u32> = dups
        .iter()
        .flat_map(|d| d.offsets.iter().map(|o| line_col(text, *o).0))
        .collect();
    lines.sort_unstable();
    lines.dedup();
    view.highlight_lines = lines;
    Err(Box::new(Blocked {
        diagnostics,
        source: Some((view, originals)),
    }))
}

/// A key that appears more than once in one object.
pub struct Duplicate {
    /// Pointer of the (first) occurrence.
    pub pointer: String,
    pub key: String,
    /// Byte offsets of every occurrence's key literal.
    pub offsets: Vec<usize>,
}

/// Walk the whole CST (nested objects too) and report duplicate member names.
/// The CST keeps duplicates silently and `get()` returns the first (eng R3).
pub fn find_duplicates(root: &CstRootNode) -> Vec<Duplicate> {
    let mut out = Vec::new();
    let mut offset = 0;
    walk(
        &CstNode::Container(CstContainerNode::Root(root.clone())),
        "",
        &mut offset,
        &mut out,
    );
    out
}

fn walk(node: &CstNode, pointer: &str, offset: &mut usize, out: &mut Vec<Duplicate>) {
    let CstNode::Container(container) = node else {
        *offset += node.to_string().len();
        return;
    };
    match container {
        CstContainerNode::Object(_) => {
            let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
            let mut order: Vec<String> = Vec::new();
            for child in container.children() {
                if let CstNode::Container(CstContainerNode::ObjectProp(prop)) = &child {
                    let name = prop.decoded_name().unwrap_or_default();
                    let group = groups.entry(name.clone()).or_default();
                    if group.is_empty() {
                        order.push(name.clone());
                    }
                    group.push(*offset);
                    let child_pointer = format!("{pointer}/{}", escape_segment(&name));
                    walk(&child, &child_pointer, offset, out);
                } else {
                    walk(&child, pointer, offset, out);
                }
            }
            for name in order {
                let offsets = groups.remove(&name).unwrap_or_default();
                if offsets.len() > 1 {
                    out.push(Duplicate {
                        pointer: format!("{pointer}/{}", escape_segment(&name)),
                        key: name,
                        offsets,
                    });
                }
            }
        }
        CstContainerNode::Array(_) => {
            let mut index = 0;
            for child in container.children() {
                let child_pointer = format!("{pointer}/{index}");
                if kind_of(&child).is_some() {
                    index += 1;
                    walk(&child, &child_pointer, offset, out);
                } else {
                    walk(&child, pointer, offset, out);
                }
            }
        }
        // Root and property containers: children keep this pointer (a property's
        // value is its only value child).
        _ => {
            for child in container.children() {
                walk(&child, pointer, offset, out);
            }
        }
    }
}

/// 1-based line and column (in chars) of a byte offset.
pub fn line_col(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() as u32 + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) as u32 + 1;
    (line, column)
}

/// LF, CRLF, mixed, or none, from the newlines in `body`.
pub fn detect_line_ending(body: &[u8]) -> LineEnding {
    let (mut lf, mut crlf) = (0usize, 0usize);
    for (i, b) in body.iter().enumerate() {
        if *b == b'\n' {
            if i > 0 && body[i - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
    }
    match (lf, crlf) {
        (0, 0) => LineEnding::None,
        (_, 0) => LineEnding::Lf,
        (0, _) => LineEnding::Crlf,
        _ => LineEnding::Mixed,
    }
}
