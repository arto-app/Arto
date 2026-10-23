//! Arto's Markdown to HTML pipeline.
//!
//! One rendering path serves the desktop app, `arto page` and the Quick
//! Look extension, so the HTML this crate produces is a contract: the
//! frontend scripts, the app and the stylesheets read the attributes and
//! class names listed below by name. Changing any of them means changing
//! every reader in the same step, and the sample documents under
//! `samples/` are snapshot-tested (`tests/samples.rs`) so that an unintended
//! change shows up as a diff.
//!
//! # Pipeline
//!
//! 0. **Line endings.** A lone `\r` becomes `\n`, byte for byte, so a file
//!    written with them reports the lines it has; `\r\n` is left as it is.
//! 1. **Frontmatter.** A leading YAML block is cut off and rendered to a
//!    `<details class="frontmatter">` table that is prepended to the
//!    output at the very end. The lines it occupied are added to every
//!    source line below it.
//! 2. **The engine** (`engine` module: everything that knows the parser).
//!    The body is parsed once and rendered with hooks that replace fenced
//!    `mermaid` and `math` blocks and `$…$` expressions by the
//!    `preprocessed-*` containers described below. A second pass over the
//!    rendered HTML turns the byte range on each block element into the
//!    `data-source-range` described below, gives GitHub alerts
//!    the class names GitHub uses, and keeps the heading
//!    ids when a table of contents was requested.
//! 3. **Post-processing** with lol_html: local images are inlined as data
//!    URLs and local Markdown links become `<span class="md-link">`.
//!
//! # HTML contract
//!
//! ## Source ranges
//!
//! Every element rendered from a block of the source carries
//! `data-source-range="L:C-L:C"`, the range of the file it was rendered from:
//! paragraphs, headings, quotes and alerts, lists and their items, tables
//! down to each cell, rules, code blocks, definition lists, footnote
//! definitions and the Mermaid and math containers. Elements the pipeline
//! makes up, such as an alert's title, carry none.
//!
//! Lines and columns are 1-based, lines count through the whole file
//! (frontmatter included) and columns count Unicode code points, so the
//! range reads the way an editor shows it. Both ends are inclusive: the end
//! is the last character of the block, not the one after it.
//!
//! A range is the source the element shows, without surrounding
//! whitespace: a table cell starts after the padding inside its pipes, and
//! an alert's body starts after the `[!KIND]` marker. It is a span of the
//! file, so a block nested in a quote or a list keeps that container's
//! markers on every line after its first.
//!
//! A code block's `<code>` carries the range of its content — from the
//! first column of the line after the opening fence, or of the first line
//! of an indented block, even when that line is blank — so the frontend can
//! count the newlines inside `<code>` down to the exact line. A code block
//! with no content has no range on its `<code>`.
//!
//! The ranges are not monotonic in document order: footnote definitions
//! are moved to the section at the end while keeping the lines they were
//! written on, so that section reports lines from the middle of the file.
//!
//! Readers: `frontend/src/source-range.ts` parses the attribute for
//! `frontend/src/context-menu-handler.ts` (copy path with line),
//! `frontend/src/content-cursor.ts` (keyboard cursor) and
//! `frontend/src/scroll-anchor.ts`; the app receives the range through
//! `crates/arto/src/components/content/context_menu/data.rs` and turns it
//! back into text in `crates/arto/src/utils/source_lines.rs`.
//!
//! ## Mermaid and math
//!
//! The frontend renders diagrams and formulas client-side, so the pipeline
//! emits containers that hold the source text twice: escaped as the
//! visible fallback, and in `data-original-content` for the renderer.
//!
//! - ` ```mermaid ` → `<pre class="preprocessed-mermaid" data-original-content="…">`
//! - ` ```math ` → `<pre class="preprocessed-math" data-original-content="…">`
//! - `$$…$$` → `<div class="preprocessed-math-display" data-original-content="…">`
//! - `$…$` → `<span class="preprocessed-math-inline" data-original-content="…">`
//!
//! Readers: `frontend/src/mermaid-renderer.ts`, `frontend/src/math-renderer.ts`,
//! `frontend/src/code-copy.ts` (copy as image), `frontend/src/content-cursor.ts`,
//! `frontend/src/context-menu-handler.ts`, `frontend/src/render-coordinator.ts`
//! and `crates/arto/src/keybindings/dispatcher.rs` (open in a window);
//! styled by `frontend/style/components/content/markdown-viewer.css`,
//! `frontend/style/components/mermaid-window.css`,
//! `frontend/style/components/math-window.css` and `frontend/style/print.css`.
//!
//! ## GitHub alerts
//!
//! `> [!NOTE]` and the other kinds become
//! `<div class="markdown-alert markdown-alert-<kind>" dir="auto">` with a
//! `<p class="markdown-alert-title">` holding
//! `<span class="alert-icon" data-alert-type="<kind>">` and the kind name.
//! The class names are GitHub's, so the frontend stylesheet styles them; the
//! icon span is a placeholder for the frontend to fill in. The marker is
//! matched case-insensitively, so `[!note]` is an alert too.
//!
//! ## Footnotes
//!
//! References render as `<sup><a href="#fn-<id>" id="fnref-<id>">N</a></sup>`
//! and the definitions are collected into one
//! `<section class="footnotes"><ol>` at the end of the document, numbered in
//! the order they are first referenced. That is GitHub's shape, so
//! the frontend styles it. The links back to a note's references,
//! `<a href="#fnref-<id>">`, follow its last block inside one
//! `<span class="footnote-backrefs">`, which the stylesheet places beside
//! the note's number; the note's `<li>` carries how many there are as
//! `style="--footnote-backrefs: N"`.
//!
//! ## Frontmatter
//!
//! `<details class="frontmatter">` wraps `<summary class="frontmatter-summary">`
//! and `<table class="frontmatter-table">`. Values are typed with
//! `yaml-null`, `yaml-bool`, `yaml-number`, `yaml-empty`, `yaml-list` and
//! `yaml-nested`; styled by `frontend/style/components/content/frontmatter.css`.
//!
//! ## Links and images
//!
//! A link to a local file becomes `<span class="md-link" data-md-link="…">`
//! with an inline `onmousedown` that calls
//! `window.handleMarkdownLinkClick(path, button)`, which the app installs
//! in `crates/arto/src/components/content/file_viewer.rs`. `data-md-link`
//! keeps the href as written, fragment included, and the app splits off
//! the fragment and scrolls to that heading after opening the file. Links
//! to files that are not Markdown add `md-link-invalid`; links to Markdown
//! files that do not exist add `md-link-missing`. Links that carry a scheme
//! of their own (`http(s):`, `mailto:`, `tel:`, …) and fragment-only links
//! stay anchors. Local images are inlined as `data:` URLs so the page works
//! offline and in Quick Look — `<img src>` as well as every candidate of an
//! `<img srcset>` or a `<source srcset>`, so a theme-aware `<picture>`
//! renders whichever one the browser picks. A `srcset` candidate that names
//! a local file which cannot be read is dropped rather than left in place,
//! because the page has no base URL to resolve it against and the browser
//! would otherwise pick it over a candidate that did inline. Readers of
//! `data-md-link` and `.md-link` are the app,
//! `frontend/src/link-preview.ts` (which previews the target on hover and
//! leaves `md-link-missing` and `md-link-invalid` alone) and
//! `frontend/style/components/content/markdown-viewer.css`.
//!
//! HTML shown on a page other than its own document's goes through
//! [`rebase_document_links`] first, so its links keep naming what they named
//! there.
//!
//! ## Videos
//!
//! A paragraph that is nothing but the URL of a video uploaded to GitHub
//! (`https://github.com/user-attachments/assets/…`, or a video file on
//! `user-images.githubusercontent.com`) becomes
//! `<p data-source-range="…"><video src="…" controls muted><a href="…">…</a></video></p>`,
//! which is how github.com shows the URL its editor writes for a dropped
//! video. The link inside is the player's fallback content: a page that
//! loads no remote media (a preview, `arto page` under its policy) goes
//! through [`without_remote_media`] and keeps only the link, and
//! `frontend/src/video-fallback.ts` swaps a player that fails to play for
//! it. A URL inside a sentence, or a link with a label, stays a link.
//! Styled by `frontend/style/components/content/markdown-viewer.css`.
//!
//! ## Headings
//!
//! [`render_to_html_with_toc`] returns [`HeadingInfo`] for every heading
//! and sets the same `id` on the rendered `h1`–`h6`, so the table of
//! contents gutter (`crates/arto/src/components/content/gutter.rs`)
//! can scroll to it with `getElementById`. The id is the heading text
//! lowercased with every run of non-alphanumerics replaced by `-`, keeping
//! Unicode letters, so `## 日本語の見出し` is reachable; a text that leaves
//! nothing becomes `section`, and a repeated id gets a `-1`, `-2`, … suffix.
//! [`render_to_html`] adds no ids at all.
//!
//! A heading may end in a `{#id .class}` block, which becomes the `id` and
//! `class` of the tag instead of showing as text and is left out of
//! [`HeadingInfo::text`]. An id written that way is content rather than a
//! generated anchor, so it survives [`render_to_html`] too.
//!
//! ## Wiki links
//!
//! `[[Page]]` and `[[Page|Label]]` become links to `Page.md` — a target
//! without a file extension names a Markdown document, so the link goes on
//! to become an `.md-link` like any other document link. A target that
//! already has an extension, or an `http(s)` URL, is used as written, and a
//! `#fragment` stays on the end. The target reaches the app as it was
//! written rather than percent-encoded, because it is opened as a file
//! name; the label is inline Markdown. Inside code and inside the math
//! containers the brackets are text.
//!
//! ## Code blocks
//!
//! `<pre><code class="language-<lang>">`; the frontend highlights by that
//! class and `frontend/src/code-copy.ts` adds the copy button to every
//! `pre`.

mod block;
mod engine;
mod error;
mod frontmatter;
mod headings;
mod images;
mod line_endings;
mod links;
mod options;
mod post_process;
mod reading;
mod remote_media;
mod rewrite;
mod sanitize;
mod urls;

pub use block::*;
pub use engine::*;
pub use error::*;
pub use headings::*;
pub use images::*;
pub use links::*;
pub use options::*;
pub use reading::*;
pub use remote_media::*;

use frontmatter::extract_and_render_frontmatter;
use post_process::post_process_html_tags;
use remote_media::detach;
use std::path::{Path, PathBuf};

/// Everything the render functions need after the document was rendered.
struct PipelineResult {
    raw_html: String,
    frontmatter_html: String,
    base_dir: PathBuf,
    headings: Vec<HeadingInfo>,
    reading: ReadingProfile,
}

/// Run the pipeline up to the raw HTML: frontmatter extraction and the
/// engine.
///
/// With `with_toc`, the headings are collected from the same parse and keep
/// their ids on the rendered headings; without it no ids are written and
/// `headings` comes back empty.
fn run_pipeline(
    markdown: &str,
    base_path: &Path,
    options: &RenderOptions,
    with_toc: bool,
) -> Result<PipelineResult, RenderError> {
    let base_dir = base_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let markdown = line_endings::normalize(markdown);
    let (frontmatter_html, body, frontmatter_lines) = extract_and_render_frontmatter(&markdown);
    let rendered = engine::render(&body, frontmatter_lines, options, with_toc)?;

    Ok(PipelineResult {
        raw_html: rendered.html,
        frontmatter_html,
        base_dir,
        headings: rendered.headings,
        reading: rendered.reading,
    })
}

/// Prepend frontmatter HTML to the post-processed output.
fn prepend_frontmatter(frontmatter_html: &str, html_output: String) -> String {
    if frontmatter_html.is_empty() {
        html_output
    } else {
        format!("{}\n{}", frontmatter_html, html_output)
    }
}

/// A local image the host agreed to serve, and the file it stands for.
///
/// Produced only under [`ImageResolution::Deferred`]; the id is the one that
/// appears in the `<img src>` the render wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredImage {
    pub id: String,
    pub path: PathBuf,
    /// What to answer the request with, from the same table the inlined
    /// path uses, so the two do not disagree about a format.
    pub mime: &'static str,
}

/// What a render produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderResult {
    /// The rendered document.
    pub html: String,
    /// Every heading, when a table of contents was asked for; otherwise empty,
    /// because a render without one writes no ids to point at.
    pub headings: Vec<HeadingInfo>,
    /// The images the host has to serve, in the order they were first
    /// referenced. Always empty under [`ImageResolution::DataUrl`], where the
    /// bytes are in the document already.
    pub images: Vec<DeferredImage>,
    /// What the document asks a reader to read, one entry per top-level
    /// block (see [`ReadingProfile`]). The frontmatter is not read.
    pub reading: ReadingProfile,
}

/// Render Markdown to HTML.
///
/// Relative links and images resolve against the directory of `base_path`.
pub fn render_to_html(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<RenderResult, RenderError> {
    render(markdown.as_ref(), base_path.as_ref(), options, false)
}

/// Render Markdown to HTML, keeping the heading ids and reporting the
/// headings a table of contents is built from.
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<RenderResult, RenderError> {
    render(markdown.as_ref(), base_path.as_ref(), options, true)
}

/// Render Markdown that was not read from the file it is shown with — a
/// translation of the whole document, a summary of a block — the way a
/// document is rendered, frontmatter included.
///
/// The result carries no source ranges: they would point into the file, at
/// text that is not what was rendered. Images resolve against `base_path`
/// like the file's own.
///
/// Nothing in it is fetched from elsewhere: an image, a video poster or any
/// other media from another host is dropped, leaving an image's alt text.
/// What is rendered here was written by a model, and a document can talk a
/// model into putting what it read into an image's address — which showing
/// the answer would then send to whoever the address names, whichever agent
/// the reader chose to keep the document to. A link stays, since it is only
/// followed when clicked.
pub fn render_detached(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<RenderResult, RenderError> {
    let mut rendered = render(markdown.as_ref(), base_path.as_ref(), options, false)?;
    let local = match &options.images {
        ImageResolution::Deferred { base_url } => Some(base_url.as_str()),
        _ => None,
    };
    rendered.html = detach(&rendered.html, local);
    Ok(rendered)
}

/// Render the document at `base_path` to be previewed on another
/// document's page, beside a link to it.
///
/// Heading ids are kept, so the part a link's fragment names can be found
/// in it, and its links are rebased (see [`rebase_document_links`]) so that
/// they go on naming what they named in their own document. The result
/// carries no source ranges, which would point into the wrong file.
///
/// Nothing in it is fetched from elsewhere, whatever `options` allow: a
/// preview appears when the pointer passes over a link, which is not the
/// reader opening the document, so nothing the document names is sent a
/// request for having been passed over. As in [`render_detached`], raw HTML
/// is escaped — the filter a reader trusts their own documents to lets
/// through a stylesheet or a style's `url()` — and media from another host
/// is dropped. A Mermaid diagram is the exception: it is handed to the page,
/// which draws it as it would its own — images it names included — and
/// takes out of the drawing only what would run in the app
/// (`frontend/src/preview-diagram.ts`).
pub fn render_preview(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<RenderResult, RenderError> {
    let base_path = base_path.as_ref();
    let options = RenderOptions {
        raw_html: RawHtml::Escape,
        ..options.clone()
    };
    let mut rendered = render(markdown.as_ref(), base_path, &options, true)?;
    let local = match &options.images {
        ImageResolution::Deferred { base_url } => Some(base_url.as_str()),
        _ => None,
    };
    let html = detach(&rendered.html, local);
    rendered.html = rebase_document_links(&html, base_path);
    Ok(rendered)
}

/// `html`, rendered from the document at `document_path`, made to be shown
/// on another document's page: every document link names its target the
/// way it did in its own document.
///
/// `data-md-link` holds a path as written, which the app resolves against
/// the document on screen, so a relative one is joined onto the directory
/// of `document_path`; a fragment-only link, which would otherwise scroll
/// the page it is shown on, becomes a document link to `document_path`
/// itself. Links with a scheme of their own are left alone.
pub fn rebase_document_links(html: &str, document_path: impl AsRef<Path>) -> String {
    links::rebase_document_links(html, document_path.as_ref())
}

fn render(
    markdown: &str,
    base_path: &Path,
    options: &RenderOptions,
    with_toc: bool,
) -> Result<RenderResult, RenderError> {
    let pipeline = run_pipeline(markdown, base_path, options, with_toc)?;

    let post_processed = post_process_html_tags(
        &pipeline.raw_html,
        &pipeline.base_dir,
        &options.images,
        options.raw_html,
    );
    let (html_output, images) = match post_processed {
        Ok(post_processed) => post_processed,
        Err(interrupted) => {
            tracing::debug!(error = %interrupted.error, "post-processing stopped part way");
            match options.raw_html {
                // What the filter did not reach would reach the page
                // unfiltered. Escaped, the raw HTML needs no filter to be
                // safe, and the document is shown whole.
                RawHtml::Filter => {
                    let escaped = RenderOptions {
                        raw_html: RawHtml::Escape,
                        ..options.clone()
                    };
                    return render(markdown, base_path, &escaped, with_toc);
                }
                // Trusted with its raw HTML, the document loses only its
                // images and link targets by being shown as it came.
                RawHtml::Allow => (pipeline.raw_html, Vec::new()),
                // Nothing raw is left to be ambiguous, so this is not
                // expected; what was written went through the filter.
                RawHtml::Escape => (interrupted.written, Vec::new()),
            }
        }
    };

    Ok(RenderResult {
        html: prepend_frontmatter(&pipeline.frontmatter_html, html_output),
        headings: pipeline.headings,
        images,
        reading: pipeline.reading,
    })
}
