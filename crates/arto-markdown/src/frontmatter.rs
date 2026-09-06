use serde_yaml_ng::Value as YamlValue;

/// Extract frontmatter from markdown and render it as an HTML table
///
/// Returns (frontmatter_html, content, frontmatter_lines) where frontmatter_lines
/// is the number of lines consumed by frontmatter (including delimiters and trailing whitespace).
///
/// A leading `---` … `---` block is only frontmatter when it holds a YAML
/// mapping, which is what metadata is. Anything else stays in the body: a
/// document may legitimately open with a `---` thematic break, and cutting
/// to the next `---` would delete the prose between them.
pub(super) fn extract_and_render_frontmatter(markdown: &str) -> (String, String, usize) {
    // Check if markdown starts with frontmatter delimiter
    let Some(rest) = markdown.strip_prefix("---") else {
        return (String::new(), markdown.to_string(), 0);
    };

    // The opening delimiter owns its whole line: `----` and longer runs are
    // thematic breaks, and reading one as an opening fence would drop the
    // document down to the next `---` line.
    let first_line_end = rest.find('\n').unwrap_or(rest.len());
    if !rest[..first_line_end].trim().is_empty() {
        return (String::new(), markdown.to_string(), 0);
    }

    // The closing delimiter owns its whole line too: cutting inside a longer
    // run of dashes would leave the leftover ones at the head of the body.
    let Some((yaml_end, closing_end)) = find_closing_delimiter(rest) else {
        return (String::new(), markdown.to_string(), 0);
    };

    let frontmatter_str = rest[..yaml_end].trim();
    let Ok(YamlValue::Mapping(mapping)) = serde_yaml_ng::from_str::<YamlValue>(frontmatter_str)
    else {
        return (String::new(), markdown.to_string(), 0);
    };

    let content = rest[closing_end..].trim_start();

    // Count lines consumed before content starts
    let consumed_bytes = markdown.len() - content.len();
    let frontmatter_lines = markdown[..consumed_bytes]
        .bytes()
        .filter(|&b| b == b'\n')
        .count();

    (
        render_frontmatter_table(&mapping),
        content.to_string(),
        frontmatter_lines,
    )
}

/// Locate the first line of `rest` that is exactly `---`, ignoring trailing
/// whitespace.
///
/// Returns the offset of the newline that ends the YAML and the offset just
/// past the closing line, or `None` when the block is never closed. A line of
/// four or more dashes is a thematic break, not a closing fence.
fn find_closing_delimiter(rest: &str) -> Option<(usize, usize)> {
    let mut search = 0;
    while let Some(index) = rest[search..].find("\n---") {
        let line_start = search + index + 1;
        let line_end = rest[line_start..]
            .find('\n')
            .map_or(rest.len(), |offset| line_start + offset);
        if rest[line_start..line_end].trim_end() == "---" {
            return Some((line_start - 1, line_end));
        }
        search = line_start;
    }
    None
}

/// Render YAML frontmatter as an HTML table
fn render_frontmatter_table(mapping: &serde_yaml_ng::Mapping) -> String {
    if mapping.is_empty() {
        return String::new();
    }

    let mut rows = String::new();
    for (key, value) in mapping {
        let key_str = yaml_to_string(key);
        let value_str = render_yaml_value(value);
        rows.push_str(&format!(
            "<tr><th>{}</th><td>{}</td></tr>\n",
            html_escape::encode_text(&key_str),
            value_str
        ));
    }

    format!(
        r#"<details class="frontmatter">
<summary class="frontmatter-summary">Frontmatter</summary>
<table class="frontmatter-table">
<tbody>
{}
</tbody>
</table>
</details>"#,
        rows
    )
}

/// Convert a YAML value to a string representation
fn yaml_to_string(value: &YamlValue) -> String {
    match value {
        YamlValue::Null => "null".to_string(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Number(n) => n.to_string(),
        YamlValue::String(s) => s.clone(),
        YamlValue::Sequence(seq) => seq
            .iter()
            .map(yaml_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        YamlValue::Mapping(_) => "[object]".to_string(),
        YamlValue::Tagged(tagged) => yaml_to_string(&tagged.value),
    }
}

/// Render a YAML value as HTML (with special handling for arrays and objects)
fn render_yaml_value(value: &YamlValue) -> String {
    match value {
        YamlValue::Null => "<span class=\"yaml-null\">null</span>".to_string(),
        YamlValue::Bool(b) => format!("<span class=\"yaml-bool\">{}</span>", b),
        YamlValue::Number(n) => format!("<span class=\"yaml-number\">{}</span>", n),
        YamlValue::String(s) => html_escape::encode_text(s).to_string(),
        YamlValue::Sequence(seq) => {
            if seq.is_empty() {
                return "<span class=\"yaml-empty\">[]</span>".to_string();
            }
            let items: Vec<String> = seq
                .iter()
                .map(|v| format!("<li>{}</li>", render_yaml_value(v)))
                .collect();
            format!("<ul class=\"yaml-list\">{}</ul>", items.join(""))
        }
        YamlValue::Mapping(mapping) => {
            if mapping.is_empty() {
                return "<span class=\"yaml-empty\">{{}}</span>".to_string();
            }
            let rows: Vec<String> = mapping
                .iter()
                .map(|(k, v)| {
                    format!(
                        "<tr><th>{}</th><td>{}</td></tr>",
                        html_escape::encode_text(&yaml_to_string(k)),
                        render_yaml_value(v)
                    )
                })
                .collect();
            format!(
                "<table class=\"yaml-nested-table\"><tbody>{}</tbody></table>",
                rows.join("")
            )
        }
        YamlValue::Tagged(tagged) => render_yaml_value(&tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_extract_and_render_frontmatter_basic() {
        let markdown = indoc! {"
            ---
            title: Test Document
            author: John Doe
            ---

            # Hello World
        "};

        let (html, content, _frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.contains(r#"<details class="frontmatter">"#));
        assert!(html.contains(r#"<table class="frontmatter-table""#));
        assert!(html.contains("<th>title</th>"));
        assert!(html.contains("<td>Test Document</td>"));
        assert!(html.contains("<th>author</th>"));
        assert!(html.contains("<td>John Doe</td>"));
        assert!(content.starts_with("# Hello World"));
    }

    #[test]
    fn test_extract_and_render_frontmatter_with_types() {
        let markdown = indoc! {r#"
            ---
            enabled: true
            count: 42
            empty:
            ---

            Content
        "#};

        let (html, _content, _frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.contains(r#"<span class="yaml-bool">true</span>"#));
        assert!(html.contains(r#"<span class="yaml-number">42</span>"#));
        assert!(html.contains(r#"<span class="yaml-null">null</span>"#));
    }

    #[test]
    fn test_extract_and_render_frontmatter_with_list() {
        let markdown = indoc! {"
            ---
            tags:
              - rust
              - markdown
            ---

            Content
        "};

        let (html, _content, _frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.contains(r#"<ul class="yaml-list">"#));
        assert!(html.contains("<li>rust</li>"));
        assert!(html.contains("<li>markdown</li>"));
    }

    #[test]
    fn test_extract_and_render_frontmatter_no_frontmatter() {
        let markdown = "# Just a heading\n\nSome content";

        let (html, content, _frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty());
        assert_eq!(content, markdown);
    }

    #[test]
    fn test_frontmatter_line_count() {
        let markdown = indoc! {"
            ---
            title: Test
            ---

            # Content
        "};
        let (_html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);
        assert_eq!(frontmatter_lines, 4); // "---\ntitle: Test\n---\n\n"
        assert!(content.starts_with("# Content"));
    }

    #[test]
    fn test_frontmatter_line_count_no_frontmatter() {
        let markdown = "# Just content";
        let (_html, _content, frontmatter_lines) = extract_and_render_frontmatter(markdown);
        assert_eq!(frontmatter_lines, 0);
    }

    #[test]
    fn test_extract_and_render_frontmatter_invalid_yaml() {
        // Unclosed bracket is invalid YAML, so the block is not metadata and
        // the text stays in the body rather than being deleted with it.
        let markdown = indoc! {"
            ---
            invalid: [unclosed
            ---

            Content
        "};

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty(), "Invalid YAML should produce no HTML");
        assert_eq!(content, markdown);
        assert_eq!(frontmatter_lines, 0);
    }

    #[test]
    fn test_a_block_that_is_not_a_mapping_stays_in_the_body() {
        // A document opening with a `---` rule: valid YAML (a scalar), but
        // prose, not metadata.
        let markdown = indoc! {"
            ---

            Just some prose.

            ---

            More prose.
        "};

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty());
        assert_eq!(content, markdown);
        assert_eq!(frontmatter_lines, 0);
    }

    #[test]
    fn test_a_longer_dash_run_is_a_rule_not_an_opening_fence() {
        // `----` opens a thematic break. Reading it as frontmatter would cut
        // the document down to the next rule.
        let markdown = indoc! {"
            ----

            # Changelog

            - fix: a line: with colons

            ----
        "};

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty());
        assert_eq!(content, markdown);
        assert_eq!(frontmatter_lines, 0);
    }

    #[test]
    fn test_a_longer_dash_run_is_not_a_closing_fence_either() {
        // Cutting at the first `---` inside `-----` would leave `--` at the
        // head of the body, where it renders as an en dash.
        let markdown = indoc! {"
            ---
            title: x
            -----

            body
        "};

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty());
        assert_eq!(content, markdown);
        assert_eq!(frontmatter_lines, 0);
    }

    #[test]
    fn test_closing_fence_tolerates_trailing_whitespace() {
        let markdown = "---\ntitle: Test\n--- \n\nContent\n";

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.contains("<th>title</th>"), "{html}");
        assert_eq!(content, "Content\n");
        assert_eq!(frontmatter_lines, 4);
    }

    #[test]
    fn test_extract_and_render_frontmatter_only_no_body() {
        let markdown = "---\ntitle: Test\n---\n";

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(
            html.contains("<th>title</th>"),
            "Should render frontmatter table"
        );
        assert!(
            content.is_empty(),
            "Content should be empty when no body: '{content}'"
        );
        assert!(frontmatter_lines > 0, "Should count frontmatter lines");
    }

    #[test]
    fn test_extract_and_render_frontmatter_unclosed_delimiter() {
        // Only opening --- without closing --- should not be treated as frontmatter
        let markdown = "---\ntitle: Test\nContent without closing";

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty(), "Should produce no HTML");
        assert_eq!(content, markdown, "Should return original markdown");
        assert_eq!(frontmatter_lines, 0);
    }
}
