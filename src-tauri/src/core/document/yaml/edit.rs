//! The six primitive edits as text splices computed from node positions.

use super::model::{Item, Kind, Node, Parsed};
use super::render::{Fmt, block_lines, inline, key, pad};
use super::scalar::{Style, emit_scalar, literal_block};
use crate::core::document::cst::parse_index;
use crate::core::document::value::Val;

/// Why an op is refused on this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The node is (or sits in) an alias or an anchored node others use.
    Shared,
    Tagged,
    /// The layout is one this editor cannot change faithfully.
    Unsafe,
}

pub struct Splice {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

fn splice(start: usize, end: usize, text: impl Into<String>) -> Splice {
    Splice {
        start,
        end,
        text: text.into(),
    }
}

pub fn apply(text: &str, mut splices: Vec<Splice>) -> String {
    // Back to front; at one position a deletion goes before an insertion.
    splices.sort_by_key(|s| std::cmp::Reverse((s.start, s.end)));
    let mut out = text.to_string();
    for s in splices {
        out.replace_range(s.start..s.end, &s.text);
    }
    out
}

/// The node at a path, and how it hangs off its parent.
pub struct Hit<'a> {
    /// Root to target.
    pub chain: Vec<&'a Node>,
    pub link: Link<'a>,
}

#[derive(Clone, Copy)]
pub enum Link<'a> {
    Root,
    Entry(&'a Node, usize),
    Item(&'a Node, usize),
}

impl Hit<'_> {
    pub fn target(&self) -> &Node {
        self.chain[self.chain.len() - 1]
    }
}

#[derive(Clone, Copy)]
pub enum Act {
    Replace,
    Remove,
    Rename,
    /// Add/insert into the target container.
    Container,
    /// Move among items `a..=b` of the target sequence.
    Move(usize, usize),
}

fn is_flow(n: &Node) -> bool {
    matches!(
        n.kind,
        Kind::Map { flow: true, .. } | Kind::Seq { flow: true, .. }
    )
}

fn keep_style(old: &Node, new: &Val) -> Option<Style> {
    match (&old.kind, new) {
        (
            Kind::Scalar {
                style: style @ (Style::Plain | Style::Single | Style::Double),
                typed: Val::String(_),
                ..
            },
            Val::String(_),
        ) => Some(*style),
        _ => None,
    }
}

impl Parsed {
    pub fn walk(&self, path: &[String]) -> Result<Hit<'_>, Reason> {
        let mut chain = vec![&self.root];
        let mut link = Link::Root;
        for seg in path {
            let cur = chain[chain.len() - 1];
            match &cur.kind {
                Kind::Map { entries, .. } => {
                    let i = entries
                        .iter()
                        .position(|e| e.key == *seg)
                        .ok_or(Reason::Unsafe)?;
                    chain.push(&entries[i].value);
                    link = Link::Entry(cur, i);
                }
                Kind::Seq { items, .. } => {
                    let i = parse_index(seg)
                        .filter(|i| *i < items.len())
                        .ok_or(Reason::Unsafe)?;
                    chain.push(&items[i].value);
                    link = Link::Item(cur, i);
                }
                Kind::Alias => return Err(Reason::Shared),
                _ => return Err(Reason::Unsafe),
            }
        }
        if matches!(chain[chain.len() - 1].kind, Kind::Alias) {
            return Err(Reason::Shared);
        }
        Ok(Hit { chain, link })
    }

    pub fn guard(&self, hit: &Hit<'_>, act: Act) -> Result<(), Reason> {
        let n = hit.chain.len();
        let container = matches!(act, Act::Container | Act::Move(..));
        for (i, node) in hit.chain.iter().enumerate() {
            let above = i + 1 < n || container;
            if above && node.anchor.as_ref().is_some_and(|a| self.refs.contains(a)) {
                return Err(Reason::Shared);
            }
        }
        let t = hit.target();
        match act {
            Act::Replace | Act::Remove => {
                if t.anchor.is_some() || t.defines_anchor(&self.refs, false) {
                    return Err(Reason::Shared);
                }
                if t.tag.is_some() {
                    return Err(Reason::Tagged);
                }
            }
            Act::Rename => {
                if t.anchor.is_some() {
                    return Err(Reason::Shared);
                }
            }
            Act::Container => {
                if t.tag.is_some() {
                    return Err(Reason::Tagged);
                }
            }
            Act::Move(a, b) => {
                if t.tag.is_some() {
                    return Err(Reason::Tagged);
                }
                if let Kind::Seq { items, .. } = &t.kind
                    && items[a..=b]
                        .iter()
                        .any(|i| i.value.defines_anchor(&self.refs, true))
                {
                    return Err(Reason::Shared);
                }
            }
        }
        Ok(())
    }

    /// Start of item `k`'s whole-line chunk: the comment lines directly above
    /// it belong to it (except above the first item, which belong to the key).
    fn chunk_start(&self, items: &[Item], k: usize) -> usize {
        let mut cs = self.line_start(items[k].start);
        if k == 0 {
            return cs;
        }
        let floor = self.next_line(self.line_end(items[k - 1].end));
        while cs > floor {
            let prev = self.line_start(cs - 1);
            if self.text[prev..cs].trim().starts_with('#') {
                cs = prev;
            } else {
                break;
            }
        }
        cs
    }

    fn fmt(&self) -> Fmt {
        self.fmt
    }

    /// A scalar or empty/flow collection as one piece of text.
    fn inline_text(&self, old: &Node, val: &Val, flow: bool) -> Option<String> {
        if let (
            Kind::Scalar {
                style: Style::Literal | Style::Folded,
                block_indent,
                ..
            },
            Val::String(s),
        ) = (&old.kind, val)
        {
            let indent = if *block_indent > 0 {
                *block_indent
            } else {
                self.col(old.start) + self.fmt.step
            };
            let at_eof = self.line_end(old.end) == self.text.len();
            if !(at_eof && s.ends_with('\n'))
                && let Some(b) = literal_block(s, indent, self.eol)
            {
                return Some(b);
            }
        }
        match val {
            Val::Array(_) | Val::Object(_) => inline(val, flow),
            scalar => Some(emit_scalar(scalar, keep_style(old, val), flow)),
        }
    }

    fn append_root(&self, val: &Val) -> Splice {
        let mut text = String::new();
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            text.push_str(self.eol);
        }
        match inline(val, false) {
            Some(s) => text.push_str(&s),
            None => text.push_str(&block_lines(val, self.fmt()).join(self.eol)),
        }
        text.push_str(self.eol);
        splice(self.text.len(), self.text.len(), text)
    }

    /// Replace the value at `hit` (also the document root).
    pub fn replace(&self, hit: &Hit<'_>, val: &Val) -> Result<Vec<Splice>, Reason> {
        if self.empty {
            return Ok(vec![self.append_root(val)]);
        }
        let t = hit.target();
        let eol = self.eol;
        let flow_parent = match hit.link {
            Link::Root => false,
            Link::Entry(p, _) | Link::Item(p, _) => is_flow(p),
        };
        let inline_text = self.inline_text(t, val, flow_parent);
        let null = matches!(t.kind, Kind::Null);
        match hit.link {
            Link::Root => {
                let text = inline_text.unwrap_or_else(|| block_lines(val, self.fmt()).join(eol));
                Ok(vec![splice(t.start, t.end, text)])
            }
            Link::Entry(map, idx) => {
                let Kind::Map { entries, .. } = &map.kind else {
                    return Err(Reason::Unsafe);
                };
                let colon = entries[idx].colon_end.ok_or(Reason::Unsafe)?;
                if flow_parent {
                    let text = inline_text.ok_or(Reason::Unsafe)?;
                    return Ok(vec![if null {
                        splice(colon, colon, format!(" {text}"))
                    } else {
                        splice(t.start, t.end, text)
                    }]);
                }
                let base = t.props_end.unwrap_or(colon).max(colon);
                let same_line = null || !self.text[base..t.start].contains('\n');
                let nl = self.line_end(base);
                let key_col = self.col(entries[idx].key_start);
                match inline_text {
                    Some(s) if null => Ok(vec![splice(base, base, format!(" {s}"))]),
                    Some(s) if same_line => Ok(vec![splice(t.start, t.end, s)]),
                    Some(s) => Ok(vec![
                        splice(base, base, format!(" {s}")),
                        splice(nl, self.line_end(t.end), ""),
                    ]),
                    None => {
                        let child = key_col
                            + if matches!(val, Val::Array(_)) && self.fmt.indentless {
                                0
                            } else {
                                self.fmt.step
                            };
                        let text: String = block_lines(val, self.fmt())
                            .iter()
                            .map(|l| format!("{eol}{}{l}", pad(child)))
                            .collect();
                        // A comment on the key line stays on it.
                        Ok(if same_line {
                            let end = t.end.max(base);
                            vec![
                                splice(base, end, ""),
                                splice(self.line_end(end), self.line_end(end), text),
                            ]
                        } else {
                            vec![splice(nl, self.line_end(t.end), text)]
                        })
                    }
                }
            }
            Link::Item(seq, idx) => {
                let Kind::Seq { items, .. } = &seq.kind else {
                    return Err(Reason::Unsafe);
                };
                if flow_parent {
                    let text = inline_text.ok_or(Reason::Unsafe)?;
                    return Ok(vec![splice(t.start, t.end, text)]);
                }
                let dash_end = items[idx].start + 1;
                let base = t.props_end.unwrap_or(dash_end).max(dash_end);
                match inline_text {
                    Some(s) if null => Ok(vec![splice(base, base, format!(" {s}"))]),
                    Some(s) => Ok(vec![splice(t.start, t.end, s)]),
                    None => {
                        let lines = block_lines(val, self.fmt());
                        if null {
                            let cont = pad(self.col(items[idx].start) + 2);
                            Ok(vec![splice(
                                base,
                                base,
                                format!(" {}", lines.join(&format!("{eol}{cont}"))),
                            )])
                        } else {
                            let cont = pad(self.col(t.start));
                            Ok(vec![splice(
                                t.start,
                                t.end,
                                lines.join(&format!("{eol}{cont}")),
                            )])
                        }
                    }
                }
            }
        }
    }

    /// Append member `k` to the mapping at `hit`.
    pub fn add(&self, hit: &Hit<'_>, k: &str, val: &Val) -> Result<Vec<Splice>, Reason> {
        if self.empty {
            return Ok(vec![
                self.append_root(&Val::Object(vec![(k.to_string(), val.clone())])),
            ]);
        }
        let t = hit.target();
        let Kind::Map { flow, entries } = &t.kind else {
            return Err(Reason::Unsafe);
        };
        if *flow {
            let v = inline(val, true).ok_or(Reason::Unsafe)?;
            let entry = format!("{}: {v}", key(k, None, true));
            return Ok(vec![match entries.last() {
                None => splice(t.start + 1, t.start + 1, entry),
                Some(l) => splice(l.end, l.end, format!(", {entry}")),
            }]);
        }
        let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
            return Err(Reason::Unsafe);
        };
        let col = pad(self.col(first.start));
        let one = Val::Object(vec![(k.to_string(), val.clone())]);
        let text: String = block_lines(&one, self.fmt())
            .iter()
            .map(|l| format!("{}{col}{l}", self.eol))
            .collect();
        let at = self.line_end(last.end);
        Ok(vec![splice(at, at, text)])
    }

    /// Insert before item `index` of the sequence at `hit` (`len` appends).
    pub fn insert(&self, hit: &Hit<'_>, index: usize, val: &Val) -> Result<Vec<Splice>, Reason> {
        if self.empty {
            return Err(Reason::Unsafe);
        }
        let t = hit.target();
        let Kind::Seq { flow, items } = &t.kind else {
            return Err(Reason::Unsafe);
        };
        if *flow {
            let v = inline(val, true).ok_or(Reason::Unsafe)?;
            return Ok(vec![if items.is_empty() {
                splice(t.start + 1, t.start + 1, v)
            } else if index < items.len() {
                splice(items[index].start, items[index].start, format!("{v}, "))
            } else {
                let end = items[items.len() - 1].end;
                splice(end, end, format!(", {v}"))
            }]);
        }
        let first = items.first().ok_or(Reason::Unsafe)?;
        let col = self.col(first.start);
        let one = Val::Array(vec![val.clone()]);
        let lines = block_lines(&one, self.fmt());
        let eol = self.eol;
        if index == 0 {
            let at = items[0].start;
            let col = pad(self.col(at));
            Ok(vec![splice(
                at,
                at,
                format!("{}{eol}{col}", lines.join(&format!("{eol}{col}"))),
            )])
        } else if index < items.len() {
            // Before the comment lines that belong to the item it displaces.
            let at = self.chunk_start(items, index);
            let col = pad(self.col(items[index].start));
            let text: String = lines.iter().map(|l| format!("{col}{l}{eol}")).collect();
            Ok(vec![splice(at, at, text)])
        } else {
            let at = self.line_end(items[items.len() - 1].end);
            let text: String = lines
                .iter()
                .map(|l| format!("{eol}{}{l}", pad(col)))
                .collect();
            Ok(vec![splice(at, at, text)])
        }
    }

    /// Remove the child at `hit`, which hangs off a container with siblings.
    pub fn remove_child(&self, hit: &Hit<'_>) -> Result<Vec<Splice>, Reason> {
        let (container, idx) = match hit.link {
            Link::Entry(c, i) | Link::Item(c, i) => (c, i),
            Link::Root => return Err(Reason::Unsafe),
        };
        // (start, end) of every child.
        let spans: Vec<(usize, usize)> = match &container.kind {
            Kind::Map { entries, .. } => entries.iter().map(|e| (e.start, e.end)).collect(),
            Kind::Seq { items, .. } => items.iter().map(|i| (i.start, i.end)).collect(),
            _ => return Err(Reason::Unsafe),
        };
        let (start, end) = spans[idx];
        if is_flow(container) {
            return Ok(vec![if idx + 1 < spans.len() {
                splice(start, spans[idx + 1].0, "")
            } else {
                splice(spans[idx - 1].1, end, "")
            }]);
        }
        let ls = self.line_start(start);
        if self.text[ls..start].trim_matches([' ', '\t']).is_empty() {
            let mut a = ls;
            let b = self.next_line(end);
            if b == self.text.len() && !self.text.ends_with('\n') && a > 0 {
                a -= if self.text[..a].ends_with("\r\n") {
                    2
                } else {
                    1
                };
            }
            return Ok(vec![splice(a, b, "")]);
        }
        // The first child shares its line with the parent's `- `: pull the
        // next sibling into its place.
        match spans.get(idx + 1) {
            Some(next) => Ok(vec![splice(start, next.0, "")]),
            None => Err(Reason::Unsafe),
        }
    }

    pub fn rename(&self, hit: &Hit<'_>, new_key: &str) -> Result<Vec<Splice>, Reason> {
        let Link::Entry(map, idx) = hit.link else {
            return Err(Reason::Unsafe);
        };
        let Kind::Map { entries, .. } = &map.kind else {
            return Err(Reason::Unsafe);
        };
        let e = &entries[idx];
        let keep = matches!(e.key_style, Style::Plain | Style::Single | Style::Double)
            .then_some(e.key_style);
        Ok(vec![splice(
            e.key_start,
            e.key_end,
            key(new_key, keep, is_flow(map)),
        )])
    }

    /// Move item `from` to `to` in the sequence at `hit`.
    pub fn move_item(&self, hit: &Hit<'_>, from: usize, to: usize) -> Result<Vec<Splice>, Reason> {
        let t = hit.target();
        let Kind::Seq { flow, items } = &t.kind else {
            return Err(Reason::Unsafe);
        };
        let (min, max) = (from.min(to), from.max(to));
        let reorder = |n: usize| -> Vec<usize> {
            let mut order: Vec<usize> = (0..n).collect();
            let moved = order.remove(from - min);
            order.insert(to - min, moved);
            order
        };
        if *flow {
            let order = reorder(max - min + 1);
            let piece = |k: usize| &self.text[items[k].start..items[k].end];
            let mut out = String::new();
            for (n, k) in order.iter().enumerate() {
                out.push_str(piece(min + k));
                if min + n < max {
                    out.push_str(&self.text[items[min + n].end..items[min + n + 1].start]);
                }
            }
            return Ok(vec![splice(items[min].start, items[max].end, out)]);
        }
        let first_on_line = |k: usize| {
            let ls = self.line_start(items[k].start);
            self.text[ls..items[k].start]
                .trim_matches([' ', '\t'])
                .is_empty()
        };
        // The first item of a nested `- - a` shares its line with the dash of
        // its parent: that place stays, and chunks are compared by their text.
        let compact0 = min == 0 && !first_on_line(0);
        if (min..=max).any(|k| !first_on_line(k) && !(k == 0 && compact0)) {
            return Err(Reason::Unsafe);
        }
        let indent = pad(self.col(items[0].start));
        // Whole-line chunks; comment lines right above an item travel with it.
        let chunk_start = |k: usize| {
            if k == 0 && compact0 {
                items[0].start
            } else {
                self.chunk_start(items, k)
            }
        };
        let region_start = chunk_start(min);
        let region_end = self.next_line(self.line_end(items[max].end));
        let mut chunks: Vec<String> = (min..=max)
            .map(|k| {
                let end = if k < max {
                    chunk_start(k + 1)
                } else {
                    region_end
                };
                let text = &self.text[chunk_start(k)..end];
                if k == 0 && compact0 {
                    format!("{indent}{text}")
                } else {
                    text.to_string()
                }
            })
            .collect();
        let unterminated = region_end == self.text.len() && !self.text.ends_with('\n');
        if unterminated && let Some(last) = chunks.last_mut() {
            last.push_str(self.eol);
        }
        let order = reorder(chunks.len());
        let mut out: String = order.iter().map(|&k| chunks[k].as_str()).collect();
        if unterminated {
            out.truncate(out.len() - self.eol.len());
        }
        if compact0 {
            // The first slot follows the parent's dash: no indentation and no
            // comment lines above it.
            out = out.strip_prefix(&indent).ok_or(Reason::Unsafe)?.to_string();
            if out.starts_with([' ', '#']) {
                return Err(Reason::Unsafe);
            }
        }
        Ok(vec![splice(region_start, region_end, out)])
    }
}
