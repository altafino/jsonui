//! The seam between the session and each file syntax.
//!
//! A [`Syntax`] is one parsed, editable document. JSON/JSONC implement it on
//! the jsonc-parser CST directly (`JsonSyntax`). Other formats implement the
//! six primitive edits of [`Edit`] on their own lossless tree; [`apply_edit`]
//! then validates every op and computes its effect against the neutral
//! [`Val`], so all formats share one set of op semantics and messages.

use jsonc_parser::cst::{CstContainerNode, CstLeafNode, CstNode, CstRootNode};

use crate::core::patch::{escape_segment, parse_pointer, to_pointer};
use crate::core::types::{ChangeKind, DocKind, DraftOp, Pointer, TreeNode};
use crate::core::{CoreError, CoreResult, ErrorCode};

use super::cst::{self, parse_index};
use super::ops::{self, OpEffect};
use super::scan::Blocked;
use super::value::{self, Val};
use super::{SecretPredicate, toml, yaml};

/// One parsed, editable document.
pub trait Syntax {
    /// Apply one op in place. On error the document is unchanged.
    fn apply(&mut self, op: &DraftOp) -> CoreResult<OpEffect>;
    /// Current text, without BOM. Untouched regions are byte-identical.
    fn render(&self) -> String;
    /// The neutral value of the whole document.
    fn value(&self) -> Val;
    /// Raw source lexemes of the scalars at or under a secret pointer,
    /// masked wherever they appear in review diffs.
    fn secret_lexemes(&self, secret: SecretPredicate) -> Vec<String>;
    /// UI tree at `pointer`; `None` when it does not exist.
    fn tree(&self, pointer: &str, depth: u32, touched: &[Pointer]) -> Option<TreeNode> {
        let segments = parse_pointer(pointer).ok()?;
        let root = self.value();
        let node = root.lookup(&segments)?;
        Some(value::tree(
            node,
            pointer,
            key_of(&root, &segments),
            index_of(&segments),
            depth,
            touched,
        ))
    }
}

/// Parse `text` (no BOM, valid UTF-8, not blank) as `kind`.
pub fn parse(text: &str, kind: DocKind) -> Result<Box<dyn Syntax>, Box<Blocked>> {
    match kind {
        DocKind::Json | DocKind::Jsonc => {
            super::scan::parse_json(text, kind).map(|r| Box::new(JsonSyntax(r)) as Box<dyn Syntax>)
        }
        DocKind::Yaml => yaml::parse(text).map(|d| Box::new(d) as Box<dyn Syntax>),
        DocKind::Toml => toml::parse(text).map(|d| Box::new(d) as Box<dyn Syntax>),
    }
}

fn key_of(root: &Val, segments: &[String]) -> Option<String> {
    let (last, parent) = segments.split_last()?;
    matches!(root.lookup(parent)?, Val::Object(_)).then(|| last.clone())
}

fn index_of(segments: &[String]) -> Option<u32> {
    // Only meaningful for array items; key_of wins for object members.
    segments
        .last()
        .and_then(|s| parse_index(s))
        .map(|i| i as u32)
}

// ---------------------------------------------------------------- JSON/JSONC

pub struct JsonSyntax(pub CstRootNode);

impl Syntax for JsonSyntax {
    fn apply(&mut self, op: &DraftOp) -> CoreResult<OpEffect> {
        ops::apply_op(&self.0, op)
    }

    fn render(&self) -> String {
        self.0.to_string()
    }

    fn value(&self) -> Val {
        self.0
            .value()
            .map_or(Val::Null, |n| cst::node_to_value(&n).into())
    }

    fn secret_lexemes(&self, secret: SecretPredicate) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(value) = self.0.value() {
            json_secret_walk(&value, "", false, secret, &mut out);
        }
        out
    }

    // The CST walk avoids building the whole value tree per edit.
    fn tree(&self, pointer: &str, depth: u32, touched: &[Pointer]) -> Option<TreeNode> {
        let segments = parse_pointer(pointer).ok()?;
        let target = cst::resolve(&self.0, &segments)?;
        let (key, index) = match (segments.last(), &target.prop) {
            (Some(last), Some(_)) => (Some(last.clone()), None),
            (Some(last), None) => (None, parse_index(last).map(|i| i as u32)),
            (None, _) => (None, None),
        };
        Some(cst::build_tree(
            &target.node,
            pointer,
            key,
            index,
            depth,
            touched,
        ))
    }
}

fn json_secret_walk(
    node: &CstNode,
    pointer: &str,
    inherited: bool,
    secret: SecretPredicate,
    out: &mut Vec<String>,
) {
    let here = inherited || secret(pointer);
    match node {
        CstNode::Container(CstContainerNode::Object(object)) => {
            for prop in object.properties() {
                if let (Some(name), Some(value)) = (prop.decoded_name(), prop.value()) {
                    let p = format!("{pointer}/{}", escape_segment(&name));
                    json_secret_walk(&value, &p, here, secret, out);
                }
            }
        }
        CstNode::Container(CstContainerNode::Array(array)) => {
            for (i, item) in array.elements().iter().enumerate() {
                json_secret_walk(item, &format!("{pointer}/{i}"), here, secret, out);
            }
        }
        CstNode::Leaf(CstLeafNode::StringLit(_) | CstLeafNode::NumberLit(_)) if here => {
            let lexeme = node.to_string();
            if !lexeme.trim_matches('"').is_empty() {
                out.push(lexeme);
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------- shared op semantics

/// Primitive, format-preserving edits on a lossless tree. Callers
/// ([`apply_edit`]) have already checked that paths exist, kinds fit, indices
/// are in range and keys are free; implementations only edit, keeping every
/// byte outside the edited node unchanged where the format allows.
pub trait Edit {
    fn value(&self) -> Val;
    /// Replace the value at `path` (`[]` = the document root).
    fn set(&mut self, path: &[String], value: &Val) -> CoreResult<()>;
    /// Append member `key` to the object at `parent`.
    fn add(&mut self, parent: &[String], key: &str, value: &Val) -> CoreResult<()>;
    /// Remove the object member or array item at `path` (never the root).
    fn remove(&mut self, path: &[String]) -> CoreResult<()>;
    /// Insert into the array at `array`; `index == len` appends.
    fn insert(&mut self, array: &[String], index: usize, value: &Val) -> CoreResult<()>;
    /// Move an item within the array at `array` (`from != to`).
    fn move_item(&mut self, array: &[String], from: usize, to: usize) -> CoreResult<()>;
    /// Rename the object member at `path` to `new_key` (keeps its position).
    fn rename(&mut self, path: &[String], new_key: &str) -> CoreResult<()>;
}

fn invalid(path: &str, message: impl Into<String>) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, message).with_path(path)
}

fn missing(path: &str) -> CoreError {
    invalid(path, "The path does not exist in the document")
}

fn duplicate(path: &str, key: &str, remediation: &str) -> CoreError {
    CoreError::new(
        ErrorCode::DuplicateKeys,
        format!("The key `{key}` already exists"),
    )
    .with_path(path)
    .with_remediation(remediation)
}

fn child_pointer(parent: &str, key: &str) -> Pointer {
    format!("{parent}/{}", escape_segment(key))
}

/// The value of `valueJson` (strict JSON, number lexemes kept).
pub fn parse_value(value_json: &str) -> CoreResult<Val> {
    ops::parse_value(value_json).map(Val::from)
}

/// Validate `op` against the current value, run the primitive, and describe
/// the effect exactly as the JSON engine does (same kinds, paths, messages).
pub fn apply_edit(doc: &mut impl Edit, op: &DraftOp) -> CoreResult<OpEffect> {
    let before = doc.value();
    let at = |path: &str| -> CoreResult<(Vec<String>, &Val)> {
        let segments = parse_pointer(path)?;
        let node = before.lookup(&segments).ok_or_else(|| missing(path))?;
        Ok((segments, node))
    };
    let array_len = |path: &str| -> CoreResult<(Vec<String>, usize)> {
        match at(path)? {
            (segments, Val::Array(items)) => Ok((segments, items.len())),
            _ => Err(invalid(path, "The path is not an array")),
        }
    };
    let shown = |doc: &dyn Fn() -> Val, p: &str| {
        let segments = parse_pointer(p).unwrap_or_default();
        doc().lookup(&segments).map(Val::display)
    };
    match op {
        DraftOp::Set { path, value_json } => {
            let value = parse_value(value_json)?;
            let (segments, old) = at(path)?;
            let old = old.display();
            doc.set(&segments, &value)?;
            Ok(OpEffect {
                kind: ChangeKind::Changed,
                pointer: path.clone(),
                old: Some(old),
                new: shown(&|| doc.value(), path),
                touched: vec![path.clone()],
                reshaped: Vec::new(),
                paths: vec![path.clone()],
            })
        }
        DraftOp::Add { path, value_json } => {
            let value = parse_value(value_json)?;
            let segments = parse_pointer(path)?;
            let Some((key, parent)) = segments.split_last() else {
                return Err(invalid(path, "Cannot add at the root; use set"));
            };
            let parent_pointer = to_pointer(parent);
            match before.lookup(parent) {
                None => return Err(missing(&parent_pointer)),
                Some(Val::Object(members)) => {
                    if members.iter().any(|(k, _)| k == key) {
                        return Err(duplicate(path, key, "Pick a different key name."));
                    }
                }
                Some(_) => {
                    return Err(invalid(
                        path,
                        "Add needs an object parent; use array-insert for arrays",
                    ));
                }
            }
            doc.add(parent, key, &value)?;
            Ok(OpEffect {
                kind: ChangeKind::Added,
                pointer: path.clone(),
                old: None,
                new: shown(&|| doc.value(), path),
                touched: vec![path.clone()],
                reshaped: vec![parent_pointer],
                paths: vec![path.clone()],
            })
        }
        DraftOp::Remove { path } => {
            let (segments, old) = at(path)?;
            if segments.is_empty() {
                return Err(invalid(path, "The document root cannot be removed"));
            }
            let old = old.display();
            doc.remove(&segments)?;
            Ok(OpEffect {
                kind: ChangeKind::Removed,
                pointer: path.clone(),
                old: Some(old),
                new: None,
                touched: vec![path.clone()],
                reshaped: vec![to_pointer(&segments[..segments.len() - 1])],
                paths: vec![path.clone()],
            })
        }
        DraftOp::ArrayInsert {
            path,
            index,
            value_json,
        } => {
            let value = parse_value(value_json)?;
            let (segments, len) = array_len(path)?;
            let index = *index as usize;
            if index > len {
                return Err(invalid(
                    path,
                    format!("Index {index} is out of range for an array of {len} items"),
                ));
            }
            doc.insert(&segments, index, &value)?;
            let item = child_pointer(path, &index.to_string());
            Ok(OpEffect {
                kind: ChangeKind::Added,
                pointer: item.clone(),
                old: None,
                new: shown(&|| doc.value(), &item),
                touched: vec![item],
                reshaped: vec![path.clone()],
                paths: vec![path.clone()],
            })
        }
        DraftOp::ArrayMove { path, from, to } => {
            let (segments, len) = array_len(path)?;
            let (from, to) = (*from as usize, *to as usize);
            if from >= len || to >= len {
                return Err(invalid(
                    path,
                    format!("Move {from} -> {to} is out of range for an array of {len} items"),
                ));
            }
            if from != to {
                doc.move_item(&segments, from, to)?;
            }
            let item = child_pointer(path, &to.to_string());
            Ok(OpEffect {
                kind: ChangeKind::Moved,
                pointer: item.clone(),
                old: Some(format!("position {}", from + 1)),
                new: Some(format!("position {}", to + 1)),
                touched: vec![child_pointer(path, &from.to_string()), item],
                reshaped: vec![path.clone()],
                paths: vec![path.clone()],
            })
        }
        DraftOp::RenameKey { path, new_key } => {
            let segments = parse_pointer(path)?;
            let Some((old_key, parent)) = segments.split_last() else {
                return Err(invalid(path, "The document root has no key"));
            };
            let Some(Val::Object(members)) = before.lookup(parent) else {
                return Err(match before.lookup(&segments) {
                    None => missing(path),
                    Some(_) => invalid(path, "Only object members can be renamed"),
                });
            };
            if !members.iter().any(|(k, _)| k == old_key) {
                return Err(missing(path));
            }
            let parent_pointer = to_pointer(parent);
            let new_pointer = child_pointer(&parent_pointer, new_key);
            if new_key != old_key {
                if members.iter().any(|(k, _)| k == new_key) {
                    return Err(duplicate(
                        &new_pointer,
                        new_key,
                        "Pick a key name that is not used in this object.",
                    ));
                }
                doc.rename(&segments, new_key)?;
            }
            Ok(OpEffect {
                kind: ChangeKind::Renamed,
                pointer: new_pointer.clone(),
                old: Some(old_key.clone()),
                new: Some(new_key.clone()),
                touched: vec![path.clone(), new_pointer.clone()],
                reshaped: vec![parent_pointer],
                paths: vec![path.clone(), new_pointer],
            })
        }
    }
}
