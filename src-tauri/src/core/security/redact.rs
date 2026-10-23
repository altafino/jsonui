//! Secret masking (document lane).

use crate::core::types::SourceView;

/// Default secret-like key test for a pointer's last segment
/// (`*TOKEN*`, `*KEY*`, `*SECRET*`, `*PASSWORD*`, case-insensitive). Advisory only (spec §6).
pub fn default_is_secret(pointer: &str) -> bool {
    let _ = pointer;
    todo!("document lane")
}

/// Masked source for a file that does not parse (eng R17): tokenize with the
/// jsonc Scanner, mask every string value (a string not followed by `:`),
/// and after a lexical error mask every quoted run on the remaining lines.
/// Returns the view and the original text of each mask id.
pub fn mask_unparseable(text: &str, error_line: Option<u32>, error_column: Option<u32>) -> (SourceView, Vec<String>) {
    let _ = (text, error_line, error_column);
    todo!("document lane")
}
