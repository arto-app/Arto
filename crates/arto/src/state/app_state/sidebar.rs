use super::AppState;
use crate::bookmarks::BOOKMARKS;
use crate::roots::{Origin, Roots};
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Which of the tree's two groups a row belongs to.
///
/// The same folder can be in both — the window is working in a folder that is
/// also bookmarked — and then it is two rows, drawn in two places, which open
/// and shut on their own. So a row is named by its group as well as by its
/// root and its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// The one folder this window is in.
    Current,
    /// One of the folders kept, which every window has.
    Bookmark,
}

/// A row of the tree: which group it is drawn in, which root it descends
/// from, and where it is.
pub type TreeRow = (Group, PathBuf, PathBuf);

/// A row the panel's cursor can rest on: which list it is in, and what it is.
///
/// The list matters because one folder can be two rows — the window is
/// working in a folder that is also bookmarked — and a cursor that knew only
/// the path could not tell them apart, so it could never walk from the one to
/// the other.
pub type PanelRow = (Group, PathBuf);

/// Represents the state of the sidebar file explorer
#[derive(Debug, Clone, PartialEq)]
pub struct Sidebar {
    pub pinned: bool,
    /// The directories the tree is rooted at: the bookmarked places, shared by
    /// every window, and this window's own temporaries.
    pub roots: Roots,
    /// The directories opened, by the row that opened them — see [`TreeRow`].
    pub expanded_dirs: HashSet<TreeRow>,
    pub width: f64,
    pub show_all_files: bool,
    pub zoom_level: f64,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            pinned: false,
            roots: Roots::default(),
            expanded_dirs: HashSet::new(),
            width: 280.0,
            show_all_files: false,
            zoom_level: 1.0,
        }
    }
}

impl Sidebar {
    /// Whether this row's directory is open.
    pub fn is_expanded(&self, group: Group, root: &Path, path: &Path) -> bool {
        self.expanded_dirs
            .contains(&(group, root.to_path_buf(), path.to_path_buf()))
    }

    /// Open this row's directory, or close it.
    pub fn toggle_expansion(&mut self, group: Group, root: &Path, path: &Path) {
        let key = (group, root.to_path_buf(), path.to_path_buf());
        if !self.expanded_dirs.remove(&key) {
            self.expanded_dirs.insert(key);
        }
    }

    /// The root to answer a question that can only have one answer: what a new
    /// window inherits, what the state file records, what "the directory" means
    /// to something outside the tree.
    ///
    /// The most recent temporary, or failing that the first place — the one
    /// most likely to be what is being worked in.
    pub fn primary_root(&self) -> Option<&PathBuf> {
        self.roots
            .temps()
            .last()
            .or_else(|| self.roots.places().first())
    }

    /// Expand every directory between a root and `path`, so revealing a
    /// document opens the way down to it rather than only selecting it.
    pub fn expand_towards(&mut self, group: Group, root: &Path, path: &Path) {
        let mut current = path.parent();
        while let Some(dir) = current {
            self.expanded_dirs
                .insert((group, root.to_path_buf(), dir.to_path_buf()));
            if dir == root {
                break;
            }
            current = dir.parent();
        }
    }
}

impl AppState {
    /// Pin the panel beside the document, or unpin it.
    pub fn toggle_sidebar(&mut self) {
        let pinned = self.sidebar.read().pinned;
        self.sidebar.write().pinned = !pinned;
    }

    /// Take in the current bookmarked directories as the tree's places.
    ///
    /// Bookmarking a folder and giving the tree somewhere to start are the same
    /// act, so there is no second list to keep in step — only this, run
    /// whenever the bookmarks change.
    pub fn sync_places(&mut self) {
        let places = BOOKMARKS.read().places();
        self.sidebar.write().roots.set_places(places);
    }

    /// Add a directory someone pointed at.
    ///
    /// Explicit: it joins even when an existing root already covers it, since
    /// pointing at it is the whole of the intent. Only an exact duplicate is
    /// refused, and then it is revealed instead.
    pub fn add_root(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        {
            let mut sidebar = self.sidebar.write();
            let decision = sidebar.roots.decide(path, Origin::Explicit);
            sidebar.roots.apply(&decision);
        }
    }

    /// Make room in the tree for a document that is about to be opened.
    ///
    /// Implicit: a root that already covers it is expanded down to it, and
    /// only a document outside every root brings a new one in.
    pub fn reveal_in_roots(&mut self, file: &Path) {
        let mut sidebar = self.sidebar.write();
        let decision = sidebar.roots.decide(file, Origin::Implicit);
        sidebar.roots.apply(&decision);
        let root = match &decision {
            crate::roots::Decision::Reveal { root } => root.clone(),
            crate::roots::Decision::Push { root, .. } => root.clone(),
        };
        // Both sides spelled the way the tree spells them: expanding walks the
        // document's own path up to the root, and a key would not match it.
        sidebar.expand_towards(Group::Current, &root, file);
    }

    /// Toggle directory expansion state
    pub fn toggle_directory_expansion(&mut self, group: Group, root: &Path, path: &Path) {
        self.sidebar.write().toggle_expansion(group, root, path);
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
        let root = Path::new("/test");
        let path = Path::new("/test/dir");

        assert!(!sidebar.is_expanded(Group::Current, root, path));

        sidebar.toggle_expansion(Group::Current, root, path);
        assert!(sidebar.is_expanded(Group::Current, root, path));

        sidebar.toggle_expansion(Group::Current, root, path);
        assert!(!sidebar.is_expanded(Group::Current, root, path));
    }

    #[test]
    fn test_sidebar_toggle_multiple_paths() {
        let mut sidebar = Sidebar::default();
        let root = Path::new("/test");
        let path1 = Path::new("/test/dir1");
        let path2 = Path::new("/test/dir2");

        sidebar.toggle_expansion(Group::Current, root, path1);
        sidebar.toggle_expansion(Group::Current, root, path2);

        assert!(sidebar.is_expanded(Group::Current, root, path1));
        assert!(sidebar.is_expanded(Group::Current, root, path2));

        sidebar.toggle_expansion(Group::Current, root, path1);

        assert!(!sidebar.is_expanded(Group::Current, root, path1));
        assert!(sidebar.is_expanded(Group::Current, root, path2));
    }

    #[test]
    fn one_folder_drawn_under_two_roots_opens_once() {
        // A bookmarked folder is also a row inside the folder this window is
        // in. Opening it in one tree must not open it in the other.
        let mut sidebar = Sidebar::default();
        let shared = Path::new("/w/arto/docs");

        sidebar.toggle_expansion(Group::Bookmark, shared, shared);

        assert!(sidebar.is_expanded(Group::Bookmark, shared, shared));
        assert!(!sidebar.is_expanded(Group::Bookmark, Path::new("/w/arto"), shared));
    }

    #[test]
    fn the_same_folder_in_both_groups_opens_on_its_own() {
        // The window is working in a folder that is also bookmarked, so it is
        // drawn twice — and the two rows are not one row.
        let mut sidebar = Sidebar::default();
        let both = Path::new("/w/arto");

        sidebar.toggle_expansion(Group::Current, both, both);

        assert!(sidebar.is_expanded(Group::Current, both, both));
        assert!(!sidebar.is_expanded(Group::Bookmark, both, both));
    }

    /// The pinned flag on its own, which is what `AppState::toggle_sidebar`
    /// writes.
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
    fn primary_root_prefers_the_newest_temporary() {
        let sidebar = Sidebar {
            roots: Roots::new(
                vec![PathBuf::from("/place")],
                vec![PathBuf::from("/a"), PathBuf::from("/b")],
            ),
            ..Default::default()
        };

        assert_eq!(sidebar.primary_root(), Some(&PathBuf::from("/b")));
    }

    #[test]
    fn primary_root_falls_back_to_the_first_place() {
        let sidebar = Sidebar {
            roots: Roots::new(vec![PathBuf::from("/place")], Vec::new()),
            ..Default::default()
        };

        assert_eq!(sidebar.primary_root(), Some(&PathBuf::from("/place")));
    }

    #[test]
    fn primary_root_of_an_empty_tree_is_nothing() {
        assert_eq!(Sidebar::default().primary_root(), None);
    }

    #[test]
    fn expanding_towards_opens_every_directory_down_to_the_document() {
        let mut sidebar = Sidebar::default();
        let root = Path::new("/w/arto");
        sidebar.expand_towards(Group::Current, root, Path::new("/w/arto/docs/api/auth.md"));

        assert!(sidebar.is_expanded(Group::Current, root, root));
        assert!(sidebar.is_expanded(Group::Current, root, Path::new("/w/arto/docs")));
        assert!(sidebar.is_expanded(Group::Current, root, Path::new("/w/arto/docs/api")));
        assert!(!sidebar.is_expanded(Group::Current, root, Path::new("/w")));
    }
}
