//! Mapping rendered text back to the Markdown source that produced it.
//!
//! When the user selects text in the rendered document, the app wants the
//! corresponding Markdown, including the inline formatting markers that the
//! rendering stripped. That requires parsing the source again with the same
//! engine that rendered it, which is why this lives next to the pipeline
//! rather than in the app.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, TagEnd};

/// A segment mapping between rendered plain text and markdown source byte positions.
struct TextSegment {
    rendered: Range<usize>,
    source: Range<usize>,
}

/// Build a mapping from rendered plain text to markdown source positions.
///
/// Parses the markdown source, concatenating all visible text events into a
/// "rendered" string while recording which source byte range each rendered
/// segment came from.
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
pub fn extract_source_selection(
    source: impl AsRef<str>,
    selected_text: impl AsRef<str>,
) -> Option<String> {
    let source = source.as_ref();
    let selected_text = selected_text.as_ref();
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
