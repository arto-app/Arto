//! Mapping rendered text back to the Markdown source that produced it.
//!
//! When the user selects text in the rendered document, the app wants the
//! Markdown behind it, including the inline markers the rendering stripped.
//! That means parsing the source again the same way it was rendered, which
//! is why this lives next to the engine rather than in the app.
//!
//! The document is parsed and its visible text concatenated into a
//! "rendered" string while recording which source byte range each piece came
//! from. Node spans index the body handed to the parser, so the frontmatter
//! length is added back to land on whole-document offsets.

use super::parser_options;
use crate::frontmatter::extract_and_render_frontmatter;
use ox_content_allocator::Allocator;
use ox_content_ast::Node;
use ox_content_parser::Parser;
use std::ops::Range;

/// A segment mapping between rendered plain text and document byte positions.
struct TextSegment {
    rendered: Range<usize>,
    source: Range<usize>,
    /// Index of the block (paragraph, heading, cell, …) the segment belongs
    /// to. Only the gap between two segments of the same block is inline
    /// markup that a selection may absorb.
    block: usize,
    /// Document range of that block, so markup around the first or last
    /// segment of a block can be absorbed without reaching into a neighbor.
    block_source: Range<usize>,
}

impl TextSegment {
    /// Whether byte `n` of the rendered text is byte `n` of the source.
    ///
    /// Smart punctuation rewrites the text the parser reports (`"` becomes
    /// `“`, `--` becomes `–`), so a segment can render to a different number
    /// of bytes than it occupies in the document. Counting into such a
    /// segment would land somewhere else entirely, so a boundary inside one
    /// snaps to its edge instead.
    fn is_linear(&self) -> bool {
        self.rendered.len() == self.source.len()
    }
}

/// Walks the AST collecting the visible text of every block.
struct Collector<'a> {
    body: &'a str,
    /// Offset of the body within the whole document.
    base: usize,
    rendered: String,
    segments: Vec<TextSegment>,
    block: usize,
    block_source: Range<usize>,
}

impl Collector<'_> {
    fn push(&mut self, text: &str, start: u32, end: u32) {
        // Soft breaks are newlines inside text nodes; they render as spaces.
        self.push_rendered(&text.replace('\n', " "), start, end);
    }

    /// Push a piece whose rendered form is already known. It has to occupy as
    /// many bytes as `start..end` does, or [`TextSegment::is_linear`] stops
    /// counting into it.
    fn push_rendered(&mut self, rendered: &str, start: u32, end: u32) {
        let rendered_start = self.rendered.len();
        self.rendered.push_str(rendered);
        let source = self.base + start as usize..self.base + end as usize;

        // A segment boundary means "markup may sit here", and the gap
        // between two segments is what a selection absorbs. Two pieces that
        // touch in both the rendered text and the source have nothing
        // between them, so keeping them apart would offer an absorption
        // that cannot exist — and would make the map depend on how the
        // parser happened to split the run. Merging is only sound while
        // both sides count byte for byte, so a rewritten piece never joins.
        if let Some(last) = self.segments.last_mut() {
            if last.block == self.block
                && last.rendered.end == rendered_start
                && last.source.end == source.start
                && last.is_linear()
                && rendered.len() == source.len()
            {
                last.rendered.end = self.rendered.len();
                last.source.end = source.end;
                return;
            }
        }

        self.segments.push(TextSegment {
            rendered: rendered_start..self.rendered.len(),
            source,
            block: self.block,
            block_source: self.block_source.clone(),
        });
    }

    /// Start a block whose content spans `start..end` in the body. Trailing
    /// whitespace (the newline that closes most blocks) is left out so a
    /// selection never absorbs it.
    fn begin_block(&mut self, start: u32, end: u32) {
        let slice = self
            .body
            .get(start as usize..end as usize)
            .unwrap_or_default();
        let trimmed_end = start as usize + slice.trim_end().len();
        self.block_source = self.base + start as usize..self.base + trimmed_end;
    }

    /// Close the current block: rendered text gets a line break and later
    /// segments no longer share markup with earlier ones.
    fn end_block(&mut self) {
        self.rendered.push('\n');
        self.block += 1;
    }

    /// Where the value of a delimited node (code, math) sits inside its span,
    /// which covers the delimiters as well. `None` when the value is not a
    /// substring of the span, which means the two do not describe the same
    /// text and no honest mapping exists.
    fn value_range(&self, value: &str, start: u32, end: u32) -> Option<Range<u32>> {
        let slice = self.body.get(start as usize..end as usize)?;
        let offset = slice.find(value)? as u32;
        Some(start + offset..start + offset + value.len() as u32)
    }

    /// Push a node whose value is a substring of its span (code, math): the
    /// segment covers only the value, not the delimiters.
    fn push_delimited(&mut self, value: &str, start: u32, end: u32) {
        if let Some(range) = self.value_range(value, start, end) {
            self.push(value, range.start, range.end);
        }
    }

    /// Push a block whose text is verbatim: a fenced code block or a display
    /// math block. Its newlines are content rather than soft breaks, so they
    /// survive — the selection the frontend hands back spans several lines of
    /// a `<pre>` — and the block range is the value alone, so a selection of
    /// the whole content comes back without the fence glued to its front.
    fn verbatim_block(&mut self, value: &str, start: u32, end: u32) {
        let Some(range) = self.value_range(value, start, end) else {
            return;
        };
        self.block_source = self.base + range.start as usize..self.base + range.end as usize;
        self.push_rendered(value, range.start, range.end);
        self.end_block();
    }

    fn blocks(&mut self, nodes: &[Node<'_>]) {
        for node in nodes {
            match node {
                Node::Paragraph(node) => {
                    self.begin_block(node.span.start, node.span.end);
                    self.inlines(&node.children);
                    self.end_block();
                }
                Node::Heading(node) => {
                    self.begin_block(node.span.start, node.span.end);
                    self.inlines(&node.children);
                    self.end_block();
                }
                Node::BlockQuote(node) => self.blocks(&node.children),
                Node::List(list) => {
                    for item in list.children.iter() {
                        self.blocks(&item.children);
                    }
                }
                Node::FootnoteDefinition(node) => self.blocks(&node.children),
                Node::DefinitionList(node) => self.blocks(&node.children),
                Node::DefinitionListTerm(node) => {
                    self.begin_block(node.span.start, node.span.end);
                    self.inlines(&node.children);
                    self.end_block();
                }
                Node::DefinitionListDefinition(node) => self.blocks(&node.children),
                Node::Table(table) => {
                    for row in table.children.iter() {
                        for cell in row.children.iter() {
                            self.begin_block(cell.span.start, cell.span.end);
                            self.inlines(&cell.children);
                            self.end_block();
                        }
                    }
                }
                Node::CodeBlock(code) => {
                    self.verbatim_block(code.value, code.span.start, code.span.end);
                }
                Node::MathBlock(math) => {
                    self.verbatim_block(math.value, math.span.start, math.span.end);
                }
                _ => {}
            }
        }
    }

    fn inlines(&mut self, nodes: &[Node<'_>]) {
        for node in nodes {
            match node {
                Node::Text(text) => self.push(text.value, text.span.start, text.span.end),
                Node::InlineCode(code) => {
                    self.push_delimited(code.value, code.span.start, code.span.end);
                }
                Node::InlineMath(math) => {
                    self.push_delimited(math.value, math.span.start, math.span.end);
                }
                Node::Emphasis(node) => self.inlines(&node.children),
                Node::Strong(node) => self.inlines(&node.children),
                Node::Delete(node) => self.inlines(&node.children),
                Node::Link(node) => self.inlines(&node.children),
                Node::Superscript(node) => self.inlines(&node.children),
                Node::Subscript(node) => self.inlines(&node.children),
                _ => {}
            }
        }
    }
}

/// Build a mapping from rendered plain text to document byte positions.
fn build_source_map(source: &str, auto_link_urls: bool) -> (String, Vec<TextSegment>) {
    let (_, body, _) = extract_and_render_frontmatter(source);
    let allocator = Allocator::new();
    let options = parser_options(auto_link_urls);
    let Ok(document) = Parser::with_options(&allocator, &body, options).parse() else {
        return (String::new(), Vec::new());
    };
    let mut collector = Collector {
        body: &body,
        base: source.len() - body.len(),
        rendered: String::new(),
        segments: Vec::new(),
        block: 0,
        block_source: 0..0,
    };
    collector.blocks(&document.children);
    (collector.rendered, collector.segments)
}

/// Find the source byte range corresponding to a rendered text selection.
///
/// When the selection boundary aligns with a segment boundary, the range is
/// expanded to include surrounding formatting markers. For example, selecting
/// rendered "bold" from source `**bold**` returns the range covering `**bold**`.
fn find_source_range(
    segments: &[TextSegment],
    source_len: usize,
    rendered_start: usize,
    rendered_end: usize,
) -> Option<Range<usize>> {
    // First and last segment overlapping the selection.
    let first = segments
        .iter()
        .position(|s| s.rendered.end > rendered_start)?;
    let last = segments
        .iter()
        .rposition(|s| s.rendered.start < rendered_end)?;

    let source_start = if rendered_start <= segments[first].rendered.start {
        // The selection starts at or before this segment, so it takes the
        // markup in front of it — but only within the same block: the gap to
        // a previous paragraph or cell is structure, not markup.
        match first.checked_sub(1).map(|index| &segments[index]) {
            Some(previous) if previous.block == segments[first].block => previous.source.end,
            _ => segments[first]
                .block_source
                .start
                .min(segments[first].source.start),
        }
    } else if segments[first].is_linear() {
        let offset = rendered_start - segments[first].rendered.start;
        segments[first].source.start + offset
    } else {
        // The offset cannot be counted into this segment, so the selection
        // takes the whole of it: more source than was selected, but source
        // that does contain it.
        segments[first].source.start
    };

    let source_end = if rendered_end >= segments[last].rendered.end {
        match segments.get(last + 1) {
            Some(next) if next.block == segments[last].block => next.source.start,
            _ => segments[last]
                .block_source
                .end
                .max(segments[last].source.end),
        }
    } else if segments[last].is_linear() {
        let offset = rendered_end - segments[last].rendered.start;
        segments[last].source.start + offset
    } else {
        segments[last].source.end
    };

    (source_start <= source_end && source_end <= source_len).then_some(source_start..source_end)
}

/// Extract the Markdown source behind a selection of the rendered text.
///
/// Returns `None` when the selection cannot be located, or when it appears
/// more than once in the rendered text and the match would be a guess.
pub fn extract_source_selection(
    source: impl AsRef<str>,
    selected_text: impl AsRef<str>,
) -> Option<String> {
    // The map is built over the same text the document was rendered from,
    // so the Markdown handed back uses `\n` even when the file on disk does
    // not. That is what a paste of it should contain anyway.
    let source = crate::line_endings::normalize(source.as_ref());
    let source = source.as_ref();
    let selected_text = selected_text.as_ref();
    if selected_text.is_empty() || source.is_empty() {
        return None;
    }

    // Autolinking is the one render option that reaches the parser, and it
    // only decides whether a bare URL is wrapped in a `Link`. That splits
    // the run into more `Text` nodes without moving a byte, and touching
    // pieces are merged back together as they are collected, so the map
    // comes out the same either way — `the_map_does_not_depend_on_autolinking`
    // pins that, which is what lets this pass a fixed value rather than
    // carrying `RenderOptions` through the public signature.
    let (rendered, segments) = build_source_map(source, true);

    let mut matches = rendered.match_indices(selected_text);
    let (rendered_start, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let rendered_end = rendered_start + selected_text.len();

    let range = find_source_range(&segments, source.len(), rendered_start, rendered_end)?;
    source.get(range).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_does_not_depend_on_autolinking() {
        // The invariant the engine states is that the map and the rendering
        // agree on the text and where it came from. Autolinking moves a bare
        // URL into a `Link` node but leaves the `Text` under it untouched,
        // so it cannot move that agreement.
        for source in [
            "See https://example.com/a and www.example.com now",
            "Mail contact@example.com about <https://example.com/b>",
            "A **bold** run, `code`, and https://example.com/c.",
        ] {
            let (on_rendered, on_segments) = build_source_map(source, true);
            let (off_rendered, off_segments) = build_source_map(source, false);

            assert_eq!(on_rendered, off_rendered, "rendered text: {source:?}");
            assert_eq!(
                on_segments
                    .iter()
                    .map(|segment| (segment.rendered.clone(), segment.source.clone()))
                    .collect::<Vec<_>>(),
                off_segments
                    .iter()
                    .map(|segment| (segment.rendered.clone(), segment.source.clone()))
                    .collect::<Vec<_>>(),
                "segments: {source:?}"
            );
        }
    }

    #[test]
    fn plain_text_maps_to_itself() {
        assert_eq!(
            extract_source_selection("hello world end", "world"),
            Some("world".to_string())
        );
    }

    #[test]
    fn a_whole_emphasized_run_takes_its_markers() {
        assert_eq!(
            extract_source_selection("hello **world** end", "world"),
            Some("**world**".to_string())
        );
        assert_eq!(
            extract_source_selection("hello *italic* end", "italic"),
            Some("*italic*".to_string())
        );
        assert_eq!(
            extract_source_selection("old ~~removed~~ text", "removed"),
            Some("~~removed~~".to_string())
        );
    }

    #[test]
    fn part_of_an_emphasized_run_takes_only_the_characters() {
        assert_eq!(
            extract_source_selection("hello **world** end", "orl"),
            Some("orl".to_string())
        );
    }

    #[test]
    fn a_selection_across_markup_keeps_the_markers_inside_it() {
        assert_eq!(
            extract_source_selection("hello **world** end", "lo world e"),
            Some("lo **world** e".to_string())
        );
        assert_eq!(
            extract_source_selection("text **bold *italic*** end", "bold italic"),
            Some("**bold *italic***".to_string())
        );
    }

    #[test]
    fn delimited_values_take_their_delimiters() {
        assert_eq!(
            extract_source_selection("use `println!` here", "println!"),
            Some("`println!`".to_string())
        );
        assert_eq!(
            extract_source_selection("energy $E=mc^2$ here", "E=mc^2"),
            Some("$E=mc^2$".to_string())
        );
        assert_eq!(
            extract_source_selection("click [here](http://example.com) now", "here"),
            Some("[here](http://example.com)".to_string())
        );
    }

    #[test]
    fn the_whole_rendered_text_maps_to_the_whole_source() {
        assert_eq!(
            extract_source_selection("hello **world** end", "hello world end"),
            Some("hello **world** end".to_string())
        );
    }

    #[test]
    fn selections_inside_containers_are_found() {
        assert_eq!(
            extract_source_selection("intro\n\n> quoted **bold** here\n", "bold"),
            Some("**bold**".to_string())
        );
        assert_eq!(
            extract_source_selection("- item\n\n  second `code` paragraph\n- other\n", "code"),
            Some("`code`".to_string())
        );
        assert_eq!(
            extract_source_selection("| a | b |\n| - | - |\n| **c** | d |\n", "c"),
            Some("**c**".to_string())
        );
    }

    #[test]
    fn a_selection_does_not_reach_into_a_neighboring_block() {
        // The second paragraph is one plain segment: no markup to absorb,
        // and the gap to the first paragraph is structure.
        assert_eq!(
            extract_source_selection("first **para**\n\nsecond\n", "second"),
            Some("second".to_string())
        );
        // A block that is nothing but one emphasized run still absorbs its
        // own markers.
        assert_eq!(
            extract_source_selection("first\n\n**whole**\n", "whole"),
            Some("**whole**".to_string())
        );
    }

    #[test]
    fn the_map_parses_the_way_the_document_was_rendered() {
        // Without the CJK emphasis option the `**` around the brackets would
        // be missed here while the document shows them as bold.
        assert_eq!(
            extract_source_selection("これは**「重要」**です。", "「重要」"),
            Some("**「重要」**".to_string())
        );
    }

    #[test]
    fn a_rewritten_run_gives_back_source_that_contains_the_selection() {
        // Smart punctuation makes the rendered run longer than the source it
        // came from (`"` is one byte, `“` is three), so counting into it
        // would land past the selected word. The whole run comes back
        // instead, which still contains what was selected.
        for (source, selection) in [
            ("He said \"hello\" very loudly today", "loudly"),
            ("pages 10--20 are relevant here", "relevant"),
        ] {
            let found = extract_source_selection(source, selection).unwrap();
            assert!(
                found.contains(selection),
                "{selection:?} not in {found:?} (from {source:?})"
            );
        }
    }

    #[test]
    fn a_code_block_gives_back_its_content_without_the_fence() {
        // The fence is not inline markup around the content: absorbing it at
        // the front while the selection ends inside the block would hand back
        // an unbalanced ```-run.
        assert_eq!(
            extract_source_selection("```\nzzz\n```\n", "zzz"),
            Some("zzz".to_string())
        );
        assert_eq!(
            extract_source_selection("> ```\n> qqq\n> ```\n", "qqq"),
            Some("qqq".to_string())
        );
    }

    #[test]
    fn a_multi_line_code_selection_keeps_its_newlines() {
        // A `<pre>` renders its newlines, so the selection the frontend hands
        // back has them; a rendered text that turned them into spaces would
        // never match.
        assert_eq!(
            extract_source_selection(
                "```rust\nlet a = 1;\nlet b = 2;\n```\n",
                "let a = 1;\nlet b = 2;"
            ),
            Some("let a = 1;\nlet b = 2;".to_string())
        );
    }

    #[test]
    fn offsets_are_document_offsets_not_body_offsets() {
        assert_eq!(
            extract_source_selection("---\ntitle: x\n---\n\nsome *word* here\n", "word"),
            Some("*word*".to_string())
        );
    }

    #[test]
    fn nothing_is_returned_without_a_single_match() {
        assert_eq!(extract_source_selection("hello", ""), None);
        assert_eq!(extract_source_selection("", "hello"), None);
        assert_eq!(extract_source_selection("hello world", "xyz"), None);
        // Two occurrences are ambiguous.
        assert_eq!(extract_source_selection("word and word", "word"), None);
    }
}
