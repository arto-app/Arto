//! The pulldown-cmark rendering engine.
//!
//! Everything that knows the parser lives below this module: the parser
//! configuration, the GitHub alert rewrite (whose bodies are rendered
//! with the parser), the event transforms for Mermaid and math, the
//! source line injection, the heading outline, and the selection source
//! map. The rest of the crate sees [`render`] with its [`Rendered`]
//! output and [`extract_source_selection`], none of which mention parser
//! types, so replacing the engine is a change inside this directory.

mod alerts;
mod event_processors;
mod outline;
mod source_lines;
mod source_map;

pub use source_map::*;

use crate::HeadingInfo;
use alerts::process_github_alerts;
use event_processors::{extend_table_ranges, process_code_blocks, process_math_expressions};
use outline::collect_headings;
use pulldown_cmark::{html, Options, Parser};
use source_lines::{extract_table_source_lines, inject_source_lines};

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
    /// HTML of the body with the `data-source-line` attributes of every
    /// block element except tables already in place.
    pub html: String,
    /// Headings in document order, with the ids the rendered headings
    /// carry; empty unless a table of contents was requested.
    pub headings: Vec<HeadingInfo>,
    /// `(start_line, end_line)` of every table in document order, to be
    /// written onto the `<table>` elements by the post-processing pass.
    pub table_source_lines: Vec<(usize, usize)>,
}

/// Render a document body (the Markdown after the frontmatter was cut off
/// and bare URLs were turned into autolinks).
///
/// `frontmatter_lines` is added to every source line so that the numbers
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

    // The events are needed twice: once to read positions and headings,
    // once to render. Injection consumes the ranges.
    let events: Vec<_> = parser.collect();
    let table_source_lines = extract_table_source_lines(
        &events,
        &processed_markdown,
        &line_origins,
        frontmatter_lines,
    );
    let (headings, heading_ids) = if with_toc {
        let headings = collect_headings(&events);
        let ids = headings.iter().map(|h| h.id.clone()).collect();
        (headings, ids)
    } else {
        (Vec::new(), Vec::new())
    };

    let parser = inject_source_lines(
        events.into_iter(),
        &processed_markdown,
        &line_origins,
        frontmatter_lines,
        heading_ids,
    );

    let mut html = String::new();
    html::push_html(&mut html, parser);

    Rendered {
        html,
        headings,
        table_source_lines,
    }
}
