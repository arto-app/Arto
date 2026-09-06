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
//! 0. **Line endings.** `\r\n` and lone `\r` become `\n`, which keeps the
//!    line count, so a CRLF file parses and reports lines like any other.
//! 1. **Frontmatter.** A leading YAML block is cut off and rendered to a
//!    `<details class="frontmatter">` table that is prepended to the
//!    output at the very end. The lines it occupied are added to every
//!    source line below it.
//! 2. **The engine** (`engine` module: everything that knows the parser).
//!    The body is parsed once and rendered with hooks that replace fenced
//!    `mermaid` and `math` blocks and `$…$` expressions by the
//!    `preprocessed-*` containers described below. A second pass over the
//!    rendered HTML turns the byte range on each block element into the
//!    `data-source-line` attributes described below, gives GitHub alerts
//!    the class names `github-markdown-css` styles, and keeps the heading
//!    ids when a table of contents was requested.
//! 3. **Post-processing** with lol_html: local images are inlined as data
//!    URLs and local Markdown links become `<span class="md-link">`.
//!
//! # HTML contract
//!
//! ## Source lines
//!
//! Block elements carry `data-source-line="N"`, the 1-based line of the
//! file where the block starts: `p`, `h1`–`h6`, `blockquote`, `hr`, `tr`
//! and `div.markdown-alert`. Blocks that span several lines also carry
//! `data-source-line-end="N"`: `ul`, `ol`, `li`, `table`, `pre` and
//! `div.preprocessed-math-display`. Code blocks additionally carry
//! `data-source-line-start="N"`, the line their content starts on (the
//! line after the fence, or the same line for an indented block), so the
//! frontend can count newlines inside `<code>` down to the exact line.
//!
//! The numbers are not monotonic in document order: footnote definitions
//! are moved to the section at the end while keeping the lines they were
//! written on, so that section reports lines from the middle of the file.
//!
//! Readers: `frontend/src/context-menu-handler.ts` (copy path with line)
//! and `frontend/src/content-cursor.ts` (keyboard cursor) resolve line
//! ranges from these; the app receives the range through
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
//! The class names are GitHub's, so `github-markdown-css` styles them; the
//! icon span is a placeholder for the frontend to fill in. The marker is
//! matched case-insensitively, so `[!note]` is an alert too.
//!
//! ## Footnotes
//!
//! References render as `<sup><a href="#fn-<id>" id="fnref-<id>">N</a></sup>`
//! and the definitions are collected into one
//! `<section class="footnotes"><ol>` at the end of the document, numbered in
//! the order they are first referenced. That is GitHub's shape, so
//! `github-markdown-css` styles it.
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
//! offline and in Quick Look; readers of `data-md-link` and `.md-link` are
//! the app and `frontend/style/components/content/markdown-viewer.css`.
//!
//! ## Headings
//!
//! [`render_to_html_with_toc`] returns [`HeadingInfo`] for every heading
//! and sets the same `id` on the rendered `h1`–`h6`, so the table of
//! contents (`crates/arto/src/components/right_sidebar/contents_tab.rs`)
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
//! `#fragment` stays on the end. Inside code, inside a link and inside the
//! math containers the brackets are text.
//!
//! ## Code blocks
//!
//! `<pre><code class="language-<lang>">`; the frontend highlights by that
//! class and `frontend/src/code-copy.ts` adds the copy button to every
//! `pre`.

mod engine;
mod frontmatter;
mod headings;
mod line_endings;
mod options;
mod post_process;

pub use engine::*;
pub use headings::*;
pub use options::*;

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

/// Render Markdown to HTML.
///
/// Relative links and images resolve against the directory of `base_path`.
pub fn render_to_html(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<String> {
    let pipeline = run_pipeline(markdown.as_ref(), base_path.as_ref(), options, false)?;

    let html_output = post_process_html_tags(&pipeline.raw_html, &pipeline.base_dir);

    Ok(prepend_frontmatter(&pipeline.frontmatter_html, html_output))
}

/// Render Markdown to HTML with TOC information
///
/// Returns a tuple of (rendered HTML with heading IDs, extracted headings)
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<(String, Vec<HeadingInfo>)> {
    let pipeline = run_pipeline(markdown.as_ref(), base_path.as_ref(), options, true)?;

    let html_output = post_process_html_tags(&pipeline.raw_html, &pipeline.base_dir);

    Ok((
        prepend_frontmatter(&pipeline.frontmatter_html, html_output),
        pipeline.headings,
    ))
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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(result.contains("<h1 data-source-line="));
        assert!(result.contains("Hello"));
        assert!(result.contains("<p data-source-line="));
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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains("<h1 data-source-line="),
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

        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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

        let (html, headings) =
            render_to_html_with_toc(markdown, &md_path, &RenderOptions::default()).unwrap();

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
            html.contains("data-source-line="),
            "Headings should have source line attributes"
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

        let html_basic = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();
        let (html_toc, headings) =
            render_to_html_with_toc(markdown, &md_path, &RenderOptions::default()).unwrap();

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
    // Source line annotation integration tests
    // ========================================================================

    #[test]
    fn test_source_line_basic_elements() {
        let markdown = indoc! {"
            # Heading

            Paragraph text.

            - item1
            - item2
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<h1 data-source-line="1">"#),
            "Heading should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="3">"#),
            "Paragraph should be on line 3: {result}"
        );
        assert!(
            result.contains(r#"<ul data-source-line="5""#),
            "List should be on line 5: {result}"
        );
        assert!(
            result.contains(r#"<li data-source-line="5""#),
            "First item should be on line 5: {result}"
        );
        assert!(
            result.contains(r#"<li data-source-line="6""#),
            "Second item should be on line 6: {result}"
        );
    }

    #[test]
    fn test_source_line_with_frontmatter() {
        let markdown = indoc! {"
            ---
            title: Test
            ---

            # Heading

            Content here.
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<h1 data-source-line="5">"#),
            "Heading should be on line 5 (after frontmatter): {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="7">"#),
            "Paragraph should be on line 7: {result}"
        );
    }

    #[test]
    fn test_source_line_code_block() {
        let markdown = indoc! {"
            # Title

            ```rust
            fn main() {}
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(
                r#"<pre data-source-line="3" data-source-line-end="5" data-source-line-start="4"><code class="language-rust">"#
            ),
            "Code block should be on line 3 with content starting at line 4: {result}"
        );
    }

    #[test]
    fn test_source_line_code_block_multiline() {
        let markdown = indoc! {"
            ```rust
            fn main() {
                println!();
            }
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(
                r#"<pre data-source-line="1" data-source-line-end="5" data-source-line-start="2">"#
            ),
            "Code block should start at line 1 with content at line 2: {result}"
        );
    }

    #[test]
    fn test_source_line_blockquote() {
        let markdown = indoc! {"
            # Title

            > This is a quote
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<blockquote data-source-line="3">"#),
            "Blockquote should be on line 3: {result}"
        );
    }

    #[test]
    fn test_source_line_hr() {
        let markdown = indoc! {"
            Above

            ---

            Below
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<hr data-source-line="3">"#),
            "HR should be on line 3: {result}"
        );
    }

    #[test]
    fn test_source_line_ordered_list() {
        let markdown = indoc! {"
            1. first
            2. second
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<ol data-source-line="1""#),
            "Ordered list should be on line 1: {result}"
        );
    }

    #[test]
    fn test_source_line_table() {
        let markdown = indoc! {"
            | A | B |
            |---|---|
            | 1 | 2 |
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"data-source-line="1""#),
            "Table should have source line: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="3""#),
            "Table should have source line end: {result}"
        );
        assert!(result.contains("<th"), "Table head should render: {result}");
        assert!(result.contains("<td"), "Table data should render: {result}");
        assert!(
            result.contains(r#"<tr data-source-line="#),
            "Table rows should have source line: {result}"
        );
    }

    #[test]
    fn test_source_line_table_multirow() {
        let markdown = indoc! {"
            | A | B |
            |---|---|
            | 1 | 2 |
            | 3 | 4 |
            | 5 | 6 |
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<tr data-source-line="3">"#),
            "First body row should be line 3: {result}"
        );
        assert!(
            result.contains(r#"<tr data-source-line="4">"#),
            "Second body row should be line 4: {result}"
        );
        assert!(
            result.contains(r#"<tr data-source-line="5">"#),
            "Third body row should be line 5: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="5""#),
            "Table should span to line 5: {result}"
        );
    }

    #[test]
    fn test_source_line_alert_content() {
        let markdown = indoc! {"
            > [!NOTE]
            > This is a note
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"data-source-line="1""#),
            "Alert div should have source line 1: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="2">"#),
            "Alert content paragraph should have source line 2: {result}"
        );
    }

    #[test]
    fn test_source_line_after_alert() {
        let markdown = indoc! {"
            > [!NOTE]
            > This is a note

            # Heading After Alert

            Paragraph after alert.
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"<h1 data-source-line="4">"#),
            "Heading after alert should be on line 4: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="6">"#),
            "Paragraph after alert should be on line 6: {result}"
        );
    }

    #[test]
    fn test_source_line_code_block_after_alert() {
        let markdown = indoc! {"
            > [!TIP]
            > Some tip

            ```rust
            fn main() {}
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(
                r#"<pre data-source-line="4" data-source-line-end="6" data-source-line-start="5"><code class="language-rust">"#
            ),
            "Code block after alert should be on line 4 with content at line 5: {result}"
        );
    }

    // ========================================================================
    // Source line annotation tests for preprocessed blocks
    // ========================================================================

    #[test]
    fn test_source_line_mermaid_block() {
        let markdown = indoc! {"
            # Title

            ```mermaid
            graph TD
                A-->B
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"data-source-line="3""#),
            "Mermaid block should have data-source-line: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="6""#),
            "Mermaid block should have data-source-line-end: {result}"
        );
    }

    #[test]
    fn test_source_line_math_display() {
        let markdown = indoc! {"
            # Title

            $$
            x = \\frac{-b}{2a}
            $$
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"data-source-line="3""#),
            "Display math should have data-source-line: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="5""#),
            "Display math should have data-source-line-end: {result}"
        );
    }

    #[test]
    fn test_source_line_math_block() {
        let markdown = indoc! {"
            # Title

            ```math
            E = mc^2
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        assert!(
            result.contains(r#"data-source-line="3""#),
            "Math code block should have data-source-line: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="5""#),
            "Math code block should have data-source-line-end: {result}"
        );
    }

    // ========================================================================
    // New integration tests
    // ========================================================================

    #[test]
    fn test_source_line_table_with_frontmatter() {
        let markdown = indoc! {"
            ---
            title: Test
            ---

            | A | B |
            |---|---|
            | 1 | 2 |
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        // Table starts on line 5 of original (after 4 frontmatter lines)
        assert!(
            result.contains(r#"<table data-source-line="5""#),
            "Table should be on line 5 after frontmatter: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="7""#),
            "Table should end on line 7: {result}"
        );
    }

    #[test]
    fn test_source_line_mermaid_after_alert() {
        let markdown = indoc! {"
            > [!NOTE]
            > Some note

            ```mermaid
            graph TD
                A-->B
            ```
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        // Mermaid block starts on line 4 of original
        assert!(
            result.contains(r#"data-source-line="4""#),
            "Mermaid block after alert should have correct source line: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="7""#),
            "Mermaid block should have correct end line: {result}"
        );
    }

    #[test]
    fn test_source_line_multiple_tables() {
        let markdown = indoc! {"
            | A |
            |---|
            | 1 |

            | X |
            |---|
            | Y |
        "};
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

        // First table: lines 1-3
        assert!(
            result.contains(r#"<table data-source-line="1""#),
            "First table should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="3""#),
            "First table should end on line 3: {result}"
        );
        // Second table: lines 5-7
        assert!(
            result.contains(r#"<table data-source-line="5""#),
            "Second table should be on line 5: {result}"
        );
        assert!(
            result.contains(r#"data-source-line-end="7""#),
            "Second table should end on line 7: {result}"
        );
    }

    // ========================================================================
    // Autolink integration tests
    // ========================================================================

    fn render_with_autolink(markdown: &str, base_path: &Path, auto_link_urls: bool) -> String {
        render_to_html(markdown, base_path, &RenderOptions { auto_link_urls }).unwrap()
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
            result.contains(r#"<h1 data-source-line="1">"#),
            "Heading should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="3">"#),
            "URL paragraph should be on line 3: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="5">"#),
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
        let result = render_to_html("", &md_path, &RenderOptions::default()).unwrap();

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
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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
        let result = render_to_html(markdown, &md_path, &RenderOptions::default()).unwrap();

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
            result.contains(r#"data-source-line="1""#),
            "First alert should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"data-source-line="4""#),
            "Second alert should be on line 4: {result}"
        );
        assert!(
            result.contains(r#"data-source-line="7""#),
            "Third alert should be on line 7: {result}"
        );
    }
}
