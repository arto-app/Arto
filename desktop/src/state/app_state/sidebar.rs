use super::AppState;
use crate::history::HistoryManager;
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Represents the state of the sidebar file explorer
#[derive(Debug, Clone, PartialEq)]
pub struct Sidebar {
    pub pinned: bool,
    pub root_directory: Option<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,
    pub width: f64,
    pub show_all_files: bool,
    pub zoom_level: f64,
    /// History of root directory navigation.
    ///
    /// This history is intentionally kept in-memory only and is not persisted
    /// across application restarts. Each new session starts with a clean
    /// navigation history to avoid storing potentially stale directory paths
    /// on disk and to provide a fresh navigation experience.
    dir_history: HistoryManager,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            pinned: false,
            root_directory: None,
            expanded_dirs: HashSet::new(),
            width: 280.0,
            show_all_files: false,
            zoom_level: 1.0,
            dir_history: HistoryManager::new(),
        }
    }
}

impl Sidebar {
    /// Toggle directory expansion state
    pub fn toggle_expansion(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        if self.expanded_dirs.contains(path) {
            self.expanded_dirs.remove(path);
        } else {
            self.expanded_dirs.insert(path.to_owned());
        }
    }

    /// Check if we can go back in directory history
    pub fn can_go_back(&self) -> bool {
        self.dir_history.can_go_back()
    }

    /// Check if we can go forward in directory history
    pub fn can_go_forward(&self) -> bool {
        self.dir_history.can_go_forward()
    }

    /// Push a directory to history
    pub fn push_to_history(&mut self, path: impl Into<PathBuf>) {
        self.dir_history.push(path);
    }

    /// Go back in directory history
    pub fn go_back(&mut self) -> Option<PathBuf> {
        self.dir_history.go_back().map(|e| e.path.clone())
    }

    /// Go forward in directory history
    pub fn go_forward(&mut self) -> Option<PathBuf> {
        self.dir_history.go_forward().map(|e| e.path.clone())
    }
}

impl AppState {
    /// Toggle sidebar between pinned (flex layout) and unpinned (overlay/hover).
    ///
    /// - Pinned: visible in flex layout, pushes content aside
    /// - Unpinned: accessible via hover as an overlay
    pub fn toggle_sidebar(&mut self) {
        let mut sidebar = self.sidebar.write();
        sidebar.pinned = !sidebar.pinned;
    }

    /// Toggle directory expansion state
    pub fn toggle_directory_expansion(&mut self, path: impl AsRef<Path>) {
        let mut sidebar = self.sidebar.write();
        sidebar.toggle_expansion(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_default() {
        let sidebar = Sidebar::default();

        assert!(!sidebar.pinned);
        assert_eq!(sidebar.width, 280.0);
        assert!(!sidebar.show_all_files);
        assert_eq!(sidebar.zoom_level, 1.0);
        assert!(sidebar.expanded_dirs.is_empty());
    }

    #[test]
    fn test_sidebar_toggle_expansion() {
        let mut sidebar = Sidebar::default();
        let path = PathBuf::from("/test/dir");

        // Initially empty
        assert!(!sidebar.expanded_dirs.contains(&path));

        // First toggle - expands
        sidebar.toggle_expansion(path.clone());
        assert!(sidebar.expanded_dirs.contains(&path));

        // Second toggle - collapses
        sidebar.toggle_expansion(path.clone());
        assert!(!sidebar.expanded_dirs.contains(&path));
    }

    #[test]
    fn test_sidebar_toggle_multiple_paths() {
        let mut sidebar = Sidebar::default();
        let path1 = PathBuf::from("/test/dir1");
        let path2 = PathBuf::from("/test/dir2");

        sidebar.toggle_expansion(path1.clone());
        sidebar.toggle_expansion(path2.clone());

        assert!(sidebar.expanded_dirs.contains(&path1));
        assert!(sidebar.expanded_dirs.contains(&path2));

        sidebar.toggle_expansion(path1.clone());

        assert!(!sidebar.expanded_dirs.contains(&path1));
        assert!(sidebar.expanded_dirs.contains(&path2));
    }

    /// Simulates the toggle logic from `AppState::toggle_sidebar()`.
    /// The actual method operates on `Signal<Sidebar>`, but the state
    /// transition logic is identical.
    fn apply_toggle(sidebar: &mut Sidebar) {
        sidebar.pinned = !sidebar.pinned;
    }

    #[test]
    fn test_toggle_from_unpinned_to_pinned() {
        let mut sidebar = Sidebar::default();
        assert!(!sidebar.pinned);

        apply_toggle(&mut sidebar);
        assert!(sidebar.pinned);
    }

    #[test]
    fn test_toggle_from_pinned_to_unpinned() {
        let mut sidebar = Sidebar {
            pinned: true,
            ..Default::default()
        };

        apply_toggle(&mut sidebar);
        assert!(!sidebar.pinned);
    }

    #[test]
    fn test_toggle_full_cycle() {
        // unpinned → pinned → unpinned → pinned
        let mut sidebar = Sidebar::default();
        assert!(!sidebar.pinned);

        apply_toggle(&mut sidebar);
        assert!(sidebar.pinned);

        apply_toggle(&mut sidebar);
        assert!(!sidebar.pinned);

        apply_toggle(&mut sidebar);
        assert!(sidebar.pinned);

        apply_toggle(&mut sidebar);
        assert!(!sidebar.pinned);
    }

    #[test]
    fn test_sidebar_history_initial_state() {
        let sidebar = Sidebar::default();

        // Initially, no history to navigate
        assert!(!sidebar.can_go_back());
        assert!(!sidebar.can_go_forward());
    }

    #[test]
    fn test_sidebar_history_push_and_back() {
        let mut sidebar = Sidebar::default();
        let path1 = PathBuf::from("/test/dir1");
        let path2 = PathBuf::from("/test/dir2");

        sidebar.push_to_history(path1.clone());
        sidebar.push_to_history(path2.clone());

        // After pushing two paths, we can go back
        assert!(sidebar.can_go_back());
        assert!(!sidebar.can_go_forward());

        // Go back returns the previous path
        let back = sidebar.go_back();
        assert_eq!(back, Some(path1.clone()));

        // Now we can go forward but not back
        assert!(!sidebar.can_go_back());
        assert!(sidebar.can_go_forward());
    }

    #[test]
    fn test_sidebar_history_forward() {
        let mut sidebar = Sidebar::default();
        let path1 = PathBuf::from("/test/dir1");
        let path2 = PathBuf::from("/test/dir2");

        sidebar.push_to_history(path1.clone());
        sidebar.push_to_history(path2.clone());

        // Go back first
        let _ = sidebar.go_back();

        // Now go forward
        let forward = sidebar.go_forward();
        assert_eq!(forward, Some(path2));

        // Can't go forward anymore
        assert!(!sidebar.can_go_forward());
        assert!(sidebar.can_go_back());
    }

    #[test]
    fn test_sidebar_history_push_clears_forward() {
        let mut sidebar = Sidebar::default();
        let path1 = PathBuf::from("/test/dir1");
        let path2 = PathBuf::from("/test/dir2");
        let path3 = PathBuf::from("/test/dir3");

        sidebar.push_to_history(path1.clone());
        sidebar.push_to_history(path2.clone());

        // Go back
        let _ = sidebar.go_back();
        assert!(sidebar.can_go_forward());

        // Push a new path - should clear forward history
        sidebar.push_to_history(path3.clone());
        assert!(!sidebar.can_go_forward());
        assert!(sidebar.can_go_back());
    }
}
