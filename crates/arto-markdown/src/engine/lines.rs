//! Byte offsets to line numbers.
//!
//! The parser reports positions as byte offsets into the text it was given,
//! which is the document with its frontmatter cut off. The frontend wants
//! 1-based line numbers into the whole file, so every offset is turned into a
//! line here, shifted by the number of lines the frontmatter occupied.

/// Line lookup over the document body.
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

    /// 1-based line in the whole file of the byte at `offset`.
    pub(super) fn line_at(&self, offset: usize) -> usize {
        let offset = offset.min(self.body.len());
        let line = self.line_starts.partition_point(|&start| start <= offset);
        self.frontmatter_lines + line.max(1)
    }

    /// 1-based line of the last byte in `start..end`.
    pub(super) fn line_at_end(&self, start: usize, end: usize) -> usize {
        self.line_at(end.saturating_sub(1).max(start))
    }

    /// The body text for `start..end`, clamped to the body and to char
    /// boundaries.
    pub(super) fn slice(&self, start: usize, end: usize) -> &'a str {
        let start = self.boundary(start);
        let end = self.boundary(end).max(start);
        &self.body[start..end]
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

    #[test]
    fn offsets_map_to_one_based_lines() {
        let table = LineTable::new("hello\nworld", 0);
        assert_eq!(table.line_at(0), 1);
        assert_eq!(table.line_at(5), 1);
        assert_eq!(table.line_at(6), 2);

        let table = LineTable::new("a\nb\nc\n", 0);
        assert_eq!(table.line_at(0), 1);
        assert_eq!(table.line_at(2), 2);
        assert_eq!(table.line_at(4), 3);
    }

    #[test]
    fn lines_are_shifted_by_the_frontmatter() {
        let table = LineTable::new("one\ntwo\nthree", 4);
        assert_eq!(table.line_at(0), 5);
        assert_eq!(table.line_at(4), 6);
        assert_eq!(table.line_at_end(0, 8), 6);
        // An offset past the end is clamped to the last line.
        assert_eq!(table.line_at(100), 7);
    }

    #[test]
    fn offsets_inside_a_char_are_safe() {
        let table = LineTable::new("a\n盤\nc", 0);
        // Bytes 2..5 are '盤'; an offset inside it still resolves.
        assert_eq!(table.line_at(3), 2);
    }

    #[test]
    fn slices_clamp_to_char_boundaries() {
        let table = LineTable::new("これは。\n次", 0);
        assert_eq!(table.slice(0, 5), "こ");
        assert_eq!(table.line_at_end(0, 14), 2);
    }
}
