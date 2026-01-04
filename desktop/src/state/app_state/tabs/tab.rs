use super::content::TabContent;
use crate::history::HistoryManager;
use std::path::{Path, PathBuf};

/// Represents a single tab with its content and navigation history
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tab {
    pub content: TabContent,
    pub history: HistoryManager,
}

impl Tab {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        let file = file.into();
        let mut history = HistoryManager::new();
        history.push(file.clone());
        let content = TabContent::File(file);
        Self { content, history }
    }

    pub fn with_inline_content(content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            content: TabContent::Inline(content),
            history: HistoryManager::new(),
        }
    }

    /// Get the file path if this tab has a file
    pub fn file(&self) -> Option<&Path> {
        match &self.content {
            TabContent::File(path) | TabContent::FileError(path, _) => Some(path),
            _ => None,
        }
    }

    /// Check if this tab has no file (None, Inline, or FileError)
    pub fn is_no_file(&self) -> bool {
        matches!(
            self.content,
            TabContent::None | TabContent::Inline(_) | TabContent::FileError(_, _)
        )
    }

    /// Get display name for this tab (used in tab bar)
    pub fn display_name(&self) -> String {
        match &self.content {
            TabContent::File(path) | TabContent::FileError(path, _) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unnamed".to_string()),
            TabContent::Inline(_) => "Welcome".to_string(),
            TabContent::Preferences => "Preferences".to_string(),
            TabContent::None => "New Tab".to_string(),
        }
    }

    /// Navigate to a file in this tab
    pub fn navigate_to(&mut self, file: impl Into<PathBuf>) {
        let file = file.into();
        self.history.push(file.clone());
        self.content = TabContent::File(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Basic Tab tests ===

    #[test]
    fn test_tab_empty() {
        let tab = Tab::default();
        assert_eq!(tab.content, TabContent::None);
        assert!(tab.is_no_file());
    }

    #[test]
    fn test_tab_new_with_file() {
        let path = PathBuf::from("/test/file.md");
        let tab = Tab::new(path.clone());

        assert_eq!(tab.content, TabContent::File(path.clone()));
        assert_eq!(tab.file(), Some(path.as_path()));
        assert!(!tab.is_no_file());
    }

    #[test]
    fn test_tab_with_inline_content() {
        let content = "# Welcome".to_string();
        let tab = Tab::with_inline_content(content.clone());

        assert_eq!(tab.content, TabContent::Inline(content));
        assert!(tab.is_no_file());
        assert_eq!(tab.file(), None);
    }

    #[test]
    fn test_tab_is_no_file() {
        assert!(Tab::default().is_no_file());
        assert!(Tab::with_inline_content("test".to_string()).is_no_file());

        let tab = Tab {
            content: TabContent::FileError(PathBuf::from("/test"), "error".to_string()),
            ..Default::default()
        };
        assert!(tab.is_no_file());

        let tab = Tab {
            content: TabContent::File(PathBuf::from("/test")),
            ..Default::default()
        };
        assert!(!tab.is_no_file());

        let tab = Tab {
            content: TabContent::Preferences,
            ..Default::default()
        };
        assert!(!tab.is_no_file());
    }

    #[test]
    fn test_tab_navigate_to() {
        let mut tab = Tab::default();
        let path = PathBuf::from("/test/file.md");

        tab.navigate_to(path.clone());

        assert_eq!(tab.content, TabContent::File(path.clone()));
        assert_eq!(tab.file(), Some(path.as_path()));
    }

    #[test]
    fn test_tab_file() {
        let path = PathBuf::from("/test/file.md");

        let mut tab = Tab::new(path.clone());
        assert_eq!(tab.file(), Some(path.as_path()));

        tab.content = TabContent::FileError(path.clone(), "error".to_string());
        assert_eq!(tab.file(), Some(path.as_path()));

        tab.content = TabContent::None;
        assert_eq!(tab.file(), None);

        tab.content = TabContent::Inline("test".to_string());
        assert_eq!(tab.file(), None);

        tab.content = TabContent::Preferences;
        assert_eq!(tab.file(), None);
    }

    // === display_name() tests ===

    #[test]
    fn test_display_name_none() {
        let tab = Tab::default();
        assert_eq!(tab.display_name(), "New Tab");
    }

    #[test]
    fn test_display_name_file() {
        let tab = Tab::new("/path/to/document.md");
        assert_eq!(tab.display_name(), "document.md");
    }

    #[test]
    fn test_display_name_file_error() {
        let tab = Tab {
            content: TabContent::FileError(
                PathBuf::from("/path/to/binary.exe"),
                "Binary file".to_string(),
            ),
            ..Default::default()
        };
        assert_eq!(tab.display_name(), "binary.exe");
    }

    #[test]
    fn test_display_name_inline() {
        let tab = Tab::with_inline_content("# Welcome to Arto");
        assert_eq!(tab.display_name(), "Welcome");
    }

    #[test]
    fn test_display_name_preferences() {
        let tab = Tab {
            content: TabContent::Preferences,
            ..Default::default()
        };
        assert_eq!(tab.display_name(), "Preferences");
    }

    // === Edge case tests ===

    #[test]
    fn test_display_name_file_no_extension() {
        let tab = Tab::new("/path/to/README");
        assert_eq!(tab.display_name(), "README");
    }

    #[test]
    fn test_display_name_root_path() {
        let tab = Tab {
            content: TabContent::File(PathBuf::from("/")),
            ..Default::default()
        };
        assert_eq!(tab.display_name(), "Unnamed");
    }

    #[test]
    fn test_display_name_unicode_filename() {
        let tab = Tab::new("/path/to/日本語ファイル.md");
        assert_eq!(tab.display_name(), "日本語ファイル.md");
    }

    #[test]
    fn test_display_name_emoji_filename() {
        let tab = Tab::new("/path/to/notes_📝.md");
        assert_eq!(tab.display_name(), "notes_📝.md");
    }

    #[test]
    fn test_display_name_hidden_file() {
        let tab = Tab::new("/path/to/.hidden.md");
        assert_eq!(tab.display_name(), ".hidden.md");
    }
}
