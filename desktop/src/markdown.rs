mod alerts;
mod event_processors;
mod frontmatter;
mod headings;
mod post_process;
mod source_lines;
pub(crate) mod source_maps;

pub use headings::*;

use alerts::process_github_alerts;
use anyhow::Result;
use event_processors::{extend_table_ranges, process_code_blocks, process_math_expressions};
use frontmatter::extract_and_render_frontmatter;
use headings::extract_headings;
use post_process::{post_process_html_tags, post_process_html_with_headings};
use pulldown_cmark::{html, Options, Parser};
use source_lines::{extract_table_source_lines, inject_source_lines};
use std::path::{Path, PathBuf};

/// Render Markdown to HTML
pub fn render_to_html(markdown: impl AsRef<str>, base_path: impl AsRef<Path>) -> Result<String> {
    let markdown = markdown.as_ref();
    let base_path = base_path.as_ref();

    // Enable GitHub Flavored Markdown options
    let options = Options::all();

    // Get base directory for resolving relative paths
    let base_dir = base_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Extract frontmatter if present
    let (frontmatter_html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

    // Process GitHub alerts (returns line origin mapping for correct source line tracking)
    let (processed_markdown, line_origins) = process_github_alerts(&content, frontmatter_lines);

    // Parse Markdown with offset tracking and process blocks
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
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Post-process HTML to handle all img, anchor, and table tags
    let html_output = post_process_html_tags(&html_output, base_dir.as_path(), &table_source_lines);

    // Prepend frontmatter table if present
    let final_output = if frontmatter_html.is_empty() {
        html_output
    } else {
        format!("{}\n{}", frontmatter_html, html_output)
    };

    Ok(final_output)
}

/// Render Markdown to HTML with TOC information
///
/// Returns a tuple of (rendered HTML with heading IDs, extracted headings)
pub fn render_to_html_with_toc(
    markdown: impl AsRef<str>,
    base_path: impl AsRef<Path>,
) -> Result<(String, Vec<HeadingInfo>)> {
    let markdown = markdown.as_ref();
    let base_path = base_path.as_ref();

    // Extract headings first
    let headings = extract_headings(markdown);

    // Enable GitHub Flavored Markdown options
    let options = Options::all();

    // Get base directory for resolving relative paths
    let base_dir = base_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Extract frontmatter if present
    let (frontmatter_html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

    // Process GitHub alerts (returns line origin mapping for correct source line tracking)
    let (processed_markdown, line_origins) = process_github_alerts(&content, frontmatter_lines);

    // Parse Markdown with offset tracking and process blocks
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
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Post-process HTML with heading IDs and table source lines
    let html_output = post_process_html_with_headings(
        &html_output,
        base_dir.as_path(),
        &headings,
        &table_source_lines,
    );

    // Prepend frontmatter table if present
    let final_output = if frontmatter_html.is_empty() {
        html_output
    } else {
        format!("{}\n{}", frontmatter_html, html_output)
    };

    Ok((final_output, headings))
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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let result = render_to_html(markdown, &md_path).unwrap();

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

        let (html, headings) = render_to_html_with_toc(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

        assert!(
            result.contains(r#"<h1 data-source-line="1">"#),
            "Heading should be on line 1: {result}"
        );
        assert!(
            result.contains(r#"<p data-source-line="3">"#),
            "Paragraph should be on line 3: {result}"
        );
        assert!(
            result.contains(r#"<ul data-source-line="5">"#),
            "List should be on line 5: {result}"
        );
        assert!(
            result.contains(r#"<li data-source-line="5">"#),
            "First item should be on line 5: {result}"
        );
        assert!(
            result.contains(r#"<li data-source-line="6">"#),
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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

        assert!(
            result.contains(
                r#"<pre data-source-line="3" data-source-line-start="4"><code class="language-rust">"#
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
        let result = render_to_html(markdown, &md_path).unwrap();

        assert!(
            result.contains(r#"<pre data-source-line="1" data-source-line-start="2">"#),
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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

        assert!(
            result.contains(r#"<ol data-source-line="1">"#),
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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

        assert!(
            result.contains(
                r#"<pre data-source-line="4" data-source-line-start="5"><code class="language-rust">"#
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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_render_to_html_empty_input() {
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");
        let result = render_to_html("", &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
        let result = render_to_html(markdown, &md_path).unwrap();

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
