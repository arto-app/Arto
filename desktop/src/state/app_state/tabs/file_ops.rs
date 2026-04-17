//! AppState extension methods for file operations in tabs.

use super::content::TabContent;
use super::tab::Tab;
use crate::history::HistoryManager;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Record a file in recent history, most-recent-first.
    pub fn record_recent_file(&mut self, file: impl Into<PathBuf>) {
        const MAX_RECENT_FILES: usize = 10;

        let file = file.into();
        let mut recent_files = self.recent_files.write();
        recent_files.retain(|path| path != &file);
        recent_files.insert(0, file);
        recent_files.truncate(MAX_RECENT_FILES);
    }

    /// Open a file, reusing NoFile tab or existing tab with the same file if possible
    /// Used when opening from sidebar or external sources
    pub fn open_file(&mut self, file: impl AsRef<Path>) {
        let file = file.as_ref().to_path_buf();
        self.record_recent_file(file.clone());

        // Check if the file is already open in another tab
        if let Some(tab_index) = self.find_tab_with_file(&file) {
            // Switch to the existing tab instead of creating a new one
            self.switch_to_tab(tab_index);
        } else if self.is_current_tab_no_file() {
            // If current tab is NoFile, open the file in it
            self.update_current_tab(|tab| {
                tab.navigate_to(file);
            });
        } else {
            // Otherwise, create a new tab
            self.add_file_tab(file, true);
        }
    }

    /// Navigate to a file in the current tab (for in-tab navigation like markdown links)
    /// Always opens in current tab regardless of whether file is open elsewhere
    pub fn navigate_to_file(&mut self, file: impl Into<PathBuf>) {
        let file = file.into();
        self.record_recent_file(file.clone());
        self.update_current_tab(|tab| {
            tab.navigate_to(file);
        });
    }

    pub fn navigate_to_file_at_heading(
        &mut self,
        file: impl Into<PathBuf>,
        heading_id: Option<String>,
    ) {
        let file = file.into();
        if let Some(heading_id) = heading_id {
            self.set_pending_heading_navigation(file.clone(), heading_id);
        } else {
            self.clear_pending_heading_navigation();
        }
        self.navigate_to_file(file);
    }

    /// Open preferences in a tab. Reuses existing preferences tab if found.
    /// Preserves pinned sidebars, but closes any transient overlay sidebars.
    pub fn open_preferences(&mut self) {
        // Close transient overlays so preferences opens into a stable layout.
        self.left_hover_active.set(false);
        self.right_hover_active.set(false);

        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        if let Some(index) = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences))
        {
            drop(tabs);
            self.switch_to_tab(index);
            return;
        }
        drop(tabs);

        // Check if current tab is empty (None, Inline, or FileError) - reuse it
        if self.is_current_tab_no_file() {
            self.update_current_tab(|tab| {
                tab.content = TabContent::Preferences;
            });
        } else {
            // Create new tab with preferences
            let mut tabs = self.tabs.write();
            tabs.push(Tab {
                content: TabContent::Preferences,
                history: HistoryManager::new(),
                pinned: false,
            });
            let new_index = tabs.len() - 1;
            drop(tabs);
            self.active_tab.set(new_index);
        }
    }

    /// Toggle preferences tab. Opens if not present, closes if currently active.
    pub fn toggle_preferences(&mut self) {
        // Check if preferences tab already exists
        let tabs = self.tabs.read();
        let preferences_index = tabs
            .iter()
            .position(|tab| matches!(tab.content, TabContent::Preferences));
        drop(tabs);

        if let Some(index) = preferences_index {
            // Preferences tab exists - check if it's the active tab
            let active_index = *self.active_tab.read();
            if active_index == index {
                // Close the preferences tab
                self.close_tab(index);
            } else {
                // Switch to the preferences tab
                self.switch_to_tab(index);
            }
        } else {
            // No preferences tab - open new one
            self.open_preferences();
        }
    }
}
