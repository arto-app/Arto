//! Bookmark management for Quick Access feature.
//!
//! This module provides:
//! - `Bookmark`: A single bookmarked file or directory
//! - `Bookmarks`: Collection of bookmarks with persistence
//! - `BOOKMARKS`: Global static for app-wide bookmark access
//! - `BOOKMARKS_CHANGED`: Broadcast channel for cross-window sync

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::broadcast;

/// A single bookmark entry for Quick Access
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    /// Path to the bookmarked file or directory
    pub path: PathBuf,
    /// Custom display name (if None, use file/directory name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Bookmark {
    /// Create a new bookmark with the given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            name: None,
        }
    }

    /// Get display name (custom name or filename)
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or_else(|| {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
        })
    }

    /// Check if this bookmark points to a directory
    pub fn is_dir(&self) -> bool {
        self.path.is_dir()
    }

    /// Check if the bookmarked path exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

/// Bookmarks storage (saved to bookmarks.json)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Bookmarks {
    /// List of bookmarked paths
    pub items: Vec<Bookmark>,
}

impl Bookmarks {
    /// Get the bookmarks file path
    fn path() -> PathBuf {
        const FILENAME: &str = "bookmarks.json";
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("arto");
            path.push(FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(FILENAME);
            return path;
        }

        PathBuf::from(FILENAME)
    }

    /// Load bookmarks from file or return empty
    pub fn load() -> Self {
        let path = Self::path();

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save bookmarks to file
    pub fn save(&self) {
        let path = Self::path();

        tracing::debug!(path = %path.display(), count = self.items.len(), "Saving bookmarks");

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::error!(?e, "Failed to create bookmarks directory");
                return;
            }
        }

        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    tracing::error!(?e, "Failed to save bookmarks");
                }
            }
            Err(e) => {
                tracing::error!(?e, "Failed to serialize bookmarks");
            }
        }
    }

    /// Remove a bookmark by path
    pub fn remove(&mut self, path: &Path) {
        self.items.retain(|b| b.path != path);
    }

    /// Toggle bookmark (add if not present, remove if present)
    ///
    /// Returns `true` if the path is now bookmarked, `false` if removed.
    pub fn toggle(&mut self, path: impl Into<PathBuf>) -> bool {
        let path = path.into();
        if self.contains(&path) {
            self.remove(&path);
            false
        } else {
            self.items.push(Bookmark::new(path));
            true
        }
    }

    /// Check if a path is already bookmarked
    pub fn contains(&self, path: &Path) -> bool {
        self.items.iter().any(|b| b.path == path)
    }

    /// Move a bookmark from one index to another
    ///
    /// Returns `true` if the move was successful.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.items.len() || to_index >= self.items.len() {
            return false;
        }
        if from_index == to_index {
            return true;
        }

        let item = self.items.remove(from_index);
        // After removing, indices shift: if from < to, we need to insert at to - 1
        let insert_at = if from_index < to_index {
            to_index - 1
        } else {
            to_index
        };
        self.items.insert(insert_at, item);
        true
    }

    // Test-only methods
    #[cfg(test)]
    pub fn add(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if !self.contains(&path) {
            self.items.push(Bookmark::new(path));
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Global bookmarks instance
pub static BOOKMARKS: LazyLock<RwLock<Bookmarks>> =
    LazyLock::new(|| RwLock::new(Bookmarks::load()));

/// Broadcast channel for bookmark changes
///
/// All windows subscribe to this to update their UI when bookmarks change.
/// The payload is empty since subscribers should read from BOOKMARKS directly.
pub static BOOKMARKS_CHANGED: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(10).0);

/// Toggle a bookmark and broadcast the change
///
/// This is a convenience function that handles the common pattern of:
/// 1. Toggle the bookmark in BOOKMARKS
/// 2. Save to disk
/// 3. Broadcast the change to all windows
///
/// Returns `true` if the path is now bookmarked, `false` if removed.
pub fn toggle_bookmark(path: impl AsRef<Path>) -> bool {
    let result = {
        let mut bookmarks = BOOKMARKS.write();
        let result = bookmarks.toggle(path.as_ref().to_path_buf());
        bookmarks.save();
        result
    };
    BOOKMARKS_CHANGED.send(()).ok();
    result
}

/// Reorder bookmarks and broadcast the change
///
/// Moves a bookmark from `from_index` to `to_index`.
/// Returns `true` if the reorder was successful.
pub fn reorder_bookmark(from_index: usize, to_index: usize) -> bool {
    let result = {
        let mut bookmarks = BOOKMARKS.write();
        let result = bookmarks.reorder(from_index, to_index);
        if result {
            bookmarks.save();
        }
        result
    };
    if result {
        BOOKMARKS_CHANGED.send(()).ok();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bookmark_display_name() {
        let bookmark = Bookmark::new("/path/to/file.md");
        assert_eq!(bookmark.display_name(), "file.md");

        let bookmark_with_name = Bookmark {
            path: PathBuf::from("/path/to/file.md"),
            name: Some("My Notes".to_string()),
        };
        assert_eq!(bookmark_with_name.display_name(), "My Notes");
    }

    #[test]
    fn test_bookmark_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        std::fs::write(&file_path, "test").unwrap();

        let existing = Bookmark::new(&file_path);
        assert!(existing.exists());

        let non_existing = Bookmark::new("/non/existent/path.md");
        assert!(!non_existing.exists());
    }

    #[test]
    fn test_bookmarks_add_remove() {
        let mut bookmarks = Bookmarks::default();

        bookmarks.add("/path/to/file1.md");
        assert_eq!(bookmarks.len(), 1);
        assert!(bookmarks.contains(Path::new("/path/to/file1.md")));

        // Adding same path again should not duplicate
        bookmarks.add("/path/to/file1.md");
        assert_eq!(bookmarks.len(), 1);

        bookmarks.add("/path/to/file2.md");
        assert_eq!(bookmarks.len(), 2);

        bookmarks.remove(Path::new("/path/to/file1.md"));
        assert_eq!(bookmarks.len(), 1);
        assert!(!bookmarks.contains(Path::new("/path/to/file1.md")));
    }

    #[test]
    fn test_bookmarks_toggle() {
        let mut bookmarks = Bookmarks::default();

        // Toggle on
        let result = bookmarks.toggle("/path/to/file.md");
        assert!(result);
        assert!(bookmarks.contains(Path::new("/path/to/file.md")));

        // Toggle off
        let result = bookmarks.toggle("/path/to/file.md");
        assert!(!result);
        assert!(!bookmarks.contains(Path::new("/path/to/file.md")));
    }

    #[test]
    fn test_bookmarks_reorder_forward() {
        // Drag from earlier to later position
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");
        bookmarks.add("/c");
        bookmarks.add("/d");
        // [A, B, C, D] -> drag A(0) to C(2)'s drop zone -> [B, A, C, D]
        assert!(bookmarks.reorder(0, 2));
        assert_eq!(
            bookmarks
                .items
                .iter()
                .map(|b| b.path.to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["/b", "/a", "/c", "/d"]
        );
    }

    #[test]
    fn test_bookmarks_reorder_backward() {
        // Drag from later to earlier position
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");
        bookmarks.add("/c");
        bookmarks.add("/d");
        // [A, B, C, D] -> drag D(3) to B(1)'s drop zone -> [A, D, B, C]
        assert!(bookmarks.reorder(3, 1));
        assert_eq!(
            bookmarks
                .items
                .iter()
                .map(|b| b.path.to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["/a", "/d", "/b", "/c"]
        );
    }

    #[test]
    fn test_bookmarks_reorder_same_index() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");
        // Same index should return true but not change order
        assert!(bookmarks.reorder(1, 1));
        assert_eq!(
            bookmarks
                .items
                .iter()
                .map(|b| b.path.to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["/a", "/b"]
        );
    }

    #[test]
    fn test_bookmarks_reorder_invalid_index() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        // Invalid indices should return false
        assert!(!bookmarks.reorder(0, 5));
        assert!(!bookmarks.reorder(5, 0));
    }

    #[test]
    fn test_bookmarks_serialization() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/path/to/file.md");
        bookmarks.items[0].name = Some("My File".to_string());

        let json = serde_json::to_string_pretty(&bookmarks).unwrap();
        let parsed: Bookmarks = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].path, PathBuf::from("/path/to/file.md"));
        assert_eq!(parsed.items[0].name, Some("My File".to_string()));
    }
}
