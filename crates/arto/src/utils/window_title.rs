use crate::state::DocumentContent;
use std::path::Path;

/// Extract filename from path, returning "Unknown" if unavailable
fn extract_filename(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
}

/// Generate window title from the document being read
pub fn generate_window_title(content: &DocumentContent) -> String {
    match content {
        DocumentContent::File(path) => format!("Arto - {}", extract_filename(path)),
        DocumentContent::FileError(path, _) => {
            format!("Arto - {} (Error)", extract_filename(path))
        }
        DocumentContent::None => "Arto".to_string(),
    }
}
