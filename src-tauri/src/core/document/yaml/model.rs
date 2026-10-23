//! The yaml-edit syntax tree, flattened into a position-carrying node tree.
//!
//! yaml-edit supplies the lossless parse (every byte belongs to a token);
//! this module reads it into [`Node`]s that know their byte ranges, so the
//! edit code can splice text exactly where a value, key or entry lives.

use std::collections::{HashMap, HashSet};

use yaml_edit::validator::{Rule, Severity, Validator};
use yaml_edit::{AsYaml, SyntaxKind as K, YamlFile};

use super::render::Fmt;
use super::scalar::{Style, decode_block, decode_double, decode_plain, decode_single, plain_val};
use crate::core::document::value::Val;
use crate::core::patch::escape_segment;

/// Why a file cannot be loaded for editing.
pub enum Fail {
    /// Parse or validation error at a 1-based position.
    Syntax {
        line: u32,
        column: u32,
        what: &'static str,
    },
    Multi,
    /// Valid YAML this editor cannot represent.
    Unsupported {
        pos: usize,
        what: &'static str,
    },
    /// An alias without an anchor, an invalid escape, and the like.
    Invalid {
        pos: usize,
        what: &'static str,
    },
    Duplicates(Vec<Dup>),
}

/// A key that appears more than once in one mapping.
pub struct Dup {
    pub pointer: String,
    pub key: String,
    /// Byte offsets of every occurrence's key.
    pub offsets: Vec<usize>,
}

#[derive(Debug)]
pub struct Node {
    pub kind: Kind,
    /// Byte range of the value itself: no anchor/tag, no trailing comment.
    /// `Null` is zero-width at the place a value would be written.
    pub start: usize,
    pub end: usize,
    pub anchor: Option<String>,
    pub tag: Option<String>,
    /// End of the anchor/tag tokens, when present.
    pub props_end: Option<usize>,
}

#[derive(Debug)]
pub enum Kind {
    Null,
    Alias,
    Scalar {
        style: Style,
        /// Decoded text (what a string value reads as).
        text: String,
        typed: Val,
        block_indent: usize,
    },
    Map {
        flow: bool,
        entries: Vec<Entry>,
    },
    Seq {
        flow: bool,
        items: Vec<Item>,
    },
}

#[derive(Debug)]
pub struct Entry {
    pub key: String,
    pub key_style: Style,
    pub key_start: usize,
    pub key_end: usize,
    /// Start of the entry (its `?` for explicit keys, else the key).
    pub start: usize,
    /// End of the entry's content (the value's end, or the colon).
    pub end: usize,
    pub colon_end: Option<usize>,
    pub value: Node,
}

#[derive(Debug)]
pub struct Item {
    /// The dash of a block item; the value of a flow item.
    pub start: usize,
    pub end: usize,
    pub value: Node,
}

impl Node {
    fn null(pos: usize) -> Self {
        Self {
            kind: Kind::Null,
            start: pos,
            end: pos,
            anchor: None,
            tag: None,
            props_end: None,
        }
    }

    /// Whether the subtree defines an anchor that an alias refers to (any
    /// anchor when `any`).
    pub fn defines_anchor(&self, refs: &HashSet<String>, any: bool) -> bool {
        if let Some(a) = &self.anchor
            && (any || refs.contains(a))
        {
            return true;
        }
        match &self.kind {
            Kind::Map { entries, .. } => entries.iter().any(|e| e.value.defines_anchor(refs, any)),
            Kind::Seq { items, .. } => items.iter().any(|i| i.value.defines_anchor(refs, any)),
            _ => false,
        }
    }
}

// ------------------------------------------------------------------- arena

pub struct Raw {
    pub kind: K,
    pub start: usize,
    pub end: usize,
    pub kids: Vec<usize>,
}

fn syntax_phrase(rule: Rule) -> &'static str {
    match rule {
        Rule::InvalidIndentation => "inconsistent indentation",
        Rule::InvalidDocumentMarker => "a document marker in the wrong place",
        Rule::InvalidTabUsage => "a tab where YAML does not allow one",
        Rule::MissingSyntax => "missing syntax",
        Rule::InvalidEscape => "an invalid escape sequence",
        Rule::InvalidAnchor => "an invalid anchor or alias",
        Rule::InvalidTag => "an invalid tag",
        Rule::InvalidCharacter => "a character YAML does not allow",
        _ => "invalid syntax",
    }
}

fn parse_location(loc: &str) -> (u32, u32) {
    let mut parts = loc.split(':').map(|p| p.trim().parse::<u32>().unwrap_or(1));
    (parts.next().unwrap_or(1), parts.next().unwrap_or(1))
}

/// Parse, validate, and flatten. `None` for a file without any document.
fn load(text: &str) -> Result<Option<Vec<Raw>>, Fail> {
    let parse = YamlFile::parse(text);
    if let Some(e) = parse.positioned_errors().first() {
        let lc = e.start_position(text);
        return Err(Fail::Syntax {
            line: lc.line as u32,
            column: lc.column as u32,
            what: "invalid syntax",
        });
    }
    let file = parse.tree();
    let docs: Vec<_> = file.documents().collect();
    if docs.len() > 1 {
        return Err(Fail::Multi);
    }
    let Some(doc) = docs.first() else {
        return Ok(None);
    };
    let Some(doc_node) = doc.as_node() else {
        return Ok(None);
    };
    let root = doc_node
        .ancestors()
        .last()
        .unwrap_or_else(|| doc_node.clone());
    if let Some(v) = Validator::new()
        .validate_syntax(&root)
        .into_iter()
        .find(|v| v.severity == Severity::Error && v.rule != Rule::DuplicateKeys)
    {
        let (line, column) = v.location.as_deref().map_or((1, 1), parse_location);
        return Err(Fail::Syntax {
            line,
            column,
            what: syntax_phrase(v.rule),
        });
    }
    // Walk an immutable copy: the parser hands out a mutable tree, whose
    // bookkeeping is needed for editing only.
    let frozen = root.clone_subtree();
    let mut els: Vec<Raw> = Vec::new();
    let mut path: Vec<usize> = Vec::new();
    for el in frozen.descendants_with_tokens() {
        let depth = el.parent().map_or(0, |p| p.ancestors().count());
        path.truncate(depth);
        let r = el.text_range();
        let idx = els.len();
        els.push(Raw {
            kind: el.kind(),
            start: usize::from(r.start()),
            end: usize::from(r.end()),
            kids: Vec::new(),
        });
        if let Some(&p) = path.last() {
            els[p].kids.push(idx);
        }
        path.push(idx);
    }
    Ok(Some(els))
}

// ------------------------------------------------------------------ parsed

pub struct Parsed {
    pub text: String,
    pub root: Node,
    pub val: Val,
    /// Names of anchors some alias refers to.
    pub refs: HashSet<String>,
    pub eol: &'static str,
    pub fmt: Fmt,
    /// No content at all (blank or comment-only): an empty mapping.
    pub empty: bool,
}

impl Parsed {
    pub fn parse(text: &str) -> Result<Self, Fail> {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let (root, val, refs, empty) = match load(text)? {
            None => (
                Node::null(text.len()),
                Val::Object(Vec::new()),
                HashSet::new(),
                true,
            ),
            Some(els) => {
                let mut b = Builder {
                    text,
                    els: &els,
                    anchors: HashMap::new(),
                    refs: HashSet::new(),
                    dups: Vec::new(),
                };
                let doc = els[0]
                    .kids
                    .iter()
                    .copied()
                    .find(|&k| els[k].kind == K::DOCUMENT)
                    .ok_or(Fail::Invalid {
                        pos: 0,
                        what: "no document",
                    })?;
                let (node, val) = b.slot(doc, "", text.len(), 0)?;
                if !b.dups.is_empty() {
                    return Err(Fail::Duplicates(b.dups));
                }
                if matches!(node.kind, Kind::Null) {
                    (node, Val::Object(Vec::new()), b.refs, true)
                } else {
                    (node, val, b.refs, false)
                }
            }
        };
        let fmt = detect_fmt(text, &root);
        Ok(Self {
            text: text.to_string(),
            root,
            val,
            refs,
            eol,
            fmt,
            empty,
        })
    }

    pub fn line_start(&self, pos: usize) -> usize {
        self.text[..pos].rfind('\n').map_or(0, |i| i + 1)
    }

    pub fn col(&self, pos: usize) -> usize {
        pos - self.line_start(pos)
    }

    /// Position of the line break ending the line that contains `pos`
    /// (the text length on the last line).
    pub fn line_end(&self, pos: usize) -> usize {
        match self.text[pos..].find('\n') {
            Some(i) if i > 0 && self.text.as_bytes()[pos + i - 1] == b'\r' => pos + i - 1,
            Some(i) => pos + i,
            None => self.text.len(),
        }
    }

    /// Start of the line after the one containing `pos` (the text length on
    /// the last line).
    pub fn next_line(&self, pos: usize) -> usize {
        self.text[pos..]
            .find('\n')
            .map_or(self.text.len(), |i| pos + i + 1)
    }
}

/// Indentation conventions of the document, from its first nested blocks.
fn detect_fmt(text: &str, root: &Node) -> Fmt {
    let col = |pos: usize| pos - text[..pos].rfind('\n').map_or(0, |i| i + 1);
    let (mut step, mut indentless) = (None, None);
    fn walk(
        n: &Node,
        col: &dyn Fn(usize) -> usize,
        step: &mut Option<usize>,
        indentless: &mut Option<bool>,
    ) {
        match &n.kind {
            Kind::Map {
                flow: false,
                entries,
            } => {
                for e in entries {
                    if let Kind::Map { flow: false, .. } | Kind::Seq { flow: false, .. } =
                        &e.value.kind
                    {
                        let (kc, vc) = (col(e.start), col(e.value.start));
                        let is_seq = matches!(e.value.kind, Kind::Seq { .. });
                        if is_seq && indentless.is_none() && e.value.start > e.key_end {
                            *indentless = Some(vc == kc);
                        }
                        if vc > kc && step.is_none() {
                            *step = Some(vc - kc);
                        }
                    }
                    walk(&e.value, col, step, indentless);
                }
            }
            Kind::Seq { flow: false, items } => {
                for i in items {
                    walk(&i.value, col, step, indentless);
                }
            }
            _ => {}
        }
    }
    walk(root, &col, &mut step, &mut indentless);
    Fmt {
        step: step.unwrap_or(2),
        indentless: indentless.unwrap_or(false),
    }
}

// ----------------------------------------------------------------- builder

struct Builder<'a> {
    text: &'a str,
    els: &'a [Raw],
    anchors: HashMap<String, Val>,
    refs: HashSet<String>,
    dups: Vec<Dup>,
}

type R<T> = Result<T, Fail>;

fn trivia(k: K) -> bool {
    matches!(k, K::WHITESPACE | K::NEWLINE | K::INDENT | K::COMMENT)
}

impl Builder<'_> {
    fn col(&self, pos: usize) -> usize {
        pos - self.text[..pos].rfind('\n').map_or(0, |i| i + 1)
    }

    fn el(&self, i: usize) -> &Raw {
        &self.els[i]
    }

    fn kid(&self, c: usize, kind: K) -> Option<&Raw> {
        self.el(c)
            .kids
            .iter()
            .map(|&k| self.el(k))
            .find(|e| e.kind == kind)
    }

    /// A value slot (`VALUE`, `SEQUENCE_ENTRY`, `DOCUMENT`, `TAGGED_NODE`):
    /// optional anchor and tag, then the content.
    fn slot(&mut self, c: usize, pointer: &str, fallback: usize, col: usize) -> R<(Node, Val)> {
        let (mut anchor, mut tag, mut props_end, mut content) = (None, None, None::<usize>, None);
        for &k in &self.el(c).kids {
            let e = self.el(k);
            match e.kind {
                K::ANCHOR => {
                    anchor = Some(self.text[e.start + 1..e.end].to_string());
                    props_end = props_end.max(Some(e.end));
                }
                K::TAG => {
                    tag = Some(self.text[e.start..e.end].to_string());
                    props_end = props_end.max(Some(e.end));
                }
                K::SCALAR | K::MAPPING | K::SEQUENCE | K::ALIAS | K::TAGGED_NODE
                    if content.is_none() =>
                {
                    content = Some(k);
                }
                _ => {}
            }
        }
        let (mut node, mut val) = match content {
            Some(k) => self.node(k, pointer, fallback, col)?,
            None => (Node::null(fallback), Val::Null),
        };
        if matches!(node.kind, Kind::Null) {
            node.start = fallback;
            node.end = fallback;
        }
        if let (Some(t), Kind::Scalar { text, .. }) = (&tag, &node.kind)
            && t == "!!str"
        {
            val = Val::String(text.clone());
        }
        node.anchor = anchor.or(node.anchor);
        node.tag = tag.or(node.tag);
        node.props_end = node.props_end.max(props_end);
        if let Some(name) = &node.anchor {
            self.anchors.insert(name.clone(), val.clone());
        }
        Ok((node, val))
    }

    fn node(&mut self, k: usize, pointer: &str, fallback: usize, col: usize) -> R<(Node, Val)> {
        match self.el(k).kind {
            K::SCALAR => self.scalar(k, col),
            K::ALIAS => {
                let e = self.el(k);
                let (start, end) = (e.start, e.end);
                let name = self.text[start + 1..end].to_string();
                let val = self.anchors.get(&name).cloned().ok_or(Fail::Invalid {
                    pos: start,
                    what: "an alias refers to an anchor that is not defined before it",
                })?;
                self.refs.insert(name);
                Ok((
                    Node {
                        kind: Kind::Alias,
                        start,
                        end,
                        anchor: None,
                        tag: None,
                        props_end: None,
                    },
                    val,
                ))
            }
            K::MAPPING => self.mapping(k, pointer, col),
            K::SEQUENCE => self.sequence(k, pointer, col),
            K::TAGGED_NODE => self.slot(k, pointer, fallback, col),
            _ => Err(Fail::Unsupported {
                pos: self.el(k).start,
                what: "unsupported syntax",
            }),
        }
    }

    /// End of the last token that is not whitespace, comment or newline.
    fn content_end(&self, i: usize) -> Option<usize> {
        let e = self.el(i);
        if e.kids.is_empty() {
            return (!trivia(e.kind) && e.start != e.end).then_some(e.end);
        }
        e.kids.iter().rev().find_map(|&k| self.content_end(k))
    }

    fn scalar(&mut self, k: usize, col: usize) -> R<(Node, Val)> {
        let e = self.el(k);
        let first = e
            .kids
            .iter()
            .map(|&i| self.el(i))
            .find(|t| !trivia(t.kind))
            .ok_or(Fail::Invalid {
                pos: e.start,
                what: "an empty scalar",
            })?;
        let bad = |pos| Fail::Invalid {
            pos,
            what: "an invalid quoted string",
        };
        let node = |start, end, kind| Node {
            kind,
            start,
            end,
            anchor: None,
            tag: None,
            props_end: None,
        };
        if first.kind == K::NULL && first.start == first.end {
            return Ok((Node::null(first.start), Val::Null));
        }
        let start = first.start;
        let end = self.content_end(k).unwrap_or(first.end);
        match first.kind {
            K::UNTERMINATED_STRING => Err(bad(start)),
            K::PIPE | K::GREATER => {
                let raw = &self.text[start..e.end];
                let (text, block_indent) =
                    decode_block(raw, first.kind == K::GREATER, col).ok_or_else(|| bad(start))?;
                let style = if first.kind == K::PIPE {
                    Style::Literal
                } else {
                    Style::Folded
                };
                let typed = Val::String(text.clone());
                Ok((
                    node(
                        start,
                        end,
                        Kind::Scalar {
                            style,
                            text,
                            typed: typed.clone(),
                            block_indent,
                        },
                    ),
                    typed,
                ))
            }
            _ => {
                let raw = &self.text[start..end];
                let (style, text) = match raw.chars().next() {
                    Some('"') => (Style::Double, decode_double(raw).ok_or_else(|| bad(start))?),
                    Some('\'') => (Style::Single, decode_single(raw).ok_or_else(|| bad(start))?),
                    _ => (Style::Plain, decode_plain(raw)),
                };
                let typed = if style == Style::Plain {
                    plain_val(&text)
                } else {
                    Val::String(text.clone())
                };
                Ok((
                    node(
                        start,
                        end,
                        Kind::Scalar {
                            style,
                            text,
                            typed: typed.clone(),
                            block_indent: 0,
                        },
                    ),
                    typed,
                ))
            }
        }
    }

    fn key(&self, c: usize) -> R<(String, Style, usize, usize)> {
        let unsupported = |pos| Fail::Unsupported {
            pos,
            what: "complex mapping keys",
        };
        let mut scalar = None;
        for &k in &self.el(c).kids {
            let e = self.el(k);
            match e.kind {
                K::SCALAR => scalar = Some(k),
                K::ANCHOR | K::TAG | K::MAPPING | K::SEQUENCE | K::ALIAS | K::TAGGED_NODE => {
                    return Err(unsupported(e.start));
                }
                _ => {}
            }
        }
        let k = scalar.ok_or_else(|| unsupported(self.el(c).start))?;
        let (start, end) = (&self.el(k).start, &self.el(k).end);
        let probe = self
            .el(k)
            .kids
            .iter()
            .map(|&i| self.el(i))
            .find(|t| !trivia(t.kind));
        if probe.is_none_or(|t| matches!(t.kind, K::PIPE | K::GREATER)) {
            return Err(unsupported(*start));
        }
        let (node, _) = self.scalar_readonly(k)?;
        match node.kind {
            Kind::Scalar { style, text, .. } => Ok((text, style, node.start, node.end)),
            Kind::Null => Ok((String::new(), Style::Plain, *start, *end)),
            _ => Err(unsupported(*start)),
        }
    }

    /// `scalar` for keys, which never register anchors or touch state.
    fn scalar_readonly(&self, k: usize) -> R<(Node, Val)> {
        let mut scratch = Builder {
            text: self.text,
            els: self.els,
            anchors: HashMap::new(),
            refs: HashSet::new(),
            dups: Vec::new(),
        };
        scratch.scalar(k, 0)
    }

    fn mapping(&mut self, k: usize, pointer: &str, _col: usize) -> R<(Node, Val)> {
        let kids = self.el(k).kids.clone();
        let flow = kids.iter().any(|&i| self.el(i).kind == K::LEFT_BRACE);
        let mut entries: Vec<Entry> = Vec::new();
        let mut members: Vec<(String, Val)> = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        let els = self.els;
        for &e in kids.iter().filter(|&&i| els[i].kind == K::MAPPING_ENTRY) {
            let question = self.kid(e, K::QUESTION).map(|q| q.start);
            let key_c = self
                .el(e)
                .kids
                .iter()
                .copied()
                .find(|&i| self.el(i).kind == K::KEY)
                .ok_or(Fail::Unsupported {
                    pos: self.el(e).start,
                    what: "mapping entries without a key",
                })?;
            let (key, key_style, key_start, key_end) = self.key(key_c)?;
            let colon_end = self.kid(e, K::COLON).map(|c| c.end);
            let child = format!("{pointer}/{}", escape_segment(&key));
            let key_col = self.col(key_start);
            let value_c = self
                .el(e)
                .kids
                .iter()
                .copied()
                .find(|&i| self.el(i).kind == K::VALUE);
            let (value, val) = match value_c {
                Some(vc) => self.slot(vc, &child, colon_end.unwrap_or(key_end), key_col)?,
                None => (Node::null(colon_end.unwrap_or(key_end)), Val::Null),
            };
            let end = if matches!(value.kind, Kind::Null) && value.props_end.is_none() {
                colon_end.unwrap_or(key_end)
            } else {
                value.end.max(value.props_end.unwrap_or(0))
            };
            let group = groups.entry(key.clone()).or_default();
            if group.is_empty() {
                order.push(key.clone());
            }
            group.push(key_start);
            members.push((key.clone(), val));
            entries.push(Entry {
                key,
                key_style,
                key_start,
                key_end,
                start: question.unwrap_or(key_start),
                end,
                colon_end,
                value,
            });
        }
        for key in order {
            let offsets = groups.remove(&key).unwrap_or_default();
            if offsets.len() > 1 {
                self.dups.push(Dup {
                    pointer: format!("{pointer}/{}", escape_segment(&key)),
                    key,
                    offsets,
                });
            }
        }
        let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
            // `{}` has no entries; an empty block mapping cannot occur.
            let (open, close) = self.brackets(&kids, K::LEFT_BRACE, K::RIGHT_BRACE, k)?;
            return Ok((
                Node {
                    kind: Kind::Map { flow, entries },
                    start: open,
                    end: close,
                    anchor: None,
                    tag: None,
                    props_end: None,
                },
                Val::Object(members),
            ));
        };
        let (mut start, mut end) = (first.start, last.end);
        if flow {
            (start, end) = self.brackets(&kids, K::LEFT_BRACE, K::RIGHT_BRACE, k)?;
        }
        // Duplicates keep their first value in the neutral map.
        let mut seen = HashSet::new();
        members.retain(|(k, _)| seen.insert(k.clone()));
        Ok((
            Node {
                kind: Kind::Map { flow, entries },
                start,
                end,
                anchor: None,
                tag: None,
                props_end: None,
            },
            Val::Object(members),
        ))
    }

    fn brackets(&self, kids: &[usize], open: K, close: K, k: usize) -> R<(usize, usize)> {
        let find = |kind| kids.iter().map(|&i| self.el(i)).find(|e| e.kind == kind);
        match (find(open), find(close)) {
            (Some(o), Some(c)) => Ok((o.start, c.end)),
            _ => Err(Fail::Unsupported {
                pos: self.el(k).start,
                what: "an empty block collection",
            }),
        }
    }

    fn sequence(&mut self, k: usize, pointer: &str, _col: usize) -> R<(Node, Val)> {
        let kids = self.el(k).kids.clone();
        let flow = kids.iter().any(|&i| self.el(i).kind == K::LEFT_BRACKET);
        let mut items: Vec<Item> = Vec::new();
        let mut vals: Vec<Val> = Vec::new();
        let els = self.els;
        for &e in kids.iter().filter(|&&i| els[i].kind == K::SEQUENCE_ENTRY) {
            if let Some(odd) = self
                .el(e)
                .kids
                .iter()
                .map(|&i| self.el(i))
                .find(|t| matches!(t.kind, K::KEY | K::VALUE | K::COLON | K::QUESTION))
            {
                return Err(Fail::Unsupported {
                    pos: odd.start,
                    what: "single-pair flow mappings",
                });
            }
            let dash = self.kid(e, K::DASH).map(|d| (d.start, d.end));
            let fallback = dash.map_or(self.el(e).start, |d| d.1);
            let col = dash.map_or(0, |d| self.col(d.0));
            let child = format!("{pointer}/{}", items.len());
            let (value, val) = self.slot(e, &child, fallback, col)?;
            let start = dash.map_or(value.start, |d| d.0);
            let end = if matches!(value.kind, Kind::Null) && value.props_end.is_none() {
                fallback
            } else {
                value.end.max(value.props_end.unwrap_or(0))
            };
            vals.push(val);
            items.push(Item { start, end, value });
        }
        let (start, end) = if flow {
            self.brackets(&kids, K::LEFT_BRACKET, K::RIGHT_BRACKET, k)?
        } else {
            match (items.first(), items.last()) {
                (Some(f), Some(l)) => (f.start, l.end),
                _ => {
                    return Err(Fail::Unsupported {
                        pos: self.el(k).start,
                        what: "an empty block collection",
                    });
                }
            }
        };
        Ok((
            Node {
                kind: Kind::Seq { flow, items },
                start,
                end,
                anchor: None,
                tag: None,
                props_end: None,
            },
            Val::Array(vals),
        ))
    }
}
