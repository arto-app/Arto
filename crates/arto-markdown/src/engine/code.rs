//! Where the content of each code block sits in the source.
//!
//! A code block's span covers its fences, and whether there are fences is not
//! something the rendered HTML says: an indented block may well start with a
//! line of backticks, and a fence may be longer than the one that seems to
//! close it. The parser has already decided all of that, and its answer is in
//! the block's value — the content, one line per content line, without the
//! fences and without the indentation or container markers. So the content is
//! found by counting lines: a value with as many lines as the span is an
//! indented block, and otherwise the content starts on the line after the
//! opening fence.

use ox_content_ast::{Document, Node};
use std::collections::HashMap;
use std::ops::Range;

/// The content range of every code block, keyed by the start of its span.
///
/// Mermaid and math blocks are code blocks to the parser too; they are listed
/// like any other and simply never looked up.
pub(super) fn collect(document: &Document<'_>, body: &str) -> HashMap<usize, Range<usize>> {
    let mut contents = HashMap::new();
    collect_blocks(&document.children, body, &mut contents);
    contents
}

fn collect_blocks(nodes: &[Node<'_>], body: &str, contents: &mut HashMap<usize, Range<usize>>) {
    for node in nodes {
        match node {
            Node::CodeBlock(code) => {
                let span = code.span.start as usize..code.span.end as usize;
                if let Some(content) = content_range(body, span.clone(), code.value) {
                    contents.insert(span.start, content);
                }
            }
            Node::BlockQuote(node) => collect_blocks(&node.children, body, contents),
            Node::List(list) => {
                for item in list.children.iter() {
                    collect_blocks(&item.children, body, contents);
                }
            }
            Node::FootnoteDefinition(node) => collect_blocks(&node.children, body, contents),
            Node::DefinitionList(node) => collect_blocks(&node.children, body, contents),
            Node::DefinitionListDefinition(node) => collect_blocks(&node.children, body, contents),
            _ => {}
        }
    }
}

/// The bytes of `body` holding the content lines of the code block at `span`
/// whose value is `value`: from the start of its first line to the end of its
/// last, or `None` when the block has no content.
fn content_range(body: &str, span: Range<usize>, value: &str) -> Option<Range<usize>> {
    let source = body.get(span.clone())?;
    let content_lines = value.lines().count();
    if content_lines == 0 {
        return None;
    }
    let start = if source.trim_end_matches(['\r', '\n']).lines().count() == content_lines {
        body[..span.start]
            .rfind('\n')
            .map_or(0, |newline| newline + 1)
    } else {
        span.start + source.find('\n')? + 1
    };
    let end = body[start..span.end]
        .match_indices('\n')
        .nth(content_lines - 1)
        .map_or(span.end, |(newline, _)| start + newline);
    Some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content<'a>(body: &'a str, value: &str) -> Option<&'a str> {
        content_range(body, 0..body.len(), value).map(|range| &body[range])
    }

    #[test]
    fn a_fenced_block_starts_after_its_fence() {
        assert_eq!(content("```\nx\ny\n```\n", "x\ny\n"), Some("x\ny"));
    }

    #[test]
    fn an_indented_block_starting_with_backticks_is_all_content() {
        assert_eq!(
            content("    ```\n    text\n", "```\ntext\n"),
            Some("    ```\n    text")
        );
    }

    #[test]
    fn a_shorter_fence_inside_a_longer_one_is_content() {
        assert_eq!(content("````\nx\n```\n", "x\n```\n"), Some("x\n```"));
    }

    #[test]
    fn an_unclosed_fence_runs_to_the_end() {
        assert_eq!(content("```\nx\ny\n", "x\ny\n"), Some("x\ny"));
    }

    #[test]
    fn an_empty_block_has_no_content() {
        assert_eq!(content("```\n```\n", ""), None);
    }

    #[test]
    fn a_quoted_fence_keeps_the_markers_of_its_content_lines() {
        // The span of a fence inside a quote starts after the marker.
        let body = "> ```\n> x\n> ```\n";
        assert_eq!(
            content_range(body, 2..body.len(), "x\n").map(|range| &body[range]),
            Some("> x")
        );
    }

    #[test]
    fn an_indented_block_in_a_list_starts_at_its_line() {
        let body = "- a\n\n      code\n";
        assert_eq!(
            content_range(body, 5..body.len(), "code\n").map(|range| &body[range]),
            Some("      code")
        );
    }

    #[test]
    fn a_crlf_block_counts_the_same_lines() {
        assert_eq!(content("```\r\nx\r\n```\r\n", "x\n"), Some("x\r"));
    }
}
