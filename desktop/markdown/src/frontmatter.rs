use serde_yaml::Value as YamlValue;

/// Extract frontmatter from markdown and render it as an HTML table
///
/// Returns (frontmatter_html, content, frontmatter_lines) where frontmatter_lines
/// is the number of lines consumed by frontmatter (including delimiters and trailing whitespace).
pub(super) fn extract_and_render_frontmatter(markdown: &str) -> (String, String, usize) {
    // Check if markdown starts with frontmatter delimiter
    if !markdown.starts_with("---") {
        return (String::new(), markdown.to_string(), 0);
    }

    // Find the closing delimiter
    let rest = &markdown[3..];
    let Some(end_pos) = rest.find("\n---") else {
        return (String::new(), markdown.to_string(), 0);
    };

    let frontmatter_str = rest[..end_pos].trim();
    let after_closing = &rest[end_pos + 4..];
    let content = after_closing.trim_start();

    // Count lines consumed before content starts
    let trimmed_len = after_closing.len() - content.len();
    let consumed_bytes = 3 + end_pos + 4 + trimmed_len;
    let frontmatter_lines = markdown[..consumed_bytes]
        .bytes()
        .filter(|&b| b == b'\n')
        .count();

    // Parse YAML
    let Ok(yaml) = serde_yaml::from_str::<YamlValue>(frontmatter_str) else {
        return (String::new(), markdown.to_string(), 0);
    };

    // Render frontmatter as table
    let html = render_frontmatter_table(&yaml);

    (html, content.to_string(), frontmatter_lines)
}

/// Render YAML frontmatter as an HTML table
fn render_frontmatter_table(yaml: &YamlValue) -> String {
    let YamlValue::Mapping(mapping) = yaml else {
        return String::new();
    };

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
        // Unclosed bracket is invalid YAML — should fall back to returning original text
        let markdown = indoc! {"
            ---
            invalid: [unclosed
            ---

            Content
        "};

        let (html, content, frontmatter_lines) = extract_and_render_frontmatter(markdown);

        assert!(html.is_empty(), "Invalid YAML should produce no HTML");
        assert_eq!(content, markdown, "Should return original markdown");
        assert_eq!(frontmatter_lines, 0, "Should report 0 frontmatter lines");
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
