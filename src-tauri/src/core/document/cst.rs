//! Reading the CST: node kinds, pointer resolution, the UI tree DTO, and a
//! neutral value tree ([`CstInputValue`]) that keeps number lexemes.

use jsonc_parser::cst::{
    CstContainerNode, CstInputValue, CstLeafNode, CstNode, CstObjectProp, CstRootNode,
};

use crate::core::patch::escape_segment;
use crate::core::types::{NodeKind, Pointer, TreeNode};

/// Kind of a value node; `None` for trivia, tokens and property containers.
pub fn kind_of(node: &CstNode) -> Option<NodeKind> {
    match node {
        CstNode::Container(CstContainerNode::Object(_)) => Some(NodeKind::Object),
        CstNode::Container(CstContainerNode::Array(_)) => Some(NodeKind::Array),
        CstNode::Container(_) => None,
        CstNode::Leaf(CstLeafNode::StringLit(_) | CstLeafNode::WordLit(_)) => {
            Some(NodeKind::String)
        }
        CstNode::Leaf(CstLeafNode::NumberLit(_)) => Some(NodeKind::Number),
        CstNode::Leaf(CstLeafNode::BooleanLit(_)) => Some(NodeKind::Boolean),
        CstNode::Leaf(CstLeafNode::NullKeyword(_)) => Some(NodeKind::Null),
        CstNode::Leaf(_) => None,
    }
}

/// A value node plus the member it belongs to (when it is an object value).
pub struct Target {
    pub node: CstNode,
    pub prop: Option<CstObjectProp>,
}

/// Array index from a pointer segment (RFC 6901: digits, no leading zeros).
pub fn parse_index(segment: &str) -> Option<usize> {
    let canonical = !segment.is_empty()
        && segment.bytes().all(|b| b.is_ascii_digit())
        && (segment == "0" || !segment.starts_with('0'));
    if canonical {
        segment.parse().ok()
    } else {
        None
    }
}

/// Find the value at `segments`.
pub fn resolve(root: &CstRootNode, segments: &[String]) -> Option<Target> {
    let mut target = Target {
        node: root.value()?,
        prop: None,
    };
    for segment in segments {
        target = match &target.node {
            CstNode::Container(CstContainerNode::Object(object)) => {
                let prop = object.get(segment)?;
                Target {
                    node: prop.value()?,
                    prop: Some(prop),
                }
            }
            CstNode::Container(CstContainerNode::Array(array)) => {
                let node = array.elements().into_iter().nth(parse_index(segment)?)?;
                Target { node, prop: None }
            }
            _ => return None,
        };
    }
    Some(target)
}

/// Scalar text: decoded string, exact number lexeme, or `true`/`false`/`null`.
fn scalar_text(node: &CstNode) -> Option<String> {
    match node {
        CstNode::Leaf(CstLeafNode::StringLit(s)) => {
            Some(s.decoded_value().unwrap_or_else(|_| s.raw_value()))
        }
        CstNode::Leaf(
            CstLeafNode::WordLit(_) | CstLeafNode::NumberLit(_) | CstLeafNode::BooleanLit(_),
        ) => Some(node.to_string()),
        CstNode::Leaf(CstLeafNode::NullKeyword(_)) => Some("null".to_string()),
        _ => None,
    }
}

fn child_count(node: &CstNode) -> Option<usize> {
    match node {
        CstNode::Container(CstContainerNode::Object(o)) => Some(o.properties().len()),
        CstNode::Container(CstContainerNode::Array(a)) => Some(a.elements().len()),
        _ => None,
    }
}

/// Short display text for change lists: scalars as text (truncated),
/// containers as `{n keys}` / `[n items]`.
pub fn display(node: &CstNode) -> String {
    match (kind_of(node), child_count(node)) {
        (Some(NodeKind::Object), Some(n)) => format!("{{{n} {}}}", plural(n, "key")),
        (Some(NodeKind::Array), Some(n)) => format!("[{n} {}]", plural(n, "item")),
        _ => truncate(&scalar_text(node).unwrap_or_default()),
    }
}

pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

pub fn truncate(text: &str) -> String {
    const MAX: usize = 200;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(MAX).collect();
    cut.push('…');
    cut
}

/// Two pointers are related when one is the other or an ancestor of it.
pub fn related(a: &str, b: &str) -> bool {
    fn ancestor_or_self(anc: &str, desc: &str) -> bool {
        anc.is_empty()
            || desc == anc
            || desc
                .strip_prefix(anc)
                .is_some_and(|rest| rest.starts_with('/'))
    }
    ancestor_or_self(a, b) || ancestor_or_self(b, a)
}

/// Build the UI tree for `node`, loading `depth` levels of children.
/// `touched` are pointers edited in the draft; a node is `changed` when it is
/// related (ancestor, self or descendant) to one of them.
pub fn build_tree(
    node: &CstNode,
    pointer: &str,
    key: Option<String>,
    index: Option<u32>,
    depth: u32,
    touched: &[Pointer],
) -> TreeNode {
    let kind = kind_of(node).unwrap_or(NodeKind::Null);
    let count = child_count(node);
    let children = (depth > 0 && count.is_some()).then(|| {
        let entries: Vec<(Option<String>, Option<u32>, CstNode)> = match node {
            CstNode::Container(CstContainerNode::Object(object)) => object
                .properties()
                .into_iter()
                .filter_map(|p| Some((Some(p.decoded_name()?), None, p.value()?)))
                .collect(),
            CstNode::Container(CstContainerNode::Array(array)) => array
                .elements()
                .into_iter()
                .enumerate()
                .map(|(i, n)| (None, Some(i as u32), n))
                .collect(),
            _ => Vec::new(),
        };
        entries
            .into_iter()
            .map(|(k, i, child)| {
                let segment = k
                    .as_deref()
                    .map_or_else(|| i.unwrap_or(0).to_string(), escape_segment);
                build_tree(
                    &child,
                    &format!("{pointer}/{segment}"),
                    k,
                    i,
                    depth - 1,
                    touched,
                )
            })
            .collect()
    });
    TreeNode {
        pointer: pointer.to_string(),
        key,
        index,
        kind,
        text: scalar_text(node),
        child_count: count.map(|n| n as u32),
        children,
        changed: touched.iter().any(|t| related(pointer, t)),
    }
}

/// Convert a node into the neutral value tree (strings decoded, numbers keep
/// their lexeme). Trivia and comments are dropped.
pub fn node_to_value(node: &CstNode) -> CstInputValue {
    match node {
        CstNode::Container(CstContainerNode::Object(object)) => CstInputValue::Object(
            object
                .properties()
                .into_iter()
                .filter_map(|p| Some((p.decoded_name()?, node_to_value(&p.value()?))))
                .collect(),
        ),
        CstNode::Container(CstContainerNode::Array(array)) => {
            CstInputValue::Array(array.elements().iter().map(node_to_value).collect())
        }
        CstNode::Leaf(CstLeafNode::NumberLit(n)) => CstInputValue::Number(n.to_string()),
        CstNode::Leaf(CstLeafNode::BooleanLit(b)) => CstInputValue::Bool(b.value()),
        CstNode::Leaf(CstLeafNode::NullKeyword(_)) => CstInputValue::Null,
        other => CstInputValue::String(scalar_text(other).unwrap_or_default()),
    }
}

/// Resolve `segments` inside a neutral value tree.
pub fn lookup<'a>(value: &'a CstInputValue, segments: &[String]) -> Option<&'a CstInputValue> {
    let mut cur = value;
    for segment in segments {
        cur = match cur {
            CstInputValue::Object(members) => &members.iter().find(|(k, _)| k == segment)?.1,
            CstInputValue::Array(items) => items.get(parse_index(segment)?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// `serde_json` view of a value tree, for schema validation. Numbers beyond
/// `f64` precision lose digits here only; the document text keeps its lexeme.
pub fn to_json(value: &CstInputValue) -> serde_json::Value {
    use serde_json::Value;
    match value {
        CstInputValue::Null => Value::Null,
        CstInputValue::Bool(b) => Value::Bool(*b),
        CstInputValue::Number(n) => n
            .parse()
            .map_or_else(|_| Value::String(n.clone()), Value::Number),
        CstInputValue::String(s) => Value::String(s.clone()),
        CstInputValue::Array(items) => Value::Array(items.iter().map(to_json).collect()),
        CstInputValue::Object(members) => Value::Object(
            members
                .iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect(),
        ),
    }
}
