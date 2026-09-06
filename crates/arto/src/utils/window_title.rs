use crate::state::TabContent;
use std::path::Path;

/// Extract filename from path, returning "Unknown" if unavailable
fn extract_filename(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
}

/// Generate window title based on active tab content
pub fn generate_window_title(tab_content: &TabContent) -> String {
    match tab_content {
        TabContent::File(path) => format!("Arto - {}", extract_filename(path)),
        TabContent::Inline(_) => "Arto - Welcome".to_string(),
        TabContent::Preferences => "Arto - Preferences".to_string(),
        TabContent::FileError(path, _) => format!("Arto - {} (Error)", extract_filename(path)),
        TabContent::None => "Arto".to_string(),
    }
}
