//! AppState extension methods for bulk tab operations.

use super::content::TabContent;
use crate::state::AppState;
use dioxus::prelude::*;

impl AppState {
    /// Close all tabs except the one at `keep_index`.
    /// Pinned tabs are always preserved (not closed).
    /// Switches active tab to the kept tab after closing.
    pub fn close_others(&mut self, keep_index: usize) {
        let mut tabs = self.tabs.write();
        if keep_index >= tabs.len() {
            return;
        }

        // Collect tabs to retain: pinned tabs + the kept tab
        let mut new_tabs = Vec::new();
        let mut new_active = 0;
        for (i, tab) in tabs.drain(..).enumerate() {
            if tab.pinned || i == keep_index {
                if i == keep_index {
                    new_active = new_tabs.len();
                }
                new_tabs.push(tab);
            }
        }
        *tabs = new_tabs;
        drop(tabs);
        self.active_tab.set(new_active);
    }

    /// Close all unpinned tabs.
    /// If no tabs remain after closing, closes the window.
    /// Preserves the current active tab if it's pinned.
    pub fn close_all_unpinned(&mut self) {
        // Capture current active tab before modification
        let current_active = *self.active_tab.read();

        let mut tabs = self.tabs.write();

        // Calculate new index for the current active tab if it's pinned
        let mut preserved_active_index: Option<usize> = None;
        if current_active < tabs.len() && tabs[current_active].pinned {
            let mut pinned_before_or_at_active = 0usize;
            for (i, tab) in tabs.iter().enumerate() {
                if tab.pinned {
                    if i == current_active {
                        preserved_active_index = Some(pinned_before_or_at_active);
                        break;
                    }
                    pinned_before_or_at_active += 1;
                }
            }
        }

        tabs.retain(|tab| tab.pinned);

        if tabs.is_empty() {
            drop(tabs);
            dioxus::desktop::window().close();
        } else {
            // Preserve the current active tab if it's pinned; otherwise, fall back to last pinned tab
            let new_active = preserved_active_index.unwrap_or_else(|| tabs.len().saturating_sub(1));
            drop(tabs);
            self.active_tab.set(new_active);
        }
    }

    /// Toggle pin state of the tab at `index`.
    /// When pinning, the tab moves to the end of the pinned group (left side).
    /// When unpinning, the tab moves to the start of the unpinned group.
    /// Only updates active_tab if the toggled tab was currently active.
    pub fn toggle_pin(&mut self, index: usize) {
        // Capture current active tab before modification
        let current_active = *self.active_tab.read();

        let mut tabs = self.tabs.write();
        if index >= tabs.len() {
            return;
        }

        let is_pinned = tabs[index].pinned;
        tabs[index].pinned = !is_pinned;

        // Move tab to appropriate position
        let tab = tabs.remove(index);
        let pinned_count = tabs.iter().filter(|t| t.pinned).count();

        let new_index = if tab.pinned {
            // Pinning: insert at end of pinned group
            pinned_count
        } else {
            // Unpinning: insert at start of unpinned group
            pinned_count
        };
        tabs.insert(new_index, tab);

        drop(tabs);

        // Only update active_tab if we toggled the currently active tab
        if index == current_active {
            self.active_tab.set(new_index);
        } else {
            // Adjust current active tab index based on remove/insert effects
            let adjusted_active = if index < current_active && new_index >= current_active {
                // Tab moved from before to at-or-after current -> shift current left
                current_active.saturating_sub(1)
            } else if index > current_active && new_index <= current_active {
                // Tab moved from after to at-or-before current -> shift current right
                current_active + 1
            } else {
                // No adjustment needed
                current_active
            };
            self.active_tab.set(adjusted_active);
        }
    }

    /// Reload the current tab's file from disk, preserving scroll position.
    ///
    /// Increments the `reload_trigger` counter which FileViewer subscribes to
    /// via Dioxus auto-subscription. This bypasses the `use_memo` `PartialEq`
    /// gate in `content.rs` that would otherwise block re-renders when the
    /// `PathBuf` hasn't changed.
    ///
    /// For `TabContent::FileError` tabs, switches back to `TabContent::File`
    /// to allow retrying the load (without pushing to history).
    ///
    /// No-op for non-file tabs (Preferences, Inline, NoFile, etc.).
    pub fn reload_current_tab(&mut self) {
        if self.current_tab().is_some_and(|tab| tab.file().is_some()) {
            // If current tab is FileError, switch back to File to allow retry
            self.update_current_tab(|tab| {
                if let TabContent::FileError(path, _) = &tab.content {
                    tab.content = TabContent::File(path.clone());
                }
            });

            // Save current scroll position so it can be restored after reload
            let scroll = *self.current_scroll_position.read();
            self.pending_scroll_position.set(Some(scroll));
            let current = *self.reload_trigger.read();
            self.reload_trigger.set(current + 1);
        } else {
            tracing::debug!("Reload requested but current tab is not a file tab");
        }
    }
}
