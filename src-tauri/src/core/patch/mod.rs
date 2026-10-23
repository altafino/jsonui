//! JSON Pointer utilities and the shared overlap helper (document lane).

/// Decode an RFC 6901 pointer into segments (`~1` → `/`, `~0` → `~`).
pub fn parse_pointer(pointer: &str) -> crate::core::CoreResult<Vec<String>> {
    let _ = pointer;
    todo!("document lane")
}

/// Encode segments into an RFC 6901 pointer.
pub fn to_pointer(segments: &[String]) -> String {
    let _ = segments;
    todo!("document lane")
}

/// Do two pointers touch the same data? (eng R16, R18)
///
/// True when equal, when either is a segment-wise prefix of the other, or
/// when both address items of the same array (`is_array` reports whether a
/// pointer addresses an array). Segment-aware: `/env/A` does not overlap `/env/AB`.
pub fn pointers_overlap(a: &str, b: &str, is_array: &dyn Fn(&str) -> bool) -> bool {
    let _ = (a, b, is_array);
    todo!("document lane")
}
