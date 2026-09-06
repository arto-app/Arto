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
mod attributes;
mod hooks;
mod lines;
mod outline;
mod source_map;
mod wiki;

pub use source_map::*;

use crate::{HeadingInfo, RenderOptions};
use anyhow::{anyhow, Result};
use lines::LineTable;
use ox_content_allocator::Allocator;
use ox_content_parser::{Parser, ParserOptions};
use ox_content_renderer::{HtmlRenderer, HtmlRendererOptions};

/// The parser configuration every parse in the engine uses.
///
/// Rendering and the selection source map must agree on the text of a
/// document and on where each piece of it came from, or a selection in the
/// rendered view maps onto a different document. Everything here is fixed
/// for that reason; only `auto_link_urls` varies, and the map is built so
/// that it cannot see the difference (see [`source_map`]).
///
/// GFM is the baseline; the extensions on top of it are the ones Arto's
/// documents rely on.
fn parser_options(auto_link_urls: bool) -> ParserOptions {
    ParserOptions {
        autolinks: auto_link_urls,
        // `**強調。**` against CJK punctuation is ordinary Japanese prose;
        // CommonMark's flanking rules would leave the delimiters literal.
        cjk_emphasis: true,
        math: true,
        superscript: true,
        subscript: true,
        smart_punctuation: true,
        definition_lists: true,
        ..ParserOptions::gfm()
    }
}

/// The renderer configuration, which the crate's HTML contract depends on.
fn renderer_options(auto_link_urls: bool) -> HtmlRendererOptions {
    HtmlRendererOptions {
        autolink_urls: auto_link_urls,
        // Arto opens links itself, in the window the user asked for; a
        // `target` would only confuse the webview's click handling.
        autolink_target_blank: false,
        link_target_blank: false,
        // GFM's tagfilter: `<style>`, `<script>` and friends show as text
        // instead of restyling the page around the document.
        disallow_raw_html: true,
        // Footnotes as GitHub writes them — one `<section class="footnotes">`
        // with a numbered list — which is the shape `github-markdown-css`
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
    let document = Parser::with_options(&allocator, body, parser_options(options.auto_link_urls))
        .parse()
        .map_err(|error| anyhow!("failed to parse Markdown: {error}"))?;

    let html = HtmlRenderer::with_options(renderer_options(options.auto_link_urls))
        .render_with_hooks(&document, &mut hooks::ArtoHooks::default());

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
