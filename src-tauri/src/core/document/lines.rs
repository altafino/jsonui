//! Line diff with context hunks for the Review dialog's "Text diff" tab.

use crate::core::types::DiffLineKind;

/// One line of a hunk before masking. Line numbers are 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

const CONTEXT: usize = 3;
/// LCS table cap; above it the changed middle is shown as remove-all/add-all.
const MAX_CELLS: usize = 4_000_000;

/// Split keeping line terminators, so a missing final newline is a difference.
fn split(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn display(line: &str) -> String {
    line.trim_end_matches(['\n', '\r']).to_string()
}

/// Changed lines of `old` → `new` with 3 lines of context, grouped in hunks.
///
/// Edits are local, so the common prefix and suffix are trimmed first and an
/// LCS runs only on the middle. ponytail: past MAX_CELLS the middle is a full
/// replace; swap in Myers if reformat-sized diffs ever matter.
pub fn diff_hunks(old: &str, new: &str) -> Vec<Vec<RawLine>> {
    let (a, b) = (split(old), split(new));
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (mid_a, mid_b) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    if mid_a.is_empty() && mid_b.is_empty() {
        return Vec::new();
    }

    let mut entries: Vec<RawLine> = Vec::new();
    let line = |kind, o: Option<usize>, n: Option<usize>, text: &str| RawLine {
        kind,
        old_line: o.map(|i| i as u32 + 1),
        new_line: n.map(|i| i as u32 + 1),
        text: display(text),
    };
    for (i, text) in a
        .iter()
        .enumerate()
        .take(prefix)
        .skip(prefix.saturating_sub(CONTEXT))
    {
        entries.push(line(DiffLineKind::Context, Some(i), Some(i), text));
    }
    for (kind, oi, ni) in lcs_ops(mid_a, mid_b) {
        let (o, n) = (oi.map(|i| i + prefix), ni.map(|i| i + prefix));
        let text = if kind == DiffLineKind::Added {
            b[n.unwrap_or(0)]
        } else {
            a[o.unwrap_or(0)]
        };
        entries.push(line(kind, o, n, text));
    }
    for k in 0..suffix.min(CONTEXT) {
        let (o, n) = (a.len() - suffix + k, b.len() - suffix + k);
        entries.push(line(DiffLineKind::Context, Some(o), Some(n), a[o]));
    }
    group(entries)
}

/// Edit script for the middle section as (kind, old index, new index).
fn lcs_ops(a: &[&str], b: &[&str]) -> Vec<(DiffLineKind, Option<usize>, Option<usize>)> {
    use DiffLineKind::{Added, Context, Removed};
    if a.len() * b.len() > MAX_CELLS {
        return (0..a.len())
            .map(|i| (Removed, Some(i), None))
            .chain((0..b.len()).map(|j| (Added, None, Some(j))))
            .collect();
    }
    let (n, m) = (a.len(), b.len());
    // lcs[i][j] = LCS length of a[i..] and b[j..]
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if a[i] == b[j] {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::new();
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push((Context, Some(i), Some(j)));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
            ops.push((Removed, Some(i), None)); // removals before additions on ties
            i += 1;
        } else {
            ops.push((Added, None, Some(j)));
            j += 1;
        }
    }
    ops
}

/// Keep changed lines plus 3 context lines either side; split where context runs out.
fn group(entries: Vec<RawLine>) -> Vec<Vec<RawLine>> {
    let changed: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind != DiffLineKind::Context)
        .map(|(i, _)| i)
        .collect();
    let mut keep = vec![false; entries.len()];
    for c in &changed {
        let from = c.saturating_sub(CONTEXT);
        let to = (c + CONTEXT).min(entries.len().saturating_sub(1));
        keep[from..=to].iter_mut().for_each(|k| *k = true);
    }
    let mut hunks: Vec<Vec<RawLine>> = Vec::new();
    let mut open = false;
    for (entry, keep) in entries.into_iter().zip(keep) {
        if !keep {
            open = false;
            continue;
        }
        if !open {
            hunks.push(Vec::new());
            open = true;
        }
        if let Some(hunk) = hunks.last_mut() {
            hunk.push(entry);
        }
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use DiffLineKind::{Added, Context, Removed};

    fn kinds(h: &[RawLine]) -> Vec<DiffLineKind> {
        h.iter().map(|l| l.kind).collect()
    }

    #[test]
    fn identical_has_no_hunks() {
        assert!(diff_hunks("a\nb\n", "a\nb\n").is_empty());
        assert!(diff_hunks("", "").is_empty());
    }

    #[test]
    fn single_line_change_with_context() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let new = "1\n2\n3\n4\nX\n6\n7\n8\n9\n";
        let hunks = diff_hunks(old, new);
        assert_eq!(hunks.len(), 1);
        let h = &hunks[0];
        assert_eq!(
            kinds(h),
            [
                Context, Context, Context, Removed, Added, Context, Context, Context
            ]
        );
        assert_eq!(
            (h[0].old_line, h[0].new_line, h[0].text.as_str()),
            (Some(2), Some(2), "2")
        );
        assert_eq!((h[3].old_line, h[3].new_line), (Some(5), None));
        assert_eq!(
            (h[4].old_line, h[4].new_line, h[4].text.as_str()),
            (None, Some(5), "X")
        );
    }

    #[test]
    fn far_apart_changes_split_into_two_hunks() {
        let old: String = (1..=30).map(|i| format!("{i}\n")).collect();
        let new = old.replacen("3\n", "three\n", 1).replacen("28\n", "x\n", 1);
        assert_eq!(diff_hunks(&old, &new).len(), 2);
    }

    #[test]
    fn insert_delete_and_missing_final_newline() {
        let hunks = diff_hunks("a\nb\nc\n", "a\nc\n");
        assert_eq!(kinds(&hunks[0]), [Context, Removed, Context]);
        let hunks = diff_hunks("a\nb", "a\nb\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(kinds(&hunks[0]), [Context, Removed, Added]);
    }

    #[test]
    fn crlf_is_trimmed_for_display_only() {
        let hunks = diff_hunks("a\r\nb\r\n", "a\r\nc\r\n");
        assert_eq!(
            hunks[0].iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn huge_middle_falls_back_to_replace() {
        let old: String = (0..2500).map(|i| format!("o{i}\n")).collect();
        let new: String = (0..2500).map(|i| format!("n{i}\n")).collect();
        let hunks = diff_hunks(&old, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].len(), 5000);
    }
}
