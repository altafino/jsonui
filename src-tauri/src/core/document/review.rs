//! Text-diff half of the Review dialog: hunks with secret values masked.

use jsonc_parser::cst::{CstContainerNode, CstLeafNode, CstNode, CstRootNode};

use crate::core::patch::escape_segment;
use crate::core::security::redact::Masker;
use crate::core::types::{DiffHunk, DiffLine, MaskSpan};

use super::lines::diff_hunks;
use super::scan::parse_options;
use super::{BOM, DocumentSession, SecretPredicate};

impl DocumentSession {
    /// Hunks between the original and the candidate text. Values of
    /// secret-like members (in either version) are replaced by `•` runs whose
    /// ids `reveal_masked` resolves.
    pub(super) fn review_hunks(&self, secret: SecretPredicate) -> Vec<DiffHunk> {
        let (Some(root), Ok(old)) = (&self.root, self.original_text()) else {
            return Vec::new();
        };
        if self.ops.is_empty() {
            return Vec::new();
        }
        let candidate = self.candidate_bytes();
        let candidate = candidate.strip_prefix(&BOM[..]).unwrap_or(&candidate);
        let Ok(new) = std::str::from_utf8(candidate) else {
            return Vec::new();
        };

        let mut lexemes = Vec::new();
        collect_secret_lexemes(root, secret, &mut lexemes);
        if let Ok(original_root) = CstRootNode::parse(old, &parse_options(self.kind)) {
            collect_secret_lexemes(&original_root, secret, &mut lexemes);
        }
        // Longest first so a secret that contains another is masked whole.
        lexemes.sort_by_key(|l| std::cmp::Reverse(l.len()));
        lexemes.dedup();

        let mut registry = self.masks.borrow_mut();
        registry.truncate(self.source_mask_count);
        diff_hunks(old, new)
            .into_iter()
            .map(|hunk| DiffHunk {
                lines: hunk
                    .into_iter()
                    .map(|line| {
                        let (text, masks) = mask_line(&line.text, &lexemes, &mut registry);
                        DiffLine {
                            kind: line.kind,
                            old_line: line.old_line,
                            new_line: line.new_line,
                            text,
                            masks,
                        }
                    })
                    .collect(),
            })
            .collect()
    }
}

/// Raw lexemes (strings with their quotes, numbers) of every scalar at or
/// under a secret-like pointer.
fn collect_secret_lexemes(root: &CstRootNode, secret: SecretPredicate, out: &mut Vec<String>) {
    if let Some(value) = root.value() {
        walk(&value, "", false, secret, out);
    }
}

fn walk(
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
                    walk(
                        &value,
                        &format!("{pointer}/{}", escape_segment(&name)),
                        here,
                        secret,
                        out,
                    );
                }
            }
        }
        CstNode::Container(CstContainerNode::Array(array)) => {
            for (i, item) in array.elements().iter().enumerate() {
                walk(item, &format!("{pointer}/{i}"), here, secret, out);
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

/// Replace every occurrence of a secret lexeme in `text` (inside the quotes
/// for strings) with `•`; returns the masked text and spans (UTF-16 offsets).
fn mask_line(
    text: &str,
    lexemes: &[String],
    registry: &mut Vec<String>,
) -> (String, Vec<MaskSpan>) {
    let mut masker = Masker::new(registry.len() as u32);
    let mut cursor = 0;
    while let Some((start, lexeme)) = next_match(text, cursor, lexemes) {
        let (from, to) = if lexeme.starts_with('"') {
            (start + 1, start + lexeme.len() - 1)
        } else {
            (start, start + lexeme.len())
        };
        masker.push_plain(&text[cursor..from]);
        masker.push_masked(&text[from..to]);
        cursor = to;
    }
    masker.push_plain(&text[cursor..]);
    let (masked, spans, originals) = masker.finish();
    registry.extend(originals);
    (masked, spans)
}

/// Leftmost match at or after `from`; unquoted lexemes must stand alone
/// (not inside a longer number or word).
fn next_match<'a>(text: &str, from: usize, lexemes: &'a [String]) -> Option<(usize, &'a String)> {
    let mut best: Option<(usize, &String)> = None;
    for lexeme in lexemes {
        let mut at = from;
        while let Some(found) = text[at..].find(lexeme.as_str()) {
            let start = at + found;
            let end = start + lexeme.len();
            let word = |c: Option<char>| {
                c.is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
            };
            let standalone = lexeme.starts_with('"')
                || (!word(text[..start].chars().next_back()) && !word(text[end..].chars().next()));
            if standalone {
                if best.is_none_or(|(b, _)| start < b) {
                    best = Some((start, lexeme));
                }
                break;
            }
            at = start + 1;
            while !text.is_char_boundary(at) {
                at += 1;
            }
        }
    }
    best
}
