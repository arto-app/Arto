use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::alerts::process_github_alerts;
use super::frontmatter::extract_and_render_frontmatter;

/// Information about a heading extracted from markdown
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingInfo {
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text content
    pub text: String,
    /// Generated anchor ID for linking
    pub id: String,
}

/// Generate a URL-safe slug from heading text
fn generate_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' || c == '.' {
                '-'
            } else {
                // Skip other characters (including non-ASCII)
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Extract headings from markdown content
pub(super) fn extract_headings(markdown: &str, auto_link_urls: bool) -> Vec<HeadingInfo> {
    let options = Options::all();

    // Reuse the renderer's frontmatter detection to stay in sync:
    // invalid YAML frontmatter is NOT stripped (same as render pipeline).
    let (_, content, _) = extract_and_render_frontmatter(markdown);

    // Convert bare URLs to <URL> autolinks (consistent with render pipeline, if enabled)
    let content = if auto_link_urls {
        super::autolinks::preprocess_autolinks(&content)
    } else {
        content
    };

    // Process GitHub alerts (they contain their own parsing)
    // frontmatter_lines=0 since extract_headings doesn't need source line tracking
    let (processed, _) = process_github_alerts(&content, 0);
    let parser = Parser::new_ext(&processed, options);

    let mut headings = Vec::new();
    let mut current_level: Option<u8> = None;
    let mut current_text = String::new();
    let mut slug_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_level = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
                current_text.clear();
            }
            Event::Text(text) if current_level.is_some() => {
                current_text.push_str(&text);
            }
            Event::Code(code) if current_level.is_some() => {
                current_text.push_str(&code);
            }
            Event::SoftBreak | Event::HardBreak if current_level.is_some() => {
                current_text.push(' ');
            }
            Event::End(TagEnd::Heading(_)) if current_level.is_some() => {
                let level = current_level.take().unwrap();
                let base_slug = generate_slug(&current_text);

                // Handle duplicate slugs by appending a number
                let id = if let Some(count) = slug_counts.get(&base_slug) {
                    let new_count = count + 1;
                    slug_counts.insert(base_slug.clone(), new_count);
                    format!("{}-{}", base_slug, new_count)
                } else {
                    slug_counts.insert(base_slug.clone(), 0);
                    base_slug
                };

                headings.push(HeadingInfo {
                    level,
                    text: current_text.trim().to_string(),
                    id,
                });
            }
            _ => {}
        }
    }

    headings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug() {
        assert_eq!(generate_slug("Hello World"), "hello-world");
        assert_eq!(generate_slug("My Heading"), "my-heading");
        assert_eq!(
            generate_slug("Heading with  Multiple   Spaces"),
            "heading-with-multiple-spaces"
        );
        assert_eq!(
            generate_slug("Special: Characters! Here?"),
            "special-characters-here"
        );
        assert_eq!(generate_slug("日本語"), ""); // Non-ASCII characters are stripped
        assert_eq!(generate_slug("Code `example`"), "code-example");
        assert_eq!(generate_slug("under_score"), "under-score");
    }
}
