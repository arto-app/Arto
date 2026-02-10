use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::right_sidebar::RightSidebarTab;
use crate::config::DEFAULT_RIGHT_SIDEBAR_WIDTH;
use crate::finder::{DirectorySearchMatch, FinderMode};
use crate::markdown::HeadingInfo;
use crate::theme::Theme;

mod sidebar;
mod tabs;

pub use sidebar::Sidebar;
pub use tabs::{OpenFileOptions, Tab, TabContent};

/// Scroll target specification for FileViewer.
///
/// Unifies pixel-based scrolling (for tab switching/history) and line-based scrolling
/// (for search results/jump-to-line features).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PendingScroll {
    /// Scroll to a specific pixel position (for tab switching, back/forward navigation)
    Position(f64),
    /// Scroll to a specific line number (for search results, jump-to-line)
    Line(usize),
}

/// Per-window application state.
///
/// # Copy Semantics
///
/// This struct implements `Copy` because all fields are `Signal<T>`, which are cheap to copy
/// (they contain only Arc pointers internally). This allows passing `AppState` to closures
/// and async blocks without explicit `.clone()` calls, making the code cleaner.
///
/// **This aligns with Dioxus design philosophy**: `Signal<T>` is intentionally `Copy` to enable
/// ergonomic state passing in reactive UIs. Wrapping `Signal` fields in a `Copy` struct is the
/// recommended pattern in Dioxus applications.
///
/// # Why Per-field Signals?
///
/// We use per-field `Signal<T>` instead of `Signal<AppState>` for fine-grained reactivity:
/// - Changing `current_theme` doesn't trigger re-renders in components that only watch `tabs`
/// - Different components can update different fields concurrently without conflicts
/// - Components subscribe only to the fields they need (e.g., Header watches theme, TabBar watches tabs)
///
/// If we used `Signal<AppState>`, any field change would trigger re-renders in ALL components
/// that access the state, causing unnecessary performance overhead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppState {
    pub tabs: Signal<Vec<Tab>>,
    pub active_tab: Signal<usize>,
    pub current_theme: Signal<Theme>,
    pub zoom_level: Signal<f64>,
    pub sidebar: Signal<Sidebar>,
    pub right_sidebar_open: Signal<bool>,
    pub right_sidebar_width: Signal<f64>,
    pub right_sidebar_tab: Signal<RightSidebarTab>,
    pub right_sidebar_zoom_level: Signal<f64>,
    pub right_sidebar_headings: Signal<Vec<HeadingInfo>>,
    pub position: Signal<LogicalPosition<i32>>,
    pub size: Signal<LogicalSize<u32>>,
    // Finder state (Fuzzy Finder overlay)
    pub finder_open: Signal<bool>,
    pub finder_mode: Signal<FinderMode>,
    /// Selected index in the Finder results list (for the active mode)
    pub finder_selected_index: Signal<usize>,
    /// Saved selected index for the inactive mode (swapped on mode switch)
    pub finder_saved_selected_index: Signal<usize>,
    /// Query string for directory-wide search (separate from in-file search)
    pub directory_search_query: Signal<Option<String>>,
    /// Results from directory-wide fuzzy search
    pub directory_search_results: Signal<Vec<DirectorySearchMatch>>,
    /// Whether directory search is currently indexing/searching
    pub directory_search_loading: Signal<bool>,
    /// Index of the selected search result (for highlighting active match)
    pub selected_search_result_index: Signal<Option<usize>>,
    /// Initial search text to populate when opening finder
    pub search_initial_text: Signal<Option<String>>,
    /// Pending scroll target for FileViewer.
    /// Unifies pixel position (tab switching) and line number (search jump).
    pub pending_scroll: Signal<Option<PendingScroll>>,
    /// Current scroll position of the content area.
    /// Updated by scroll events, used to save position before back/forward navigation.
    pub current_scroll_position: Signal<f64>,
    /// Last non-empty search results for dimmed sidebar display.
    /// Persists in AppState (not in SearchTab hooks) so it survives
    /// component unmount/remount when right sidebar tabs switch.
    pub sidebar_last_search: Signal<Option<(String, Vec<DirectorySearchMatch>)>>,
}

impl AppState {
    /// Create a new AppState with the specified theme.
    /// Used when creating windows with specific initial state.
    pub fn new(theme: Theme) -> Self {
        Self {
            tabs: Signal::new(vec![Tab::default()]),
            active_tab: Signal::new(0),
            current_theme: Signal::new(theme),
            zoom_level: Signal::new(1.0),
            sidebar: Signal::new(Sidebar::default()),
            right_sidebar_open: Signal::new(false),
            right_sidebar_width: Signal::new(DEFAULT_RIGHT_SIDEBAR_WIDTH),
            right_sidebar_tab: Signal::new(RightSidebarTab::default()),
            right_sidebar_zoom_level: Signal::new(1.0),
            right_sidebar_headings: Signal::new(Vec::new()),
            position: Signal::new(Default::default()),
            size: Signal::new(Default::default()),
            // Finder state
            finder_open: Signal::new(false),
            finder_mode: Signal::new(FinderMode::default()),
            finder_selected_index: Signal::new(0),
            finder_saved_selected_index: Signal::new(0),
            directory_search_query: Signal::new(None),
            directory_search_results: Signal::new(Vec::new()),
            directory_search_loading: Signal::new(false),
            selected_search_result_index: Signal::new(None),
            search_initial_text: Signal::new(None),
            pending_scroll: Signal::new(None),
            current_scroll_position: Signal::new(0.0),
            sidebar_last_search: Signal::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl AppState {
    /// Set the root directory and add to history
    /// Note: The directory is persisted to state file when window closes
    pub fn set_root_directory(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        let mut sidebar = self.sidebar.write();
        sidebar.root_directory = Some(path.clone());
        sidebar.expanded_dirs.clear();
        sidebar.push_to_history(path);
    }

    /// Set the root directory without adding to history (used for history navigation)
    fn set_root_directory_no_history(&mut self, path: PathBuf) {
        let mut sidebar = self.sidebar.write();
        sidebar.root_directory = Some(path);
        sidebar.expanded_dirs.clear();
    }

    /// Go back in directory history
    pub fn go_back_directory(&mut self) {
        let path = self.sidebar.write().go_back();
        if let Some(path) = path {
            self.set_root_directory_no_history(path);
        }
    }

    /// Go forward in directory history
    pub fn go_forward_directory(&mut self) {
        let path = self.sidebar.write().go_forward();
        if let Some(path) = path {
            self.set_root_directory_no_history(path);
        }
    }

    /// Navigate to parent directory
    pub fn go_to_parent_directory(&mut self) {
        let parent = {
            let sidebar = self.sidebar.read();
            sidebar
                .root_directory
                .as_ref()
                .and_then(|d| d.parent().map(|p| p.to_path_buf()))
        };
        if let Some(parent) = parent {
            self.set_root_directory(parent);
        }
    }

    /// Toggle right sidebar visibility
    pub fn toggle_right_sidebar(&mut self) {
        let new_state = !*self.right_sidebar_open.read();
        self.right_sidebar_open.set(new_state);
    }

    /// Set right sidebar width
    pub fn set_right_sidebar_width(&mut self, width: f64) {
        self.right_sidebar_width.set(width);
    }

    /// Set right sidebar active tab
    pub fn set_right_sidebar_tab(&mut self, tab: RightSidebarTab) {
        self.right_sidebar_tab.set(tab);
    }

    /// Toggle finder visibility
    ///
    /// Note: Does NOT clear search state when closing. Search highlights and
    /// results persist until the user explicitly clears them (via clear button)
    /// or the content changes. This enables the "persistent highlighting" feature.
    pub fn toggle_finder(&mut self) {
        let new_state = !*self.finder_open.read();
        self.finder_open.set(new_state);
    }

    /// Check if the given finder mode has valid search context.
    ///
    /// - File: requires the active tab to be a file
    /// - Directory: requires a root directory in the sidebar
    pub fn can_use_finder_mode(&self, mode: FinderMode) -> bool {
        match mode {
            FinderMode::File => {
                let tabs = self.tabs.read();
                let active = *self.active_tab.read();
                tabs.get(active).and_then(|t| t.file()).is_some()
            }
            FinderMode::Directory => self.sidebar.read().root_directory.is_some(),
        }
    }

    /// Open finder in the last-used mode.
    ///
    /// If the last-used mode is no longer valid (e.g., no file open for File
    /// mode), falls back to the other mode. No-op if neither mode is available.
    pub fn open_finder(&mut self) {
        let current = *self.finder_mode.read();
        if self.can_use_finder_mode(current) {
            self.finder_open.set(true);
        } else {
            // Fall back to the other mode
            let fallback = match current {
                FinderMode::File => FinderMode::Directory,
                FinderMode::Directory => FinderMode::File,
            };
            if self.can_use_finder_mode(fallback) {
                self.finder_mode.set(fallback);
                self.finder_selected_index.set(0);
                self.finder_open.set(true);
            }
        }
    }

    /// Switch finder mode, swapping the per-mode selected index.
    ///
    /// No-op if the target mode has no valid search context.
    pub fn switch_finder_mode(&mut self) {
        let current = *self.finder_mode.read();
        let new_mode = match current {
            FinderMode::File => FinderMode::Directory,
            FinderMode::Directory => FinderMode::File,
        };
        if !self.can_use_finder_mode(new_mode) {
            return;
        }
        // Swap active ↔ saved selected index
        let active = *self.finder_selected_index.read();
        let saved = *self.finder_saved_selected_index.read();
        self.finder_saved_selected_index.set(active);
        self.finder_selected_index.set(saved);
        self.finder_mode.set(new_mode);
    }

    /// Close finder (results are preserved for persistent highlighting)
    pub fn close_finder(&mut self) {
        // Snapshot current search results for dimmed sidebar display.
        // Captured once on close so SearchTab can show stale results
        // without subscribing to the frequently-updating result signals.
        if let Some(q) = self.directory_search_query.read().as_ref() {
            if !q.is_empty() {
                let results = self.directory_search_results.read().clone();
                self.sidebar_last_search.set(Some((q.clone(), results)));
            }
        }
        self.finder_open.set(false);
    }

    /// Update directory search results from background search
    pub fn update_directory_search_results(
        &mut self,
        query: Option<String>,
        results: Vec<DirectorySearchMatch>,
    ) {
        self.directory_search_query.set(query);
        self.directory_search_results.set(results);
        self.directory_search_loading.set(false);
        // Clear search state when updating results (new search)
        self.selected_search_result_index.set(None);
        self.pending_scroll.set(None);
    }

    /// Set the selected search result index for highlighting active match
    pub fn set_selected_search_result_index(&mut self, index: Option<usize>) {
        self.selected_search_result_index.set(index);
    }

    /// Open finder and populate with given text (defaults to File mode)
    pub fn open_finder_with_text(&mut self, text: Option<String>) {
        // Set initial text for FuzzyFinder to pick up
        self.search_initial_text.set(text);
        // Open finder in current file mode
        self.finder_open.set(true);
    }
}
