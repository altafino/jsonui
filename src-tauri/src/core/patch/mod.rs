//! JSON Pointer utilities and the shared overlap helper (document lane).

use crate::core::{CoreError, CoreResult, ErrorCode};

/// Decode an RFC 6901 pointer into segments (`~1` → `/`, `~0` → `~`).
///
/// `""` is the root (no segments). Anything else must start with `/`, and `~`
/// may only be followed by `0` or `1`.
pub fn parse_pointer(pointer: &str) -> CoreResult<Vec<String>> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Err(invalid(
            pointer,
            "a JSON Pointer must be empty or start with '/'",
        ));
    };
    rest.split('/')
        .map(|raw| decode_segment(raw).ok_or_else(|| invalid(pointer, "invalid '~' escape")))
        .collect()
}

/// Encode segments into an RFC 6901 pointer.
pub fn to_pointer(segments: &[String]) -> String {
    segments.iter().fold(String::new(), |mut out, seg| {
        out.push('/');
        out.push_str(&escape_segment(seg));
        out
    })
}

/// Escape one segment (`~` → `~0`, `/` → `~1`).
pub fn escape_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn decode_segment(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '~' {
            match chars.next()? {
                '0' => out.push('~'),
                '1' => out.push('/'),
                _ => return None,
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn invalid(pointer: &str, why: &str) -> CoreError {
    CoreError::new(
        ErrorCode::InvalidOperation,
        format!("Invalid JSON Pointer: {why}"),
    )
    .with_path(pointer)
}

/// Do two pointers touch the same data? (eng R16, R18)
///
/// True when equal, when either is a segment-wise prefix of the other, or
/// when both address items of the same array (`is_array` reports whether a
/// pointer addresses an array; it is called with the shared parent pointer).
/// Segment-aware: `/env/A` does not overlap `/env/AB`. Pointers that fail to
/// parse are treated as overlapping everything (the safe answer).
pub fn pointers_overlap(a: &str, b: &str, is_array: &dyn Fn(&str) -> bool) -> bool {
    let (Ok(sa), Ok(sb)) = (parse_pointer(a), parse_pointer(b)) else {
        return true;
    };
    let common = sa.len().min(sb.len());
    if sa[..common] == sb[..common] {
        return true; // equal or one is a prefix of the other (root is a prefix of all)
    }
    // Siblings: same length, same parent, differing last segment.
    sa.len() == sb.len()
        && sa[..sa.len() - 1] == sb[..sb.len() - 1]
        && is_array(&to_pointer(&sa[..sa.len() - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_round_trip() {
        for p in ["", "/a", "/a~1b/c~0d", "/", "//", "/0/1"] {
            assert_eq!(to_pointer(&parse_pointer(p).unwrap()), p, "{p}");
        }
        assert_eq!(parse_pointer("/a~1b/~0").unwrap(), vec!["a/b", "~"]);
        assert_eq!(parse_pointer("/").unwrap(), vec![""]);
    }

    #[test]
    fn bad_pointers() {
        for p in ["a", "/a~", "/a~2", "~0"] {
            assert_eq!(
                parse_pointer(p).unwrap_err().code,
                ErrorCode::InvalidOperation,
                "{p}"
            );
        }
    }

    #[test]
    fn overlap_table() {
        // `/list` and `/a/list` are arrays; nothing else is.
        let is_array = |p: &str| p == "/list" || p == "/a/list";
        let cases: &[(&str, &str, bool)] = &[
            ("/env/A", "/env/A", true),
            ("/env/A", "/env/AB", false),
            ("/env/AB", "/env/A", false),
            ("/env", "/env/A", true),
            ("/env/A", "/env", true),
            ("", "/env/A", true),
            ("/env/A", "", true),
            ("", "", true),
            ("/a~1b", "/a/b", false),
            ("/a~1b", "/a~1b/c", true),
            ("/a~0b", "/a~1b", false),
            ("/a~0b", "/a~0b", true),
            ("/list/0", "/list/1", true),
            ("/list/0", "/list", true),
            ("/list/0/x", "/list/1/y", false),
            ("/env/A", "/env/B", false),
            ("/a/list/2", "/a/list/9", true),
            ("/x/list/2", "/y/list/2", false),
            ("/", "/a", false),
            ("/", "/", true),
            ("/bad~", "/a", true),
        ];
        for (a, b, want) in cases {
            assert_eq!(pointers_overlap(a, b, &is_array), *want, "{a} vs {b}");
            assert_eq!(pointers_overlap(b, a, &is_array), *want, "{b} vs {a}");
        }
    }
}
