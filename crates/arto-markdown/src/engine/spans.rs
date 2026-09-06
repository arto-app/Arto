//! Mark block elements with the byte range they came from.
//!
//! pulldown-cmark's HTML writer has no hook for attributes, so the start
//! event of each block element is replaced by an `Html` event that writes
//! the tag itself with `data-source-span="start-end"`. The annotation pass
//! outside the engine turns those spans into the line attributes the
//! frontend reads.
//!
//! End events are independent in `push_html` (they just write closing
//! tags), so replacing only start events is safe. Two elements are left to
//! `push_html`: `Start(Table)`, so that the writer keeps the column
//! alignments it needs for the cells (the table gets its span through a
//! marker comment that [`super::attach_table_spans`] folds into the tag),
//! and `TableHead` / `TableCell`, which depend on the writer's table state.

use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Tag};
use std::ops::Range;

/// Marker written right before a `<table>`; see [`super::attach_table_spans`].
pub(super) const TABLE_MARKER: &str = "<!--arto-table ";

/// Replace block-level start events with `Html` events carrying
/// `data-source-span`.
///
/// `heading_ids` are written onto the headings in document order and take
/// precedence over an explicit `{#id}` attribute; pass an empty list to
/// leave headings without generated ids.
pub(super) fn inject_source_spans<'a>(
    parser: impl Iterator<Item = (Event<'a>, Range<usize>)> + 'a,
    heading_ids: Vec<String>,
) -> impl Iterator<Item = Event<'a>> + 'a {
    let mut heading_ids = heading_ids.into_iter();
    parser.flat_map(move |(event, range)| {
        let span = format!("data-source-span=\"{}-{}\"", range.start, range.end);

        match event {
            Event::Start(Tag::Paragraph) => vec![Event::Html(format!("<p {span}>").into())],
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                let mut html = format!("<{level} {span}");
                let id = heading_ids
                    .next()
                    .or_else(|| id.map(|explicit| explicit.to_string()));
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
                vec![Event::Html(html.into())]
            }
            Event::Start(Tag::CodeBlock(ref kind)) => {
                let lang_class = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => format!(
                        " class=\"language-{}\"",
                        html_escape::encode_double_quoted_attribute(lang)
                    ),
                    _ => String::new(),
                };
                vec![Event::Html(
                    format!("<pre {span}><code{lang_class}>").into(),
                )]
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
                vec![Event::Html(
                    format!("<blockquote {span}{class_attr}>\n").into(),
                )]
            }
            Event::Start(Tag::List(start)) => vec![Event::Html(
                match start {
                    Some(1) => format!("<ol {span}>\n"),
                    Some(n) => format!("<ol start=\"{n}\" {span}>\n"),
                    None => format!("<ul {span}>\n"),
                }
                .into(),
            )],
            Event::Start(Tag::Item) => vec![Event::Html(format!("<li {span}>").into())],
            Event::Rule => vec![Event::Html(format!("<hr {span} />\n").into())],
            // Mermaid / math containers built by the event processors
            Event::Html(ref html) if html.starts_with("<pre class=\"preprocessed-") => {
                vec![Event::Html(
                    html.replacen("<pre ", &format!("<pre {span} "), 1).into(),
                )]
            }
            Event::Html(ref html)
                if html.starts_with("<div class=\"preprocessed-math-display\"") =>
            {
                vec![Event::Html(
                    html.replacen("<div ", &format!("<div {span} "), 1).into(),
                )]
            }
            Event::Start(Tag::Table(_)) => vec![
                Event::Html(format!("{TABLE_MARKER}{}-{}-->", range.start, range.end).into()),
                event,
            ],
            Event::Start(Tag::TableRow) => vec![Event::Html(format!("<tr {span}>").into())],
            // All other events pass through unchanged (inline elements, table internals, etc.)
            other => vec![other],
        }
    })
}
