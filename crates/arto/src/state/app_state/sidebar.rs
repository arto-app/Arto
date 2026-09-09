use super::{AppState, FocusedPanel};
use crate::bookmarks::BOOKMARKS;
use crate::roots::{Origin, Roots};
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
    /// The tree of roots — the places kept, and the folder this window is in.
    #[default]
    Places,
    Recent,
    Starred,
}

impl Face {
    /// The faces in the order the rail draws them, which is the order the
    /// keyboard steps through them.
    pub const ORDER: [Face; 3] = [Face::Places, Face::Starred, Face::Recent];

    /// The next face along, wrapping. `forward` is down the rail.
    pub fn step(self, forward: bool) -> Face {
        let at = Self::ORDER
            .iter()
            .position(|face| *face == self)
            .unwrap_or(0);
        let len = Self::ORDER.len();
        let next = if forward {
            (at + 1) % len
        } else {
            (at + len - 1) % len
        };
        Self::ORDER[next]
    }
}

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
    /// A face whose rows are one list with no groups in it: Starred.
    Flat,
    /// One day of the history. The same document is a row under every day it
    /// was read on — which is what the history is for — so, as with a folder
    /// that is in both of the tree's groups, the day is part of what names
    /// the row: without it a cursor could not walk from today's copy to
    /// yesterday's, because it could not tell them apart.
    Day(crate::visits::Bucket),
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
    /// Which of the three faces the panel is showing.
    pub face: Face,
    /// The directories opened, by the row that opened them — see [`TreeRow`].
    pub expanded_dirs: HashSet<TreeRow>,
    /// The history groups the reader folded away, by heading.
    ///
    /// Beside `expanded_dirs` rather than inside the Recent face, for the same
    /// reason: what is folded decides which rows are drawn, and the keyboard
    /// cursor has to walk the rows that are drawn.
    pub recent_collapsed: HashSet<String>,
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
            recent_collapsed: HashSet::new(),
            width: 280.0,
            show_all_files: false,
            zoom_level: 1.0,
        }
    }
}

impl Sidebar {
    /// Fold a history group away, or open it again.
    pub fn toggle_group(&mut self, heading: &str) {
        if !self.recent_collapsed.remove(heading) {
            self.recent_collapsed.insert(heading.to_string());
        }
    }

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
    /// Whether the panel is on screen, pinned beside the document or peeking
    /// over it.
    pub fn panel_is_showing(&self) -> bool {
        (self.sidebar.read().pinned && self.visible_chrome().panel)
            || *self.left_hover_active.read()
    }

    /// Open a document the reader picked out of the panel.
    ///
    /// Not quite the same act as opening a document: the panel is the other
    /// half of it. Some readers work down the list, opening one document after
    /// another; others go to it for one thing and want the page to themselves
    /// once they have it. `sidebar.onOpen` says which.
    pub fn open_from_panel(&mut self, path: impl AsRef<std::path::Path>) {
        self.open_file(path.as_ref());
        if crate::config::CONFIG.read().sidebar.on_open == crate::config::OpenFromPanel::ClosePanel
        {
            self.hide_panel();
        }
    }

    /// Put the panel away, however it is showing.
    ///
    /// A keyboard cursor inside it goes with it: leaving the focus on rows
    /// that are no longer drawn would send the next keystroke somewhere the
    /// reader cannot see.
    pub fn hide_panel(&mut self) {
        self.sidebar.write().pinned = false;
        self.focus_content();
    }

    /// Give the keyboard back to the page, and let a peeking panel go with
    /// it.
    ///
    /// A panel peeks because the pointer is in it or the keys are; saying the
    /// keys have left without saying the peek is over leaves it on screen
    /// with nothing holding it there.
    pub fn focus_content(&mut self) {
        self.focused_panel.set(FocusedPanel::Content);
        self.left_hover_active.set(false);
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
        // one would be pointing at a row that is not drawn any more.
        if self.sidebar.peek().face != face {
            self.panel_cursor.set(None);
        }
        self.sidebar.write().face = face;
        self.show_panel();
    }

    /// Show a face and put the keyboard in it.
    ///
    /// Asking for a face by key is asking to use it, and a face that came out
    /// without the keyboard would have to be reached for a second time.
    pub fn focus_face(&mut self, face: Face) {
        self.show_face(face);
        self.focused_panel.set(FocusedPanel::Panel);
    }

    /// Step to the next face along the rail, keeping the keyboard in the
    /// panel.
    pub fn step_face(&mut self, forward: bool) {
        let next = self.sidebar.peek().face.step(forward);
        self.focus_face(next);
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
        let path = path.as_ref();
        {
            let mut sidebar = self.sidebar.write();
            let decision = sidebar.roots.decide(path, Origin::Explicit);
            sidebar.roots.apply(&decision);
        }
        self.show_face(Face::Places);
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
    fn the_faces_step_in_the_order_the_rail_draws_them() {
        assert_eq!(Face::Places.step(true), Face::Starred);
        assert_eq!(Face::Starred.step(true), Face::Recent);
        assert_eq!(Face::Recent.step(true), Face::Places);
    }

    #[test]
    fn stepping_back_is_stepping_the_other_way() {
        for face in Face::ORDER {
            assert_eq!(face.step(true).step(false), face);
        }
    }

    #[test]
    fn folding_a_history_group_is_a_toggle() {
        let mut sidebar = Sidebar::default();

        sidebar.toggle_group("Last week");
        assert!(sidebar.recent_collapsed.contains("Last week"));

        sidebar.toggle_group("Last week");
        assert!(!sidebar.recent_collapsed.contains("Last week"));
    }

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
        let root = Path::new("/w/arto");
        sidebar.expand_towards(Group::Current, root, Path::new("/w/arto/docs/api/auth.md"));

        assert!(sidebar.is_expanded(Group::Current, root, root));
        assert!(sidebar.is_expanded(Group::Current, root, Path::new("/w/arto/docs")));
        assert!(sidebar.is_expanded(Group::Current, root, Path::new("/w/arto/docs/api")));
        assert!(!sidebar.is_expanded(Group::Current, root, Path::new("/w")));
    }
}
