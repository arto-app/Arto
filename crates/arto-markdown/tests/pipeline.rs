//! Behaviour of the rendering pipeline seen through its public API.
//!
//! These tests describe what the rendered HTML looks like for each
//! construct, not how the engine produces it, so they hold across an
//! engine swap. Internals that are only observable at the engine level
//! (event streams, offset mapping) are tested next to that code.

use arto_markdown::{
    rebase_document_links, render_preview, render_to_html, render_to_html_with_toc, HeadingInfo,
    ImageResolution, RawHtml, ReadingBlock, RenderOptions,
};
use indoc::indoc;
use std::path::Path;

fn render(markdown: &str) -> String {
    render_to_html(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html
}

fn headings(markdown: &str) -> Vec<HeadingInfo> {
    render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .headings
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
    let cr = lf.replace('\n', "\r");

    assert_eq!(render(&crlf), render(lf));
    assert_eq!(render(&cr), render(lf));
    // The rule must stay a rule: read as a setext underline it would close
    // the paragraph above it as a heading instead.
    assert!(render(&crlf).contains("<hr data-source-range=\"3:1-3:3\">"));
}

#[test]
fn a_crlf_math_block_hands_the_frontend_the_same_source_as_lf() {
    // `data-original-content` is what the client-side renderer is given,
    // and the parser reports a math block's value with the `\r` still in it.
    let lf = "$$\nx = 1\n$$\n";

    assert_eq!(render(&lf.replace('\n', "\r\n")), render(lf));
}

#[test]
fn a_crlf_selection_maps_back_to_its_source() {
    use arto_markdown::extract_source_selection;

    assert_eq!(
        extract_source_selection(
            "intro\r\n\r\nsome **word** here\r\n",
            "word",
            &RenderOptions::default()
        ),
        Some("**word**".to_string())
    );
}

#[test]
fn a_crlf_selection_inside_a_code_block_maps_back_too() {
    use arto_markdown::extract_source_selection;

    // The parser reports a code block's value with the `\r` taken out, so a
    // map built over the CRLF source would not find it and would lose the
    // whole block.
    assert_eq!(
        extract_source_selection(
            "```rust\r\nlet x = 1;\r\n```\r\n",
            "let x = 1;",
            &RenderOptions::default()
        ),
        Some("let x = 1;".to_string())
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
fn a_wiki_target_with_spaces_stays_a_path() {
    // The app opens the target as a file name, so it must not reach it
    // percent-encoded the way an ordinary link's href would.
    let html = render("[[My Note]]");

    assert!(html.contains(r#"data-md-link="My Note.md""#), "{html}");
}

#[test]
fn a_wiki_label_is_rendered_as_markup() {
    let html = render("[[Guide|**the** guide]]");

    assert!(html.contains("<strong>the</strong> guide</span>"), "{html}");
}

#[test]
fn a_url_in_a_wiki_label_does_not_become_a_second_link() {
    // The label is already inside the anchor the wiki link opened, so an
    // autolinked URL there would nest one `<a>` in another and the browser
    // would close the outer one early, dropping the rest of the label out
    // of the link.
    let html = render("[[https://example.com|see https://other.com now]]");

    assert_eq!(html.matches("<a ").count(), 1, "{html}");
    assert!(html.contains("see https://other.com now</a>"), "{html}");
}

// ----------------------------------------------------------------------
// Links shown away from their document
// ----------------------------------------------------------------------

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
fn a_preview_shows_a_diagram_as_its_source() {
    // A diagram is drawn by the page, and what it draws can fetch: a node
    // may name an image by address. Left as source, nothing draws it.
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

    assert!(!html.contains("preprocessed-mermaid"), "{html}");
    assert!(!html.contains("data-original-content"), "{html}");
    assert!(html.contains("flowchart LR"), "{html}");
}

// ----------------------------------------------------------------------
// Images
// ----------------------------------------------------------------------

/// The `<picture>` shape GitHub documents for theme-aware images. The
/// document has no base URL, so every candidate has to be inlined or the
/// theme that picks the `<source>` shows an empty space.
#[test]
fn a_picture_inlines_both_the_source_and_the_img() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("dark.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
    std::fs::write(dir.path().join("light.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let html = render_to_html(
        indoc! {r#"
            <picture>
              <source media="(prefers-color-scheme: dark)" srcset="./dark.png">
              <img src="./light.png" alt="hero">
            </picture>
        "#},
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert_eq!(html.matches("data:image/png;base64,").count(), 2, "{html}");
    assert!(!html.contains("./dark.png"), "{html}");
}

#[test]
fn a_deferred_image_leaves_the_bytes_out_and_names_the_file() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hero.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let rendered = render_to_html(
        "![hero](./hero.png)\n\n![again](./hero.png)\n",
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert!(
        !rendered.html.contains("base64"),
        "the bytes stayed in the document: {}",
        rendered.html
    );
    // One file drawn twice is one entry and one URL, which is what lets the
    // host serve it once and the WebView cache it.
    assert_eq!(rendered.images.len(), 1, "{:?}", rendered.images);
    assert_eq!(
        rendered.images[0].path,
        dir.path().join("hero.png").canonicalize().unwrap()
    );
    let url = format!("artoasset://localhost/img/{}", rendered.images[0].id);
    assert_eq!(rendered.html.matches(&url).count(), 2, "{}", rendered.html);
}

/// Whether an image can be shown decides the markup, so it is decided while
/// the markup is written — the host serving the URL is far too late to drop a
/// candidate the browser has already preferred.
#[test]
fn a_deferred_srcset_drops_the_candidate_it_cannot_serve() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("small.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
    std::fs::write(dir.path().join("empty.png"), []).unwrap();

    let rendered = render_to_html(
        r#"<img src="./small.png" srcset="./empty.png 2x, ./small.png 1x">"#,
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert_eq!(rendered.images.len(), 1, "{:?}", rendered.images);
    assert!(
        rendered
            .images
            .iter()
            .all(|image| image.path.ends_with("small.png")),
        "{:?}",
        rendered.images
    );
    assert!(!rendered.html.contains("empty.png"), "{}", rendered.html);
}

#[test]
fn a_deferred_image_carries_the_type_to_answer_with() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("diagram.svg"), b"<svg/>").unwrap();

    let rendered = render_to_html(
        "![diagram](./diagram.svg)",
        dir.path().join("doc.md"),
        &RenderOptions {
            images: ImageResolution::Deferred {
                base_url: "artoasset://localhost/img".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("renders");

    assert_eq!(rendered.images[0].mime, "image/svg+xml");
}

#[test]
fn an_inlined_image_leaves_nothing_for_the_host_to_serve() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hero.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

    let rendered = render_to_html(
        "![hero](./hero.png)",
        dir.path().join("doc.md"),
        &RenderOptions::default(),
    )
    .expect("renders");

    assert!(rendered.html.contains("data:image/png;base64,"));
    assert!(rendered.images.is_empty(), "{:?}", rendered.images);
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
        html.contains(r#"<blockquote data-source-range="2:1-2:15">"#),
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
    assert!(
        html.contains(r#"<code data-source-range="2:1-2:14" class="language-python">"#),
        "{html}"
    );
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
                ("data-source-range", "1:1-3:3"),
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
        html.contains(r#"<p data-source-range="1:1-1:16">Just a paragraph</p>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p data-source-range="3:1-3:11">Another one</p>"#),
        "{html}"
    );
    assert!(!html.contains("<table"), "{html}");
}

#[test]
fn table_range_extends_to_its_last_row() {
    let html = render("| A | B |\n|---|---|\n| 1 | 2 |");

    assert!(
        has_element(&html, "table", &[("data-source-range", "1:1-3:9")]),
        "{html}"
    );
}

#[test]
fn each_table_gets_its_own_range() {
    let html = render("| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |\n\n| P |\n|---|\n| Q |");

    for range in ["1:1-3:5", "5:1-7:5", "9:1-11:5"] {
        assert!(
            has_element(&html, "table", &[("data-source-range", range)]),
            "table {range}: {html}"
        );
    }
}

#[test]
fn header_only_table_has_a_range() {
    let html = render("| A | B |\n|---|---|");

    assert!(
        has_element(&html, "table", &[("data-source-range", "1:1-2:9")]),
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

// ----------------------------------------------------------------------
// Source ranges
// ----------------------------------------------------------------------

/// The `data-source-range` of every `<tag …>` start tag in `html`, in order.
fn ranges(html: &str, tag: &str) -> Vec<String> {
    html.match_indices(&format!("<{tag} "))
        .filter_map(|(pos, _)| {
            let end = html[pos..].find('>').map_or(html.len(), |end| pos + end);
            let start_tag = &html[pos..end];
            let value = start_tag.split(r#" data-source-range=""#).nth(1)?;
            Some(value[..value.find('"')?].to_string())
        })
        .collect()
}

/// The text of `markdown` that the inclusive `L:C-L:C` range covers.
fn source_text<'a>(markdown: &'a str, range: &str) -> &'a str {
    let offset = |position: &str| {
        let (line, column) = position.split_once(':').expect("L:C");
        let (line, column): (usize, usize) = (line.parse().unwrap(), column.parse().unwrap());
        let line_start: usize = markdown
            .split_inclusive('\n')
            .take(line - 1)
            .map(str::len)
            .sum();
        let (index, ch) = markdown[line_start..]
            .char_indices()
            .nth(column - 1)
            .expect("column inside the line");
        (line_start + index, ch.len_utf8())
    };
    let (start, end) = range.split_once('-').expect("L:C-L:C");
    let (start, _) = offset(start);
    let (end, width) = offset(end);
    &markdown[start..end + width]
}

#[test]
fn every_block_names_the_source_it_was_rendered_from() {
    let markdown = indoc! {"
        # Title

        A paragraph
        over two lines.

        - one
        - two

        ---
    "};
    let html = render(markdown);

    assert_eq!(ranges(&html, "h1"), ["1:1-1:7"], "{html}");
    assert_eq!(ranges(&html, "p"), ["3:1-4:15"], "{html}");
    assert_eq!(ranges(&html, "ul"), ["6:1-7:5"], "{html}");
    assert_eq!(ranges(&html, "li"), ["6:1-6:5", "7:1-7:5"], "{html}");
    assert_eq!(ranges(&html, "hr"), ["9:1-9:3"], "{html}");
    assert_eq!(
        source_text(markdown, "3:1-4:15"),
        "A paragraph\nover two lines."
    );
}

#[test]
fn the_range_attribute_stands_in_place_of_the_engine_span() {
    let html = render("# Title {#my-id .my-class}");

    assert!(
        html.contains(r#"<h1 data-source-range="1:1-1:26" id="my-id" class="my-class">Title</h1>"#),
        "{html}"
    );
}

#[test]
fn a_table_names_every_cell_without_its_padding() {
    let markdown = "| Name | 値 |\n| --- | --- |\n| `a \\| b` | 日本語 |\n";
    let html = render(markdown);

    assert_eq!(ranges(&html, "table"), ["1:1-3:18"], "{html}");
    assert_eq!(ranges(&html, "tr"), ["1:1-1:12", "3:1-3:18"], "{html}");
    assert_eq!(ranges(&html, "th"), ["1:3-1:6", "1:10-1:10"], "{html}");
    assert_eq!(ranges(&html, "td"), ["3:3-3:10", "3:14-3:16"], "{html}");
    assert_eq!(source_text(markdown, "3:3-3:10"), "`a \\| b`");
    assert_eq!(source_text(markdown, "3:14-3:16"), "日本語");
}

#[test]
fn a_cell_the_row_left_out_has_no_range() {
    let html = render("| A | B |\n| - | - |\n| 1 |\n");

    assert_eq!(ranges(&html, "td"), ["3:3-3:3"], "{html}");
    assert!(html.contains("<td></td>"), "{html}");
}

#[test]
fn lines_count_through_the_frontmatter() {
    let html = render(indoc! {"
        ---
        title: Test
        ---

        # Title
    "});

    assert_eq!(ranges(&html, "h1"), ["5:1-5:7"], "{html}");
}

#[test]
fn a_body_right_after_the_frontmatter_keeps_its_indentation() {
    let html = render("---\ntitle: Test\n---\n    code\n");

    assert_eq!(ranges(&html, "pre"), ["4:5-4:8"], "{html}");
    assert_eq!(ranges(&html, "code"), ["4:1-4:8"], "{html}");
}

#[test]
fn a_fenced_code_block_hands_its_content_range_to_the_code_element() {
    let markdown = "```rust\nfn main() {}\n```\n";
    let html = render(markdown);

    assert!(
        html.contains(
            r#"<pre data-source-range="1:1-3:3"><code data-source-range="2:1-2:12" class="language-rust">"#
        ),
        "{html}"
    );
}

#[test]
fn code_content_starts_on_the_line_after_the_fence_even_when_blank() {
    let html = render("```\n\nx\n```\n");

    assert_eq!(ranges(&html, "code"), ["2:1-3:1"], "{html}");
}

#[test]
fn an_unclosed_fence_runs_its_content_to_the_end() {
    let html = render("```\nx\ny\n");

    assert_eq!(ranges(&html, "code"), ["2:1-3:1"], "{html}");
}

#[test]
fn an_empty_code_block_has_no_content_range() {
    let html = render("```\n```\n");

    assert_eq!(ranges(&html, "pre"), ["1:1-2:3"], "{html}");
    assert!(html.contains("<code>"), "{html}");
}

#[test]
fn an_indented_code_block_content_starts_on_its_first_line() {
    let html = render("    fn main() {}\n    let x = 1;\n");

    assert_eq!(ranges(&html, "pre"), ["1:5-2:14"], "{html}");
    assert_eq!(ranges(&html, "code"), ["1:1-2:14"], "{html}");
}

#[test]
fn an_indented_block_that_starts_with_backticks_is_all_content() {
    let html = render("    ```\n    text\n");

    assert_eq!(ranges(&html, "code"), ["1:1-2:8"], "{html}");
}

#[test]
fn a_shorter_fence_inside_a_longer_one_is_content() {
    let html = render("````\nx\n```\n");

    assert_eq!(ranges(&html, "code"), ["2:1-3:3"], "{html}");
}

#[test]
fn a_fence_in_a_footnote_hands_over_its_content_too() {
    let html = render("Text[^1]\n\n[^1]: Note\n\n    ```rust\n    x\n    ```\n");

    assert_eq!(ranges(&html, "code"), ["6:1-6:5"], "{html}");
}

#[test]
fn a_range_the_document_wrote_itself_does_not_survive() {
    let html = render("<div data-source-range=\"0:0-0:0\">raw</div>\n\ntext\n");

    assert!(!html.contains("0:0-0:0"), "{html}");
    assert_eq!(ranges(&html, "p"), ["3:1-3:4"], "{html}");
}

#[test]
fn a_quoted_block_keeps_the_quote_markers_after_its_first_line() {
    let markdown = "> one\n> two\n";
    let html = render(markdown);

    assert_eq!(ranges(&html, "blockquote"), ["1:1-2:5"], "{html}");
    assert_eq!(ranges(&html, "p"), ["1:3-2:5"], "{html}");
    assert_eq!(source_text(markdown, "1:3-2:5"), "one\n> two");
}

#[test]
fn an_alert_body_starts_after_its_marker() {
    let html = render("> [!NOTE]\n> This is a note\n");

    assert!(
        html.contains(
            r#"<div class="markdown-alert markdown-alert-note" data-source-range="1:1-2:16" dir="auto">"#
        ),
        "{html}"
    );
    // The title is made up by the pipeline, so it names no source.
    assert!(
        html.contains(r#"<p class="markdown-alert-title" dir="auto">"#),
        "{html}"
    );
    assert_eq!(ranges(&html, "p"), ["2:3-2:16"], "{html}");
}

#[test]
fn an_alert_body_on_the_marker_line_starts_after_the_marker() {
    let html = render("> [!TIP] Inline body\n");

    assert_eq!(ranges(&html, "p"), ["1:10-1:20"], "{html}");
}

#[test]
fn an_alert_body_on_the_marker_line_keeps_a_leading_bracket() {
    let html = render("> [!NOTE] > 0\n");

    assert!(html.contains("&gt; 0"), "{html}");
    assert_eq!(ranges(&html, "p"), ["1:11-1:13"], "{html}");
}

#[test]
fn an_alert_body_that_starts_on_its_own_line_keeps_that_line() {
    let html = render(indoc! {"
        > [!NOTE]
        >
        > first line
        > second ] line
    "});

    assert_eq!(ranges(&html, "p"), ["3:3-4:15"], "{html}");
}

#[test]
fn an_alert_without_a_body_does_not_shift_the_paragraph_after_it() {
    let html = render(indoc! {"
        > [!NOTE]

        first line
        second ] line
    "});

    assert_eq!(ranges(&html, "p"), ["3:1-4:13"], "{html}");
}

#[test]
fn a_crlf_block_ends_before_its_line_break() {
    let html = render("# T\r\n\r\npara\r\n");

    assert_eq!(ranges(&html, "p"), ["3:1-3:4"], "{html}");
}

#[test]
fn containers_carry_the_range_of_their_fence() {
    let html = render("# Title\n\n```mermaid\ngraph TD\n    A-->B\n```\n\n$$\nx = 1\n$$\n");

    assert!(
        has_element(
            &html,
            "pre",
            &[
                ("class", "preprocessed-mermaid"),
                ("data-source-range", "3:1-6:3")
            ]
        ),
        "{html}"
    );
    assert!(
        has_element(
            &html,
            "div",
            &[
                ("class", "preprocessed-math-display"),
                ("data-source-range", "8:1-10:2")
            ]
        ),
        "{html}"
    );
}

#[test]
fn a_footnote_definition_keeps_the_line_it_was_written_on() {
    let html = render("Text[^1]\n\n[^1]: The note.\n\nAfter.\n");

    let footnotes = &html[html
        .find(r#"<section class="footnotes""#)
        .expect("footnotes")..];
    assert!(footnotes.contains(r#"data-source-range="3:"#), "{html}");
}

#[test]
fn inline_markup_passes_through_untouched() {
    let html = render("Hello **bold** world");
    assert!(
        html.contains(r#"<p data-source-range="1:1-1:20">Hello <strong>bold</strong> world</p>"#),
        "{html}"
    );
}

#[test]
fn nothing_of_the_line_attributes_is_left() {
    let html = render("# T\n\n- a\n\n```\nx\n```\n\n| A |\n| - |\n| 1 |\n");

    assert!(!html.contains("data-source-line"), "{html}");
    assert!(!html.contains("data-source-span"), "{html}");
}

#[test]
fn heading_attributes_survive_next_to_the_range() {
    let html = render("### 日本語の見出し {.highlight}");

    assert!(
        html.contains(r#"<h3 data-source-range="1:1-1:24" class="highlight">日本語の見出し</h3>"#),
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
    // The heading is rendered here rather than by the engine, so the slug it
    // would have been given has to be derived from the text.
    let headings = headings("### 日本語の見出し {.highlight}");

    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].id, "日本語の見出し");
}

#[test]
fn braces_that_are_not_an_attribute_block_stay_text() {
    let html = render("# What {this means}");

    assert!(html.contains("What {this means}</h1>"), "{html}");
}

#[test]
fn each_construct_names_its_own_range() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("1. first\n2. second\n", "ol", &["1:1-2:9"]),
        (
            "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n",
            "tr",
            &["1:1-1:9", "3:1-3:9", "4:1-4:9", "5:1-5:9"],
        ),
        (
            "---\ntitle: Test\n---\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
            "table",
            &["5:1-7:9"],
        ),
        (
            "> [!NOTE]\n> This is a note\n\n# Heading After Alert\n",
            "h1",
            &["4:1-4:21"],
        ),
        (
            "> [!TIP]\n> Some tip\n\n```rust\nfn main() {}\n```\n",
            "code",
            &["5:1-5:12"],
        ),
        (
            "> [!NOTE]\n> Some note\n\n```mermaid\ngraph TD\n    A-->B\n```\n",
            "pre",
            &["4:1-7:3"],
        ),
        ("# Title\n\n```math\nE = mc^2\n```\n", "pre", &["3:1-5:3"]),
    ];

    for (markdown, tag, expected) in cases {
        let html = render(markdown);
        assert_eq!(ranges(&html, tag), *expected, "{markdown:?}: {html}");
    }
}

#[test]
fn ranges_after_an_alert_and_frontmatter_point_at_the_original_file() {
    // The frontmatter is cut off and the alert is rewritten; neither may
    // shift the ranges reported for the blocks that follow.
    let html = render(indoc! {"
        ---
        title: Test
        ---

        # Title

        > [!NOTE]
        > Paragraph A

        Paragraph B
    "});

    assert_eq!(ranges(&html, "h1"), ["5:1-5:7"], "{html}");
    assert_eq!(ranges(&html, "p"), ["8:3-8:13", "10:1-10:11"], "{html}");
}

// ----------------------------------------------------------------------
// Rendering options
// ----------------------------------------------------------------------

fn render_with(markdown: &str, options: RenderOptions) -> String {
    render_to_html(markdown, Path::new("/nonexistent/test.md"), &options)
        .expect("renders")
        .html
}

#[test]
fn math_off_leaves_the_dollars_to_the_prose() {
    // Two shell variables in one line are what the option is for: the parser
    // pairs their `$`s into a formula spanning the text between them.
    let source = "run echo $HOME/$USER now";
    assert!(
        render(source).contains("preprocessed-math"),
        "the default reads this as a formula"
    );

    let html = render_with(
        source,
        RenderOptions {
            math: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("preprocessed-math"), "{html}");
    assert!(html.contains("$HOME/$USER"), "{html}");
}

#[test]
fn wiki_links_off_leave_the_brackets_in_the_text() {
    let source = "see [[Page]] for more";
    assert!(render(source).contains("md-link"), "the default links this");

    let html = render_with(
        source,
        RenderOptions {
            wiki_links: false,
            ..Default::default()
        },
    );
    assert!(html.contains("[[Page]]"), "{html}");
}

#[test]
fn subscript_off_leaves_a_lone_tilde_alone() {
    let source = "about ~5 minutes~ long";
    assert!(render(source).contains("<sub>"), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            subscript: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<sub>"), "{html}");
}

#[test]
fn superscript_off_leaves_a_lone_caret_alone() {
    let source = "the 2^nd^ time";
    assert!(render(source).contains("<sup>"), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            superscript: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<sup>"), "{html}");
}

#[test]
fn smart_punctuation_off_keeps_the_quotes_as_typed() {
    let source = r#"He said "hello" -- loudly"#;
    assert!(
        render(source).contains('\u{201c}'),
        "the default curls this"
    );

    let html = render_with(
        source,
        RenderOptions {
            smart_punctuation: false,
            ..Default::default()
        },
    );
    assert!(html.contains("&quot;hello&quot;"), "{html}");
}

#[test]
fn cjk_emphasis_off_renders_the_way_github_does() {
    let source = "これは**「重要」**です。";
    assert!(
        render(source).contains("<strong>"),
        "the default pairs this"
    );

    let html = render_with(
        source,
        RenderOptions {
            cjk_emphasis: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<strong>"), "{html}");
}

#[test]
fn definition_lists_off_leave_the_colon_in_the_paragraph() {
    let source = "Term\n\n: The definition\n";
    assert!(render(source).contains("<dl "), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            definition_lists: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<dl>"), "{html}");
}

#[test]
fn heading_attributes_off_leave_the_block_in_the_heading_text() {
    let source = "# Title {#custom}\n";
    assert_eq!(
        headings(source)[0].id,
        "custom",
        "the default takes the id from the block"
    );

    let html = render_with(
        source,
        RenderOptions {
            heading_attributes: false,
            ..Default::default()
        },
    );
    assert!(html.contains("{#custom}"), "{html}");
}

#[test]
fn heading_permalinks_appear_only_when_asked_for() {
    let source = "# Title\n";
    assert!(!render(source).contains("header-anchor"), "off by default");

    let html = render_with(
        source,
        RenderOptions {
            heading_permalinks: true,
            ..Default::default()
        },
    );
    assert!(html.contains(r#"class="header-anchor""#), "{html}");
}

#[test]
fn raw_html_is_filtered_by_default() {
    // The tags that would restyle or script the page around the document are
    // escaped; everything else keeps working.
    let source = "<kbd>Esc</kbd> and <style>p{color:red}</style>\n";

    let filtered = render(source);
    assert!(filtered.contains("<kbd>Esc</kbd>"), "{filtered}");
    assert!(!filtered.contains("<style>"), "{filtered}");

    let allowed = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Allow,
            ..Default::default()
        },
    );
    assert!(allowed.contains("<style>"), "{allowed}");

    let escaped = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Escape,
            ..Default::default()
        },
    );
    assert!(!escaped.contains("<kbd>"), "{escaped}");
    assert!(escaped.contains("&lt;kbd&gt;"), "{escaped}");
}

#[test]
fn filtering_raw_html_takes_the_script_out_of_it() {
    // The tag filter escapes `<script>`, so a document that wants to run
    // anyway writes it as an attribute instead. Neither spelling may reach the
    // reader under the default setting, which is the one that says the
    // document is not trusted.
    let source = indoc! {r#"
        <img src="nothing.png" onerror="alert(1)">

        <a href="javascript:alert(2)">follow me</a>

        <p ontoggle="alert(3)">text</p>
    "#};

    let filtered = render(source);
    assert!(!filtered.contains("onerror"), "{filtered}");
    assert!(!filtered.contains("ontoggle"), "{filtered}");
    assert!(!filtered.contains("javascript:"), "{filtered}");
    assert!(!filtered.contains("alert"), "{filtered}");

    // What the markup said, minus the script, is still there to read.
    assert!(filtered.contains("follow me"), "{filtered}");
    assert!(filtered.contains("text"), "{filtered}");
}

#[test]
fn a_script_url_spelled_in_entities_is_still_a_script_url() {
    // The attribute reaches the filter as the document spelled it, and the
    // browser is what resolves `&#106;` back into a `j`. A filter reading only
    // what was written would hand the reader a link that runs on click.
    let source = indoc! {r#"
        <a href="&#106;avascript:alert(1)">follow me</a>
    "#};

    let filtered = render(source);
    assert!(!filtered.contains("avascript:"), "{filtered}");
    assert!(filtered.contains("follow me"), "{filtered}");
}

#[test]
fn allowing_raw_html_allows_all_of_it() {
    // `Allow` is the setting for a document the reader trusts, and it has
    // always meant every raw node through untouched. Filtering it would be a
    // different setting wearing its name.
    let source = indoc! {r#"
        <p onclick="alert(1)">text</p>
    "#};

    let allowed = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Allow,
            ..Default::default()
        },
    );
    assert!(allowed.contains("onclick"), "{allowed}");
}

#[test]
fn filtering_leaves_an_ordinary_document_whole() {
    // The filter reads every element of every document, so the cost of getting
    // it wrong is paid by markup that was never dangerous.
    let source = indoc! {r#"
        <kbd id="key" class="mod">Cmd</kbd>

        <details><summary>More</summary>Hidden</details>

        [a link](https://example.com)
    "#};

    let filtered = render(source);
    assert!(filtered.contains(r#"id="key""#), "{filtered}");
    assert!(filtered.contains(r#"class="mod""#), "{filtered}");
    assert!(filtered.contains("<details>"), "{filtered}");
    assert!(filtered.contains("https://example.com"), "{filtered}");
}

#[test]
fn the_gfm_baseline_survives_every_option_being_turned_off() {
    // Tables, task lists, strikethrough and footnotes are what a document
    // written for GitHub contains, so nothing a reader can switch may take
    // them away.
    let everything_off = RenderOptions {
        auto_link_urls: false,
        math: false,
        wiki_links: false,
        superscript: false,
        subscript: false,
        definition_lists: false,
        heading_attributes: false,
        smart_punctuation: false,
        cjk_emphasis: false,
        heading_permalinks: false,
        raw_html: RawHtml::Escape,
        ..Default::default()
    };

    let html = render_with(
        indoc! {"
            | a | b |
            | - | - |
            | c | d |

            - [x] done

            ~~gone~~ and a note[^1]

            [^1]: the note
        "},
        everything_off,
    );

    assert!(html.contains("<table"), "{html}");
    assert!(html.contains(r#"type="checkbox""#), "{html}");
    assert!(html.contains("<del>gone</del>"), "{html}");
    assert!(html.contains(r#"class="footnotes""#), "{html}");
}

// ----------------------------------------------------------------------
// Reading profile
// ----------------------------------------------------------------------

fn reading(markdown: &str) -> Vec<ReadingBlock> {
    render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders")
    .reading
    .blocks
}

#[test]
fn every_top_level_block_is_profiled_at_the_line_its_range_starts_on() {
    let markdown = indoc! {"
        ---
        title: Not read
        tags: [a, b]
        ---

        # Title

        A paragraph of five words.

        - one item
        - two items

        > quoted words here

        ---

        | a | b |
        | - | - |
        | c | d |
    "};
    let rendered = render_to_html_with_toc(
        markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect("renders");
    let profiled: Vec<u32> = rendered.reading.blocks.iter().map(|b| b.line).collect();

    assert_eq!(profiled, vec![6, 8, 10, 13, 15, 17]);
    // Each is where the page says a top-level block starts: the renderer
    // writes those at the start of a line of their own.
    for line in profiled {
        let range = format!(r#" data-source-range="{line}:1-"#);
        assert!(
            rendered
                .html
                .lines()
                .any(|row| row.starts_with('<') && row.split('>').next().unwrap().contains(&range)),
            "no top-level block at line {line} in {}",
            rendered.html
        );
    }
}

#[test]
fn the_frontmatter_is_not_read() {
    let blocks = reading(indoc! {"
        ---
        title: Many words that nobody reads as prose
        ---

        Two words.
    "});
    assert_eq!(
        blocks,
        vec![ReadingBlock {
            line: 5,
            words: 2,
            ..Default::default()
        }]
    );
}

#[test]
fn a_list_or_a_quote_is_one_block() {
    let blocks = reading(indoc! {"
        - one
        - two three
          - four

        > five
        >
        > six
    "});
    assert_eq!(blocks.len(), 2);
    assert_eq!((blocks[0].line, blocks[0].words), (1, 4));
    assert_eq!((blocks[1].line, blocks[1].words), (5, 2));
}

#[test]
fn code_is_counted_in_lines() {
    let blocks = reading(indoc! {"
        ```rust
        fn main() {
            println!(\"many words in a string\");
        }
        ```
    "});
    assert_eq!(
        blocks,
        vec![ReadingBlock {
            line: 1,
            code_lines: 3,
            ..Default::default()
        }]
    );
}

#[test]
fn images_diagrams_and_formulas_are_figures() {
    let blocks = reading(indoc! {"
        ![alt text](a.png) and ![](b.png)

        ```mermaid
        graph TD
          A --> B
        ```

        $$
        x^2
        $$

        ```math
        y
        ```
    "});
    let figures: Vec<(u32, u32, u32)> = blocks
        .iter()
        .map(|b| (b.line, b.figures, b.code_lines))
        .collect();
    assert_eq!(figures, vec![(1, 2, 0), (3, 1, 0), (8, 1, 0), (12, 1, 0)]);
    // The alt text is not read; the word between the images is.
    assert_eq!(blocks[0].words, 1);
}

#[test]
fn a_link_is_read_by_its_text_not_its_url() {
    let blocks = reading("Read [the manual](https://example.com/some/long/path) now.\n");
    assert_eq!(blocks[0].words, 4);
}

#[test]
fn raw_html_is_read_without_its_tags() {
    let blocks = reading(indoc! {r#"
        <details>
        <summary>Two words</summary>
        <img src="x.png">
        </details>
    "#});
    assert_eq!((blocks[0].words, blocks[0].figures), (2, 1));
}

#[test]
fn japanese_counts_characters() {
    let blocks = reading("Rustの所有権を学ぶ。\n");
    assert_eq!((blocks[0].cjk_chars, blocks[0].words), (7, 1));
}

#[test]
fn footnotes_are_read_at_the_end_where_they_are_shown() {
    let blocks = reading(indoc! {"
        A note[^1] here.

        [^1]: The note itself.

        After the note.
    "});
    let lines: Vec<u32> = blocks.iter().map(|b| b.line).collect();
    assert_eq!(lines[..2], [1, 5]);
    let last = blocks.last().unwrap();
    assert!(last.line > 5, "{blocks:?}");
    assert_eq!(last.words, 3);
}

#[test]
fn a_block_with_nothing_to_read_still_marks_where_it_is() {
    // A scroll anchor can land on the rule; without an entry of its own the
    // paragraph above would be taken for the block at the top of the view.
    let blocks = reading(indoc! {"
        Before the rule.

        ---

        After the rule.
    "});
    let lines: Vec<(u32, u32)> = blocks.iter().map(|b| (b.line, b.words)).collect();
    assert_eq!(lines, vec![(1, 3), (3, 0), (5, 3)]);
}

#[test]
fn a_word_split_by_inline_markup_is_one_word() {
    let blocks = reading("inter**national**ization and foo[bar](https://example.com)baz\n");
    assert_eq!(blocks[0].words, 3);
}

#[test]
fn escaped_html_is_read_as_the_text_it_shows() {
    let blocks = render_to_html_with_toc(
        "<span title=\"two words\">shown</span> <img src=\"a.png\">\n",
        Path::new("/nonexistent/test.md"),
        &RenderOptions {
            raw_html: RawHtml::Escape,
            ..Default::default()
        },
    )
    .expect("renders")
    .reading
    .blocks;
    assert_eq!((blocks[0].figures, blocks[0].words > 3), (0, true));
}
