//! Which hrefs name a local document, and which documents a source links to.
//!
//! One rule, read in two places: the post-processing pass uses it to turn an
//! anchor into an in-app link, and [`document_links`] uses it to list the
//! documents a source points at — so what a reader can click and what is
//! listed as a link are the same set.

use crate::{frontmatter, line_endings, RenderOptions, SourceRange};

/// A link to another Markdown document, as a click on it would open it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    /// The path the link opens, relative to the directory of the document it
    /// was written in unless it is absolute. A `file:` URL arrives as a path.
    pub path: String,
    /// The part after `#`, as written.
    pub fragment: Option<String>,
    /// Whether it was written as `[[target]]`.
    pub wiki: bool,
    /// Where it was written, lines counted in the whole file.
    pub range: SourceRange,
}

/// Every link in `markdown` that opens a Markdown document, in document
/// order.
///
/// What counts as a link is what the parser made one under `options`: a
/// `[x](y)` inside code is not listed, a reference-style link is listed where
/// it is used, and a wiki link is listed only while wiki links are on. Images
/// and links written as raw HTML are not listed.
pub fn document_links(markdown: impl AsRef<str>, options: &RenderOptions) -> Vec<DocumentLink> {
    let markdown = line_endings::normalize(markdown.as_ref());
    let (_, body, frontmatter_lines) = frontmatter::extract_and_render_frontmatter(&markdown);
    let links = match crate::engine::links(&body, frontmatter_lines, options) {
        Ok(links) => links,
        Err(error) => {
            tracing::debug!(%error, "Could not read the links of a document");
            return Vec::new();
        }
    };
    links
        .into_iter()
        .filter_map(|link| {
            let local = local_link(&link.href)?;
            local.is_markdown().then(|| DocumentLink {
                path: local.path,
                fragment: local.fragment.map(str::to_string),
                wiki: link.wiki,
                range: link.range,
            })
        })
        .collect()
}

/// An href that names something on the filesystem.
pub(crate) struct LocalLink<'a> {
    /// The path, with a `file:` URL turned into one.
    pub path: String,
    pub fragment: Option<&'a str>,
}

impl LocalLink<'_> {
    /// Whether the path names a Markdown document, by its extension.
    pub fn is_markdown(&self) -> bool {
        matches!(
            std::path::Path::new(&self.path)
                .extension()
                .and_then(|ext| ext.to_str()),
            Some("md" | "markdown")
        )
    }
}

/// The file an href names, or `None` when it addresses something else — a
/// URL with a scheme of its own, a place in the same document, or nothing
/// with an extension.
///
/// A fragment belongs to the target document, not to its file name, so it
/// is split off before the extension is read.
pub(crate) fn local_link(href: &str) -> Option<LocalLink<'_>> {
    if has_foreign_scheme(href) {
        return None;
    }
    let (path, fragment) = href
        .split_once('#')
        .map_or((href, None), |(path, fragment)| (path, Some(fragment)));
    // The app resolves the link as a filesystem path, so a `file:` URL is
    // turned into one here rather than being joined onto the base directory
    // as a literal string.
    let path = file_url_to_path(path).unwrap_or_else(|| path.to_string());
    std::path::Path::new(&path).extension()?;
    Some(LocalLink { path, fragment })
}

/// Whether `href` names a URL scheme that is not `file:`.
///
/// `http:`, `mailto:`, `tel:` and the like address something outside the
/// file system, so they stay anchors instead of being resolved as document
/// paths. A one-letter prefix is not read as a scheme, which keeps a Windows
/// path such as `C:\notes\a.md` a path.
pub(crate) fn has_foreign_scheme(href: &str) -> bool {
    let Some(colon) = href.find(':') else {
        return false;
    };
    let scheme = &href[..colon];
    scheme.len() > 1
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        && !scheme.eq_ignore_ascii_case("file")
}

/// A `file:` URL as a filesystem path, or `None` when `href` is not one.
///
/// `file://`, `file://localhost/…` and `file:/…` all parse; percent-encoding
/// and the platform's path shape are handled by `url`.
pub(crate) fn file_url_to_path(href: &str) -> Option<String> {
    if !href.starts_with("file:") {
        return None;
    }
    let path = url::Url::parse(href)
        .ok()
        .and_then(|url| url.to_file_path().ok());
    if path.is_none() {
        tracing::debug!(?href, "file: URL could not be parsed; left as written");
    }
    Some(path?.to_string_lossy().into_owned())
}
