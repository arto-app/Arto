//! AppState extension methods for scroll position and history navigation.

use super::content::TabContent;
use crate::state::AppState;
use dioxus::prelude::*;

impl AppState {
    /// Save current scroll position and go back in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for back navigation from UI (menu/header buttons).
    /// It saves the current scroll position before navigating so forward can restore it.
    pub fn save_scroll_and_go_back(&mut self) -> bool {
        let position = *self.current_scroll_position.read();
        self.save_current_scroll_position(position);
        self.go_back_in_history()
    }

    /// Save current scroll position and go forward in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for forward navigation from UI (menu/header buttons).
    pub fn save_scroll_and_go_forward(&mut self) -> bool {
        let position = *self.current_scroll_position.read();
        self.save_current_scroll_position(position);
        self.go_forward_in_history()
    }

    /// Go back in history for the current tab.
    /// Returns true if navigation occurred.
    ///
    /// Sets pending_scroll_position to restore scroll position after navigation.
    /// Note: Prefer `save_scroll_and_go_back` for UI-triggered navigation.
    pub fn go_back_in_history(&mut self) -> bool {
        let active_index = *self.active_tab.read();

        // First, get the target entry info without modifying content
        let target = {
            let mut tabs = self.tabs.write();
            tabs.get_mut(active_index).and_then(|tab| {
                tab.history
                    .go_back()
                    .map(|entry| (entry.path.clone(), entry.scroll_position))
            })
        };

        if let Some((path, scroll)) = target {
            tracing::debug!(
                ?path,
                scroll,
                "go_back_in_history: restoring scroll position"
            );
            // Set pending scroll BEFORE changing content
            // This ensures FileViewer sees the scroll position when it loads
            self.pending_scroll_position.set(Some(scroll));

            // Now change content, which triggers re-render
            let mut tabs = self.tabs.write();
            if let Some(tab) = tabs.get_mut(active_index) {
                tab.content = TabContent::File(path);
            }
            return true;
        }
        false
    }

    /// Go forward in history for the current tab.
    /// Returns true if navigation occurred.
    ///
    /// Sets pending_scroll_position to restore scroll position after navigation.
    /// Note: Prefer `save_scroll_and_go_forward` for UI-triggered navigation.
    pub fn go_forward_in_history(&mut self) -> bool {
        let active_index = *self.active_tab.read();

        // First, get the target entry info without modifying content
        let target = {
            let mut tabs = self.tabs.write();
            tabs.get_mut(active_index).and_then(|tab| {
                tab.history
                    .go_forward()
                    .map(|entry| (entry.path.clone(), entry.scroll_position))
            })
        };

        if let Some((path, scroll)) = target {
            tracing::debug!(
                ?path,
                scroll,
                "go_forward_in_history: restoring scroll position"
            );
            // Set pending scroll BEFORE changing content
            // This ensures FileViewer sees the scroll position when it loads
            self.pending_scroll_position.set(Some(scroll));

            // Now change content, which triggers re-render
            let mut tabs = self.tabs.write();
            if let Some(tab) = tabs.get_mut(active_index) {
                tab.content = TabContent::File(path);
            }
            return true;
        }
        false
    }

    /// Save the current scroll position to the current history entry.
    ///
    /// Call this before navigating away to preserve scroll position for back/forward.
    pub fn save_current_scroll_position(&mut self, scroll: f64) {
        self.update_current_tab(|tab| {
            let current_path = tab.history.current_path().map(|p| p.to_path_buf());
            tracing::debug!(
                ?current_path,
                scroll,
                "Saving scroll position to history entry"
            );
            tab.history.save_scroll_position(scroll);
        });
    }
}
