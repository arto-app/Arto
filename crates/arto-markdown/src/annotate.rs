//! Turn the engine's `data-source-span` byte offsets into the
//! `data-source-line` attributes the frontend reads.
//!
//! The engine marks every block element with `data-source-span="S-E"`.
//! The frontend contract is line based and differs per element:
//! paragraphs, headings, quotes, rules and table rows carry the start
//! line; lists, items, tables and the Mermaid and math containers also
//! carry the end line; code blocks additionally carry the line their
//! content starts on. The span attribute is replaced in place, so the
//! other attributes keep their order.

use crate::lines::LineTable;
use lol_html::{element, HtmlRewriter, Settings};

/// Parse `S-E` into byte offsets.
fn parse_span(value: &str) -> Option<(usize, usize)> {
    let (start, end) = value.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}

/// Whether a code block's source starts with a fence rather than
/// indentation. The source may start inside a container, so quote markers
/// and indentation are skipped first.
fn is_fenced(source: &str) -> bool {
    let trimmed = source.trim_start_matches([' ', '\t', '>']);
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// The line attributes an element with the given tag and class carries.
fn line_attributes(
    tag: &str,
    class: &str,
    start: usize,
    end: usize,
    lines: &LineTable,
) -> Vec<(&'static str, usize)> {
    let start_line = lines.line_at(start);
    let end_line = lines.line_at_end(start, end);
    match tag {
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "hr" | "tr" => {
            vec![("data-source-line", start_line)]
        }
        "ul" | "ol" | "li" | "table" => vec![
            ("data-source-line", start_line),
            ("data-source-line-end", end_line),
        ],
        "pre" | "div" if class.contains("preprocessed-") => vec![
            ("data-source-line", start_line),
            ("data-source-line-end", end_line),
        ],
        "pre" => {
            // Fenced blocks: content starts on the line after the fence.
            // Indented blocks: content starts on the same line.
            let content_start = if is_fenced(lines.slice(start, end)) {
                start_line + 1
            } else {
                start_line
            };
            vec![
                ("data-source-line", start_line),
                ("data-source-line-end", end_line),
                ("data-source-line-start", content_start),
            ]
        }
        _ => Vec::new(),
    }
}

/// Replace every `data-source-span` with the line attributes for its element.
pub(crate) fn annotate(html: &str, lines: &LineTable) -> String {
    let mut output = Vec::new();
    let mut rewriter = HtmlRewriter::new(
        Settings::new().append_element_content_handler(element!("[data-source-span]", |el| {
            let attrs: Vec<(String, String)> = el
                .attributes()
                .iter()
                .map(|attr| (attr.name(), attr.value()))
                .collect();
            let span = attrs
                .iter()
                .find(|(name, _)| name == "data-source-span")
                .and_then(|(_, value)| parse_span(value));
            let class = attrs
                .iter()
                .find(|(name, _)| name == "class")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default();
            let line_attrs = span
                .map(|(start, end)| line_attributes(&el.tag_name(), class, start, end, lines))
                .unwrap_or_default();

            // Rebuild the attribute list so the line attributes take the
            // span's position instead of being appended at the end.
            for (name, _) in &attrs {
                el.remove_attribute(name);
            }
            for (name, value) in &attrs {
                if name == "data-source-span" {
                    for (line_name, line) in &line_attrs {
                        el.set_attribute(line_name, &line.to_string())?;
                    }
                } else {
                    el.set_attribute(name, value)?;
                }
            }
            Ok(())
        })),
        |chunk: &[u8]| output.extend_from_slice(chunk),
    );

    let _ = rewriter.write(html.as_bytes());
    let _ = rewriter.end();
    String::from_utf8(output).unwrap_or_else(|_| html.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> LineTable {
        LineTable::new(text.to_string(), 0)
    }

    #[test]
    fn paragraphs_get_their_start_line() {
        let text = "# T\n\nhi\n";
        let html = "<p data-source-span=\"5-7\">hi</p>\n";
        assert_eq!(
            annotate(html, &table(text)),
            "<p data-source-line=\"3\">hi</p>\n"
        );
    }

    #[test]
    fn code_blocks_get_all_three_attributes() {
        let text = "# T\n\n```rust\nx\n```\n";
        let html = "<pre data-source-span=\"5-18\"><code class=\"language-rust\">x\n</code></pre>";
        assert_eq!(
            annotate(html, &table(text)),
            "<pre data-source-line=\"3\" data-source-line-end=\"5\" data-source-line-start=\"4\"><code class=\"language-rust\">x\n</code></pre>"
        );
    }

    #[test]
    fn indented_and_quoted_code_blocks_are_told_apart() {
        let indented = table("    x\n");
        assert_eq!(
            annotate("<pre data-source-span=\"0-6\"><code>x\n</code></pre>", &indented),
            "<pre data-source-line=\"1\" data-source-line-end=\"1\" data-source-line-start=\"1\"><code>x\n</code></pre>"
        );
        let quoted = table("> ```\n> x\n> ```\n");
        assert!(annotate(
            "<pre data-source-span=\"0-16\"><code>x\n</code></pre>",
            &quoted
        )
        .contains("data-source-line-start=\"2\""));
    }

    #[test]
    fn lists_and_tables_carry_end_lines_and_keep_attribute_order() {
        let text = "- a\n- b\n\n| x |\n| - |\n| 1 |\n";
        let html = concat!(
            "<ul data-source-span=\"0-8\">\n<li data-source-span=\"0-4\">a</li>\n</ul>\n",
            "<ol start=\"2\" data-source-span=\"0-4\">\n",
            "<table data-source-span=\"9-27\"><tr data-source-span=\"9-14\"><td>x</td></tr></table>"
        );
        let annotated = annotate(html, &table(text));
        assert!(annotated.contains(r#"<ul data-source-line="1" data-source-line-end="2">"#));
        assert!(annotated.contains(r#"<li data-source-line="1" data-source-line-end="1">"#));
        assert!(
            annotated.contains(r#"<ol start="2" data-source-line="1" data-source-line-end="1">"#)
        );
        assert!(annotated.contains(r#"<table data-source-line="4" data-source-line-end="6">"#));
        assert!(annotated.contains(r#"<tr data-source-line="4">"#));
        assert!(!annotated.contains("data-source-span"), "{annotated}");
    }

    #[test]
    fn containers_keep_their_content_attribute_untouched() {
        let text = "```mermaid\nA-->B\n```\n";
        let html = "<pre data-source-span=\"0-20\" class=\"preprocessed-mermaid\" data-original-content=\"A--&gt;B &quot;\">A--&gt;B</pre>";
        assert_eq!(
            annotate(html, &table(text)),
            "<pre data-source-line=\"1\" data-source-line-end=\"3\" class=\"preprocessed-mermaid\" data-original-content=\"A--&gt;B &quot;\">A--&gt;B</pre>"
        );
    }

    #[test]
    fn headings_keep_their_id_after_the_line() {
        let html = "<h1 data-source-span=\"0-8\" id=\"title\" class=\"c\">Title</h1>\n";
        assert_eq!(
            annotate(html, &table("# Title\n")),
            "<h1 data-source-line=\"1\" id=\"title\" class=\"c\">Title</h1>\n"
        );
    }

    #[test]
    fn rules_stay_self_closing() {
        let html = "<hr data-source-span=\"7-10\" />\n";
        assert_eq!(
            annotate(html, &table("Above\n\n---\n")),
            "<hr data-source-line=\"3\" />\n"
        );
    }

    #[test]
    fn unknown_elements_and_malformed_spans_lose_the_attribute() {
        let html = "<td data-source-span=\"0-1\">x</td><p data-source-span=\"nope\">y</p>";
        assert_eq!(annotate(html, &table("x")), "<td>x</td><p>y</p>");
    }
}
