//! Headings and the table of contents

use super::*;

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
    let rendered = render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");
    let (with_toc, headings) = (rendered.html, rendered.headings);
    let plain = render(markdown);

    assert_eq!(headings.len(), 2);
    assert!(
        with_toc.contains(r#"<h1 data-source-range="1:1-1:7" id="title">"#),
        "{with_toc}"
    );
    assert!(
        with_toc.contains(r#"<h2 data-source-range="3:1-3:10" id="section">"#),
        "{with_toc}"
    );
    assert!(
        plain.contains(r#"<h1 data-source-range="1:1-1:7">"#),
        "{plain}"
    );
    assert!(
        plain.contains(r#"<h2 data-source-range="3:1-3:10">"#),
        "{plain}"
    );
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
    let rendered = render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");
    let (with_toc, headings) = (rendered.html, rendered.headings);

    let texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert_eq!(texts, ["Real"]);
    assert!(with_toc.contains("<h2>Raw</h2>"), "{with_toc}");
    assert!(
        with_toc.contains(r#"<h2 data-source-range="3:1-3:7" id="real">"#),
        "{with_toc}"
    );
}

#[test]
fn a_document_without_headings_has_an_empty_toc() {
    let rendered = render_to_html_with_toc(
        "Just text.",
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");
    let (html, headings) = (rendered.html, rendered.headings);

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

#[test]
fn an_empty_document_renders_nothing() {
    let html = render("");

    assert!(html.trim().is_empty(), "{html:?}");
}

#[test]
fn a_document_of_frontmatter_alone_renders_only_its_table() {
    let html = render("---\ntitle: Test\n---\n");

    assert!(html.contains(r#"<details class="frontmatter""#), "{html}");
    assert!(!html.contains("<h1"), "{html}");
    assert!(!html.contains("<p "), "{html}");
}
