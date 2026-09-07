//! Line endings, for the two readers of the source that are not the parser.
//!
//! The parser takes all three of CommonMark's line endings, but this crate
//! reads the source itself as well, and the two readers want different
//! things:
//!
//! * [`normalize`] turns a lone `\r` into `\n` byte for byte, so every offset
//!   the parser reports keeps naming the same character. The line table
//!   counts `\n` to number the lines an element covers, so a file written
//!   with lone `\r` would otherwise render correctly and report every block
//!   as line 1. A `\r\n` is left as it is: dropping its `\r` would shorten
//!   the text and move every offset after it, and `\n` already ends that
//!   line.
//! * [`to_lf`] drops the `\r` of a `\r\n` as well, for the selection source
//!   map. That map indexes text of its own rather than the file, and both
//!   sides it compares are `\n`-only: the browser reports a selection with
//!   `\n`, and the parser hands over a code block's value with the `\r`
//!   already taken out — which then no longer matches the CRLF source it
//!   came from, so the block would drop out of the map entirely.

use std::borrow::Cow;

/// Replace every lone `\r` with `\n`, borrowing when there is nothing to do.
pub(crate) fn normalize(source: &str) -> Cow<'_, str> {
    if !has_lone_cr(source) {
        return Cow::Borrowed(source);
    }

    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(index) = rest.find('\r') {
        out.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        out.push(if after.starts_with('\n') { '\r' } else { '\n' });
        rest = after;
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Replace every line ending with `\n`, borrowing when there is nothing to do.
pub(crate) fn to_lf(source: &str) -> Cow<'_, str> {
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

/// Whether `source` holds a `\r` that is not part of a `\r\n`.
///
/// Every document goes through this, so the scan hops from `\r` to `\r` with
/// `find` rather than walking each byte.
fn has_lone_cr(source: &str) -> bool {
    let mut rest = source;
    while let Some(index) = rest.find('\r') {
        rest = &rest[index + 1..];
        if !rest.starts_with('\n') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_without_a_lone_cr_is_borrowed() {
        assert!(matches!(normalize("a\nb"), Cow::Borrowed(_)));
        assert!(matches!(normalize("a\r\nb"), Cow::Borrowed(_)));
    }

    #[test]
    fn a_lone_cr_becomes_a_newline() {
        assert_eq!(normalize("a\rb\rc"), "a\nb\nc");
        assert_eq!(normalize("trailing\r"), "trailing\n");
    }

    #[test]
    fn a_crlf_next_to_a_lone_cr_keeps_both_bytes() {
        assert_eq!(normalize("a\r\nb\rc"), "a\r\nb\nc");
    }

    #[test]
    fn the_text_keeps_its_length() {
        let source = "a\rb\r\nc\r";
        assert_eq!(normalize(source).len(), source.len());
    }

    #[test]
    fn to_lf_replaces_every_line_ending() {
        assert!(matches!(to_lf("a\nb\n"), Cow::Borrowed("a\nb\n")));
        assert_eq!(to_lf("a\r\nb\r\nc"), "a\nb\nc");
        assert_eq!(to_lf("a\rb\rc"), "a\nb\nc");
        assert_eq!(to_lf("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn to_lf_keeps_the_line_count() {
        let crlf = "one\r\ntwo\r\n\r\nthree\r\n";
        assert_eq!(
            to_lf(crlf).lines().count(),
            crlf.lines().count(),
            "normalizing must not add or drop a line"
        );
    }
}
