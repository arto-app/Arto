use super::AppState;
use crate::history::HistoryManager;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

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

    /// Navigate to a file in this tab
    pub fn navigate_to(&mut self, file: impl Into<PathBuf>) {
        let file = file.into();
        self.history.push(file.clone());
        self.content = TabContent::File(file);
    }
}

impl AppState {
    /// Get a tab by index (returns a clone)
    ///
    /// Used in Prepare phase of Two-Phase Commit.
    /// Note: Clone cost is low (~2-10 KB for typical tabs with history).
    pub fn get_tab(&self, index: usize) -> Option<Tab> {
        self.tabs.read().get(index).cloned()
    }

    /// Get a read-only copy of the current active tab
    pub fn current_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.read();
        let active_index = *self.active_tab.read();
        tabs.get(active_index).cloned()
    }

    /// Update the current active tab using a closure
    pub fn update_current_tab<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut Tab),
    {
        let active_index = *self.active_tab.read();
        let mut tabs = self.tabs.write();

        if let Some(tab) = tabs.get_mut(active_index) {
            update_fn(tab);
        }
    }

    /// Close a tab at index
    /// If all tabs are removed, automatically adds an empty tab to keep window open
    /// Returns true if tab was closed, false if index was invalid
    pub fn close_tab(&mut self, index: usize) -> bool {
        let mut tabs = self.tabs.write();

        if index >= tabs.len() {
            return false;
        }

        tabs.remove(index);

        // Update active tab index
        let current_active = *self.active_tab.read();
        let new_active = match current_active.cmp(&index) {
            std::cmp::Ordering::Greater => current_active - 1,
            std::cmp::Ordering::Equal if current_active >= tabs.len() => {
                tabs.len().saturating_sub(1)
            }
            _ => current_active,
        };

        if new_active != current_active && !tabs.is_empty() {
            self.active_tab.set(new_active);
        }

        // If all tabs removed, add empty tab (keep window open)
        if tabs.is_empty() {
            tabs.push(Tab::default());
            self.active_tab.set(0);
        }

        true
    }

    /// Insert tab at specified position
    /// Returns the index where the tab was inserted
    pub fn insert_tab(&mut self, tab: Tab, index: usize) -> usize {
        let mut tabs = self.tabs.write();
        let insert_index = index.min(tabs.len()); // Clamp to valid range
        tabs.insert(insert_index, tab);
        insert_index
    }

    /// Add a tab and optionally switch to it
    pub fn add_tab(&mut self, tab: Tab, switch_to: bool) -> usize {
        let tabs_len = self.tabs.read().len();
        let index = self.insert_tab(tab, tabs_len);
        if switch_to {
            self.switch_to_tab(index);
        }
        index
    }

    /// Add a file tab and optionally switch to it
    pub fn add_file_tab(&mut self, file: impl Into<PathBuf>, switch_to: bool) -> usize {
        self.add_tab(Tab::new(file.into()), switch_to)
    }

    /// Add an empty tab and optionally switch to it
    pub fn add_empty_tab(&mut self, switch_to: bool) -> usize {
        self.add_tab(Tab::default(), switch_to)
    }

    /// Switch to a specific tab by index
    pub fn switch_to_tab(&mut self, index: usize) {
        let tabs = self.tabs.read();
        if index < tabs.len() {
            self.active_tab.set(index);
        }
    }

    /// Check if the current active tab has no file (NoFile tab, Inline content, or FileError)
    /// None, Inline content, and FileError can be replaced when opening a file
    pub fn is_current_tab_no_file(&self) -> bool {
        self.current_tab()
            .map(|tab| tab.is_no_file())
            .unwrap_or(false)
    }

    /// Find the index of a tab that has the specified file open
    pub fn find_tab_with_file(&self, file: impl AsRef<Path>) -> Option<usize> {
        let file = file.as_ref();
        let tabs = self.tabs.read();
        tabs.iter()
            .position(|tab| tab.file().map(|f| f == file).unwrap_or(false))
    }

    /// Open a file, reusing NoFile tab or existing tab with the same file if possible
    /// Used when opening from sidebar or external sources
    pub fn open_file(&mut self, file: impl AsRef<Path>) {
        let file = file.as_ref();
        // Check if the file is already open in another tab
        if let Some(tab_index) = self.find_tab_with_file(file) {
            // Switch to the existing tab instead of creating a new one
            self.switch_to_tab(tab_index);
        } else if self.is_current_tab_no_file() {
            // If current tab is NoFile, open the file in it
            self.update_current_tab(|tab| {
                tab.navigate_to(file);
            });
        } else {
            // Otherwise, create a new tab
            self.add_file_tab(file, true);
        }
    }

    /// Navigate to a file in the current tab (for in-tab navigation like markdown links)
    /// Always opens in current tab regardless of whether file is open elsewhere
    pub fn navigate_to_file(&mut self, file: impl Into<PathBuf>) {
        self.update_current_tab(|tab| {
            tab.navigate_to(file);
        });
    }

    /// Open preferences in a tab. Reuses existing preferences tab if found.
    pub fn open_preferences(&mut self) {
        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        if let Some(index) = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences))
        {
            drop(tabs);
            self.switch_to_tab(index);
            return;
        }
        drop(tabs);

        // Check if current tab is empty (None, Inline, or FileError) - reuse it
        if self.is_current_tab_no_file() {
            self.update_current_tab(|tab| {
                tab.content = TabContent::Preferences;
            });
        } else {
            // Create new tab with preferences
            let mut tabs = self.tabs.write();
            tabs.push(Tab {
                content: TabContent::Preferences,
                history: HistoryManager::new(),
            });
            let new_index = tabs.len() - 1;
            drop(tabs);
            self.active_tab.set(new_index);
        }
    }

    /// Toggle preferences tab. Opens if not present, closes if currently active.
    pub fn toggle_preferences(&mut self) {
        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        let preferences_index = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences));
        drop(tabs);

        if let Some(index) = preferences_index {
            // Preferences tab exists - check if it's the active tab
            let active_index = *self.active_tab.read();
            if active_index == index {
                // Close the preferences tab
                self.close_tab(index);
            } else {
                // Switch to the preferences tab
                self.switch_to_tab(index);
            }
        } else {
            // No preferences tab - open new one
            self.open_preferences();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
