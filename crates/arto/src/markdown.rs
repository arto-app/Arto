//! Markdown rendering for the desktop app.
//!
//! The rendering pipeline itself lives in the `arto-markdown` crate (shared
//! with `arto-page`). This module re-exports the items the app uses and
//! supplies the user's rendering preferences from the global `CONFIG`, so the
//! app's call sites keep their two-argument signatures.
//!
//! The re-export is an explicit list rather than a glob: the two render
//! functions below share their names with arto-markdown's three-argument
//! originals, and a glob would import those only to shadow them.

pub use arto_markdown::{extract_source_selection, HeadingInfo, RenderOptions};

use crate::config::CONFIG;
use anyhow::Result;
use std::path::Path;

/// The user's rendering preferences, copied out so the config lock is not
/// held while rendering.
fn render_options() -> RenderOptions {
    CONFIG.read().markdown.clone()
}

/// Render Markdown to HTML, honoring the user's rendering preferences.
pub fn render_to_html(markdown: impl AsRef<str>, base_path: impl AsRef<Path>) -> Result<String> {
    arto_markdown::render_to_html(markdown, base_path, &render_options())
}

/// Render Markdown to HTML with TOC information, honoring the user's
/// rendering preferences.
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
) -> Result<(String, Vec<HeadingInfo>)> {
    arto_markdown::render_to_html_with_toc(markdown, base_path, &render_options())
}
