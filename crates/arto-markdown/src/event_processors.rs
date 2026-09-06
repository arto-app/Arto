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
