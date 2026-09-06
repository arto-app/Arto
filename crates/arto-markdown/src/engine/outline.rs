//! The heading outline of a parsed document.
//!
//! Levels and text come from the AST. The `id` each heading ends up with is
//! read back from the rendered HTML in [`super::annotate`], so the table of
//! contents always agrees with the anchors the renderer wrote, including its
//! `-1` suffix for a repeated slug.

use ox_content_ast::{Document, Node};

/// One heading, in document order.
pub(super) struct Heading {
    pub level: u8,
    pub text: String,
}

/// Collect every heading, including those nested in quotes, list items and
/// definition lists — the renderer gives those an `id` too, so the outline
/// has to list them in the same order.
///
/// Footnote definitions are the one place where document order and render
/// order disagree: the renderer moves their bodies into the trailing
/// `<section class="footnotes">`. A heading inside one is part of a note,
/// not a section of the document, so it stays out of the outline — and
/// [`super::annotate`] leaves its `id` out for the same reason, which is
/// what keeps the two lists aligned.
pub(super) fn collect(document: &Document<'_>) -> Vec<Heading> {
    let mut headings = Vec::new();
    collect_blocks(&document.children, &mut headings);
    headings
}

fn collect_blocks(nodes: &[Node<'_>], headings: &mut Vec<Heading>) {
    for node in nodes {
        match node {
            Node::Heading(heading) => headings.push(Heading {
                level: heading.depth.clamp(1, 6),
                text: heading_text(&heading.children),
            }),
            Node::BlockQuote(node) => collect_blocks(&node.children, headings),
            Node::List(list) => {
                for item in list.children.iter() {
                    collect_blocks(&item.children, headings);
                }
            }
            Node::DefinitionList(node) => collect_blocks(&node.children, headings),
            Node::DefinitionListDefinition(node) => collect_blocks(&node.children, headings),
            _ => {}
        }
    }
}

/// Plain text of a heading: inline markup is flattened, line breaks become
/// spaces, and math keeps its TeX source.
pub(super) fn heading_text(children: &[Node<'_>]) -> String {
    let mut text = String::new();
    push_text(children, &mut text);
    let text = text.replace('\n', " ").trim().to_string();
    // A `{#id .class}` block is markup, not part of the title.
    match super::attributes::split_trailing(&text) {
        Some((stripped, _)) => stripped.to_string(),
        None => text,
    }
}

fn push_text(nodes: &[Node<'_>], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text.value),
            Node::InlineCode(code) => out.push_str(code.value),
            Node::InlineMath(math) => out.push_str(math.value),
            Node::Emphasis(node) => push_text(&node.children, out),
            Node::Strong(node) => push_text(&node.children, out),
            Node::Delete(node) => push_text(&node.children, out),
            Node::Link(node) => push_text(&node.children, out),
            Node::Superscript(node) => push_text(&node.children, out),
            Node::Subscript(node) => push_text(&node.children, out),
            _ => {}
        }
    }
}
