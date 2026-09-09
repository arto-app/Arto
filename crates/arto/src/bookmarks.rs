//! What the reader kept: one list, read as two.
//!
//! A bookmarked *folder* is one of the tree's places — the Files face heads a
//! tree with it — and a bookmarked *file* is a starred document, which the
//! Starred face lists. Nothing is in both, so keeping them apart on disk would
//! buy nothing and cost a second list to keep in step; the two faces are the
//! two halves of this one, told apart by what the path points at.
//!
//! Every path that goes in is repaired to the spelling its own folders use
//! ([`crate::utils::paths::true_spelling`]), because everything asked of this
//! list — is it bookmarked, remove it, move it beside that one — compares
//! paths, and a caller that spelled one differently gets "no" to all of them.
//!
//! `BOOKMARKS_CHANGED` announces every change; every window listens, because
//! the list is the app's, not a window's.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::broadcast;

/// A single bookmark: a file starred, or a folder kept as a place.
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
    /// Create a new bookmark with the given path.
    ///
    /// Spelled by its own folders. Everything that answers a question about
    /// this list — is it bookmarked, remove it, move it beside that one —
    /// compares paths, and a caller that spelled one differently from the
    /// stored entry gets "no" to every one of those questions. Only what goes
    /// in is repaired: what comes back out is then already right, so nothing
    /// on a render path has to touch the filesystem.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: crate::utils::paths::true_spelling(&path.into()),
            name: None,
        }
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

/// `<platform data dir>/arto/<name>`, or `~/.arto/<name>` when the platform
/// has no data directory, or just `<name>` as a last resort.
///
/// This is for what the app writes about itself — bookmarks, the visit
/// history — as opposed to `config.json`, which a reader edits by hand and
/// which lives in the configuration directory instead.
pub(crate) fn data_file(name: &str) -> PathBuf {
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("arto");
        path.push(name);
        return path;
    }

    if let Some(mut path) = dirs::home_dir() {
        path.push(".arto");
        path.push(name);
        return path;
    }

    PathBuf::from(name)
}

impl Bookmarks {
    /// Get the bookmarks file path
    fn path() -> PathBuf {
        data_file("bookmarks.json")
    }

    /// The bookmarked directories, in the order they were arranged.
    ///
    /// These are the tree's permanent roots. Bookmarking a folder and giving
    /// the tree a place to start are the same act, so they are the same list
    /// rather than two that have to be kept in step.
    ///
    /// Unused until the tree grows more than one root.
    #[allow(dead_code)]
    pub fn places(&self) -> Vec<PathBuf> {
        self.items
            .iter()
            .filter(|bookmark| bookmark.is_dir())
            .map(|bookmark| bookmark.path.clone())
            .collect()
    }

    /// Load bookmarks from file or return empty
    pub fn load() -> Self {
        let path = Self::path();

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str::<Self>(&content)
                .map(Self::folded)
                .unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// One entry per path, however the file spells them.
    ///
    /// A file written before the spellings were repaired holds paths as they
    /// were typed. Left alone they would name the same folders as the tree
    /// does without being equal to them, so nothing done to a row would find
    /// the entry behind it — and a folder listed twice under two spellings
    /// would be drawn twice.
    fn folded(self) -> Self {
        let mut seen = std::collections::HashSet::new();
        Self {
            items: self
                .items
                .into_iter()
                .map(|item| Bookmark {
                    path: crate::utils::paths::true_spelling(&item.path),
                    ..item
                })
                .filter(|item| seen.insert(item.path.clone()))
                .collect(),
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
        let path = crate::utils::paths::true_spelling(&path.into());
        if self.contains(&path) {
            self.remove(&path);
            false
        } else {
            self.items.push(Bookmark::new(path));
            true
        }
    }

    /// Put `new` where `old` was.
    ///
    /// The position is the point: the places are in an order somebody
    /// arranged, and a bookmark that moved up a folder is still the same
    /// entry on that list. A `new` already on the list absorbs `old` rather
    /// than appearing twice.
    ///
    /// Returns `true` if `old` was there to replace.
    pub fn replace(&mut self, old: &Path, new: impl Into<PathBuf>) -> bool {
        let new = new.into();
        let Some(index) = self.items.iter().position(|item| item.path == old) else {
            return false;
        };
        if self.contains(&new) {
            self.items.remove(index);
        } else {
            self.items[index] = Bookmark::new(new);
        }
        true
    }

    /// Check if a path is already bookmarked
    pub fn contains(&self, path: &Path) -> bool {
        self.items.iter().any(|b| b.path == path)
    }

    /// Move one bookmark to just before or just after another.
    ///
    /// Named by path rather than by position, because the list is drawn in
    /// more than one place and never all of it: the tree's places are the
    /// directories out of this list, so a row's position on screen is not its
    /// position here.
    ///
    /// Which side is the caller's to decide, and it is what makes every
    /// position reachable — landing only ever *before* a row leaves no way to
    /// say "last".
    ///
    /// Returns `true` if the list changed.
    pub fn move_to(&mut self, moved: &Path, target: &Path, after: bool) -> bool {
        if moved == target {
            return false;
        }
        let Some(from) = self.items.iter().position(|item| item.path == moved) else {
            return false;
        };
        let item = self.items.remove(from);
        match self.items.iter().position(|item| item.path == target) {
            Some(at) => self.items.insert(at + usize::from(after), item),
            None => {
                self.items.insert(from, item);
                return false;
            }
        }
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

/// Replace a bookmark with another path and broadcast the change.
pub fn replace_bookmark(old: &Path, new: impl AsRef<Path>) -> bool {
    let result = {
        let mut bookmarks = BOOKMARKS.write();
        let result = bookmarks.replace(old, new.as_ref().to_path_buf());
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

/// Move a bookmark beside another and broadcast the change.
///
/// Returns `true` if the list changed.
pub fn move_bookmark(moved: &Path, target: &Path, after: bool) -> bool {
    let result = {
        let mut bookmarks = BOOKMARKS.write();
        let result = bookmarks.move_to(moved, target, after);
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

    /// The list as written down. `places()` reads the filesystem to tell a
    /// folder from a file, which these paths are neither of.
    fn paths(bookmarks: &Bookmarks) -> Vec<String> {
        bookmarks
            .items
            .iter()
            .map(|item| item.path.to_string_lossy().to_string())
            .collect()
    }

    // The spelling is repaired against the disk, which only answers on the
    // platforms whose filesystem is case-insensitive. Elsewhere the two names
    // are two directories and there is nothing to fold.
    #[cfg(target_os = "macos")]
    #[test]
    fn loading_folds_two_spellings_of_one_path_into_one_entry() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("Notes")).unwrap();

        let loaded = Bookmarks {
            items: vec![
                Bookmark::new(dir.path().join("Notes")),
                Bookmark::new("/elsewhere"),
                Bookmark::new(dir.path().join("notes")),
            ],
        }
        .folded();

        assert_eq!(
            paths(&loaded),
            vec![
                dir.path().join("Notes").to_string_lossy().to_string(),
                "/elsewhere".to_string(),
            ]
        );
    }

    #[test]
    fn moving_a_bookmark_after_another_puts_it_there() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");
        bookmarks.add("/c");

        assert!(bookmarks.move_to(Path::new("/a"), Path::new("/c"), true));

        assert_eq!(paths(&bookmarks), vec!["/b", "/c", "/a"]);
    }

    #[test]
    fn moving_a_bookmark_before_another_puts_it_there() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");
        bookmarks.add("/c");

        assert!(bookmarks.move_to(Path::new("/c"), Path::new("/a"), false));

        assert_eq!(paths(&bookmarks), vec!["/c", "/a", "/b"]);
    }

    #[test]
    fn moving_a_bookmark_past_entries_the_list_is_drawn_without() {
        // The tree's places are the directories out of this list, so the row
        // above one of them on screen need not be the entry above it here.
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/dir-a");
        bookmarks.add("/note.md");
        bookmarks.add("/dir-b");

        assert!(bookmarks.move_to(Path::new("/dir-b"), Path::new("/dir-a"), false));

        assert_eq!(paths(&bookmarks), vec!["/dir-b", "/dir-a", "/note.md"]);
    }

    #[test]
    fn moving_a_bookmark_onto_itself_changes_nothing() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");

        assert!(!bookmarks.move_to(Path::new("/a"), Path::new("/a"), true));
        assert_eq!(paths(&bookmarks), vec!["/a", "/b"]);
    }

    #[test]
    fn moving_a_bookmark_that_is_not_there_leaves_the_list_alone() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/b");

        assert!(!bookmarks.move_to(Path::new("/a"), Path::new("/gone"), true));
        assert_eq!(paths(&bookmarks), vec!["/a", "/b"]);
    }

    #[test]
    fn replacing_a_place_keeps_its_position() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");
        bookmarks.add("/w/arto/docs");
        bookmarks.add("/b");

        assert!(bookmarks.replace(Path::new("/w/arto/docs"), "/w/arto"));

        assert_eq!(paths(&bookmarks), vec!["/a", "/w/arto", "/b"]);
    }

    #[test]
    fn replacing_a_place_with_one_already_listed_only_removes_it() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/w/arto");
        bookmarks.add("/w/arto/docs");

        assert!(bookmarks.replace(Path::new("/w/arto/docs"), "/w/arto"));

        assert_eq!(paths(&bookmarks), vec!["/w/arto"]);
    }

    #[test]
    fn replacing_a_place_that_is_not_there_changes_nothing() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("/a");

        assert!(!bookmarks.replace(Path::new("/b"), "/c"));
        assert_eq!(paths(&bookmarks), vec!["/a"]);
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
