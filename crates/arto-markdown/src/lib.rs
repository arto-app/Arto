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
//! the frontend styles it.
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
//! `data-md-link` and `.md-link` are the app and
//! `frontend/style/components/content/markdown-viewer.css`.
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
mod frontmatter;
mod headings;
mod line_endings;
mod options;
mod post_process;
mod reading;
mod sanitize;

pub use block::*;
pub use engine::*;
pub use headings::*;
pub use options::*;
pub use reading::*;

use anyhow::Result;
use frontmatter::extract_and_render_frontmatter;
use post_process::post_process_html_tags;
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
) -> Result<PipelineResult> {
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
) -> Result<RenderResult> {
    render(markdown.as_ref(), base_path.as_ref(), options, false)
}

/// Render Markdown to HTML, keeping the heading ids and reporting the
/// headings a table of contents is built from.
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<RenderResult> {
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
) -> Result<RenderResult> {
    let mut rendered = render(markdown.as_ref(), base_path.as_ref(), options, false)?;
    let local = match &options.images {
        ImageResolution::Deferred { base_url } => Some(base_url.as_str()),
        _ => None,
    };
    rendered.html = detach(&rendered.html, local);
    Ok(rendered)
}

/// Whether `url` makes the page fetch something from another host: an
/// absolute or protocol-relative address that is neither inline data nor
/// under `local`, where the app serves images from.
///
/// Read the way a browser's URL parser reads it on a page served over
/// http: tabs and newlines dropped, a backslash taken for a slash — so
/// `\\host/x` is as protocol-relative as `//host/x`.
fn is_remote(url: &str, local: Option<&str>) -> bool {
    let url: String = url
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .map(|c| if c == '\\' { '/' } else { c })
        .collect();
    let url = url.trim_matches(|c: char| c <= ' ');
    if local.is_some_and(|local| url.starts_with(local)) {
        return false;
    }
    if url.starts_with("//") {
        return true;
    }
    match url.split_once(':') {
        Some((scheme, _)) => {
            let is_scheme = !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
            is_scheme && !scheme.eq_ignore_ascii_case("data")
        }
        None => false,
    }
}

/// `html` without its `data-source-range` attributes, and without media
/// fetched from anywhere but `local` (see [`render_detached`]).
fn detach(html: &str, local: Option<&str>) -> String {
    let fetches_elsewhere = |el: &lol_html::html_content::Element| {
        let srcset = el.get_attribute("srcset").unwrap_or_default();
        ["src", "poster", "data"]
            .iter()
            .filter_map(|name| el.get_attribute(name))
            .chain(
                srcset
                    .split(',')
                    .filter_map(|candidate| candidate.split_whitespace().next())
                    .map(str::to_string),
            )
            .any(|url| is_remote(&url, local))
    };
    let settings = lol_html::Settings::new()
        .append_element_content_handler(lol_html::element!("[data-source-range]", |el| {
            el.remove_attribute("data-source-range");
            Ok(())
        }))
        .append_element_content_handler(lol_html::element!(
            "img, video, audio, source, track, picture, object, embed, input",
            move |el| {
                if fetches_elsewhere(el) {
                    let alt = el.get_attribute("alt").unwrap_or_default();
                    el.replace(&alt, lol_html::html_content::ContentType::Text);
                }
                Ok(())
            }
        ));
    let mut output = Vec::new();
    let mut rewriter = lol_html::HtmlRewriter::new(settings, |chunk: &[u8]| {
        output.extend_from_slice(chunk);
    });
    let rewritten = rewriter.write(html.as_bytes()).and(rewriter.end());
    match rewritten.map(|_| String::from_utf8(output)) {
        Ok(Ok(html)) => html,
        _ => html.to_string(),
    }
}

fn render(
    markdown: &str,
    base_path: &Path,
    options: &RenderOptions,
    with_toc: bool,
) -> Result<RenderResult> {
    let pipeline = run_pipeline(markdown, base_path, options, with_toc)?;

    let (html_output, images) = post_process_html_tags(
        &pipeline.raw_html,
        &pipeline.base_dir,
        &options.images,
        options.raw_html,
    );

    Ok(RenderResult {
        html: prepend_frontmatter(&pipeline.frontmatter_html, html_output),
        headings: pipeline.headings,
        images,
        reading: pipeline.reading,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_render_to_html_basic() {
        let markdown = "# Hello\n\nThis is a test.";
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(result.contains("<h1 data-source-range="));
        assert!(result.contains("Hello"));
        assert!(result.contains("<p data-source-range="));
        assert!(result.contains("This is a test."));
    }

    #[test]
    fn test_code_block_language_classes() {
        let markdown = indoc! {"
            # Code Blocks Test

            ```rust
            fn main() {
                println!(\"Hello\");
            }
            ```

            ```python
            def hello():
                print(\"world\")
            ```

            ```
            no language specified
            ```
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        let has_rust = result.contains("language-rust") || result.contains("class=\"rust\"");
        let has_python = result.contains("language-python") || result.contains("class=\"python\"");

        assert!(has_rust, "Should have rust language class: {result}");
        assert!(has_python, "Should have python language class: {result}");
    }

    #[test]
    fn test_render_to_html_with_alert() {
        let markdown = indoc! {"
            # Title

            > [!NOTE]
            > This is important
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(result.contains("markdown-alert-note"));
        assert!(result.contains("This is important"));
    }

    #[test]
    fn test_render_to_html_with_mermaid() {
        let markdown = indoc! {"
            ```mermaid
            graph LR
                A-->B
            ```
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(result.contains(r#"class="preprocessed-mermaid""#));
        assert!(result.contains("graph LR"));
    }

    #[test]
    fn test_render_to_html_with_math() {
        let markdown = indoc! {"
            # Math Test

            Inline math: $E = mc^2$

            Display math:
            $$
            \\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}
            $$
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(
            result.contains(r#"class="preprocessed-math-inline""#),
            "Should render inline math"
        );
        assert!(
            result.contains(r#"class="preprocessed-math-display""#),
            "Should render display math"
        );
        assert!(
            result.contains("data-original-content"),
            "Should include data attributes"
        );
    }

    #[test]
    fn test_render_to_html_integrated() {
        let temp_dir = TempDir::new().unwrap();

        // Create test image
        let image_path = temp_dir.path().join("image.png");
        let png_data = vec![0x89, 0x50, 0x4E, 0x47];
        fs::write(&image_path, png_data).unwrap();
        fs::write(temp_dir.path().join("other.md"), "# Other").unwrap();

        let markdown = indoc! {"
            # Test Document

            > [!WARNING]
            > Be careful

            ![Test Image](image.png)

            [Link to other doc](other.md)

            ```mermaid
            graph TD
                A-->B
            ```
        "};

        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(
            result.contains("<h1 data-source-range="),
            "Should render heading"
        );
        assert!(
            result.contains("markdown-alert-warning"),
            "Should render alert"
        );
        assert!(
            result.contains("data:image/png"),
            "Should convert image to data URL"
        );
        assert!(
            result.contains(r#"class="md-link""#),
            "Should convert md link"
        );
        assert!(
            result.contains(r#"class="preprocessed-mermaid""#),
            "Should render mermaid"
        );
    }

    #[test]
    fn test_render_to_html_with_frontmatter() {
        let markdown = indoc! {"
            ---
            title: My Document
            draft: false
            ---

            # Content Here
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(result.contains(r#"<details class="frontmatter""#));
        assert!(result.contains("<th>title</th>"));
        assert!(result.contains("<td>My Document</td>"));
        assert!(result.contains(r#"<span class="yaml-bool">false</span>"#));
        assert!(result.contains("Content Here</h1>"));

        let frontmatter_pos = result.find("frontmatter-table").unwrap();
        let heading_pos = result.find("<h1 ").unwrap();
        assert!(
            frontmatter_pos < heading_pos,
            "Frontmatter should appear before content"
        );
    }

    #[test]
    fn test_render_to_html_with_toc() {
        let markdown = indoc! {"
            # Title

            Some content

            ## Section 1

            More content
        "};

        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let rendered =
            render_to_html_with_toc(markdown, &md_path, &RenderOptions::default()).unwrap();
        let (html, headings) = (rendered.html, rendered.headings);

        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "Title");
        assert_eq!(headings[1].text, "Section 1");

        assert!(
            html.contains(r#"id="title""#),
            "H1 should have id attribute"
        );
        assert!(
            html.contains(r#"id="section-1""#),
            "H2 should have id attribute"
        );
        assert!(
            html.contains("data-source-range="),
            "Headings should have source ranges"
        );
    }

    // ========================================================================
    // Output equivalence characterization tests (Phase 0-2)
    // ========================================================================

    /// Characterization: render_to_html and render_to_html_with_toc produce
    /// equivalent HTML output except for heading IDs.
    /// This guarantees safety for Phase 3-1 common pipeline extraction.
    #[test]
    fn test_render_to_html_and_with_toc_produce_equivalent_output() {
        let temp = TempDir::new().unwrap();
        let md_path = temp.path().join("test.md");
        let markdown = indoc! {"
            # Heading 1

            Some paragraph with **bold** and `code`.

            ## Heading 2

            - list item 1
            - list item 2

            ```mermaid
            graph TD
                A --> B
            ```

            > [!NOTE]
            > This is a note
        "};

        let html_basic = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;
        let rendered =
            render_to_html_with_toc(markdown, &md_path, &RenderOptions::default()).unwrap();
        let (html_toc, headings) = (rendered.html, rendered.headings);

        // Strip heading IDs for comparison (without regex dependency)
        fn strip_heading_ids(s: &str) -> String {
            let mut result = s.to_string();
            while let Some(start) = result.find(" id=\"") {
                if let Some(end) = result[start + 5..].find('"') {
                    result.replace_range(start..start + 5 + end + 1, "");
                } else {
                    break;
                }
            }
            result
        }
        let stripped_basic = strip_heading_ids(&html_basic);
        let stripped_toc = strip_heading_ids(&html_toc);

        assert_eq!(
            stripped_basic, stripped_toc,
            "Both functions should produce identical HTML except for heading IDs"
        );

        // Verify TOC headings were extracted
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "Heading 1");
        assert_eq!(headings[1].text, "Heading 2");
    }

    // ========================================================================
    // Autolink integration tests
    // ========================================================================

    fn render_with_autolink(markdown: &str, base_path: &Path, auto_link_urls: bool) -> String {
        render_to_html(
            markdown,
            base_path,
            &RenderOptions {
                auto_link_urls,
                ..Default::default()
            },
        )
        .unwrap()
        .html
    }

    #[test]
    fn test_bare_url_becomes_link() {
        let markdown = "Visit https://example.com for info";
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_with_autolink(markdown, &md_path, true);

        assert!(
            result.contains(r#"<a href="https://example.com">https://example.com</a>"#),
            "Bare URL should become a link: {result}"
        );
    }

    #[test]
    fn test_bare_url_not_linked_when_disabled() {
        let markdown = "Visit https://example.com for info";
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_with_autolink(markdown, &md_path, false);

        assert!(
            !result.contains(r#"<a href"#),
            "Bare URL should NOT become a link when disabled: {result}"
        );
        assert!(
            result.contains("https://example.com"),
            "URL text should still be present: {result}"
        );
    }

    #[test]
    fn test_bare_url_in_code_block_not_linked() {
        let markdown = indoc! {"
            ```
            https://example.com
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_with_autolink(markdown, &md_path, true);

        assert!(
            !result.contains(r#"<a href"#),
            "URL inside code block should NOT become a link: {result}"
        );
    }

    #[test]
    fn test_bare_url_source_lines_preserved() {
        let markdown = indoc! {"
            # Title

            https://example.com

            After URL
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_with_autolink(markdown, &md_path, true);

        assert!(
            result.contains(r#"<h1 data-source-range="1:1-1:7">"#),
            "Heading should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-range="3:1-3:19">"#),
            "URL paragraph should be on line 3: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-range="5:1-5:9">"#),
            "After paragraph should be on line 5: {result}"
        );
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_render_to_html_empty_input() {
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html("", &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(
            result.is_empty() || result.trim().is_empty(),
            "Empty input should produce empty or whitespace-only output: '{result}'"
        );
    }

    #[test]
    fn test_render_to_html_frontmatter_only() {
        let markdown = "---\ntitle: Test\n---\n";
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(
            result.contains("frontmatter"),
            "Should render frontmatter table: {result}"
        );
        // Should not contain any markdown body elements
        assert!(!result.contains("<h1"), "Should have no heading: {result}");
    }

    #[test]
    fn test_render_to_html_consecutive_alerts() {
        let markdown = indoc! {"
            > [!NOTE]
            > First note

            > [!WARNING]
            > A warning

            > [!TIP]
            > A tip
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default())
            .unwrap()
            .html;

        assert!(
            result.contains("markdown-alert-note"),
            "Should contain note alert: {result}"
        );
        assert!(
            result.contains("markdown-alert-warning"),
            "Should contain warning alert: {result}"
        );
        assert!(
            result.contains("markdown-alert-tip"),
            "Should contain tip alert: {result}"
        );

        // Verify correct source lines for each alert
        assert!(
            result.contains(r#"data-source-range="1:1-"#),
            "First alert should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"data-source-range="4:1-"#),
            "Second alert should be on line 4: {result}"
        );
        assert!(
            result.contains(r#"data-source-range="7:1-"#),
            "Third alert should be on line 7: {result}"
        );
    }
}
