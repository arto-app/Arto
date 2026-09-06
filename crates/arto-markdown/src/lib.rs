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
//! 1. **Frontmatter.** A leading YAML block is cut off and rendered to a
//!    `<details class="frontmatter">` table that is prepended to the
//!    output at the very end. The lines it occupied are added to every
//!    source line below it.
//! 2. **Text pre-processing.** Bare URLs become `<URL>` autolinks (when
//!    [`RenderOptions::auto_link_urls`] is set) and GitHub alerts
//!    (`> [!NOTE]`) are rendered to HTML before the parser sees them. The
//!    first keeps the line count; the second records which original line
//!    each processed line came from, so source lines stay correct.
//! 3. **Parsing and rendering.** pulldown-cmark with every extension.
//!    Fenced `mermaid` and `math` blocks and `$…$` expressions are replaced
//!    by the `preprocessed-*` containers described below, and block
//!    elements receive their `data-source-line` attributes.
//! 4. **Post-processing** with lol_html: local images are inlined as data
//!    URLs, local Markdown links become `<span class="md-link">`, tables get
//!    their source lines, and headings get their ids when a table of
//!    contents was requested.
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
//! icon span is filled in by the frontend.
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
//! in `crates/arto/src/components/content/file_viewer.rs`. Links to files
//! that are not Markdown add `md-link-invalid`. `http(s)` links stay
//! anchors. Local images are inlined as `data:` URLs so the page works
//! offline and in Quick Look; readers of `data-md-link` and `.md-link` are
//! the app and `frontend/style/components/content/markdown-viewer.css`.
//!
//! ## Headings
//!
//! [`render_to_html_with_toc`] returns [`HeadingInfo`] for every heading
//! and sets the same `id` on the rendered `h1`–`h6`, so the table of
//! contents (`crates/arto/src/components/right_sidebar/contents_tab.rs`)
//! can scroll to it with `getElementById`. The id is derived from the
//! heading text and replaces an explicit `{#id}` attribute.
//! [`render_to_html`] adds no ids; only an explicit `{#id}` survives.
//!
//! ## Code blocks
//!
//! `<pre><code class="language-<lang>">` as pulldown-cmark writes it; the
//! frontend highlights by that class and `frontend/src/code-copy.ts` adds
//! the copy button to every `pre`.

mod alerts;
mod autolinks;
mod event_processors;
mod frontmatter;
mod headings;
mod options;
mod post_process;
mod source_extract;
mod source_lines;

pub use headings::*;
pub use options::*;
pub use source_extract::*;

use alerts::process_github_alerts;
use anyhow::Result;
use event_processors::{extend_table_ranges, process_code_blocks, process_math_expressions};
use frontmatter::extract_and_render_frontmatter;
use headings::extract_headings;
use post_process::{post_process_html_tags, post_process_html_with_headings};
use pulldown_cmark::{html, Options, Parser};
use source_lines::{extract_table_source_lines, inject_source_lines};
use std::path::{Path, PathBuf};

/// Intermediate result from the common markdown parsing pipeline.
///
/// Contains all data needed for post-processing, allowing the two public
/// render functions to share the parsing logic while differing only in
/// how they post-process the raw HTML output.
struct PipelineResult {
    raw_html: String,
    frontmatter_html: String,
    base_dir: PathBuf,
    table_source_lines: Vec<(usize, usize)>,
}

/// Run the common markdown parsing pipeline: frontmatter extraction,
/// GitHub alert preprocessing, pulldown-cmark parsing, source line
/// injection, and HTML generation.
fn run_pipeline(markdown: &str, base_path: &Path, auto_link_urls: bool) -> PipelineResult {
    let base_dir = base_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Extract frontmatter if present
    let (frontmatter_html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

    // Convert bare URLs to <URL> autolinks before any parsing (if enabled)
    let content = if auto_link_urls {
        autolinks::preprocess_autolinks(&content)
    } else {
        content
    };

    // Process GitHub alerts (returns line origin mapping for correct source line tracking)
    let (processed_markdown, line_origins) = process_github_alerts(&content, frontmatter_lines);

    // Parse Markdown with offset tracking and process blocks
    let options = Options::all();
    let parser = Parser::new_ext(&processed_markdown, options).into_offset_iter();
    let parser = extend_table_ranges(parser);
    let parser = process_code_blocks(parser, "mermaid");
    let parser = process_code_blocks(parser, "math");
    let parser = process_math_expressions(parser);

    // Collect events to extract table source lines before inject_source_lines consumes ranges
    let events: Vec<_> = parser.collect();
    let table_source_lines = extract_table_source_lines(
        &events,
        &processed_markdown,
        &line_origins,
        frontmatter_lines,
    );

    let parser = inject_source_lines(
        events.into_iter(),
        &processed_markdown,
        &line_origins,
        frontmatter_lines,
    );

    // Convert to HTML
    let mut raw_html = String::new();
    html::push_html(&mut raw_html, parser);

    PipelineResult {
        raw_html,
        frontmatter_html,
        base_dir,
        table_source_lines,
    }
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
    let pipeline = run_pipeline(
        markdown.as_ref(),
        base_path.as_ref(),
        options.auto_link_urls,
    );

    let html_output = post_process_html_tags(
        &pipeline.raw_html,
        &pipeline.base_dir,
        &pipeline.table_source_lines,
    );

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
    let markdown = markdown.as_ref();
    let headings = extract_headings(markdown, options.auto_link_urls);
    let pipeline = run_pipeline(markdown, base_path.as_ref(), options.auto_link_urls);

    let html_output = post_process_html_with_headings(
        &pipeline.raw_html,
        &pipeline.base_dir,
        &headings,
        &pipeline.table_source_lines,
    );

    Ok((
        prepend_frontmatter(&pipeline.frontmatter_html, html_output),
        headings,
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
            result.contains(r#"<hr data-source-line="3" />"#),
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
