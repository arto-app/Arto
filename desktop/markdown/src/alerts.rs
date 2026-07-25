use pulldown_cmark::{html, Options, Parser};

use super::source_lines::{byte_offset_to_line, inject_source_lines_impl};

/// Get SVG icon placeholder for alert type (actual SVG injected by JavaScript)
fn get_alert_icon_placeholder(alert_type: &str) -> String {
    format!(
        r#"<span class="alert-icon" data-alert-type="{}"></span>"#,
        alert_type
    )
}

/// Check if a line starts a GitHub alert and return alert info
fn parse_alert_start(line: &str) -> Option<(&'static str, &'static str, &str)> {
    const ALERT_TYPES: [(&str, &str); 5] = [
        ("NOTE", "note"),
        ("TIP", "tip"),
        ("IMPORTANT", "important"),
        ("WARNING", "warning"),
        ("CAUTION", "caution"),
    ];

    for &(alert_name, alert_class) in &ALERT_TYPES {
        if let Some(rest) = line.strip_prefix(&format!("> [!{}]", alert_name)) {
            return Some((alert_name, alert_class, rest));
        }
    }
    None
}

/// Process a single alert block and return HTML lines and next index.
///
/// Injects `data-source-line` attributes into alert HTML elements so that
/// content inside alerts can be traced back to source line numbers.
fn process_alert_block(
    lines: &[&str],
    start_index: usize,
    alert_name: &str,
    alert_class: &str,
    first_line_content: &str,
    frontmatter_lines: usize,
) -> (Vec<String>, usize) {
    let mut html_lines = Vec::new();

    // Alert opening tag with source line
    let alert_source_line = start_index + 1 + frontmatter_lines;
    html_lines.push(format!(
        r#"<div class="markdown-alert markdown-alert-{}" data-source-line="{}" dir="auto">"#,
        alert_class, alert_source_line
    ));

    // Alert title with icon
    let icon_placeholder = get_alert_icon_placeholder(alert_class);
    html_lines.push(format!(
        r#"<p class="markdown-alert-title" dir="auto">{}{}</p>"#,
        icon_placeholder, alert_name
    ));

    // Collect alert content as markdown, tracking original line indices
    let mut content_lines = Vec::new();
    let mut content_origins: Vec<usize> = Vec::new(); // 0-based original line index
    if !first_line_content.trim().is_empty() {
        content_lines.push(first_line_content.trim().to_string());
        content_origins.push(start_index);
    }

    // Collect following quoted lines
    let mut i = start_index + 1;
    while i < lines.len() && lines[i].starts_with('>') {
        if let Some(content) = lines[i].strip_prefix('>') {
            // Preserve the structure by keeping leading space after '>'
            content_lines.push(content.trim_start().to_string());
            content_origins.push(i);
        }
        i += 1;
    }

    // Render the collected content as markdown with source line annotations
    if !content_lines.is_empty() {
        let content_markdown = content_lines.join("\n");
        let options = Options::all();
        let parser = Parser::new_ext(&content_markdown, options).into_offset_iter();
        let parser = inject_source_lines_impl(parser, |byte_offset| {
            let content_line = byte_offset_to_line(&content_markdown, byte_offset) - 1; // 0-based
            let original_line = content_origins
                .get(content_line)
                .copied()
                .unwrap_or(content_line);
            original_line + 1 + frontmatter_lines // 1-based
        });
        let mut content_html = String::new();
        html::push_html(&mut content_html, parser);
        html_lines.push(content_html);
    }

    html_lines.push("</div>".to_string());

    (html_lines, i)
}

/// Process GitHub alert format.
///
/// Returns `(processed_text, line_origins)` where `line_origins[i]` is the 0-based
/// line index in the original `markdown` that corresponds to line `i` of the processed text.
/// This mapping is used by `inject_source_lines` to compute correct original line numbers
/// even when alert conversion changes the number of lines.
pub(super) fn process_github_alerts(
    markdown: &str,
    frontmatter_lines: usize,
) -> (String, Vec<usize>) {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut result = Vec::new();
    let mut line_origins = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if let Some((alert_name, alert_class, rest)) = parse_alert_start(line) {
            let (alert_html, next_index) =
                process_alert_block(&lines, i, alert_name, alert_class, rest, frontmatter_lines);
            // Each html_line may contain embedded newlines (e.g., from push_html),
            // so count actual lines contributed to the joined output
            for html_line in &alert_html {
                let num_lines = html_line.bytes().filter(|&b| b == b'\n').count() + 1;
                for _ in 0..num_lines {
                    line_origins.push(i);
                }
            }
            result.extend(alert_html);
            i = next_index;
        } else {
            line_origins.push(i);
            result.push(line.to_string());
            i += 1;
        }
    }

    (result.join("\n"), line_origins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_get_alert_icon_placeholder() {
        let result = get_alert_icon_placeholder("note");
        assert_eq!(
            result,
            r#"<span class="alert-icon" data-alert-type="note"></span>"#
        );

        let result = get_alert_icon_placeholder("warning");
        assert_eq!(
            result,
            r#"<span class="alert-icon" data-alert-type="warning"></span>"#
        );
    }

    #[test]
    fn test_process_github_alerts_note() {
        let input = indoc! {"
            > [!NOTE]
            > This is a note
        "};
        let (result, _) = process_github_alerts(input, 0);

        assert!(result.contains(r#"<div class="markdown-alert markdown-alert-note""#));
        assert!(result.contains(r#"<p class="markdown-alert-title""#));
        assert!(result.contains("NOTE"));
        assert!(result.contains("This is a note"));
        assert!(result.contains("</div>"));
    }

    #[test]
    fn test_process_github_alerts_warning() {
        let input = indoc! {"
            > [!WARNING]
            > Be careful!
        "};
        let (result, _) = process_github_alerts(input, 0);

        assert!(result.contains(r#"markdown-alert-warning"#));
        assert!(result.contains("WARNING"));
        assert!(result.contains("Be careful!"));
    }

    #[test]
    fn test_process_github_alerts_with_multiline() {
        let input = indoc! {"
            > [!IMPORTANT]
            > First line
            > Second line
            > Third line
        "};
        let (result, _) = process_github_alerts(input, 0);

        assert!(result.contains(r#"markdown-alert-important"#));
        assert!(result.contains("First line"));
        assert!(result.contains("Second line"));
        assert!(result.contains("Third line"));
    }

    #[test]
    fn test_process_github_alerts_all_types() {
        let alert_types = vec![
            ("NOTE", "note"),
            ("TIP", "tip"),
            ("IMPORTANT", "important"),
            ("WARNING", "warning"),
            ("CAUTION", "caution"),
        ];

        for (alert_name, alert_class) in alert_types {
            let input = format!("> [!{}]\n> Test content", alert_name);
            let (result, _) = process_github_alerts(&input, 0);

            assert!(
                result.contains(&format!(r#"markdown-alert-{}"#, alert_class)),
                "Should contain alert class for {}",
                alert_name
            );
            assert!(
                result.contains(alert_name),
                "Should contain alert name {}",
                alert_name
            );
        }
    }

    #[test]
    fn test_process_github_alerts_no_match() {
        let input = "Regular paragraph\n> Regular quote";
        let (result, line_origins) = process_github_alerts(input, 0);

        assert_eq!(result, input);
        assert!(!result.contains("markdown-alert"));
        // Without alerts, each line maps to itself
        assert_eq!(line_origins, vec![0, 1]);
    }

    #[test]
    fn test_process_github_alerts_line_origins() {
        let input = indoc! {"
            # Title

            > [!NOTE]
            > Content

            After alert
        "};
        let (_, line_origins) = process_github_alerts(input, 0);

        // Line 0: "# Title" → maps to original line 0
        assert_eq!(line_origins[0], 0);
        // Line 1: "" → maps to original line 1
        assert_eq!(line_origins[1], 1);
        // Lines 2..N: alert HTML lines → all map to original line 2 (the alert start)
        for &origin in &line_origins[2..line_origins.len() - 2] {
            assert_eq!(origin, 2, "Alert HTML lines should map to line 2");
        }
        // Last lines: "" and "After alert" → map to original lines 4 and 5
        let last = line_origins.len();
        assert_eq!(line_origins[last - 2], 4);
        assert_eq!(line_origins[last - 1], 5);
    }
}
