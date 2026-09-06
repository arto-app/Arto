use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};
use std::ops::Range;

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

/// Collect the headings of a parsed document in order, with unique ids.
///
/// Headings inside pre-rendered HTML (alert bodies) are not events and so
/// are not listed, which matches the rendered output: they get no id.
pub(super) fn collect_headings(events: &[(Event<'_>, Range<usize>)]) -> Vec<HeadingInfo> {
    let mut headings = Vec::new();
    let mut current_level: Option<u8> = None;
    let mut current_text = String::new();
    let mut slug_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (event, _) in events {
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
                current_text.push_str(text);
            }
            Event::Code(code) if current_level.is_some() => {
                current_text.push_str(code);
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
