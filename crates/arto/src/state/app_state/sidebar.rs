use super::{AppState, FocusedPanel};
use crate::bookmarks::BOOKMARKS;
use crate::roots::{canonical_key, Origin, Roots};
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Which face the panel is showing.
///
/// The rail switches between them; only one is drawn at a time, and the rail
/// itself never goes away, so there is always something visible to switch
/// back with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Face {
    #[default]
    Files,
    Recent,
    Starred,
}

/// Represents the state of the sidebar file explorer
#[derive(Debug, Clone, PartialEq)]
pub struct Sidebar {
    pub pinned: bool,
    /// The directories the tree is rooted at: the bookmarked places, shared by
    /// every window, and this window's own temporaries.
    pub roots: Roots,
    /// Which of the three faces the panel is showing.
    pub face: Face,
    pub expanded_dirs: HashSet<PathBuf>,
    pub width: f64,
    pub show_all_files: bool,
    pub zoom_level: f64,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            pinned: false,
            roots: Roots::default(),
            face: Face::default(),
            expanded_dirs: HashSet::new(),
            width: 280.0,
            show_all_files: false,
            zoom_level: 1.0,
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
    pub fn expand_towards(&mut self, root: &Path, path: &Path) {
        let mut current = path.parent();
        while let Some(dir) = current {
            self.expanded_dirs.insert(dir.to_path_buf());
            if dir == root {
                break;
            }
            current = dir.parent();
        }
    }
}

impl AppState {
    /// Whether the panel is on screen, pinned beside the document or peeking
    /// over it.
    pub fn panel_is_showing(&self) -> bool {
        (self.sidebar.read().pinned && self.visible_chrome().panel)
            || *self.left_hover_active.read()
    }

    /// Put the panel away, however it is showing.
    ///
    /// A keyboard cursor inside it goes with it: leaving the focus on rows
    /// that are no longer drawn would send the next keystroke somewhere the
    /// reader cannot see.
    pub fn hide_panel(&mut self) {
        self.sidebar.write().pinned = false;
        self.left_hover_active.set(false);
        self.release_panel_focus();
    }

    /// Return the keyboard focus to the document if it is sitting in the
    /// panel.
    fn release_panel_focus(&mut self) {
        if matches!(
            *self.focused_panel.read(),
            FocusedPanel::LeftSidebar | FocusedPanel::QuickAccess
        ) {
            self.focused_panel.set(FocusedPanel::Content);
        }
    }

    /// Bring the panel out, in whichever way the width allows.
    ///
    /// A window wide enough holds it beside the document; a narrower one
    /// shows it over the document instead, so Cmd+B still opens something
    /// when the layout has folded the panel away. The pinned choice is left
    /// alone in that case, so widening the window restores it as configured.
    pub fn show_panel(&mut self) {
        if self.visible_chrome().panel {
            self.sidebar.write().pinned = true;
            self.left_hover_active.set(false);
        } else {
            self.left_hover_active.set(true);
        }
    }

    /// Toggle the panel, whichever way it is currently showing.
    pub fn toggle_sidebar(&mut self) {
        if self.panel_is_showing() {
            self.hide_panel();
        } else {
            self.show_panel();
        }
    }

    /// Show one of the panel's three faces, bringing the panel out if it is
    /// away.
    ///
    /// Asking for a face is asking to look at it, so it does not also require
    /// opening the panel first.
    pub fn show_face(&mut self, face: Face) {
        // Each face is a different list, so a cursor left over from the last
        // one would move over rows nobody can see. The face that owns the
        // focused panel keeps it; any other change hands it back.
        let keeps_focus = match *self.focused_panel.read() {
            FocusedPanel::LeftSidebar => face == Face::Files,
            FocusedPanel::QuickAccess => face == Face::Starred,
            _ => true,
        };
        if !keeps_focus {
            self.release_panel_focus();
        }

        self.sidebar.write().face = face;
        self.show_panel();
    }

    /// The rail's own gesture: show this face, or put the panel away when it
    /// is already the one showing.
    ///
    /// The same glyph that brought the panel out puts it back, so nothing
    /// else has to be found.
    pub fn toggle_face(&mut self, face: Face) {
        if self.panel_is_showing() && self.sidebar.read().face == face {
            self.hide_panel();
        } else {
            self.show_face(face);
        }
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
    ///
    /// Pointing at a folder is asking to see it, so the tree comes to the
    /// front — through [`Self::show_face`], which is what keeps the keyboard
    /// cursor and the face in step.
    pub fn add_root(&mut self, path: impl AsRef<Path>) {
        let key = canonical_key(path.as_ref());
        {
            let mut sidebar = self.sidebar.write();
            let decision = sidebar.roots.decide(&key, Origin::Explicit);
            sidebar.roots.apply(&decision);
        }
        self.show_face(Face::Files);
    }

    /// Make room in the tree for a document that is about to be opened.
    ///
    /// Implicit: a root that already covers it is expanded down to it, and
    /// only a document outside every root brings a new one in.
    pub fn reveal_in_roots(&mut self, file: &Path) {
        let key = canonical_key(file);
        let mut sidebar = self.sidebar.write();
        let decision = sidebar.roots.decide(&key, Origin::Implicit);
        sidebar.roots.apply(&decision);
        let root = match &decision {
            crate::roots::Decision::Reveal { root } => root.clone(),
            crate::roots::Decision::Push { root, .. } => root.clone(),
        };
        sidebar.expand_towards(&root, &key);
    }

    /// Drop one temporary root. Places leave by being unbookmarked instead.
    pub fn close_root(&mut self, path: &Path) {
        self.sidebar.write().roots.close_temp(path);
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

    /// The pinned flag on its own, which is what `AppState::show_panel` and
    /// `AppState::hide_panel` write. Whether a pinned panel is actually drawn
    /// also depends on the width, and that rule is tested in
    /// `crate::hooks::layout_budget`.
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
        sidebar.expand_towards(Path::new("/w/arto"), Path::new("/w/arto/docs/api/auth.md"));

        assert!(sidebar.expanded_dirs.contains(Path::new("/w/arto")));
        assert!(sidebar.expanded_dirs.contains(Path::new("/w/arto/docs")));
        assert!(sidebar
            .expanded_dirs
            .contains(Path::new("/w/arto/docs/api")));
        assert!(!sidebar.expanded_dirs.contains(Path::new("/w")));
    }
}
