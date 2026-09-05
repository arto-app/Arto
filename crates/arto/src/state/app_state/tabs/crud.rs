//! AppState extension methods for tab CRUD operations.
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
//! - The Tab/TabContent module tests cover the underlying data structures
//!   (see `tabs/tab.rs` and `tabs/content.rs` for unit tests)

use super::content::TabContent;
use super::tab::Tab;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::PathBuf;

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
    /// If no tabs remain, closes the window — except when the last tab is a
    /// Preferences tab, in which case an empty tab is created instead.
    ///
    /// Returns `true` if the tab was closed successfully.
    /// Returns `false` if the index was out of bounds.
    ///
    /// Note: When the last tab is closed, this method also closes the window
    /// (unless it was a Preferences tab). The caller cannot distinguish between
    /// "tab closed" and "window closed" from the return value alone.
    pub fn close_tab(&mut self, index: usize) -> bool {
        let tab = self.take_tab(index);
        if let Some(tab) = tab {
            if self.tabs.read().is_empty() {
                if tab.content == TabContent::Preferences {
                    // Replace with an empty tab instead of closing the window
                    self.add_empty_tab(true);
                } else {
                    dioxus::desktop::window().close();
                }
            }
            true
        } else {
            false
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

    /// Check if the current active tab has no file (NoFile tab, Inline content, or FileError)
    /// None, Inline content, and FileError can be replaced when opening a file
    pub fn is_current_tab_no_file(&self) -> bool {
        self.current_tab()
            .map(|tab| tab.is_no_file())
            .unwrap_or(false)
    }
}
