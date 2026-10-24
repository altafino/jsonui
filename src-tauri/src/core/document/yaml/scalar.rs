//! YAML 1.2 core-schema typing, scalar decoding, and scalar emission.

use crate::core::document::value::Val;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Single,
    Double,
    Literal,
    Folded,
}

// ------------------------------------------------------------------ typing

/// Type a plain scalar's text by the YAML 1.2 core schema.
pub fn plain_val(s: &str) -> Val {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => Val::Null,
        "true" | "True" | "TRUE" => Val::Bool(true),
        "false" | "False" | "FALSE" => Val::Bool(false),
        _ if is_number(s) => Val::Number(s.to_string()),
        _ => Val::String(s.to_string()),
    }
}

fn digits(b: &[u8], i: &mut usize) -> usize {
    let from = *i;
    while b.get(*i).is_some_and(u8::is_ascii_digit) {
        *i += 1;
    }
    *i - from
}

fn is_number(s: &str) -> bool {
    if let Some(h) = s.strip_prefix("0x") {
        return !h.is_empty() && h.bytes().all(|c| c.is_ascii_hexdigit());
    }
    if let Some(o) = s.strip_prefix("0o") {
        return !o.is_empty() && o.bytes().all(|c| (b'0'..=b'7').contains(&c));
    }
    if matches!(s, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if matches!(t, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    let b = t.as_bytes();
    let mut i = 0;
    let int = digits(b, &mut i);
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        i += 1;
        frac = digits(b, &mut i);
    }
    if int == 0 && frac == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(b, &mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

// ---------------------------------------------------------------- decoding

fn is_blank_ch(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Fold the line breaks of a plain or single-quoted scalar.
pub fn fold_lines(raw: &str) -> String {
    let raw = raw.replace("\r\n", "\n");
    let mut out = String::new();
    let (mut blanks, mut first) = (0usize, true);
    let lines: Vec<&str> = raw.split('\n').collect();
    let last = lines.len() - 1;
    for (i, line) in lines.into_iter().enumerate() {
        let t = match (i == 0, i == last) {
            (true, true) => line,
            (true, false) => line.trim_end_matches(is_blank_ch),
            (false, true) => line.trim_start_matches(is_blank_ch),
            _ => line.trim_matches(is_blank_ch),
        };
        if t.is_empty() && !first {
            blanks += 1;
            continue;
        }
        if first {
            out.push_str(t);
            first = false;
        } else {
            if blanks == 0 {
                out.push(' ');
            } else {
                out.extend(std::iter::repeat_n('\n', blanks));
            }
            out.push_str(t);
        }
        blanks = 0;
    }
    out
}

pub fn decode_plain(raw: &str) -> String {
    fold_lines(raw)
}

/// `raw` includes both quotes.
pub fn decode_single(raw: &str) -> Option<String> {
    let inner = raw.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(fold_lines(inner).replace("''", "'"))
}

/// `raw` includes both quotes. `None` on an invalid escape.
pub fn decode_double(raw: &str) -> Option<String> {
    let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
    let chars: Vec<char> = inner.replace("\r\n", "\n").chars().collect();
    let n = chars.len();
    let (mut out, mut pending, mut i) = (String::new(), String::new(), 0);
    while i < n {
        match chars[i] {
            c @ (' ' | '\t') => {
                pending.push(c);
                i += 1;
            }
            '\n' => {
                pending.clear();
                let mut breaks = 1;
                i += 1;
                loop {
                    while i < n && is_blank_ch(chars[i]) {
                        i += 1;
                    }
                    if i < n && chars[i] == '\n' {
                        breaks += 1;
                        i += 1;
                    } else {
                        break;
                    }
                }
                if breaks == 1 {
                    out.push(' ');
                } else {
                    out.extend(std::iter::repeat_n('\n', breaks - 1));
                }
            }
            '\\' => {
                out.push_str(&pending);
                pending.clear();
                i += 1;
                let e = *chars.get(i)?;
                i += 1;
                let hex = |len: usize, i: &mut usize| -> Option<char> {
                    let s: String = chars.get(*i..*i + len)?.iter().collect();
                    *i += len;
                    char::from_u32(u32::from_str_radix(&s, 16).ok()?)
                };
                match e {
                    '0' => out.push('\0'),
                    'a' => out.push('\u{7}'),
                    'b' => out.push('\u{8}'),
                    't' | '\t' => out.push('\t'),
                    'n' => out.push('\n'),
                    'v' => out.push('\u{b}'),
                    'f' => out.push('\u{c}'),
                    'r' => out.push('\r'),
                    'e' => out.push('\u{1b}'),
                    ' ' => out.push(' '),
                    '"' => out.push('"'),
                    '/' => out.push('/'),
                    '\\' => out.push('\\'),
                    'N' => out.push('\u{85}'),
                    '_' => out.push('\u{a0}'),
                    'L' => out.push('\u{2028}'),
                    'P' => out.push('\u{2029}'),
                    'x' => out.push(hex(2, &mut i)?),
                    'u' => out.push(hex(4, &mut i)?),
                    'U' => out.push(hex(8, &mut i)?),
                    '\n' => {
                        while i < n && is_blank_ch(chars[i]) {
                            i += 1;
                        }
                    }
                    _ => return None,
                }
            }
            c => {
                out.push_str(&pending);
                pending.clear();
                out.push(c);
                i += 1;
            }
        }
    }
    out.push_str(&pending);
    Some(out)
}

/// Decode a block scalar from its source (`|`/`>` header through the last
/// content line). Returns the text and the content indentation.
/// `parent_col` is the column of the owning key or dash.
pub fn decode_block(raw: &str, folded: bool, parent_col: usize) -> Option<(String, usize)> {
    let raw = raw.replace("\r\n", "\n");
    let final_newline = raw.ends_with('\n');
    let mut lines: Vec<&str> = raw.split('\n').collect();
    let header = lines.remove(0);
    if final_newline {
        lines.pop();
    }
    let (mut keep, mut strip, mut explicit) = (false, false, None);
    for c in header.chars().skip(1) {
        match c {
            '+' => keep = true,
            '-' => strip = true,
            '1'..='9' => explicit = c.to_digit(10).map(|d| d as usize),
            _ => break,
        }
    }
    let indent = explicit.map_or_else(
        || {
            lines
                .iter()
                .find(|l| !l.trim_matches(' ').is_empty())
                .map_or(0, |l| l.len() - l.trim_start_matches(' ').len())
        },
        |d| parent_col + d,
    );
    let mut content: Vec<&str> = lines
        .iter()
        .map(|l| {
            let lead = l.len() - l.trim_start_matches(' ').len();
            if lead >= indent { &l[indent..] } else { "" }
        })
        .collect();
    let mut trailing = 0;
    while content
        .last()
        .is_some_and(|l| l.trim_matches(' ').is_empty())
    {
        content.pop();
        trailing += 1;
    }
    let mut text = if folded {
        fold_block(&content)
    } else {
        content.join("\n")
    };
    if content.is_empty() {
        if keep {
            text = "\n".repeat(trailing);
        }
    } else if !strip && (final_newline || trailing > 0) {
        text.push('\n');
        if keep {
            text.push_str(&"\n".repeat(trailing));
        }
    }
    Some((text, indent))
}

fn fold_block(lines: &[&str]) -> String {
    let mut out = String::new();
    let (mut blanks, mut first, mut prev_more) = (0usize, true, false);
    for line in lines {
        if line.is_empty() {
            blanks += 1;
            continue;
        }
        let more = line.starts_with(is_blank_ch);
        if first {
            out.extend(std::iter::repeat_n('\n', blanks));
        } else if prev_more || more {
            out.extend(std::iter::repeat_n('\n', blanks + 1));
        } else if blanks == 0 {
            out.push(' ');
        } else {
            out.extend(std::iter::repeat_n('\n', blanks));
        }
        out.push_str(line);
        (blanks, first, prev_more) = (0, false, more);
    }
    out
}

// ---------------------------------------------------------------- emission

fn is_special_char(c: char) -> bool {
    c.is_control() || matches!(c, '\u{feff}' | '\u{2028}' | '\u{2029}')
}

/// Can `s` be written as a plain scalar and read back as the same string,
/// by YAML 1.2 and by common YAML 1.1 readers?
pub fn plain_ok(s: &str, flow: bool) -> bool {
    let mut chars = s.chars();
    let (Some(first), Some(last)) = (chars.next(), s.chars().next_back()) else {
        return false;
    };
    if !matches!(plain_val(s), Val::String(_)) || s == "<<" {
        return false;
    }
    if matches!(s.to_ascii_lowercase().as_str(), "yes" | "no" | "on" | "off") {
        return false;
    }
    if is_blank_ch(first) || is_blank_ch(last) || last == ':' || s.chars().any(is_special_char) {
        return false;
    }
    if ",[]{}#&*!|>'\"%@`?:".contains(first) {
        return false;
    }
    if first == '-' && !chars.next().is_some_and(char::is_alphanumeric) {
        return false;
    }
    if s.starts_with("...") || s.contains(": ") || s.contains(" #") {
        return false;
    }
    // Date-like text reads back as a timestamp in common YAML 1.1 readers.
    let b = s.as_bytes();
    if b.len() >= 5 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-' {
        return false;
    }
    !(flow && s.contains([',', '[', ']', '{', '}']))
}

pub fn single_ok(s: &str) -> bool {
    !s.chars().any(is_special_char)
}

pub fn single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            c if is_special_char(c) => {
                let n = u32::from(c);
                if n < 0x100 {
                    out.push_str(&format!("\\x{n:02x}"));
                } else {
                    out.push_str(&format!("\\u{n:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A string as one scalar, keeping `keep` (the style it replaces) when that
/// style can represent it.
pub fn emit_string(s: &str, keep: Option<Style>, flow: bool) -> String {
    match keep {
        Some(Style::Single) if single_ok(s) => single_quote(s),
        Some(Style::Double) => double_quote(s),
        _ if plain_ok(s, flow) => s.to_string(),
        _ => double_quote(s),
    }
}

/// A scalar value (not a collection) as flow-safe or block text.
pub fn emit_scalar(v: &Val, keep: Option<Style>, flow: bool) -> String {
    match v {
        Val::Null => "null".to_string(),
        Val::Bool(b) => b.to_string(),
        Val::Number(n) => n.clone(),
        Val::String(s) => emit_string(s, keep, flow),
        Val::Array(_) | Val::Object(_) => String::new(),
    }
}

/// `s` as a literal block scalar body (header plus indented lines, no final
/// newline), or `None` when a block scalar cannot hold it faithfully.
pub fn literal_block(s: &str, indent: usize, eol: &str) -> Option<String> {
    let body = s.trim_end_matches('\n');
    let trailing = s.len() - body.len();
    if body.is_empty()
        || s.chars()
            .any(|c| c != '\n' && c != '\t' && is_special_char(c))
        || body.starts_with(' ')
        || body.starts_with('\n')
        || body
            .split('\n')
            .any(|l| !l.is_empty() && l.trim_matches([' ', '\t']).is_empty())
    {
        return None;
    }
    let chomp = match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let pad = " ".repeat(indent);
    let lines: Vec<String> = body
        .split('\n')
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect();
    let mut out = format!("|{chomp}{eol}{}", lines.join(eol));
    for _ in 2..=trailing {
        out.push_str(eol);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_schema_typing() {
        for (s, v) in [
            ("true", Val::Bool(true)),
            ("False", Val::Bool(false)),
            ("~", Val::Null),
            ("", Val::Null),
            ("0x1F", Val::Number("0x1F".into())),
            ("0o17", Val::Number("0o17".into())),
            ("-1.5e3", Val::Number("-1.5e3".into())),
            (".5", Val::Number(".5".into())),
            ("1.", Val::Number("1.".into())),
            ("-.inf", Val::Number("-.inf".into())),
            (".nan", Val::Number(".nan".into())),
            ("yes", Val::String("yes".into())),
            ("1_000", Val::String("1_000".into())),
            ("1e", Val::String("1e".into())),
            (".", Val::String(".".into())),
            ("0b11", Val::String("0b11".into())),
        ] {
            assert_eq!(plain_val(s), v, "{s}");
        }
    }

    #[test]
    fn decodes_quoted_and_folded() {
        assert_eq!(decode_single("'it''s'").unwrap(), "it's");
        assert_eq!(decode_double(r#""a\tbé\x41""#).unwrap(), "a\tb\u{e9}A");
        assert_eq!(decode_double("\"q\n  w\"").unwrap(), "q w");
        assert_eq!(decode_double("\"q\n\n  w\"").unwrap(), "q\nw");
        assert_eq!(decode_double("\"a \\\n   b\"").unwrap(), "a b");
        assert_eq!(decode_plain("plain\n  continued"), "plain continued");
        assert!(decode_double(r#""\q""#).is_none());
    }

    #[test]
    fn decodes_block_scalars() {
        let d = |raw, folded| decode_block(raw, folded, 0).unwrap().0;
        assert_eq!(d("|\n  a\n  b\n", false), "a\nb\n");
        assert_eq!(d("|-\n  a\n  b\n", false), "a\nb");
        assert_eq!(d("|+\n  a\n\n", false), "a\n\n");
        assert_eq!(d(">\n  a\n  b\n\n  c\n", true), "a b\nc\n");
        assert_eq!(d(">-\n  a\n   x\n  b\n", true), "a\n x\nb");
        assert_eq!(d("|\n  a\n    b\n", false), "a\n  b\n");
        assert_eq!(d("|1\n  a\n", false), " a\n");
        assert_eq!(d("|\r\n  a\r\n", false), "a\n");
    }

    #[test]
    fn emission_round_trips_through_decoding() {
        for s in [
            "web",
            "123",
            "true",
            "null",
            "",
            " lead",
            "a: b",
            "x #y",
            "- x",
            "-x",
            "it's",
            "q\"uote",
            "tab\there",
            "line\nbreak",
            "yes",
            "2024-01-02",
            "<<",
            "a,b",
            "[x]",
            "é ü",
            "...",
        ] {
            let plain = emit_string(s, None, false);
            let back = if plain.starts_with('"') {
                decode_double(&plain).unwrap()
            } else {
                match plain_val(&plain) {
                    Val::String(t) => t,
                    other => panic!("{s:?} -> {plain:?} -> {other:?}"),
                }
            };
            assert_eq!(back, s, "{plain}");
            assert_eq!(
                decode_single(&single_quote(s))
                    .filter(|_| single_ok(s))
                    .unwrap_or_else(|| s.into()),
                s
            );
        }
    }

    #[test]
    fn literal_blocks() {
        assert_eq!(literal_block("a\nb\n", 2, "\n").unwrap(), "|\n  a\n  b");
        assert_eq!(literal_block("a", 2, "\n").unwrap(), "|-\n  a");
        assert_eq!(literal_block("a\n\n", 2, "\n").unwrap(), "|+\n  a\n");
        assert!(literal_block(" a\n", 2, "\n").is_none());
        let (back, _) = decode_block("|\n  a\n\n  b\n", false, 0).unwrap();
        assert_eq!(back, "a\n\nb\n");
    }
}
