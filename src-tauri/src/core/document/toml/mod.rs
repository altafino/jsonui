//! TOML syntax: lossless editing on `toml_edit`.
//!
//! Each primitive edit runs on a clone of the tree; the result is rendered,
//! parsed again and compared with the value the op should produce. Only a
//! result that re-parses to exactly the expected data replaces the live tree
//! (which is then the re-parsed one), so a library quirk can refuse an edit
//! but never corrupt a file. Layouts `toml_edit` cannot print back
//! byte-for-byte (e.g. dotted keys interleaved with other keys) open read-only.

mod edit;
mod layout;
mod mask;
mod read;
#[cfg(test)]
mod tests;
mod write;

use std::str::FromStr;

use toml_edit::DocumentMut;

use crate::core::patch::to_pointer;
use crate::core::types::{Diagnostic, DiagnosticSource, DraftOp, Severity};
use crate::core::{CoreError, CoreResult, ErrorCode};

use super::SecretPredicate;
use super::ops::OpEffect;
use super::scan::{Blocked, line_col};
use super::syntax::{Edit, Syntax, apply_edit, parse_value};
use super::value::Val;
use write::{NULL_MESSAGE, contains_null};

pub struct TomlDoc {
    doc: DocumentMut,
    /// Every line ends in CRLF: new lines must too.
    crlf: bool,
    /// The file ends with a newline (the writer always adds one).
    final_newline: bool,
}

fn blocked(
    text: &str,
    source: DiagnosticSource,
    code: &str,
    message: String,
    at: Option<usize>,
) -> Box<Blocked> {
    let (line, column) = at.map(|o| line_col(text, o)).unzip();
    let (mut view, originals) = mask::mask_source(text, line, column);
    if source == DiagnosticSource::DuplicateKey {
        view.highlight_lines = line.into_iter().collect();
    }
    Box::new(Blocked {
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            source,
            code: code.to_string(),
            message,
            pointer: None,
            line,
            column,
        }],
        source: Some((view, originals)),
    })
}

pub fn parse(text: &str) -> Result<TomlDoc, Box<Blocked>> {
    let doc = DocumentMut::from_str(text).map_err(|e| {
        let at = e.span().map(|s| s.start);
        if e.message().contains("duplicate") {
            let (line, _) = at.map_or((0, 0), |o| line_col(text, o));
            blocked(
                text,
                DiagnosticSource::DuplicateKey,
                "DUPLICATE_KEYS",
                format!("A key or table is defined twice (line {line})"),
                at,
            )
        } else {
            blocked(
                text,
                DiagnosticSource::Syntax,
                "INVALID_TOML",
                format!("Invalid TOML: {}", e.message()),
                at,
            )
        }
    })?;
    let crlf = text.contains("\r\n") && !text.replace("\r\n", "").contains('\n');
    let parsed = TomlDoc {
        doc,
        crlf,
        final_newline: text.is_empty() || text.ends_with('\n'),
    };
    if parsed.render() != text {
        return Err(blocked(
            text,
            DiagnosticSource::File,
            "UNSUPPORTED_TOML_LAYOUT",
            "This file's layout (for example dotted keys spread over several places, or \
             mixed line endings) cannot be saved without reformatting it, so it is read-only."
                .to_string(),
            None,
        ));
    }
    Ok(parsed)
}

impl TomlDoc {
    fn finish(&self, mut text: String) -> String {
        if self.crlf {
            text = text.replace("\r\n", "\n").replace('\n', "\r\n");
        }
        if !self.final_newline {
            let eol = if self.crlf { "\r\n" } else { "\n" };
            if text.ends_with(eol) {
                text.truncate(text.len() - eol.len());
            }
        }
        text
    }

    /// Run `change` on a copy; keep it only if it re-parses to `expected`.
    fn commit(
        &mut self,
        path: &[String],
        expected: &Val,
        change: impl FnOnce(&mut DocumentMut) -> CoreResult<()>,
    ) -> CoreResult<()> {
        let mut next = self.doc.clone();
        change(&mut next)?;
        let text = self.finish(next.to_string());
        let unsafe_edit = || {
            CoreError::new(
                ErrorCode::InvalidOperation,
                "This change cannot be written to the TOML file without risking other settings",
            )
            .with_path(to_pointer(path))
        };
        let reparsed = DocumentMut::from_str(&text).map_err(|_| unsafe_edit())?;
        if !read::same_data(&read::table_val(reparsed.as_table()), expected) {
            return Err(unsafe_edit());
        }
        let candidate = Self {
            doc: reparsed,
            crlf: self.crlf,
            final_newline: self.final_newline,
        };
        if candidate.render() != text {
            return Err(unsafe_edit());
        }
        *self = candidate;
        Ok(())
    }
}

// ------------------------------------------------- expected-value model

fn slot<'a>(v: &'a mut Val, path: &[String]) -> Option<&'a mut Val> {
    let mut cur = v;
    for seg in path {
        cur = match cur {
            Val::Object(m) => &mut m.iter_mut().find(|(k, _)| k == seg)?.1,
            Val::Array(a) => a.get_mut(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn expect(before: Val, path: &[String], f: impl FnOnce(&mut Val) -> Option<()>) -> CoreResult<Val> {
    let mut v = before;
    f(&mut v).ok_or_else(|| {
        CoreError::new(ErrorCode::InvalidOperation, "The path does not exist")
            .with_path(to_pointer(path))
    })?;
    Ok(v)
}

impl Edit for TomlDoc {
    fn value(&self) -> Val {
        read::table_val(self.doc.as_table())
    }

    fn set(&mut self, path: &[String], value: &Val) -> CoreResult<()> {
        let expected = expect(Edit::value(self), path, |v| {
            *slot(v, path)? = value.clone();
            Some(())
        })?;
        let crlf = self.crlf;
        self.commit(path, &expected, |d| edit::set(d, path, value, crlf))
    }

    fn add(&mut self, parent: &[String], key: &str, value: &Val) -> CoreResult<()> {
        let expected = expect(Edit::value(self), parent, |v| {
            let Val::Object(m) = slot(v, parent)? else {
                return None;
            };
            m.push((key.to_string(), value.clone()));
            Some(())
        })?;
        let crlf = self.crlf;
        self.commit(parent, &expected, |d| {
            edit::add(d, parent, key, value, crlf)
        })
    }

    fn remove(&mut self, path: &[String]) -> CoreResult<()> {
        let (last, parent) = path.split_last().ok_or_else(|| {
            CoreError::new(
                ErrorCode::InvalidOperation,
                "The document root cannot be removed",
            )
        })?;
        let expected = expect(Edit::value(self), path, |v| {
            match slot(v, parent)? {
                Val::Object(m) => m.retain(|(k, _)| k != last),
                Val::Array(a) => {
                    a.remove(last.parse::<usize>().ok().filter(|i| *i < a.len())?);
                }
                _ => return None,
            }
            Some(())
        })?;
        self.commit(path, &expected, |d| edit::remove(d, path))
    }

    fn insert(&mut self, array: &[String], index: usize, value: &Val) -> CoreResult<()> {
        let expected = expect(Edit::value(self), array, |v| {
            let Val::Array(a) = slot(v, array)? else {
                return None;
            };
            a.insert(index, value.clone());
            Some(())
        })?;
        let crlf = self.crlf;
        self.commit(array, &expected, |d| {
            edit::insert(d, array, index, value, crlf)
        })
    }

    fn move_item(&mut self, array: &[String], from: usize, to: usize) -> CoreResult<()> {
        let expected = expect(Edit::value(self), array, |v| {
            let Val::Array(a) = slot(v, array)? else {
                return None;
            };
            let item = a.remove(from);
            a.insert(to, item);
            Some(())
        })?;
        self.commit(array, &expected, |d| edit::move_item(d, array, from, to))
    }

    fn rename(&mut self, path: &[String], new_key: &str) -> CoreResult<()> {
        let (last, parent) = path.split_last().ok_or_else(|| {
            CoreError::new(ErrorCode::InvalidOperation, "The document root has no key")
        })?;
        let expected = expect(Edit::value(self), path, |v| {
            let Val::Object(m) = slot(v, parent)? else {
                return None;
            };
            m.iter_mut().find(|(k, _)| k == last)?.0 = new_key.to_string();
            Some(())
        })?;
        self.commit(path, &expected, |d| edit::rename(d, path, new_key))
    }
}

impl Syntax for TomlDoc {
    fn apply(&mut self, op: &DraftOp) -> CoreResult<OpEffect> {
        let new = match op {
            DraftOp::Set { path, value_json }
            | DraftOp::Add { path, value_json }
            | DraftOp::ArrayInsert {
                path, value_json, ..
            } => parse_value(value_json).ok().map(|v| (path, v)),
            _ => None,
        };
        if let Some((path, value)) = new
            && contains_null(&value)
        {
            return Err(CoreError::new(ErrorCode::InvalidOperation, NULL_MESSAGE).with_path(path));
        }
        apply_edit(self, op)
    }

    fn render(&self) -> String {
        self.finish(self.doc.to_string())
    }

    fn value(&self) -> Val {
        Edit::value(self)
    }

    fn secret_lexemes(&self, secret: SecretPredicate) -> Vec<String> {
        read::secret_lexemes(self.doc.as_table(), secret)
    }
}
