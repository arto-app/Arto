//! AppState extension methods for tab management.
//!
//! # Testing Note
//!
//! These methods are NOT unit tested because:
//!
//! 1. AppState uses Dioxus Signals (`Signal<T>`), which require a Dioxus runtime
//! 2. Signal operations (read/write) panic outside of Dioxus components
//! 3. Creating a Dioxus runtime for unit tests adds significant complexity
//!
//! These methods are tested via:
//! - Integration tests that run within a Dioxus app context
//! - Manual testing through the UI
//! - The Tab/TabContent tests cover the underlying data structures

use super::content::TabContent;
use super::tab::Tab;
use crate::history::HistoryManager;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Get a tab by index (returns a clone)
    ///
    /// Used in Prepare phase of Two-Phase Commit.
    /// Note: Clone cost is low (~2-10 KB for typical tabs with history).
    pub fn get_tab(&self, index: usize) -> Option<Tab> {
        self.tabs.read().get(index).cloned()
    }

    /// Get a read-only copy of the current active tab
    pub fn current_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.read();
        let active_index = *self.active_tab.read();
        tabs.get(active_index).cloned()
    }

    /// Update the current active tab using a closure
    pub fn update_current_tab<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut Tab),
    {
        let active_index = *self.active_tab.read();
        let mut tabs = self.tabs.write();

        if let Some(tab) = tabs.get_mut(active_index) {
            update_fn(tab);
        }
    }

    /// Close a tab at index.
    /// If no tabs remain, closes the window.
    pub fn close_tab(&mut self, index: usize) {
        if self.take_tab(index).is_some() {
            // Close window if no tabs remain
            if self.tabs.read().is_empty() {
                dioxus::desktop::window().close();
            }
        }
    }

    /// Remove a tab at index and return it.
    /// Unlike close_tab, does NOT close the window if no tabs remain.
    /// Used for drag operations where the tab may be re-inserted.
    pub fn take_tab(&mut self, index: usize) -> Option<Tab> {
        let mut tabs = self.tabs.write();

        if index >= tabs.len() {
            return None;
        }

        let tab = tabs.remove(index);

        // Update active tab index
        let current_active = *self.active_tab.read();
        let new_active = match current_active.cmp(&index) {
            std::cmp::Ordering::Greater => current_active - 1,
            std::cmp::Ordering::Equal if current_active >= tabs.len() => {
                tabs.len().saturating_sub(1)
            }
            _ => current_active,
        };

        if new_active != current_active && !tabs.is_empty() {
            drop(tabs); // Release borrow before updating
            self.active_tab.set(new_active);
        }

        Some(tab)
    }

    /// Insert tab at specified position
    /// Returns the index where the tab was inserted
    pub fn insert_tab(&mut self, tab: Tab, index: usize) -> usize {
        let mut tabs = self.tabs.write();
        let insert_index = index.min(tabs.len()); // Clamp to valid range
        tabs.insert(insert_index, tab);
        insert_index
    }

    /// Add a tab and optionally switch to it
    pub fn add_tab(&mut self, tab: Tab, switch_to: bool) -> usize {
        let tabs_len = self.tabs.read().len();
        let index = self.insert_tab(tab, tabs_len);
        if switch_to {
            self.switch_to_tab(index);
        }
        index
    }

    /// Add a file tab and optionally switch to it
    pub fn add_file_tab(&mut self, file: impl Into<PathBuf>, switch_to: bool) -> usize {
        self.add_tab(Tab::new(file.into()), switch_to)
    }

    /// Add an empty tab and optionally switch to it
    pub fn add_empty_tab(&mut self, switch_to: bool) -> usize {
        self.add_tab(Tab::default(), switch_to)
    }

    /// Switch to a specific tab by index
    pub fn switch_to_tab(&mut self, index: usize) {
        let tabs = self.tabs.read();
        if index < tabs.len() {
            self.active_tab.set(index);
        }
    }

    /// Check if the current active tab has no file (NoFile tab, Inline content, or FileError)
    /// None, Inline content, and FileError can be replaced when opening a file
    pub fn is_current_tab_no_file(&self) -> bool {
        self.current_tab()
            .map(|tab| tab.is_no_file())
            .unwrap_or(false)
    }

    /// Find the index of a tab that has the specified file open
    pub fn find_tab_with_file(&self, file: impl AsRef<Path>) -> Option<usize> {
        let file = file.as_ref();
        let tabs = self.tabs.read();
        tabs.iter()
            .position(|tab| tab.file().map(|f| f == file).unwrap_or(false))
    }

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

    /// Open preferences in a tab. Reuses existing preferences tab if found.
    pub fn open_preferences(&mut self) {
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
