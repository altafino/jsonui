//! Rendering new values as YAML text.

use super::scalar::{Style, emit_scalar, emit_string};
use crate::core::document::value::Val;

/// How nested block collections are indented in this document.
#[derive(Clone, Copy, Debug)]
pub struct Fmt {
    /// Spaces per nesting level.
    pub step: usize,
    /// Block sequences sit at their parent key's column (`k:\n- a`).
    pub indentless: bool,
}

/// A mapping key, keeping the style of the key it replaces.
pub fn key(k: &str, keep: Option<Style>, flow: bool) -> String {
    emit_string(k, keep, flow)
}

/// Scalars and empty collections as one line; in flow context also every
/// collection. `None` for a non-empty collection in block context.
pub fn inline(v: &Val, flow: bool) -> Option<String> {
    match v {
        Val::Array(a) if a.is_empty() => Some("[]".into()),
        Val::Object(m) if m.is_empty() => Some("{}".into()),
        Val::Array(a) if flow => Some(format!(
            "[{}]",
            a.iter()
                .filter_map(|i| inline(i, true))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Val::Object(m) if flow => Some(format!(
            "{{{}}}",
            m.iter()
                .filter_map(|(k, v)| Some(format!("{}: {}", key(k, None, true), inline(v, true)?)))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Val::Array(_) | Val::Object(_) => None,
        scalar => Some(emit_scalar(scalar, None, flow)),
    }
}

/// A non-empty collection as block lines at indentation 0.
pub fn block_lines(v: &Val, fmt: Fmt) -> Vec<String> {
    let mut lines = Vec::new();
    match v {
        Val::Object(members) => {
            for (k, val) in members {
                let k = key(k, None, false);
                if let Some(s) = inline(val, false) {
                    lines.push(format!("{k}: {s}"));
                    continue;
                }
                lines.push(format!("{k}:"));
                let pad = if matches!(val, Val::Array(_)) && fmt.indentless {
                    0
                } else {
                    fmt.step
                };
                for l in block_lines(val, fmt) {
                    lines.push(format!("{}{l}", " ".repeat(pad)));
                }
            }
        }
        Val::Array(items) => {
            for item in items {
                if let Some(s) = inline(item, false) {
                    lines.push(format!("- {s}"));
                    continue;
                }
                for (i, l) in block_lines(item, fmt).into_iter().enumerate() {
                    lines.push(format!("{}{l}", if i == 0 { "- " } else { "  " }));
                }
            }
        }
        _ => {}
    }
    lines
}

/// Indentation of `n` spaces.
pub fn pad(n: usize) -> String {
    " ".repeat(n)
}
