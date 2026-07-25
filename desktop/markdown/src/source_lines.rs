use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Tag};
use std::ops::Range;

/// Convert a byte offset in text to a 1-based line number
pub(super) fn byte_offset_to_line(text: &str, offset: usize) -> usize {
    let clamped = clamp_to_char_boundary(text, offset);
    text[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Clamp byte offset to a valid UTF-8 char boundary at or before the offset.
fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut clamped = offset.min(text.len());
    while clamped > 0 && !text.is_char_boundary(clamped) {
        clamped -= 1;
    }
    clamped
}

/// Core implementation: replace block-level Start events with Html events that include
/// data-source-line attributes, using a caller-provided function to compute line numbers.
///
/// End events are independent in pulldown-cmark's push_html (they just write closing tags),
/// so replacing only Start events is safe.
///
/// Table: Start(Tag::Table) is left untouched so that push_html preserves column alignment
/// (text-align styles on cells).  Table source lines are injected by lol_html post-processing
/// via `extract_table_source_lines`.  Start(Tag::TableRow) is replaced to inject data-source-line.
/// TableHead is kept as-is because it sets table_state = Head (needed for th vs td).
/// TableCell is kept as-is because it uses table_state for element selection.
///
/// For code blocks, `<pre>` receives `data-source-line-start="N"` indicating where
/// the code content begins.  The frontend counts newlines from there for per-line tracking.
pub(super) fn inject_source_lines_impl<'a, F>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)> + 'a,
    line_fn: F,
) -> impl Iterator<Item = Event<'a>> + 'a
where
    F: Fn(usize) -> usize + 'a,
{
    parser.map(move |(event, range)| {
        let line = || line_fn(range.start);
        let line_end = || line_fn(range.end.saturating_sub(1).max(range.start));

        match event {
            Event::Start(Tag::Paragraph) => {
                Event::Html(format!("<p data-source-line=\"{}\">", line()).into())
            }
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                let mut html = format!("<{} data-source-line=\"{}\"", level, line());
                if let Some(id) = id {
                    html.push_str(&format!(
                        " id=\"{}\"",
                        html_escape::encode_double_quoted_attribute(&id)
                    ));
                }
                if !classes.is_empty() {
                    let class_str: String = classes
                        .iter()
                        .map(|c| html_escape::encode_text(c).to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    html.push_str(&format!(" class=\"{}\"", class_str));
                }
                for (attr, value) in &attrs {
                    match value {
                        Some(val) => html.push_str(&format!(
                            " {}=\"{}\"",
                            html_escape::encode_text(attr),
                            html_escape::encode_double_quoted_attribute(val)
                        )),
                        None => html.push_str(&format!(" {}=\"\"", html_escape::encode_text(attr))),
                    }
                }
                html.push('>');
                Event::Html(html.into())
            }
            Event::Start(Tag::CodeBlock(ref kind)) => {
                let block_line = line();
                // Fenced blocks: content starts on the line after the fence
                // Indented blocks: content starts on the same line
                let content_start = match kind {
                    CodeBlockKind::Fenced(_) => block_line + 1,
                    CodeBlockKind::Indented => block_line,
                };
                let lang_class = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => format!(
                        " class=\"language-{}\"",
                        html_escape::encode_double_quoted_attribute(lang)
                    ),
                    _ => String::new(),
                };
                Event::Html(
                    format!(
                        "<pre data-source-line=\"{}\" data-source-line-end=\"{}\" data-source-line-start=\"{}\"><code{}>",
                        block_line, line_end(), content_start, lang_class
                    )
                    .into(),
                )
            }
            Event::Start(Tag::BlockQuote(kind)) => {
                let class_attr = match &kind {
                    Some(bqk) => {
                        let class = match bqk {
                            BlockQuoteKind::Note => "markdown-alert-note",
                            BlockQuoteKind::Tip => "markdown-alert-tip",
                            BlockQuoteKind::Important => "markdown-alert-important",
                            BlockQuoteKind::Warning => "markdown-alert-warning",
                            BlockQuoteKind::Caution => "markdown-alert-caution",
                        };
                        format!(" class=\"{}\"", class)
                    }
                    None => String::new(),
                };
                Event::Html(
                    format!(
                        "<blockquote data-source-line=\"{}\"{}>\n",
                        line(),
                        class_attr
                    )
                    .into(),
                )
            }
            Event::Start(Tag::List(start)) => match start {
                Some(1) => Event::Html(
                    format!(
                        "<ol data-source-line=\"{}\" data-source-line-end=\"{}\">\n",
                        line(),
                        line_end()
                    )
                    .into(),
                ),
                Some(n) => Event::Html(
                    format!(
                        "<ol start=\"{}\" data-source-line=\"{}\" data-source-line-end=\"{}\">\n",
                        n,
                        line(),
                        line_end()
                    )
                    .into(),
                ),
                None => Event::Html(
                    format!(
                        "<ul data-source-line=\"{}\" data-source-line-end=\"{}\">\n",
                        line(),
                        line_end()
                    )
                    .into(),
                ),
            },
            Event::Start(Tag::Item) => {
                Event::Html(
                    format!(
                        "<li data-source-line=\"{}\" data-source-line-end=\"{}\">",
                        line(),
                        line_end()
                    )
                    .into(),
                )
            }
            Event::Rule => Event::Html(format!("<hr data-source-line=\"{}\" />\n", line()).into()),
            // Preprocessed code blocks (mermaid, math): inject source line range
            Event::Html(ref html) if html.starts_with("<pre class=\"preprocessed-") => {
                let (s, e) = (line(), line_end());
                Event::Html(
                    html.replacen(
                        "<pre ",
                        &format!("<pre data-source-line=\"{s}\" data-source-line-end=\"{e}\" "),
                        1,
                    )
                    .into(),
                )
            }
            // Preprocessed display math ($$...$$): inject source line range
            Event::Html(ref html)
                if html.starts_with("<div class=\"preprocessed-math-display\"") =>
            {
                let (s, e) = (line(), line_end());
                Event::Html(
                    html.replacen(
                        "<div ",
                        &format!("<div data-source-line=\"{s}\" data-source-line-end=\"{e}\" "),
                        1,
                    )
                    .into(),
                )
            }
            Event::Start(Tag::TableRow) => {
                Event::Html(format!("<tr data-source-line=\"{}\">", line()).into())
            }
            // All other events pass through unchanged (inline elements, table internals, etc.)
            other => other,
        }
    })
}

/// Replace block-level Start events with Html events that include data-source-line attributes.
///
/// Uses `line_origins` to map byte offsets in `processed_markdown` back to original source lines.
/// This is necessary because `process_github_alerts` may change line counts.
pub(super) fn inject_source_lines<'a>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)> + 'a,
    processed_markdown: &'a str,
    line_origins: &'a [usize],
    frontmatter_lines: usize,
) -> impl Iterator<Item = Event<'a>> + 'a {
    inject_source_lines_impl(parser, move |byte_offset| {
        let processed_line = byte_offset_to_line(processed_markdown, byte_offset) - 1; // 0-based
        let original_line = line_origins
            .get(processed_line)
            .copied()
            .unwrap_or(processed_line);
        original_line + 1 + frontmatter_lines // 1-based
    })
}

/// Extract source-line ranges for table elements before `inject_source_lines` consumes
/// the byte-offset ranges.  Returns `(start_line, end_line)` pairs in document order.
///
/// These are later applied to `<table>` elements by lol_html post-processing so that
/// `push_html` can handle `Start(Table)` natively and preserve column alignment styles.
pub(super) fn extract_table_source_lines(
    events: &[(Event<'_>, Range<usize>)],
    processed_markdown: &str,
    line_origins: &[usize],
    frontmatter_lines: usize,
) -> Vec<(usize, usize)> {
    let line_fn = |byte_offset: usize| -> usize {
        let processed_line = byte_offset_to_line(processed_markdown, byte_offset) - 1;
        let original_line = line_origins
            .get(processed_line)
            .copied()
            .unwrap_or(processed_line);
        original_line + 1 + frontmatter_lines
    };
    events
        .iter()
        .filter_map(|(event, range)| {
            if matches!(event, Event::Start(Tag::Table(_))) {
                let start = line_fn(range.start);
                let end = line_fn(range.end.saturating_sub(1).max(range.start));
                Some((start, end))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{html, Options, Parser};

    #[test]
    fn test_byte_offset_to_line() {
        assert_eq!(byte_offset_to_line("hello", 0), 1);
        assert_eq!(byte_offset_to_line("hello\nworld", 0), 1);
        assert_eq!(byte_offset_to_line("hello\nworld", 6), 2);
        assert_eq!(byte_offset_to_line("hello\nworld", 5), 1);
        assert_eq!(byte_offset_to_line("a\nb\nc\n", 0), 1);
        assert_eq!(byte_offset_to_line("a\nb\nc\n", 2), 2);
        assert_eq!(byte_offset_to_line("a\nb\nc\n", 4), 3);
        // Offset beyond text length is clamped
        assert_eq!(byte_offset_to_line("hi", 100), 1);
    }

    #[test]
    fn test_byte_offset_to_line_mid_char_boundary_is_safe() {
        let text = "a\n盤\nc";
        let mid_char = 3; // inside '盤' (bytes are 2..5)
        assert_eq!(byte_offset_to_line(text, mid_char), 2);
    }

    // Helper: render markdown through inject_source_lines_impl with identity line mapping
    fn render_with_source_lines(markdown: &str) -> String {
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();
        let events = inject_source_lines_impl(parser, |byte_offset| {
            byte_offset_to_line(markdown, byte_offset)
        });
        let mut html_output = String::new();
        html::push_html(&mut html_output, events);
        html_output
    }

    #[test]
    fn test_inject_paragraph() {
        let result = render_with_source_lines("Hello world");
        assert!(
            result.contains(r#"<p data-source-line="1">"#),
            "Paragraph should have data-source-line: {result}"
        );
    }

    #[test]
    fn test_inject_heading_with_attrs() {
        // pulldown-cmark supports heading attributes via {#id .class}
        let result = render_with_source_lines("# Title {#my-id .my-class}");
        assert!(
            result.contains("data-source-line=\"1\""),
            "Heading should have data-source-line: {result}"
        );
        assert!(
            result.contains("id=\"my-id\""),
            "Heading should preserve id: {result}"
        );
        assert!(
            result.contains("class=\"my-class\""),
            "Heading should preserve class: {result}"
        );
    }

    #[test]
    fn test_inject_code_block_fenced() {
        let md = "```rust\nfn main() {}\n```";
        let result = render_with_source_lines(md);
        assert!(
            result.contains(r#"data-source-line="1""#),
            "Code block should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="3""#),
            "Code block should end on line 3: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-start="2""#),
            "Fenced content should start at line 2: {result}"
        );
    }

    #[test]
    fn test_inject_code_block_indented() {
        // Indented code block (4 spaces)
        let md = "    fn main() {}\n    let x = 1;";
        let result = render_with_source_lines(md);
        assert!(
            result.contains(r#"data-source-line-end="2""#),
            "Indented code block should end on line 2: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-start="1""#),
            "Indented content should start at same line: {result}"
        );
    }

    #[test]
    fn test_inject_blockquote_with_kind() {
        // pulldown-cmark GFM alert syntax
        let md = "> [!NOTE]\n> This is a note";
        let options = Options::all();
        let parser = Parser::new_ext(md, options).into_offset_iter();

        // Check that we get a BlockQuote event with kind
        let events: Vec<_> = parser.collect();
        let has_blockquote_kind = events
            .iter()
            .any(|(e, _)| matches!(e, Event::Start(Tag::BlockQuote(Some(_)))));

        if has_blockquote_kind {
            let result = render_with_source_lines(md);
            assert!(
                result.contains("data-source-line=\"1\""),
                "Blockquote should have data-source-line: {result}"
            );
            assert!(
                result.contains("class=\"markdown-alert-"),
                "Alert blockquote should have alert class: {result}"
            );
        }
    }

    #[test]
    fn test_inject_list_ordered_unordered() {
        let md = "- a\n- b\n\n1. x\n2. y";
        let result = render_with_source_lines(md);
        assert!(
            result.contains(r#"<ul data-source-line="1""#),
            "Unordered list should have source line: {result}"
        );
        assert!(
            result.contains(r#"<li data-source-line="1""#),
            "First ul item should have source line: {result}"
        );
        assert!(
            result.contains(r#"<ol data-source-line="4""#),
            "Ordered list should have source line: {result}"
        );
    }

    #[test]
    fn test_inject_table_and_row() {
        use super::super::event_processors::extend_table_ranges;

        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let options = Options::all();
        let parser = Parser::new_ext(md, options).into_offset_iter();
        let parser = extend_table_ranges(parser);
        let events =
            inject_source_lines_impl(parser, |byte_offset| byte_offset_to_line(md, byte_offset));
        let mut html_output = String::new();
        html::push_html(&mut html_output, events);

        // Table element itself is NOT annotated here (handled by lol_html post-processing)
        // but TableRow IS annotated directly
        assert!(
            html_output.contains("<table>"),
            "Table should pass through for push_html alignment handling: {html_output}"
        );
        assert!(
            html_output.contains(r#"<tr data-source-line="#),
            "Table rows should have source line: {html_output}"
        );
    }

    #[test]
    fn test_extract_table_source_lines() {
        use super::super::event_processors::extend_table_ranges;

        let md = "| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |";
        let options = Options::all();
        let parser = Parser::new_ext(md, options).into_offset_iter();
        let events: Vec<_> = extend_table_ranges(parser).collect();

        let line_origins: Vec<usize> = (0..md.lines().count()).collect();
        let table_lines = extract_table_source_lines(&events, md, &line_origins, 0);

        assert_eq!(table_lines.len(), 2, "Should find two tables");
        assert_eq!(table_lines[0].0, 1, "First table starts on line 1");
        assert_eq!(table_lines[0].1, 3, "First table ends on line 3");
        assert_eq!(table_lines[1].0, 5, "Second table starts on line 5");
        assert_eq!(table_lines[1].1, 7, "Second table ends on line 7");
    }

    #[test]
    fn test_inject_rule() {
        let md = "Above\n\n---\n\nBelow";
        let result = render_with_source_lines(md);
        assert!(
            result.contains(r#"<hr data-source-line="3" />"#),
            "HR should have data-source-line: {result}"
        );
    }

    #[test]
    fn test_inject_preprocessed_mermaid() {
        // Simulate what process_code_blocks produces
        let html_str =
            r#"<pre class="preprocessed-mermaid" data-original-content="graph TD">graph TD</pre>"#;
        let events = vec![(Event::Html(html_str.into()), 10..50)];
        let result: Vec<Event> = inject_source_lines_impl(events.into_iter(), |byte_offset| {
            // Simple identity for test: pretend offset 10 = line 3, offset 49 = line 5
            if byte_offset <= 10 {
                3
            } else {
                5
            }
        })
        .collect();

        if let Event::Html(html) = &result[0] {
            assert!(
                html.contains(r#"data-source-line="3""#),
                "Should inject source line: {html}"
            );
            assert!(
                html.contains(r#"data-source-line-end="5""#),
                "Should inject source line end: {html}"
            );
        } else {
            panic!("Expected Html event");
        }
    }

    #[test]
    fn test_inject_preprocessed_math_display() {
        let html_str =
            r#"<div class="preprocessed-math-display" data-original-content="x=1">x=1</div>"#;
        let events = vec![(Event::Html(html_str.into()), 20..60)];
        let result: Vec<Event> =
            inject_source_lines_impl(
                events.into_iter(),
                |byte_offset| {
                    if byte_offset <= 20 {
                        4
                    } else {
                        6
                    }
                },
            )
            .collect();

        if let Event::Html(html) = &result[0] {
            assert!(
                html.contains(r#"data-source-line="4""#),
                "Should inject source line: {html}"
            );
            assert!(
                html.contains(r#"data-source-line-end="6""#),
                "Should inject source line end: {html}"
            );
        } else {
            panic!("Expected Html event");
        }
    }

    #[test]
    fn test_inject_passthrough() {
        // Inline elements should pass through unchanged
        let md = "Hello **bold** world";
        let options = Options::all();
        let parser = Parser::new_ext(md, options).into_offset_iter();
        let events: Vec<Event> =
            inject_source_lines_impl(parser, |byte_offset| byte_offset_to_line(md, byte_offset))
                .collect();

        // Should still contain Text events for inline content
        let has_text = events
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t.as_ref() == "bold"));
        assert!(has_text, "Inline text should pass through unchanged");
    }

    #[test]
    fn test_inject_source_lines_with_line_origins() {
        // Test the inject_source_lines wrapper that maps via line_origins.
        // Simulate: an alert on original line 3 expanded into 3 processed lines.
        //
        // Processed text (5 lines, 0-based):
        //   0: "# Title"
        //   1: ""                ← expanded from original line 3
        //   2: "Paragraph A"    ← expanded from original line 3
        //   3: ""                ← expanded from original line 3
        //   4: "Paragraph B"    ← from original line 5
        let processed = "# Title\n\nParagraph A\n\nParagraph B";
        let line_origins = vec![0, 3, 3, 3, 5];
        let frontmatter_lines = 2;

        let options = Options::all();
        let parser = Parser::new_ext(processed, options).into_offset_iter();
        let events: Vec<Event> =
            inject_source_lines(parser, processed, &line_origins, frontmatter_lines).collect();

        let mut output = String::new();
        html::push_html(&mut output, events.into_iter());

        // "# Title": processed line 0 → line_origins[0]=0 → 0+1+2 = line 3
        assert!(
            output.contains(r#"data-source-line="3""#),
            "Title should map to line 3: {output}"
        );
        // "Paragraph A": processed line 2 → line_origins[2]=3 → 3+1+2 = line 6
        assert!(
            output.contains(r#"data-source-line="6""#),
            "Paragraph A should map to line 6: {output}"
        );
        // "Paragraph B": processed line 4 → line_origins[4]=5 → 5+1+2 = line 8
        assert!(
            output.contains(r#"data-source-line="8""#),
            "Paragraph B should map to line 8: {output}"
        );
    }
}
