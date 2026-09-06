//! Byte offsets to line numbers.
//!
//! The engine reports positions as byte offsets into the text it parsed.
//! The frontend wants 1-based line numbers into the whole file, so every
//! offset is turned into a line here: shifted by the lines the frontmatter
//! took, and mapped through the origin table when the parsed text is not
//! the file's text line for line (the alert rewrite expands one quoted
//! line into several lines of HTML).

/// Line lookup over the text the engine parsed.
pub(crate) struct LineTable {
    text: String,
    /// Byte offset at which each line of `text` starts.
    line_starts: Vec<usize>,
    /// For each line of `text`, the 0-based line of the original body it
    /// came from. Absent when the text is the body itself.
    origins: Option<Vec<usize>>,
    /// Number of lines the frontmatter occupied before the body.
    frontmatter_lines: usize,
}

impl LineTable {
    pub(crate) fn new(text: String, frontmatter_lines: usize) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| index + 1),
        );
        Self {
            text,
            line_starts,
            origins: None,
            frontmatter_lines,
        }
    }

    /// Map each line of the text back to the body line it came from.
    pub(crate) fn with_origins(mut self, origins: Vec<usize>) -> Self {
        self.origins = Some(origins);
        self
    }

    /// 1-based line in the whole file of the byte at `offset`.
    pub(crate) fn line_at(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        let index = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let original = match &self.origins {
            Some(origins) => origins.get(index).copied().unwrap_or(index),
            None => index,
        };
        original + 1 + self.frontmatter_lines
    }

    /// 1-based line of the last byte in `start..end`.
    pub(crate) fn line_at_end(&self, start: usize, end: usize) -> usize {
        self.line_at(end.saturating_sub(1).max(start))
    }

    /// The text for `start..end`, clamped to the text and to char boundaries.
    pub(crate) fn slice(&self, start: usize, end: usize) -> &str {
        let start = self.boundary(start);
        let end = self.boundary(end).max(start);
        &self.text[start..end]
    }

    fn boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while offset > 0 && !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> LineTable {
        LineTable::new(text.to_string(), 0)
    }

    #[test]
    fn offsets_map_to_one_based_lines() {
        assert_eq!(table("hello").line_at(0), 1);
        assert_eq!(table("hello\nworld").line_at(0), 1);
        assert_eq!(table("hello\nworld").line_at(6), 2);
        assert_eq!(table("hello\nworld").line_at(5), 1);
        assert_eq!(table("a\nb\nc\n").line_at(0), 1);
        assert_eq!(table("a\nb\nc\n").line_at(2), 2);
        assert_eq!(table("a\nb\nc\n").line_at(4), 3);
        // Offset beyond text length is clamped
        assert_eq!(table("hi").line_at(100), 1);
    }

    #[test]
    fn offsets_inside_a_char_are_safe() {
        let text = "a\n盤\nc";
        let mid_char = 3; // inside '盤' (bytes are 2..5)
        assert_eq!(table(text).line_at(mid_char), 2);
    }

    #[test]
    fn lines_are_shifted_by_the_frontmatter() {
        let table = LineTable::new("one\ntwo\nthree".to_string(), 4);
        assert_eq!(table.line_at(0), 5);
        assert_eq!(table.line_at(4), 6);
        assert_eq!(table.line_at_end(0, 8), 6);
        assert_eq!(table.line_at(100), 7);
    }

    #[test]
    fn origins_redirect_lines_to_the_body() {
        // Five lines of text that came from body lines 0, 3, 3, 3 and 5.
        let table = LineTable::new("# Title\n\nParagraph A\n\nParagraph B".to_string(), 2)
            .with_origins(vec![0, 3, 3, 3, 5]);
        assert_eq!(table.line_at(0), 3);
        assert_eq!(table.line_at(9), 6);
        assert_eq!(table.line_at(22), 8);
    }

    #[test]
    fn slices_clamp_to_char_boundaries() {
        let table = table("これは。\n次");
        assert_eq!(table.slice(0, 5), "こ");
        assert_eq!(table.line_at_end(0, 14), 2);
    }
}
