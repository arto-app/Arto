//! Markdown rendering for the desktop app.
//!
//! The rendering pipeline itself lives in the `arto-markdown` crate (shared
//! with `arto-page`). This module re-exports the items the app uses and
//! supplies the user's rendering preferences from the global `CONFIG`, so the
//! app's call sites keep their two-argument signatures.
//!
//! The re-export is an explicit list rather than a glob: the functions below
//! share their names with arto-markdown's originals, which take the options
//! as one more argument, and a glob would import those only to shadow them.

pub use arto_markdown::{HeadingInfo, ImageResolution, RawHtml, RenderOptions};

use crate::config::CONFIG;
use anyhow::Result;
use std::path::Path;

/// The user's rendering preferences, copied out so the config lock is not
/// held while rendering.
///
/// The app serves the images a document references rather than carrying them
/// in it: inlining costs the bytes twice over, once in the HTML built here
/// and once in the document the WebView parses, and a page of screenshots
/// pays that on every re-render. `arto page` and Quick Look still inline,
/// having nothing to serve from.
fn render_options() -> RenderOptions {
    RenderOptions {
        images: ImageResolution::Deferred {
            base_url: crate::assets::image_base_url(),
        },
        ..CONFIG.read().markdown.clone()
    }
}

/// Render Markdown to HTML with TOC information, honoring the user's
/// rendering preferences.
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
) -> Result<(String, Vec<HeadingInfo>)> {
    let rendered = arto_markdown::render_to_html_with_toc(markdown, base_path, &render_options())?;
    crate::assets::images::register(rendered.images);
    Ok((rendered.html, rendered.headings))
}

/// Extract the Markdown source behind a selection of the rendered text.
///
/// The same preferences the document was rendered with, because the map that
/// answers this is built by parsing the source again: reading it any other
/// way would describe a document the reader is not looking at.
pub fn extract_source_selection(
    source: impl AsRef<str>,
    selected_text: impl AsRef<str>,
) -> Option<String> {
    arto_markdown::extract_source_selection(source, selected_text, &render_options())
}
