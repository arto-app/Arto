//! Reading profile

use super::*;

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
