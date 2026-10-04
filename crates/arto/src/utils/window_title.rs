use crate::state::DocumentContent;
use rust_i18n::t;
use std::borrow::Cow;
use std::path::Path;

/// Extract filename from path, returning "Unknown" if unavailable
fn extract_filename(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| t!("app.window_title.unknown_file"), Cow::Borrowed)
}

/// Generate window title from the document being read
pub fn generate_window_title(content: &DocumentContent) -> String {
    match content {
        DocumentContent::File(path) => format!("Arto - {}", extract_filename(path)),
        DocumentContent::FileError(path, _) => {
            t!("app.window_title.error", name = extract_filename(path)).into_owned()
        }
        DocumentContent::None => "Arto".to_string(),
    }
}
