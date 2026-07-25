/// Pre-process bare URLs (`http://` / `https://`) into CommonMark autolinks (`<URL>`).
///
/// This compensates for pulldown-cmark not supporting GFM autolink literals.
/// The `<URL>` syntax is native CommonMark and rendered correctly by pulldown-cmark.
///
/// Preserves line count so that source-line annotations remain correct.
pub(super) fn preprocess_autolinks(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut in_fenced_block = false;
    let mut fence_char: u8 = 0;
    let mut fence_len: usize = 0;

    for (i, line) in markdown.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }

        if in_fenced_block {
            // Check for closing fence: same char, at least same length, only whitespace after
            if is_closing_fence(line, fence_char, fence_len) {
                in_fenced_block = false;
            }
            result.push_str(line);
        } else if let Some((ch, len)) = detect_opening_fence(line) {
            in_fenced_block = true;
            fence_char = ch;
            fence_len = len;
            result.push_str(line);
        } else {
            result.push_str(&process_line(line));
        }
    }

    // Preserve trailing newline if present
    if markdown.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// Detect an opening code fence (``` or ~~~). Returns (fence_char, fence_length).
fn detect_opening_fence(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let ch = bytes[0];
    if ch != b'`' && ch != b'~' {
        return None;
    }

    let count = bytes.iter().take_while(|&&b| b == ch).count();
    if count >= 3 {
        Some((ch, count))
    } else {
        None
    }
}

/// Check if a line is a closing fence matching the opening fence.
fn is_closing_fence(line: &str, fence_char: u8, fence_len: usize) -> bool {
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    if bytes[0] != fence_char {
        return false;
    }

    let count = bytes.iter().take_while(|&&b| b == fence_char).count();
    if count < fence_len {
        return false;
    }

    // Only whitespace may follow the closing fence
    bytes[count..].iter().all(|&b| b == b' ' || b == b'\t')
}

/// Process a single line, converting bare URLs to `<URL>` while skipping inline code spans.
fn process_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let len = bytes.len();
    // +16 bytes headroom for <> wrapping of a few URLs
    let mut result = String::with_capacity(len + 16);
    let mut pos = 0;

    while pos < len {
        // Skip inline code spans
        if bytes[pos] == b'`' {
            let backtick_start = pos;
            let backtick_count = bytes[pos..].iter().take_while(|&&b| b == b'`').count();
            pos += backtick_count;

            // Find matching closing backticks
            if let Some(close) = find_closing_backticks(&bytes[pos..], backtick_count) {
                // Copy everything including backticks verbatim
                result.push_str(&line[backtick_start..pos + close + backtick_count]);
                pos += close + backtick_count;
            } else {
                // No closing backticks found, just copy the opening backticks
                result.push_str(&line[backtick_start..backtick_start + backtick_count]);
            }
            continue;
        }

        // Look for http:// or https://
        if let Some(proto_len) = match_http_protocol(&bytes[pos..]) {
            if is_valid_predecessor(line, pos) {
                let url_end = find_url_end(&line[pos..]);
                let url = &line[pos..pos + url_end];

                // Only wrap if the URL has content after the protocol
                if url.len() > proto_len {
                    result.push('<');
                    result.push_str(url);
                    result.push('>');
                    pos += url_end;
                    continue;
                }
            }
        }

        // Default: copy character as-is
        // Handle multi-byte UTF-8 correctly
        let ch_len = utf8_char_len(bytes[pos]);
        result.push_str(&line[pos..pos + ch_len]);
        pos += ch_len;
    }

    result
}

/// Find closing backticks of the same count in the remaining bytes.
/// Returns the byte offset (from `start`) where the closing backticks begin.
fn find_closing_backticks(bytes: &[u8], count: usize) -> Option<usize> {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if run == count {
                return Some(i);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

/// Check if `http://` or `https://` starts at position 0 of the slice.
/// Returns the protocol length if matched.
fn match_http_protocol(bytes: &[u8]) -> Option<usize> {
    if bytes.starts_with(b"https://") {
        Some(8)
    } else if bytes.starts_with(b"http://") {
        Some(7)
    } else {
        None
    }
}

/// Check that the character before a URL allows autolink conversion.
///
/// Valid predecessors: start of line, whitespace, `(` (when not after `]`)
/// Invalid predecessors: `<` (already autolink), `"`, `'` (in attribute/quotes), `]` + `(` (markdown link)
fn is_valid_predecessor(line: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }

    let bytes = line.as_bytes();
    let prev = bytes[pos - 1];

    match prev {
        // Already inside <URL> or HTML attribute
        b'<' | b'"' | b'\'' => false,
        // Markdown link syntax [text](url)
        b'(' => {
            // Check if preceded by `]`
            if pos >= 2 && bytes[pos - 2] == b']' {
                return false;
            }
            true
        }
        // Whitespace is always valid
        b' ' | b'\t' => true,
        // Other characters: generally not valid (avoid mid-word matching)
        _ => false,
    }
}

/// Determine the end of a URL starting at position 0 of the input string.
///
/// Strips trailing punctuation (`.` `,` `;` `:` `!` `?` `'` `"`) and
/// balances parentheses to support Wikipedia-style URLs.
fn find_url_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut end = 0;

    // Advance past URL characters (non-whitespace, non-control, non-angle-bracket)
    while end < bytes.len() {
        let b = bytes[end];
        if b <= b' ' || b == b'<' || b == b'>' {
            break;
        }
        end += utf8_char_len(b);
    }

    // Strip trailing punctuation, considering parenthesis balance
    strip_trailing_punctuation(s, end)
}

/// Strip trailing punctuation from a URL, handling balanced parentheses.
fn strip_trailing_punctuation(s: &str, mut end: usize) -> usize {
    let bytes = s.as_bytes();

    loop {
        if end == 0 {
            break;
        }

        let last = bytes[end - 1];

        // Standard trailing punctuation to strip
        if matches!(last, b'.' | b',' | b';' | b':' | b'!' | b'?' | b'\'' | b'"') {
            end -= 1;
            continue;
        }

        // Handle closing parenthesis with balance check
        if last == b')' {
            let open_count = bytes[..end].iter().filter(|&&b| b == b'(').count();
            let close_count = bytes[..end].iter().filter(|&&b| b == b')').count();

            if close_count > open_count {
                end -= 1;
                continue;
            }
        }

        break;
    }

    end
}

/// Return the byte length of a UTF-8 character from its leading byte.
fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    // ========================================================================
    // Basic conversion tests
    // ========================================================================

    #[test]
    fn test_basic_url_start_of_line() {
        let input = "https://example.com";
        assert_eq!(preprocess_autolinks(input), "<https://example.com>");
    }

    #[test]
    fn test_basic_url_mid_line() {
        let input = "Visit https://example.com for details";
        assert_eq!(
            preprocess_autolinks(input),
            "Visit <https://example.com> for details"
        );
    }

    #[test]
    fn test_basic_url_end_of_line() {
        let input = "See https://example.com";
        assert_eq!(preprocess_autolinks(input), "See <https://example.com>");
    }

    #[test]
    fn test_http_url() {
        let input = "Visit http://example.com for info";
        assert_eq!(
            preprocess_autolinks(input),
            "Visit <http://example.com> for info"
        );
    }

    #[test]
    fn test_url_with_path() {
        let input = "https://example.com/path/to/page";
        assert_eq!(
            preprocess_autolinks(input),
            "<https://example.com/path/to/page>"
        );
    }

    #[test]
    fn test_url_with_query() {
        let input = "https://example.com/search?q=test&lang=en";
        assert_eq!(
            preprocess_autolinks(input),
            "<https://example.com/search?q=test&lang=en>"
        );
    }

    #[test]
    fn test_url_with_fragment() {
        let input = "https://example.com/page#section";
        assert_eq!(
            preprocess_autolinks(input),
            "<https://example.com/page#section>"
        );
    }

    #[test]
    fn test_multiple_urls_on_line() {
        let input = "Visit https://a.com and https://b.com today";
        assert_eq!(
            preprocess_autolinks(input),
            "Visit <https://a.com> and <https://b.com> today"
        );
    }

    #[test]
    fn test_multiple_lines() {
        let input = indoc! {"
            # Title

            https://example.com

            Some text https://other.com here
        "};
        let expected = indoc! {"
            # Title

            <https://example.com>

            Some text <https://other.com> here
        "};
        assert_eq!(preprocess_autolinks(input), expected);
    }

    // ========================================================================
    // Skip cases
    // ========================================================================

    #[test]
    fn test_skip_existing_autolink() {
        let input = "<https://example.com>";
        assert_eq!(preprocess_autolinks(input), "<https://example.com>");
    }

    #[test]
    fn test_skip_markdown_link() {
        let input = "[text](https://example.com)";
        assert_eq!(preprocess_autolinks(input), "[text](https://example.com)");
    }

    #[test]
    fn test_skip_inline_code() {
        let input = "Use `https://example.com` as URL";
        assert_eq!(
            preprocess_autolinks(input),
            "Use `https://example.com` as URL"
        );
    }

    #[test]
    fn test_skip_double_backtick_code() {
        let input = "Use ``https://example.com`` as URL";
        assert_eq!(
            preprocess_autolinks(input),
            "Use ``https://example.com`` as URL"
        );
    }

    #[test]
    fn test_skip_fenced_code_block() {
        let input = indoc! {"
            ```
            https://example.com
            ```
        "};
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_skip_fenced_code_block_with_language() {
        let input = indoc! {"
            ```rust
            let url = \"https://example.com\";
            ```
        "};
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_skip_tilde_fenced_code_block() {
        let input = indoc! {"
            ~~~
            https://example.com
            ~~~
        "};
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_skip_quoted_url() {
        let input = "The URL \"https://example.com\" is valid";
        assert_eq!(
            preprocess_autolinks(input),
            "The URL \"https://example.com\" is valid"
        );
    }

    #[test]
    fn test_skip_single_quoted_url() {
        let input = "The URL 'https://example.com' is valid";
        assert_eq!(
            preprocess_autolinks(input),
            "The URL 'https://example.com' is valid"
        );
    }

    #[test]
    fn test_skip_image_link() {
        let input = "![alt text](https://example.com/image.png)";
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_url_after_non_ascii_with_space() {
        let input = "café https://example.com for info";
        assert_eq!(
            preprocess_autolinks(input),
            "café <https://example.com> for info"
        );
    }

    // ========================================================================
    // Trailing punctuation stripping
    // ========================================================================

    #[test]
    fn test_trailing_period() {
        let input = "Visit https://example.com.";
        assert_eq!(preprocess_autolinks(input), "Visit <https://example.com>.");
    }

    #[test]
    fn test_trailing_comma() {
        let input = "See https://example.com, for info";
        assert_eq!(
            preprocess_autolinks(input),
            "See <https://example.com>, for info"
        );
    }

    #[test]
    fn test_trailing_exclamation() {
        let input = "Check https://example.com!";
        assert_eq!(preprocess_autolinks(input), "Check <https://example.com>!");
    }

    #[test]
    fn test_trailing_question() {
        let input = "Did you see https://example.com?";
        assert_eq!(
            preprocess_autolinks(input),
            "Did you see <https://example.com>?"
        );
    }

    #[test]
    fn test_trailing_multiple_punctuation() {
        let input = "See https://example.com...";
        assert_eq!(preprocess_autolinks(input), "See <https://example.com>...");
    }

    // ========================================================================
    // Parenthesis balancing
    // ========================================================================

    #[test]
    fn test_balanced_parens_wikipedia() {
        let input = "See https://en.wikipedia.org/wiki/Rust_(programming_language) for info";
        assert_eq!(
            preprocess_autolinks(input),
            "See <https://en.wikipedia.org/wiki/Rust_(programming_language)> for info"
        );
    }

    #[test]
    fn test_unbalanced_closing_paren() {
        let input = "(https://example.com)";
        assert_eq!(preprocess_autolinks(input), "(<https://example.com>)");
    }

    #[test]
    fn test_url_in_parens_with_trailing_period() {
        let input = "(See https://example.com).";
        assert_eq!(preprocess_autolinks(input), "(See <https://example.com>).");
    }

    // ========================================================================
    // Edge cases
    // ========================================================================

    #[test]
    fn test_empty_input() {
        assert_eq!(preprocess_autolinks(""), "");
    }

    #[test]
    fn test_no_urls() {
        let input = "Just some text without any URLs";
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_protocol_only() {
        let input = "https://";
        assert_eq!(preprocess_autolinks(input), "https://");
    }

    #[test]
    fn test_preserves_line_count() {
        let input = indoc! {"
            Line 1
            https://example.com
            Line 3
            Line 4
            https://other.com
        "};
        let output = preprocess_autolinks(input);
        assert_eq!(
            input.lines().count(),
            output.lines().count(),
            "Line count must be preserved"
        );
    }

    #[test]
    fn test_preserves_trailing_newline() {
        let input = "https://example.com\n";
        let output = preprocess_autolinks(input);
        assert!(output.ends_with('\n'), "Should preserve trailing newline");
    }

    #[test]
    fn test_no_trailing_newline() {
        let input = "https://example.com";
        let output = preprocess_autolinks(input);
        assert!(
            !output.ends_with('\n'),
            "Should not add trailing newline when input doesn't have one"
        );
    }

    #[test]
    fn test_url_after_code_block() {
        let input = indoc! {"
            ```
            code
            ```

            https://example.com
        "};
        let expected = indoc! {"
            ```
            code
            ```

            <https://example.com>
        "};
        assert_eq!(preprocess_autolinks(input), expected);
    }

    #[test]
    fn test_mixed_content() {
        let input = indoc! {"
            # Links

            Visit https://example.com for info.

            Already linked: <https://auto.link>

            In markdown: [click](https://link.com)

            ```
            https://in-code.com
            ```

            Back to normal https://again.com here
        "};
        let expected = indoc! {"
            # Links

            Visit <https://example.com> for info.

            Already linked: <https://auto.link>

            In markdown: [click](https://link.com)

            ```
            https://in-code.com
            ```

            Back to normal <https://again.com> here
        "};
        assert_eq!(preprocess_autolinks(input), expected);
    }

    #[test]
    fn test_url_preceded_by_open_paren_not_markdown() {
        // Standalone parenthesized URL (not markdown link)
        let input = "(https://example.com)";
        assert_eq!(preprocess_autolinks(input), "(<https://example.com>)");
    }

    #[test]
    fn test_indented_code_fence() {
        let input = indoc! {"
              ```
            https://example.com
              ```
        "};
        // Indented fences are still valid
        assert_eq!(preprocess_autolinks(input), input);
    }

    #[test]
    fn test_url_with_semicolon_trailing() {
        let input = "Visit https://example.com;";
        assert_eq!(preprocess_autolinks(input), "Visit <https://example.com>;");
    }

    #[test]
    fn test_url_with_colon_trailing() {
        let input = "See https://example.com:";
        assert_eq!(preprocess_autolinks(input), "See <https://example.com>:");
    }

    #[test]
    fn test_url_with_port() {
        let input = "See https://localhost:3000/path";
        assert_eq!(
            preprocess_autolinks(input),
            "See <https://localhost:3000/path>"
        );
    }
}
