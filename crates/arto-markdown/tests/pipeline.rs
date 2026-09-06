//! Behaviour of the rendering pipeline seen through its public API.
//!
//! These tests describe what the rendered HTML looks like for each
//! construct, not how the engine produces it, so they hold across an
//! engine swap. Internals that are only observable at the engine level
//! (event streams, offset mapping) are tested next to that code.

use arto_markdown::{render_to_html, render_to_html_with_toc, HeadingInfo, RenderOptions};
use indoc::indoc;
use std::path::Path;

fn render(markdown: &str) -> String {
    render_to_html(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
}

fn headings(markdown: &str) -> Vec<HeadingInfo> {
    render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .1
}

/// Whether some `<tag …>` start tag in `html` carries every attribute in
/// `attrs`, in any order.
fn has_element(html: &str, tag: &str, attrs: &[(&str, &str)]) -> bool {
    html.match_indices(&format!("<{tag} ")).any(|(pos, _)| {
        let end = html[pos..].find('>').map_or(html.len(), |end| pos + end);
        let start_tag = &html[pos..end];
        attrs
            .iter()
            .all(|(name, value)| start_tag.contains(&format!(r#" {name}="{value}""#)))
    })
}

// ----------------------------------------------------------------------
// Line endings
// ----------------------------------------------------------------------

#[test]
fn a_crlf_document_renders_like_the_same_document_in_lf() {
    // A Windows checkout hands the reader CRLF. Every construct here is one
    // whose parse depends on where a line ends.
    let lf = indoc! {"
        Paragraph before the rule.

        ---

        # Heading

        - item one
        - item two

        ```rust
        fn main() {}
        ```

        > [!NOTE]
        > A note body.
    "};
    let crlf = lf.replace('\n', "\r\n");

    assert_eq!(render(&crlf), render(lf));
    // The rule must stay a rule: read as a setext underline it would close
    // the paragraph above it as a heading instead.
    assert!(render(&crlf).contains("<hr data-source-line=\"3\">"));
}

#[test]
fn a_crlf_selection_maps_back_to_its_source() {
    use arto_markdown::extract_source_selection;

    assert_eq!(
        extract_source_selection("intro\r\n\r\nsome **word** here\r\n", "word"),
        Some("**word**".to_string())
    );
}

// ----------------------------------------------------------------------
// Wiki links
// ----------------------------------------------------------------------

#[test]
fn a_wiki_link_becomes_a_document_link() {
    // The href gets `.md`, so the post-processing pass turns it into an
    // in-app link like any other document link.
    let html = render("See [[README]] and [[README|the index]].");

    assert!(html.contains(r#"data-md-link="README.md""#), "{html}");
    assert!(html.contains(">README</span>"), "{html}");
    assert!(html.contains(">the index</span>"), "{html}");
    assert!(!html.contains("[["), "{html}");
}

#[test]
fn a_wiki_link_inside_code_is_left_alone() {
    let html = render("Inline `[[README]]` and:\n\n```\n[[README]]\n```\n");

    assert_eq!(html.matches("[[README]]").count(), 2, "{html}");
    assert!(!html.contains("<a href"), "{html}");
}

#[test]
fn a_wiki_link_survives_being_split_across_text_chunks() {
    // The `&` forces lol_html to hand the text over in several chunks; the
    // link must still be found across the boundary.
    let html = render("a &amp; b [[Guide]] c");

    assert!(html.contains(r#"data-md-link="Guide.md""#), "{html}");
    assert!(html.contains("a &amp; b "), "{html}");
}

// ----------------------------------------------------------------------
// GitHub alerts
// ----------------------------------------------------------------------

#[test]
fn alert_note_renders_title_and_body() {
    let html = render(indoc! {"
        > [!NOTE]
        > This is a note
    "});

    assert!(html.contains(r#"<div class="markdown-alert markdown-alert-note""#));
    assert!(html.contains(r#"<p class="markdown-alert-title""#));
    assert!(html.contains(r#"<span class="alert-icon" data-alert-type="note"></span>NOTE"#));
    assert!(html.contains("This is a note"));
    assert!(html.contains("</div>"));
}

#[test]
fn alert_warning_uses_its_own_kind() {
    let html = render(indoc! {"
        > [!WARNING]
        > Be careful!
    "});

    assert!(html.contains("markdown-alert-warning"));
    assert!(html.contains(r#"data-alert-type="warning""#));
    assert!(html.contains("WARNING"));
    assert!(html.contains("Be careful!"));
}

#[test]
fn alert_keeps_every_quoted_line() {
    let html = render(indoc! {"
        > [!IMPORTANT]
        > First line
        > Second line
        > Third line
    "});

    assert!(html.contains("markdown-alert-important"));
    assert!(html.contains("First line"));
    assert!(html.contains("Second line"));
    assert!(html.contains("Third line"));
}

#[test]
fn every_alert_kind_is_recognized() {
    for (name, class) in [
        ("NOTE", "note"),
        ("TIP", "tip"),
        ("IMPORTANT", "important"),
        ("WARNING", "warning"),
        ("CAUTION", "caution"),
    ] {
        let html = render(&format!("> [!{name}]\n> Test content"));
        assert!(
            html.contains(&format!("markdown-alert-{class}")),
            "{name}: {html}"
        );
        assert!(html.contains(name), "{name}: {html}");
    }
}

#[test]
fn plain_blockquote_is_not_an_alert() {
    let html = render("Regular paragraph\n> Regular quote");

    assert!(!html.contains("markdown-alert"), "{html}");
    assert!(
        html.contains(r#"<blockquote data-source-line="2">"#),
        "{html}"
    );
    assert!(html.contains("Regular quote"));
}

// ----------------------------------------------------------------------
// Mermaid and math containers
// ----------------------------------------------------------------------

#[test]
fn mermaid_block_becomes_a_preprocessed_pre() {
    let html = render(indoc! {"
        ```mermaid
        graph TD
            A-->B
        ```
    "});

    assert!(html.contains(r#"class="preprocessed-mermaid""#), "{html}");
    // The attribute holds the diagram source; the newlines in it are
    // escaped, so the value survives as one attribute.
    assert!(
        html.contains(r#"data-original-content="graph TD&#10;    A--&gt;B&#10;""#),
        "{html}"
    );
    assert!(html.contains("</pre>"));
    assert!(!html.contains("language-mermaid"), "{html}");
}

#[test]
fn inline_math_becomes_a_preprocessed_span() {
    let html = render("This is inline math: $x = y + z$");

    assert!(
        html.contains(r#"<span class="preprocessed-math-inline" data-original-content="x = y + z">x = y + z</span>"#),
        "{html}"
    );
}

#[test]
fn display_math_becomes_a_preprocessed_div() {
    let html = render(indoc! {"
        Display math:

        $$
        x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}
        $$
    "});

    assert!(
        html.contains(r#"class="preprocessed-math-display""#),
        "{html}"
    );
    assert!(html.contains("data-original-content"), "{html}");
    assert!(html.contains("frac"), "{html}");
}

#[test]
fn display_math_in_the_middle_of_a_line_stays_inline() {
    // `$$…$$` is a block construct: written inside a paragraph it is read
    // as inline math, so both formulas here land in the inline container.
    let html = render("Inline $a + b$ and display $$c = d$$");

    assert_eq!(
        html.matches(r#"class="preprocessed-math-inline""#).count(),
        2,
        "{html}"
    );
    assert!(
        !html.contains(r#"class="preprocessed-math-display""#),
        "{html}"
    );
}

#[test]
fn empty_mermaid_block_still_becomes_a_container() {
    let html = render("```mermaid\n```");

    assert!(html.contains(r#"class="preprocessed-mermaid""#), "{html}");
    assert!(html.contains(r#"data-original-content="""#), "{html}");
}

#[test]
fn other_languages_stay_code_blocks() {
    let html = render("```python\nprint('hello')\n```");

    assert!(!html.contains("preprocessed"), "{html}");
    assert!(html.contains(r#"<code class="language-python">"#), "{html}");
}

#[test]
fn mermaid_source_lines_cover_both_fences() {
    let html = render("```mermaid\ngraph TD\n```");

    assert!(
        has_element(
            &html,
            "pre",
            &[
                ("class", "preprocessed-mermaid"),
                ("data-source-line", "1"),
                ("data-source-line-end", "3"),
            ]
        ),
        "{html}"
    );
}

#[test]
fn empty_display_math_does_not_break_rendering() {
    let html = render("$$$$");

    // Whether the engine reports an empty display formula is its call; the
    // pipeline must survive either way.
    if html.contains("preprocessed") {
        assert!(html.contains("preprocessed-math-display"), "{html}");
    }
}

// ----------------------------------------------------------------------
// Tables
// ----------------------------------------------------------------------

#[test]
fn paragraphs_without_tables_keep_their_lines() {
    let html = render("Just a paragraph\n\nAnother one");

    assert!(
        html.contains(r#"<p data-source-line="1">Just a paragraph</p>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p data-source-line="3">Another one</p>"#),
        "{html}"
    );
    assert!(!html.contains("<table"), "{html}");
}

#[test]
fn table_range_extends_to_its_last_row() {
    let html = render("| A | B |\n|---|---|\n| 1 | 2 |");

    assert!(
        has_element(
            &html,
            "table",
            &[("data-source-line", "1"), ("data-source-line-end", "3")]
        ),
        "{html}"
    );
}

#[test]
fn each_table_gets_its_own_range() {
    let html = render("| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |\n\n| P |\n|---|\n| Q |");

    for (start, end) in [("1", "3"), ("5", "7"), ("9", "11")] {
        assert!(
            has_element(
                &html,
                "table",
                &[("data-source-line", start), ("data-source-line-end", end)]
            ),
            "table {start}-{end}: {html}"
        );
    }
}

#[test]
fn header_only_table_has_a_range() {
    let html = render("| A | B |\n|---|---|");

    assert!(
        has_element(
            &html,
            "table",
            &[("data-source-line", "1"), ("data-source-line-end", "2")]
        ),
        "{html}"
    );
    assert!(html.contains("<thead>"), "{html}");
}

// ----------------------------------------------------------------------
// Headings and the table of contents
// ----------------------------------------------------------------------

#[test]
fn headings_are_listed_with_levels_and_slugs() {
    let headings = headings(indoc! {"
        # Title

        Some content

        ## Section 1

        More content

        ### Subsection 1.1

        Even more content

        ## Section 2
    "});

    assert_eq!(
        headings,
        vec![
            HeadingInfo {
                level: 1,
                text: "Title".to_string(),
                id: "title".to_string()
            },
            HeadingInfo {
                level: 2,
                text: "Section 1".to_string(),
                id: "section-1".to_string()
            },
            HeadingInfo {
                level: 3,
                text: "Subsection 1.1".to_string(),
                id: "subsection-1-1".to_string()
            },
            HeadingInfo {
                level: 2,
                text: "Section 2".to_string(),
                id: "section-2".to_string()
            },
        ]
    );
}

#[test]
fn duplicate_headings_get_numbered_ids() {
    let headings = headings(indoc! {"
        # Introduction

        ## Overview

        Content

        ## Overview

        More content

        ## Overview
    "});

    let ids: Vec<&str> = headings.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(
        ids,
        ["introduction", "overview", "overview-1", "overview-2"]
    );
}

#[test]
fn heading_ids_are_written_only_when_a_toc_is_requested() {
    let markdown = "# Title\n\n## Section\n";
    let (with_toc, headings) = render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");
    let plain = render(markdown);

    assert_eq!(headings.len(), 2);
    assert!(
        with_toc.contains(r#"<h1 data-source-line="1" id="title">"#),
        "{with_toc}"
    );
    assert!(
        with_toc.contains(r#"<h2 data-source-line="3" id="section">"#),
        "{with_toc}"
    );
    assert!(plain.contains(r#"<h1 data-source-line="1">"#), "{plain}");
    assert!(plain.contains(r#"<h2 data-source-line="3">"#), "{plain}");
    assert!(!plain.contains(" id="), "{plain}");
}

#[test]
fn heading_ids_keep_unicode_text() {
    let markdown = "## 日本語の見出し\n\n## 日本語の見出し\n";
    let ids: Vec<String> = headings(markdown).into_iter().map(|h| h.id).collect();

    assert_eq!(ids, ["日本語の見出し", "日本語の見出し-1"]);
}

#[test]
fn a_heading_inside_a_footnote_is_not_listed_in_the_outline() {
    // Footnote bodies are rendered at the end of the document, not where
    // they were written, so listing their headings would pair every later
    // entry of the outline with the wrong anchor.
    let markdown = indoc! {"
        Ref[^a]

        [^a]: A note

            # In the footnote

        # After
    "};

    let listed: Vec<(String, String)> = headings(markdown)
        .into_iter()
        .map(|heading| (heading.text, heading.id))
        .collect();

    assert_eq!(listed, [("After".to_string(), "after".to_string())]);
}

#[test]
fn headings_inside_raw_html_do_not_shift_ids() {
    // A heading written as HTML is not a Markdown heading: it gets no id
    // and must not consume the id of the Markdown heading after it.
    let markdown = "<h2>Raw</h2>\n\n## Real\n";
    let (with_toc, headings) = render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");

    let texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert_eq!(texts, ["Real"]);
    assert!(with_toc.contains("<h2>Raw</h2>"), "{with_toc}");
    assert!(
        with_toc.contains(r#"<h2 data-source-line="3" id="real">"#),
        "{with_toc}"
    );
}

#[test]
fn a_document_without_headings_has_an_empty_toc() {
    let (html, headings) = render_to_html_with_toc(
        "Just text.",
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");

    assert!(headings.is_empty());
    assert!(html.contains("Just text."));
    assert!(!html.contains(" id="), "{html}");
}

#[test]
fn frontmatter_is_not_a_heading() {
    let headings = headings(indoc! {"
        ---
        title: Test
        ---

        # Heading After Frontmatter

        Content
    "});

    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].text, "Heading After Frontmatter");
}

#[test]
fn invalid_frontmatter_stays_content() {
    // Only a YAML mapping is metadata. Anything else is prose the reader
    // must still see, even at the cost of rendering the fences as a rule
    // and a setext heading.
    let markdown = indoc! {"
        ---
        invalid: [unclosed
        ---

        # Heading After Invalid Frontmatter
    "};

    assert!(!render(markdown).contains("frontmatter"), "{markdown}");

    let texts: Vec<String> = headings(markdown).into_iter().map(|h| h.text).collect();
    assert_eq!(
        texts,
        ["invalid: [unclosed", "Heading After Invalid Frontmatter"]
    );
}

// ----------------------------------------------------------------------
// Source lines
// ----------------------------------------------------------------------

#[test]
fn paragraph_carries_its_line() {
    let html = render("Hello world");
    assert!(html.contains(r#"<p data-source-line="1">"#), "{html}");
}

#[test]
fn heading_attributes_survive_next_to_the_line() {
    let html = render("# Title {#my-id .my-class}");

    assert!(
        html.contains(r#"<h1 data-source-line="1" id="my-id" class="my-class">Title</h1>"#),
        "{html}"
    );
}

#[test]
fn an_authored_heading_id_wins_over_the_generated_one() {
    let markdown = "## Section {#custom}\n";
    let headings = headings(markdown);

    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].text, "Section");
    assert_eq!(headings[0].id, "custom");
    // An id the document asked for by name is content, so it is there even
    // without a table of contents.
    assert!(render(markdown).contains(r#"id="custom""#));
}

#[test]
fn a_heading_with_only_classes_still_gets_an_id() {
    let html = render("### 日本語の見出し {.highlight}");

    assert!(
        html.contains(r#"id="日本語の見出し" class="highlight">日本語の見出し</h3>"#),
        "{html}"
    );
}

#[test]
fn braces_that_are_not_an_attribute_block_stay_text() {
    let html = render("# What {this means}");

    assert!(html.contains("What {this means}</h1>"), "{html}");
}

#[test]
fn fenced_code_block_content_starts_after_the_fence() {
    let html = render("```rust\nfn main() {}\n```");

    assert!(
        html.contains(
            r#"<pre data-source-line="1" data-source-line-end="3" data-source-line-start="2"><code class="language-rust">"#
        ),
        "{html}"
    );
}

#[test]
fn indented_code_block_content_starts_on_its_own_line() {
    let html = render("    fn main() {}\n    let x = 1;");

    assert!(
        html.contains(r#"<pre data-source-line="1" data-source-line-end="2" data-source-line-start="1"><code>"#),
        "{html}"
    );
}

#[test]
fn blockquote_and_alert_both_carry_lines() {
    let html = render("> plain quote\n\n> [!NOTE]\n> This is a note");

    assert!(
        html.contains(r#"<blockquote data-source-line="1">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<div class="markdown-alert markdown-alert-note" data-source-line="3""#),
        "{html}"
    );
}

#[test]
fn an_alert_without_a_body_does_not_shift_the_paragraph_after_it() {
    let html = render(indoc! {"
        > [!NOTE]

        first line
        second ] line
    "});

    assert!(
        html.contains(r#"<p data-source-line="3">first line"#),
        "{html}"
    );
}

#[test]
fn an_alert_body_that_starts_on_its_own_line_keeps_that_line() {
    let html = render(indoc! {"
        > [!NOTE]
        >
        > first line
        > second ] line
    "});

    assert!(
        html.contains(r#"<p data-source-line="3">first line"#),
        "{html}"
    );
}

#[test]
fn lists_and_items_carry_start_and_end_lines() {
    let html = render("- a\n- b\n\n1. x\n2. y");

    assert!(
        html.contains(r#"<ul data-source-line="1" data-source-line-end="2">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<li data-source-line="1" data-source-line-end="1">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<li data-source-line="2" data-source-line-end="2">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<ol data-source-line="4" data-source-line-end="5">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<li data-source-line="5" data-source-line-end="5">"#),
        "{html}"
    );
}

#[test]
fn table_rows_carry_lines_and_header_cells_keep_alignment() {
    let html = render("| A | B |\n|:--|--:|\n| 1 | 2 |");

    assert!(html.contains(r#"<tr data-source-line="3">"#), "{html}");
    // Alignment travels on the `align` attribute GitHub also emits, on the
    // header and the body cells alike.
    assert!(html.contains(r#"<th align="left">A</th>"#), "{html}");
    assert!(html.contains(r#"<th align="right">B</th>"#), "{html}");
    assert!(html.contains(r#"<td align="left">1</td>"#), "{html}");
    assert!(html.contains(r#"<td align="right">2</td>"#), "{html}");
}

#[test]
fn tables_are_numbered_in_document_order() {
    let html = render("| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |");

    let first = html
        .find(r#"<table data-source-line="1""#)
        .expect("first table");
    let second = html
        .find(r#"<table data-source-line="5""#)
        .expect("second table");
    assert!(first < second, "{html}");
}

#[test]
fn rule_carries_its_line() {
    let html = render("Above\n\n---\n\nBelow");
    assert!(html.contains(r#"<hr data-source-line="3">"#), "{html}");
}

#[test]
fn mermaid_container_carries_a_range() {
    let html = render("# Title\n\n```mermaid\ngraph TD\n    A-->B\n```");

    assert!(
        has_element(
            &html,
            "pre",
            &[
                ("class", "preprocessed-mermaid"),
                ("data-source-line", "3"),
                ("data-source-line-end", "6"),
            ]
        ),
        "{html}"
    );
}

#[test]
fn display_math_container_carries_a_range() {
    let html = render("# Title\n\n$$\nx = 1\n$$");

    assert!(
        has_element(
            &html,
            "div",
            &[
                ("class", "preprocessed-math-display"),
                ("data-source-line", "3"),
                ("data-source-line-end", "5"),
            ]
        ),
        "{html}"
    );
}

#[test]
fn inline_markup_passes_through_untouched() {
    let html = render("Hello **bold** world");
    assert!(
        html.contains(r#"<p data-source-line="1">Hello <strong>bold</strong> world</p>"#),
        "{html}"
    );
}

#[test]
fn lines_after_an_alert_and_frontmatter_point_at_the_original_file() {
    // The alert is rewritten to several HTML lines before parsing and the
    // frontmatter is cut off; neither may shift the lines reported for the
    // blocks that follow.
    let html = render(indoc! {"
        ---
        title: Test
        ---

        # Title

        > [!NOTE]
        > Paragraph A

        Paragraph B
    "});

    assert!(html.contains(r#"<h1 data-source-line="5">"#), "{html}");
    assert!(
        html.contains(r#"markdown-alert-note" data-source-line="7""#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p data-source-line="8">Paragraph A</p>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p data-source-line="10">Paragraph B</p>"#),
        "{html}"
    );
}
