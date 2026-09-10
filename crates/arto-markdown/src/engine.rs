//! The ox-content rendering engine.
//!
//! Everything that knows the parser, and the HTML the renderer writes, lives
//! below this module: the parser and renderer options, the hooks that swap
//! Mermaid and math for the containers the frontend renders client-side, the
//! heading outline, the pass that turns the rendered markup into the crate's
//! HTML contract, and the selection source map. The rest of the crate sees
//! [`render`] with its [`Rendered`] output and [`extract_source_selection`],
//! neither of which mentions an ox-content type, so replacing the engine is
//! a change inside this directory.

mod annotate;
mod hooks;
mod lines;
mod outline;
mod source_map;
mod wiki;

pub use source_map::*;

use crate::{HeadingInfo, RawHtml, RenderOptions};
use anyhow::{anyhow, Result};
use lines::LineTable;
use ox_content_allocator::Allocator;
use ox_content_parser::{Parser, ParserOptions};
use ox_content_renderer::{HtmlRenderer, HtmlRendererOptions};

/// The parser configuration every parse in the engine uses.
///
/// Rendering and the selection source map must agree on the text of a
/// document and on where each piece of it came from, or a selection in the
/// rendered view maps onto a different document — which is why both build
/// their options from the same [`RenderOptions`] through this function, and
/// why the map takes options it does not otherwise need (see [`source_map`]).
///
/// GFM is the baseline and is not configurable: a document written for GitHub
/// contains tables and task lists, and showing those as literal pipes and
/// brackets is a broken reader rather than a configured one.
fn parser_options(options: &RenderOptions) -> ParserOptions {
    ParserOptions {
        autolinks: options.auto_link_urls,
        cjk_emphasis: options.cjk_emphasis,
        math: options.math,
        superscript: options.superscript,
        subscript: options.subscript,
        smart_punctuation: options.smart_punctuation,
        definition_lists: options.definition_lists,
        heading_attributes: options.heading_attributes,
        wiki_links: options.wiki_links,
        ..ParserOptions::gfm()
    }
}

/// The renderer configuration, which the crate's HTML contract depends on.
///
/// What the reader chooses is autolinking, the heading permalinks and the
/// treatment of raw HTML; the rest is the contract the frontend, the app and
/// the stylesheet read, and is fixed.
fn renderer_options(options: &RenderOptions) -> HtmlRendererOptions {
    HtmlRendererOptions {
        autolink_urls: options.auto_link_urls,
        // Arto opens links itself, in the window the user asked for; a
        // `target` would only confuse the webview's click handling.
        autolink_target_blank: false,
        link_target_blank: false,
        sanitize: options.raw_html == RawHtml::Escape,
        disallow_raw_html: options.raw_html == RawHtml::Filter,
        heading_permalinks: options.heading_permalinks,
        // Footnotes as GitHub writes them — one `<section class="footnotes">`
        // with a numbered list — which is the shape the frontend stylesheet
        // styles and the shape that numbers a named footnote.
        semantic_footnotes: true,
        // The byte ranges the annotation pass turns into source lines.
        source_spans: true,
        ..HtmlRendererOptions::default()
    }
}

/// What the engine produces for one document body.
pub(crate) struct Rendered {
    /// HTML of the body, in the shape the crate documentation describes.
    pub html: String,
    /// Headings in document order, with the ids the rendered headings
    /// carry; empty unless a table of contents was requested.
    pub headings: Vec<HeadingInfo>,
}

/// Render a document body (the Markdown after the frontmatter was cut off).
///
/// `frontmatter_lines` is the offset the source lines are shifted by so they
/// point into the original file. Heading ids stay on the rendered headings
/// only when `with_toc` is set; without it no outline is returned.
pub(crate) fn render(
    body: &str,
    frontmatter_lines: usize,
    options: &RenderOptions,
    with_toc: bool,
) -> Result<Rendered> {
    let allocator = Allocator::new();
    let document = Parser::with_options(&allocator, body, parser_options(options))
        .parse()
        .map_err(|error| anyhow!("failed to parse Markdown: {error}"))?;

    let html = HtmlRenderer::with_options(renderer_options(options))
        .render_with_hooks(&document, &mut hooks::ArtoHooks::new(body));

    let lines = LineTable::new(body, frontmatter_lines);
    let annotated = annotate::annotate(&html, &lines, with_toc);

    let headings = if with_toc {
        // The ids come back from the rendered headings, so the outline and
        // the anchors cannot disagree even about a repeated slug.
        outline::collect(&document)
            .into_iter()
            .zip(annotated.heading_ids)
            .map(|(heading, id)| HeadingInfo {
                level: heading.level,
                text: heading.text,
                id,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(Rendered {
        html: annotated.html,
        headings,
    })
}
