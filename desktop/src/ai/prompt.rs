//! Prompt template rendering. Pure logic, no IO.
//!
//! Templates are user-authored strings with `{name}` placeholders. Unknown
//! placeholders are left as-is so users can include literal `{`/`}` text
//! when needed without escaping.

use std::path::Path;

/// Inputs available to a prompt template.
#[derive(Debug, Default, Clone)]
pub struct PromptInputs<'a> {
    /// Full document content (Markdown source).
    pub content: &'a str,
    /// Selected text within the document, or empty.
    pub selection: &'a str,
    /// Absolute path of the document, or empty if no file is open.
    pub path: &'a str,
    /// Display title (file stem) of the document, or empty.
    pub title: &'a str,
}

impl<'a> PromptInputs<'a> {
    pub fn from_parts(content: &'a str, selection: &'a str, path: Option<&'a Path>) -> Self {
        let (path_str, title) = match path {
            Some(p) => {
                let path_str = p.to_str().unwrap_or("");
                let title = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                (path_str, title)
            }
            None => ("", ""),
        };
        Self {
            content,
            selection,
            path: path_str,
            title,
        }
    }
}

/// Render `template` with the given inputs.
///
/// Supported placeholders: `{content}`, `{selection}`, `{path}`, `{title}`.
/// Unknown placeholders pass through unchanged.
pub fn render(template: &str, inputs: &PromptInputs<'_>) -> String {
    let mut out = String::with_capacity(template.len() + inputs.content.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                let replacement = match name {
                    "content" => Some(inputs.content),
                    "selection" => Some(inputs.selection),
                    "path" => Some(inputs.path),
                    "title" => Some(inputs.title),
                    _ => None,
                };
                match replacement {
                    Some(value) => out.push_str(value),
                    None => {
                        // Unknown placeholder — preserve literal text.
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                // Unterminated brace — emit the rest verbatim.
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::path::PathBuf;

    fn inputs() -> PromptInputs<'static> {
        PromptInputs {
            content: "hello",
            selection: "ello",
            path: "/tmp/doc.md",
            title: "doc",
        }
    }

    #[test]
    fn renders_all_placeholders() {
        let tmpl = "c={content} s={selection} p={path} t={title}";
        assert_eq!(
            render(tmpl, &inputs()),
            "c=hello s=ello p=/tmp/doc.md t=doc"
        );
    }

    #[test]
    fn unknown_placeholder_preserved() {
        let tmpl = "{unknown} {content}";
        assert_eq!(render(tmpl, &inputs()), "{unknown} hello");
    }

    #[test]
    fn unterminated_brace_preserved() {
        let tmpl = "x = { unterminated";
        assert_eq!(render(tmpl, &inputs()), "x = { unterminated");
    }

    #[test]
    fn empty_template_returns_empty() {
        assert_eq!(render("", &inputs()), "");
    }

    #[test]
    fn multiline_template_with_indoc() {
        let tmpl = indoc! {"
            Translate the following Markdown to Japanese.
            Path: {path}

            ---
            {content}
        "};
        let out = render(tmpl, &inputs());
        assert!(out.contains("Path: /tmp/doc.md"));
        assert!(out.ends_with("---\nhello\n"));
    }

    #[test]
    fn from_parts_extracts_title_and_path() {
        let path = PathBuf::from("/a/b/notes.md");
        let inputs = PromptInputs::from_parts("body", "sel", Some(&path));
        assert_eq!(inputs.path, "/a/b/notes.md");
        assert_eq!(inputs.title, "notes");
        assert_eq!(inputs.content, "body");
        assert_eq!(inputs.selection, "sel");
    }

    #[test]
    fn from_parts_handles_missing_path() {
        let inputs = PromptInputs::from_parts("body", "", None);
        assert_eq!(inputs.path, "");
        assert_eq!(inputs.title, "");
    }

    #[test]
    fn placeholder_with_empty_value() {
        let mut input = inputs();
        input.selection = "";
        assert_eq!(render("[{selection}]", &input), "[]");
    }
}
