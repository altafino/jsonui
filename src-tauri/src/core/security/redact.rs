//! Secret masking (document lane).

use jsonc_parser::{Scanner, ScannerOptions, tokens::Token};

use crate::core::patch::parse_pointer;
use crate::core::types::{MaskSpan, SourceCaret, SourceView};

/// Mask glyph; one per masked character.
pub const MASK_CHAR: char = '•';

/// Default secret-like key test for a pointer's last segment
/// (`*TOKEN*`, `*KEY*`, `*SECRET*`, `*PASSWORD*`, case-insensitive). Advisory only (spec §6).
pub fn default_is_secret(pointer: &str) -> bool {
    let Ok(segments) = parse_pointer(pointer) else {
        return false;
    };
    let Some(last) = segments.last() else {
        return false;
    };
    let last = last.to_uppercase();
    ["TOKEN", "KEY", "SECRET", "PASSWORD"]
        .iter()
        .any(|needle| last.contains(needle))
}

/// Accumulates masked text plus the original text of each mask (ids are
/// positions in `originals`, offset by `first_id`).
pub struct Masker {
    text: String,
    utf16_len: u32,
    spans: Vec<MaskSpan>,
    originals: Vec<String>,
    first_id: u32,
}

impl Masker {
    pub fn new(first_id: u32) -> Self {
        Self {
            text: String::new(),
            utf16_len: 0,
            spans: Vec::new(),
            originals: Vec::new(),
            first_id,
        }
    }

    /// Append text that stays visible.
    pub fn push_plain(&mut self, s: &str) {
        self.text.push_str(s);
        self.utf16_len += s.encode_utf16().count() as u32;
    }

    /// Append `original` as runs of `•` (one per char). Line breaks inside
    /// stay, so line numbers do not shift; each line's run is its own mask.
    /// Empty input adds nothing.
    pub fn push_masked(&mut self, original: &str) {
        for piece in original.split_inclusive('\n') {
            let body = piece.trim_end_matches(['\n', '\r']);
            let n = body.chars().count() as u32;
            if n > 0 {
                let id = self.first_id + self.originals.len() as u32;
                self.spans.push(MaskSpan {
                    id,
                    start: self.utf16_len,
                    end: self.utf16_len + n,
                });
                self.originals.push(body.to_string());
                self.text.extend(std::iter::repeat_n(MASK_CHAR, n as usize));
                self.utf16_len += n;
            }
            self.push_plain(&piece[body.len()..]);
        }
    }

    pub fn finish(self) -> (String, Vec<MaskSpan>, Vec<String>) {
        (self.text, self.spans, self.originals)
    }
}

/// Masked source for a file that does not parse (eng R17): tokenize with the
/// jsonc Scanner, mask every string value (a string not followed by `:`),
/// and after a lexical error mask every quoted run on the remaining lines.
/// Returns the view and the original text of each mask id.
///
/// The quotes themselves stay visible; only the characters between them are
/// masked, so a missing quote is still easy to see.
pub fn mask_unparseable(
    text: &str,
    error_line: Option<u32>,
    error_column: Option<u32>,
) -> (SourceView, Vec<String>) {
    let (ranges, lexical_error_at) = value_string_ranges(text);
    // After a lexical error the scanner cannot continue: mask quoted runs
    // line by line from the start of the offending line.
    let tail_start =
        lexical_error_at.map_or(text.len(), |at| text[..at].rfind('\n').map_or(0, |i| i + 1));
    let mut masker = Masker::new(0);
    let mut cursor = 0;
    for (start, end) in ranges.into_iter().filter(|&(_, end)| end <= tail_start) {
        masker.push_plain(&text[cursor..start]);
        masker.push_masked(&text[start..end]);
        cursor = end;
    }
    masker.push_plain(&text[cursor..tail_start]);
    mask_quoted_runs(&text[tail_start..], &mut masker);
    let (masked_text, masks, originals) = masker.finish();
    let view = SourceView {
        text: masked_text,
        masks,
        highlight_lines: error_line.into_iter().collect(),
        caret: error_line
            .zip(error_column)
            .map(|(line, column)| SourceCaret { line, column }),
    };
    (view, originals)
}

/// Byte ranges (inside the quotes) of every string token not followed by `:`,
/// plus the byte offset where the scanner hit a lexical error, if any.
fn value_string_ranges(text: &str) -> (Vec<(usize, usize)>, Option<usize>) {
    let options = ScannerOptions::default();
    let mut scanner = Scanner::new(text, &options);
    let mut tokens: Vec<(Token<'_>, usize, usize)> = Vec::new();
    let mut error_at = None;
    loop {
        match scanner.scan() {
            Ok(Some(token)) => tokens.push((token, scanner.token_start(), scanner.token_end())),
            Ok(None) => break,
            Err(_) => {
                // The scanner lets a broken string run across lines, so the
                // last good token may be the start of the damage too.
                let last_start = tokens.last().map_or(usize::MAX, |(_, start, _)| *start);
                error_at = Some(scanner.token_start().min(last_start).min(text.len()));
                break;
            }
        }
    }
    let mut ranges = Vec::new();
    for (i, (token, start, end)) in tokens.iter().enumerate() {
        // Strings span both quotes (only the inside is masked). Bare words are
        // not valid values, but a pasted token without quotes is still a secret.
        let range = match token {
            Token::String(_) if end - start >= 2 => (start + 1, end - 1),
            Token::Word(_) => (*start, *end),
            _ => continue,
        };
        let next = tokens[i + 1..]
            .iter()
            .find(|(t, ..)| !matches!(t, Token::CommentLine(_) | Token::CommentBlock(_)))
            .map(|(t, ..)| t);
        if !matches!(next, Some(Token::Colon)) {
            ranges.push(range);
        }
    }
    (ranges, error_at)
}

/// Byte index of the first `"` when the line has an odd number of unescaped `"`.
fn unbalanced_quote(body: &str) -> Option<usize> {
    let mut escaped = false;
    let quotes: Vec<usize> = body
        .char_indices()
        .filter(|&(_, c)| {
            let is_quote = c == '"' && !escaped;
            escaped = c == '\\' && !escaped;
            is_quote
        })
        .map(|(i, _)| i)
        .collect();
    (quotes.len() % 2 == 1).then(|| quotes[0])
}

/// Mask everything between a quote and the next unescaped matching quote (or
/// the end of the line when unterminated), line by line.
fn mask_quoted_runs(tail: &str, masker: &mut Masker) {
    for line in tail.split_inclusive('\n') {
        let body_len = line.trim_end_matches(['\n', '\r']).len();
        let (body, eol) = line.split_at(body_len);
        // An odd number of quotes means a quote is missing somewhere, so pairs
        // cannot be trusted: hide everything after the first quote.
        if let Some(first) = unbalanced_quote(body) {
            masker.push_plain(&body[..=first]);
            masker.push_masked(&body[first + 1..]);
            masker.push_plain(eol);
            continue;
        }
        let mut i = 0;
        let mut plain_from = 0;
        while i < body.len() {
            let c = body[i..].chars().next().unwrap_or_default();
            if c != '"' && c != '\'' {
                i += c.len_utf8();
                continue;
            }
            let open = i + 1;
            let mut close = body.len();
            let mut escaped = false;
            for (j, d) in body[open..].char_indices() {
                if escaped {
                    escaped = false;
                } else if d == '\\' {
                    escaped = true;
                } else if d == c {
                    close = open + j;
                    break;
                }
            }
            masker.push_plain(&body[plain_from..open]);
            masker.push_masked(&body[open..close]);
            plain_from = close;
            i = (close + 1).min(body.len());
        }
        masker.push_plain(&body[plain_from..]);
        masker.push_plain(eol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
            .unwrap()
    }

    #[test]
    fn default_secret_names() {
        for p in [
            "/env/API_TOKEN",
            "/a/password",
            "/x/mySecret",
            "/apiKey",
            "/a~1key",
        ] {
            assert!(default_is_secret(p), "{p}");
        }
        for p in ["", "/env", "/env/A", "/token/x", "bad"] {
            assert!(!default_is_secret(p), "{p}");
        }
    }

    #[test]
    fn masks_values_but_not_keys() {
        let text = "{\n  \"API_TOKEN\": \"sk-test\",\n  \"auth\": \"ghx_fake\"\n  \"b\": 2\n}\n";
        let (view, originals) = mask_unparseable(text, Some(3), Some(10));
        assert!(!view.text.contains("sk-test") && !view.text.contains("ghx_fake"));
        assert!(view.text.contains("\"API_TOKEN\"") && view.text.contains("\"auth\""));
        assert_eq!(originals, vec!["sk-test", "ghx_fake"]);
        assert_eq!(view.masks.len(), 2);
        assert_eq!(view.highlight_lines, vec![3]);
        assert_eq!(
            view.caret,
            Some(SourceCaret {
                line: 3,
                column: 10
            })
        );
        assert_eq!(view.text.chars().count(), text.chars().count());
        let first = &view.masks[0];
        assert_eq!((first.start, first.end), (18, 25));
    }

    #[test]
    fn mask_spans_are_utf16_units() {
        let text = "{\"k\": \"😀x\", \"j\": \"yé\" \"z\"}";
        let (view, originals) = mask_unparseable(text, None, None);
        assert_eq!(originals[0], "😀x");
        // Masked text uses one `•` (1 UTF-16 unit) per char, so offsets count masked units.
        let units: Vec<u16> = view.text.encode_utf16().collect();
        for m in &view.masks {
            assert!(
                units[m.start as usize..m.end as usize]
                    .iter()
                    .all(|u| *u == MASK_CHAR as u16)
            );
        }
    }

    #[test]
    fn missing_comma_and_unterminated_never_leak() {
        let missing = "{\n  \"a\": \"sk-secret-123\"\n  \"b\": \"ghx_other\"\n}\n";
        let (v, _) = mask_unparseable(missing, Some(3), Some(3));
        assert!(!v.text.contains("sk-secret-123") && !v.text.contains("ghx_other"));

        let unterminated = "{\n  \"a\": \"sk-secret-123\",\n  \"b\": \"ghx_unterminated,\n  \"c\": \"tail-secret\"\n}\n";
        let (v, originals) = mask_unparseable(unterminated, Some(3), Some(8));
        for leak in ["sk-secret-123", "ghx_unterminated", "tail-secret"] {
            assert!(!v.text.contains(leak), "{leak} leaked: {}", v.text);
        }
        assert!(originals.iter().any(|o| o.contains("tail-secret")));
        assert_eq!(v.text.lines().count(), unterminated.lines().count());
    }

    #[test]
    fn fixtures_with_secrets_stay_masked_in_broken_variants() {
        let base = read("secret.json");
        let variants = [
            base.replace("\"auth\":", "\"auth\" "), // missing colon
            base.replacen(
                "\"sk-test-0000000000000000\"",
                "\"sk-test-0000000000000000",
                1,
            ), // unterminated
            base.replace(",\n", "\n")
                .replace("fake_token_value\"", "fake_token_value\"\n  \"x\": 1"),
            format!("{}\n// trailing", base.trim_end().trim_end_matches('}')),
            base.replace("\"API_TOKEN\"", "\"API_TOKEN")
                .replace("}\n  }", "}"),
        ];
        for text in variants {
            let (view, _) = mask_unparseable(&text, Some(1), Some(1));
            assert!(
                !view.text.contains("sk-test-0000000000000000"),
                "{text}\n---\n{}",
                view.text
            );
            assert!(!view.text.contains("ghx_fake_token_value"), "{}", view.text);
        }
    }

    #[test]
    fn multi_line_scanner_strings_keep_line_count() {
        // The scanner lets a broken string run to the next quote, across lines.
        let text = "{\n  \"a\": \"secret-one,\n  \"b\": \"secret-two\"\n}\n";
        let (view, _) = mask_unparseable(text, None, None);
        assert!(!view.text.contains("secret"), "{}", view.text);
        assert_eq!(view.text.lines().count(), text.lines().count());
        assert_eq!(view.text.chars().count(), text.chars().count());
    }

    #[test]
    fn plain_escapes_and_single_quotes_in_tail() {
        let text = "{\"a\": \"it's\\\" fine\", 'sec': 'x'\n \"b\": \"unterminated";
        let (view, _) = mask_unparseable(text, None, None);
        assert!(!view.text.contains("unterminated"));
        assert!(!view.text.contains("fine"));
    }
}
