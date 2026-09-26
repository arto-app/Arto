//! Which documents a Markdown source links to, seen through the public API.
//!
//! The rule is the one a click follows: whatever renders as a link to a
//! Markdown document is listed, and nothing else is.

use arto_markdown::{document_links, DocumentLink, RenderOptions, SourcePosition, SourceRange};
use indoc::indoc;

fn links(markdown: &str) -> Vec<DocumentLink> {
    document_links(markdown, &RenderOptions::default())
}

fn paths(markdown: &str) -> Vec<String> {
    links(markdown).into_iter().map(|link| link.path).collect()
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

#[test]
fn an_inline_link_to_a_document_is_listed_with_where_it_was_written() {
    let found = links(indoc! {"
        # Title

        See [the guide](./guide.md) first.
    "});

    assert_eq!(
        found,
        vec![DocumentLink {
            path: "./guide.md".to_string(),
            fragment: None,
            wiki: false,
            range: SourceRange {
                start: at(3, 5),
                end: at(3, 27),
            },
        }]
    );
}

#[test]
fn a_fragment_is_split_off_the_path() {
    let found = links("[setup](../docs/guide.md#setup)\n");

    assert_eq!(found[0].path, "../docs/guide.md");
    assert_eq!(found[0].fragment.as_deref(), Some("setup"));
}

#[test]
fn a_reference_style_link_is_listed_where_it_is_used() {
    let found = links(indoc! {"
        Read [the guide][g].

        [g]: guide.md
    "});

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, "guide.md");
    assert_eq!(found[0].range.start.line, 1);
}

#[test]
fn a_wiki_link_names_the_document_it_opens() {
    let found = links("Back to [[Guide]] and [[notes/today|today]].\n");

    assert_eq!(
        found
            .iter()
            .map(|link| (link.path.as_str(), link.wiki))
            .collect::<Vec<_>>(),
        vec![("Guide.md", true), ("notes/today.md", true)]
    );
}

#[test]
fn a_wiki_link_is_not_a_link_when_wiki_links_are_off() {
    let options = RenderOptions {
        wiki_links: false,
        ..RenderOptions::default()
    };

    assert!(document_links("Back to [[Guide]].\n", &options).is_empty());
}

#[test]
fn links_to_anything_but_a_document_are_left_out() {
    assert!(paths(indoc! {"
        [site](https://example.com/guide.md)
        [mail](mailto:someone@example.com)
        [picture](./image.png)
        [here](#section)
        [folder](./docs/)
    "})
    .is_empty());
}

#[test]
fn an_image_is_not_a_link() {
    assert!(paths("![diagram](./guide.md)\n").is_empty());
}

#[test]
fn a_link_written_inside_code_is_not_a_link() {
    assert!(paths(indoc! {"
        `[guide](guide.md)`

        ```
        [guide](guide.md)
        ```
    "})
    .is_empty());
}

#[test]
fn links_inside_containers_are_found() {
    let found = paths(indoc! {"
        > quoted [a](a.md)

        - listed [b](b.md)
          1. nested [c](c.md)

        | cell |
        | ---- |
        | [d](d.md) |

        *emphasised [e](e.md)*

        ## heading [f](f.md)
    "});

    assert_eq!(found, vec!["a.md", "b.md", "c.md", "d.md", "e.md", "f.md"]);
}

#[cfg(unix)]
#[test]
fn a_file_url_becomes_a_path() {
    let found = paths("[note](file:///tmp/notes/note.md)\n");

    assert_eq!(found, vec!["/tmp/notes/note.md"]);
}

#[cfg(windows)]
#[test]
fn a_file_url_becomes_a_path() {
    let found = paths("[note](file:///C:/notes/note.md)\n");

    assert_eq!(found, vec![r"C:\notes\note.md"]);
}

#[test]
fn both_markdown_extensions_count() {
    assert_eq!(
        paths("[a](a.markdown) [b](b.md)\n"),
        vec!["a.markdown", "b.md"]
    );
}

#[test]
fn lines_count_the_frontmatter() {
    let found = links(indoc! {"
        ---
        title: Notes
        ---

        [guide](guide.md)
    "});

    assert_eq!(found[0].range.start.line, 5);
}

#[test]
fn a_file_with_lone_carriage_returns_reports_its_own_lines() {
    let found = links("first\r\r[guide](guide.md)\r");

    assert_eq!(found[0].range.start.line, 3);
}

#[test]
fn columns_count_characters_rather_than_bytes() {
    let found = links("日本語 [guide](guide.md)\n");

    assert_eq!(found[0].range.start, at(1, 5));
}

#[test]
fn a_footnote_lists_the_links_it_renders() {
    // A wiki link inside a footnote renders with its target as written, so
    // it opens a document only when the target names one with its extension.
    let found = paths(indoc! {"
        Text.[^1]

        [^1]: See [the guide](guide.md), [[Other]] and [[Notes.md]].
    "});

    assert_eq!(found, vec!["guide.md", "Notes.md"]);
}
