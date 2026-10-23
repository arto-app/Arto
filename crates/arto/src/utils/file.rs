use std::path::Path;

/// The extensions a Markdown file is known by, for every place that asks —
/// the check below and the Open dialogs' filter alike.
pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];

/// Check if a file path has a Markdown extension ([`MARKDOWN_EXTENSIONS`]),
/// in any case: `README.MD` is as much Markdown as `README.md`.
///
/// # Examples
///
/// ```no_run
/// use arto::utils::file::is_markdown_file;
///
/// assert!(is_markdown_file("notes.md"));
/// assert!(is_markdown_file("README.MD"));
/// assert!(!is_markdown_file("notes.txt"));
/// ```
pub fn is_markdown_file(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            MARKDOWN_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_extension_is_markdown() {
        for extension in MARKDOWN_EXTENSIONS {
            assert!(
                is_markdown_file(format!("notes.{extension}")),
                "{extension}"
            );
        }
    }

    #[test]
    fn the_case_of_the_extension_does_not_matter() {
        assert!(is_markdown_file("README.MD"));
        assert!(is_markdown_file("notes.Markdown"));
    }

    #[test]
    fn other_files_are_not_markdown() {
        assert!(!is_markdown_file("notes.txt"));
        assert!(!is_markdown_file("notes.md.bak"));
        assert!(!is_markdown_file("md"));
        assert!(!is_markdown_file(".md"));
    }
}
