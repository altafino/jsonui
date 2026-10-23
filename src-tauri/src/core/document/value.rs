//! Format-neutral value tree shared by every syntax (JSON/JSONC, YAML, TOML):
//! strings decoded, numbers kept as their source lexeme, member order kept.
//! The UI tree, change lists, conflict diffs and schema validation all derive
//! from it, so every format behaves the same.

use crate::core::patch::escape_segment;
use crate::core::types::{NodeKind, Pointer, TreeNode};

use super::cst::{parse_index, plural, related, truncate};

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    /// Source lexeme, e.g. `1.50` or `0x1F`.
    Number(String),
    String(String),
    Array(Vec<Val>),
    Object(Vec<(String, Val)>),
}

impl Val {
    pub fn kind(&self) -> NodeKind {
        match self {
            Val::Null => NodeKind::Null,
            Val::Bool(_) => NodeKind::Boolean,
            Val::Number(_) => NodeKind::Number,
            Val::String(_) => NodeKind::String,
            Val::Array(_) => NodeKind::Array,
            Val::Object(_) => NodeKind::Object,
        }
    }

    /// Scalar text as `TreeNode::text` shows it; `None` for containers.
    pub fn text(&self) -> Option<String> {
        match self {
            Val::Null => Some("null".into()),
            Val::Bool(b) => Some(b.to_string()),
            Val::Number(n) | Val::String(n) => Some(n.clone()),
            Val::Array(_) | Val::Object(_) => None,
        }
    }

    pub fn child_count(&self) -> Option<usize> {
        match self {
            Val::Array(items) => Some(items.len()),
            Val::Object(members) => Some(members.len()),
            _ => None,
        }
    }

    /// Short display text for change lists: scalars as text (truncated),
    /// containers as `{n keys}` / `[n items]`.
    pub fn display(&self) -> String {
        match self {
            Val::Array(items) => format!("[{} {}]", items.len(), plural(items.len(), "item")),
            Val::Object(members) => {
                format!("{{{} {}}}", members.len(), plural(members.len(), "key"))
            }
            scalar => truncate(&scalar.text().unwrap_or_default()),
        }
    }

    /// The value at `segments`.
    pub fn lookup(&self, segments: &[String]) -> Option<&Val> {
        let mut cur = self;
        for segment in segments {
            cur = match cur {
                Val::Object(members) => &members.iter().find(|(k, _)| k == segment)?.1,
                Val::Array(items) => items.get(parse_index(segment)?)?,
                _ => return None,
            };
        }
        Some(cur)
    }

    /// `serde_json` view for schema validation. Numbers beyond `f64`
    /// precision lose digits here only; the document text keeps its lexeme.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value;
        match self {
            Val::Null => Value::Null,
            Val::Bool(b) => Value::Bool(*b),
            Val::Number(n) => n
                .parse()
                .map_or_else(|_| Value::String(n.clone()), Value::Number),
            Val::String(s) => Value::String(s.clone()),
            Val::Array(items) => Value::Array(items.iter().map(Val::to_json).collect()),
            Val::Object(members) => Value::Object(
                members
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect(),
            ),
        }
    }
}

impl From<jsonc_parser::cst::CstInputValue> for Val {
    fn from(v: jsonc_parser::cst::CstInputValue) -> Self {
        use jsonc_parser::cst::CstInputValue as V;
        match v {
            V::Null => Val::Null,
            V::Bool(b) => Val::Bool(b),
            V::Number(n) => Val::Number(n),
            V::String(s) => Val::String(s),
            V::Array(items) => Val::Array(items.into_iter().map(Val::from).collect()),
            V::Object(members) => {
                Val::Object(members.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

/// UI tree for `value` at `pointer`, loading `depth` levels of children.
/// A node is `changed` when it is related to one of the `touched` pointers.
pub fn tree(
    value: &Val,
    pointer: &str,
    key: Option<String>,
    index: Option<u32>,
    depth: u32,
    touched: &[Pointer],
) -> TreeNode {
    let children = (depth > 0)
        .then(|| match value {
            Val::Object(members) => Some(
                members
                    .iter()
                    .map(|(k, v)| {
                        let p = format!("{pointer}/{}", escape_segment(k));
                        tree(v, &p, Some(k.clone()), None, depth - 1, touched)
                    })
                    .collect(),
            ),
            Val::Array(items) => Some(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        tree(
                            v,
                            &format!("{pointer}/{i}"),
                            None,
                            Some(i as u32),
                            depth - 1,
                            touched,
                        )
                    })
                    .collect(),
            ),
            _ => None,
        })
        .flatten();
    TreeNode {
        pointer: pointer.to_string(),
        key,
        index,
        kind: value.kind(),
        text: value.text(),
        child_count: value.child_count().map(|n| n as u32),
        children,
        changed: touched.iter().any(|t| related(pointer, t)),
    }
}
