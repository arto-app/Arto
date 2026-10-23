//! Behaviour of the rendering pipeline seen through its public API.
//!
//! These tests describe what the rendered HTML looks like for each
//! construct, not how the engine produces it, so they hold across an
//! engine swap. Internals that are only observable at the engine level
//! (event streams, offset mapping) are tested next to that code.

use arto_markdown::{
    rebase_document_links, render_preview, render_to_html, render_to_html_with_toc, HeadingInfo,
    ImageResolution, RawHtml, ReadingBlock, RenderOptions,
};
use indoc::indoc;
use std::path::Path;

mod alerts;
mod containers;
mod errors;
mod images;
mod line_endings;
mod options;
mod reading_profile;
mod rebased_links;
mod remote_media;
mod source_ranges;
mod tables;
mod toc;
mod video;
mod wiki_links;

fn render(markdown: &str) -> String {
    render_to_html(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html
}

fn headings(markdown: &str) -> Vec<HeadingInfo> {
    render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .headings
}

/// Whether some `<tag …>` start tag in `html` carries every attribute in
/// `attrs`, in any order.
fn has_element(html: &str, tag: &str, attrs: &[(&str, &str)]) -> bool {
    html.match_indices(&format!("<{tag} ")).any(|(pos, _)| {
        let end = html[pos..].find('>').map_or(html.len(), |end| pos + end);
        let start_tag = &html[pos..end];
        attrs
            .iter()
            .all(|(name, value)| start_tag.contains(&format!(r#" {name}="{value}""#)))
    })
}
