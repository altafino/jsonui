//! Source view of a YAML file that cannot be edited: every value masked.
//!
//! The file may be broken, so this is a conservative line scanner, not a
//! parse. Keys, comments, dashes, anchors, tags and document markers stay
//! visible; everything else after a `key:` or `- `, block scalar bodies, and
//! the contents of flow collections and multi-line quotes are masked.

use crate::core::security::redact::Masker;
use crate::core::types::{SourceCaret, SourceView};

#[derive(Default)]
struct State {
    /// Inside a block scalar whose owner sits at this column.
    block: Option<usize>,
    /// Inside a quoted scalar continued from an earlier line.
    quote: Option<char>,
    /// Flow collection nesting continued from an earlier line.
    depth: usize,
    /// Column of the key or dash owning the open flow collection or quote.
    owner: usize,
}

pub fn masked_view(
    text: &str,
    highlight_lines: Vec<u32>,
    caret: Option<(u32, u32)>,
) -> (SourceView, Vec<String>) {
    let mut masker = Masker::new(0);
    let mut st = State::default();
    for line in text.split_inclusive('\n') {
        let body_len = line.trim_end_matches(['\n', '\r']).len();
        let (body, eol) = line.split_at(body_len);
        mask_line(body, &mut st, &mut masker);
        masker.push_plain(eol);
    }
    let (masked, masks, originals) = masker.finish();
    (
        SourceView {
            text: masked,
            masks,
            highlight_lines,
            caret: caret.map(|(line, column)| SourceCaret { line, column }),
        },
        originals,
    )
}

fn lead(s: &str) -> usize {
    s.len() - s.trim_start_matches([' ', '\t']).len()
}

/// Byte index of the quote closing the quoted run `s` continues (`s` starts
/// inside the quotes).
fn closing_quote(s: &str, q: char) -> Option<usize> {
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if q == '"' && c == '\\' {
            it.next();
        } else if c == q {
            if q == '\'' && it.peek().is_some_and(|&(_, n)| n == '\'') {
                it.next();
            } else {
                return Some(i);
            }
        }
    }
    None
}

fn mask_line(body: &str, st: &mut State, m: &mut Masker) {
    let indent = lead(body);
    let (pad, rest) = body.split_at(indent);
    if let Some(parent) = st.block {
        if rest.is_empty() {
            m.push_plain(body);
            return;
        }
        if indent > parent {
            m.push_plain(pad);
            m.push_masked(rest);
            return;
        }
        st.block = None;
    }
    // Flow content must be indented past its owner; a line that is not has
    // left the (unclosed) collection.
    if st.depth > 0 && indent <= st.owner {
        st.depth = 0;
    }
    m.push_plain(pad);
    if let Some(q) = st.quote {
        match closing_quote(rest, q) {
            Some(j) => {
                m.push_masked(&rest[..j]);
                m.push_plain(&rest[j..=j]);
                st.quote = None;
                mask_value(&rest[j + 1..], st.owner, st, m);
            }
            None => m.push_masked(rest),
        }
        return;
    }
    if st.depth > 0 {
        mask_value(rest, st.owner, st, m);
        return;
    }
    if rest.is_empty() || rest.starts_with('#') || rest.starts_with('%') {
        m.push_plain(rest);
        return;
    }
    for marker in ["---", "..."] {
        if indent == 0
            && let Some(after) = rest.strip_prefix(marker)
            && (after.is_empty() || after.starts_with([' ', '\t']))
        {
            m.push_plain(marker);
            mask_value(after, 0, st, m);
            return;
        }
    }
    let mut cur = rest;
    let (mut col, mut owner) = (indent, indent);
    while let Some(after) = ["- ", "? ", ": "].iter().find_map(|p| cur.strip_prefix(p)) {
        m.push_plain(&cur[..2]);
        owner = col;
        let skip = lead(after);
        m.push_plain(&after[..skip]);
        col += 2 + skip;
        cur = &after[skip..];
    }
    if cur == "-" || cur == "?" || cur == ":" {
        m.push_plain(cur);
        return;
    }
    match key_colon(cur) {
        Some(colon) => {
            m.push_plain(&cur[..=colon]);
            mask_value(&cur[colon + 1..], col, st, m);
        }
        None => mask_value(cur, owner, st, m),
    }
}

/// Byte index of the colon ending the implicit key at the start of `cur`.
fn key_colon(cur: &str) -> Option<usize> {
    let quoted = cur.starts_with(['"', '\'']);
    if quoted {
        let q = cur.chars().next()?;
        let end = closing_quote(&cur[1..], q)? + 1;
        let after = end + 1 + lead(&cur[end + 1..]);
        let tail = &cur[after..];
        return (tail.starts_with(':') && tail[1..].chars().next().is_none_or(char::is_whitespace))
            .then_some(after);
    }
    if cur.starts_with(['[', '{', '|', '>', '*', '&', '!', '%', '@', '`', '#']) {
        return None;
    }
    let mut prev_space = true;
    let mut it = cur.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if c == ':' && it.peek().is_none_or(|&(_, n)| n.is_whitespace()) {
            return Some(i);
        }
        if c == '#' && prev_space {
            return None;
        }
        prev_space = c.is_whitespace();
    }
    None
}

/// Mask the value text `s`, keeping props, a block scalar header and a
/// trailing comment. `owner_col` is the column of the key or dash owning it.
fn mask_value(s: &str, owner_col: usize, st: &mut State, m: &mut Masker) {
    let mut s = s;
    // Whitespace, anchors and tags stay visible.
    loop {
        let skip = lead(s);
        m.push_plain(&s[..skip]);
        s = &s[skip..];
        if s.starts_with(['&', '!']) {
            let end = s.find([' ', '\t']).unwrap_or(s.len());
            m.push_plain(&s[..end]);
            s = &s[end..];
        } else {
            break;
        }
    }
    if s.is_empty() {
        return;
    }
    if block_header(s).is_some() {
        m.push_plain(s);
        st.block = Some(owner_col);
        return;
    }
    // Scan for the comment, tracking quotes and flow nesting.
    let mut end = s.len();
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    let mut start_of_token = true;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if let Some(q) = quote {
            if q == '"' && c == '\\' {
                it.next();
            } else if c == q {
                if q == '\'' && it.peek().is_some_and(|&(_, n)| n == '\'') {
                    it.next();
                } else {
                    quote = None;
                }
            }
            prev = c;
            continue;
        }
        match c {
            '#' if prev.is_whitespace() => {
                end = i;
                break;
            }
            '"' | '\'' if start_of_token => quote = Some(c),
            '[' | '{' if start_of_token || st.depth > 0 => st.depth += 1,
            ']' | '}' => st.depth = st.depth.saturating_sub(1),
            _ => {}
        }
        start_of_token = c.is_whitespace() || matches!(c, '[' | '{' | ',' | ':');
        prev = c;
    }
    st.quote = quote;
    if quote.is_some() || st.depth > 0 {
        st.owner = owner_col;
    }
    let (value, comment) = s.split_at(end);
    let trimmed = value.trim_end_matches([' ', '\t']);
    m.push_masked(trimmed);
    m.push_plain(&value[trimmed.len()..]);
    m.push_plain(comment);
}

/// Length of a block scalar header (`|`, `>-`, `|2+`), when `s` is one
/// optionally followed by a comment.
fn block_header(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    if !matches!(chars.next()?.1, '|' | '>') {
        return None;
    }
    let len = s
        .char_indices()
        .skip(1)
        .find(|&(_, c)| !matches!(c, '+' | '-' | '1'..='9'))
        .map_or(s.len(), |(i, _)| i);
    let tail = s[len..].trim_start_matches([' ', '\t']);
    (tail.is_empty() || tail.starts_with('#')).then_some(len)
}
