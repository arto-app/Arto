use std::path::PathBuf;

/// Content source for a tab
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TabContent {
    /// No content (shows NoFile component)
    #[default]
    None,
    /// File from filesystem
    File(PathBuf),
    /// Inline markdown content (for welcome screen)
    Inline(String),
    /// File that cannot be opened (binary or error)
    FileError(PathBuf, String),
    /// Preferences page (browser-style settings)
    Preferences,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_content_default() {
        assert_eq!(TabContent::default(), TabContent::None);
    }

    #[test]
    fn test_tab_content_equality() {
        let path = PathBuf::from("/test/file.md");
        assert_eq!(TabContent::File(path.clone()), TabContent::File(path));
        assert_ne!(TabContent::None, TabContent::Preferences);
    }
}
