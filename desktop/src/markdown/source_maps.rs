use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// A segment mapping source text characters to content text characters.
///
/// Within a block element, source text contains markup (`**`, `*`, `` ` ``, `[](url)`, etc.)
/// that doesn't appear in the rendered content. This segment records where visible text
/// appears in the source, allowing conversion from source character indices to content
/// character indices.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceMapSegment {
    /// Character offset in the source line where this text segment starts
    pub source_offset: usize,
    /// Number of characters in this segment
    pub char_count: usize,
}

/// Build source map segments for a markdown line by parsing its inline structure.
///
/// Parses the line with pulldown-cmark to discover inline markup and returns segments
/// that map source text positions to rendered content positions. Content character
/// offsets are computed by accumulating segment `char_count` values in order.
///
/// Handles: `**bold**`, `*italic*`, `` `code` ``, `[link](url)`, `~~strikethrough~~`
///
/// Known limitation: escaped characters (`\*`, `\_`, etc.) may cause slight offset
/// misalignment within the affected Text segment.
pub(crate) fn build_line_source_map(line: &str) -> Vec<SourceMapSegment> {
    let options = Options::all();
    let parser = Parser::new_ext(line, options).into_offset_iter();
    let mut segments = Vec::new();
    let mut inside_image = false;

    for (event, range) in parser {
        match event {
            // Skip text inside images (alt text is not visible content)
            Event::Start(Tag::Image { .. }) => {
                inside_image = true;
            }
            Event::End(TagEnd::Image) => {
                inside_image = false;
            }

            Event::Text(ref text) if !inside_image => {
                let source_offset = byte_to_char_offset(line, range.start);
                let char_count = text.chars().count();
                segments.push(SourceMapSegment {
                    source_offset,
                    char_count,
                });
            }

            Event::Code(ref text) if !inside_image => {
                let text_byte_start = find_code_text_start(line, &range);
                let source_offset = byte_to_char_offset(line, text_byte_start);
                let char_count = text.chars().count();
                segments.push(SourceMapSegment {
                    source_offset,
                    char_count,
                });
            }

            _ => {}
        }
    }

    segments
}

/// Convert source character indices to content character indices using a source map.
///
/// Each source index is looked up in the segments to find which content position it
/// corresponds to. Source indices that fall within markup characters (not mapped to
/// any segment) are silently dropped.
///
/// Content offsets are computed by accumulating `char_count` values of preceding segments.
pub(crate) fn convert_source_to_content_indices(
    segments: &[SourceMapSegment],
    source_indices: &[u32],
) -> Vec<u32> {
    source_indices
        .iter()
        .filter_map(|&source_idx| {
            let idx = source_idx as usize;
            let mut content_offset = 0;

            for seg in segments {
                let seg_end = seg.source_offset + seg.char_count;
                if idx >= seg.source_offset && idx < seg_end {
                    return Some((content_offset + (idx - seg.source_offset)) as u32);
                }
                content_offset += seg.char_count;
            }

            None
        })
        .collect()
}

/// Convert a byte offset to a character offset within a string.
fn byte_to_char_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}

/// Find the byte offset where the actual code text starts within a backtick-enclosed span.
///
/// For single backtick `` `code` ``: text starts after the opening backtick.
/// For double backtick ``` ``code`` ```: text starts after `` `` `` plus one stripped space.
fn find_code_text_start(line: &str, range: &Range<usize>) -> usize {
    let src_bytes = &line.as_bytes()[range.start..range.end];
    let backtick_count = src_bytes.iter().take_while(|&&b| b == b'`').count();
    // Double-backtick (or more) syntax strips one leading space
    let space_after = if backtick_count > 1 && src_bytes.get(backtick_count) == Some(&b' ') {
        1
    } else {
        0
    };
    range.start + backtick_count + space_after
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_line_source_map tests ────────────────────────

    #[test]
    fn test_plain_text() {
        let segments = build_line_source_map("Hello World");
        assert_eq!(
            segments,
            vec![SourceMapSegment {
                source_offset: 0,
                char_count: 11
            }]
        );
    }

    #[test]
    fn test_bold() {
        // **Hello** World
        // 0123456789...
        let segments = build_line_source_map("**Hello** World");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 5
                }, // "Hello"
                SourceMapSegment {
                    source_offset: 9,
                    char_count: 6
                }, // " World"
            ]
        );
    }

    #[test]
    fn test_italic() {
        // *Hello* World
        let segments = build_line_source_map("*Hello* World");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 1,
                    char_count: 5
                }, // "Hello"
                SourceMapSegment {
                    source_offset: 7,
                    char_count: 6
                }, // " World"
            ]
        );
    }

    #[test]
    fn test_inline_code_single_backtick() {
        // `code` text
        let segments = build_line_source_map("`code` text");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 1,
                    char_count: 4
                }, // "code"
                SourceMapSegment {
                    source_offset: 6,
                    char_count: 5
                }, // " text"
            ]
        );
    }

    #[test]
    fn test_inline_code_double_backtick() {
        // ``code`` text
        let segments = build_line_source_map("``code`` text");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 4
                }, // "code"
                SourceMapSegment {
                    source_offset: 8,
                    char_count: 5
                }, // " text"
            ]
        );
    }

    #[test]
    fn test_inline_code_double_backtick_with_spaces() {
        // `` code `` text — double backtick strips one leading/trailing space
        let segments = build_line_source_map("`` code `` text");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 3,
                    char_count: 4
                }, // "code"
                SourceMapSegment {
                    source_offset: 10,
                    char_count: 5
                }, // " text"
            ]
        );
    }

    #[test]
    fn test_link() {
        // [Hello](url) World
        let segments = build_line_source_map("[Hello](url) World");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 1,
                    char_count: 5
                }, // "Hello"
                SourceMapSegment {
                    source_offset: 12,
                    char_count: 6
                }, // " World"
            ]
        );
    }

    #[test]
    fn test_image_alt_text_excluded() {
        // ![alt](img.png) text — alt text is NOT visible content
        let segments = build_line_source_map("![alt](img.png) text");
        assert_eq!(
            segments,
            vec![SourceMapSegment {
                source_offset: 15,
                char_count: 5
            }] // " text"
        );
    }

    #[test]
    fn test_strikethrough() {
        // ~~deleted~~ text
        let segments = build_line_source_map("~~deleted~~ text");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 7
                }, // "deleted"
                SourceMapSegment {
                    source_offset: 11,
                    char_count: 5
                }, // " text"
            ]
        );
    }

    #[test]
    fn test_mixed_markup() {
        // **bold** and *italic*
        let segments = build_line_source_map("**bold** and *italic*");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 4
                }, // "bold"
                SourceMapSegment {
                    source_offset: 8,
                    char_count: 5
                }, // " and "
                SourceMapSegment {
                    source_offset: 14,
                    char_count: 6
                }, // "italic"
            ]
        );
    }

    #[test]
    fn test_nested_bold_italic() {
        // **bold *and italic* text**
        let segments = build_line_source_map("**bold *and italic* text**");
        // pulldown-cmark produces: Text("bold "), Text("and italic"), Text(" text")
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 5
                }, // "bold "
                SourceMapSegment {
                    source_offset: 8,
                    char_count: 10
                }, // "and italic"
                SourceMapSegment {
                    source_offset: 19,
                    char_count: 5
                }, // " text"
            ]
        );
    }

    #[test]
    fn test_heading_prefix() {
        // # Hello **World**
        let segments = build_line_source_map("# Hello **World**");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 6
                }, // "Hello "
                SourceMapSegment {
                    source_offset: 10,
                    char_count: 5
                }, // "World"
            ]
        );
    }

    #[test]
    fn test_list_item_prefix() {
        // - **bold** item
        let segments = build_line_source_map("- **bold** item");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 4,
                    char_count: 4
                }, // "bold"
                SourceMapSegment {
                    source_offset: 10,
                    char_count: 5
                }, // " item"
            ]
        );
    }

    #[test]
    fn test_japanese_with_markup() {
        // **日本語**のテスト
        let segments = build_line_source_map("**日本語**のテスト");
        assert_eq!(
            segments,
            vec![
                SourceMapSegment {
                    source_offset: 2,
                    char_count: 3
                }, // "日本語"
                SourceMapSegment {
                    source_offset: 7,
                    char_count: 4
                }, // "のテスト"
            ]
        );
    }

    // ── convert_source_to_content_indices tests ───────────

    #[test]
    fn test_convert_plain_text() {
        let segments = vec![SourceMapSegment {
            source_offset: 0,
            char_count: 11,
        }];
        let result = convert_source_to_content_indices(&segments, &[0, 5, 10]);
        assert_eq!(result, vec![0, 5, 10]);
    }

    #[test]
    fn test_convert_bold_text() {
        // **Hello** World → segments: (2, 5), (9, 6)
        // Content: "Hello World"
        let segments = vec![
            SourceMapSegment {
                source_offset: 2,
                char_count: 5,
            },
            SourceMapSegment {
                source_offset: 9,
                char_count: 6,
            },
        ];

        // Source index 2 (H) → content 0
        // Source index 6 (o) → content 4
        // Source index 9 ( ) → content 5
        // Source index 14 (d) → content 10
        let result = convert_source_to_content_indices(&segments, &[2, 6, 9, 14]);
        assert_eq!(result, vec![0, 4, 5, 10]);
    }

    #[test]
    fn test_convert_markup_indices_dropped() {
        // **Hello** World → segments: (2, 5), (9, 6)
        let segments = vec![
            SourceMapSegment {
                source_offset: 2,
                char_count: 5,
            },
            SourceMapSegment {
                source_offset: 9,
                char_count: 6,
            },
        ];

        // Source indices 0, 1 are ** (markup) → dropped
        // Source indices 7, 8 are ** (markup) → dropped
        let result = convert_source_to_content_indices(&segments, &[0, 1, 7, 8]);
        assert_eq!(result, Vec::<u32>::new());
    }

    #[test]
    fn test_convert_heading_prefix_dropped() {
        // # Hello → segments: (2, 5)
        // Content: "Hello"
        let segments = vec![SourceMapSegment {
            source_offset: 2,
            char_count: 5,
        }];

        // Source indices 0 (#), 1 (space) are heading markup → dropped
        // Source index 2 (H) → content 0
        let result = convert_source_to_content_indices(&segments, &[0, 1, 2, 3, 4]);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_convert_empty_segments() {
        let segments: Vec<SourceMapSegment> = vec![];
        let result = convert_source_to_content_indices(&segments, &[0, 1, 2]);
        assert_eq!(result, Vec::<u32>::new());
    }

    #[test]
    fn test_convert_link_indices() {
        // [Hello](url) World → segments: (1, 5), (12, 6)
        // Content: "Hello World"
        let segments = vec![
            SourceMapSegment {
                source_offset: 1,
                char_count: 5,
            },
            SourceMapSegment {
                source_offset: 12,
                char_count: 6,
            },
        ];

        // Source index 0 ([) → dropped
        // Source index 1 (H) → content 0
        // Source indices 6-11 (](url)) → dropped
        // Source index 12 ( ) → content 5
        let result = convert_source_to_content_indices(&segments, &[0, 1, 5, 6, 12]);
        assert_eq!(result, vec![0, 4, 5]);
    }

    #[test]
    fn test_convert_japanese_bold() {
        // **日本語**のテスト → segments: (2, 3), (7, 4)
        // Content: "日本語のテスト"
        let segments = vec![
            SourceMapSegment {
                source_offset: 2,
                char_count: 3,
            },
            SourceMapSegment {
                source_offset: 7,
                char_count: 4,
            },
        ];

        // Source index 2 (日) → content 0
        // Source index 4 (語) → content 2
        // Source index 7 (の) → content 3
        // Source index 10 (ト) → content 6
        let result = convert_source_to_content_indices(&segments, &[2, 4, 7, 10]);
        assert_eq!(result, vec![0, 2, 3, 6]);
    }

    /// Integration test: nucleo match_indices → source map → content indices
    #[test]
    fn test_nucleo_to_content_indices_integration() {
        use nucleo_matcher::{Config, Matcher, Utf32Str};

        let line = "**Hello** World";
        let query = "hw";

        let mut config = Config::DEFAULT;
        config.ignore_case = true;
        let mut matcher = Matcher::new(config);

        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(query, &mut needle_buf);
        let mut haystack_buf = Vec::new();
        let haystack = Utf32Str::new(line, &mut haystack_buf);
        let mut indices = Vec::new();

        let score = matcher.fuzzy_indices(haystack, needle, &mut indices);
        assert!(score.is_some(), "Should match 'hw' in '**Hello** World'");

        // Build source map and convert
        let segments = build_line_source_map(line);
        let content_indices = convert_source_to_content_indices(&segments, &indices);

        // Verify content indices point to 'H' and 'W' in "Hello World"
        let content = "Hello World";
        let content_chars: Vec<char> = content.chars().collect();
        for &ci in &content_indices {
            let ch = content_chars[ci as usize];
            assert!(
                "HWhw".contains(ch),
                "Content index {} points to '{}', expected H or W",
                ci,
                ch
            );
        }
    }
}
