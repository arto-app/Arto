//! AppState extension methods for file operations in tabs.

use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Open a file, reusing NoFile tab or existing tab with the same file if possible
    /// Used when opening from sidebar or external sources
    pub fn open_file(&mut self, file: impl AsRef<Path>) {
        let file = file.as_ref();
        // Check if the file is already open in another tab
        if let Some(tab_index) = self.find_tab_with_file(file) {
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
        self.update_current_tab(|tab| {
            tab.navigate_to(file);
        });
    }

    /// Open the preferences window, or focus it if it is already open.
    ///
    /// Preferences used to be a tab, which gave a short errand the lifetime of
    /// a document and left it sitting open. It is a window now; what the
    /// "Current Settings" section needs from this window is handed over as a
    /// snapshot, since the preferences window has no `AppState` of its own.
    pub fn open_preferences(&mut self) {
        let (sidebar_width, sidebar_zoom_level, directory) = {
            let sidebar = self.sidebar.read();
            (
                sidebar.width,
                sidebar.zoom_level,
                sidebar.root_directory.clone(),
            )
        };
        let (right_sidebar_width, right_sidebar_zoom_level) = {
            let right = self.right_sidebar.read();
            (right.width, right.zoom_level)
        };
        crate::window::preferences::open_or_focus_preferences_window(
            crate::window::preferences::PreferencesSnapshot {
                window_id: dioxus::desktop::window().id(),
                sidebar_width,
                sidebar_zoom_level,
                right_sidebar_width,
                right_sidebar_zoom_level,
                directory,
            },
            *self.current_theme.read(),
        );
    }
}
