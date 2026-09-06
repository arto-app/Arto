//! The pulldown-cmark rendering engine.
//!
//! Everything that knows the parser lives below this module: the parser
//! configuration, the GitHub alert rewrite (whose bodies are rendered
//! with the parser), the event transforms for Mermaid and math, the
//! source span injection, the heading outline, and the selection source
//! map. The rest of the crate sees [`render`] with its [`Rendered`]
//! output and [`extract_source_selection`], none of which mention parser
//! types, so replacing the engine is a change inside this directory.

mod alerts;
mod event_processors;
mod outline;
mod source_map;
mod spans;

pub use source_map::*;

use crate::lines::LineTable;
use crate::HeadingInfo;
use alerts::process_github_alerts;
use event_processors::{extend_table_ranges, process_code_blocks, process_math_expressions};
use outline::collect_headings;
use pulldown_cmark::{html, Event, Options, Parser};
use spans::{inject_source_spans, TABLE_MARKER};

/// The parser configuration every parse in the engine uses.
///
/// Rendering, alert bodies and the selection source map must parse the
/// same way, or a selection in the rendered view maps onto a differently
/// shaped document.
pub(super) fn parser_options() -> Options {
    Options::all()
}

/// What the engine produces for one document body.
pub(crate) struct Rendered {
    /// HTML of the body. Every block element carries
    /// `data-source-span="start-end"`, byte offsets into the text `lines`
    /// was built over.
    pub html: String,
    /// Headings in document order, with the ids the rendered headings
    /// carry; empty unless a table of contents was requested.
    pub headings: Vec<HeadingInfo>,
    /// Converts the spans in `html` to lines of the original file.
    pub lines: LineTable,
}

/// Render a document body (the Markdown after the frontmatter was cut off
/// and bare URLs were turned into autolinks).
///
/// `frontmatter_lines` is the offset the line table adds so that lines
/// point into the original file. Heading ids are written onto the
/// rendered headings only when `with_toc` is set; without it no heading
/// work is done and `headings` comes back empty.
pub(crate) fn render(markdown: &str, frontmatter_lines: usize, with_toc: bool) -> Rendered {
    let (processed_markdown, line_origins) = process_github_alerts(markdown, frontmatter_lines);

    let parser = Parser::new_ext(&processed_markdown, parser_options()).into_offset_iter();
    let parser = extend_table_ranges(parser);
    let parser = process_code_blocks(parser, "mermaid");
    let parser = process_code_blocks(parser, "math");
    let parser = process_math_expressions(parser);

    // The events are needed twice: once for the outline, once to render.
    let events: Vec<_> = parser.collect();
    let (headings, heading_ids) = if with_toc {
        let headings = collect_headings(&events);
        let ids = headings.iter().map(|h| h.id.clone()).collect();
        (headings, ids)
    } else {
        (Vec::new(), Vec::new())
    };

    let html = render_events(events.into_iter(), heading_ids);

    Rendered {
        html,
        headings,
        lines: LineTable::new(processed_markdown, frontmatter_lines).with_origins(line_origins),
    }
}

/// Write the events as HTML with `data-source-span` on every block element.
pub(super) fn render_events<'a>(
    events: impl Iterator<Item = (Event<'a>, std::ops::Range<usize>)> + 'a,
    heading_ids: Vec<String>,
) -> String {
    let mut html = String::new();
    html::push_html(&mut html, inject_source_spans(events, heading_ids));
    attach_table_spans(&html)
}

/// Fold the table marker comments into the `<table>` tags that follow them.
///
/// `Start(Table)` has to reach `push_html` untouched so the writer knows
/// the column alignments, which leaves no way to put an attribute on the
/// tag from the event stream. The span injection writes
/// `<!--arto-table S-E-->` right before it instead, and this turns the
/// pair into `<table data-source-span="S-E">`.
fn attach_table_spans(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(TABLE_MARKER) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + TABLE_MARKER.len()..];
        match after.split_once("--><table>") {
            Some((span, tail)) => {
                out.push_str(&format!("<table data-source-span=\"{span}\">"));
                rest = tail;
            }
            None => {
                out.push_str(TABLE_MARKER);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_markers_become_span_attributes() {
        let html = "<p>x</p>\n<!--arto-table 9-27--><table><thead>";
        assert_eq!(
            attach_table_spans(html),
            "<p>x</p>\n<table data-source-span=\"9-27\"><thead>"
        );
    }

    #[test]
    fn a_marker_without_a_table_is_left_alone() {
        let html = "<!--arto-table 1-2-->text";
        assert_eq!(attach_table_spans(html), html);
    }
}
