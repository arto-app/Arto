//! Markdown rendering for the desktop app.
//!
//! The rendering pipeline itself lives in the standalone `arto-markdown`
//! crate (shared with the Quick Look preview generator). This module is a
//! thin wrapper that re-exports that crate's public API and supplies the
//! `auto_link_urls` preference from the global `CONFIG` so call sites keep
//! their existing two-argument signatures.

pub use arto_markdown::*;

use crate::config::CONFIG;
use anyhow::Result;
use std::path::Path;

/// Render Markdown to HTML, honoring the user's `auto_link_urls` preference.
pub fn render_to_html(markdown: impl AsRef<str>, base_path: impl AsRef<Path>) -> Result<String> {
    let auto_link_urls = CONFIG.read().markdown.auto_link_urls;
    arto_markdown::render_to_html(markdown, base_path, auto_link_urls)
}

/// Render Markdown to HTML with TOC information, honoring the user's
/// `auto_link_urls` preference.
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
) -> Result<(String, Vec<HeadingInfo>)> {
    let auto_link_urls = CONFIG.read().markdown.auto_link_urls;
    arto_markdown::render_to_html_with_toc(markdown, base_path, auto_link_urls)
}
