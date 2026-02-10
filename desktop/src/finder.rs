mod directory_search;
pub(crate) mod nucleo_worker;

pub use directory_search::*;

use std::collections::HashSet;

/// Fuzzy Finder search mode
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FinderMode {
    /// Search within the current file only
    #[default]
    File,
    /// Search across all files in the current directory
    Directory,
}

/// Build highlighted segments from line text and match indices.
///
/// Returns `(text, is_match)` pairs suitable for rendering in both the
/// Fuzzy Finder result cards and the right sidebar directory results.
///
/// The match_indices are UTF-32 character positions from nucleo-matcher.
/// Uses a HashSet for O(1) lookup instead of linear scan.
pub fn build_highlight_segments(text: &str, match_indices: &[u32]) -> Vec<(String, bool)> {
    if match_indices.is_empty() {
        return vec![(text.to_string(), false)];
    }

    let index_set: HashSet<u32> = match_indices.iter().copied().collect();
    let mut segments = Vec::new();
    let mut current_segment = String::new();
    let mut current_is_match = false;

    for (i, ch) in text.chars().enumerate() {
        let is_match = index_set.contains(&(i as u32));

        if is_match != current_is_match && !current_segment.is_empty() {
            segments.push((std::mem::take(&mut current_segment), current_is_match));
        }

        current_is_match = is_match;
        current_segment.push(ch);
    }

    if !current_segment.is_empty() {
        segments.push((current_segment, current_is_match));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_highlight_segments_empty_indices() {
        let segments = build_highlight_segments("hello", &[]);
        assert_eq!(segments, vec![("hello".to_string(), false)]);
    }

    #[test]
    fn test_build_highlight_segments_contiguous_match() {
        let segments = build_highlight_segments("hello", &[0, 1, 2, 3, 4]);
        assert_eq!(segments, vec![("hello".to_string(), true)]);
    }

    #[test]
    fn test_build_highlight_segments_scattered_match() {
        let segments = build_highlight_segments("hello", &[0, 2, 3]);
        assert_eq!(
            segments,
            vec![
                ("h".to_string(), true),
                ("e".to_string(), false),
                ("ll".to_string(), true),
                ("o".to_string(), false),
            ]
        );
    }

    #[test]
    fn test_build_highlight_segments_japanese_text() {
        // "日本語のtest文字列" → match "tes" at char indices 4,5,6
        let text = "日本語のtest文字列";
        let segments = build_highlight_segments(text, &[4, 5, 6]);
        assert_eq!(
            segments,
            vec![
                ("日本語の".to_string(), false),
                ("tes".to_string(), true),
                ("t文字列".to_string(), false),
            ]
        );
    }

    /// End-to-end test: nucleo match_indices → build_highlight_segments
    /// for text with Japanese characters.
    #[test]
    fn test_nucleo_indices_match_highlight_segments_japanese() {
        use nucleo_matcher::{Config, Matcher, Utf32Str};

        let line = "rebase だけでなく cherry-pick, merge, reset 等も";
        let query = "mgt";

        let mut config = Config::DEFAULT;
        config.ignore_case = true;
        let mut matcher = Matcher::new(config);

        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(query, &mut needle_buf);
        let mut haystack_buf = Vec::new();
        let haystack = Utf32Str::new(line, &mut haystack_buf);
        let mut indices = Vec::new();

        let score = matcher.fuzzy_indices(haystack, needle, &mut indices);
        assert!(score.is_some(), "nucleo should match 'mgt' in this text");

        // Verify indices point to actual 'm', 'g', 't' characters
        let chars: Vec<char> = line.chars().collect();
        for &idx in &indices {
            let ch = chars[idx as usize];
            assert!(
                ch.is_ascii(),
                "index {} points to '{}' (U+{:04X}), expected ASCII char",
                idx,
                ch,
                ch as u32
            );
        }

        // Verify build_highlight_segments uses these indices correctly
        let segments = build_highlight_segments(line, &indices);
        let matched_text: String = segments
            .iter()
            .filter(|(_, is_match)| *is_match)
            .map(|(text, _)| text.as_str())
            .collect();

        // All highlighted characters should be from "mgt"
        for ch in matched_text.chars() {
            assert!(
                "mgt".contains(ch),
                "highlighted char '{}' is not in query 'mgt'",
                ch
            );
        }
    }
}
