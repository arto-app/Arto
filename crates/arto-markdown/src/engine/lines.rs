//! Byte offsets to line and column positions.
//!
//! The parser reports positions as byte offsets into the text it was given,
//! which is the document with its frontmatter cut off. The frontend wants
//! 1-based lines into the whole file and 1-based columns in characters, so
//! every offset is turned into a position here, the line shifted by the
//! number of lines the frontmatter occupied. The body starts at the
//! beginning of a line, so the columns need no shift.

use std::fmt;

/// An inclusive range of source positions, written `L:C-L:C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceRange {
    start: (usize, usize),
    end: (usize, usize),
}

impl fmt::Display for SourceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ((start_line, start_column), (end_line, end_column)) = (self.start, self.end);
        write!(f, "{start_line}:{start_column}-{end_line}:{end_column}")
    }
}

/// Position lookup over the document body.
pub(super) struct LineTable<'a> {
    body: &'a str,
    /// Byte offset at which each line of `body` starts.
    line_starts: Vec<usize>,
    /// Number of lines the frontmatter occupied before `body`.
    frontmatter_lines: usize,
}

impl<'a> LineTable<'a> {
    pub(super) fn new(body: &'a str, frontmatter_lines: usize) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            body.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| index + 1),
        );
        Self {
            body,
            line_starts,
            frontmatter_lines,
        }
    }

    /// The body the offsets index into.
    pub(super) fn body(&self) -> &'a str {
        self.body
    }

    /// The range of `start..end` without the whitespace around it, or
    /// `None` when there is nothing else.
    pub(super) fn range(&self, start: usize, end: usize) -> Option<SourceRange> {
        let text = self.slice(start, end);
        let leading = text.len() - text.trim_start().len();
        self.range_from(self.boundary(start) + leading, end)
    }

    /// The range of `start..end` without the whitespace at its end, or
    /// `None` when there is nothing else. The start stays where it is, even
    /// on whitespace.
    pub(super) fn range_from(&self, start: usize, end: usize) -> Option<SourceRange> {
        let text = self.slice(start, end);
        let last = text.trim_end().chars().next_back()?;
        let start = self.boundary(start);
        let last = start + text.trim_end().len() - last.len_utf8();
        Some(SourceRange {
            start: self.position(start),
            end: self.position(last),
        })
    }

    /// The body text for `start..end`, clamped to the body and to char
    /// boundaries.
    pub(super) fn slice(&self, start: usize, end: usize) -> &'a str {
        let start = self.boundary(start);
        let end = self.boundary(end).max(start);
        &self.body[start..end]
    }

    /// 1-based line in the whole file and 1-based column, in characters, of
    /// the character at the char boundary `offset`.
    fn position(&self, offset: usize) -> (usize, usize) {
        let index = self.line_index(offset);
        let column = self.body[self.line_starts[index]..offset].chars().count() + 1;
        (self.frontmatter_lines + index + 1, column)
    }

    fn line_index(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    fn boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.body.len());
        while offset > 0 && !self.body.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(body: &str, start: usize, end: usize) -> Option<String> {
        LineTable::new(body, 0)
            .range(start, end)
            .map(|range| range.to_string())
    }

    #[test]
    fn a_range_is_written_as_inclusive_lines_and_columns() {
        assert_eq!(range("hello\nworld", 0, 11).as_deref(), Some("1:1-2:5"));
        assert_eq!(range("hello\nworld", 6, 7).as_deref(), Some("2:1-2:1"));
    }

    #[test]
    fn the_whitespace_around_a_range_is_left_out() {
        assert_eq!(range("| a |\n", 1, 4).as_deref(), Some("1:3-1:3"));
        assert_eq!(range("para\r\n\r\n", 0, 8).as_deref(), Some("1:1-1:4"));
        assert_eq!(range("   \n", 0, 4), None);
    }

    #[test]
    fn columns_count_characters() {
        assert_eq!(range("| 日本語 |", 2, 11).as_deref(), Some("1:3-1:5"));
    }

    #[test]
    fn a_range_from_keeps_its_start_on_whitespace() {
        let table = LineTable::new("```\n\nx\n```\n", 0);
        assert_eq!(
            table.range_from(4, 7).map(|range| range.to_string()),
            Some("2:1-3:1".to_string())
        );
    }

    #[test]
    fn lines_are_shifted_by_the_frontmatter() {
        let table = LineTable::new("one\ntwo\nthree", 4);
        assert_eq!(
            table.range(4, 13).map(|range| range.to_string()),
            Some("6:1-7:5".to_string())
        );
    }

    #[test]
    fn offsets_inside_a_char_are_safe() {
        let table = LineTable::new("a\n盤\nc", 0);
        // Bytes 2..5 are '盤'; an offset inside it still resolves.
        assert_eq!(
            table.range(3, 5).map(|range| range.to_string()),
            Some("2:1-2:1".to_string())
        );
    }

    #[test]
    fn slices_clamp_to_char_boundaries() {
        let table = LineTable::new("これは。\n次", 0);
        assert_eq!(table.slice(0, 5), "こ");
    }
}
