use super::AppState;
use crate::components::right_sidebar::RightSidebarTab;
use crate::config::DEFAULT_RIGHT_SIDEBAR_WIDTH;
use dioxus::prelude::*;

/// Represents the state of the right sidebar panel (table of contents / search).
///
/// Mirrors [`super::Sidebar`] so both panels are shaped the same way. The
/// rendered headings live in `AppState` instead of here because they are
/// derived from the active document, not a user preference.
#[derive(Debug, Clone, PartialEq)]
pub struct RightSidebar {
    pub pinned: bool,
    pub width: f64,
    pub tab: RightSidebarTab,
    pub zoom_level: f64,
}

impl Default for RightSidebar {
    fn default() -> Self {
        Self {
            pinned: false,
            width: DEFAULT_RIGHT_SIDEBAR_WIDTH,
            tab: RightSidebarTab::default(),
            zoom_level: 1.0,
        }
    }
}

impl AppState {
    /// Toggle right sidebar between pinned (flex layout) and unpinned (overlay/hover).
    ///
    /// - Pinned: visible in flex layout, pushes content aside
    /// - Unpinned: accessible via hover as an overlay
    pub fn toggle_right_sidebar(&mut self) {
        let mut right_sidebar = self.right_sidebar.write();
        right_sidebar.pinned = !right_sidebar.pinned;
    }

    /// Set right sidebar width
    pub fn set_right_sidebar_width(&mut self, width: f64) {
        self.right_sidebar.write().width = width;
    }

    /// Set right sidebar active tab
    pub fn set_right_sidebar_tab(&mut self, tab: RightSidebarTab) {
        self.right_sidebar.write().tab = tab;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_right_sidebar_default() {
        let right_sidebar = RightSidebar::default();

        assert!(!right_sidebar.pinned);
        assert_eq!(right_sidebar.width, DEFAULT_RIGHT_SIDEBAR_WIDTH);
        assert_eq!(right_sidebar.tab, RightSidebarTab::default());
        assert_eq!(right_sidebar.zoom_level, 1.0);
    }

    /// Simulates the toggle logic from `AppState::toggle_right_sidebar()`.
    /// The actual method operates on `Signal<RightSidebar>`, but the state
    /// transition logic is identical.
    fn apply_toggle(right_sidebar: &mut RightSidebar) {
        right_sidebar.pinned = !right_sidebar.pinned;
    }

    #[test]
    fn test_right_toggle_from_unpinned_to_pinned() {
        let mut right_sidebar = RightSidebar::default();
        assert!(!right_sidebar.pinned);

        apply_toggle(&mut right_sidebar);
        assert!(right_sidebar.pinned);
    }

    #[test]
    fn test_right_toggle_from_pinned_to_unpinned() {
        let mut right_sidebar = RightSidebar {
            pinned: true,
            ..Default::default()
        };

        apply_toggle(&mut right_sidebar);
        assert!(!right_sidebar.pinned);
    }

    #[test]
    fn test_right_toggle_full_cycle() {
        let mut right_sidebar = RightSidebar::default();

        apply_toggle(&mut right_sidebar);
        assert!(right_sidebar.pinned);

        apply_toggle(&mut right_sidebar);
        assert!(!right_sidebar.pinned);

        apply_toggle(&mut right_sidebar);
        assert!(right_sidebar.pinned);

        apply_toggle(&mut right_sidebar);
        assert!(!right_sidebar.pinned);
    }
}
