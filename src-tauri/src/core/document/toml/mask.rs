//! Masked source for a TOML file that cannot be edited (eng R17).
//!
//! Textual and conservative: keys, `[table]` headers and comments stay
//! visible; everything after `key =` (strings, numbers, multi-line bodies,
//! array and inline-table contents) is masked. A line that does not look like
//! a header, a comment or `key = value` is masked whole, so a pasted secret
//! never shows even when the file is badly broken.

use crate::core::security::redact::Masker;
use crate::core::types::{SourceCaret, SourceView};

pub(super) fn mask_source(
    text: &str,
    error_line: Option<u32>,
    error_column: Option<u32>,
) -> (SourceView, Vec<String>) {
    let ranges = Scan::new(text).run();
    let mut masker = Masker::new(0);
    let mut cursor = 0;
    for (start, end) in ranges {
        masker.push_plain(&text[cursor..start]);
        masker.push_masked(&text[start..end]);
        cursor = end;
    }
    masker.push_plain(&text[cursor..]);
    let (masked, masks, originals) = masker.finish();
    let view = SourceView {
        text: masked,
        masks,
        highlight_lines: error_line.into_iter().collect(),
        caret: error_line
            .zip(error_column)
            .map(|(line, column)| SourceCaret { line, column }),
    };
    (view, originals)
}

struct Scan<'a> {
    b: &'a [u8],
    masked: Vec<(usize, usize)>,
}

fn is_bare(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

impl<'a> Scan<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            b: text.as_bytes(),
            masked: Vec::new(),
        }
    }

    fn at(&self, i: usize) -> u8 {
        self.b.get(i).copied().unwrap_or(0)
    }

    fn mask(&mut self, start: usize, end: usize) {
        if start < end {
            self.masked.push((start, end));
        }
    }

    fn eol(&self, from: usize) -> usize {
        self.b[from.min(self.b.len())..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(self.b.len(), |p| from + p)
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        while matches!(self.at(i), b' ' | b'\t') {
            i += 1;
        }
        i
    }

    fn run(mut self) -> Vec<(usize, usize)> {
        let n = self.b.len();
        let mut i = 0;
        while i < n {
            i = self.skip_ws(i);
            match self.at(i) {
                0 if i >= n => break,
                b'\n' | b'\r' => i += 1,
                b'#' => i = self.eol(i),
                b'[' => i = self.header(i),
                _ => i = self.statement(i),
            }
        }
        self.masked.sort_unstable();
        self.masked
    }

    /// A quoted key part starting at `i`; the end after the closing quote.
    fn quoted_key(&self, i: usize) -> Option<usize> {
        let q = self.at(i);
        let mut j = i + 1;
        loop {
            let c = self.at(j);
            if j >= self.b.len() || c == b'\n' {
                return None;
            }
            if c == b'\\' && q == b'"' {
                j += 2;
            } else if c == q {
                return Some(j + 1);
            } else {
                j += 1;
            }
        }
    }

    /// `[a.b]` or `[[a.b]]`; masks junk after it. Returns the end of the line.
    fn header(&mut self, start: usize) -> usize {
        let eol = self.eol(start);
        let mut i = start + 1;
        let double = self.at(i) == b'[';
        if double {
            i += 1;
        }
        loop {
            i = self.skip_ws(i);
            match self.at(i) {
                b'"' | b'\'' => match self.quoted_key(i) {
                    Some(end) => i = end,
                    None => {
                        self.mask(i + 1, eol);
                        return eol;
                    }
                },
                c if is_bare(c) => {
                    while is_bare(self.at(i)) {
                        i += 1;
                    }
                }
                _ => {}
            }
            i = self.skip_ws(i);
            if self.at(i) == b'.' {
                i += 1;
            } else {
                break;
            }
        }
        let close = if double { b"]]".as_slice() } else { b"]" };
        if self.b[i.min(eol)..eol].starts_with(close) {
            i += close.len();
            let j = self.skip_ws(i);
            if matches!(self.at(j), b'#' | b'\n' | b'\r' | 0) {
                return eol;
            }
            self.mask(j, self.comment_start(j, eol));
            return eol;
        }
        // A header that never closes: hide the rest of the line.
        self.mask(i, eol);
        eol
    }

    /// Start of a trailing `# comment` at or after `from` outside quotes, else `eol`.
    fn comment_start(&self, from: usize, eol: usize) -> usize {
        let mut i = from;
        while i < eol {
            match self.at(i) {
                b'#' => return i,
                b'"' | b'\'' => {
                    let q = self.at(i);
                    i += 1;
                    while i < eol && self.at(i) != q {
                        i += if q == b'"' && self.at(i) == b'\\' {
                            2
                        } else {
                            1
                        };
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        eol
    }

    /// `key = value` (or garbage). Returns where scanning resumes.
    fn statement(&mut self, start: usize) -> usize {
        let eol = self.eol(start);
        let mut i = start;
        loop {
            i = self.skip_ws(i);
            match self.at(i) {
                b'"' | b'\'' => match self.quoted_key(i) {
                    Some(end) => i = end,
                    None => return self.junk(start, eol),
                },
                c if is_bare(c) => {
                    while is_bare(self.at(i)) {
                        i += 1;
                    }
                }
                _ => return self.junk(start, eol),
            }
            i = self.skip_ws(i);
            if self.at(i) == b'.' {
                i += 1;
            } else {
                break;
            }
        }
        if self.at(i) != b'=' {
            return self.junk(start, eol);
        }
        self.value(i + 1)
    }

    fn junk(&mut self, start: usize, eol: usize) -> usize {
        self.mask(start, eol);
        eol
    }

    /// Mask a value starting at `from` (may span lines inside brackets and
    /// multi-line strings); comments stay visible. Returns where to resume.
    fn value(&mut self, from: usize) -> usize {
        let n = self.b.len();
        let mut i = from;
        let mut depth = 0usize;
        while i < n {
            match self.at(i) {
                b'\n' if depth == 0 => return i,
                b' ' | b'\t' | b'\r' | b'\n' | b',' | b'=' => i += 1,
                b'#' => i = self.eol(i),
                b'[' | b'{' => {
                    depth += 1;
                    i += 1;
                }
                b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                }
                q @ (b'"' | b'\'') => i = self.string(i, q),
                _ => {
                    let start = i;
                    while i < n
                        && !matches!(
                            self.at(i),
                            b' ' | b'\t'
                                | b'\r'
                                | b'\n'
                                | b','
                                | b']'
                                | b'}'
                                | b'#'
                                | b'='
                                | b'['
                                | b'{'
                        )
                    {
                        i += 1;
                    }
                    self.mask(start, i);
                }
            }
        }
        n
    }

    /// Mask the inside of the string starting at `open`; the end after it.
    fn string(&mut self, open: usize, q: u8) -> usize {
        let n = self.b.len();
        let multi = self.at(open + 1) == q && self.at(open + 2) == q;
        if multi {
            let body = open + 3;
            let mut i = body;
            while i < n {
                match self.at(i) {
                    b'\\' if q == b'"' => i += 2,
                    c if c == q => {
                        let run = self.b[i..].iter().take_while(|&&c| c == q).count();
                        if run >= 3 {
                            self.mask(body, i + run - 3);
                            return i + run;
                        }
                        i += run;
                    }
                    _ => i += 1,
                }
            }
            self.mask(body, n);
            return n;
        }
        let mut i = open + 1;
        while i < n {
            match self.at(i) {
                b'\n' => break,
                b'\\' if q == b'"' => i += 2,
                c if c == q => {
                    self.mask(open + 1, i);
                    return i + 1;
                }
                _ => i += 1,
            }
        }
        let end = i.min(self.eol(open));
        self.mask(open + 1, end);
        end
    }
}
