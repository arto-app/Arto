//! Normalize `\r\n` and lone `\r` to `\n` before anything reads the text.
//!
//! CommonMark counts `\r\n`, `\r` and `\n` all as line endings; ox-content
//! does not treat the other two that way everywhere. What survives a CRLF
//! document is a tight list rendered loose — the reader sees paragraph
//! margins that the same file in LF does not have — and a `\r` left inside
//! every line of a fenced code block, which reaches whatever copies or
//! highlights it. Windows checkouts hand Arto exactly that. Remove this
//! module once ox-content handles the other two line endings itself.
//!
//! Both replacements keep the number of lines, so `data-source-line` still
//! names the line of the file the user has open, and the selection source
//! map works over the same normalized text it renders.

use std::borrow::Cow;

/// Replace every line ending with `\n`, borrowing when there is nothing to do.
pub(crate) fn normalize(source: &str) -> Cow<'_, str> {
    if !source.contains('\r') {
        return Cow::Borrowed(source);
    }

    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(index) = rest.find('\r') {
        out.push_str(&rest[..index]);
        out.push('\n');
        rest = &rest[index + 1..];
        rest = rest.strip_prefix('\n').unwrap_or(rest);
    }
    out.push_str(rest);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_without_carriage_returns_is_borrowed() {
        assert!(matches!(normalize("a\nb\n"), Cow::Borrowed("a\nb\n")));
    }

    #[test]
    fn every_line_ending_becomes_a_newline() {
        assert_eq!(normalize("a\r\nb\r\nc"), "a\nb\nc");
        assert_eq!(normalize("a\rb\rc"), "a\nb\nc");
        assert_eq!(normalize("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn the_line_count_is_preserved() {
        let crlf = "one\r\ntwo\r\n\r\nthree\r\n";
        assert_eq!(
            normalize(crlf).lines().count(),
            crlf.lines().count(),
            "normalizing must not add or drop a line"
        );
    }

    #[test]
    fn a_trailing_carriage_return_still_ends_its_line() {
        assert_eq!(normalize("a\r"), "a\n");
    }
}
