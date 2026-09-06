use std::path::{Path, PathBuf};

/// A single entry in the navigation history
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub path: PathBuf,
    pub scroll_position: f64,
}

impl HistoryEntry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            scroll_position: 0.0,
        }
    }
}

/// Manages navigation history for markdown files
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryManager {
    history: Vec<HistoryEntry>,
    current_index: usize,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryManager {
    /// Create a new empty history manager
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current_index: 0,
        }
    }

    /// Push a new file to the history
    /// Clears forward history if not at the end
    pub fn push(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        // Don't add duplicate if it's the same as current
        if let Some(current) = self.current_path() {
            if current == path {
                return;
            }
        }

        if self.history.is_empty() {
            // First item
            self.history.push(HistoryEntry::new(path));
            self.current_index = 0;
        } else {
            // Remove all items after current index (forward history)
            self.history.truncate(self.current_index + 1);
            // Add new path
            self.history.push(HistoryEntry::new(path));
            self.current_index += 1;
        }
    }

    /// Save scroll position for the current history entry
    pub fn save_scroll_position(&mut self, scroll: f64) {
        if let Some(entry) = self.history.get_mut(self.current_index) {
            entry.scroll_position = scroll;
        }
    }

    /// Check if we can go back
    pub fn can_go_back(&self) -> bool {
        self.current_index > 0
    }

    /// Check if we can go forward
    pub fn can_go_forward(&self) -> bool {
        !self.history.is_empty() && self.current_index < self.history.len().saturating_sub(1)
    }

    /// Go back in history, returns the previous entry (path and scroll position)
    pub fn go_back(&mut self) -> Option<&HistoryEntry> {
        if !self.history.is_empty() && self.current_index > 0 {
            self.current_index -= 1;
            return self.current();
        }
        None
    }

    /// Go forward in history, returns the next entry (path and scroll position)
    pub fn go_forward(&mut self) -> Option<&HistoryEntry> {
        if !self.history.is_empty() && self.current_index < self.history.len().saturating_sub(1) {
            self.current_index += 1;
            return self.current();
        }
        None
    }

    /// Get the current history entry
    pub fn current(&self) -> Option<&HistoryEntry> {
        self.history.get(self.current_index)
    }

    /// Get the current file path (convenience method)
    pub fn current_path(&self) -> Option<&Path> {
        self.current().map(|entry| entry.path.as_path())
    }

    /// Get the history length
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_manager() {
        let manager = HistoryManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.current_path(), None);
        assert!(!manager.can_go_back());
        assert!(!manager.can_go_forward());
    }

    #[test]
    fn test_push_first_item() {
        let mut manager = HistoryManager::new();
        let path = Path::new("/test/file1.md");
        manager.push(path);

        assert_eq!(manager.len(), 1);
        assert_eq!(manager.current_path(), Some(path));
        assert!(!manager.can_go_back());
        assert!(!manager.can_go_forward());
    }

    #[test]
    fn test_push_multiple_items() {
        let mut manager = HistoryManager::new();
        let path1 = Path::new("/test/file1.md");
        let path2 = Path::new("/test/file2.md");
        let path3 = Path::new("/test/file3.md");

        manager.push(path1);
        manager.push(path2);
        manager.push(path3);

        assert_eq!(manager.len(), 3);
        assert_eq!(manager.current_path(), Some(path3));
        assert!(manager.can_go_back());
        assert!(!manager.can_go_forward());
    }

    #[test]
    fn test_go_back() {
        let mut manager = HistoryManager::new();
        let path1 = Path::new("/test/file1.md");
        let path2 = Path::new("/test/file2.md");

        manager.push(path1);
        manager.push(path2);

        let back = manager.go_back();
        assert_eq!(back.map(|e| e.path.as_path()), Some(path1));
        assert_eq!(manager.current_path(), Some(path1));
        assert!(!manager.can_go_back());
        assert!(manager.can_go_forward());
    }

    #[test]
    fn test_go_forward() {
        let mut manager = HistoryManager::new();
        let path1 = Path::new("/test/file1.md");
        let path2 = Path::new("/test/file2.md");

        manager.push(path1);
        manager.push(path2);
        manager.go_back();

        let forward = manager.go_forward();
        assert_eq!(forward.map(|e| e.path.as_path()), Some(path2));
        assert_eq!(manager.current_path(), Some(path2));
        assert!(manager.can_go_back());
        assert!(!manager.can_go_forward());
    }

    #[test]
    fn test_push_clears_forward_history() {
        let mut manager = HistoryManager::new();
        let path1 = Path::new("/test/file1.md");
        let path2 = Path::new("/test/file2.md");
        let path3 = Path::new("/test/file3.md");

        manager.push(path1);
        manager.push(path2);
        manager.go_back();

        // Now push a new path, should clear file2 from history
        manager.push(path3);

        assert_eq!(manager.len(), 2);
        assert_eq!(manager.current_path(), Some(path3));
        assert!(manager.can_go_back());
        assert!(!manager.can_go_forward());
    }

    #[test]
    fn test_push_duplicate_does_nothing() {
        let mut manager = HistoryManager::new();
        let path = PathBuf::from("/test/file1.md");

        manager.push(path.clone());
        manager.push(path.clone());

        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_scroll_position_saved() {
        let mut manager = HistoryManager::new();
        manager.push("/test/file1.md");
        manager.save_scroll_position(500.0);

        assert_eq!(manager.current().unwrap().scroll_position, 500.0);
    }

    #[test]
    fn test_scroll_position_preserved_on_back() {
        let mut manager = HistoryManager::new();
        manager.push("/test/file1.md");
        manager.save_scroll_position(100.0);
        manager.push("/test/file2.md");
        manager.save_scroll_position(200.0);

        let back = manager.go_back().unwrap();
        assert_eq!(back.scroll_position, 100.0);
    }
}
