use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use std::ops::Range;

/// Extend table Start events' ranges to cover the full table (start to end).
/// This enables inject_source_lines_impl to compute both data-source-line and
/// data-source-line-end for the <table> element.
///
/// Buffers events from Start(Table) to End(Table), then re-emits them all
/// with the Start event's range extended to `start..end_of_table`.
pub(super) fn extend_table_ranges<'a>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> impl Iterator<Item = (Event<'a>, Range<usize>)> {
    let mut in_table = false;
    let mut buffered: Vec<(Event<'a>, Range<usize>)> = Vec::new();

    parser.flat_map(move |item| {
        match &item.0 {
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                buffered.clear();
                buffered.push(item);
                vec![]
            }
            Event::End(TagEnd::Table) if in_table => {
                in_table = false;
                let end_offset = item.1.end;
                // Extend the Start(Table) range to cover the full table
                if let Some(first) = buffered.first_mut() {
                    first.1 = first.1.start..end_offset;
                }
                buffered.push(item);
                std::mem::take(&mut buffered)
            }
            _ if in_table => {
                buffered.push(item);
                vec![]
            }
            _ => vec![item],
        }
    })
}

/// Process Code blocks (carries byte offset ranges through for source line annotation)
pub(super) fn process_code_blocks<'a>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)>,
    target_lang: &'a str,
) -> impl Iterator<Item = (Event<'a>, Range<usize>)> {
    let mut in_block = false;
    let mut content = String::new();
    let mut start_range: Range<usize> = 0..0;

    parser.flat_map(move |item| match item {
        (Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))), range)
            if lang.as_ref() == target_lang =>
        {
            in_block = true;
            content.clear();
            start_range = range;
            vec![]
        }
        (Event::End(TagEnd::CodeBlock), end_range) if in_block => {
            in_block = false;
            let full_range = start_range.start..end_range.end;
            // Store original content in data attribute for JavaScript processing
            let html = format!(
                r#"<pre class="preprocessed-{}" data-original-content="{}">{}</pre>"#,
                target_lang,
                html_escape::encode_double_quoted_attribute(&content),
                html_escape::encode_text(&content),
            );
            vec![(Event::Html(html.into()), full_range)]
        }
        (Event::Text(text), _) if in_block => {
            content.push_str(&text);
            vec![]
        }
        other => vec![other],
    })
}

/// Process math expressions (inline and display, carries byte offset ranges through)
pub(super) fn process_math_expressions<'a>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> impl Iterator<Item = (Event<'a>, Range<usize>)> {
    parser.map(|item| match item {
        (Event::InlineMath(content), range) => {
            // Convert inline math to custom HTML structure
            let html = format!(
                r#"<span class="preprocessed-math-inline" data-original-content="{}">{}</span>"#,
                html_escape::encode_text(&content),
                html_escape::encode_text(&content),
            );
            (Event::Html(html.into()), range)
        }
        (Event::DisplayMath(content), range) => {
            // Convert display math to custom HTML structure
            let html = format!(
                r#"<div class="preprocessed-math-display" data-original-content="{}">{}</div>"#,
                html_escape::encode_text(&content),
                html_escape::encode_text(&content),
            );
            (Event::Html(html.into()), range)
        }
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pulldown_cmark::{Options, Parser};

    #[test]
    fn test_process_mermaid_blocks() {
        let markdown = indoc! {"
            ```mermaid
            graph TD
                A-->B
            ```
        "};

        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();
        let events: Vec<(Event, Range<usize>)> = process_code_blocks(parser, "mermaid").collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        assert!(!html_events.is_empty(), "Should contain HTML event");
        let html = html_events[0];
        assert!(html.contains(r#"<pre class="preprocessed-mermaid""#));
        assert!(html.contains(r#"data-original-content="#));
        assert!(html.contains("</pre>"));
    }

    #[test]
    fn test_process_math_expressions_inline() {
        let markdown = "This is inline math: $x = y + z$";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<(Event, Range<usize>)> = process_math_expressions(parser).collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            html_events
                .iter()
                .any(|h| h.contains(r#"<span class="preprocessed-math-inline""#)),
            "Should contain inline-math span"
        );
        assert!(
            html_events
                .iter()
                .any(|h| h.contains("data-original-content")),
            "Should contain data attribute"
        );
        assert!(
            html_events.iter().any(|h| h.contains("x = y + z")),
            "Should contain the math content"
        );
    }

    #[test]
    fn test_process_math_expressions_display() {
        let markdown = indoc! {"
            Display math:

            $$
            x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}
            $$
        "};
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<(Event, Range<usize>)> = process_math_expressions(parser).collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            html_events
                .iter()
                .any(|h| h.contains(r#"<div class="preprocessed-math-display""#)),
            "Should contain display-math div"
        );
        assert!(
            html_events
                .iter()
                .any(|h| h.contains("data-original-content")),
            "Should contain data attribute"
        );
        assert!(
            html_events.iter().any(|h| h.contains("frac")),
            "Should contain the math content"
        );
    }

    #[test]
    fn test_process_math_expressions_mixed() {
        let markdown = "Inline $a + b$ and display $$c = d$$";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<(Event, Range<usize>)> = process_math_expressions(parser).collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            html_events
                .iter()
                .any(|h| h.contains(r#"class="preprocessed-math-inline""#)),
            "Should contain inline math"
        );
        assert!(
            html_events
                .iter()
                .any(|h| h.contains(r#"class="preprocessed-math-display""#)),
            "Should contain display math"
        );
    }

    #[test]
    fn test_extend_table_ranges_no_table() {
        let markdown = "Just a paragraph\n\nAnother one";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let original: Vec<_> = Parser::new_ext(markdown, options)
            .into_offset_iter()
            .collect();
        let extended: Vec<_> = extend_table_ranges(parser).collect();

        // Without tables, events should pass through with same ranges
        assert_eq!(original.len(), extended.len());
        for (orig, ext) in original.iter().zip(extended.iter()) {
            assert_eq!(orig.1, ext.1, "Ranges should be identical without tables");
        }
    }

    #[test]
    fn test_extend_table_ranges_single_table() {
        let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = extend_table_ranges(parser).collect();

        // Find the Start(Table) event and End(Table) event
        let table_start = events
            .iter()
            .find(|(e, _)| matches!(e, Event::Start(Tag::Table(_))));
        let table_end = events
            .iter()
            .find(|(e, _)| matches!(e, Event::End(TagEnd::Table)));

        assert!(table_start.is_some(), "Should have table start");
        assert!(table_end.is_some(), "Should have table end");

        let start_range = &table_start.unwrap().1;
        let end_range = &table_end.unwrap().1;

        // Start(Table) range should extend to the end of the table
        assert_eq!(
            start_range.end, end_range.end,
            "Start(Table) range end should match End(Table) range end"
        );
    }

    #[test]
    fn test_extend_table_ranges_multiple_tables() {
        let markdown = "| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = extend_table_ranges(parser).collect();

        // Collect all Start(Table) events
        let table_starts: Vec<_> = events
            .iter()
            .filter(|(e, _)| matches!(e, Event::Start(Tag::Table(_))))
            .collect();

        assert_eq!(table_starts.len(), 2, "Should have two tables");

        // Each table's Start range should be independent
        assert_ne!(
            table_starts[0].1, table_starts[1].1,
            "Tables should have different ranges"
        );
    }

    #[test]
    fn test_extend_table_ranges_header_only_table() {
        let markdown = "| A | B |\n|---|---|";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = extend_table_ranges(parser).collect();

        let table_start = events
            .iter()
            .find(|(e, _)| matches!(e, Event::Start(Tag::Table(_))));

        assert!(
            table_start.is_some(),
            "Should have table start even with header only"
        );
    }

    #[test]
    fn test_process_code_blocks_empty() {
        let markdown = "```mermaid\n```";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = process_code_blocks(parser, "mermaid").collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            !html_events.is_empty(),
            "Should produce HTML for empty block"
        );
        assert!(
            html_events[0].contains(r#"class="preprocessed-mermaid""#),
            "Should still have mermaid class"
        );
    }

    #[test]
    fn test_process_code_blocks_non_matching_lang() {
        let markdown = "```python\nprint('hello')\n```";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = process_code_blocks(parser, "mermaid").collect();

        // Should NOT have preprocessed HTML — the python block passes through
        let has_preprocessed = events.iter().any(|(e, _)| {
            if let Event::Html(html) = e {
                html.contains("preprocessed-mermaid")
            } else {
                false
            }
        });
        assert!(
            !has_preprocessed,
            "Non-matching language should pass through"
        );

        // Should still have CodeBlock events
        let has_code_block = events
            .iter()
            .any(|(e, _)| matches!(e, Event::Start(Tag::CodeBlock(_))));
        assert!(has_code_block, "Code block events should remain");
    }

    #[test]
    fn test_process_code_blocks_range_covers_fences() {
        let markdown = "```mermaid\ngraph TD\n```";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = process_code_blocks(parser, "mermaid").collect();

        let html_event = events.iter().find(|(e, _)| matches!(e, Event::Html(_)));

        assert!(html_event.is_some(), "Should have HTML event");
        let range = &html_event.unwrap().1;

        // Range should start at 0 (beginning of fenced block) and extend to end
        assert_eq!(range.start, 0, "Range should start at beginning of fence");
        assert!(
            range.end >= markdown.len() - 1,
            "Range should cover to end of closing fence"
        );
    }

    #[test]
    fn test_process_math_empty_display() {
        let markdown = "$$$$";
        let options = Options::all();
        let parser = Parser::new_ext(markdown, options).into_offset_iter();

        let events: Vec<_> = process_math_expressions(parser).collect();

        let html_events: Vec<_> = events
            .iter()
            .filter_map(|(e, _)| {
                if let Event::Html(html) = e {
                    Some(html.as_ref())
                } else {
                    None
                }
            })
            .collect();

        // pulldown-cmark may or may not produce a DisplayMath event for $$$$
        // If it does, it should be converted correctly
        if !html_events.is_empty() {
            assert!(
                html_events[0].contains("preprocessed-math-display"),
                "Empty display math should still be processed"
            );
        }
    }
}
