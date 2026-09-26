//! The links of a parsed document, as the renderer would write their hrefs.
//!
//! Read off the AST rather than out of the text, so that what counts as a
//! link is what the parser made one: a reference-style link arrives with its
//! definition already applied, and a `[x](y)` inside code is code. A wiki link
//! gets the href its render hook writes (see [`super::wiki`]), so a link is
//! listed with the target a click on it would open.

use super::lines::LineTable;
use super::wiki;
use ox_content_ast::{Document, Node};

/// One link, in document order.
pub(crate) struct Link {
    /// The href as the rendered anchor carries it.
    pub href: String,
    /// Whether it was written as `[[target]]`.
    pub wiki: bool,
    /// Where it was written, lines counted in the whole file.
    pub range: crate::SourceRange,
}

/// Collect every link, including those nested in quotes, lists, tables,
/// definition lists, footnotes and inline markup.
pub(super) fn collect(document: &Document<'_>, lines: &LineTable<'_>) -> Vec<Link> {
    let mut links = Vec::new();
    walk(&document.children, lines, false, &mut links);
    links
}

/// `in_footnote` is set inside a footnote definition, whose content never
/// reaches the render hooks (ubugeeei-prod/ox-content#1362): a wiki link
/// there is written with its target as it stands, so it opens a document
/// only when the target already names one.
fn walk(nodes: &[Node<'_>], lines: &LineTable<'_>, in_footnote: bool, links: &mut Vec<Link>) {
    for node in nodes {
        match node {
            Node::Link(link) => {
                let (start, end) = (link.span.start as usize, link.span.end as usize);
                let is_wiki = wiki::is_wiki_link(lines.body(), start, end);
                let href = if is_wiki && !in_footnote {
                    wiki::href(link.url)
                } else {
                    link.url.to_string()
                };
                if let Some(range) = lines.range(start, end) {
                    links.push(Link {
                        href,
                        wiki: is_wiki,
                        range: range.to_public(),
                    });
                }
            }
            Node::Paragraph(node) => walk(&node.children, lines, in_footnote, links),
            Node::Heading(node) => walk(&node.children, lines, in_footnote, links),
            Node::BlockQuote(node) => walk(&node.children, lines, in_footnote, links),
            Node::List(list) => {
                for item in list.children.iter() {
                    walk(&item.children, lines, in_footnote, links);
                }
            }
            Node::ListItem(node) => walk(&node.children, lines, in_footnote, links),
            Node::Table(table) => {
                for row in table.children.iter() {
                    for cell in row.children.iter() {
                        walk(&cell.children, lines, in_footnote, links);
                    }
                }
            }
            Node::DefinitionList(node) => walk(&node.children, lines, in_footnote, links),
            Node::DefinitionListTerm(node) => walk(&node.children, lines, in_footnote, links),
            Node::DefinitionListDefinition(node) => walk(&node.children, lines, in_footnote, links),
            Node::FootnoteDefinition(node) => walk(&node.children, lines, true, links),
            Node::Emphasis(node) => walk(&node.children, lines, in_footnote, links),
            Node::Strong(node) => walk(&node.children, lines, in_footnote, links),
            Node::Delete(node) => walk(&node.children, lines, in_footnote, links),
            Node::Superscript(node) => walk(&node.children, lines, in_footnote, links),
            Node::Subscript(node) => walk(&node.children, lines, in_footnote, links),
            _ => {}
        }
    }
}
