//! YAML syntax: lossless editing on the `yaml-edit` parser.
//!
//! `model` reads the parse into positioned nodes, `edit` turns each of the six
//! primitive edits into text splices, so only the touched bytes change.
//! Every edit is re-parsed and compared with the value it should produce; an
//! edit that would not round-trip is refused instead of written.

mod edit;
mod mask;
mod model;
mod render;
mod scalar;
#[cfg(test)]
mod tests;

use std::panic::{AssertUnwindSafe, catch_unwind};

use self::edit::{Act, Reason, Splice, apply};
use self::model::{Fail, Kind, Node, Parsed};
use self::scalar::Style;
use super::SecretPredicate;
use super::ops::OpEffect;
use super::scan::{Blocked, line_col};
use super::syntax::{Edit, Syntax, apply_edit};
use super::value::Val;
use crate::core::patch::{escape_segment, to_pointer};
use crate::core::types::{Diagnostic, DiagnosticSource, DraftOp, Severity};
use crate::core::{CoreError, CoreResult, ErrorCode};

pub struct YamlDoc {
    p: Parsed,
}

/// Parse behind a panic guard: the lossless parser must never take the app down.
fn safe_parse(text: &str) -> Result<Parsed, Fail> {
    catch_unwind(AssertUnwindSafe(|| Parsed::parse(text))).unwrap_or(Err(Fail::Syntax {
        line: 1,
        column: 1,
        what: "invalid syntax",
    }))
}

pub fn parse(text: &str) -> Result<YamlDoc, Box<Blocked>> {
    safe_parse(text)
        .map(|p| YamlDoc { p })
        .map_err(|f| blocked(text, f))
}

fn diag(
    source: DiagnosticSource,
    code: &str,
    message: String,
    pointer: Option<String>,
    at: Option<(u32, u32)>,
) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        source,
        code: code.to_string(),
        message,
        pointer,
        line: at.map(|a| a.0),
        column: at.map(|a| a.1),
    }
}

fn blocked(text: &str, fail: Fail) -> Box<Blocked> {
    let (diagnostics, highlight, caret) = match fail {
        Fail::Syntax { line, column, what } => {
            // A parser error at the end of input points past the last line.
            let line = line.min(text.lines().count().max(1) as u32);
            (
                vec![diag(
                    DiagnosticSource::Syntax,
                    "INVALID_YAML",
                    format!("The YAML is not valid: {what}."),
                    None,
                    Some((line, column)),
                )],
                vec![line],
                Some((line, column)),
            )
        }
        Fail::Invalid { pos, what } => {
            let at = line_col(text, pos);
            (
                vec![diag(
                    DiagnosticSource::Syntax,
                    "INVALID_YAML",
                    format!("The YAML is not valid: {what}."),
                    None,
                    Some(at),
                )],
                vec![at.0],
                Some(at),
            )
        }
        Fail::Unsupported { pos, what } => {
            let at = line_col(text, pos);
            (
                vec![diag(
                    DiagnosticSource::File,
                    "UNSUPPORTED_YAML",
                    format!("This file uses {what}, which can't be edited here yet."),
                    None,
                    Some(at),
                )],
                vec![at.0],
                Some(at),
            )
        }
        Fail::Multi => (
            vec![diag(
                DiagnosticSource::File,
                "MULTIPLE_DOCUMENTS",
                "Files with several YAML documents can't be edited here yet.".to_string(),
                None,
                None,
            )],
            Vec::new(),
            None,
        ),
        Fail::Duplicates(dups) => {
            let diagnostics: Vec<Diagnostic> = dups
                .iter()
                .map(|d| {
                    let positions: Vec<(u32, u32)> =
                        d.offsets.iter().map(|o| line_col(text, *o)).collect();
                    let lines: Vec<String> = positions.iter().map(|(l, _)| l.to_string()).collect();
                    let listed = match lines.split_last() {
                        Some((last, rest)) if !rest.is_empty() => {
                            format!("{} and {last}", rest.join(", "))
                        }
                        _ => lines.join(""),
                    };
                    diag(
                        DiagnosticSource::DuplicateKey,
                        "DUPLICATE_KEYS",
                        format!(
                            "Key `{}` appears {} times: lines {listed}",
                            d.key,
                            d.offsets.len()
                        ),
                        Some(d.pointer.clone()),
                        Some(positions[0]),
                    )
                })
                .collect();
            let mut lines: Vec<u32> = dups
                .iter()
                .flat_map(|d| d.offsets.iter().map(|o| line_col(text, *o).0))
                .collect();
            lines.sort_unstable();
            lines.dedup();
            let caret = diagnostics[0].line.zip(diagnostics[0].column);
            (diagnostics, lines, caret)
        }
    };
    let source = mask::masked_view(text, highlight, caret);
    Box::new(Blocked {
        diagnostics,
        source: Some(source),
    })
}

fn refuse(reason: Reason, path: &[String]) -> CoreError {
    let message = match reason {
        Reason::Shared => {
            "This value is shared through a YAML anchor or alias. Edit it in a text editor."
        }
        Reason::Tagged => "This value has a YAML tag. Edit it in a text editor.",
        Reason::Unsafe => {
            "This part of the YAML file cannot be changed safely here. Edit it in a text editor."
        }
    };
    CoreError::new(ErrorCode::InvalidOperation, message).with_path(to_pointer(path))
}

// ----------------------------------------------- expected value after an op

fn navigate<'a>(root: &'a mut Val, path: &[String]) -> Option<&'a mut Val> {
    let mut cur = root;
    for seg in path {
        cur = match cur {
            Val::Object(m) => &mut m.iter_mut().find(|(k, _)| k == seg)?.1,
            Val::Array(a) => a.get_mut(super::cst::parse_index(seg)?)?,
            _ => return None,
        };
    }
    Some(cur)
}

impl YamlDoc {
    /// Write `splices`, keeping them only if the result parses to `expected`.
    fn commit(&mut self, splices: Vec<Splice>, expected: Val, path: &[String]) -> CoreResult<()> {
        let text = apply(&self.p.text, splices);
        match safe_parse(&text) {
            Ok(next) if next.val == expected => {
                self.p = next;
                Ok(())
            }
            _ => {
                #[cfg(test)]
                eprintln!(
                    "UNSAFE EDIT at {path:?}\n--- before\n{}\n--- after\n{text}",
                    self.p.text
                );
                Err(refuse(Reason::Unsafe, path))
            }
        }
    }

    fn expect(&self, path: &[String], f: impl FnOnce(&mut Val) -> Option<()>) -> CoreResult<Val> {
        let mut v = self.p.val.clone();
        navigate(&mut v, path)
            .and_then(f)
            .map(|()| v)
            .ok_or_else(|| refuse(Reason::Unsafe, path))
    }
}

impl Edit for YamlDoc {
    fn value(&self) -> Val {
        self.p.val.clone()
    }

    fn set(&mut self, path: &[String], value: &Val) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(path)?;
            self.p.guard(&hit, Act::Replace)?;
            self.p.replace(&hit, value)
        })()
        .map_err(|r| refuse(r, path))?;
        let expected = match path.split_last() {
            None => value.clone(),
            Some((last, parent)) => {
                let mut v = self.p.val.clone();
                let slot = navigate(&mut v, parent)
                    .and_then(|p| navigate(p, std::slice::from_ref(last)))
                    .ok_or_else(|| refuse(Reason::Unsafe, path))?;
                *slot = value.clone();
                v
            }
        };
        self.commit(splices, expected, path)
    }

    fn add(&mut self, parent: &[String], key: &str, value: &Val) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(parent)?;
            self.p.guard(&hit, Act::Container)?;
            self.p.add(&hit, key, value)
        })()
        .map_err(|r| refuse(r, parent))?;
        let expected = self.expect(parent, |v| match v {
            Val::Object(m) => {
                m.push((key.to_string(), value.clone()));
                Some(())
            }
            _ => None,
        })?;
        self.commit(splices, expected, parent)
    }

    fn remove(&mut self, path: &[String]) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(path)?;
            self.p.guard(&hit, Act::Remove)?;
            let (container, _) = match hit.link {
                edit::Link::Entry(c, i) | edit::Link::Item(c, i) => (c, i),
                edit::Link::Root => return Err(Reason::Unsafe),
            };
            let only = match &container.kind {
                Kind::Map { entries, .. } => entries.len() == 1,
                Kind::Seq { items, .. } => items.len() == 1,
                _ => return Err(Reason::Unsafe),
            };
            if only {
                // The container becomes `{}` / `[]` in its own slot.
                let parent_path = &path[..path.len() - 1];
                let parent = self.p.walk(parent_path)?;
                self.p.guard(&parent, Act::Container)?;
                let empty = if matches!(container.kind, Kind::Map { .. }) {
                    Val::Object(Vec::new())
                } else {
                    Val::Array(Vec::new())
                };
                self.p.replace(&parent, &empty)
            } else {
                self.p.remove_child(&hit)
            }
        })()
        .map_err(|r| refuse(r, path))?;
        let (last, parent) = path
            .split_last()
            .ok_or_else(|| refuse(Reason::Unsafe, path))?;
        let expected = self.expect(parent, |v| match v {
            Val::Object(m) => {
                let i = m.iter().position(|(k, _)| k == last)?;
                m.remove(i);
                Some(())
            }
            Val::Array(a) => {
                let i = super::cst::parse_index(last).filter(|i| *i < a.len())?;
                a.remove(i);
                Some(())
            }
            _ => None,
        })?;
        self.commit(splices, expected, path)
    }

    fn insert(&mut self, array: &[String], index: usize, value: &Val) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(array)?;
            self.p.guard(&hit, Act::Container)?;
            self.p.insert(&hit, index, value)
        })()
        .map_err(|r| refuse(r, array))?;
        let expected = self.expect(array, |v| match v {
            Val::Array(a) if index <= a.len() => {
                a.insert(index, value.clone());
                Some(())
            }
            _ => None,
        })?;
        self.commit(splices, expected, array)
    }

    fn move_item(&mut self, array: &[String], from: usize, to: usize) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(array)?;
            self.p.guard(&hit, Act::Move(from.min(to), from.max(to)))?;
            self.p.move_item(&hit, from, to)
        })()
        .map_err(|r| refuse(r, array))?;
        let expected = self.expect(array, |v| match v {
            Val::Array(a) if from < a.len() && to < a.len() => {
                let item = a.remove(from);
                a.insert(to, item);
                Some(())
            }
            _ => None,
        })?;
        self.commit(splices, expected, array)
    }

    fn rename(&mut self, path: &[String], new_key: &str) -> CoreResult<()> {
        let splices = (|| {
            let hit = self.p.walk(path)?;
            self.p.guard(&hit, Act::Rename)?;
            self.p.rename(&hit, new_key)
        })()
        .map_err(|r| refuse(r, path))?;
        let (last, parent) = path
            .split_last()
            .ok_or_else(|| refuse(Reason::Unsafe, path))?;
        let expected = self.expect(parent, |v| match v {
            Val::Object(m) => {
                m.iter_mut().find(|(k, _)| k == last)?.0 = new_key.to_string();
                Some(())
            }
            _ => None,
        })?;
        self.commit(splices, expected, path)
    }
}

impl Syntax for YamlDoc {
    fn apply(&mut self, op: &DraftOp) -> CoreResult<OpEffect> {
        apply_edit(self, op)
    }

    fn render(&self) -> String {
        self.p.text.clone()
    }

    fn value(&self) -> Val {
        Edit::value(self)
    }

    fn secret_lexemes(&self, secret: SecretPredicate) -> Vec<String> {
        let mut out = Vec::new();
        secret_walk(&self.p.root, &self.p.text, "", false, secret, &mut out);
        out
    }
}

fn secret_walk(
    node: &Node,
    text: &str,
    pointer: &str,
    inherited: bool,
    secret: SecretPredicate,
    out: &mut Vec<String>,
) {
    let here = inherited || secret(pointer);
    match &node.kind {
        Kind::Map { entries, .. } => {
            for e in entries {
                let p = format!("{pointer}/{}", escape_segment(&e.key));
                secret_walk(&e.value, text, &p, here, secret, out);
            }
        }
        Kind::Seq { items, .. } => {
            for (i, item) in items.iter().enumerate() {
                secret_walk(
                    &item.value,
                    text,
                    &format!("{pointer}/{i}"),
                    here,
                    secret,
                    out,
                );
            }
        }
        Kind::Scalar {
            style,
            text: decoded,
            typed: Val::String(_) | Val::Number(_),
            ..
        } if here && !decoded.is_empty() => {
            let raw = &text[node.start..node.end];
            if matches!(style, Style::Literal | Style::Folded) {
                out.extend(
                    raw.lines()
                        .skip(1)
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(String::from),
                );
            } else if raw.contains('\n') {
                // Diffs are per line: each line of a multi-line scalar.
                let lines: Vec<&str> = raw.lines().map(str::trim).collect();
                let last = lines.len() - 1;
                for (i, line) in lines.iter().enumerate() {
                    let line = if i == 0 {
                        line.trim_start_matches(['"', '\''])
                    } else {
                        line
                    };
                    let line = if i == last {
                        line.trim_end_matches(['"', '\''])
                    } else {
                        line
                    };
                    if !line.is_empty() {
                        out.push(line.to_string());
                    }
                }
            } else {
                out.push(raw.to_string());
            }
        }
        _ => {}
    }
}
