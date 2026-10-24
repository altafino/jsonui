//! Neutral values to TOML text, and back to `toml_edit` values.
//!
//! Everything is generated as TOML *text* and parsed again, so a value that
//! could not be written is refused here instead of corrupting the file.

use std::str::FromStr;

use toml_edit::Value;

use crate::core::{CoreError, CoreResult, ErrorCode};

use super::super::value::Val;

pub(super) const NULL_MESSAGE: &str = "TOML has no null value; remove the setting instead";

pub(super) fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, message)
}

/// Whether `value` holds a null anywhere.
pub(super) fn contains_null(value: &Val) -> bool {
    match value {
        Val::Null => true,
        Val::Array(items) => items.iter().any(contains_null),
        Val::Object(members) => members.iter().any(|(_, v)| contains_null(v)),
        _ => false,
    }
}

/// How an existing string value was written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum StrStyle {
    Basic,
    Literal,
    MultiBasic { lead_nl: bool },
    MultiLiteral { lead_nl: bool },
}

impl StrStyle {
    pub(super) fn of(repr: &str) -> Self {
        let lead = |rest: &str| rest.starts_with('\n') || rest.starts_with("\r\n");
        if let Some(rest) = repr.strip_prefix("\"\"\"") {
            Self::MultiBasic {
                lead_nl: lead(rest),
            }
        } else if let Some(rest) = repr.strip_prefix("'''") {
            Self::MultiLiteral {
                lead_nl: lead(rest),
            }
        } else if repr.starts_with('\'') {
            Self::Literal
        } else {
            Self::Basic
        }
    }
}

fn basic_body(s: &str, multiline: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' if multiline => out.push('\n'),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' || c == '\u{7f}' => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn decodes_to(text: &str, want: &str) -> bool {
    Value::from_str(text)
        .ok()
        .and_then(|v| v.as_str().map(|s| s == want))
        .unwrap_or(false)
}

/// TOML text for the string `s`, in `style` when `s` can use it.
pub(super) fn string_text(s: &str, style: Option<StrStyle>, crlf: bool) -> String {
    let plain = || format!("\"{}\"", basic_body(s, false));
    let multiline_ok = !(crlf && s.contains('\n'));
    let nl = |lead: bool| {
        if lead || s.starts_with('\n') {
            "\n"
        } else {
            ""
        }
    };
    let candidate = match style {
        Some(StrStyle::Literal) if !s.contains(['\'', '\n', '\r']) => format!("'{s}'"),
        Some(StrStyle::MultiBasic { lead_nl }) if multiline_ok => {
            format!("\"\"\"{}{}\"\"\"", nl(lead_nl), basic_body(s, true))
        }
        Some(StrStyle::MultiLiteral { lead_nl }) if multiline_ok && !s.contains('\r') => {
            format!("'''{}{s}'''", nl(lead_nl))
        }
        _ => return plain(),
    };
    if decodes_to(&candidate, s) {
        candidate
    } else {
        plain()
    }
}

/// A key as TOML writes it: bare when possible, else a basic string.
pub(super) fn key_text(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        key.to_string()
    } else {
        format!("\"{}\"", basic_body(key, false))
    }
}

fn number_text(lexeme: &str) -> CoreResult<String> {
    match Value::from_str(lexeme) {
        Ok(v) if v.is_integer() || v.is_float() => Ok(lexeme.to_string()),
        _ => Err(invalid(format!(
            "The number {lexeme} cannot be written in TOML (integers must fit 64 bits)"
        ))),
    }
}

/// Inline TOML text for `value`; `style` only applies to a top-level string.
pub(super) fn inline_text(value: &Val, style: Option<StrStyle>, crlf: bool) -> CoreResult<String> {
    Ok(match value {
        Val::Null => return Err(invalid(NULL_MESSAGE)),
        Val::Bool(b) => b.to_string(),
        Val::Number(n) => number_text(n)?,
        Val::String(s) => string_text(s, style, crlf),
        Val::Array(items) => {
            let parts: CoreResult<Vec<_>> =
                items.iter().map(|v| inline_text(v, None, crlf)).collect();
            format!("[{}]", parts?.join(", "))
        }
        Val::Object(members) if members.is_empty() => "{}".to_string(),
        Val::Object(members) => {
            let parts: CoreResult<Vec<_>> = members
                .iter()
                .map(|(k, v)| Ok(format!("{} = {}", key_text(k), inline_text(v, None, crlf)?)))
                .collect();
            format!("{{ {} }}", parts?.join(", "))
        }
    })
}

/// A `toml_edit` value for `value` with default (unset) decor.
pub(super) fn to_value(value: &Val, style: Option<StrStyle>, crlf: bool) -> CoreResult<Value> {
    let text = inline_text(value, style, crlf)?;
    let mut v = Value::from_str(&text)
        .map_err(|_| invalid("This value cannot be written in a TOML file"))?;
    v.decor_mut().clear();
    Ok(v)
}

/// A string that parses as a TOML date/time, exactly as written.
pub(super) fn datetime_value(s: &str) -> Option<Value> {
    if s.trim() != s || s.is_empty() {
        return None;
    }
    let mut v = Value::from_str(s).ok().filter(Value::is_datetime)?;
    v.decor_mut().clear();
    Some(v)
}
