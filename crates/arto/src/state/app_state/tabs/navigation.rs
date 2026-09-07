//! AppState extension methods for tab navigation.

use crate::scroll_anchor::ScrollAnchor;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::Path;

impl AppState {
    /// Switch to a specific tab by index.
    ///
    /// Preserves scroll position: saves the current tab's scroll position to its
    /// history entry, and sets up the target tab's scroll position for restoration.
    /// This ensures that switching between tabs doesn't reset scroll to the top.
    pub fn switch_to_tab(&mut self, index: usize) {
        let tabs = self.tabs.read();
        if index >= tabs.len() {
            return;
        }
        let current_index = *self.active_tab.read();

        // Extract target tab info before dropping tabs lock to avoid race conditions
        let target_tab = &tabs[index];
        let target_scroll = if target_tab.is_no_file() {
            // Non-file tabs (Preferences, inline content, etc.) don't have scroll restoration
            None
        } else {
            // File tabs: restore saved scroll position
            Some(
                target_tab
                    .history
                    .current()
                    .map(|entry| entry.scroll_anchor)
                    .unwrap_or(ScrollAnchor::TOP),
            )
        };
        drop(tabs);

        if index == current_index {
            return;
        }

        // Save current scroll position to the departing tab's history
        let scroll = *self.current_scroll_anchor.read();
        self.update_current_tab(|tab| {
            tab.history.save_scroll_anchor(scroll);
        });

        // Set pending scroll position for target tab (None for non-file tabs)
        self.pending_scroll_anchor.set(target_scroll);

        self.active_tab.set(index);
    }

    /// Find the index of a tab that has the specified file open
    pub fn find_tab_with_file(&self, file: impl AsRef<Path>) -> Option<usize> {
        let file = file.as_ref();
        let tabs = self.tabs.read();
        tabs.iter()
            .position(|tab| tab.file().map(|f| f == file).unwrap_or(false))
    }
}
