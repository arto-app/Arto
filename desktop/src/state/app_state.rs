use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::components::right_sidebar::RightSidebarTab;
use crate::config::DEFAULT_RIGHT_SIDEBAR_WIDTH;
use crate::markdown::HeadingInfo;
use crate::pinned_search::PinnedSearchId;
use crate::theme::Theme;

mod focused_panel;
mod sidebar;
pub(crate) mod sidebar_cursor;
mod tabs;

pub use focused_panel::*;
pub use sidebar::Sidebar;
pub use tabs::{Tab, TabContent};

/// Information about a single search match for display in the Search tab.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    /// 0-based index of this match
    pub index: usize,
    /// The matched text itself
    pub text: String,
    /// Surrounding context including the match
    pub context: String,
    /// Start position of match within context (byte index)
    pub context_start: usize,
    /// End position of match within context (byte index)
    pub context_end: usize,
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
    /// Whether the content area ignores the markdown body's max-width and fills the pane.
    pub content_full_width: Signal<bool>,
    pub sidebar: Signal<Sidebar>,
    pub right_sidebar_pinned: Signal<bool>,
    pub right_sidebar_width: Signal<f64>,
    pub right_sidebar_tab: Signal<RightSidebarTab>,
    pub right_sidebar_zoom_level: Signal<f64>,
    pub right_sidebar_headings: Signal<Vec<HeadingInfo>>,
    pub position: Signal<LogicalPosition<i32>>,
    pub size: Signal<LogicalSize<u32>>,
    // Search state (not persisted, managed via JavaScript for IME compatibility)
    pub search_open: Signal<bool>,
    pub search_match_count: Signal<usize>,
    pub search_current_index: Signal<usize>,
    /// Initial search text to populate when opening search bar
    pub search_initial_text: Signal<Option<String>>,
    /// Current search query string (for display in Search tab)
    pub search_query: Signal<Option<String>>,
    /// All search matches with context (for Search tab display)
    pub search_matches: Signal<Vec<SearchMatch>>,
    /// Pinned search matches by ID (for Search tab display)
    pub pinned_matches: Signal<HashMap<PinnedSearchId, Vec<SearchMatch>>>,
    /// Pending scroll position to restore after navigation (for back/forward).
    /// When Some, FileViewer will scroll to this position instead of resetting to top.
    pub pending_scroll_position: Signal<Option<f64>>,
    /// Current scroll position of the content area.
    /// Updated by scroll events, used to save position before back/forward navigation.
    pub current_scroll_position: Signal<f64>,
    /// Reload trigger counter. Incrementing this forces FileViewer to re-read the file
    /// from disk without going through the use_memo PartialEq gate in content.rs.
    /// Used by manual reload (header button, tab context menu) and file watcher.
    pub reload_trigger: Signal<usize>,
    /// Which panel currently has keyboard focus (for context-aware keybindings).
    pub focused_panel: Signal<FocusedPanel>,
    /// Keyboard cursor position in the left sidebar file tree.
    pub sidebar_cursor: Signal<Option<PathBuf>>,
    /// Keyboard cursor position in the right sidebar TOC (index into headings list).
    pub toc_cursor: Signal<Option<usize>>,
    /// Keyboard cursor position in the Quick Access list (index into bookmarks).
    pub quick_access_cursor: Signal<Option<usize>>,
    /// Whether the left sidebar overlay is currently shown (hover/focus triggered).
    /// Transient UI state — not persisted.
    pub left_hover_active: Signal<bool>,
    /// Whether the right sidebar overlay is currently shown (hover/focus triggered).
    /// Transient UI state — not persisted.
    pub right_hover_active: Signal<bool>,
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
            content_full_width: Signal::new(false),
            sidebar: Signal::new(Sidebar::default()),
            right_sidebar_pinned: Signal::new(false),
            right_sidebar_width: Signal::new(DEFAULT_RIGHT_SIDEBAR_WIDTH),
            right_sidebar_tab: Signal::new(RightSidebarTab::default()),
            right_sidebar_zoom_level: Signal::new(1.0),
            right_sidebar_headings: Signal::new(Vec::new()),
            position: Signal::new(Default::default()),
            size: Signal::new(Default::default()),
            // Search state
            search_open: Signal::new(false),
            search_match_count: Signal::new(0),
            search_current_index: Signal::new(0),
            search_initial_text: Signal::new(None),
            search_query: Signal::new(None),
            search_matches: Signal::new(Vec::new()),
            pinned_matches: Signal::new(HashMap::new()),
            pending_scroll_position: Signal::new(None),
            current_scroll_position: Signal::new(0.0),
            reload_trigger: Signal::new(0),
            focused_panel: Signal::new(FocusedPanel::Content),
            sidebar_cursor: Signal::new(None),
            toc_cursor: Signal::new(None),
            quick_access_cursor: Signal::new(None),
            left_hover_active: Signal::new(false),
            right_hover_active: Signal::new(false),
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

    /// Toggle full-width content mode, which lets the markdown body ignore its
    /// max-width and fill the entire content pane.
    pub fn toggle_content_full_width(&mut self) {
        let was_full_width = *self.content_full_width.read();
        self.content_full_width.set(!was_full_width);
    }

    /// Toggle right sidebar between pinned (flex layout) and unpinned (overlay/hover).
    ///
    /// - Pinned: visible in flex layout, pushes content aside
    /// - Unpinned: accessible via hover as an overlay
    pub fn toggle_right_sidebar(&mut self) {
        let was_pinned = *self.right_sidebar_pinned.read();
        self.right_sidebar_pinned.set(!was_pinned);
    }

    /// Set right sidebar width
    pub fn set_right_sidebar_width(&mut self, width: f64) {
        self.right_sidebar_width.set(width);
    }

    /// Set right sidebar active tab
    pub fn set_right_sidebar_tab(&mut self, tab: RightSidebarTab) {
        self.right_sidebar_tab.set(tab);
    }

    /// Toggle search bar visibility
    ///
    /// Note: Does NOT clear search state when closing. Search highlights and
    /// results persist until the user explicitly clears them (via clear button)
    /// or the content changes. This enables the "persistent highlighting" feature.
    pub fn toggle_search(&mut self) {
        let new_state = !*self.search_open.read();
        self.search_open.set(new_state);
    }

    /// Update search results from JavaScript callback (basic count/current only)
    pub fn update_search_results(&mut self, count: usize, current: usize) {
        self.search_match_count.set(count);
        self.search_current_index.set(current);
    }

    /// Update full search results from JavaScript callback (includes match details)
    pub fn update_search_results_full(
        &mut self,
        query: Option<String>,
        count: usize,
        current: usize,
        matches: Vec<SearchMatch>,
    ) {
        self.search_query.set(query);
        self.search_match_count.set(count);
        self.search_current_index.set(current);
        self.search_matches.set(matches);
    }

    /// Open search bar and populate with given text
    pub fn open_search_with_text(&mut self, text: Option<String>) {
        // Set initial text for SearchBar to pick up
        self.search_initial_text.set(text);
        // Open search bar
        self.search_open.set(true);
    }

    /// Update pinned search matches from JavaScript callback
    pub fn update_pinned_matches(&mut self, matches: HashMap<PinnedSearchId, Vec<SearchMatch>>) {
        self.pinned_matches.set(matches);
    }
}
