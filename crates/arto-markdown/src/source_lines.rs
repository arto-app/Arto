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
}
