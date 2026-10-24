//! Structural diff of two value trees (original vs disk, eng C4).

use super::value::Val as V;

use crate::core::patch::escape_segment;
use crate::core::types::{ChangeKind, Pointer};

use super::cst::{plural, truncate};

/// One changed pointer between two documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueChange {
    pub pointer: Pointer,
    pub kind: ChangeKind,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Display text matching `cst::display`.
pub fn display_value(v: &V) -> String {
    match v {
        V::Null => "null".to_string(),
        V::Bool(b) => b.to_string(),
        V::Number(n) => n.clone(),
        V::String(s) => truncate(s),
        V::Array(items) => format!("[{} {}]", items.len(), plural(items.len(), "item")),
        V::Object(members) => format!("{{{} {}}}", members.len(), plural(members.len(), "key")),
    }
}

/// Exact equality: numbers compare by lexeme, objects by member set (order-insensitive).
pub fn equal(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Null, V::Null) => true,
        (V::Bool(x), V::Bool(y)) => x == y,
        (V::Number(x), V::Number(y)) | (V::String(x), V::String(y)) => x == y,
        (V::Array(x), V::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equal(p, q))
        }
        (V::Object(x), V::Object(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| k2 == k)
                        .is_some_and(|(_, w)| equal(v, w))
                })
        }
        _ => false,
    }
}

/// Changed pointers from `old` to `new`. Objects recurse per key. Arrays are
/// compared positionally and any difference marks the array's own pointer
/// (conservative, matches R16's shared-parent-array rule).
pub fn diff_values(old: &V, new: &V) -> Vec<ValueChange> {
    let mut out = Vec::new();
    walk("", old, new, &mut out);
    out
}

fn walk(pointer: &str, old: &V, new: &V, out: &mut Vec<ValueChange>) {
    match (old, new) {
        (V::Object(a), V::Object(b)) => {
            for (key, old_child) in a {
                let child = format!("{pointer}/{}", escape_segment(key));
                match b.iter().find(|(k, _)| k == key) {
                    Some((_, new_child)) => walk(&child, old_child, new_child, out),
                    None => out.push(ValueChange {
                        pointer: child,
                        kind: ChangeKind::Removed,
                        old: Some(display_value(old_child)),
                        new: None,
                    }),
                }
            }
            for (key, new_child) in b.iter().filter(|(k, _)| !a.iter().any(|(k2, _)| k2 == k)) {
                out.push(ValueChange {
                    pointer: format!("{pointer}/{}", escape_segment(key)),
                    kind: ChangeKind::Added,
                    old: None,
                    new: Some(display_value(new_child)),
                });
            }
        }
        _ if equal(old, new) => {}
        _ => out.push(ValueChange {
            pointer: pointer.to_string(),
            kind: ChangeKind::Changed,
            old: Some(display_value(old)),
            new: Some(display_value(new)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonc_parser::cst::CstRootNode;

    use crate::core::document::cst::node_to_value;
    use crate::core::document::scan::parse_options;
    use crate::core::types::DocKind;

    fn v(text: &str) -> V {
        let root = CstRootNode::parse(text, &parse_options(DocKind::Json)).unwrap();
        node_to_value(&root.value().unwrap()).into()
    }

    fn changes(a: &str, b: &str) -> Vec<(String, ChangeKind)> {
        diff_values(&v(a), &v(b))
            .into_iter()
            .map(|c| (c.pointer, c.kind))
            .collect()
    }

    #[test]
    fn object_member_changes() {
        let got = changes(
            r#"{"a":1,"b":{"c":true,"d":2},"e":3}"#,
            r#"{"a":1,"b":{"c":false,"x":0},"f":4}"#,
        );
        assert_eq!(
            got,
            [
                ("/b/c".into(), ChangeKind::Changed),
                ("/b/d".into(), ChangeKind::Removed),
                ("/b/x".into(), ChangeKind::Added),
                ("/e".into(), ChangeKind::Removed),
                ("/f".into(), ChangeKind::Added),
            ]
        );
    }

    #[test]
    fn array_change_marks_the_array_itself() {
        assert_eq!(
            changes(r#"{"l":[1,2,3],"k":1}"#, r#"{"l":[1,9,3],"k":1}"#),
            [("/l".into(), ChangeKind::Changed)]
        );
        assert_eq!(
            changes(r#"{"l":[{"a":1}]}"#, r#"{"l":[{"a":1},2]}"#),
            [("/l".into(), ChangeKind::Changed)]
        );
        assert!(changes(r#"{"l":[1,2]}"#, r#"{"l":[1,2]}"#).is_empty());
    }

    #[test]
    fn lexeme_and_type_changes_count() {
        assert_eq!(
            changes(r#"{"n":1.50}"#, r#"{"n":1.5}"#),
            [("/n".into(), ChangeKind::Changed)]
        );
        assert_eq!(
            changes(r#"{"a":{"b":1}}"#, r#"{"a":"s"}"#),
            [("/a".into(), ChangeKind::Changed)]
        );
        assert_eq!(changes("1", "2"), [("".into(), ChangeKind::Changed)]);
        // Key order alone is not a change.
        assert!(changes(r#"{"a":1,"b":2}"#, r#"{"b":2,"a":1}"#).is_empty());
    }
}
