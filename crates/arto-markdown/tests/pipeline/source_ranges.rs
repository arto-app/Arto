//! Source ranges

use super::*;

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
