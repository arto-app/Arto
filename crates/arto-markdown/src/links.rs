//! Which hrefs name a local document, and which documents a source links to.
//!
//! One rule, read in two places: the post-processing pass uses it to turn an
//! anchor into an in-app link, and [`document_links`] uses it to list the
//! documents a source points at — so what a reader can click and what is
//! listed as a link are the same set.

use lol_html::{element, Settings};
use std::path::Path;

use crate::rewrite::rewrite;
use crate::urls::{file_url_to_path, has_foreign_scheme};
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
    let path = file_url_to_path(path).map_or_else(
        || path.to_string(),
        |path| path.to_string_lossy().into_owned(),
    );
    std::path::Path::new(&path).extension()?;
    Some(LocalLink { path, fragment })
}

/// Whether the file a local link points at exists. The check is what the
/// click handler will do with the same path, so a link that cannot open is
/// shown as such before it is clicked.
pub(crate) fn link_target_exists(base_dir: &Path, path: &str) -> bool {
    let target = Path::new(path);
    if target.is_absolute() {
        target.is_file()
    } else {
        base_dir.join(target).is_file()
    }
}

/// Turn `el` into the `span.md-link` the app follows (see the HTML contract
/// in the crate docs).
pub(crate) fn make_document_link(
    el: &mut lol_html::html_content::Element,
    link: &str,
    class: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    el.set_tag_name("span")?;
    el.remove_attribute("href");
    el.set_attribute("data-md-link", link)?;
    el.set_attribute("class", class)?;
    el.set_attribute(
        "onmousedown",
        "if(event.button===0||event.button===1){event.preventDefault();window.handleMarkdownLinkClick(this.dataset.mdLink,event.button)}",
    )?;
    Ok(())
}

/// `html`, rendered from the document at `document_path`, with its links
/// made to name their targets the same wherever it is shown.
///
/// A document link keeps the path as written, which the app resolves
/// against the document on screen; shown on another document's page it
/// would resolve against that one instead. A relative path is therefore
/// joined onto the directory `document_path` is in, and a link that is only
/// a fragment becomes a document link to `document_path` itself.
pub(crate) fn rebase_document_links(html: &str, document_path: &Path) -> String {
    let base_dir = document_path.parent().unwrap_or(Path::new(""));
    let settings = Settings::new()
        .append_element_content_handler(element!("span.md-link[data-md-link]", |el| {
            let Some(link) = el.get_attribute("data-md-link") else {
                return Ok(());
            };
            let (path, fragment) = link
                .split_once('#')
                .map_or((link.as_str(), None), |(path, fragment)| {
                    (path, Some(fragment))
                });
            if Path::new(path).is_absolute() {
                return Ok(());
            }
            let rebased = base_dir.join(path).display().to_string();
            let rebased = match fragment {
                Some(fragment) => format!("{rebased}#{fragment}"),
                None => rebased,
            };
            el.set_attribute("data-md-link", &rebased)?;
            Ok(())
        }))
        .append_element_content_handler(element!(r##"a[href^="#"]"##, |el| {
            let Some(href) = el.get_attribute("href") else {
                return Ok(());
            };
            let link = format!("{}{href}", document_path.display());
            make_document_link(el, &link, "md-link")?;
            Ok(())
        }));
    // Links left unrebased still open something; links cut off open nothing.
    rewrite(html, settings).unwrap_or_else(|_| html.to_string())
}
