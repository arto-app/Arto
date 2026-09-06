//! Shared utilities for search tab components.

/// Split context string into (before, matched, after) parts.
///
/// Takes character indices and converts them to byte indices for safe slicing.
pub fn split_context(
    context: &str,
    char_start: usize,
    char_end: usize,
) -> (String, String, String) {
    let byte_start = char_index_to_byte_index(context, char_start);
    let byte_end = char_index_to_byte_index(context, char_end);

    let before = &context[..byte_start];
    let matched = &context[byte_start..byte_end];
    let after = &context[byte_end..];

    (before.to_string(), matched.to_string(), after.to_string())
}

/// Convert a character index to a byte index in a UTF-8 string.
fn char_index_to_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(byte_pos, _)| byte_pos)
        .unwrap_or(s.len())
}
