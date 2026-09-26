//! The reading profile of a parsed document.
//!
//! One entry per top-level block, at the line its `data-source-range` starts
//! on: that is the unit a scroll anchor names (`frontend/src/scroll-anchor.ts`
//! anchors to the children of the document with a range), so the host can
//! tell how much is left below any anchor without measuring the page. A block
//! with nothing to read, such as a rule, keeps its entry for that reason: an
//! anchor landing on it must not be taken for the block above.
//!
//! Footnote definitions are the exception, as in [`super::outline`]: the
//! renderer moves them into a trailing `<section class="footnotes">` that
//! carries no range, so they are read after everything else and are profiled
//! as one block past the last line.

use super::lines::LineTable;
use crate::{count_text, ReadingBlock, ReadingProfile};
use ox_content_ast::{Document, Node};

/// `escape_html` says raw HTML is shown as written, tags and all, rather than
/// rendered.
pub(super) fn collect(
    document: &Document<'_>,
    lines: &LineTable<'_>,
    escape_html: bool,
) -> ReadingProfile {
    let mut profile = ReadingProfile::default();
    let mut footnotes = Counter::new(
        u32::try_from(lines.last_line() + 1).unwrap_or(u32::MAX),
        escape_html,
    );
    let mut has_footnotes = false;
    for node in document.children.iter() {
        if let Node::FootnoteDefinition(definition) = node {
            footnotes.blocks(&definition.children);
            has_footnotes = true;
            continue;
        }
        let span = node.span();
        let Some(line) = lines.start_line(span.start as usize, span.end as usize) else {
            continue;
        };
        let mut counter = Counter::new(u32::try_from(line).unwrap_or(u32::MAX), escape_html);
        counter.blocks(std::slice::from_ref(node));
        profile.blocks.push(counter.finish());
    }
    if has_footnotes {
        profile.blocks.push(footnotes.finish());
    }
    profile
}

/// Counts one block. The text is gathered first and counted at the end,
/// because a word can span inline markup — `inter**national**ization` is one
/// word in three nodes.
struct Counter {
    block: ReadingBlock,
    text: String,
    escape_html: bool,
}

impl Counter {
    fn new(line: u32, escape_html: bool) -> Self {
        Self {
            block: ReadingBlock {
                line,
                ..Default::default()
            },
            text: String::new(),
            escape_html,
        }
    }

    fn finish(mut self) -> ReadingBlock {
        let (cjk_chars, words) = count_text(&self.text);
        self.block.cjk_chars += cjk_chars;
        self.block.words += words;
        self.block
    }

    /// Ends the text run, so that the next piece of text starts a new word.
    fn separate(&mut self) {
        self.text.push('\n');
    }

    fn blocks(&mut self, nodes: &[Node<'_>]) {
        for node in nodes {
            match node {
                Node::Paragraph(node) => self.inline(&node.children),
                Node::Heading(node) => self.inline(&node.children),
                Node::BlockQuote(node) => self.blocks(&node.children),
                Node::List(list) => {
                    for item in list.children.iter() {
                        self.blocks(&item.children);
                    }
                }
                Node::ListItem(item) => self.blocks(&item.children),
                Node::CodeBlock(code) => match code.lang {
                    // The two the frontend draws are looked at, not read.
                    Some("mermaid" | "math") => self.block.figures += 1,
                    _ => self.block.code_lines += count(code.value.lines().count()),
                },
                Node::MathBlock(_) => self.block.figures += 1,
                Node::Html(html) => self.html(html.value),
                Node::Table(table) => {
                    for row in table.children.iter() {
                        for cell in row.children.iter() {
                            self.inline(&cell.children);
                            self.separate();
                        }
                    }
                }
                Node::DefinitionList(node) => self.blocks(&node.children),
                Node::DefinitionListTerm(node) => self.inline(&node.children),
                Node::DefinitionListDefinition(node) => self.blocks(&node.children),
                Node::FootnoteDefinition(node) => self.blocks(&node.children),
                _ => self.inline(std::slice::from_ref(node)),
            }
            self.separate();
        }
    }

    fn inline(&mut self, nodes: &[Node<'_>]) {
        for node in nodes {
            match node {
                Node::Text(text) => self.text.push_str(text.value),
                Node::InlineCode(code) => self.text.push_str(code.value),
                // A formula is taken in at a glance, whatever its TeX says.
                Node::InlineMath(_) => {
                    self.block.words += 1;
                    self.separate();
                }
                Node::Break(_) => self.separate(),
                Node::Emphasis(node) => self.inline(&node.children),
                Node::Strong(node) => self.inline(&node.children),
                Node::Delete(node) => self.inline(&node.children),
                Node::Link(node) => self.inline(&node.children),
                Node::Superscript(node) => self.inline(&node.children),
                Node::Subscript(node) => self.inline(&node.children),
                Node::Image(_) => {
                    self.block.figures += 1;
                    self.separate();
                }
                Node::Html(html) => self.html(html.value),
                _ => {}
            }
        }
    }

    /// Escaped raw HTML is text like any other. Rendered, it is read for its
    /// text; its tags are not, except that an image in it is looked at like
    /// any other.
    fn html(&mut self, html: &str) {
        if self.escape_html {
            self.text.push_str(html);
            return;
        }
        let (text, images) = strip_tags(html);
        self.text.push_str(&text);
        self.block.figures += images;
    }
}

/// The text of `html` without its tags and comments, each replaced by a
/// space, and how many of the tags were images.
fn strip_tags(html: &str) -> (String, u32) {
    let mut text = String::with_capacity(html.len());
    let mut images = 0;
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        text.push(' ');
        let tag = &rest[open..];
        let close = if tag.starts_with("<!--") {
            tag.find("-->").map(|end| end + 3)
        } else {
            tag_end(tag)
        };
        let Some(close) = close else {
            rest = "";
            break;
        };
        if tag[..close]
            .get(..4)
            .is_some_and(|name| name.eq_ignore_ascii_case("<img"))
        {
            images += 1;
        }
        rest = &tag[close..];
    }
    text.push_str(rest);
    (text, images)
}

/// The byte just past the `>` that closes the tag `tag` starts with, where
/// a `>` inside a quoted attribute value does not count.
fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    for (index, c) in tag.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '>') => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(value: &str) -> (u32, u32) {
        let (text, images) = strip_tags(value);
        (count_text(text).1, images)
    }

    #[test]
    fn html_tags_and_comments_are_not_read() {
        assert_eq!(
            html(r#"<p class="lead">Two words</p><!-- a hidden note -->"#),
            (2, 0)
        );
    }

    #[test]
    fn an_image_in_html_is_a_figure() {
        assert_eq!(
            html(r#"<IMG src="a.png" alt="many words of alt text"><img src="b.png"/>"#),
            (0, 2)
        );
    }

    #[test]
    fn a_quoted_bracket_does_not_end_a_tag() {
        assert_eq!(
            html(r#"<span title="hidden > two hidden" data-x='a>b'>shown</span>"#),
            (1, 0)
        );
    }

    #[test]
    fn an_unclosed_tag_is_not_read() {
        assert_eq!(html("text <span class="), (1, 0));
    }
}
