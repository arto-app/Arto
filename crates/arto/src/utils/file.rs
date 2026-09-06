use std::path::Path;

/// Check if a file path has a markdown extension (.md or .markdown)
///
/// # Examples
///
/// ```no_run
/// use arto::utils::file::is_markdown_file;
///
/// assert!(is_markdown_file("notes.md"));
/// assert!(is_markdown_file("notes.markdown"));
/// assert!(!is_markdown_file("notes.txt"));
/// ```
pub fn is_markdown_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "md" || ext == "markdown")
        .unwrap_or(false)
}
