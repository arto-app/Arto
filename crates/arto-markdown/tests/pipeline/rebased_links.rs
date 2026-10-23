//! Links shown away from their document

use super::video::ATTACHMENT;
use super::*;

/// `markdown` rendered as the document at `<dir>/sub/doc.md`, with its links
/// rebased onto that document, next to a `<dir>/other.md` that exists.
fn rebased(markdown: &str) -> (String, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("other.md"), "# Other").unwrap();
    let document = dir.path().join("sub").join("doc.md");
    let html = render_to_html(markdown, &document, &RenderOptions::default())
        .expect("renders")
        .html;
    (rebase_document_links(&html, &document), dir)
}

#[test]
fn a_rebased_document_link_names_its_target_from_the_document_it_was_in() {
    let (html, dir) = rebased("[other](../other.md#part)");

    let expected = format!(
        "{}#part",
        dir.path().join("sub").join("../other.md").display()
    );
    assert!(
        has_element(
            &html,
            "span",
            &[("data-md-link", &expected), ("class", "md-link")]
        ),
        "{html}"
    );
}

#[test]
fn a_rebased_absolute_link_is_left_as_it_is() {
    let (_, dir) = rebased("");
    // Forward slashes, because a backslash in a Markdown link destination
    // escapes the character after it and a Windows path would lose some.
    let target = dir
        .path()
        .join("other.md")
        .display()
        .to_string()
        .replace('\\', "/");
    let document = dir.path().join("sub").join("doc.md");
    let html = render_to_html(
        format!("[other]({target})"),
        &document,
        &RenderOptions::default(),
    )
    .unwrap()
    .html;

    let html = rebase_document_links(&html, &document);

    assert!(
        has_element(&html, "span", &[("data-md-link", &target)]),
        "{html}"
    );
}

#[test]
fn a_rebased_link_to_a_missing_document_stays_marked_missing() {
    let (html, _dir) = rebased("[gone](gone.md)");

    assert!(
        has_element(&html, "span", &[("class", "md-link md-link-missing")]),
        "{html}"
    );
}

#[test]
fn a_rebased_in_page_link_points_into_the_document_it_was_in() {
    // Shown elsewhere, `#part` would name a heading of whatever page it is
    // shown on; it has to go on naming the one in its own document.
    let (html, dir) = rebased("[see](#part)");

    let expected = format!("{}#part", dir.path().join("sub").join("doc.md").display());
    assert!(
        has_element(
            &html,
            "span",
            &[("data-md-link", &expected), ("class", "md-link")]
        ),
        "{html}"
    );
    assert!(html.contains("window.handleMarkdownLinkClick"), "{html}");
    assert!(!html.contains(r##"href="#part""##), "{html}");
}

#[test]
fn a_rebased_web_link_stays_an_anchor() {
    let (html, _dir) = rebased("[web](https://example.com/)");

    assert!(
        has_element(&html, "a", &[("href", "https://example.com/")]),
        "{html}"
    );
}

#[test]
fn a_preview_keeps_heading_ids_and_rebases_its_links() {
    let dir = tempfile::TempDir::new().unwrap();
    let document = dir.path().join("doc.md");

    let html = render_preview(
        "# Title\n\n[next](next.md) and [back](#title)",
        &document,
        &RenderOptions::default(),
    )
    .unwrap()
    .html;

    assert!(has_element(&html, "h1", &[("id", "title")]), "{html}");
    let next = dir.path().join("next.md").display().to_string();
    assert!(
        has_element(&html, "span", &[("data-md-link", &next)]),
        "{html}"
    );
    let back = format!("{}#title", document.display());
    assert!(
        has_element(&html, "span", &[("data-md-link", &back)]),
        "{html}"
    );
}

#[test]
fn a_preview_fetches_nothing_from_elsewhere() {
    // A preview appears when the pointer merely passes over a link, so the
    // document it shows has not been opened: an address in it must not be
    // sent anything.
    let html = render_preview(
        "![tracker](https://example.com/pixel.png)\n\n[a link](https://example.com/)",
        Path::new("/nonexistent/doc.md"),
        &RenderOptions::default(),
    )
    .unwrap()
    .html;

    assert!(!html.contains("pixel.png"), "{html}");
    assert!(html.contains("tracker"), "{html}");
    assert!(
        has_element(&html, "a", &[("href", "https://example.com/")]),
        "{html}"
    );
}

#[test]
fn a_preview_of_a_video_attachment_leaves_its_link() {
    let html = render_preview(
        format!("{ATTACHMENT}\n"),
        Path::new("/nonexistent/doc.md"),
        &RenderOptions::default(),
    )
    .unwrap()
    .html;

    assert!(!html.contains("<video"), "{html}");
    assert!(has_element(&html, "a", &[("href", ATTACHMENT)]), "{html}");
}

#[test]
fn a_preview_escapes_raw_html_whatever_the_options_allow() {
    let html = render_preview(
        r#"<link rel="stylesheet" href="https://example.com/x.css">"#,
        Path::new("/nonexistent/doc.md"),
        &RenderOptions {
            raw_html: RawHtml::Allow,
            ..RenderOptions::default()
        },
    )
    .unwrap()
    .html;

    assert!(!html.contains("<link"), "{html}");
}

#[test]
fn a_preview_hands_a_diagram_to_the_page_to_draw() {
    // The page draws a preview's diagram as it would its own, so the
    // container reaches it as on any other page.
    let html = render_preview(
        indoc! {r#"
            ```mermaid
            flowchart LR
            A@{ img: "https://example.com/pixel.png" }
            ```
        "#},
        Path::new("/nonexistent/doc.md"),
        &RenderOptions::default(),
    )
    .unwrap()
    .html;

    assert!(
        has_element(&html, "pre", &[("class", "preprocessed-mermaid")]),
        "{html}"
    );
    assert!(
        html.contains(r#"data-original-content="flowchart LR"#),
        "{html}"
    );
}
