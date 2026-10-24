//! Applying [`DraftOp`]s in place to the live CST.
//!
//! Every op validates first and mutates second, so a failed op leaves the
//! CST untouched. Edits go through the CST's own mutators, which keep the
//! trivia (whitespace, comments, trailing commas) of everything not edited.

use std::collections::HashMap;

use jsonc_parser::cst::{
    CstContainerNode, CstInputValue, CstLeafNode, CstNode, CstRootNode, ObjectPropName,
};

use crate::core::patch::{escape_segment, parse_pointer, to_pointer};
use crate::core::types::{ChangeKind, DocKind, DraftOp, Pointer};
use crate::core::{CoreError, CoreResult, ErrorCode};

use super::cst::{display, node_to_value, resolve};
use super::scan::{find_duplicates, parse_cst};

/// What an applied op did, recorded for the review/conflict lists and for
/// building the minimal [`EditResult`](crate::core::types::EditResult).
#[derive(Debug, Clone, PartialEq)]
pub struct OpEffect {
    pub kind: ChangeKind,
    /// Pointer the change list shows (the new location for renames/moves).
    pub pointer: Pointer,
    pub old: Option<String>,
    pub new: Option<String>,
    /// Pointers whose node (re)appeared, changed or vanished.
    pub touched: Vec<Pointer>,
    /// Containers whose child list changed.
    pub reshaped: Vec<Pointer>,
    /// Paths this op depends on, for `pointers_overlap` (revert, reapply).
    pub paths: Vec<Pointer>,
}

fn invalid(path: &str, message: impl Into<String>) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, message).with_path(path)
}

fn missing(path: &str) -> CoreError {
    invalid(path, "The path does not exist in the document")
}

/// Parse `valueJson` as strict JSON (all `ParseOptions` false) into a value
/// that keeps number lexemes. Rejects comments, trailing commas, duplicate keys.
pub fn parse_value(value_json: &str) -> CoreResult<CstInputValue> {
    let root = parse_cst(value_json, DocKind::Json).map_err(|e| {
        CoreError::new(
            ErrorCode::InvalidJson,
            format!("The value is not valid JSON: {}", e.message),
        )
        .with_position(e.line, e.column)
    })?;
    let node = root.value().ok_or_else(|| {
        CoreError::new(
            ErrorCode::InvalidJson,
            "The value is empty; expected one JSON value",
        )
    })?;
    if let Some(dup) = find_duplicates(&root).first() {
        return Err(CoreError::new(
            ErrorCode::DuplicateKeys,
            format!("The value repeats the key `{}`", dup.key),
        ));
    }
    Ok(node_to_value(&node))
}

/// Replace a value node. Booleans flip in place so only the token changes.
fn replace_node(node: CstNode, value: CstInputValue) -> Option<CstNode> {
    if let (CstNode::Leaf(CstLeafNode::BooleanLit(lit)), CstInputValue::Bool(v)) = (&node, &value) {
        lit.set_value(*v);
        return Some(node);
    }
    match node {
        CstNode::Container(CstContainerNode::Object(o)) => o.replace_with(value),
        CstNode::Container(CstContainerNode::Array(a)) => a.replace_with(value),
        CstNode::Leaf(CstLeafNode::StringLit(l)) => l.replace_with(value),
        CstNode::Leaf(CstLeafNode::WordLit(l)) => l.replace_with(value),
        CstNode::Leaf(CstLeafNode::NumberLit(l)) => l.replace_with(value),
        CstNode::Leaf(CstLeafNode::BooleanLit(l)) => l.replace_with(value),
        CstNode::Leaf(CstLeafNode::NullKeyword(l)) => l.replace_with(value),
        _ => None,
    }
}

fn child_pointer(parent: &str, key: &str) -> Pointer {
    format!("{parent}/{}", escape_segment(key))
}

/// Apply one op to `root`.
pub fn apply_op(root: &CstRootNode, op: &DraftOp) -> CoreResult<OpEffect> {
    match op {
        DraftOp::Set { path, value_json } => set(root, path, value_json),
        DraftOp::Add { path, value_json } => add(root, path, value_json),
        DraftOp::Remove { path } => remove(root, path),
        DraftOp::ArrayInsert {
            path,
            index,
            value_json,
        } => array_insert(root, path, *index as usize, value_json),
        DraftOp::ArrayMove { path, from, to } => {
            array_move(root, path, *from as usize, *to as usize)
        }
        DraftOp::RenameKey { path, new_key } => rename_key(root, path, new_key),
    }
}

fn set(root: &CstRootNode, path: &str, value_json: &str) -> CoreResult<OpEffect> {
    let value = parse_value(value_json)?;
    let segments = parse_pointer(path)?;
    let target = resolve(root, &segments).ok_or_else(|| missing(path))?;
    let old = display(&target.node);
    let new_node = if segments.is_empty() {
        root.set_value(value);
        root.value()
    } else {
        replace_node(target.node, value)
    }
    .ok_or_else(|| invalid(path, "The value could not be replaced"))?;
    Ok(OpEffect {
        kind: ChangeKind::Changed,
        pointer: path.to_string(),
        old: Some(old),
        new: Some(display(&new_node)),
        touched: vec![path.to_string()],
        reshaped: Vec::new(),
        paths: vec![path.to_string()],
    })
}

fn add(root: &CstRootNode, path: &str, value_json: &str) -> CoreResult<OpEffect> {
    let value = parse_value(value_json)?;
    let segments = parse_pointer(path)?;
    let Some((key, parent_segments)) = segments.split_last() else {
        return Err(invalid(path, "Cannot add at the root; use set"));
    };
    let parent_pointer = to_pointer(parent_segments);
    let parent = resolve(root, parent_segments).ok_or_else(|| missing(&parent_pointer))?;
    let CstNode::Container(CstContainerNode::Object(object)) = parent.node else {
        return Err(invalid(
            path,
            "Add needs an object parent; use array-insert for arrays",
        ));
    };
    if object.get(key).is_some() {
        return Err(CoreError::new(
            ErrorCode::DuplicateKeys,
            format!("The key `{key}` already exists"),
        )
        .with_path(path)
        .with_remediation("Pick a different key name."));
    }
    let prop = object.append(key, value);
    let new = prop.value().map(|n| display(&n));
    Ok(OpEffect {
        kind: ChangeKind::Added,
        pointer: path.to_string(),
        old: None,
        new,
        touched: vec![path.to_string()],
        reshaped: vec![parent_pointer],
        paths: vec![path.to_string()],
    })
}

fn remove(root: &CstRootNode, path: &str) -> CoreResult<OpEffect> {
    let segments = parse_pointer(path)?;
    if segments.is_empty() {
        return Err(invalid(path, "The document root cannot be removed"));
    }
    let target = resolve(root, &segments).ok_or_else(|| missing(path))?;
    let old = display(&target.node);
    match target.prop {
        Some(prop) => prop.remove(),
        None => target.node.remove(),
    }
    Ok(OpEffect {
        kind: ChangeKind::Removed,
        pointer: path.to_string(),
        old: Some(old),
        new: None,
        touched: vec![path.to_string()],
        reshaped: vec![to_pointer(&segments[..segments.len() - 1])],
        paths: vec![path.to_string()],
    })
}

fn array_at(root: &CstRootNode, path: &str) -> CoreResult<jsonc_parser::cst::CstArray> {
    let segments = parse_pointer(path)?;
    match resolve(root, &segments).ok_or_else(|| missing(path))?.node {
        CstNode::Container(CstContainerNode::Array(array)) => Ok(array),
        _ => Err(invalid(path, "The path is not an array")),
    }
}

fn array_insert(
    root: &CstRootNode,
    path: &str,
    index: usize,
    value_json: &str,
) -> CoreResult<OpEffect> {
    let value = parse_value(value_json)?;
    let array = array_at(root, path)?;
    let len = array.elements().len();
    if index > len {
        return Err(invalid(
            path,
            format!("Index {index} is out of range for an array of {len} items"),
        ));
    }
    let node = if index == len {
        array.append(value)
    } else {
        array.insert(index, value)
    };
    let item = child_pointer(path, &index.to_string());
    Ok(OpEffect {
        kind: ChangeKind::Added,
        pointer: item.clone(),
        old: None,
        new: Some(display(&node)),
        touched: vec![item],
        reshaped: vec![path.to_string()],
        paths: vec![path.to_string()],
    })
}

fn array_move(root: &CstRootNode, path: &str, from: usize, to: usize) -> CoreResult<OpEffect> {
    let array = array_at(root, path)?;
    let elements = array.elements();
    let len = elements.len();
    if from >= len || to >= len {
        return Err(invalid(
            path,
            format!("Move {from} -> {to} is out of range for an array of {len} items"),
        ));
    }
    // Final order, as indices into the current order: take `from` out, put it back at `to`.
    let mut order: Vec<usize> = (0..len).collect();
    let moved = order.remove(from);
    order.insert(to, moved);
    let rank_of_child: HashMap<usize, usize> = order
        .iter()
        .enumerate()
        .map(|(rank, original)| (elements[*original].child_index(), rank))
        .collect();
    if from != to {
        // The CST sort moves comments and blank lines with their element.
        array
            .sort_elements()
            .by_key(|element| rank_of_child[&element.child_index()]);
    }
    let item = child_pointer(path, &to.to_string());
    Ok(OpEffect {
        kind: ChangeKind::Moved,
        pointer: item.clone(),
        old: Some(format!("position {}", from + 1)),
        new: Some(format!("position {}", to + 1)),
        touched: vec![child_pointer(path, &from.to_string()), item],
        reshaped: vec![path.to_string()],
        paths: vec![path.to_string()],
    })
}

fn rename_key(root: &CstRootNode, path: &str, new_key: &str) -> CoreResult<OpEffect> {
    let segments = parse_pointer(path)?;
    let Some((old_key, parent_segments)) = segments.split_last() else {
        return Err(invalid(path, "The document root has no key"));
    };
    let target = resolve(root, &segments).ok_or_else(|| missing(path))?;
    let prop = target
        .prop
        .ok_or_else(|| invalid(path, "Only object members can be renamed"))?;
    let parent_pointer = to_pointer(parent_segments);
    let new_pointer = child_pointer(&parent_pointer, new_key);
    let mk_effect = || OpEffect {
        kind: ChangeKind::Renamed,
        pointer: new_pointer.clone(),
        old: Some(old_key.clone()),
        new: Some(new_key.to_string()),
        touched: vec![path.to_string(), new_pointer.clone()],
        reshaped: vec![parent_pointer.clone()],
        paths: vec![path.to_string(), new_pointer.clone()],
    };
    if new_key == old_key {
        return Ok(mk_effect());
    }
    let parent = prop
        .parent()
        .and_then(|p| p.as_object())
        .ok_or_else(|| invalid(path, "The member has no parent object"))?;
    if parent.get(new_key).is_some() {
        return Err(CoreError::new(
            ErrorCode::DuplicateKeys,
            format!("The key `{new_key}` already exists"),
        )
        .with_path(new_pointer)
        .with_remediation("Pick a key name that is not used in this object."));
    }
    match prop.name() {
        Some(ObjectPropName::String(literal)) => {
            literal.replace_with(CstInputValue::String(new_key.to_string()));
        }
        _ => {
            return Err(invalid(
                path,
                "The key is not a quoted string and cannot be renamed",
            ));
        }
    }
    Ok(mk_effect())
}
