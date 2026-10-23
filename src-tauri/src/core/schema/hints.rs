//! Schema → `FieldHint` resolution (private to the schema module).
//!
//! Walks the schema along an instance pointer through `properties`,
//! `additionalProperties`, `items`/`prefixItems`, local `$ref`, `allOf` and,
//! best effort, `oneOf`/`anyOf` (all branches are merged; first value wins).

use serde_json::Value;

use super::decode_pointer;
use crate::core::types::FieldHint;

const MAX_DEPTH: usize = 32;

/// Collect `schema` and everything it pulls in (local `$ref`, combinators).
/// The schema itself comes first so its own title/description win.
fn expand<'a>(root: &'a Value, schema: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
    if depth > MAX_DEPTH || !schema.is_object() {
        return;
    }
    out.push(schema);
    let target = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|p| root.pointer(p));
    if let Some(t) = target {
        expand(root, t, out, depth + 1);
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        for sub in schema
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            expand(root, sub, out, depth + 1);
        }
    }
}

/// Subschemas that describe the child `seg` of an instance described by `schema`.
fn step<'a>(schema: &'a Value, seg: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    if let Some(p) = schema.get("properties").and_then(|p| p.get(seg)) {
        out.push(p);
    } else if let Some(ap) = schema.get("additionalProperties").filter(|v| v.is_object()) {
        out.push(ap);
    }
    if let Ok(idx) = seg.parse::<usize>() {
        if let Some(p) = schema.get("prefixItems").and_then(|v| v.get(idx)) {
            out.push(p);
        } else {
            match schema.get("items") {
                Some(i) if i.is_object() => out.push(i),
                Some(Value::Array(tuple)) => {
                    out.extend(
                        tuple
                            .get(idx)
                            .or_else(|| schema.get("additionalItems").filter(|v| v.is_object())),
                    );
                }
                _ => {}
            }
        }
    }
    out
}

fn child<'a>(node: &'a Value, seg: &str) -> Option<&'a Value> {
    match node {
        Value::Object(m) => m.get(seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    }
}

fn first<'a>(cands: &[&'a Value], key: &str) -> Option<&'a Value> {
    cands.iter().find_map(|s| s.get(key))
}

fn first_u32(cands: &[&Value], key: &str) -> Option<u32> {
    first(cands, key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

fn first_text(cands: &[&Value], key: &str) -> Option<String> {
    first(cands, key).and_then(Value::as_str).map(str::to_owned)
}

pub(super) fn hint_for(root: &Value, instance: &Value, pointer: &str) -> FieldHint {
    let mut cands = Vec::new();
    expand(root, root, &mut cands, 0);
    let mut node = Some(instance);
    for seg in decode_pointer(pointer) {
        let next: Vec<&Value> = cands.iter().flat_map(|s| step(s, &seg)).collect();
        cands = Vec::new();
        for s in next {
            expand(root, s, &mut cands, 0);
        }
        node = node.and_then(|n| child(n, &seg));
    }

    let enum_values = first(&cands, "enum")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(Value::to_string).collect())
        .or_else(|| first(&cands, "const").map(|c| vec![c.to_string()]));

    let mut addable_properties = Vec::new();
    if node.is_none_or(Value::is_object) {
        for name in cands
            .iter()
            .filter_map(|s| s.get("properties")?.as_object())
            .flat_map(|p| p.keys())
        {
            let present = node
                .and_then(Value::as_object)
                .is_some_and(|o| o.contains_key(name));
            if !present && !addable_properties.contains(name) {
                addable_properties.push(name.clone());
            }
        }
    }

    let has_fixed = cands.iter().any(|s| {
        s.get("properties")
            .and_then(Value::as_object)
            .is_some_and(|p| !p.is_empty())
    });
    let ap_open = cands.iter().any(|s| {
        matches!(
            s.get("additionalProperties"),
            Some(Value::Bool(true)) | Some(Value::Object(_))
        )
    });
    let ap_closed = cands
        .iter()
        .any(|s| s.get("additionalProperties") == Some(&Value::Bool(false)));

    FieldHint {
        pointer: pointer.to_owned(),
        title: first_text(&cands, "title"),
        description: first_text(&cands, "description"),
        enum_values,
        default_json: first(&cands, "default").map(Value::to_string),
        format: first_text(&cands, "format"),
        minimum: first(&cands, "minimum").map(Value::to_string),
        maximum: first(&cands, "maximum").map(Value::to_string),
        min_length: first_u32(&cands, "minLength"),
        max_length: first_u32(&cands, "maxLength"),
        pattern: first_text(&cands, "pattern"),
        secret: cands.iter().any(|s| {
            s.get("format").and_then(Value::as_str) == Some("password")
                || s.get("writeOnly") == Some(&Value::Bool(true))
        }),
        addable_properties,
        open_map: ap_open && !ap_closed && !has_fixed,
        item_summary: None,
        sensitive: false,
    }
}
