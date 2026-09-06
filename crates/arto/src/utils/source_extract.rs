use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, TagEnd};

/// Read a source file and extract lines in the range `start..=end` (1-based, inclusive).
///
/// Uses `BufReader` to read line-by-line, avoiding loading the entire file into memory.
/// Returns `None` if the file cannot be read. Lines beyond the file length are
/// silently ignored, so the result may contain fewer lines than requested.
pub fn extract_source_lines(file: impl AsRef<Path>, start: u32, end: u32) -> Option<String> {
    // Lines are 1-based; reject 0 or inverted ranges
    if start == 0 || end < start {
        return None;
    }
    let reader = BufReader::new(File::open(file.as_ref()).ok()?);
    let mut result = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line_num = (idx as u32) + 1;
        if line_num > end {
            break;
        }
        if line_num >= start {
            result.push(line.ok()?);
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("\n"))
    }
}

// ============================================================================
// Source selection extraction (rendered text → markdown source mapping)
// ============================================================================

/// A segment mapping between rendered plain text and markdown source byte positions.
struct TextSegment {
    rendered: Range<usize>,
    source: Range<usize>,
}

/// Build a mapping from rendered plain text to markdown source positions.
///
/// Parses the markdown source with pulldown-cmark, concatenating all visible
/// text events into a "rendered" string while recording which source byte
/// range each rendered segment came from.
fn build_source_map(source: &str) -> (String, Vec<TextSegment>) {
    let parser = Parser::new_ext(source, Options::all());
    let mut rendered = String::new();
    let mut segments = Vec::new();

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Text(text) => {
                let start = rendered.len();
                rendered.push_str(&text);
                segments.push(TextSegment {
                    rendered: start..rendered.len(),
                    source: range,
                });
            }
            Event::Code(text) => {
                let start = rendered.len();
                rendered.push_str(&text);
                // Adjust source range to skip backtick delimiters
                let text_offset = source[range.clone()].find(&*text).unwrap_or(0);
                let adjusted_start = range.start + text_offset;
                segments.push(TextSegment {
                    rendered: start..rendered.len(),
                    source: adjusted_start..adjusted_start + text.len(),
                });
            }
            Event::SoftBreak => {
                // Soft break renders as a space in HTML (within a paragraph)
                let start = rendered.len();
                rendered.push(' ');
                if !range.is_empty() {
                    segments.push(TextSegment {
                        rendered: start..rendered.len(),
                        source: range,
                    });
                }
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                rendered.push('\n');
            }
            _ => {}
        }
    }

    (rendered, segments)
}

/// Find the source byte range corresponding to a rendered text selection.
///
/// When the selection boundary aligns with a segment boundary, the range is
/// expanded to include surrounding formatting markers. For example, selecting
/// rendered "bold" from source `**bold**` returns the range covering `**bold**`.
fn find_source_range(
    segments: &[TextSegment],
    source_len: usize,
    rendered_start: usize,
    rendered_end: usize,
) -> Option<Range<usize>> {
    if segments.is_empty() {
        return None;
    }

    // Find first segment overlapping with the selection
    let first_idx = segments
        .iter()
        .position(|s| s.rendered.end > rendered_start)?;
    // Find last segment overlapping with the selection
    let last_idx = segments
        .iter()
        .rposition(|s| s.rendered.start < rendered_end)?;

    // Compute source start
    let src_start = if rendered_start <= segments[first_idx].rendered.start {
        // Selection starts at/before this segment — include formatting marker before it
        if first_idx > 0 {
            segments[first_idx - 1].source.end
        } else {
            segments[first_idx].source.start
        }
    } else {
        // Selection starts within this segment — direct offset mapping
        let offset = rendered_start - segments[first_idx].rendered.start;
        segments[first_idx].source.start + offset
    };

    // Compute source end
    let src_end = if rendered_end >= segments[last_idx].rendered.end {
        // Selection ends at/after this segment — include formatting marker after it
        if last_idx + 1 < segments.len() {
            segments[last_idx + 1].source.start
        } else {
            segments[last_idx].source.end
        }
    } else {
        // Selection ends within this segment — direct offset mapping
        let offset = rendered_end - segments[last_idx].rendered.start;
        segments[last_idx].source.start + offset
    };

    if src_start <= src_end && src_end <= source_len {
        Some(src_start..src_end)
    } else {
        None
    }
}

/// Extract the markdown source substring corresponding to a rendered text selection.
///
/// Parses the source markdown to build a rendered↔source position mapping,
/// finds where `selected_text` appears in the rendered output, and extracts
/// the corresponding portion of the original markdown source — including any
/// surrounding inline formatting markers (e.g., `**`, `*`, `` ` ``).
///
/// Returns `None` if the selected text cannot be located in the rendered output.
pub fn extract_source_selection(source: &str, selected_text: &str) -> Option<String> {
    if selected_text.is_empty() || source.is_empty() {
        return None;
    }

    let (rendered, segments) = build_source_map(source);

    // Find all occurrences of the selected text in the rendered output.
    // If there are multiple matches the selection is ambiguous, so return
    // None rather than incorrectly mapping an arbitrary occurrence.
    let mut matches = rendered.match_indices(selected_text);
    let (rendered_start, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let rendered_end = rendered_start + selected_text.len();

    let range = find_source_range(&segments, source.len(), rendered_start, rendered_end)?;
    Some(source[range].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    // ========================================================================
    // extract_source_lines tests
    // ========================================================================

    #[test]
    fn test_extract_single_line() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            line 1
            line 2
            line 3
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 2, 2),
            Some("line 2".to_string())
        );
    }

    #[test]
    fn test_extract_range() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            alpha
            beta
            gamma
            delta
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 2, 3),
            Some("beta\ngamma".to_string())
        );
    }

    #[test]
    fn test_extract_entire_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            one
            two
            three
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 1, 3),
            Some("one\ntwo\nthree".to_string())
        );
    }

    #[test]
    fn test_extract_beyond_file_length() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            only
            two
        "},
        )
        .unwrap();

        // Request lines 1-100, should return only existing lines
        assert_eq!(
            extract_source_lines(&file, 1, 100),
            Some("only\ntwo".to_string())
        );
    }

    #[test]
    fn test_extract_nonexistent_file() {
        assert_eq!(extract_source_lines("/nonexistent/path.md", 1, 5), None);
    }

    #[test]
    fn test_extract_start_beyond_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "one line\n").unwrap();

        assert_eq!(extract_source_lines(&file, 10, 20), None);
    }

    #[test]
    fn test_extract_zero_start() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "line 1\nline 2\n").unwrap();

        // start=0 is invalid for 1-based line numbers
        assert_eq!(extract_source_lines(&file, 0, 5), None);
    }

    #[test]
    fn test_extract_inverted_range() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        // end < start is an inverted range
        assert_eq!(extract_source_lines(&file, 5, 2), None);
    }

    // ========================================================================
    // extract_source_selection tests
    // ========================================================================

    #[test]
    fn test_selection_plain_text() {
        assert_eq!(
            extract_source_selection("hello world end", "world"),
            Some("world".to_string())
        );
    }

    #[test]
    fn test_selection_bold_full() {
        // Selecting the entire bold word includes the ** markers
        assert_eq!(
            extract_source_selection("hello **world** end", "world"),
            Some("**world**".to_string())
        );
    }

    #[test]
    fn test_selection_bold_partial() {
        // Selecting part of a bold word gives just the selected characters
        assert_eq!(
            extract_source_selection("hello **world** end", "orl"),
            Some("orl".to_string())
        );
    }

    #[test]
    fn test_selection_across_formatting() {
        // Selection spanning plain → bold → plain includes the markers
        assert_eq!(
            extract_source_selection("hello **world** end", "lo world e"),
            Some("lo **world** e".to_string())
        );
    }

    #[test]
    fn test_selection_italic() {
        assert_eq!(
            extract_source_selection("hello *italic* end", "italic"),
            Some("*italic*".to_string())
        );
    }

    #[test]
    fn test_selection_inline_code() {
        assert_eq!(
            extract_source_selection("use `println!` here", "println!"),
            Some("`println!`".to_string())
        );
    }

    #[test]
    fn test_selection_link_text() {
        assert_eq!(
            extract_source_selection("click [here](http://example.com) now", "here"),
            Some("[here](http://example.com)".to_string())
        );
    }

    #[test]
    fn test_selection_entire_line() {
        // Selecting the full rendered text gives the full source
        assert_eq!(
            extract_source_selection("hello **world** end", "hello world end"),
            Some("hello **world** end".to_string())
        );
    }

    #[test]
    fn test_selection_empty() {
        assert_eq!(extract_source_selection("hello", ""), None);
    }

    #[test]
    fn test_selection_not_found() {
        assert_eq!(extract_source_selection("hello world", "xyz"), None);
    }

    #[test]
    fn test_selection_empty_source() {
        assert_eq!(extract_source_selection("", "hello"), None);
    }

    #[test]
    fn test_selection_strikethrough() {
        assert_eq!(
            extract_source_selection("old ~~removed~~ text", "removed"),
            Some("~~removed~~".to_string())
        );
    }

    #[test]
    fn test_selection_nested_formatting() {
        // Bold containing italic: **bold *and italic***
        assert_eq!(
            extract_source_selection("text **bold *italic*** end", "bold italic"),
            Some("**bold *italic***".to_string())
        );
    }
}
